// The theme compiler's worker (ARCHITECTURE.md R36, src/platform/compiler.rs): rustc and wasm-ld
// built as WebAssembly (the toolchain of github.com/AnselmSchaefer/rust-compiler-wasm-investigation,
// whose glue this folder is, made JavaScript), run in a worker of their own beside Caprice's page,
// never in Caprice's module or memory. It loads the toolchain as it starts (52 MiB the first time,
// then from Cache Storage), and builds one theme at a time: its one file in, the module or rustc's
// diagnostics out. Caprice ends it when it is done with it, which gives back the gigabyte it holds.
//
// The flags are the toolchain's own (toolchain/pack.mjs writes them into its manifest), so a theme
// built here is the same bytes as built anywhere with the same toolchain: themes/build.sh builds
// the fixtures the same way, natively, and the smoke test holds the two to it.
//
// Caprice says { build: source }; the worker says { loading: { fetched, of } } while it loads,
// { ready: toolchain } once loaded, then { built: module, toolchain, ms } or { errors, ms } for
// each build, or { failed: why, unsupported? } when it cannot go on.

import { current, Release } from './toolchain.js';
import { Orchestrator } from './orchestrator.js';

// Every theme is built as this file, wherever it was written: its path is in the module (where a
// panic would say it happened), so the same source gives the same bytes everywhere.
const SOURCE = '/theme/theme.rs';
const OUT = '/theme/theme.wasm';

const say = (message, transfer = []) => postMessage(message, transfer);

async function start() {
    // The toolchain is built for Chromium on a computer (a gigabyte beside Caprice while it lives),
    // and needs threads and shared memory; elsewhere say so before fetching 52 MiB.
    const ua = navigator.userAgentData;
    if (!ua || ua.mobile || typeof SharedArrayBuffer !== 'function')
        return { unsupported: 'Making a theme needs Chrome or Edge (or another browser built on Chromium) on a computer.' };
    const found = await current();
    if (!found)
        return { unsupported: 'This build of Caprice has no compiler for themes.' };
    const release = await Release.open(found.at, found.manifest, (fetched) => say({ loading: { fetched, of } }));
    const of = await release.toFetch();
    say({ loading: { fetched: 0, of } });
    const [orch, sdk] = await Promise.all([
        Orchestrator.create({ release, workerUrl: new URL('process-worker.js', import.meta.url) }),
        release.bytes('sdk'),
        // The linker and the sysroot's libraries, which the first build would wait for.
        release.tool('lld'),
    ]);
    await orch.loadLinkGroup();
    orch.vfs.writeFile(found.manifest.sdk, sdk);
    orch.vfs.mkdirAll('/theme');
    return { orch, manifest: found.manifest };
}

const started = start().catch((e) => ({ failed: String(e?.message ?? e) }));

started.then((s) => {
    if (s.unsupported)
        say({ failed: s.unsupported, unsupported: true });
    else if (s.failed)
        say({ failed: `the compiler could not be loaded: ${s.failed}` });
    else
        say({ ready: s.manifest.id });
});

let queue = Promise.resolve();

async function build(source) {
    const s = await started;
    if (!s.orch)
        return;
    const { orch, manifest } = s;
    const t0 = performance.now();
    orch.vfs.writeFile(SOURCE, source);
    if (orch.vfs.exists(OUT))
        orch.vfs.unlink(OUT);
    const args = [...manifest.flags, '--extern', `theme_sdk=${manifest.sdk}`, SOURCE, '-o', OUT];
    const r = await orch.rustcJob(args);
    const ms = Math.round(performance.now() - t0);
    if (r.code === 0 && orch.vfs.exists(OUT)) {
        const module = orch.vfs.readFile(OUT);
        say({ built: module.buffer, toolchain: manifest.id, ms }, [module.buffer]);
    }
    else if (r.code === 1) {
        say({ errors: r.stderr, ms });
    }
    else {
        // Not the theme's mistake: rustc itself failed (101 is an internal error), or the linker.
        say({ failed: `the compiler stopped (${r.code}): ${r.stderr.slice(-2000)}` });
    }
}

onmessage = ({ data }) => {
    if (typeof data.build === 'string')
        queue = queue.then(() => build(data.build)).catch((e) => say({ failed: `the compiler stopped: ${e?.message ?? e}` }));
};
