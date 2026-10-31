//! What the desktop and the browser do differently, kept in one place. The rest of the app asks
//! here for the time, for background work, for files and the data folder, and for file dialogs, so
//! it reads the same on both. On the desktop each of these is what Caprice always did; in the
//! browser, where `std` compiles but panics or fails when run, each has its own way. The `std`
//! calls that panic there are disallowed everywhere else (`clippy.toml`).

#![allow(clippy::disallowed_methods)]

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;

pub mod fs;

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

/// The app's data folder, where its versions and settings are kept, as each system has it. In the
/// browser it is a folder among the browser's own files (`fs`).
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    return Some(PathBuf::from("/caprice"));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if cfg!(target_os = "macos") {
            return home.map(|h| h.join("Library/Application Support/Caprice Web"));
        }
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute());
        data.or_else(|| home.map(|h| h.join(".local/share"))).map(|d| d.join("caprice-web"))
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

/// Seconds east of UTC where the writer is, if it can be found: on the desktop from `date`, in
/// the browser from the page's clock.
pub fn utc_offset() -> Option<i64> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let out = std::process::Command::new("date").arg("+%z").output().ok()?;
        crate::words::utc_offset(String::from_utf8_lossy(&out.stdout).trim())
    }
    #[cfg(target_arch = "wasm32")]
    return Some(-(js_sys::Date::new_0().get_timezone_offset() as i64) * 60);
}

/// A number for this run of the app, to tell it from another one started at the same moment. A
/// page has no process id (asking `std` for one panics), so it draws one at random.
pub fn run_id() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    return std::process::id() as u64;
    #[cfg(target_arch = "wasm32")]
    return (js_sys::Math::random() * u32::MAX as f64) as u64;
}

/// Is the page hidden (another tab shown, the window minimised, or the tab closing)? Never on the
/// desktop, where closing has its own question.
pub fn page_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web_sys::window().and_then(|w| w.document()).is_some_and(|d| d.hidden());
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// In the browser: a frame as soon as the page is hidden or shown, so the app sees it at once
/// (`App::page_hidden`) rather than at its next frame, which may come too late for a tab closing.
#[cfg(target_arch = "wasm32")]
pub fn frame_on_visibility_change(ctx: &egui::Context) {
    use wasm_bindgen::JsCast;
    let ctx = ctx.clone();
    let wake = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || ctx.request_repaint());
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        let _ = document.add_event_listener_with_callback("visibilitychange", wake.as_ref().unchecked_ref());
    }
    // It listens as long as the page is there.
    wake.forget();
}

/// In the browser: whether leaving the page now would lose changes, so that the browser asks
/// first ("Leave site?"). It cannot ask Caprice's own question: the page is going.
pub fn warn_on_leaving(on: bool) {
    #[cfg(target_arch = "wasm32")]
    browser::warn_on_leaving(on);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = on;
}

/// A file dialog to show: the kinds of file it offers, and the name it suggests.
#[derive(Clone, Default)]
pub struct Dialog {
    pub filters: Vec<(String, Vec<String>)>,
    pub name: Option<String>,
}

impl Dialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.filters.push((name.to_owned(), extensions.iter().map(|e| (*e).to_owned()).collect()));
        self
    }

    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Show the system's dialog, to choose a file to read (`pick`) or where to write one; it waits
    /// for the writer, so it runs off the frame.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn show(self, pick: bool) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new();
        for (name, extensions) in &self.filters {
            d = d.add_filter(name, extensions);
        }
        if let Some(name) = &self.name {
            d = d.set_file_name(name);
        }
        if pick { d.pick_file() } else { d.save_file() }
    }

    /// What the browser's file picker takes: the extensions offered, as `.png,.jpg`.
    #[cfg(target_arch = "wasm32")]
    fn accept(&self) -> String {
        self.filters.iter().flat_map(|(_, exts)| exts.iter().map(|e| format!(".{e}"))).collect::<Vec<_>>().join(",")
    }
}

/// Where a file written to be given to the writer goes in the browser: written there, it is
/// downloaded rather than kept (`fs`).
#[cfg(target_arch = "wasm32")]
pub const DOWNLOADS: &str = "/downloads";
/// Where a file the writer gives the browser is put to be read: kept in memory only.
#[cfg(target_arch = "wasm32")]
pub const UPLOADS: &str = "/uploads";

/// In the browser: let the writer pick a file from their computer, put it in the folder `into`
/// among the browser's files (under a name of its own there) and send its path on `tx`; `None`
/// if they cancel. The picker is the page's, so this stays on the main thread.
#[cfg(target_arch = "wasm32")]
pub fn upload(dialog: &Dialog, into: PathBuf, tx: std::sync::mpsc::Sender<Option<PathBuf>>, wake: Waker) {
    let accept = dialog.accept();
    wasm_bindgen_futures::spawn_local(async move {
        let picked = browser::pick_file(&accept).await;
        let path = picked.and_then(|(name, bytes)| {
            let path = free_name(&into, &name);
            fs::write(&path, bytes).ok().map(|()| path)
        });
        let _ = tx.send(path);
        wake.wake();
    });
}

/// `name` in `dir`, or, if a file has it already, `name 2`, `name 3`…
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn free_name(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = std::path::Path::new(name);
    let stem = path.file_stem().map_or(name.into(), |s| s.to_string_lossy());
    let ext = path.extension().map(|e| e.to_string_lossy());
    (1..)
        .map(|k| {
            let stem = if k == 1 { stem.to_string() } else { format!("{stem} {k}") };
            dir.join(ext.as_ref().map_or(stem.clone(), |e| format!("{stem}.{e}")))
        })
        .find(|p| !fs::exists(p))
        .unwrap_or_else(|| dir.join(name))
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::prelude::*;

    // What the page does for files: its picker, downloads, a PDF in a tab, and fetching its own.
    // Bytes going out are copied out of the app's memory first, which is shared with the workers
    // and so cannot go into a Blob as it is.
    // The page's own ways first, where Caprice wants another: the browser asks before leaving
    // with changes not kept, and Caprice's shortcuts (`editor.rs`, `App::frame`) are not the
    // browser's (Ctrl+R aligns right rather than reloading the page, Ctrl+F finds in the story).
    // Copy, cut and paste stay the browser's: egui takes them from its events. Only in the page,
    // not in a worker, which imports this too.
    #[wasm_bindgen(inline_js = r#"
        let warn = false;
        export function warn_on_leaving(on) { warn = on; }
        if (typeof document !== "undefined") {
            addEventListener("beforeunload", e => { if (warn) { e.preventDefault(); e.returnValue = ""; } });
            const ours = new Set(["z", "y", "enter", "l", "e", "j", "b", "i", "u", "a", "f", "h", "n", "p", "=", "+", "-", "0"]);
            addEventListener("keydown", e => {
                if (!(e.ctrlKey || e.metaKey)) return;
                const key = e.key.toLowerCase();
                // Ctrl+Shift+R still reloads; Caprice has no use for it.
                if (ours.has(key) || (key === "r" && !e.shiftKey)) e.preventDefault();
            }, { capture: true });
        }
        export function pick_file(accept) {
            return new Promise(done => {
                const input = document.createElement("input");
                input.type = "file";
                input.accept = accept;
                input.addEventListener("change", async () => {
                    const f = input.files && input.files[0];
                    done(f ? [f.name, new Uint8Array(await f.arrayBuffer())] : null);
                });
                input.addEventListener("cancel", () => done(null));
                input.click();
            });
        }
        export async function fetch_bytes(url) {
            const r = await fetch(url);
            if (!r.ok) throw new Error(`${url}: ${r.status}`);
            return new Uint8Array(await r.arrayBuffer());
        }
        export function show_pdf(bytes) {
            const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "application/pdf" }));
            window.open(url, "_blank");
            setTimeout(() => URL.revokeObjectURL(url), 600000);
        }
        export function download(name, bytes) {
            const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
            const a = document.createElement("a");
            a.href = url;
            a.download = name;
            document.body.appendChild(a);
            a.click();
            a.remove();
            setTimeout(() => URL.revokeObjectURL(url), 30000);
        }
    "#)]
    extern "C" {
        #[wasm_bindgen(js_name = pick_file)]
        async fn js_pick_file(accept: &str) -> JsValue;
        #[wasm_bindgen(js_name = download)]
        fn js_download(name: &str, bytes: &[u8]);
        pub fn warn_on_leaving(on: bool);
        #[wasm_bindgen(js_name = show_pdf)]
        fn js_show_pdf(bytes: &[u8]);
        #[wasm_bindgen(js_name = fetch_bytes, catch)]
        async fn js_fetch_bytes(url: &str) -> Result<JsValue, JsValue>;
    }

    /// The file at `url`, from where the page came from.
    pub async fn fetch(url: &str) -> Result<Vec<u8>, String> {
        let bytes = js_fetch_bytes(url).await.map_err(|e| format!("{e:?}"))?;
        Ok(js_sys::Uint8Array::new(&bytes).to_vec())
    }

    /// The file the writer picks: its name and bytes.
    pub async fn pick_file(accept: &str) -> Option<(String, Vec<u8>)> {
        let picked = js_pick_file(accept).await;
        if picked.is_null() || picked.is_undefined() {
            return None;
        }
        let picked = js_sys::Array::from(&picked);
        Some((picked.get(0).as_string()?, js_sys::Uint8Array::new(&picked.get(1)).to_vec()))
    }

    pub fn download(name: &str, bytes: &[u8]) {
        js_download(name, bytes);
    }

    /// A PDF in a new tab, where the browser shows it and can print it.
    pub fn show_pdf(bytes: &[u8]) {
        js_show_pdf(bytes);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use browser::{download, fetch, show_pdf};
