// The toolchain as Caprice serves it (toolchain/pack.mjs): /compiler/toolchain.json names the one
// this build uses, and /toolchain/<id>/manifest.json names each of its files (rustc, lld, the
// sysroot's two groups, the theme SDK) by its size and sha256, and the brotli pieces it was cut
// into, each under Workers' 25 MiB and named by its own hash. The pieces never change (a new
// toolchain is new names), so they are kept in Cache Storage as they came, their hashes checked
// once, when fetched: decoding them again on every load is what costs (about 1.3 s), not the
// check. Chromium's DecompressionStream has no brotli, so they are decoded here, by a decoder of
// our own (toolchain/unbrotli), into a buffer of the size the manifest says.

import { importedMemoryLimits } from './process.js';

const hex = (bytes) => Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
const sha256 = async (bytes) => hex(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)));

/** The toolchain this build of Caprice uses, if it has one: its manifest, and where it lives. */
export async function current() {
    const named = await fetch(new URL('toolchain.json', import.meta.url), { cache: 'no-cache' });
    if (!named.ok)
        return null;
    const { id } = await named.json();
    const at = new URL(`../toolchain/${id}/`, import.meta.url);
    const res = await fetch(new URL('manifest.json', at));
    if (!res.ok)
        throw new Error(`its manifest could not be fetched (HTTP ${res.status})`);
    return { at, manifest: await res.json() };
}

export class Release {
    /** `at`: the toolchain's folder; `progress(fetched, of)` is told the bytes fetched so far. */
    constructor(at, manifest, cache, progress) {
        this.at = at;
        this.manifest = manifest;
        this.cache = cache;
        this.progress = progress;
        this.fetched = 0;
        this.decoder = null;
        this.tools = new Map();
        this.stats = [];
    }

    static async open(at, manifest, progress = () => { }) {
        let cache = null;
        try {
            cache = await caches.open(`caprice-toolchain-${manifest.id}`);
        }
        catch {
            // No Cache Storage (a private window, say): everything is fetched each time.
        }
        return new Release(at, manifest, cache, progress);
    }

    get id() { return this.manifest.id; }

    /** The bytes the pieces not yet kept come to: what loading it costs the network. */
    async toFetch() {
        let n = 0;
        for (const f of Object.values(this.manifest.files))
            for (const p of f.pieces)
                if (!(await this.cache?.match(new URL(p.file, this.at))))
                    n += p.size;
        return n;
    }

    /** One file served as it is (the decoder, the VFS), its hash checked. */
    async plain(name) {
        const f = this.manifest.plain[name];
        const res = await fetch(new URL(f.file, this.at));
        if (!res.ok)
            throw new Error(`${f.file}: HTTP ${res.status}`);
        const bytes = new Uint8Array(await res.arrayBuffer());
        if ((await sha256(bytes)) !== f.sha256)
            throw new Error(`${f.file} is not the file its manifest names`);
        return bytes;
    }

    async piece(p) {
        const url = new URL(p.file, this.at);
        const kept = await this.cache?.match(url);
        if (kept) {
            const data = new Uint8Array(await kept.arrayBuffer());
            if (data.length === p.size)
                return { data, fromCache: true };
        }
        const res = await fetch(url);
        if (!res.ok)
            throw new Error(`${p.file}: HTTP ${res.status}`);
        const data = new Uint8Array(await res.arrayBuffer());
        if (data.length !== p.size || (await sha256(data)) !== p.sha256)
            throw new Error(`${p.file} is not the piece its manifest names`);
        this.fetched += data.length;
        this.progress(this.fetched);
        await this.cache?.put(url, new Response(data, { headers: { 'Content-Type': 'application/octet-stream' } }));
        return { data, fromCache: false };
    }

    /** One brotli stream (its pieces in order) decoded into a buffer of the size the manifest says. */
    async unbrotli(pieces, size) {
        this.decoder ??= this.plain('unbrotli').then((b) => WebAssembly.compile(b));
        const x = (await WebAssembly.instantiate(await this.decoder)).exports;
        const len = pieces.reduce((a, p) => a + p.length, 0), CHUNK = 1 << 20;
        let at = x.input(len, CHUNK);
        for (const p of pieces) {
            new Uint8Array(x.memory.buffer, at, p.length).set(p);
            at += p.length;
        }
        const out = new Uint8Array(size), from = x.output();
        let o = 0;
        for (;;) {
            const n = x.read();
            if (n < 0)
                throw new Error('a piece of it is not brotli');
            if (n === 0)
                break;
            if (o + n > size)
                throw new Error('it decodes to more bytes than its manifest says');
            out.set(new Uint8Array(x.memory.buffer, from, n), o);
            o += n;
        }
        if (o !== size)
            throw new Error(`it decodes to ${o} bytes, its manifest says ${size}`);
        return out;
    }

    /** The bytes of one file, decoded. */
    async bytes(name) {
        const f = this.manifest.files[name];
        const t0 = performance.now();
        const got = await Promise.all(f.pieces.map((p) => this.piece(p)));
        const t1 = performance.now();
        const data = await this.unbrotli(got.map((g) => g.data), f.size);
        this.stats.push({ file: name, bytes: data.length, fromCache: got.every((g) => g.fromCache), fetchMs: Math.round(t1 - t0), decodeMs: Math.round(performance.now() - t1) });
        return data;
    }

    /** A tool (rustc or lld), compiled once. */
    tool(which) {
        let t = this.tools.get(which);
        if (!t) {
            t = this.bytes(which).then(async (b) => {
                const t0 = performance.now();
                const module = await WebAssembly.compile(b);
                this.stats.find((s) => s.file === which).compileMs = Math.round(performance.now() - t0);
                return { module, limits: importedMemoryLimits(b) };
            });
            t.catch(() => this.tools.delete(which));
            this.tools.set(which, t);
        }
        return t;
    }
}
