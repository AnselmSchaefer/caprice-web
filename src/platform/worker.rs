//! Web Workers for background work in the browser (`platform::spawn`). Each piece of work gets a
//! worker of its own, started from `worker.js` on the page's own site; it starts the app's
//! WebAssembly on the same shared memory, runs the work (a boxed closure, passed as a pointer into
//! that memory) and closes. Nothing is evaluated from a string, so the Content-Security-Policy can
//! forbid it (`web/csp.mjs`): the reason this is not `wasm_thread`, which evaluates code to start
//! its workers.

use std::cell::RefCell;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Worker, WorkerOptions, WorkerType};

// The app's own script (wasm-bindgen's, named by its hash) is found from the stack of a call that
// comes from it, as there is no other way to ask: every frame but this file's own.
#[wasm_bindgen(inline_js = r#"
    export function app_script_url() {
        const urls = (new Error().stack.match(/https?:\/\/[^\s)'"]+?\.js/g) || [])
            .filter(url => !url.includes("/snippets/"));
        return urls[0] || "";
    }
"#)]
extern "C" {
    fn app_script_url() -> String;
}

/// The work a worker runs: what `spawn` boxed, given back from its pointer.
type Work = Box<dyn FnOnce() + Send>;

/// Called in the worker by `worker.js` with the pointer `spawn` sent.
#[wasm_bindgen]
pub fn caprice_worker_run(work: u32) {
    // SAFETY: the pointer is one `spawn` made with `Box::into_raw` and sent to this worker only.
    let work = unsafe { Box::from_raw(work as *mut Work) };
    work();
}

thread_local! {
    /// The workers running, so none is let go of before it has finished.
    static WORKERS: RefCell<Vec<Worker>> = const { RefCell::new(Vec::new()) };
    static APP_SCRIPT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Run `work` in a new worker. It is sent no further message; it says when it is done.
pub fn spawn(work: Work) -> Result<(), JsValue> {
    let script = APP_SCRIPT.with_borrow_mut(|s| s.get_or_insert_with(app_script_url).clone());
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    let worker = Worker::new_with_options("/worker.js", &options)?;
    let done = {
        let worker = worker.clone();
        Closure::once_into_js(move || WORKERS.with_borrow_mut(|w| w.retain(|w| *w != worker)))
    };
    worker.set_onmessage(Some(done.unchecked_ref()));
    let work = Box::into_raw(Box::new(work)) as u32;
    let message = js_sys::Array::of4(&script.into(), &wasm_bindgen::module(), &wasm_bindgen::memory(), &work.into());
    worker.post_message(&message)?;
    WORKERS.with_borrow_mut(|w| w.push(worker));
    Ok(())
}
