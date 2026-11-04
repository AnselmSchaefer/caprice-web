//! Stories online (R33). The shelf (`shelf.rs`) is where a story is kept. Signed in, every story
//! on it also has a copy on Caprice's own server (`web/api`), so the same stories are there in any
//! browser. The copy follows the shelf: a story saved goes up a little later, and one newer online
//! comes down onto the shelf. Nothing is overwritten unseen: a story changed in two places is kept
//! twice, and nothing is asked of the server at all unless the writer has signed in.
//!
//! What to do with each story is `decide`, from three things: the file on the shelf, what was
//! known to be online when the two last agreed (kept in `cloud.json`), and what is online now.
//! One request goes at a time; a limit of the server's, or no network, pauses until it lifts.
//!
//! A story online is named by its id and its name on the shelf together (`key`): "Save as" keeps
//! the id, so two stories on a shelf can share one, and each must have its own copy.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use eframe::egui::{self, Id, pos2};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::platform::{Answer, Request, Waker, fs};

/// Seconds after a save before it goes up: the writer often saves again soon.
pub const UP_AFTER: f64 = 20.0;
/// Seconds between looks for what changed elsewhere, while the page is shown.
pub const LOOK_EVERY: f64 = 600.0;
/// Seconds to wait before trying again with no network, when the server says nothing sooner.
const AGAIN_AFTER: f64 = 60.0;

/// Where requests go: the page's own server in the browser, a stand-in in tests.
pub trait Server {
    fn send(&self, request: Request, tx: Sender<Answer>, wake: Waker);
    /// Leave the page for a path on the server: signing in, which comes back to the page.
    fn go_to(&self, path: &str);
}

#[cfg(target_arch = "wasm32")]
struct Page;

#[cfg(target_arch = "wasm32")]
impl Server for Page {
    fn send(&self, request: Request, tx: Sender<Answer>, wake: Waker) {
        crate::platform::send(request, tx, wake);
    }

    fn go_to(&self, path: &str) {
        crate::platform::go_to(path);
    }
}

/// What was online, and on the shelf, when the two last agreed about a story.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Record {
    pub path: PathBuf,
    pub version: String,
    pub hash: u64,
}

/// `cloud.json`: whose stories these are, whether a sign-in was begun, and each story's record.
#[derive(Serialize, Deserialize, Default)]
struct Kept {
    #[serde(default)]
    user: String,
    #[serde(default)]
    signing_in: bool,
    #[serde(default)]
    stories: BTreeMap<String, Record>,
}

/// A story the server has.
#[derive(Deserialize)]
struct Online {
    id: String,
    version: String,
}

/// Who is signed in.
#[derive(Deserialize, Clone)]
pub struct Who {
    pub name: String,
    pub provider: String,
    #[serde(default)]
    user: String,
}

/// What is to be done with one story.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Do {
    Nothing,
    Upload,
    Download,
    /// It changed on both sides: the copy online comes down as a story of its own, then this one
    /// goes up in its place.
    KeepBoth,
    /// Gone from both sides: its record goes too.
    Forget,
}

/// The story on the shelf: what its file holds, and whether it is open with changes not yet saved.
pub struct Local {
    pub hash: u64,
    pub open_unsaved: bool,
}

/// What to do with a story, from the shelf's file, the record of when the two sides last agreed,
/// and the version online now. Whatever happens, no words are lost: a story only on one side goes
/// to the other, even if it was deleted there.
pub fn decide(local: Option<&Local>, known: Option<&Record>, online: Option<&str>) -> Do {
    match (local, known, online) {
        (Some(_), _, None) => Do::Upload,
        (None, _, None) => Do::Forget,
        (None, _, Some(_)) => Do::Download,
        // Never agreed: the two may hold the same words, or not.
        (Some(_), None, Some(_)) => Do::KeepBoth,
        (Some(l), Some(k), Some(version)) => {
            let saved_here = l.hash != k.hash;
            let there = version != k.version;
            match (saved_here || l.open_unsaved, there) {
                (true, true) => Do::KeepBoth,
                (false, true) => Do::Download,
                _ if saved_here => Do::Upload,
                _ => Do::Nothing,
            }
        }
    }
}

/// A fingerprint that stays the same from one run to the next, and in every browser (FNV-1a).
pub fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// The name of a story online: its id and its name on the shelf together.
pub fn key(id: &str, name: &str) -> String {
    format!("{:016x}", fingerprint(format!("{id}\n{name}").as_bytes()))
}

/// A story's id, from its file (empty in older files).
fn id_of(bytes: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Id {
        #[serde(default)]
        id: String,
    }
    serde_json::from_slice::<Id>(bytes).map(|i| i.id).unwrap_or_default()
}

/// A story's name as a header can carry it (the server reads it with `decodeURIComponent`).
fn encode(name: &str) -> String {
    let mut out = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%').then(|| text.get(i + 1..i + 3)).flatten().and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unquote(tag: &str) -> String {
    tag.trim().trim_start_matches("W/").trim_matches('"').to_owned()
}

/// Work to do, one request each.
#[derive(Clone, Debug)]
enum Task {
    Upload { key: String, path: PathBuf, name: String, replaces: Option<String> },
    /// Bring a story down: to where its record says it is, or onto the shelf under its own name.
    Download { key: String, to: Option<PathBuf> },
    /// The copy online may hold other words than the shelf's: fetch it and see.
    Compare { key: String, path: PathBuf },
}

/// The request waiting for its answer.
enum Asking {
    Me,
    Providers,
    List,
    Get(Task),
    Put(Task, u64),
    Logout,
    DeleteAll,
}

pub struct Cloud {
    /// Where requests go; None: nowhere (the desktop, and tests that do not say otherwise).
    pub server: Option<Rc<dyn Server>>,
    /// Where `cloud.json` is kept; None: nowhere.
    pub file: Option<PathBuf>,
    /// The time, in seconds, when a test says what it is.
    pub clock: Option<f64>,
    /// The Stories online window is showing.
    pub open: bool,
    pub who: Option<Who>,
    /// The ways of signing in the server has; None until asked.
    pub providers: Option<Vec<String>>,
    /// How it goes now: paused by a limit, no network. It goes when that is over.
    pub note: Option<String>,
    /// What happened that the writer should know (a story kept twice, one too big), until they
    /// close the window.
    pub told: Option<String>,
    kept: Kept,
    asking: Option<(Asking, Receiver<Answer>)>,
    tasks: VecDeque<Task>,
    /// When the next look at the server is due; None: none is.
    next_round: Option<f64>,
    /// Nothing is sent before this: a limit or no network.
    paused_until: f64,
    /// Stories too big to keep online, as they were when the server said so.
    too_big: BTreeMap<String, u64>,
    wake: Option<Waker>,
    /// The open story was changed online and brought down: it is shown again from its file.
    reload: bool,
    begun: bool,
    was_hidden: bool,
    asked_providers: bool,
    confirm_delete: bool,
}

impl Cloud {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        let (server, file) = (Some(Rc::new(Page) as Rc<dyn Server>), crate::platform::data_dir().map(|d| d.join("cloud.json")));
        #[cfg(not(target_arch = "wasm32"))]
        let (server, file) = (None, None);
        Self::with(server, file)
    }

    pub fn with(server: Option<Rc<dyn Server>>, file: Option<PathBuf>) -> Self {
        let kept = file.as_ref().and_then(|f| fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Cloud {
            server,
            file,
            clock: None,
            open: false,
            who: None,
            providers: None,
            note: None,
            told: None,
            kept,
            asking: None,
            tasks: VecDeque::new(),
            next_round: None,
            paused_until: f64::NEG_INFINITY,
            too_big: BTreeMap::new(),
            wake: None,
            reload: false,
            begun: false,
            was_hidden: false,
            asked_providers: false,
            confirm_delete: false,
        }
    }

    fn now(&self) -> f64 {
        self.clock.unwrap_or_else(|| crate::platform::since_epoch().as_secs_f64())
    }

    fn keep(&self) {
        if let (Some(file), Ok(json)) = (&self.file, serde_json::to_string(&self.kept)) {
            let _ = file.parent().map(fs::create_dir_all);
            let _ = fs::write(file, json);
        }
    }

    /// Is anything being sent or waiting to be?
    pub fn busy(&self) -> bool {
        self.asking.is_some() || !self.tasks.is_empty()
    }

    /// A story was saved on the shelf: it goes up a little later.
    pub fn saved(&mut self) {
        if self.who.is_some() {
            let soon = self.now() + UP_AFTER;
            self.next_round = Some(self.next_round.map_or(soon, |t| t.min(soon)));
        }
    }

    fn due_now(&mut self) {
        self.next_round = Some(self.now());
    }

    /// What the shelf says beside a story, while signed in.
    pub fn mark(&self, path: &Path) -> Option<&'static str> {
        self.who.as_ref()?;
        let hash = fingerprint(&fs::read(path).ok()?);
        let online = self.kept.stories.values().any(|r| r.path == path && r.hash == hash);
        Some(if online { "online" } else { "not online yet" })
    }

    fn send(&mut self, asking: Asking, request: Request) {
        let (Some(server), Some(wake)) = (self.server.clone(), self.wake.clone()) else { return };
        let (tx, rx) = channel();
        self.asking = Some((asking, rx));
        server.send(request, tx, wake);
    }

    fn get(path: impl Into<String>) -> Request {
        Request { method: "GET", path: path.into(), headers: Vec::new(), body: None }
    }

    fn ask_providers(&mut self) {
        if self.providers.is_none() && !self.asked_providers && self.asking.is_none() {
            self.asked_providers = true;
            self.send(Asking::Providers, Self::get("/api/providers"));
        }
    }

    /// Sign in with `provider`: the page goes to its sign-in and comes back, and then asks who is in.
    fn sign_in(&mut self, provider: &str) {
        self.kept.signing_in = true;
        self.keep();
        if let Some(server) = &self.server {
            server.go_to(&format!("/api/login/{provider}"));
        }
    }

    /// Stop keeping stories online here. They stay on the shelf; what was known online is forgotten,
    /// so whoever signs in next starts afresh.
    fn signed_out(&mut self) {
        self.who = None;
        self.kept = Kept::default();
        self.keep();
        self.tasks.clear();
        self.next_round = None;
    }

    /// A limit or a fault: nothing is sent until it lifts, then everything is looked at again.
    fn pause(&mut self, answer: &Answer, note: String) {
        let wait = answer.header("Retry-After").and_then(|s| s.parse::<f64>().ok()).unwrap_or(AGAIN_AFTER);
        self.paused_until = self.now() + wait;
        self.next_round = Some(self.paused_until);
        self.tasks.clear();
        self.note = Some(note);
    }

    /// What every request may be answered: not signed in any more, a limit, or no answer at all.
    /// True when the answer was one of those, and has been dealt with.
    fn refused(&mut self, answer: &Answer) -> bool {
        let said = String::from_utf8_lossy(&answer.body).trim().to_owned();
        match answer.status {
            401 => {
                self.who = None;
                self.tasks.clear();
                self.next_round = None;
                self.note = Some("Signed out: sign in again to keep your stories online.".into());
            }
            0 => self.pause(answer, "No connection: trying again soon. Your stories are safe in this browser.".into()),
            429 | 503 | 507 => self.pause(answer, format!("Paused: {said}")),
            500.. => self.pause(answer, "The server had a problem: trying again soon.".into()),
            _ => return false,
        }
        true
    }
}

impl App {
    /// Each frame: keep the means to wake the app, look after the stories online, and show again
    /// the open story if it was changed online and brought down.
    pub fn cloud_frame(&mut self, ctx: &egui::Context) {
        if self.cloud.server.is_none() {
            return;
        }
        if self.cloud.wake.is_none() {
            self.cloud.wake = Some(Waker::new(ctx));
        }
        self.tend_cloud();
        if std::mem::take(&mut self.cloud.reload)
            && let Some(path) = self.path.clone()
        {
            let caret = self.caret;
            self.open_path(ctx, path);
            let caret = caret.min(self.doc.total_chars().saturating_sub(1));
            self.set_caret(ctx, caret, false);
            self.go_to_page(ctx, self.doc.page_of(caret));
            self.status = "- changed in another browser".into();
        }
        if self.cloud.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        } else if let Some(due) = self.cloud.next_round {
            let wait = (due - self.cloud.now()).clamp(0.05, LOOK_EVERY);
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        }
    }

    /// Answers in, requests out: in the frame, and also while the page is hidden (`App::logic`).
    pub fn tend_cloud(&mut self) {
        let cloud = &mut self.cloud;
        if cloud.server.is_none() || cloud.wake.is_none() || self.shelf.dir.is_none() {
            return;
        }
        if !cloud.begun {
            cloud.begun = true;
            // Only someone who signed in here is asked about: otherwise nothing is sent at all.
            if cloud.kept.signing_in || !cloud.kept.user.is_empty() {
                cloud.send(Asking::Me, Cloud::get("/api/me"));
            }
        }
        // Going out of sight, what was saved goes up; coming back, what changed elsewhere comes down.
        if self.keep.hidden() != cloud.was_hidden {
            cloud.was_hidden = self.keep.hidden();
            if cloud.who.is_some() {
                cloud.due_now();
            }
        }
        if let Some((_, rx)) = &cloud.asking {
            let answer = match rx.try_recv() {
                Ok(answer) => Some(answer),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Answer::default()),
            };
            if let Some(answer) = answer
                && let Some((asking, _)) = self.cloud.asking.take()
            {
                self.answered(asking, answer);
            }
        }
        let cloud = &mut self.cloud;
        if cloud.asking.is_some() || cloud.now() < cloud.paused_until {
            return;
        }
        if let Some(task) = cloud.tasks.pop_front() {
            self.start_task(task);
        } else if cloud.who.is_some() && cloud.next_round.is_some_and(|t| cloud.now() >= t) {
            cloud.next_round = None;
            cloud.send(Asking::List, Cloud::get("/api/stories"));
        }
    }

    fn start_task(&mut self, task: Task) {
        match &task {
            Task::Upload { key, path, name, replaces } => {
                let Ok(bytes) = fs::read(path) else { return };
                let hash = fingerprint(&bytes);
                let mut headers = vec![("X-Caprice-Name", encode(name))];
                match replaces {
                    Some(version) => headers.push(("If-Match", format!("\"{version}\""))),
                    None => headers.push(("If-None-Match", "*".into())),
                }
                let request = Request { method: "PUT", path: format!("/api/stories/{key}"), headers, body: Some(bytes) };
                self.cloud.send(Asking::Put(task, hash), request);
            }
            Task::Download { key, .. } | Task::Compare { key, .. } => {
                let request = Cloud::get(format!("/api/stories/{key}"));
                self.cloud.send(Asking::Get(task), request);
            }
        }
    }

    fn answered(&mut self, asking: Asking, answer: Answer) {
        match asking {
            Asking::Me => match answer.status {
                200 => {
                    let Ok(who) = serde_json::from_slice::<Who>(&answer.body) else { return };
                    let cloud = &mut self.cloud;
                    // Someone else signed in here: what was known online was theirs.
                    if who.user != cloud.kept.user {
                        cloud.kept = Kept { user: who.user.clone(), ..Kept::default() };
                    }
                    cloud.kept.signing_in = false;
                    cloud.keep();
                    cloud.who = Some(who);
                    cloud.note = None;
                    cloud.due_now();
                }
                401 => {
                    let cloud = &mut self.cloud;
                    // A sign-in turned back at Google says nothing; a session run out says so.
                    if !cloud.kept.user.is_empty() {
                        cloud.note = Some("Signed out: sign in again to keep your stories online.".into());
                    }
                    cloud.kept.signing_in = false;
                    cloud.keep();
                }
                _ => {
                    let cloud = &mut self.cloud;
                    cloud.pause(&answer, "No connection: trying again soon. Your stories are safe in this browser.".into());
                    // Asked again once it lifts.
                    cloud.begun = false;
                }
            },
            Asking::Providers => {
                self.cloud.providers = Some(if answer.status == 200 { serde_json::from_slice(&answer.body).unwrap_or_default() } else { Vec::new() });
            }
            Asking::Logout => self.cloud.signed_out(),
            Asking::DeleteAll => {
                if answer.status == 204 {
                    self.cloud.told = Some("Your stories online are deleted. They are still in this browser.".into());
                    self.cloud.send(Asking::Logout, Request { method: "POST", path: "/api/logout".into(), headers: Vec::new(), body: None });
                } else if !self.cloud.refused(&answer) {
                    self.cloud.told = Some("The stories online could not be deleted.".into());
                }
            }
            Asking::List => {
                if self.cloud.refused(&answer) {
                    return;
                }
                match serde_json::from_slice::<Vec<Online>>(&answer.body) {
                    Ok(online) => self.plan(online),
                    Err(_) => self.cloud.pause(&answer, "The server had a problem: trying again soon.".into()),
                }
            }
            Asking::Get(task) => {
                if self.cloud.refused(&answer) {
                    return;
                }
                if answer.status != 200 {
                    // Gone since the list: look again.
                    self.cloud.due_now();
                    return;
                }
                self.fetched(task, answer);
            }
            Asking::Put(Task::Upload { key, path, name, .. }, hash) => {
                if self.cloud.refused(&answer) {
                    return;
                }
                #[derive(Deserialize)]
                struct Saved {
                    version: String,
                }
                match answer.status {
                    200 => {
                        let Ok(saved) = serde_json::from_slice::<Saved>(&answer.body) else { return };
                        self.cloud.kept.stories.insert(key, Record { path, version: saved.version, hash });
                        self.cloud.keep();
                        if !self.cloud.busy() {
                            self.cloud.note = None;
                        }
                    }
                    413 => {
                        self.cloud.too_big.insert(key, hash);
                        self.cloud.told = Some(format!("\u{201c}{name}\u{201d} is too big to keep online. It is safe in this browser."));
                    }
                    // Changed online since the list (412), or being saved from elsewhere (409): look again.
                    _ => self.cloud.next_round = Some(self.cloud.now() + 5.0),
                }
            }
            Asking::Put(..) => {}
        }
    }

    /// The shelf's stories, each with its name online: the one its record gives, else its own.
    fn shelf_stories(&self, dir: &Path) -> Vec<(String, PathBuf, Vec<u8>)> {
        let mut found = Vec::new();
        for path in fs::read_dir(dir).unwrap_or_default() {
            if path.extension().is_none_or(|e| e != "caprice") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let recorded = self.cloud.kept.stories.iter().find(|(_, r)| r.path == path).map(|(k, _)| k.clone());
            let key = recorded.unwrap_or_else(|| key(&id_of(&bytes), &story_name(&path)));
            found.push((key, path, bytes));
        }
        found
    }

    /// From the list of what is online and what is on the shelf, the work to do.
    fn plan(&mut self, online: Vec<Online>) {
        let Some(dir) = self.shelf.dir.clone() else { return };
        let here = self.shelf_stories(&dir);
        let open_unsaved = self.unsaved();
        let mut keys: Vec<String> = here.iter().map(|(k, ..)| k.clone()).collect();
        keys.extend(online.iter().map(|o| o.id.clone()));
        keys.extend(self.cloud.kept.stories.keys().cloned());
        keys.sort();
        keys.dedup();
        let cloud = &mut self.cloud;
        cloud.tasks.clear();
        for key in keys {
            let shelf = here.iter().find(|(k, ..)| *k == key);
            let local = shelf.map(|(_, path, bytes)| Local {
                hash: fingerprint(bytes),
                open_unsaved: open_unsaved && self.path.as_deref() == Some(path.as_path()),
            });
            let there = online.iter().find(|o| o.id == key);
            let known = cloud.kept.stories.get(&key);
            if let (Some(l), Some(big)) = (&local, cloud.too_big.get(&key))
                && l.hash == *big
            {
                continue;
            }
            match decide(local.as_ref(), known, there.map(|o| o.version.as_str())) {
                Do::Nothing => {}
                Do::Forget => {
                    cloud.kept.stories.remove(&key);
                }
                Do::Upload => {
                    let (_, path, _) = shelf.expect("an upload is of a story on the shelf");
                    let replaces = there.and(known).map(|k| k.version.clone());
                    cloud.tasks.push_back(Task::Upload { key, path: path.clone(), name: story_name(path), replaces });
                }
                Do::Download => {
                    let to = known.map(|k| k.path.clone());
                    cloud.tasks.push_back(Task::Download { key, to });
                }
                Do::KeepBoth => {
                    let (_, path, _) = shelf.expect("both sides have it");
                    cloud.tasks.push_back(Task::Compare { key, path: path.clone() });
                }
            }
        }
        cloud.keep();
        cloud.next_round = Some(cloud.now() + LOOK_EVERY);
        if cloud.tasks.is_empty() && cloud.now() >= cloud.paused_until {
            cloud.note = None;
        }
    }

    /// A story fetched from online, to bring down or to compare with the shelf's.
    fn fetched(&mut self, task: Task, answer: Answer) {
        let Some(dir) = self.shelf.dir.clone() else { return };
        let version = answer.header("ETag").map(unquote).unwrap_or_default();
        let name = answer.header("X-Caprice-Name").map(decode).unwrap_or_else(|| "Untitled".into());
        let bytes = answer.body;
        let hash = fingerprint(&bytes);
        match task {
            Task::Download { key, to } => {
                let new_here = to.is_none();
                let path = to.unwrap_or_else(|| crate::platform::free_name(&dir, &format!("{}.caprice", crate::shelf::file_name(&name))));
                let open = self.path.as_deref() == Some(path.as_path());
                if open && self.unsaved() {
                    // Written in here meanwhile: the words from online are kept beside it.
                    self.keep_both(&dir, &key, &path, &name, &bytes, &version);
                    return;
                }
                let _ = fs::create_dir_all(&dir);
                if fs::write(&path, &bytes).is_ok() {
                    self.cloud.kept.stories.insert(key, Record { path: path.clone(), version, hash });
                    self.cloud.keep();
                    self.cloud.reload |= open;
                    // New to this browser: the welcome page offers it among the stories last opened.
                    if new_here {
                        self.start.remember(&path, self.keep.now());
                        self.start.stories = None;
                    }
                }
            }
            Task::Compare { key, path } => {
                let same = fs::read(&path).is_ok_and(|b| fingerprint(&b) == hash) && !(self.path.as_deref() == Some(path.as_path()) && self.unsaved());
                if same {
                    self.cloud.kept.stories.insert(key, Record { path, version, hash });
                    self.cloud.keep();
                } else {
                    self.keep_both(&dir, &key, &path, &name, &bytes, &version);
                }
            }
            Task::Upload { .. } => {}
        }
    }

    /// The copy online is saved on the shelf as a story of its own, and the shelf's story then
    /// goes up in its place: both are kept, on both sides.
    fn keep_both(&mut self, dir: &Path, key: &str, path: &Path, name: &str, bytes: &[u8], version: &str) {
        let copy = crate::platform::free_name(dir, &format!("{} (from another browser).caprice", crate::shelf::file_name(name)));
        if fs::write(&copy, bytes).is_err() {
            return;
        }
        self.start.remember(&copy, self.keep.now());
        self.start.stories = None;
        // Agreeing on the version online, but not on the words: the shelf's go up next.
        let record = Record { path: path.to_owned(), version: version.to_owned(), hash: 0 };
        self.cloud.kept.stories.insert(key.to_owned(), record);
        self.cloud.keep();
        self.cloud.told = Some(format!("\u{201c}{}\u{201d} was changed here and in another browser: both are kept.", story_name(path)));
        self.cloud.due_now();
    }

    /// The Stories online window: signing in and out, and how the stories online are.
    pub fn cloud_window(&mut self, ctx: &egui::Context) {
        if !self.cloud.open || self.cloud.server.is_none() {
            return;
        }
        if self.cloud.who.is_none() {
            self.cloud.ask_providers();
        }
        let screen = ctx.content_rect();
        let mut open = true;
        let mut sign_in = None;
        let (mut sign_out, mut delete) = (false, false);
        let cloud = &mut self.cloud;
        egui::Window::new("Stories online")
            .id(Id::new("cloud"))
            .open(&mut open)
            .default_pos(pos2(screen.center().x - 170.0, screen.top() + 90.0))
            .default_width(crate::ui::window_width(ctx, 340.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                match &cloud.who {
                    Some(who) => {
                        let by = if who.provider == "github" { "GitHub" } else { "Google" };
                        ui.label(format!("Signed in with {by} as {}.", who.name));
                        ui.add_space(4.0);
                        let how = match &cloud.note {
                            Some(note) => note.clone(),
                            None if cloud.busy() || cloud.next_round.is_some_and(|t| t <= cloud.now() + UP_AFTER) => "Saving online\u{2026}".into(),
                            None => "Your stories are online, and in this browser.".into(),
                        };
                        ui.weak(how);
                        if let Some(told) = &cloud.told {
                            ui.add_space(4.0);
                            ui.label(told);
                        }
                        ui.add_space(8.0);
                        if cloud.confirm_delete {
                            ui.label("Delete every copy online? The stories stay in this browser, and you are signed out.");
                            ui.horizontal(|ui| {
                                if ui.button("Delete them online").clicked() {
                                    delete = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    cloud.confirm_delete = false;
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                if ui.button("Sign out").on_hover_text("Your stories stay in this browser").clicked() {
                                    sign_out = true;
                                }
                                if ui.button("Delete my stories online\u{2026}").clicked() {
                                    cloud.confirm_delete = true;
                                }
                            });
                        }
                    }
                    None => {
                        ui.label("When you sign in, your stories are also kept online, to find them in any browser. They stay in this browser too, and Caprice works just the same without signing in.");
                        ui.add_space(4.0);
                        ui.weak("Only your name is asked for, never your email.");
                        for said in [&cloud.note, &cloud.told].into_iter().flatten() {
                            ui.add_space(4.0);
                            ui.weak(said);
                        }
                        ui.add_space(8.0);
                        match &cloud.providers {
                            None => {
                                ui.weak("\u{2026}");
                            }
                            Some(none) if none.is_empty() => {
                                ui.weak("Signing in is not set up here yet.");
                            }
                            Some(providers) => {
                                for p in providers {
                                    let by = if p == "github" { "GitHub" } else { "Google" };
                                    if ui.button(format!("Sign in with {by}")).clicked() {
                                        sign_in = Some(p.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            });
        if !open {
            self.cloud.open = false;
            self.cloud.confirm_delete = false;
            self.cloud.told = None;
        }
        if let Some(provider) = sign_in {
            // The page goes: the story is saved, and a version kept, first.
            self.save_before_leaving();
            self.keep_now();
            self.cloud.sign_in(&provider);
        }
        let cloud = &mut self.cloud;
        if (sign_out || delete) && cloud.asking.is_none() {
            cloud.tasks.clear();
            cloud.confirm_delete = false;
            if delete {
                cloud.send(Asking::DeleteAll, Request { method: "DELETE", path: "/api/stories".into(), headers: Vec::new(), body: None });
            } else {
                cloud.note = None;
                cloud.send(Asking::Logout, Request { method: "POST", path: "/api/logout".into(), headers: Vec::new(), body: None });
            }
        }
    }
}

fn story_name(path: &Path) -> String {
    path.file_stem().map_or_else(|| "Untitled".into(), |s| s.to_string_lossy().into_owned())
}

/// A stand-in for Caprice's server, in memory, as `web/api` answers: for the app's tests.
#[cfg(test)]
pub mod stand_in {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::mpsc::Sender;

    use super::{Answer, Request, Server, Waker, encode};

    #[derive(Default)]
    pub struct Online {
        /// Who has a session: their name and id.
        pub signed_in: Option<(String, String)>,
        pub providers: Vec<String>,
        /// Each story online: its name, version and file.
        pub stories: BTreeMap<String, (String, String, Vec<u8>)>,
        /// Every request, as "GET /api/me".
        pub asked: Vec<String>,
        pub went_to: Vec<String>,
        /// An answer to give every story request instead: its status, Retry-After, and what it says.
        pub refuse: Option<(u16, u32, String)>,
        versions: u32,
    }

    #[derive(Default)]
    pub struct StandIn(pub RefCell<Online>);

    impl StandIn {
        /// Another browser saves a story online.
        pub fn put(&self, key: &str, name: &str, bytes: &[u8]) {
            let mut o = self.0.borrow_mut();
            o.versions += 1;
            let version = format!("v{}", o.versions);
            o.stories.insert(key.into(), (name.into(), version, bytes.into()));
        }

        fn answer(&self, r: Request) -> Answer {
            let mut o = self.0.borrow_mut();
            o.asked.push(format!("{} {}", r.method, r.path));
            let header = |name: &str| r.headers.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone());
            let json = |v: serde_json::Value| Answer { status: 200, headers: Vec::new(), body: v.to_string().into_bytes() };
            let status = |status: u16| Answer { status, ..Answer::default() };
            if r.path == "/api/providers" {
                return json(serde_json::json!(o.providers));
            }
            if r.path == "/api/logout" {
                o.signed_in = None;
                return status(204);
            }
            let Some((name, user)) = o.signed_in.clone() else { return status(401) };
            if r.path == "/api/me" {
                return json(serde_json::json!({ "name": name, "provider": "google", "user": user }));
            }
            if let Some((code, retry, said)) = &o.refuse {
                return Answer { status: *code, headers: vec![("Retry-After".into(), retry.to_string())], body: said.clone().into_bytes() };
            }
            match (r.method, r.path.strip_prefix("/api/stories").unwrap_or("")) {
                ("GET", "") => json(serde_json::json!(o.stories.iter()
                    .map(|(id, (name, version, _))| serde_json::json!({ "id": id, "name": name, "version": version }))
                    .collect::<Vec<_>>())),
                ("DELETE", "") => {
                    o.stories.clear();
                    status(204)
                }
                ("GET", key) => match o.stories.get(&key[1..]) {
                    Some((name, version, bytes)) => Answer {
                        status: 200,
                        headers: vec![("ETag".into(), format!("\"{version}\"")), ("X-Caprice-Name".into(), encode(name))],
                        body: bytes.clone(),
                    },
                    None => status(404),
                },
                ("PUT", key) => {
                    let key = &key[1..];
                    let now = o.stories.get(key).map(|(_, v, _)| v.clone());
                    let fits = match (header("If-None-Match"), header("If-Match")) {
                        (Some(_), _) => now.is_none(),
                        (None, Some(tag)) => now.as_deref() == Some(tag.trim_matches('"')),
                        (None, None) => return status(428),
                    };
                    if !fits {
                        return status(412);
                    }
                    o.versions += 1;
                    let version = format!("v{}", o.versions);
                    let name = super::decode(&header("X-Caprice-Name").unwrap_or_default());
                    o.stories.insert(key.into(), (name, version.clone(), r.body.unwrap_or_default()));
                    json(serde_json::json!({ "version": version }))
                }
                _ => status(404),
            }
        }
    }

    impl Server for StandIn {
        fn send(&self, request: Request, tx: Sender<Answer>, wake: Waker) {
            let _ = tx.send(self.answer(request));
            wake.wake();
        }

        fn go_to(&self, path: &str) {
            self.0.borrow_mut().went_to.push(path.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(hash: u64) -> Local {
        Local { hash, open_unsaved: false }
    }

    fn record(hash: u64, version: &str) -> Record {
        Record { path: "/s/a.caprice".into(), version: version.into(), hash }
    }

    #[test]
    fn a_story_only_on_one_side_goes_to_the_other() {
        assert_eq!(decide(Some(&local(1)), None, None), Do::Upload, "new here");
        assert_eq!(decide(Some(&local(1)), Some(&record(1, "v1")), None), Do::Upload, "gone online: put back, not lost");
        assert_eq!(decide(None, None, Some("v1")), Do::Download, "new online");
        assert_eq!(decide(None, Some(&record(1, "v1")), Some("v1")), Do::Download, "gone from the shelf: brought back");
        assert_eq!(decide(None, Some(&record(1, "v1")), None), Do::Forget, "gone from both");
    }

    #[test]
    fn a_story_changed_on_one_side_follows_it_and_on_both_is_kept_twice() {
        let known = record(1, "v1");
        assert_eq!(decide(Some(&local(1)), Some(&known), Some("v1")), Do::Nothing, "the same on both");
        assert_eq!(decide(Some(&local(2)), Some(&known), Some("v1")), Do::Upload, "saved here");
        assert_eq!(decide(Some(&local(1)), Some(&known), Some("v2")), Do::Download, "saved elsewhere");
        assert_eq!(decide(Some(&local(2)), Some(&known), Some("v2")), Do::KeepBoth, "saved in both");
        assert_eq!(decide(Some(&local(1)), None, Some("v1")), Do::KeepBoth, "never agreed: compared");
    }

    #[test]
    fn a_story_open_with_changes_not_yet_saved_is_never_written_over() {
        let open = Local { hash: 1, open_unsaved: true };
        assert_eq!(decide(Some(&open), Some(&record(1, "v1")), Some("v2")), Do::KeepBoth);
        assert_eq!(decide(Some(&open), Some(&record(1, "v1")), Some("v1")), Do::Nothing, "it goes up once saved");
    }

    #[test]
    fn a_name_goes_to_the_server_and_back_as_it_was() {
        for name in ["The Lion Boy, second draft", "Ärger & Glück", "50% done / maybe"] {
            assert_eq!(decode(&encode(name)), name);
            assert!(encode(name).bytes().all(|b| b.is_ascii_graphic()), "a header carries it");
        }
    }

    #[test]
    fn a_story_s_name_online_fits_the_server_and_differs_by_name() {
        let one = key("3f2a", "Hatch");
        assert!(one.len() <= 64 && one.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase()));
        assert_eq!(one, key("3f2a", "Hatch"), "the same in every browser");
        assert_ne!(one, key("3f2a", "Hatch, second draft"), "saved as another name: a story of its own");
    }
}
