// Worker entry for one thread of a WASI process (see process.js): the main thread runs `_start`,
// spawned threads run `wasi_thread_start(tid, arg)`. Posts `{ exit }` when the process ends and
// `{ idle }` when a spawned thread finishes.
import { Vfs } from './vfs.js';
import { ProcExit, WasiProcess } from './wasi.js';
import { requestThread, stdoutWritten } from './process.js';
let thread;
onmessage = async (e) => {
    // A worker whose thread has finished stays idle; the page can hand it the next thread.
    if (!('module' in e.data))
        return runThread(e.data.thread);
    const { vfs: handle, module, memory, channel, pid, args, env } = e.data;
    const vfs = await Vfs.attach(handle);
    const wasi = new WasiProcess(vfs, pid, args, env, fd => { if (fd === 1)
        stdoutWritten(channel); });
    let inst;
    inst = await WebAssembly.instantiate(module, {
        env: { memory },
        wasi_snapshot_preview1: wasi.imports,
        wasi: { 'thread-spawn': (arg) => requestThread(channel, arg) },
        // rustc-multi's host for proc macros built as WebAssembly (WP-20). A theme is one file and
        // names no crate, so none is ever loaded: rustc is told there is none.
        rust_pm: { load: () => -1, expand: () => 1 },
    });
    wasi.setMemory(memory);
    const x = inst.exports;
    thread = x.wasi_thread_start;
    if (e.data.thread)
        return runThread(e.data.thread);
    try {
        x._start();
        postMessage({ exit: 0 });
    }
    catch (err) {
        exitWith(err);
    }
};
// wasi-libc's wasi_thread_start sets the stack pointer and TLS base from `arg` each time, so one
// instance can run one thread after another.
function runThread(t) {
    try {
        thread(t.tid, t.arg);
        postMessage({ idle: true });
    }
    catch (err) {
        exitWith(err);
    }
}
// proc_exit from any thread ends the whole process; a trap does too.
function exitWith(err) {
    if (err instanceof ProcExit)
        postMessage({ exit: err.code });
    else
        postMessage({ exit: 134, error: String(err?.stack ?? err) });
}
