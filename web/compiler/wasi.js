// WASI preview 1 on top of the shared VFS (WP-11). One `WasiProcess` per thread: all threads of a
// process share the fd table (kept in the VFS by pid), so files opened in one worker can be used
// from another. Reads and writes copy directly between the VFS memory and the guest's memory.
//
// Covers what rustc imports (see docs/baseline-notes.md) plus the rest of the fd/path API;
// sockets and signals are ENOSYS.
import { Kind, ROOT } from './vfs.js';
export const E = { SUCCESS: 0, ACCES: 2, BADF: 8, INVAL: 28, IO: 29, ISDIR: 31, NOSYS: 52, NOTDIR: 54, NOTSUP: 58, SPIPE: 70 };
const FILETYPE_DIR = 3, FILETYPE_FILE = 4, FILETYPE_SYMLINK = 7;
const LOOKUP_SYMLINK_FOLLOW = 1;
const filetype = (k) => (k === Kind.Dir ? FILETYPE_DIR : k === Kind.Symlink ? FILETYPE_SYMLINK : FILETYPE_FILE);
const OFLAGS = { CREAT: 1, DIRECTORY: 2, EXCL: 4, TRUNC: 8 };
const FDFLAGS_APPEND = 1;
export class ProcExit extends Error {
    code;
    constructor(code) {
        super(`proc_exit(${code})`);
        this.code = code;
    }
}
/** Create a process's fd table in the VFS (stdio + preopens). Returns its pid. */
export function createProcess(vfs, spec) {
    for (const p of [spec.stdin, spec.stdout, spec.stderr])
        if (p && !vfs.exists(p))
            vfs.writeFile(p, '');
    return vfs.locked(() => {
        const pid = vfs.rawProcNew();
        const open = (path, append) => {
            const ino = path ? vfs.rawLookup(ROOT, path) : vfs.rawCreate(ROOT, '/dev-null', false, true);
            if (ino < 0)
                throw new Error(`createProcess: ${path}: errno ${-ino}`);
            vfs.rawFdOpen(pid, ino, append);
        };
        open(spec.stdin, false);
        open(spec.stdout, true);
        open(spec.stderr, true);
        for (const [guest, path] of spec.preopens) {
            const ino = vfs.rawLookup(ROOT, path);
            if (ino < 0)
                throw new Error(`createProcess: preopen ${path}: errno ${-ino}`);
            vfs.rawFdOpen(pid, ino, false, guest);
        }
        return pid;
    });
}
export function freeProcess(vfs, pid) {
    vfs.locked(() => vfs.rawProcFree(pid));
}
const enc = new TextEncoder();
const dec = new TextDecoder();
const sleepCell = new Int32Array(new SharedArrayBuffer(4));
export class WasiProcess {
    vfs;
    pid;
    args;
    env;
    onWrite;
    readStdin;
    memory;
    /**
     * `onWrite` is called after each successful fd_write (the host's process runner uses it for stdout).
     * `readStdin`, if given, serves fd_read on fd 0 instead of the VFS: it fills the buffers (blocking
     * as long as it likes, outside the VFS lock) and returns the byte count, 0 at end of input.
     */
    constructor(vfs, pid, args, env, onWrite, readStdin) {
        this.vfs = vfs;
        this.pid = pid;
        this.args = args;
        this.env = env;
        this.onWrite = onWrite;
        this.readStdin = readStdin;
    }
    /** Call once the instance exists (the guest's memory is imported or exported). */
    setMemory(memory) { this.memory = memory; }
    get dv() { return new DataView(this.memory.buffer); }
    get u8() { return new Uint8Array(this.memory.buffer); }
    str(ptr, len) { return dec.decode(this.u8.slice(ptr, ptr + len)); }
    /** iovec array → list of guest byte views. */
    iovs(ptr, n) {
        const dv = this.dv, u8 = this.u8, out = [];
        for (let i = 0; i < n; i++) {
            const p = dv.getUint32(ptr + i * 8, true), l = dv.getUint32(ptr + i * 8 + 4, true);
            out.push(u8.subarray(p, p + l));
        }
        return out;
    }
    writeStrings(list, ptrs, buf) {
        const dv = this.dv, u8 = this.u8;
        for (const s of list) {
            dv.setUint32(ptrs, buf, true);
            ptrs += 4;
            const b = enc.encode(s);
            u8.set(b, buf);
            u8[buf + b.length] = 0;
            buf += b.length + 1;
        }
        return E.SUCCESS;
    }
    sizes(list, countPtr, sizePtr) {
        this.dv.setUint32(countPtr, list.length, true);
        this.dv.setUint32(sizePtr, list.reduce((n, s) => n + enc.encode(s).length + 1, 0), true);
        return E.SUCCESS;
    }
    /** Run `f` with the fd's info under the VFS lock; negative results are -errno. */
    withFd(fd, f) {
        return this.vfs.locked(() => {
            const info = this.vfs.rawFdGet(this.pid, fd);
            if (typeof info === 'number')
                return -info;
            const r = f(info);
            return r < 0 ? -r : r;
        });
    }
    writeFilestat(out, ino) {
        const s = this.vfs.rawStat(ino);
        if (typeof s === 'number')
            return s;
        const dv = this.dv;
        dv.setBigUint64(out, 0n, true); // dev
        dv.setBigUint64(out + 8, BigInt(ino), true);
        dv.setUint8(out + 16, filetype(s.kind));
        dv.setBigUint64(out + 24, 1n, true); // nlink
        dv.setBigUint64(out + 32, BigInt(s.size), true);
        dv.setBigUint64(out + 40, s.mtimeNs, true);
        dv.setBigUint64(out + 48, s.mtimeNs, true);
        dv.setBigUint64(out + 56, s.mtimeNs, true);
        return 0;
    }
    /** Directory inode behind a `dirfd` for path_* calls. */
    dirOf(fd) {
        const info = this.vfs.rawFdGet(this.pid, fd);
        return typeof info === 'number' ? info : info.ino;
    }
    imports = {
        args_sizes_get: (c, s) => this.sizes(this.args, c, s),
        args_get: (p, b) => this.writeStrings(this.args, p, b),
        environ_sizes_get: (c, s) => this.sizes(this.env, c, s),
        environ_get: (p, b) => this.writeStrings(this.env, p, b),
        clock_res_get: (_id, out) => { this.dv.setBigUint64(out, 1000n, true); return 0; },
        clock_time_get: (id, _precision, out) => {
            const ns = id === 0
                ? BigInt(Date.now()) * 1000000n
                : BigInt(Math.round((performance.timeOrigin + performance.now()) * 1e6));
            this.dv.setBigUint64(out, ns, true);
            return 0;
        },
        random_get: (ptr, len) => {
            // getRandomValues refuses shared views; 64 KiB per call.
            for (let o = 0; o < len; o += 65536) {
                const tmp = crypto.getRandomValues(new Uint8Array(Math.min(65536, len - o)));
                this.u8.set(tmp, ptr + o);
            }
            return 0;
        },
        fd_write: (fd, iovs, n, nwritten) => {
            const bufs = this.iovs(iovs, n);
            const errno = this.withFd(fd, info => {
                let pos = info.append ? this.vfs.rawStat(info.ino).size : info.pos, total = 0;
                for (const b of bufs) {
                    const r = this.vfs.rawWrite(info.ino, pos, b);
                    if (r < 0)
                        return r;
                    pos += r;
                    total += r;
                }
                this.vfs.rawFdSetPos(this.pid, fd, pos);
                this.dv.setUint32(nwritten, total, true);
                return 0;
            });
            // After the VFS lock is released: the callback may use the VFS itself (the sandbox does).
            if (errno === 0)
                this.onWrite?.(fd);
            return errno;
        },
        fd_pwrite: (fd, iovs, n, offset, nwritten) => {
            const bufs = this.iovs(iovs, n);
            return this.withFd(fd, info => {
                let pos = Number(offset), total = 0;
                for (const b of bufs) {
                    const r = this.vfs.rawWrite(info.ino, pos, b);
                    if (r < 0)
                        return r;
                    pos += r;
                    total += r;
                }
                this.dv.setUint32(nwritten, total, true);
                return 0;
            });
        },
        fd_read: (fd, iovs, n, nread) => {
            const bufs = this.iovs(iovs, n);
            if (fd === 0 && this.readStdin) {
                // The views may be detached if the guest's memory grows meanwhile; take fresh ones after.
                const tmp = bufs.map(b => new Uint8Array(b.length));
                const total = this.readStdin(tmp);
                let left = total;
                const fresh = this.iovs(iovs, n);
                for (let i = 0; i < fresh.length && left > 0; i++) {
                    const k = Math.min(left, fresh[i].length);
                    fresh[i].set(tmp[i].subarray(0, k));
                    left -= k;
                }
                this.dv.setUint32(nread, total, true);
                return 0;
            }
            return this.withFd(fd, info => {
                let pos = info.pos, total = 0;
                for (const b of bufs) {
                    const r = this.vfs.rawRead(info.ino, pos, b);
                    if (r < 0)
                        return r;
                    pos += r;
                    total += r;
                    if (r < b.length)
                        break;
                }
                this.vfs.rawFdSetPos(this.pid, fd, pos);
                this.dv.setUint32(nread, total, true);
                return 0;
            });
        },
        fd_pread: (fd, iovs, n, offset, nread) => {
            const bufs = this.iovs(iovs, n);
            return this.withFd(fd, info => {
                let pos = Number(offset), total = 0;
                for (const b of bufs) {
                    const r = this.vfs.rawRead(info.ino, pos, b);
                    if (r < 0)
                        return r;
                    pos += r;
                    total += r;
                    if (r < b.length)
                        break;
                }
                this.dv.setUint32(nread, total, true);
                return 0;
            });
        },
        fd_seek: (fd, offset, whence, out) => this.withFd(fd, info => {
            const size = this.vfs.rawStat(info.ino).size;
            const base = whence === 0 ? 0 : whence === 1 ? info.pos : whence === 2 ? size : NaN;
            const pos = base + Number(offset);
            if (!(pos >= 0))
                return -E.INVAL;
            this.vfs.rawFdSetPos(this.pid, fd, pos);
            this.dv.setBigUint64(out, BigInt(pos), true);
            return 0;
        }),
        fd_tell: (fd, out) => this.withFd(fd, info => { this.dv.setBigUint64(out, BigInt(info.pos), true); return 0; }),
        fd_close: (fd) => -Math.min(0, this.vfs.locked(() => this.vfs.rawFdClose(this.pid, fd))),
        fd_renumber: (from, to) => -Math.min(0, this.vfs.locked(() => this.vfs.rawFdRenumber(this.pid, from, to))),
        fd_sync: (fd) => this.withFd(fd, () => 0),
        fd_datasync: (fd) => this.withFd(fd, () => 0),
        fd_advise: (fd) => this.withFd(fd, () => 0),
        fd_allocate: (fd) => this.withFd(fd, () => 0),
        fd_fdstat_set_flags: (fd) => this.withFd(fd, () => 0),
        fd_fdstat_set_rights: (fd) => this.withFd(fd, () => 0),
        fd_fdstat_get: (fd, out) => this.withFd(fd, info => {
            const s = this.vfs.rawStat(info.ino);
            if (typeof s === 'number')
                return s;
            const dv = this.dv;
            dv.setUint8(out, filetype(s.kind));
            dv.setUint16(out + 2, info.append ? FDFLAGS_APPEND : 0, true);
            dv.setBigUint64(out + 8, 0xffffffffffffffffn, true);
            dv.setBigUint64(out + 16, 0xffffffffffffffffn, true);
            return 0;
        }),
        fd_filestat_get: (fd, out) => this.withFd(fd, info => this.writeFilestat(out, info.ino)),
        fd_filestat_set_size: (fd, size) => this.withFd(fd, info => this.vfs.rawTruncate(info.ino, Number(size))),
        fd_filestat_set_times: (fd) => this.withFd(fd, () => 0),
        fd_prestat_get: (fd, out) => this.vfs.locked(() => {
            const info = this.vfs.rawFdGet(this.pid, fd, true);
            if (typeof info === 'number')
                return -info;
            if (info.preopen === null)
                return E.BADF;
            this.dv.setUint8(out, 0);
            this.dv.setUint32(out + 4, enc.encode(info.preopen).length, true);
            return 0;
        }),
        fd_prestat_dir_name: (fd, ptr, len) => this.vfs.locked(() => {
            const info = this.vfs.rawFdGet(this.pid, fd, true);
            if (typeof info === 'number')
                return -info;
            if (info.preopen === null)
                return E.BADF;
            this.u8.set(enc.encode(info.preopen).subarray(0, len), ptr);
            return 0;
        }),
        fd_readdir: (fd, buf, bufLen, cookie, usedPtr) => this.withFd(fd, info => {
            const dv = this.dv, u8 = this.u8;
            let used = 0;
            for (let i = Number(cookie); used < bufLen; i++) {
                const e = this.vfs.rawReaddir(info.ino, i);
                if (typeof e === 'number')
                    return e;
                if (e === null)
                    break;
                const name = enc.encode(e.name), head = new Uint8Array(24), hv = new DataView(head.buffer);
                hv.setBigUint64(0, BigInt(i + 1), true);
                hv.setBigUint64(8, BigInt(e.ino), true);
                hv.setUint32(16, name.length, true);
                hv.setUint8(20, filetype(e.kind));
                const entry = new Uint8Array(24 + name.length);
                entry.set(head);
                entry.set(name, 24);
                const n = Math.min(entry.length, bufLen - used); // a truncated last entry tells the guest to call again
                u8.set(entry.subarray(0, n), buf + used);
                used += n;
            }
            dv.setUint32(usedPtr, used, true);
            return 0;
        }),
        path_open: (dirfd, _dirflags, p, l, oflags, _rb, _ri, fdflags, out) => {
            const path = this.str(p, l);
            return this.vfs.locked(() => {
                const dir = this.dirOf(dirfd);
                if (dir < 0)
                    return -dir;
                let ino = oflags & OFLAGS.CREAT
                    ? this.vfs.rawCreate(dir, path, !!(oflags & OFLAGS.EXCL), !!(oflags & OFLAGS.TRUNC))
                    : this.vfs.rawLookup(dir, path);
                if (ino < 0)
                    return -ino;
                const s = this.vfs.rawStat(ino);
                if (oflags & OFLAGS.DIRECTORY && s.kind !== Kind.Dir)
                    return E.NOTDIR;
                if (oflags & OFLAGS.TRUNC && !(oflags & OFLAGS.CREAT) && s.kind === Kind.File)
                    this.vfs.rawTruncate(ino, 0);
                const fd = this.vfs.rawFdOpen(this.pid, ino, !!(fdflags & FDFLAGS_APPEND));
                if (fd < 0)
                    return -fd;
                this.dv.setUint32(out, fd, true);
                return 0;
            });
        },
        path_create_directory: (fd, p, l) => this.pathOp(fd, p, l, (d, s) => this.vfs.rawMkdir(d, s)),
        path_remove_directory: (fd, p, l) => this.pathOp(fd, p, l, (d, s) => this.vfs.rawRmdir(d, s)),
        path_unlink_file: (fd, p, l) => this.pathOp(fd, p, l, (d, s) => this.vfs.rawUnlink(d, s)),
        path_filestat_get: (fd, flags, p, l, out) => this.pathOp(fd, p, l, (d, s) => {
            const ino = flags & LOOKUP_SYMLINK_FOLLOW ? this.vfs.rawLookup(d, s) : this.vfs.rawLookupNofollow(d, s);
            return ino < 0 ? ino : this.writeFilestat(out, ino);
        }),
        path_filestat_set_times: (fd, _flags, p, l) => this.pathOp(fd, p, l, (d, s) => Math.min(0, this.vfs.rawLookup(d, s))),
        path_readlink: (fd, p, l, buf, bufLen, usedPtr) => this.pathOp(fd, p, l, (d, s) => {
            const t = this.vfs.rawReadlink(d, s);
            if (typeof t === 'number')
                return t;
            const n = Math.min(t.length, bufLen); // truncated, as POSIX readlink
            this.u8.set(t.subarray(0, n), buf);
            this.dv.setUint32(usedPtr, n, true);
            return 0;
        }),
        path_rename: (fd, p, l, fd2, p2, l2) => {
            const from = this.str(p, l), to = this.str(p2, l2);
            return this.vfs.locked(() => {
                const d1 = this.dirOf(fd), d2 = this.dirOf(fd2);
                if (d1 < 0)
                    return -d1;
                if (d2 < 0)
                    return -d2;
                return -Math.min(0, this.vfs.rawRename(d1, from, d2, to));
            });
        },
        path_link: () => E.NOTSUP, // callers (rustc's link_or_copy) fall back to copying
        path_symlink: (op, ol, fd, p, l) => {
            const target = this.str(op, ol);
            return this.pathOp(fd, p, l, (d, s) => this.vfs.rawSymlink(d, target, s));
        },
        poll_oneoff: (inPtr, outPtr, n, neventsPtr) => {
            // Clock subscriptions sleep; fd subscriptions are always ready (files never block).
            const dv = this.dv;
            let sleepNs = Infinity;
            for (let i = 0; i < n; i++) {
                const s = inPtr + i * 48;
                if (dv.getUint8(s + 8) === 0) {
                    const timeout = Number(dv.getBigUint64(s + 24, true)), abs = dv.getUint16(s + 40, true) & 1;
                    sleepNs = Math.min(sleepNs, abs ? Math.max(0, timeout - Date.now() * 1e6) : timeout);
                }
            }
            if (sleepNs !== Infinity && sleepNs > 0)
                Atomics.wait(sleepCell, 0, 0, sleepNs / 1e6);
            for (let i = 0; i < n; i++) {
                const s = inPtr + i * 48, e = outPtr + i * 32;
                dv.setBigUint64(e, dv.getBigUint64(s, true), true);
                dv.setUint16(e + 8, 0, true);
                dv.setUint8(e + 10, dv.getUint8(s + 8));
            }
            dv.setUint32(neventsPtr, n, true);
            return 0;
        },
        sched_yield: () => 0,
        proc_exit: (code) => { throw new ProcExit(code); },
        proc_raise: () => E.NOSYS,
        sock_accept: () => E.NOSYS,
        sock_recv: () => E.NOSYS,
        sock_send: () => E.NOSYS,
        sock_shutdown: () => E.NOSYS,
    };
    pathOp(fd, p, l, f) {
        const path = this.str(p, l);
        return this.vfs.locked(() => {
            const dir = this.dirOf(fd);
            if (dir < 0)
                return -dir;
            return -Math.min(0, f(dir, path));
        });
    }
}
