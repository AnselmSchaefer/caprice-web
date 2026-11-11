//! Starting again where the writer was. A phone's browser takes the GPU away from a page left in
//! the background (Chrome on Android loses the page's WebGL context or WebGPU device), and eframe
//! cannot draw again without it: back in the browser, the page stayed blank. Caprice cannot give
//! eframe a new one, so it loads the page again, which gets one, and comes back to what was shown:
//! the story as it was (saved or not, with a file or without), the caret and the selection, and
//! which device it is in the story's room. They are written to a file of their own just before,
//! read once as the page opens, and the file taken away. Undo starts afresh.

use std::path::PathBuf;

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::App;
use crate::fileio::DocFile;
use crate::platform::{self, fs};

/// Where the writer was: the story shown, and what the app knew of its saving.
#[derive(Serialize, Deserialize)]
pub struct Place {
    file: DocFile,
    path: Option<PathBuf>,
    saved_hash: u64,
    as_saved: Option<(u64, String)>,
    saved_at: Option<u64>,
    caret: usize,
    anchor: usize,
    /// This device in the story's room (R34): back as the same one, it keeps the pen, rather than
    /// being locked out by itself for as long as the room keeps the pen for a device gone.
    #[serde(default)]
    device: String,
}

pub struct Again {
    /// The file the place is written to (None: nowhere, as in tests that do not ask for it).
    file: Option<PathBuf>,
    /// The place read as the page opened, to go back to at the first frame.
    pub waiting: Option<Place>,
    /// The page is being loaded again: once is enough.
    going: bool,
}

impl Again {
    pub fn new() -> Self {
        Self::kept_in(platform::data_dir().map(|d| d.join("again.json")))
    }

    pub fn kept_in(file: Option<PathBuf>) -> Self {
        Self { file, waiting: None, going: false }
    }

    /// Read the place left, if the page was loaded again; the file goes, so that the next
    /// opening begins at the welcome as always.
    pub fn look(&mut self) {
        let Some(file) = &self.file else { return };
        self.waiting = fs::read_to_string(file).ok().and_then(|j| serde_json::from_str(&j).ok());
        if self.waiting.is_some() {
            let _ = fs::remove_file(file);
        }
    }
}

impl App {
    /// The page lost what it draws with: once it is shown again (not while hidden, when a frame
    /// shows nothing anyway), note where the writer is, keep the story, and load the page again.
    pub fn start_again_if_lost(&mut self) {
        if self.again.going || !platform::drawing_lost() || platform::page_hidden() {
            return;
        }
        self.again.going = true;
        self.save_before_leaving();
        self.keep_now();
        if !self.start.open
            && let Some(file) = &self.again.file
        {
            let place = Place {
                file: self.doc_file(),
                path: self.path.clone(),
                saved_hash: self.saved_hash,
                as_saved: self.as_saved.clone(),
                saved_at: self.saved_at,
                caret: self.caret,
                anchor: self.anchor,
                device: self.live.device.clone(),
            };
            if let Ok(json) = serde_json::to_string(&place) {
                let _ = fs::write(file, json);
            }
        }
        platform::reload();
    }

    /// Back where the writer was before the page was loaded again, without the welcome.
    pub fn come_back(&mut self, ctx: &egui::Context) {
        let Some(place) = self.again.waiting.take() else { return };
        (self.story_id, self.created) = (place.file.id.clone(), place.file.created);
        self.adopt(ctx, place.file);
        (self.path, self.saved_hash, self.as_saved, self.saved_at) = (place.path, place.saved_hash, place.as_saved, place.saved_at);
        self.keep.opened(&self.story_id);
        if !place.device.is_empty() {
            self.live.device = place.device;
        }
        self.start.open = false;
        self.set_caret(ctx, place.anchor, false);
        self.set_caret(ctx, place.caret, true);
    }
}
