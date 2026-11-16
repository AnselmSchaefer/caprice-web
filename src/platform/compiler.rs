//! Building a theme program from its one file (R36, the plan's step 4): in the browser by rustc and
//! wasm-ld built as WebAssembly, in a worker of their own (`web/compiler/compiler.js`), never in
//! Caprice's module or memory; on the desktop not yet; under `cargo test` by a stand-in (`played`),
//! for no test runs a compiler. Source in, the module or rustc's diagnostics out.
//!
//! The compiler starts loading the toolchain as it is made (52 MiB the first time, then from the
//! browser's cache), builds one source at a time, and is ended when it is dropped, which gives
//! back the gigabyte it holds while it lives. A build is the same bytes wherever it is made with the
//! same toolchain, so a theme kept as its source and the toolchain's id can be built again.

// On the desktop nothing is built, so what the others read goes unread there.
#![cfg_attr(all(not(test), not(target_arch = "wasm32")), allow(dead_code))]

use super::Waker;

/// The most of a source a build takes, as a theme file holds it (`bundle`).
pub const MOST_SOURCE: usize = 512 << 10;
/// Seconds a build may take before the compiler is ended: the Snow's takes 2–4.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const BUILD_SECONDS: f64 = 120.0;

/// What a build came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Built {
    /// The module, the toolchain that built it (its id), and how long it took.
    Module { module: Vec<u8>, toolchain: String, ms: u32 },
    /// The source has mistakes: rustc's diagnostics.
    Errors(String),
}

/// Where the compiler is.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Loading the toolchain: the bytes fetched so far of those it has to (0 of 0 from the cache).
    Loading { fetched: u64, of: u64 },
    Ready,
    Building,
    /// It cannot build: why, and whether it is this browser or this build that cannot.
    Failed { why: String, unsupported: bool },
}

/// The toolchain, loaded or loading, building what it is given.
pub struct Compiler(Runner);

impl Compiler {
    /// A compiler, its toolchain loading; `wake` asks for a frame when it has news.
    pub fn start(wake: Waker) -> Compiler {
        Compiler(Runner::start(wake))
    }

    /// `source` built once the toolchain is loaded, after any build before it.
    pub fn build(&mut self, source: &str) {
        self.0.build(source);
    }

    /// The newest build's result, once.
    pub fn take(&mut self) -> Option<Built> {
        self.0.take()
    }

    pub fn state(&mut self, now: f64) -> State {
        self.0.state(now)
    }
}

// ---- In the browser: a worker of its own ---------------------------------------------------

#[cfg(target_arch = "wasm32")]
use browser::Runner;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

    use super::super::program::Waiting;
    use super::{BUILD_SECONDS, Built, State, Waker};

    /// What the worker has said.
    struct Inbox {
        state: State,
        built: Option<Built>,
    }

    pub struct Runner {
        worker: Option<Worker>,
        inbox: Rc<RefCell<Inbox>>,
        /// Builds asked for and not yet answered, and since when the first of them has been on its way.
        asked: usize,
        waiting: Option<Waiting>,
        _heard: Closure<dyn FnMut(MessageEvent)>,
        _failed: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    }

    impl Runner {
        pub fn start(wake: Waker) -> Runner {
            let inbox = Rc::new(RefCell::new(Inbox { state: State::Loading { fetched: 0, of: 0 }, built: None }));
            let heard = {
                let (inbox, wake) = (inbox.clone(), wake.clone());
                Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                    let data = e.data();
                    let get = |k: &str| js_sys::Reflect::get(&data, &k.into()).unwrap_or(JsValue::UNDEFINED);
                    let mut inbox = inbox.borrow_mut();
                    let loading = get("loading");
                    if !loading.is_undefined() {
                        let n = |k: &str| js_sys::Reflect::get(&loading, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as u64;
                        inbox.state = State::Loading { fetched: n("fetched"), of: n("of") };
                    } else if get("ready").is_string() {
                        inbox.state = State::Ready;
                    } else if let Ok(module) = get("built").dyn_into::<js_sys::ArrayBuffer>() {
                        let module = js_sys::Uint8Array::new(&module).to_vec();
                        let toolchain = get("toolchain").as_string().unwrap_or_default();
                        let ms = get("ms").as_f64().unwrap_or(0.0) as u32;
                        inbox.built = Some(Built::Module { module, toolchain, ms });
                    } else if let Some(errors) = get("errors").as_string() {
                        inbox.built = Some(Built::Errors(errors));
                    } else if let Some(why) = get("failed").as_string() {
                        inbox.state = State::Failed { why, unsupported: get("unsupported").is_truthy() };
                    }
                    wake.wake();
                })
            };
            let failed = {
                let (inbox, wake) = (inbox.clone(), wake);
                Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |e: web_sys::ErrorEvent| {
                    inbox.borrow_mut().state = State::Failed { why: format!("its worker failed: {}", e.message()), unsupported: false };
                    wake.wake();
                })
            };
            let options = WorkerOptions::new();
            options.set_type(WorkerType::Module);
            let worker = Worker::new_with_options("/compiler/compiler.js", &options).ok();
            match &worker {
                Some(w) => {
                    w.set_onmessage(Some(heard.as_ref().unchecked_ref()));
                    w.set_onerror(Some(failed.as_ref().unchecked_ref()));
                }
                None => inbox.borrow_mut().state = State::Failed { why: "its worker could not be started".into(), unsupported: false },
            }
            Runner { worker, inbox, asked: 0, waiting: None, _heard: heard, _failed: failed }
        }

        pub fn build(&mut self, source: &str) {
            let Some(worker) = &self.worker else { return };
            let message = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&message, &"build".into(), &source.into());
            if let Err(e) = worker.post_message(&message) {
                self.inbox.borrow_mut().state = State::Failed { why: format!("the source could not be sent to it: {e:?}"), unsupported: false };
                return;
            }
            self.asked += 1;
        }

        pub fn take(&mut self) -> Option<Built> {
            let built = self.inbox.borrow_mut().built.take();
            if built.is_some() {
                self.asked = self.asked.saturating_sub(1);
                self.waiting = None;
            }
            built
        }

        pub fn state(&mut self, now: f64) -> State {
            let state = self.inbox.borrow().state.clone();
            if state != State::Ready || self.asked == 0 {
                return state;
            }
            // Building: a build that never ends (rustc can be given a source that keeps it busy) is
            // ended, counting only the time Caprice was there to hear it, as a theme's frame is.
            let waiting = self.waiting.get_or_insert(Waiting::new(now, BUILD_SECONDS));
            if waiting.late(now) {
                if let Some(w) = self.worker.take() {
                    w.terminate();
                }
                let why = format!("the build had not finished after {} seconds", BUILD_SECONDS as u32);
                self.inbox.borrow_mut().state = State::Failed { why, unsupported: false };
                return self.state(now);
            }
            State::Building
        }
    }

    impl Drop for Runner {
        fn drop(&mut self) {
            if let Some(w) = self.worker.take() {
                w.terminate();
            }
        }
    }
}

// ---- Under `cargo test`: a stand-in -------------------------------------------------------------

#[cfg(all(test, not(target_arch = "wasm32")))]
pub use played::stand_in;

#[cfg(all(test, not(target_arch = "wasm32")))]
use played::Runner;

/// No test runs a compiler: a source builds to what a test says it does (`stand_in`), and the
/// sources of `themes/` to their modules (`tests/fixtures/*.wasm`), which they are: CI builds them
/// again and fails if one differs, and the toolchain is packed only if each is (`toolchain/pack.mjs`).
/// Anything else has a mistake rustc would say is one. The stand-in is ready at once and answers
/// each build at the next look.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod played {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::{Built, State, Waker};

    /// The toolchain the tests' builds are said to be made by.
    pub const TOOLCHAIN: &str = "the tests' stand-in";

    thread_local! {
        static STANDS: RefCell<HashMap<String, Result<Built, State>>> = RefCell::new(HashMap::new());
    }

    /// `source` builds to `built` on this test's thread; or the compiler fails, if it is a `State`.
    pub fn stand_in(source: &str, built: Result<Built, State>) {
        STANDS.with_borrow_mut(|s| s.insert(source.to_owned(), built));
    }

    fn built(source: &str) -> Result<Built, State> {
        if let Some(b) = STANDS.with_borrow(|s| s.get(source).cloned()) {
            return b;
        }
        let themes: [(&str, &[u8]); 4] = [
            (include_str!("../../themes/echo/theme.rs"), include_bytes!("../../tests/fixtures/echo.wasm")),
            (include_str!("../../themes/fireflies/theme.rs"), include_bytes!("../../tests/fixtures/fireflies.wasm")),
            (include_str!("../../themes/snow/theme.rs"), include_bytes!("../../tests/fixtures/snow.wasm")),
            (include_str!("../../themes/vineyard/theme.rs"), include_bytes!("../../tests/fixtures/vineyard.wasm")),
        ];
        Ok(match themes.iter().find(|(s, _)| *s == source) {
            Some((_, module)) => Built::Module { module: module.to_vec(), toolchain: TOOLCHAIN.into(), ms: 1 },
            None => Built::Errors("error: the tests' stand-in builds only the themes of themes/ and what a test says\n --> /theme/theme.rs:1:1\n".into()),
        })
    }

    pub struct Runner {
        built: Vec<Built>,
        failed: Option<State>,
    }

    impl Runner {
        pub fn start(_wake: Waker) -> Runner {
            Runner { built: Vec::new(), failed: None }
        }

        pub fn build(&mut self, source: &str) {
            match built(source) {
                Ok(b) => self.built.push(b),
                Err(state) => self.failed = Some(state),
            }
        }

        pub fn take(&mut self) -> Option<Built> {
            (!self.built.is_empty()).then(|| self.built.remove(0))
        }

        pub fn state(&mut self, _now: f64) -> State {
            match (&self.failed, self.built.is_empty()) {
                (Some(f), _) => f.clone(),
                (None, true) => State::Ready,
                (None, false) => State::Building,
            }
        }
    }
}

// ---- On the desktop: none yet ---------------------------------------------------------------------

#[cfg(all(not(test), not(target_arch = "wasm32")))]
use desktop::Runner;

#[cfg(all(not(test), not(target_arch = "wasm32")))]
mod desktop {
    use super::{Built, State, Waker};

    /// Themes are built in the browser only, for now; on the desktop the compiler says so.
    pub struct Runner;

    impl Runner {
        pub fn start(_wake: Waker) -> Runner {
            Runner
        }
        pub fn build(&mut self, _source: &str) {}
        pub fn take(&mut self) -> Option<Built> {
            None
        }
        pub fn state(&mut self, _now: f64) -> State {
            State::Failed { why: "Themes are built from their source in Caprice in the browser only, for now.".into(), unsupported: true }
        }
    }
}
