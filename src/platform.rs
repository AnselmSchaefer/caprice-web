//! What the desktop and the browser do differently, kept in one place. The rest of the app asks
//! here for the time, for background work, for a file dialog and (later) for the data folder, so
//! it reads the same on both. On the desktop each of these is what Caprice always did; in the
//! browser, where `std` compiles but panics or fails when run, each has its own way. The `std`
//! calls that panic there are disallowed everywhere else (`clippy.toml`).

#![allow(clippy::disallowed_methods)]

use std::time::Duration;

use eframe::egui;

#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Background work running in the browser's workers, and whether any has news since the last frame.
#[cfg(target_arch = "wasm32")]
static RUNNING: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_arch = "wasm32")]
static WOKEN: AtomicBool = AtomicBool::new(false);

/// Do `work` in the background: on a thread on the desktop, in a Web Worker in the browser. The
/// work gets owned data and gives its answer on a channel; in the browser it must share no lock with
/// the main thread, for a page's main thread that waits on one is stopped dead (Chromium).
pub fn spawn(work: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(work);
    #[cfg(target_arch = "wasm32")]
    {
        RUNNING.fetch_add(1, Ordering::SeqCst);
        wasm_thread::spawn(move || {
            work();
            RUNNING.fetch_sub(1, Ordering::SeqCst);
            WOKEN.store(true, Ordering::SeqCst);
        });
    }
}

/// How background work asks for a frame when it has something to show. On the desktop it asks egui;
/// in the browser egui's `Context` cannot go to a worker at all, so the work leaves word here and
/// the main thread asks for the frame (`keep_awake`).
#[derive(Clone)]
pub struct Waker(#[cfg(not(target_arch = "wasm32"))] egui::Context);

impl Waker {
    pub fn new(ctx: &egui::Context) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        return Waker(ctx.clone());
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            Waker()
        }
    }

    pub fn wake(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.0.request_repaint();
        #[cfg(target_arch = "wasm32")]
        WOKEN.store(true, Ordering::SeqCst);
    }
}

/// Called each frame. In the browser: a frame at once if background work has news, and every
/// 50 ms while any is still running, so what it sends is never left waiting.
pub fn keep_awake(ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = ctx;
    #[cfg(target_arch = "wasm32")]
    if WOKEN.swap(false, Ordering::SeqCst) {
        ctx.request_repaint();
    } else if RUNNING.load(Ordering::SeqCst) > 0 {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

/// The time now, since 1970. In the browser the system clock of `std` panics; the page's clock is
/// `Date.now()`, in milliseconds.
pub fn since_epoch() -> Duration {
    #[cfg(not(target_arch = "wasm32"))]
    return std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    #[cfg(target_arch = "wasm32")]
    return Duration::from_secs_f64(js_sys::Date::now() / 1000.0);
}

/// A number for this run of the app, to tell it from another one started at the same moment. A
/// page has no process id (asking `std` for one panics), so it draws one at random.
pub fn run_id() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    return std::process::id() as u64;
    #[cfg(target_arch = "wasm32")]
    return (js_sys::Math::random() * u32::MAX as f64) as u64;
}

/// The system's file dialog on the desktop.
#[cfg(not(target_arch = "wasm32"))]
pub use rfd::FileDialog;

/// Until the browser has its own way to open and give files (WEB_PLAN step 3), a dialog there
/// chooses nothing, as if it were cancelled.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct FileDialog;

#[cfg(target_arch = "wasm32")]
impl FileDialog {
    pub fn new() -> Self {
        FileDialog
    }

    pub fn add_filter(self, _name: impl Into<String>, _extensions: &[impl ToString]) -> Self {
        self
    }

    pub fn set_file_name(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn pick_file(self) -> Option<std::path::PathBuf> {
        None
    }

    pub fn save_file(self) -> Option<std::path::PathBuf> {
        None
    }
}
