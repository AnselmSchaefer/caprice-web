// The orchestrator (WP-12): one VFS, the toolchain release, long-lived compile servers
// (rustc --server --follow, toolchain/rustc-multi) and a linker process (wasm-ld.wasm) per link.
// Up to `workers` jobs run at once (WP-17), each on a compile server of its own; the servers start
// on demand and stay. With one worker, jobs run one at a time, in order. rustc stops at the link step with its link command printed
// (link-args.js); the orchestrator runs wasm-ld with it on the same VFS, then moves the
// results out of the job's scratch dir. Here it runs in Caprice's compiler worker (compiler.js),
// with one compile server: a theme is one file, built one at a time. The toolchain comes in
// pieces (toolchain.js).
import { createVfs, Kind, Vfs } from './vfs.js';
import { runProcess } from './process.js';
import { absolute, linkerSpawnError, outputDestination, parseLinkCommand, rewriteOutputs, wasmLdArgs } from './link-args.js';
import { Release } from './toolchain.js';
import { readTar } from './tar.js';
const RUSTC_ARGV0 = '/sysroot/bin/rustc';
// rustc's main thread needs more than the default stack for deep expressions (as in WP-01).
const ENV = ['RUST_MIN_STACK=16777216'];
export class Orchestrator {
    vfs;
    handle;
    release;
    rustc;
    workerUrl;
    workers;
    jobs = 0;
    linkGroup = null;
    servers = [];
    idle = [];
    waiting = [];
    running = new Set();
    constructor(vfs, handle, release, rustc, workerUrl, workers) {
        this.vfs = vfs;
        this.handle = handle;
        this.release = release;
        this.rustc = rustc;
        this.workerUrl = workerUrl;
        this.workers = workers;
        this.idle.push(this.startServer());
    }
    /**
     * `release`: the toolchain (toolchain.js). `workerUrl`: process-worker.js. Loads rustc and the
     * sysroot's metadata group; the link group and lld come with the first link, or with `warm`.
     */
    static async create({ release, workerUrl }) {
        const handle = await createVfs(await release.plain('vfs'));
        const vfs = await Vfs.attach(handle);
        const [rustc] = await Promise.all([release.tool('rustc'), unpack(vfs, release, 'meta')]);
        // rustc trusts argv[0] for its sysroot only if it is a symlink, then checks lib/rustlib relative
        // to the working directory (/), hence the second link (scripts/verify/wp-05.sh).
        vfs.mkdirAll('/sysroot/bin');
        vfs.symlink('../lib/rustc-is-a-wasm-module', RUSTC_ARGV0);
        vfs.mkdirAll('/lib');
        vfs.symlink('/sysroot/lib/rustlib', '/lib/rustlib');
        vfs.mkdirAll('/tmp');
        vfs.mkdirAll('/proc');
        return new Orchestrator(vfs, handle, release, rustc, new URL(String(workerUrl)), 1);
    }
    startServer() {
        const s = new CompileServer(this.vfs, this.handle, this.rustc, this.workerUrl, this.servers.length);
        this.servers.push(s);
        return s;
    }
    /** A free compile server: an idle one, a new one (below `workers`), or the next one released. */
    acquire() {
        const s = this.idle.shift() ?? (this.servers.length < this.workers ? this.startServer() : undefined);
        return s ? Promise.resolve(s) : new Promise(r => this.waiting.push(r));
    }
    releaseServer(s) {
        const w = this.waiting.shift();
        if (w)
            w(s);
        else
            this.idle.push(s);
    }
    /**
     * Run rustc with `args` (paths in the VFS), linking if needed. `env`: environment variables for
     * this job only (OUT_DIR, CARGO_PKG_*, ...). `cwd` (default /) and `stdin` (the source for an
     * input of `-`): a build script's rustc probe (WP-21).
     */
    rustcJob(args, env, io) {
        const job = (async () => {
            const s = await this.acquire();
            try {
                return await this.run(s, args, env, io);
            }
            finally {
                this.releaseServer(s);
            }
        })();
        this.running.add(job);
        job.finally(() => this.running.delete(job)).catch(() => { });
        return job;
    }
    async run(server, user, env, io) {
        const t0 = performance.now();
        const scratch = `/tmp/job-${++this.jobs}`, linkFile = `${scratch}/link-args`;
        this.vfs.mkdirAll(scratch);
        try {
            const plan = rewriteOutputs(user, scratch, linkFile);
            // A build that links needs the .rlib files while rustc runs (it checks dependency formats
            // before linking); --emit=metadata or obj alone needs only the .rmeta files.
            if (emitsLink(user))
                await this.loadLinkGroup();
            const r = await server.request(plan.args, env, io);
            let code = r.exit, stderr = r.stderr, linked = false, linkMs = 0;
            if (code !== 0 && this.vfs.exists(linkFile)) {
                // rustc got to the link step and couldn't start a linker: link here instead.
                const at = linkerSpawnError(stderr);
                if (at >= 0) {
                    stderr = stderr.slice(0, at);
                    const t1 = performance.now();
                    const ld = await this.link(wasmLdArgs(parseLinkCommand(this.vfs.readText(linkFile))));
                    linkMs = performance.now() - t1;
                    code = ld.code;
                    stderr += ld.stderr;
                    linked = true;
                }
            }
            const outputs = [];
            if (code === 0) {
                for (const e of this.vfs.readdir(scratch)) {
                    // rustc's temp dirs and its link command stay behind and go with the scratch dir.
                    const dst = outputDestination(plan, e.name);
                    if (!dst || e.kind !== Kind.File || e.name === 'link-args')
                        continue;
                    const to = absolute(dst, io?.cwd);
                    this.vfs.mkdirAll(to.slice(0, to.lastIndexOf('/')) || '/');
                    this.vfs.rename(`${scratch}/${e.name}`, to);
                    outputs.push(to);
                }
            }
            return { code, stderr, stdout: r.stdout, linked, outputs, ms: { rustc: r.ms, link: Math.round(linkMs), total: Math.round(performance.now() - t0) } };
        }
        finally {
            this.vfs.removeAll(scratch);
        }
    }
    /** Run wasm-ld.wasm with `args` as a process of its own. */
    loadLinkGroup() {
        this.linkGroup ??= unpack(this.vfs, this.release, 'link');
        return this.linkGroup;
    }
    async link(args) {
        const [lld] = await Promise.all([this.release.tool('lld'), this.loadLinkGroup()]);
        const n = ++this.jobs, out = `/proc/ld-${n}.out`, err = `/proc/ld-${n}.err`;
        const memory = new WebAssembly.Memory({ ...lld.limits, shared: true });
        const r = await runProcess(this.vfs, this.handle, lld.module, memory, { args: ['wasm-ld', ...args], env: [], stdout: out, stderr: err, preopens: [['/', '/']] }, this.workerUrl);
        const res = { code: r.code, stderr: this.vfs.readText(out) + this.vfs.readText(err) + (r.error ? `\n${r.error}\n` : '') };
        this.vfs.unlink(out);
        this.vfs.unlink(err);
        return res;
    }
    stats() {
        const rustcBytes = this.servers.reduce((a, s) => a + s.memoryBytes, 0);
        return { vfsBytes: this.handle.memory.buffer.byteLength, rustcBytes, jobs: this.jobs, servers: this.servers.length };
    }
    /** Stop the compile servers after the running jobs; resolves with the first one's process result. */
    async close() {
        await Promise.allSettled([...this.running]);
        const results = await Promise.all(this.servers.map(s => s.quit()));
        return results[0];
    }
}
/** Whether rustc with `args` emits `link` (the default when there is no --emit). */
export function emitsLink(args) {
    const kinds = [];
    args.forEach((a, i) => {
        const v = a === '--emit' ? args[i + 1] : a.startsWith('--emit=') ? a.slice(7) : null;
        if (v)
            kinds.push(...v.split(',').map(k => k.split('=')[0]));
    });
    return kinds.length === 0 || kinds.includes('link');
}
/** Extract one sysroot group of the release into /sysroot. */
async function unpack(vfs, release, group) {
    const tar = await release.bytes(group);
    for (const e of readTar(tar))
        vfs.writeFile(`/sysroot/${e.path}`, e.data);
}
/**
 * rustc-multi --server --follow in one long-lived instance. Requests are appended to its stdin
 * file as JSON lines; each answer is a JSON line on stdout, and "\x1e<id>" on stderr ends that
 * request's diagnostics. The process runner wakes us on every stdout write.
 */
class CompileServer {
    vfs;
    nextId = 0;
    outPos = 0;
    errPos = 0;
    wake;
    stdin;
    stdout;
    stderr;
    done;
    memory;
    get memoryBytes() { return this.memory.buffer.byteLength; }
    constructor(vfs, handle, rustc, workerUrl, n) {
        this.vfs = vfs;
        const name = n === 0 ? 'rustc' : `rustc-${n}`;
        this.stdin = `/proc/${name}.in`;
        this.stdout = `/proc/${name}.out`;
        this.stderr = `/proc/${name}.err`;
        vfs.writeFile(this.stdin, '');
        const memory = new WebAssembly.Memory({ ...rustc.limits, shared: true });
        this.memory = memory;
        this.done = runProcess(vfs, handle, rustc.module, memory, {
            args: [RUSTC_ARGV0, '--server', '--follow'], env: ENV,
            stdin: this.stdin, stdout: this.stdout, stderr: this.stderr, preopens: [['/', '/']],
        }, workerUrl, () => this.wake?.());
    }
    async request(args, env, io) {
        const id = ++this.nextId;
        this.vfs.appendFile(this.stdin, JSON.stringify({ id, args, ...(env && { env }), ...(io?.cwd && { cwd: io.cwd }), ...(io?.stdin !== undefined && { stdin: io.stdin }) }) + '\n');
        const answer = await this.answer(id);
        const e = this.vfs.readText(this.stderr), mark = `\x1e${id}\n`, at = e.indexOf(mark, this.errPos);
        const stderr = e.slice(this.errPos, at);
        this.errPos = at + mark.length;
        return { exit: answer.exit, ms: answer.ms, stderr, stdout: answer.stdout, out: answer.out };
    }
    /** The answer line, and rustc's own stdout before it (e.g. -V). */
    async answer(id) {
        let stdout = '';
        for (;;) {
            const text = this.vfs.readText(this.stdout);
            const nl = text.indexOf('\n', this.outPos);
            if (nl >= 0) {
                const line = text.slice(this.outPos, nl);
                this.outPos = nl + 1;
                if (line.startsWith('{"') && line.includes('"id"')) {
                    const a = JSON.parse(line);
                    if (a.id === id)
                        return { ...a, stdout };
                }
                stdout += line + '\n';
                continue;
            }
            // Woken by the server's next stdout write; the timeout only guards against a lost wake-up.
            await new Promise(r => { this.wake = r; setTimeout(r, 50); });
        }
    }
    async quit() {
        this.vfs.appendFile(this.stdin, JSON.stringify({ id: ++this.nextId, quit: true }) + '\n');
        return this.done;
    }
}
