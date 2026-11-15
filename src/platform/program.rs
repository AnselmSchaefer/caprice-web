//! A theme program run where nothing it does can reach the app (R36): in the browser in a worker
//! of its own (`web/theme-worker.js`), ended when it does not answer; under `cargo test` by wasmi,
//! with fuel; on the desktop not at all. Caprice gives it, each frame, the canonical ABI's bytes of
//! the frame's input and of the events since the last (`eggs::program` writes them), with the
//! answers to `host`; it gives back the frame's output as flat words (`Words`). The frame never
//! waits: it draws the newest words it has, so a program's shapes are a frame behind its input.

// On the desktop nothing is run, so what the other two read goes unread there.
#![cfg_attr(all(not(test), not(target_arch = "wasm32")), allow(dead_code))]

use super::Waker;

/// `frame-input` as the canonical ABI lays it out, in bytes.
pub const INPUT_LEN: usize = 120;
/// What a frame and the events before it may cost under wasmi (tests only): about six times what
/// the Magic Snow's frame costs.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub const FUEL: u64 = 200_000_000;
/// Seconds a program has to load, and then to answer a frame, before it is ended (in the browser:
/// under wasmi it is run to its end, or out of fuel).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const LOAD_SECONDS: f64 = 5.0;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const ANSWER_SECONDS: f64 = 1.0;
/// The interface a program may import from, and nothing else (`eggs::program::check`).
pub const HOST: &str = "caprice:theme/host@1.0.0";

/// What Caprice sends a program for a frame: its input and the events before it, as the canonical
/// ABI lays them out (no pointers in either), and what `host` answers meanwhile.
#[derive(Clone, Debug)]
pub struct Ask {
    pub input: [u8; INPUT_LEN],
    pub events: Vec<u8>,
    pub seed: u64,
    pub garden: Vec<u32>,
    pub pages: u32,
    /// What `ask-lift` answers: whether Caprice will bring titles with this theme's carrier.
    pub lift: bool,
}

/// A frame's output as flat words: the shapes behind the page and over it, how many of each, when
/// the next frame is wanted, the carried title's offset, and whether the program asked to carry
/// titles; then the shapes, desk first. A circle is 1, colour, x, y, r; a convex polygon 2,
/// colour, n, n points; a line 3, colour, width, two points; a mesh 4, vertices, indices, each
/// vertex (x, y, colour), the indices. Both hosts (`web/theme-worker.js`, `lift` here) write them.
pub type Words = Vec<u32>;

/// The words' head: [desk shapes, over shapes, next, offset x, offset y, asked to lift].
pub const HEAD: usize = 6;

/// A program loaded, asked for frames, its newest answer taken.
pub struct Program(Runner);

impl Program {
    /// `module` (checked already, `eggs::program::check`) loaded where it is run; `wake` asks
    /// for a frame when its answer comes.
    pub fn load(module: &[u8], wake: Waker, now: f64) -> Program {
        Program(Runner::load(module, wake, now))
    }

    /// The next frame asked for, unless the last is still on its way (it is never queued).
    /// Whether it was asked.
    pub fn ask(&mut self, ask: &Ask, now: f64) -> bool {
        self.0.ask(ask, now)
    }

    /// The newest answer, once.
    pub fn take(&mut self) -> Option<Words> {
        self.0.take()
    }

    /// Why it is stopped, if it is: it trapped, could not load, or has not answered in time.
    pub fn stopped(&mut self, now: f64) -> Option<String> {
        self.0.stopped(now)
    }

    /// Ended by Caprice (it drew more than it may), never asked again.
    pub fn stop(&mut self, why: String) {
        self.0.stop(why);
    }
}

/// The program's `frame-output` at `ret` in its memory `m` lifted into words: the canonical ABI's
/// layouts (a shape is 28 bytes, its case in the first byte and its payload from the fourth),
/// every list checked to lie within the memory.
#[cfg_attr(not(test), allow(dead_code))]
pub fn lift(m: &[u8], ret: usize, asked: bool) -> Result<Words, String> {
    let word = |at: usize| m.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("its output lies outside its memory");
    let list = |at: usize, size: usize| -> Result<std::ops::Range<usize>, String> {
        let (ptr, len) = (word(at)? as usize, word(at + 4)? as usize);
        let end = len.checked_mul(size).and_then(|n| n.checked_add(ptr)).filter(|&e| e <= m.len());
        end.map(|e| ptr..e).ok_or_else(|| "a list of its output lies outside its memory".to_string())
    };
    let mut w = vec![0; HEAD];
    let (desk, over) = (list(ret, 28)?, list(ret + 8, 28)?);
    w[0] = (desk.len() / 28) as u32;
    w[1] = (over.len() / 28) as u32;
    (w[2], w[3], w[4], w[5]) = (word(ret + 16)?, word(ret + 20)?, word(ret + 24)?, u32::from(asked));
    for base in desk.step_by(28).chain(over.step_by(28)) {
        let p = base + 4;
        match m[base] {
            0 => w.extend([1, word(p + 12)?, word(p)?, word(p + 4)?, word(p + 8)?]),
            1 => {
                let points = list(p, 8)?;
                w.extend([2, word(p + 8)?, (points.len() / 8) as u32]);
                for q in points.step_by(4) {
                    w.push(word(q)?);
                }
            }
            2 => w.extend([3, word(p + 20)?, word(p + 16)?, word(p)?, word(p + 4)?, word(p + 8)?, word(p + 12)?]),
            3 => {
                let (vertices, indices) = (list(p, 12)?, list(p + 8, 4)?);
                w.extend([4, (vertices.len() / 12) as u32, (indices.len() / 4) as u32]);
                for q in vertices.step_by(4).chain(indices.step_by(4)) {
                    w.push(word(q)?);
                }
            }
            _ => return Err("a shape of no kind".into()),
        }
    }
    Ok(w)
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

    use super::{ANSWER_SECONDS, Ask, LOAD_SECONDS, Waker, Words};

    /// What the worker has said since it was last asked.
    #[derive(Default)]
    struct Inbox {
        words: Option<Words>,
        /// Its words' buffer, to be given back with the next frame asked for.
        buffer: Option<js_sys::ArrayBuffer>,
        answered: bool,
        stopped: Option<String>,
    }

    pub struct Runner {
        worker: Option<Worker>,
        inbox: Rc<RefCell<Inbox>>,
        /// When what is on its way (loading, a frame) was asked for, and how long it may take.
        asked: Option<(f64, f64)>,
        _heard: Closure<dyn FnMut(MessageEvent)>,
        _failed: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    }

    fn set(o: &js_sys::Object, key: &str, value: &JsValue) {
        let _ = js_sys::Reflect::set(o, &key.into(), value);
    }

    impl Runner {
        pub fn load(module: &[u8], wake: Waker, now: f64) -> Runner {
            let inbox = Rc::new(RefCell::new(Inbox::default()));
            let heard = {
                let (inbox, wake) = (inbox.clone(), wake.clone());
                Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                    let data = e.data();
                    let get = |k: &str| js_sys::Reflect::get(&data, &k.into()).unwrap_or(JsValue::UNDEFINED);
                    let mut inbox = inbox.borrow_mut();
                    if let Some(why) = get("stopped").as_string() {
                        inbox.stopped = Some(why);
                    } else if let Ok(buffer) = get("words").dyn_into::<js_sys::ArrayBuffer>() {
                        let n = get("n").as_f64().unwrap_or(0.0) as u32;
                        inbox.words = Some(js_sys::Uint32Array::new_with_byte_offset_and_length(&buffer, 0, n).to_vec());
                        inbox.buffer = Some(buffer);
                    }
                    inbox.answered = true;
                    wake.wake();
                })
            };
            let failed = {
                let (inbox, wake) = (inbox.clone(), wake);
                Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |e: web_sys::ErrorEvent| {
                    inbox.borrow_mut().stopped = Some(format!("its worker failed: {}", e.message()));
                    wake.wake();
                })
            };
            let options = WorkerOptions::new();
            options.set_type(WorkerType::Module);
            let worker = Worker::new_with_options("/theme-worker.js", &options).ok();
            match &worker {
                Some(w) => {
                    w.set_onmessage(Some(heard.as_ref().unchecked_ref()));
                    w.set_onerror(Some(failed.as_ref().unchecked_ref()));
                    let message = js_sys::Object::new();
                    set(&message, "load", &js_sys::Uint8Array::from(module).buffer().into());
                    if let Err(e) = w.post_message(&message) {
                        inbox.borrow_mut().stopped = Some(format!("it could not be sent to its worker: {e:?}"));
                    }
                }
                None => inbox.borrow_mut().stopped = Some("its worker could not be started".into()),
            }
            Runner { worker, inbox, asked: Some((now, LOAD_SECONDS)), _heard: heard, _failed: failed }
        }

        pub fn ask(&mut self, ask: &Ask, now: f64) -> bool {
            let mut inbox = self.inbox.borrow_mut();
            if inbox.stopped.is_some() || (self.asked.is_some() && !inbox.answered) {
                return false;
            }
            let Some(worker) = &self.worker else { return false };
            let message = js_sys::Object::new();
            set(&message, "input", &js_sys::Uint8Array::from(&ask.input[..]).buffer().into());
            set(&message, "events", &js_sys::Uint8Array::from(&ask.events[..]).buffer().into());
            set(&message, "seed", &js_sys::BigInt::from(ask.seed).into());
            set(&message, "garden", &js_sys::Uint32Array::from(&ask.garden[..]).into());
            set(&message, "pages", &ask.pages.into());
            set(&message, "lift", &ask.lift.into());
            let transfer = js_sys::Array::new();
            if let Some(buffer) = inbox.buffer.take() {
                set(&message, "buffer", &buffer);
                transfer.push(&buffer);
            }
            if let Err(e) = worker.post_message_with_transfer(&message, &transfer) {
                inbox.stopped = Some(format!("a frame could not be asked for: {e:?}"));
                return false;
            }
            inbox.answered = false;
            self.asked = Some((now, ANSWER_SECONDS));
            true
        }

        pub fn take(&mut self) -> Option<Words> {
            self.inbox.borrow_mut().words.take()
        }

        pub fn stopped(&mut self, now: f64) -> Option<String> {
            let late = {
                let inbox = self.inbox.borrow();
                !inbox.answered && inbox.stopped.is_none() && self.asked.is_some_and(|(at, most)| now - at > most)
            };
            if late {
                let what = if self.asked.is_some_and(|a| a.1 == LOAD_SECONDS) { "did not load" } else { "did not answer" };
                self.stop(format!("it {what} in time"));
            }
            self.inbox.borrow().stopped.clone()
        }

        pub fn stop(&mut self, why: String) {
            if let Some(w) = self.worker.take() {
                w.terminate();
            }
            self.inbox.borrow_mut().stopped.get_or_insert(why);
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

// ---- Under `cargo test`: wasmi, with fuel ----------------------------------------------------

#[cfg(all(test, not(target_arch = "wasm32")))]
use wasmi_host::Runner;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod wasmi_host {
    use wasmi::{Caller, Config, Engine, Error, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};

    use super::{Ask, FUEL, HOST, Waker, Words, lift};

    /// What `host` answers, from the frame asked for, and whether the program asked to lift.
    struct State {
        limits: StoreLimits,
        ask: Option<Ask>,
        asked: bool,
    }

    struct Loaded {
        store: Store<State>,
        memory: Memory,
        realloc: TypedFunc<(i32, i32, i32, i32), i32>,
        events: TypedFunc<(i32, i32), ()>,
        frame: TypedFunc<i32, i32>,
        post: TypedFunc<i32, ()>,
    }

    pub struct Runner {
        loaded: Option<Loaded>,
        words: Option<Words>,
        stopped: Option<String>,
        /// How much fuel the last frame used.
        pub used: u64,
    }

    fn text(e: impl std::fmt::Display) -> String {
        e.to_string()
    }

    fn instantiate(module: &[u8]) -> Result<Loaded, String> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, module).map_err(text)?;
        let limits = StoreLimitsBuilder::new().memory_size(32 << 20).trap_on_grow_failure(true).build();
        let mut store = Store::new(&engine, State { limits, ask: None, asked: false });
        store.limiter(|s| &mut s.limits);
        store.set_fuel(FUEL).map_err(text)?;
        let mut linker = Linker::<State>::new(&engine);
        let ask = |c: &Caller<'_, State>| c.data().ask.clone().ok_or_else(|| Error::new("asked outside a frame"));
        linker.func_wrap(HOST, "story-seed", move |c: Caller<'_, State>| ask(&c).map(|a| a.seed as i64)).map_err(text)?;
        linker.func_wrap(HOST, "page-count", move |c: Caller<'_, State>| ask(&c).map(|a| a.pages as i32)).map_err(text)?;
        linker.func_wrap(HOST, "ask-lift", move |mut c: Caller<'_, State>| {
            c.data_mut().asked = true;
            ask(&c).map(|a| i32::from(a.lift))
        }).map_err(text)?;
        // A list given to the program goes into memory it allocates (`cabi_realloc`), and where it
        // is into the place it named.
        linker.func_wrap(HOST, "garden", move |mut c: Caller<'_, State>, ret: i32| -> Result<(), Error> {
            let fail = |s: &str| Error::new(s.to_string());
            let bytes: Vec<u8> = ask(&c)?.garden.iter().flat_map(|p| p.to_le_bytes()).collect();
            let realloc = c.get_export("cabi_realloc").and_then(Extern::into_func).ok_or_else(|| fail("no cabi_realloc"))?;
            let memory = c.get_export("memory").and_then(Extern::into_memory).ok_or_else(|| fail("no memory"))?;
            let ptr = realloc.typed::<(i32, i32, i32, i32), i32>(&c)?.call(&mut c, (0, 0, 4, bytes.len() as i32))?;
            memory.write(&mut c, ptr as usize, &bytes).map_err(|_| fail("the garden lies outside its memory"))?;
            let at = [ptr.to_le_bytes(), (bytes.len() as i32 / 4).to_le_bytes()].concat();
            memory.write(&mut c, ret as usize, &at).map_err(|_| fail("its answer's place lies outside its memory"))?;
            Ok(())
        }).map_err(text)?;
        let instance = linker.instantiate_and_start(&mut store, &module).map_err(text)?;
        let memory = instance.get_memory(&store, "memory").ok_or("no memory")?;
        let get = |name: &str| instance.get_func(&store, name).ok_or(format!("no {name}"));
        Ok(Loaded {
            realloc: get("cabi_realloc")?.typed(&store).map_err(text)?,
            events: get("events")?.typed(&store).map_err(text)?,
            frame: get("frame")?.typed(&store).map_err(text)?,
            post: get("cabi_post_frame")?.typed(&store).map_err(text)?,
            store,
            memory,
        })
    }

    impl Loaded {
        /// Bytes put into memory the program allocates, and where.
        fn give(&mut self, bytes: &[u8], align: i32) -> Result<i32, String> {
            if bytes.is_empty() {
                return Ok(align);
            }
            let ptr = self.realloc.call(&mut self.store, (0, 0, align, bytes.len() as i32)).map_err(text)?;
            self.memory.write(&mut self.store, ptr as usize, bytes).map_err(text)?;
            Ok(ptr)
        }

        fn frame(&mut self, ask: &Ask) -> Result<(Words, u64), String> {
            self.store.set_fuel(FUEL).map_err(text)?;
            (self.store.data_mut().ask, self.store.data_mut().asked) = (Some(ask.clone()), false);
            let events = self.give(&ask.events, 4)?;
            self.events.call(&mut self.store, (events, (ask.events.len() / 12) as i32)).map_err(|e| why(&e))?;
            let input = self.give(&ask.input, 8)?;
            let ret = self.frame.call(&mut self.store, input).map_err(|e| why(&e))? as usize;
            let words = lift(self.memory.data(&self.store), ret, self.store.data().asked)?;
            self.post.call(&mut self.store, ret as i32).map_err(|e| why(&e))?;
            self.store.data_mut().ask = None;
            Ok((words, FUEL - self.store.get_fuel().map_err(text)?))
        }
    }

    /// Running out of fuel said as what it is: it ran too long.
    fn why(e: &Error) -> String {
        if e.as_trap_code() == Some(wasmi::TrapCode::OutOfFuel) { "it ran too long".into() } else { format!("it failed: {e}") }
    }

    impl Runner {
        pub fn load(module: &[u8], _wake: Waker, _now: f64) -> Runner {
            match instantiate(module) {
                Ok(l) => Runner { loaded: Some(l), words: None, stopped: None, used: 0 },
                Err(e) => Runner { loaded: None, words: None, stopped: Some(format!("it could not be loaded: {e}")), used: 0 },
            }
        }

        pub fn ask(&mut self, ask: &Ask, _now: f64) -> bool {
            let (Some(loaded), None) = (&mut self.loaded, &self.stopped) else { return false };
            match loaded.frame(ask) {
                Ok((words, used)) => (self.words, self.used) = (Some(words), used),
                Err(e) => self.stop(e),
            }
            true
        }

        pub fn take(&mut self) -> Option<Words> {
            self.words.take()
        }

        pub fn stopped(&mut self, _now: f64) -> Option<String> {
            self.stopped.clone()
        }

        pub fn stop(&mut self, why: String) {
            self.loaded = None;
            self.stopped.get_or_insert(why);
        }
    }
}

// ---- On the desktop: none ---------------------------------------------------------------------

#[cfg(all(not(test), not(target_arch = "wasm32")))]
use desktop::Runner;

#[cfg(all(not(test), not(target_arch = "wasm32")))]
mod desktop {
    use super::{Ask, Waker, Words};

    /// Program themes play in the browser only; on the desktop one is never run.
    pub struct Runner;

    impl Runner {
        pub fn load(_module: &[u8], _wake: Waker, _now: f64) -> Runner {
            Runner
        }
        pub fn ask(&mut self, _ask: &Ask, _now: f64) -> bool {
            false
        }
        pub fn take(&mut self) -> Option<Words> {
            None
        }
        pub fn stopped(&mut self, _now: f64) -> Option<String> {
            Some("themes that are programs play in Caprice in the browser only".into())
        }
        pub fn stop(&mut self, _why: String) {}
    }
}
