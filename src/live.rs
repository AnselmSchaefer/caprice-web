//! One story live on several devices (R34, `LIVE_PLAN.md`). Signed in, a story on the shelf
//! that is open on two devices meets in its room on Caprice's server (`web/api/room.js`). One
//! device holds the pen and writes; every edit it applies (typed, undone, redone: all go through
//! `Doc::apply_raw`) goes to the others in a batch a second at most, numbered, with a hash of the
//! story after it. The others follow: they apply each batch as it was made, with no undo, and
//! cannot edit. Nothing is ever merged.
//!
//! What does not go through `Doc::apply` (page settings, notes, boxes, the cast, scenes) is not
//! sent one by one: the hash finds it, and a follower whose story no longer matches asks for the
//! whole story. The holder saves it, it goes online (R33), and the follower fetches that
//! version. So any way a story can change heals itself instead of drifting apart.
//!
//! Transfer here moves the pen: the holder sends its last batch and gives it. A device taking
//! the pen of a story it followed first learns the version online, so that its saving goes up
//! over that version, not beside it as a story changed in two places (R33).

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui::{self, Align2, Id, RichText, vec2};
use serde::Deserialize;
use serde_json::json;

use crate::App;
use crate::cloud::Record;
use crate::edit::Edit;
use crate::platform::{Answer, Heard, LiveSocket, Request, Waker};
use crate::theme::pal;

/// Seconds between the holder's batches: one a second at most, as each is a request to the room,
/// out of a daily allowance for the whole site (R34).
pub const BATCH_EVERY: f64 = 1.0;
/// Seconds before trying the room again after it closed.
const AGAIN_AFTER: f64 = 5.0;
/// Seconds a follower waits for the whole story it asked for before asking again.
const ASK_AGAIN_AFTER: f64 = 30.0;
/// A batch no bigger than this goes as it is; a bigger one (a long paste) goes as its hash only,
/// and the others fetch the whole story. The room takes no message over 256 KB.
const BATCH_BYTES: usize = 200_000;

/// The connection to the open story's room, and what this device knows of it.
pub struct Live {
    /// This device: an id of its own, and its name as the others see it ("Firefox on Linux").
    pub device: String,
    pub name: String,
    socket: Option<Box<dyn LiveSocket>>,
    /// The room it is in, or wants to be in: the open story's key online.
    key: Option<String>,
    /// Who holds the pen, by device id and name, as the room last said.
    pub holder: Option<(String, String)>,
    /// The number of the last batch sent (holding the pen) or applied (following).
    seq: u64,
    /// When the last batch went, and the story's hash as it went.
    sent_at: f64,
    hash: u64,
    /// Following, the whole story being fetched, at which batch; batches wait meanwhile.
    fetching: Option<(u64, Receiver<Answer>)>,
    waiting: Vec<Batch>,
    /// Following, when the whole story was asked for, while no version has come yet.
    asked_whole: Option<f64>,
    /// The time of the frame, as the room's messages are handled.
    now: f64,
    /// Holding the pen, asked to save: the version online before, to tell the new one once the
    /// story has gone up.
    saving: Option<Option<String>>,
    /// Having taken the pen of a story it followed: the version online being learnt.
    claiming: Option<Receiver<Answer>>,
    /// Where the pen is writing, as its last batch said.
    pub writing_at: Option<usize>,
    /// The room closed: when to try again.
    again_at: f64,
}

/// A batch of edits, as the room passes it on.
#[derive(Deserialize)]
struct Batch {
    seq: u64,
    /// None when it was too big to send: the hash says the story changed, and that is enough.
    edits: Option<Vec<Edit>>,
    hash: String,
    #[serde(default)]
    at: usize,
}

impl Live {
    pub fn new() -> Self {
        Live {
            device: crate::versions::new_id(),
            name: crate::platform::device_name(),
            socket: None,
            key: None,
            holder: None,
            seq: 0,
            sent_at: f64::NEG_INFINITY,
            hash: 0,
            fetching: None,
            waiting: Vec::new(),
            asked_whole: None,
            now: 0.0,
            saving: None,
            claiming: None,
            writing_at: None,
            again_at: f64::NEG_INFINITY,
        }
    }

    fn say(&self, message: serde_json::Value) {
        if let Some(s) = &self.socket {
            s.send(&message.to_string());
        }
    }

    fn holding(&self) -> bool {
        self.socket.is_some() && self.holder.as_ref().is_some_and(|(d, _)| *d == self.device)
    }

    /// In a room where another device holds the pen.
    pub fn following(&self) -> bool {
        self.socket.is_some() && self.holder.as_ref().is_some_and(|(d, _)| *d != self.device)
    }

    /// Out of the room: as if it had never been in it.
    fn leave(&mut self) {
        *self = Live { device: std::mem::take(&mut self.device), name: std::mem::take(&mut self.name), ..Live::new() };
    }
}

impl App {
    /// May the story be changed here? Not while another device holds the pen (R34).
    pub fn can_edit(&self) -> bool {
        !self.live.following()
    }

    /// The room the open story belongs in: signed in, with Caprice's server, a story on the shelf.
    fn live_key(&self) -> Option<String> {
        self.cloud.server.as_ref()?;
        self.cloud.who.as_ref()?;
        let path = self.path.as_ref()?;
        Some(self.cloud.key_for(path, &self.story_id))
    }

    /// Each frame: into the open story's room or out of it, what the room said, a batch out.
    pub fn live_frame(&mut self, ctx: &egui::Context) {
        let wanted = self.live_key();
        if wanted != self.live.key {
            self.leave_room();
            self.live.key = wanted;
        }
        let Some(key) = self.live.key.clone() else { return };
        let now = ctx.input(|i| i.time);
        self.live.now = now;
        if self.live.asked_whole.is_some_and(|t| now - t > ASK_AGAIN_AFTER) {
            self.live.asked_whole = None;
            self.ask_whole();
        }
        if self.live.socket.is_none() && now >= self.live.again_at {
            let Some(server) = self.cloud.server.clone() else { return };
            let path = format!("/api/live/{key}?device={}&name={}", self.live.device, crate::cloud::encode(&self.live.name));
            self.live.socket = Some(server.live(&path, Waker::new(ctx)));
        }
        let heard = self.live.socket.as_ref().map(|s| s.take()).unwrap_or_default();
        for h in heard {
            match h {
                Heard::Open => {}
                Heard::Message(text) => self.heard(ctx, &text),
                Heard::Closed(code) => {
                    self.leave_room();
                    self.live.key = Some(key.clone());
                    // The day's messages ran out for this story: not again today, or soon after.
                    self.live.again_at = now + if code == 4002 { 3600.0 } else { AGAIN_AFTER };
                }
            }
        }
        self.take_whole(ctx);
        self.take_claim();
        if self.live.holding() {
            self.send_batch(now);
            self.tell_saved();
        }
        self.cloud.skip = if self.live.following() { self.path.clone() } else { None };
        if self.live.socket.is_some() {
            // The socket wakes the app when it hears something; a batch waits for its second.
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(BATCH_EVERY));
        }
    }

    /// Out of the room, and editing as before: the story is this device's again.
    fn leave_room(&mut self) {
        self.live.leave();
        self.doc.outbox = None;
        self.cloud.skip = None;
    }

    fn heard(&mut self, ctx: &egui::Context, text: &str) {
        let Ok(m) = serde_json::from_str::<serde_json::Value>(text) else { return };
        let holder = |m: &serde_json::Value| Some((m["holder"].as_str()?.to_owned(), m["name"].as_str().unwrap_or_default().to_owned()));
        let seq = m["seq"].as_u64().unwrap_or(0);
        match m["t"].as_str().unwrap_or_default() {
            "hello" => {
                self.live.holder = holder(&m);
                self.live.seq = seq;
                if self.live.holding() {
                    self.take_pen(false);
                } else {
                    // Coming in to follow, this story is only the shelf's: the holder's comes first.
                    self.doc.outbox = None;
                    self.ask_whole();
                }
            }
            "pen" => {
                let was_holding = self.live.holding();
                self.live.holder = holder(&m);
                self.live.seq = seq;
                if self.live.holding() && !was_holding {
                    self.take_pen(true);
                } else if !self.live.holding() {
                    self.doc.outbox = None;
                }
            }
            "edits" => {
                if let Ok(batch) = serde_json::from_value::<Batch>(m) {
                    self.follow(ctx, batch);
                }
            }
            // Transfer here, from another device: what is typed goes first, then the pen.
            "give" if self.live.holding() => {
                self.send_batch(f64::INFINITY);
                self.live.say(json!({ "t": "given" }));
            }
            // A follower lost its way: the story goes up now, and it is told which version.
            "save" if self.live.holding() => {
                self.send_batch(f64::INFINITY);
                let key = self.live.key.clone().unwrap_or_default();
                let before = self.cloud.version_of(&key).map(str::to_owned);
                self.save_quietly();
                // Online as it is already: that version is the one to fetch.
                if before.is_some() && self.path.as_ref().is_some_and(|p| self.cloud.is_up(&key, p)) {
                    self.live.say(json!({ "t": "saved", "version": before }));
                } else {
                    self.live.saving = Some(before);
                    self.cloud.up_now();
                }
            }
            "version" if self.live.following() => {
                self.live.asked_whole = None;
                if let (Some(server), Some(key)) = (self.cloud.server.clone(), self.live.key.clone()) {
                    let (tx, rx) = channel();
                    server.send(Request { method: "GET", path: format!("/api/stories/{key}"), headers: Vec::new(), body: None }, tx, Waker::new(ctx));
                    self.live.fetching = Some((seq, rx));
                }
            }
            "refused" if self.live.holding() => {
                // A batch the room did not take: number on from the room's, and the followers'
                // hash check finds what they missed.
                self.live.seq = seq;
            }
            _ => {}
        }
    }

    /// This device holds the pen now. Having followed, its undo is not the story's any more, and
    /// the version online is learnt before anything of it is saved.
    fn take_pen(&mut self, followed: bool) {
        self.doc.outbox = Some(Vec::new());
        self.live.hash = self.live_hash();
        self.live.fetching = None;
        self.live.waiting.clear();
        self.live.asked_whole = None;
        if followed {
            self.doc.history.clear();
            if let (Some(server), Some(key)) = (self.cloud.server.clone(), self.live.key.clone()) {
                let (tx, rx) = channel();
                let wake = self.cloud.waker();
                if let Some(wake) = wake {
                    server.send(Request { method: "GET", path: format!("/api/stories/{key}"), headers: Vec::new(), body: None }, tx, wake);
                    self.live.claiming = Some(rx);
                }
            }
        }
    }

    /// The version online is known: the shelf's story agrees with it, and this device's story,
    /// which is newer, is saved over it and goes up.
    fn take_claim(&mut self) {
        let Some(rx) = &self.live.claiming else { return };
        let answer = match rx.try_recv() {
            Ok(a) => a,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Answer::default(),
        };
        self.live.claiming = None;
        let (Some(key), Some(path)) = (self.live.key.clone(), self.path.clone()) else { return };
        if answer.status == 200
            && let Some(version) = answer.header("ETag")
        {
            let version = version.trim_matches('"').to_owned();
            self.cloud.agree(key, Record { path, version, hash: crate::cloud::fingerprint(&answer.body) });
        }
        self.save_quietly();
        self.cloud.saved();
    }

    /// Holding the pen: the edits of the last second, and the story's hash after them, if anything
    /// changed. `now` past every second sends at once (before giving the pen, before saving).
    fn send_batch(&mut self, now: f64) {
        if now - self.live.sent_at < BATCH_EVERY {
            return;
        }
        let edits = self.doc.outbox.as_mut().map(std::mem::take).unwrap_or_default();
        let hash = self.live_hash();
        if edits.is_empty() && hash == self.live.hash {
            return;
        }
        self.live.seq += 1;
        let mut batch = json!({ "t": "edits", "seq": self.live.seq, "edits": edits, "hash": format!("{hash:016x}"), "at": self.caret });
        if batch.to_string().len() > BATCH_BYTES {
            batch["edits"] = serde_json::Value::Null;
        }
        self.live.say(batch);
        if now.is_finite() {
            self.live.sent_at = now;
        }
        self.live.hash = hash;
    }

    /// Asked to save, the story has gone up: the followers are told which version to fetch.
    fn tell_saved(&mut self) {
        let Some(before) = &self.live.saving else { return };
        let key = self.live.key.clone().unwrap_or_default();
        let now = self.cloud.version_of(&key).map(str::to_owned);
        if now.is_some() && now != *before {
            self.live.saving = None;
            self.live.say(json!({ "t": "saved", "version": now }));
        }
    }

    fn ask_whole(&mut self) {
        if self.live.asked_whole.is_none() {
            self.live.asked_whole = Some(self.live.now);
            self.live.say(json!({ "t": "whole" }));
        }
    }

    /// Following: a batch from the holder, applied as it was made there, if it is the next one
    /// and fits; then the story must hash as the holder's did. Otherwise the whole story is asked.
    fn follow(&mut self, ctx: &egui::Context, batch: Batch) {
        if !self.live.following() {
            return;
        }
        if self.live.fetching.is_some() {
            return self.live.waiting.push(batch);
        }
        if batch.seq <= self.live.seq {
            return;
        }
        if batch.seq != self.live.seq + 1 {
            return self.ask_whole();
        }
        self.live.seq = batch.seq;
        self.live.writing_at = Some(batch.at);
        for edit in batch.edits.unwrap_or_default() {
            if !self.apply_followed(ctx, &edit) {
                return self.ask_whole();
            }
        }
        self.live.hash = self.live_hash();
        if format!("{:016x}", self.live.hash) != batch.hash {
            self.ask_whole();
        }
        // The story here only follows: there is nothing of its own to save.
        self.saved_hash = self.content_hash();
    }

    /// One edit made on the device holding the pen, with the pages worked out from where it is
    /// (R3) and the caret and selection here moved along with the text, as notes are (R7).
    fn apply_followed(&mut self, ctx: &egui::Context, edit: &Edit) -> bool {
        let (at, old) = match edit {
            Edit::Replace { at, old, .. } => (*at, old.chars()),
            Edit::Restyle { at, new, .. } => (*at, new.len()),
        };
        let (dchars, dbytes) = edit.delta();
        if !self.doc.apply_live(edit) {
            return false;
        }
        self.doc.paginate_after(ctx, &self.typing, at, at + old, dchars, dbytes);
        let moved = |c: usize| if c >= at + old { c.saturating_add_signed(dchars) } else { c.min(at) };
        let (caret, anchor) = (moved(self.caret), moved(self.anchor));
        let max = self.max_caret();
        (self.caret, self.anchor) = (caret.min(max), anchor.min(max));
        true
    }

    /// Following: the whole story fetched comes in in place of this one, the caret and the page
    /// shown kept, and the batches that came meanwhile follow it.
    fn take_whole(&mut self, ctx: &egui::Context) {
        let Some((seq, rx)) = &self.live.fetching else { return };
        let answer = match rx.try_recv() {
            Ok(a) => a,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Answer::default(),
        };
        let seq = *seq;
        self.live.fetching = None;
        let file = (answer.status == 200).then(|| serde_json::from_slice::<crate::fileio::DocFile>(&answer.body).ok()).flatten();
        let Some(file) = file else {
            self.live.waiting.clear();
            return self.ask_whole();
        };
        let (caret, page) = (self.caret, self.target);
        self.adopt(ctx, file);
        self.saved_hash = self.content_hash();
        self.live.hash = self.live_hash();
        let caret = caret.min(self.max_caret());
        self.set_caret(ctx, caret, false);
        self.go_to_page(ctx, page.min(self.last()));
        self.live.seq = seq;
        let mut waiting = std::mem::take(&mut self.live.waiting);
        waiting.sort_by_key(|b| b.seq);
        for batch in waiting {
            self.follow(ctx, batch);
        }
    }

    /// Following, after a frame with a click or a key in it: if anything changed the story here
    /// all the same (a way of editing not yet kept from it, R34), the holder's story is fetched to
    /// replace it. Only then, as working out the hash takes a moment on a long story.
    pub fn live_check(&mut self, ctx: &egui::Context) {
        if !self.live.following() || self.live.fetching.is_some() {
            return;
        }
        let touched = ctx.input(|i| i.pointer.any_released() || i.events.iter().any(|e| matches!(e, egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Paste(_))));
        if touched && self.live_hash() != self.live.hash {
            self.ask_whole();
        }
    }

    /// Following: the bar above the pages that says another device is writing, with the way to
    /// where it writes, and Transfer here.
    pub fn live_bar(&mut self, ctx: &egui::Context) {
        if !self.live.following() {
            return;
        }
        let name = self.live.holder.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
        let (mut go, mut take) = (false, false);
        egui::Area::new(Id::new("live_bar")).anchor(Align2::CENTER_TOP, vec2(0.0, 52.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(pal().dock).show(ui, |ui| {
                ui.label(RichText::new(format!("Open for writing on another device ({name}). Editing is off here.")).color(pal().text));
                ui.horizontal(|ui| {
                    if self.live.writing_at.is_some() {
                        go = ui.link("Go to where it is being written").clicked();
                    }
                    ui.add_space(24.0);
                    take = ui.button("Transfer here").on_hover_text("Write here: the other device follows instead").clicked();
                });
            });
        });
        if go && let Some(at) = self.live.writing_at {
            let at = at.min(self.max_caret());
            self.set_caret(ctx, at, false);
            self.go_to_page(ctx, self.doc.page_of(at));
        }
        if take {
            self.live.say(json!({ "t": "take" }));
        }
    }
}
