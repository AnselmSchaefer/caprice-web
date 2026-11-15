//! A theme program run where nothing it does can reach the app (R36): in the browser in a worker
//! of its own (`web/theme-worker.js`), ended when it does not answer; on the desktop not at all;
//! under `cargo test` no module is run, the theme's source is played natively in its place
//! (`played`). Caprice gives it, each frame, the canonical ABI's bytes of the frame's input and of
//! the events since the last (`eggs::program` writes them), with the answers to `host`; it gives
//! back the frame's output as flat words (`Words`). The frame never waits: it draws the newest
//! words it has, so a program's shapes are a frame behind its input.

// On the desktop nothing is run, so what the other two read goes unread there.
#![cfg_attr(all(not(test), not(target_arch = "wasm32")), allow(dead_code))]

use super::Waker;

/// `frame-input` as the canonical ABI lays it out, in bytes.
pub const INPUT_LEN: usize = 144;
/// Seconds a program has to load, and then to answer a frame, before it is ended.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const LOAD_SECONDS: f64 = 5.0;
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
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

/// A frame's output as flat words: how many shapes there are on the desk, in the window and over
/// the page, when the next frame is wanted, the carried title's offset, whether the program asked
/// to carry titles, and how high its ground is; then the shapes, the desk's first, then the
/// window's, then those over the page. A circle is 1, colour, x, y, r; a convex polygon 2, colour,
/// n, n points; a line 3, colour, width, two points; a mesh 4, vertices, indices, each vertex (x,
/// y, colour), the indices; a path 5, colour, width, closed, n, n points. The browser's host
/// (`web/theme-worker.js`) makes them out of the program's memory, and the tests' player
/// (`played`) out of a theme played natively.
pub type Words = Vec<u32>;

/// The words' head: [desk shapes, window shapes, over shapes, next, offset x, offset y, asked to
/// lift, ground].
pub const HEAD: usize = 8;

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

// ---- Under `cargo test`: the theme's source, played natively ---------------------------------

#[cfg(all(test, not(target_arch = "wasm32")))]
pub use played::{Stand, stand_in};

#[cfg(all(test, not(target_arch = "wasm32")))]
use played::Runner;

/// No WebAssembly is run under `cargo test`: a module stands for something played natively
/// (`Stand`). The modules of `themes/` (`tests/fixtures/*.wasm`) are their own sources built for
/// the machine against the SDK (`themes/played`); a module a test makes says what it stands for
/// (`stand_in`). The player gives a theme the input and the events as the browser's host does,
/// read out of Caprice's bytes, and makes its output into the same words: `web/theme-worker.test.mjs`
/// holds the two to it, through the echo theme, which draws back everything it is told.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod played {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use theme_sdk::native::{Answers, Desk, FrameInput, FrameOutput, Pages, Played, answering};
    use theme_sdk::{Carrier, Event, Point, Pose, Rect, Shape};

    use super::{ANSWER_SECONDS, Ask, HEAD, Waker, Words};

    /// What a module stands for when the tests run it.
    pub enum Stand {
        /// A theme, played from its source.
        Theme(Box<dyn Played>),
        /// The same words every frame, as a program that draws them would make.
        Words(Words),
        /// A frame that fails, as one that traps does: why.
        Fails(&'static str),
        /// A frame that never comes, as one that never ends: it is stopped after `ANSWER_SECONDS`.
        Silent,
    }

    /// What makes a stand-in, each time its module is loaded.
    type Making = Box<dyn Fn() -> Stand>;

    thread_local! {
        static STANDS: RefCell<HashMap<Vec<u8>, Making>> = RefCell::new(HashMap::new());
    }

    /// `module` stands for what `stand` makes, on this test's thread, whenever it is loaded.
    pub fn stand_in(module: &[u8], stand: impl Fn() -> Stand + 'static) {
        STANDS.with_borrow_mut(|s| s.insert(module.to_vec(), Box::new(stand)));
    }

    /// What `module` stands for: a theme of `themes/` as `themes/build.sh` built it, or what a test said.
    fn stand_for(module: &[u8]) -> Option<Stand> {
        use themes_played::*;
        type Theme = fn() -> Box<dyn Played>;
        let built: [(&[u8], Theme); 4] = [
            (include_bytes!("../../tests/fixtures/echo.wasm"), echo::played),
            (include_bytes!("../../tests/fixtures/fireflies.wasm"), fireflies::played),
            (include_bytes!("../../tests/fixtures/snow.wasm"), snow::played),
            (include_bytes!("../../tests/fixtures/vineyard.wasm"), vineyard::played),
        ];
        if let Some((_, played)) = built.iter().find(|(m, _)| *m == module) {
            return Some(Stand::Theme(played()));
        }
        STANDS.with_borrow(|s| s.get(module).map(|stand| stand()))
    }

    /// The bytes of `input_bytes` read as the world's `frame-input`, at the canonical ABI's places.
    fn input(b: &[u8]) -> FrameInput {
        let f = |at: usize| f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
        let point = |at: usize| Point { x: f(at), y: f(at + 4) };
        let rect = |at: usize| Rect { min: point(at), max: point(at + 8) };
        let time = f64::from_le_bytes(b[0..8].try_into().unwrap());
        let desk = Desk { area: rect(8), page: rect(24), bar: (b[40] == 1).then(|| rect(44)), zoom: f(60), unit: f(64), window: rect(68), ground: point(84) };
        let carrier = (b[92] == 1).then(|| Carrier {
            pose: Pose { enter: f(96), run: f(100), lower: f(104), hooked: b[108] == 1, hook: f(112), look: f(116), t: f(120) },
            from_left: b[124] == 1,
            title: rect(128),
        });
        FrameInput { time, desk, carrier }
    }

    /// The bytes of `event_bytes` read as the world's `list<event>`.
    fn events(b: &[u8]) -> Vec<Event> {
        b.chunks(12).map(|e| {
            let word = |at: usize| u32::from_le_bytes([e[at], e[at + 1], e[at + 2], e[at + 3]]);
            match e[0] {
                0 => Event::Fresh,
                1 => Event::Grown(Pages { chapter: word(4), pages: word(8) }),
                2 => Event::Begun(word(4)),
                _ => Event::Clicked(Point { x: f32::from_bits(word(4)), y: f32::from_bits(word(8)) }),
            }
        }).collect()
    }

    /// A theme's output made into the words the browser's host makes of it (`Words`).
    fn words(out: &FrameOutput, asked: bool) -> Words {
        let f = f32::to_bits;
        let mut w = vec![out.desk.len() as u32, out.window.len() as u32, out.over.len() as u32, out.next, f(out.title_offset.x), f(out.title_offset.y), u32::from(asked), f(out.ground)];
        debug_assert_eq!(w.len(), HEAD);
        let points = |w: &mut Words, ps: &[Point]| w.extend(ps.iter().flat_map(|p| [f(p.x), f(p.y)]));
        for shape in out.desk.iter().chain(&out.window).chain(&out.over) {
            match shape {
                Shape::Circle(c) => w.extend([1, c.color, f(c.at.x), f(c.at.y), f(c.r)]),
                Shape::Polygon(p) => {
                    w.extend([2, p.color, p.points.len() as u32]);
                    points(&mut w, &p.points);
                }
                Shape::Line(l) => w.extend([3, l.color, f(l.width), f(l.a.x), f(l.a.y), f(l.b.x), f(l.b.y)]),
                Shape::Mesh(m) => {
                    w.extend([4, m.vertices.len() as u32, m.indices.len() as u32]);
                    w.extend(m.vertices.iter().flat_map(|v| [f(v.at.x), f(v.at.y), v.color]));
                    w.extend(&m.indices);
                }
                Shape::Path(p) => {
                    w.extend([5, p.color, f(p.width), u32::from(p.closed), p.points.len() as u32]);
                    points(&mut w, &p.points);
                }
            }
        }
        w
    }

    pub struct Runner {
        stand: Option<Stand>,
        words: Option<Words>,
        stopped: Option<String>,
        /// When the frame that never came was asked for.
        asked: Option<f64>,
    }

    impl Runner {
        pub fn load(module: &[u8], _wake: Waker, _now: f64) -> Runner {
            let stand = stand_for(module);
            let stopped = stand.is_none().then(|| "it could not be loaded: under the tests a module stands for a theme of themes/ or what a test says".to_string());
            Runner { stand, words: None, stopped, asked: None }
        }

        pub fn ask(&mut self, ask: &Ask, now: f64) -> bool {
            if self.stopped.is_some() || self.asked.is_some() {
                return false;
            }
            match &mut self.stand {
                Some(Stand::Theme(theme)) => {
                    let answers = Answers { seed: ask.seed, garden: ask.garden.clone(), pages: ask.pages, lift: ask.lift };
                    let (out, asked) = answering(answers, || {
                        theme.events(&events(&ask.events));
                        theme.frame(input(&ask.input))
                    });
                    self.words = Some(words(&out, asked));
                }
                Some(Stand::Words(w)) => self.words = Some(w.clone()),
                Some(Stand::Fails(why)) => {
                    let why = format!("it failed: {why}");
                    self.stop(why);
                }
                Some(Stand::Silent) => self.asked = Some(now),
                None => return false,
            }
            true
        }

        pub fn take(&mut self) -> Option<Words> {
            self.words.take()
        }

        pub fn stopped(&mut self, now: f64) -> Option<String> {
            if self.asked.is_some_and(|at| now - at > ANSWER_SECONDS) {
                self.stop("it did not answer in time".into());
            }
            self.stopped.clone()
        }

        pub fn stop(&mut self, why: String) {
            self.stand = None;
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
