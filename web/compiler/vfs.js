// Shared virtual filesystem (WP-11). One WebAssembly.Memory holds the whole tree and all file
// bytes; every worker instantiates crates/vfs on it (`Vfs.attach`) and gets synchronous access.
// Reads and writes copy straight between that memory and the caller's buffer (e.g. rustc's memory).
//
// Concurrency: one lock word in shared memory around every call (see crates/vfs/src/ffi.rs).
// Workers block on it with Atomics.wait; the page thread, which may not block, spins.
export const ROOT = 0;
/** WASI preview 1 errno names for the values crates/vfs returns. */
const ERRNO = {
    20: 'EEXIST', 28: 'EINVAL', 31: 'EISDIR', 32: 'ELOOP', 37: 'ENAMETOOLONG', 44: 'ENOENT', 54: 'ENOTDIR', 55: 'ENOTEMPTY',
};
export class VfsError extends Error {
    errno;
    constructor(errno, op, path) {
        super(`${op}${path ? ` ${path}` : ''}: ${ERRNO[errno] ?? `errno ${errno}`}`);
        this.errno = errno;
    }
}
export var Kind;
(function (Kind) {
    Kind[Kind["Dir"] = 3] = "Dir";
    Kind[Kind["File"] = 4] = "File";
    Kind[Kind["Symlink"] = 7] = "Symlink";
})(Kind || (Kind = {}));
export async function createVfs(bytes) {
    const module = await WebAssembly.compile(bytes);
    // 4 GiB maximum: the sysroot alone is ~100 MB, and it all lives here.
    const memory = new WebAssembly.Memory({ initial: 512, maximum: 65536, shared: true });
    return { memory, module };
}
const canBlock = typeof globalThis.WorkerGlobalScope !== 'undefined';
const now = () => BigInt(Date.now()) * 1000000n;
const enc = new TextEncoder();
const dec = new TextDecoder();
export class Vfs {
    x;
    memory;
    scratch;
    constructor(x, memory, scratch) {
        this.x = x;
        this.memory = memory;
        this.scratch = scratch;
    }
    /** Instantiate the VFS on the shared memory in this worker (or the page). */
    static async attach({ memory, module }) {
        const inst = await WebAssembly.instantiate(module, { env: { memory } });
        const x = inst.exports;
        const i32 = new Int32Array(memory.buffer);
        const [slotSize, stackSize, maxSlots] = new Uint32Array(memory.buffer, x.VFS_SLOT_LAYOUT.value, 3);
        const slot = Atomics.add(i32, x.VFS_NEXT_SLOT.value >> 2, 1);
        if (slot >= maxSlots)
            throw new Error(`vfs: more than ${maxSlots} instances`);
        const base = x.VFS_SLOTS.value + slot * slotSize;
        x.__stack_pointer.value = base + stackSize; // stacks grow down; scratch follows the stack
        return new Vfs(x, memory, base + stackSize);
    }
    // ---------- locking and memory access ----------
    get i32() { return new Int32Array(this.memory.buffer); }
    get u8() { return new Uint8Array(this.memory.buffer); }
    /** Run `f` under the VFS lock. Everything that touches VFS memory must go through here. */
    locked(f) {
        const i32 = this.i32, lock = this.x.VFS_LOCK.value >> 2;
        while (Atomics.compareExchange(i32, lock, 0, 1) !== 0) {
            if (canBlock)
                Atomics.wait(i32, lock, 1, 5);
        }
        const seq = this.x.VFS_CHANGE_SEQ.value >> 2, before = Atomics.load(i32, seq);
        try {
            return f();
        }
        finally {
            Atomics.store(i32, lock, 0);
            Atomics.notify(i32, lock, 1);
            if (Atomics.load(i32, seq) !== before)
                Atomics.notify(i32, seq);
        }
    }
    putPath(path, at = 0) {
        // encodeInto() rejects shared buffers in Chromium; paths are short, so encode then copy.
        const bytes = enc.encode(path);
        if (bytes.length > 30 * 1024)
            throw new VfsError(37, 'path', path.slice(0, 64));
        this.u8.set(bytes, this.scratch + at);
        return [this.scratch + at, bytes.length];
    }
    get out() { return this.scratch + 61 * 1024; }
    check(r, op, path) {
        if (r < 0)
            throw new VfsError(-r, op, path);
        return r;
    }
    // ---------- raw calls (caller holds the lock; return -errno instead of throwing) ----------
    // Used by the WASI adapter, which needs errnos and already holds the lock.
    rawLookup(dir, path) { return this.x.vfs_lookup(dir, ...this.putPath(path)); }
    rawLookupNofollow(dir, path) { return this.x.vfs_lookup_nofollow(dir, ...this.putPath(path)); }
    rawSymlink(dir, target, path) {
        const [tp, tl] = this.putPath(target), [p, l] = this.putPath(path, 31 * 1024);
        return this.x.vfs_symlink(dir, tp, tl, p, l, now());
    }
    /** Link target as UTF-8 bytes, or -errno. */
    rawReadlink(dir, path) {
        const r = this.x.vfs_readlink(dir, ...this.putPath(path), this.out);
        if (r < 0)
            return r;
        const [ptr, len] = new Uint32Array(this.memory.buffer, this.out, 2);
        return this.u8.slice(ptr, ptr + len);
    }
    rawCreate(dir, path, excl, trunc) {
        return this.x.vfs_create(dir, ...this.putPath(path), (excl ? 1 : 0) | (trunc ? 2 : 0), now());
    }
    rawMkdir(dir, path) { return this.x.vfs_mkdir(dir, ...this.putPath(path), now()); }
    rawUnlink(dir, path) { return this.x.vfs_unlink(dir, ...this.putPath(path), now()); }
    rawRmdir(dir, path) { return this.x.vfs_rmdir(dir, ...this.putPath(path), now()); }
    rawRename(fromDir, from, toDir, to) {
        const [fp, fl] = this.putPath(from), [tp, tl] = this.putPath(to, 31 * 1024);
        return this.x.vfs_rename(fromDir, fp, fl, toDir, tp, tl, now());
    }
    rawStat(ino) {
        const r = this.x.vfs_stat(ino, this.out);
        if (r < 0)
            return r;
        const v = new BigUint64Array(this.memory.buffer, this.out, 3);
        return { kind: Number(v[0]), size: Number(v[1]), mtimeNs: v[2] };
    }
    rawTruncate(ino, size) { return this.x.vfs_truncate(ino, BigInt(size), now()); }
    /** Copy up to `dst.length` bytes at `offset` into `dst` (any memory, e.g. a guest's). */
    rawRead(ino, offset, dst) {
        const r = this.x.vfs_data(ino, this.out);
        if (r < 0)
            return r;
        const [ptr, len] = new Uint32Array(this.memory.buffer, this.out, 2);
        const start = Math.min(offset, len), n = Math.min(dst.length, len - start);
        dst.set(this.u8.subarray(ptr + start, ptr + start + n));
        return n;
    }
    /** Copy `src` into the file at `offset`, growing it as needed. */
    rawWrite(ino, offset, src) {
        const ptr = this.x.vfs_reserve(ino, BigInt(offset), src.length, now());
        if (ptr === 0 && src.length > 0)
            return -31; // a directory (or out of memory)
        this.u8.set(src, ptr);
        return src.length;
    }
    rawReaddir(dir, index) {
        const r = this.x.vfs_readdir(dir, index, this.out);
        if (r <= 0)
            return r === 0 ? null : r;
        const [p, l, ino, kind] = new Uint32Array(this.memory.buffer, this.out, 4);
        return { name: dec.decode(this.u8.slice(p, p + l)), ino, kind: kind };
    }
    // ---------- per-process fd tables (shared by all threads of a WASI process) ----------
    rawProcNew() { return this.x.vfs_proc_new(); }
    rawProcFree(pid) { this.x.vfs_proc_free(pid); }
    rawFdOpen(pid, ino, append, preopen = null) {
        const [p, l] = preopen === null ? [0, 0] : this.putPath(preopen);
        return this.x.vfs_fd_open(pid, ino, append ? 1 : 0, p, l);
    }
    rawFdGet(pid, fd, withName = false) {
        const r = this.x.vfs_fd_get(pid, fd, this.out);
        if (r < 0)
            return r;
        const u = new Uint32Array(this.memory.buffer, this.out, 6);
        const pos = Number(new BigUint64Array(this.memory.buffer, this.out + 8, 1)[0]);
        const isPreopen = u[4] !== 0;
        const preopen = isPreopen && withName ? dec.decode(this.u8.slice(u[4], u[4] + u[5])) : null;
        return { ino: u[0], append: u[1] !== 0, pos, isPreopen, preopen };
    }
    rawFdSetPos(pid, fd, pos) { return this.x.vfs_fd_set_pos(pid, fd, BigInt(pos)); }
    rawFdClose(pid, fd) { return this.x.vfs_fd_close(pid, fd); }
    rawFdRenumber(pid, from, to) { return this.x.vfs_fd_renumber(pid, from, to); }
    // ---------- convenience API (takes the lock, throws VfsError) ----------
    lookup(path, dir = ROOT) { return this.locked(() => this.check(this.rawLookup(dir, path), 'lookup', path)); }
    exists(path) { return this.locked(() => this.rawLookup(ROOT, path) >= 0); }
    /** Create a symlink at `path` pointing to `target` (e.g. so rustc finds its sysroot from argv[0]). */
    symlink(target, path) { this.locked(() => this.check(this.rawSymlink(ROOT, target, path), 'symlink', path)); }
    readlink(path) {
        return this.locked(() => {
            const r = this.rawReadlink(ROOT, path);
            if (typeof r === 'number')
                throw new VfsError(-r, 'readlink', path);
            return dec.decode(r);
        });
    }
    mkdir(path) { return this.locked(() => this.check(this.rawMkdir(ROOT, path), 'mkdir', path)); }
    mkdirAll(path) {
        return this.locked(() => this.check(this.x.vfs_mkdir_all(ROOT, ...this.putPath(path), now()), 'mkdirAll', path));
    }
    unlink(path) { this.locked(() => this.check(this.rawUnlink(ROOT, path), 'unlink', path)); }
    rmdir(path) { this.locked(() => this.check(this.rawRmdir(ROOT, path), 'rmdir', path)); }
    removeAll(path) {
        this.locked(() => this.check(this.x.vfs_remove_all(ROOT, ...this.putPath(path), now()), 'removeAll', path));
    }
    rename(from, to) { this.locked(() => this.check(this.rawRename(ROOT, from, ROOT, to), 'rename', from)); }
    stat(path) {
        return this.locked(() => {
            const s = this.rawStat(this.check(this.rawLookup(ROOT, path), 'stat', path));
            if (typeof s === 'number')
                throw new VfsError(-s, 'stat', path);
            return s;
        });
    }
    readFile(path) {
        return this.locked(() => {
            const ino = this.check(this.rawLookup(ROOT, path), 'readFile', path);
            this.check(this.x.vfs_data(ino, this.out), 'readFile', path);
            const [ptr, len] = new Uint32Array(this.memory.buffer, this.out, 2);
            return this.u8.slice(ptr, ptr + len);
        });
    }
    readText(path) { return dec.decode(this.readFile(path)); }
    /** Create or replace a file, creating parent directories. */
    writeFile(path, data) {
        const bytes = typeof data === 'string' ? enc.encode(data) : data;
        this.locked(() => {
            const parent = path.slice(0, path.lastIndexOf('/'));
            if (parent)
                this.check(this.x.vfs_mkdir_all(ROOT, ...this.putPath(parent), now()), 'writeFile', path);
            const ino = this.check(this.rawCreate(ROOT, path, false, true), 'writeFile', path);
            this.check(this.rawWrite(ino, 0, bytes), 'writeFile', path);
        });
    }
    /** Append to a file (created if missing), in one locked write: a reader never sees half of it. */
    appendFile(path, data) {
        const bytes = typeof data === 'string' ? enc.encode(data) : data;
        this.locked(() => {
            const ino = this.check(this.rawCreate(ROOT, path, false, false), 'appendFile', path);
            const s = this.rawStat(ino);
            if (typeof s === 'number')
                throw new VfsError(-s, 'appendFile', path);
            this.check(this.rawWrite(ino, s.size, bytes), 'appendFile', path);
        });
    }
    readdir(path) {
        return this.locked(() => {
            const dir = this.check(this.rawLookup(ROOT, path), 'readdir', path);
            const out = [];
            for (let i = 0;; i++) {
                const e = this.rawReaddir(dir, i);
                if (e === null)
                    return out;
                if (typeof e === 'number')
                    throw new VfsError(-e, 'readdir', path);
                out.push(e);
            }
        });
    }
    /** Index (in an Int32Array over the memory) of the change counter, for Atomics.waitAsync. */
    get changeSeqIndex() { return this.x.VFS_CHANGE_SEQ.value >> 2; }
    get sharedI32() { return this.i32; }
    inodeCount() { return this.locked(() => this.x.vfs_inode_count()); }
}
