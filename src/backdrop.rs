//! Scenes behind the pages. Claude paints a scene as SVG (through the Claude Code CLI), and it is
//! kept with the story, pinned to where it begins, like a note. It covers the story up to the next
//! scene, and the page shown shows the one the caret is in, faintly behind its text, so the
//! pictures change as the caret moves on through the story. A scene is one the writer describes, a
//! passage they select, or follows the writing: as sentences are written, Claude groups them into
//! scenes by what they say, and each scene is painted once the next begins, so no picture mixes
//! two places.
//! Painting again for the same scene adds a version to it rather than replacing it. The list of
//! scenes shows them all, to go to, show again, step through versions, hide or delete.
//!
//! In the browser, where Claude Code cannot run, Caprice's server has Claude paint the scene
//! instead (`/api/scene`, R33), for writers signed in, while the day's scenes last.
//!
//! Drawings are rendered on a background thread when a page near the one shown needs them, and
//! only those near are kept as textures. Parts of a drawing may move a little: clouds, wings,
//! smoke (see `motion.rs`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{self, Color32, ColorImage, Id, Key, Modifiers, Pos2, Rect, Shape, TextureHandle, TextureOptions};

use crate::App;
use crate::claude::{Outcome, run_claude};
use crate::motion::{Drawn, Layered, layered};
use crate::model::{Character, Doc, IMAGE_CHAR, PAGE_BREAK, Scene, is_terminator};
use crate::theme::pal;
use crate::platform::{Answer, Request, fs};

/// How strongly a scene shows through the paper.
pub const OPACITY: f32 = 0.2;
/// Seconds to wait after a paragraph is finished before drawing, in case the writer goes back to it.
const PAUSE: f64 = 1.5;
/// Seconds one scene takes to fade into the next.
const FADE: f64 = 2.5;
/// Width in pixels scenes are rendered at, behind the pages and in the list of scenes.
const RENDER_WIDTH: f32 = 1000.0;
const THUMB_WIDTH: f32 = 120.0;
/// How many pages either side of the one shown get their scene rendered ahead.
const NEAR: usize = 2;
/// How many full-size drawings are kept rendered at most, besides those near the page shown.
const KEEP: usize = 6;
/// Seconds between frames while the scene on the page shown moves.
const MOVE_FRAME: f64 = 0.05;

/// The model that paints the scenes.
pub const MODEL: &str = "claude-opus-5-5";

pub const SYSTEM: &str = "You are a skilled illustrator who paints with SVG code. You make detailed, \
    atmospheric illustrations for the pages of a story, in the manner of a classic book \
    illustration: believable light, depth and texture rather than cartoon shapes.";

/// A painted scene: its drawing, and the drawing rendered.
pub type Painted = (String, Layered);

/// Where a painting goes once it is done, or why there is none.
type Easel = Sender<Result<Painted, String>>;

/// One drawing of one scene: the scene's id and which of its versions.
pub type DrawingKey = (u64, usize);

/// A painting under way, for the scene with id `scene`.
struct Job {
    rx: Receiver<Result<Painted, String>>,
    cancel: Arc<AtomicBool>,
    scene: u64,
    /// The people of the cast it is painted with, for the scene once it is painted.
    people: Vec<String>,
    /// Painted by Caprice's server: its answer, still to come, and where the picture goes once
    /// it is rendered.
    online: Option<(Receiver<Answer>, Easel)>,
    /// The name Caprice's server knows the painting by, to say once it is in the story that it
    /// need wait there no longer.
    online_id: Option<String>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Following the writing, the scene being written: a scene pinned to its first sentence, not yet
/// painted (so it moves with edits), and how many of its sentences have been read.
struct Open {
    scene: u64,
    read: usize,
}

/// Sentences written since, which Claude is reading to say which of them begin a new scene. Each
/// has a scene pinned to its start meanwhile, not yet painted.
struct Reading {
    rx: Receiver<Result<Vec<usize>, String>>,
    cancel: Arc<AtomicBool>,
    pins: Vec<u64>,
    /// What Claude was asked, for the tests to look at.
    #[cfg_attr(not(test), allow(dead_code))]
    prompt: String,
}

impl Drop for Reading {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// A drawing rendered on a background thread, full size or small for the list.
struct Rendered {
    key: DrawingKey,
    thumb: bool,
    image: Result<Layered, String>,
}

/// Where new scenes come from.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The writer's description, drawn when they ask.
    #[default]
    Described,
    /// Each scene of the story as it is written, drawn once the next one begins.
    Writing,
}

/// What a picture is to show.
enum Subject {
    Description(String),
    /// A passage: one the writer selected, or the paragraph they finished.
    Passage(String),
}

impl Subject {
    fn text(&self) -> &str {
        match self {
            Subject::Description(t) | Subject::Passage(t) => t,
        }
    }
}

pub struct Backdrop {
    /// The dialog to describe the scene is open.
    pub panel: bool,
    /// The list of scenes is open.
    pub list: bool,
    pub mode: Mode,
    /// The pictures show behind the pages.
    pub visible: bool,
    /// Their parts that can move do.
    pub moving: bool,
    /// What the writer wants to see.
    pub description: String,
    /// Drawings rendered: full size behind the pages, small in the list.
    textures: HashMap<DrawingKey, Drawn>,
    thumbs: HashMap<DrawingKey, TextureHandle>,
    /// Drawings being rendered, or that could not be, so they are not tried again and again.
    rendering: HashSet<(DrawingKey, bool)>,
    rendered: (Sender<Rendered>, Receiver<Rendered>),
    /// The page shown and the drawing behind it, and the drawing it fades from and since when.
    /// Turning to another page shows its drawing at once; only a change on the same page fades.
    on_target: Option<(usize, Option<DrawingKey>)>,
    fade: Option<(Option<DrawingKey>, f64)>,
    job: Option<Job>,
    /// What Claude was last asked to paint.
    asked: String,
    /// How many scenes Caprice's server can still paint today, as it last said.
    pub left: Option<u32>,
    /// The story the pictures waiting on Caprice's server were last asked for, and the asking.
    recovered: Option<String>,
    recovering: Option<Receiver<Answer>>,
    /// Put the keyboard on the description when the dialog opens.
    focus_field: bool,
    /// Following the writing: the document version last seen and when it changed (only edits
    /// count, not moving the caret), and the version whose sentences were read last.
    seen: Option<u64>,
    edited_at: f64,
    read_at: Option<u64>,
    /// The scene being written, the sentences since being read, and the scenes they ended,
    /// waiting to be painted one after the other.
    open: Option<Open>,
    reading: Option<Reading>,
    queue: VecDeque<u64>,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            panel: false,
            list: false,
            mode: Mode::Described,
            visible: true,
            moving: true,
            description: String::new(),
            textures: HashMap::new(),
            thumbs: HashMap::new(),
            rendering: HashSet::new(),
            rendered: channel(),
            on_target: None,
            fade: None,
            job: None,
            asked: String::new(),
            left: None,
            recovered: None,
            recovering: None,
            focus_field: false,
            seen: None,
            edited_at: 0.0,
            read_at: None,
            open: None,
            reading: None,
            queue: VecDeque::new(),
        }
    }
}

/// The SVG in Claude's reply (it may wrap it in a code fence).
pub fn extract_svg(reply: &str) -> Option<&str> {
    let start = reply.find("<svg")?;
    let end = reply.rfind("</svg>")? + "</svg>".len();
    (end > start).then(|| &reply[start..end])
}

/// Render an SVG to an image `width` pixels wide.
pub fn rasterize(svg: &str, width: f32) -> Result<ColorImage, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("the picture could not be read: {e}"))?;
    let size = tree.size();
    let scale = width / size.width();
    let (w, h) = (width.round() as u32, (size.height() * scale).round().max(1.0) as u32);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("the picture has no size")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()))
}

/// What Claude is asked to paint for `subject` on a page `size` points large. The people of the
/// cast it names come first: how they look, and the model sheets to copy them from (see `cast.rs`).
fn scene_prompt(subject: &Subject, size: egui::Vec2, named: &[&Character]) -> String {
    let (w, h) = (size.x.round(), size.y.round());
    let cast = crate::cast::cast_prompt(named);
    let what = match subject {
        Subject::Description(d) => format!("{cast}<scene>\n{d}\n</scene>\n\nPaint one illustration of this scene"),
        Subject::Passage(p) => format!("{cast}<passage>\n{p}\n</passage>\n\nPaint one illustration of the scene this passage of a story describes"),
    };
    format!(
        "{what}, to be shown faintly behind the text of a page.\n\
        - One standalone SVG with viewBox=\"0 0 {w} {h}\", the scene filling the whole area, sky included.\n\
        - Realistic and detailed: correct proportions and perspective; several planes of depth with \
          atmospheric perspective (far things paler and bluer); one clear light direction with \
          highlights and cast shadows; varied, natural shapes (no identical copies); fine detail \
          where the eye goes, such as textured rock, foliage clusters, roof tiles, ripples.\n\
        - Painterly technique: layered linear and radial gradients, soft edges with feGaussianBlur, \
          natural textures with feTurbulence, fine ink or pencil detail on top.\n\
        - A harmonious, natural palette with enough contrast to read when shown faintly.\n\
        - No text, letters or numbers. No scripts, links, external references or embedded images.\n\
        - At most about 30 KB.\n\
        {}\
        Reply with only the SVG code.",
        crate::motion::PROMPT
    )
}

/// Ask Claude for a picture with `prompt`, and render it.
fn draw_scene(prompt: &str, cancel: &AtomicBool) -> Result<Option<Painted>, String> {
    let mut reply = String::new();
    match run_claude(MODEL, SYSTEM, prompt, "medium", cancel, &mut |t| reply.push_str(t))? {
        Outcome::Cancelled => Ok(None),
        Outcome::Refused => Err("Claude declined to draw this scene".into()),
        Outcome::Finished => {
            let svg = extract_svg(&reply).ok_or("Claude's reply held no picture")?;
            layered(svg, RENDER_WIDTH).map(|image| Some((svg.to_owned(), image)))
        }
    }
}

pub const READING_SYSTEM: &str = "You read stories closely, as an illustrator choosing what to paint.";

/// What Claude is asked about the `sentences` written after the scene so far: which of them
/// begin a new scene. Only what they say counts, not how they are laid out.
fn reading_prompt(scene: &str, sentences: &[String]) -> String {
    let numbered: Vec<String> = sentences.iter().enumerate().map(|(i, t)| format!("{}. {}", i + 1, t.replace('\n', " "))).collect();
    format!(
        "<scene_so_far>\n{scene}\n</scene_so_far>\n\n<new_sentences>\n{}\n</new_sentences>\n\n\
        A story is being written, and each of its scenes will be painted as one illustration. The \
        scene so far is above; the numbered sentences were written after it, in order. Which of them \
        begin a new scene: the story moves to another place, or to a moment that needs a picture of \
        its own?\n\
        Judge only by what the sentences say, not by how they are laid out: dialogue, thoughts and \
        description all belong to the scene they happen in, and a new line or paragraph alone is no \
        new scene. Sentences that go on in the same place stay with the scene before them, so that \
        no picture mixes two places.\n\
        Reply with only the numbers of the sentences that begin a new scene, separated by commas, \
        or 0 if none does.",
        numbered.join("\n")
    )
}

/// The sentences (from 0) that begin a new scene, from Claude's reply about `n` sentences.
fn parse_firsts(reply: &str, n: usize) -> Vec<usize> {
    let numbers = reply.split(|c: char| !c.is_ascii_digit()).filter_map(|w| w.parse().ok());
    let mut firsts: Vec<usize> = numbers.filter(|&k| (1..=n).contains(&k)).map(|k| k - 1).collect();
    firsts.sort_unstable();
    firsts.dedup();
    firsts
}

/// Ask Claude which of the new sentences begin a scene.
fn read_sentences(prompt: &str, n: usize, cancel: &AtomicBool) -> Result<Vec<usize>, String> {
    let mut reply = String::new();
    match run_claude(MODEL, READING_SYSTEM, prompt, "low", cancel, &mut |t| reply.push_str(t))? {
        Outcome::Finished => Ok(parse_firsts(&reply, n)),
        _ => Err("the sentences were not read".into()),
    }
}

/// The scenes' settings as kept in a document file. Older files also hold their one scene's
/// drawing here; it opens as a scene pinned to the start of the story.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SceneFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// The pictures stand still.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub still: bool,
}

impl Doc {
    /// The scene behind char `c`: the one pinned last at or before it that has a drawing and shows.
    pub fn scene_at(&self, c: usize) -> Option<&Scene> {
        self.scenes.iter().filter(|s| s.at <= c && !s.hidden && s.svg().is_some()).max_by_key(|s| (s.at, s.id))
    }

    /// The scene behind page `i` when the caret is not on it: the one of the text at its top, as
    /// the page is when turned to. The contents pages, holding no text, show the story's first.
    pub fn page_scene(&self, i: usize) -> Option<&Scene> {
        self.scene_at(self.spans[i].start)
    }

    pub fn page_scene_key(&self, i: usize) -> Option<DrawingKey> {
        self.page_scene(i).map(|s| (s.id, s.shown))
    }

    fn scene_mut(&mut self, id: u64) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|s| s.id == id)
    }
}

impl App {
    /// The scene behind the page shown: see `scene_of_page`.
    pub fn scene_here(&self) -> Option<&Scene> {
        self.scene_of_page(self.target)
    }

    /// The scene behind page `i`: on the caret's page, the one of the part of the story the caret
    /// is in, so moving the caret into the next part brings its picture, and turning back to the
    /// page finds it as it was left. On any other page, the one at its top.
    pub fn scene_of_page(&self, i: usize) -> Option<&Scene> {
        if self.doc.page_of(self.caret) == i {
            self.doc.scene_at(self.caret)
        } else {
            self.doc.page_scene(i)
        }
    }

    fn scene_key_of_page(&self, i: usize) -> Option<DrawingKey> {
        self.scene_of_page(i).map(|s| (s.id, s.shown))
    }
}

impl Backdrop {
    /// A painting is under way, or sentences being read, or scenes waiting to be painted.
    pub fn drawing(&self) -> bool {
        self.job.is_some() || self.reading.is_some() || !self.queue.is_empty()
    }

    /// What is kept in the document file besides the scenes, if anything.
    pub fn to_file(&self) -> Option<SceneFile> {
        let file = SceneFile { svg: None, description: self.description.clone(), hidden: !self.visible, still: !self.moving };
        (file != SceneFile::default()).then_some(file)
    }

    /// What Claude was last asked to paint.
    #[cfg(test)]
    pub fn asked(&self) -> &str {
        &self.asked
    }

    /// Following the writing: what Claude is being asked about the sentences written.
    #[cfg(test)]
    pub fn reading(&self) -> Option<&str> {
        self.reading.as_ref().map(|r| r.prompt.as_str())
    }

    /// Render the drawing `svg` of `key` on a background thread, unless it is or has been already.
    fn render(&mut self, ctx: &egui::Context, key: DrawingKey, svg: &str, thumb: bool) {
        let have = if thumb { self.thumbs.contains_key(&key) } else { self.textures.contains_key(&key) };
        if have || !self.rendering.insert((key, thumb)) {
            return;
        }
        let (tx, wake, svg) = (self.rendered.0.clone(), crate::platform::Waker::new(ctx), svg.to_owned());
        crate::platform::spawn(move || {
            let image = if thumb { rasterize(&svg, THUMB_WIDTH).map(Layered::still) } else { layered(&svg, RENDER_WIDTH) };
            let _ = tx.send(Rendered { key, thumb, image });
            wake.wake();
        });
    }
}

impl App {
    /// Start painting the described scene, for the paragraph the selection starts in.
    pub fn draw_backdrop(&mut self, ctx: &egui::Context) {
        let description = self.backdrop.description.trim().to_owned();
        if !description.is_empty() {
            let at = self.doc.para_start(self.selection().0).0;
            self.start_scene(ctx, Subject::Description(description), at);
        }
    }

    /// Paint the scene the passage `a..b` describes, for the paragraph it starts in.
    pub fn paint_passage(&mut self, ctx: &egui::Context, a: usize, b: usize) {
        let passage = self.selected_passage(a, b);
        if !passage.trim().is_empty() {
            self.start_scene(ctx, Subject::Passage(passage), self.doc.para_start(a).0);
        }
    }

    pub fn selected_passage(&self, a: usize, b: usize) -> String {
        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        self.doc.flow.text[ba..bb].chars().filter(|&c| c != IMAGE_CHAR).map(|c| if c == PAGE_BREAK { '\n' } else { c }).collect()
    }

    /// Paint `subject` for the scene pinned at char `at` (replacing a painting still in progress).
    /// A scene already pinned there gets another version; otherwise a new scene is pinned there,
    /// which shows once its drawing is done.
    fn start_scene(&mut self, ctx: &egui::Context, subject: Subject, at: usize) {
        let people = self.people_for(&subject, at);
        self.paint_with(ctx, subject, at, people);
    }

    /// Paint `subject` for the scene pinned at char `at` with these people of the cast.
    fn paint_with(&mut self, ctx: &egui::Context, subject: Subject, at: usize, people: Vec<String>) {
        let (tx, cancel) = self.pin_painting(subject.text(), at);
        let prompt = scene_prompt(&subject, self.doc.setup.size(), &self.doc.cast_by_names(&people));
        if let Some(job) = &mut self.backdrop.job {
            job.people = people;
        }
        self.backdrop.asked = prompt.clone();
        let wake = crate::platform::Waker::new(ctx);
        if !self.claude
            && let Some(server) = self.cloud.server.clone()
        {
            let (answer_tx, answer_rx) = channel();
            // Named, and said which story and char it is for: should the page go before it comes
            // (a reload, another story opened), the server keeps it until the story opens again.
            let painting = crate::versions::new_id();
            if let Some(job) = &mut self.backdrop.job {
                job.online = Some((answer_rx, tx));
                job.online_id = Some(painting.clone());
            }
            let headers = vec![
                ("Content-Type", "text/plain; charset=utf-8".to_owned()),
                ("X-Caprice-Painting", painting),
                ("X-Caprice-Story", self.story_id.clone()),
                ("X-Caprice-At", at.to_string()),
            ];
            server.send(Request { method: "POST", path: "/api/scene".into(), headers, body: Some(prompt.into_bytes()) }, answer_tx, wake);
            return;
        }
        crate::platform::spawn(move || {
            if let Some(result) = draw_scene(&prompt, &cancel).transpose() {
                let _ = tx.send(result);
            }
            wake.wake();
        });
    }

    /// The people of the cast a painting of `subject` for the scene at char `at` is about: those
    /// the writer chose for it, else those it names. A passage that names no one carries on those
    /// named just before it (`carried_people`); a description is all the writer wants.
    fn people_for(&self, subject: &Subject, at: usize) -> Vec<String> {
        if let Some(s) = self.doc.scenes.iter().find(|s| s.at == at && s.chosen) {
            return s.people.clone();
        }
        let named: Vec<String> = self.doc.named_in(subject.text()).iter().map(|c| c.name.trim().to_owned()).collect();
        match subject {
            Subject::Passage(_) if named.is_empty() => self.carried_people(at),
            _ => named,
        }
    }

    /// The scene pinned at char `at` a painting goes to, made if need be, and the painting as the
    /// job under way: where to send the result, and the flag that stops it.
    fn pin_painting(&mut self, subject: &str, at: usize) -> (Sender<Result<Painted, String>>, Arc<AtomicBool>) {
        self.backdrop.job = None;
        self.drop_unpainted_scenes();
        let id = match self.doc.scenes.iter_mut().find(|s| s.at == at) {
            Some(s) => {
                s.subject = subject.to_owned();
                s.id
            }
            None => {
                let id = self.doc.next_scene_id;
                self.doc.next_scene_id += 1;
                self.doc.scenes.push(Scene { id, at, versions: Vec::new(), shown: 0, subject: subject.to_owned(), hidden: false, people: Vec::new(), chosen: false });
                id
            }
        };
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.backdrop.job = Some(Job { rx, cancel: cancel.clone(), scene: id, people: Vec::new(), online: None, online_id: None });
        (tx, cancel)
    }

    /// A painting of `svg` for the paragraph of char `c`, as if Claude had just finished it.
    #[cfg(test)]
    pub fn fake_painting(&mut self, c: usize, svg: &str) {
        let (tx, _) = self.pin_painting("a test", self.doc.para_start(c).0);
        tx.send(Ok((svg.to_owned(), layered(svg, RENDER_WIDTH).unwrap()))).unwrap();
    }

    /// The passage `a..b` painted as `paint_passage` paints it, with the people it finds, as if
    /// Claude had just finished it.
    #[cfg(test)]
    pub fn fake_painting_of(&mut self, a: usize, b: usize, svg: &str) {
        let (subject, at) = (Subject::Passage(self.selected_passage(a, b)), self.doc.para_start(a).0);
        let people = self.people_for(&subject, at);
        let (tx, _) = self.pin_painting(subject.text(), at);
        if let Some(job) = &mut self.backdrop.job {
            job.people = people;
        }
        tx.send(Ok((svg.to_owned(), layered(svg, RENDER_WIDTH).unwrap()))).unwrap();
    }

    /// Start painting for char `c` without finishing, as if Claude were still at it.
    #[cfg(test)]
    pub fn fake_painting_under_way(&mut self, c: usize) -> Sender<Result<Painted, String>> {
        self.pin_painting("a test", self.doc.para_start(c).0).0
    }

    /// Claude's reply about the sentences being read, as if it had just come: which of them
    /// (numbered from 1) begin a scene, or "0".
    #[cfg(test)]
    pub fn fake_reading(&mut self, reply: &str) {
        let reading = self.backdrop.reading.take().expect("sentences being read");
        let firsts = parse_firsts(reply, reading.pins.len());
        self.reading_done(reading, Ok(firsts));
    }

    /// The scenes waiting to be painted: where each is pinned, and its passage.
    #[cfg(test)]
    pub fn queued(&self) -> Vec<(usize, String)> {
        let scene = |id: &u64| self.doc.scenes.iter().find(|s| s.id == *id).map(|s| (s.at, s.subject.clone()));
        self.backdrop.queue.iter().filter_map(scene).collect()
    }

    /// Where the scene being written begins.
    #[cfg(test)]
    pub fn open_scene_at(&self) -> Option<usize> {
        self.backdrop.open.as_ref().and_then(|o| self.doc.scenes.iter().find(|s| s.id == o.scene)).map(|s| s.at)
    }

    /// A drawing is fading in on the page shown.
    #[cfg(test)]
    pub fn scene_fading(&self) -> bool {
        self.backdrop.fade.is_some()
    }

    /// The list of scenes has the small picture of drawing `key`.
    #[cfg(test)]
    pub fn has_thumb_of(&self, key: DrawingKey) -> bool {
        self.backdrop.thumbs.contains_key(&key)
    }

    /// The drawing shown behind the page shown, once rendered.
    #[cfg(test)]
    pub fn shown_scene(&self) -> Option<DrawingKey> {
        self.backdrop.on_target.and_then(|(page, key)| (page == self.target).then_some(key)?)
    }

    /// The parts that move of the drawing shown behind the page shown, and their textures.
    #[cfg(test)]
    pub fn shown_parts(&self) -> Vec<(crate::motion::Motion, egui::TextureId)> {
        self.shown_scene().and_then(|k| self.backdrop.textures.get(&k)).map_or(Vec::new(), Drawn::part_textures)
    }

    /// A scene whose first drawing was never finished (stopped, failed) is no scene. Those of the
    /// writing followed (being written, read or waiting to be painted) are kept.
    fn drop_unpainted_scenes(&mut self) {
        let b = &self.backdrop;
        let painting = b.job.as_ref().map(|j| j.scene);
        let open = b.open.as_ref().map(|o| o.scene);
        let pinned = |id: u64| b.queue.contains(&id) || open == Some(id) || b.reading.as_ref().is_some_and(|r| r.pins.contains(&id));
        self.doc.scenes.retain(|s| !s.versions.is_empty() || Some(s.id) == painting || pinned(s.id));
    }

    /// Stop painting, and what was waiting to be painted. The scene being written stays open.
    fn stop_painting(&mut self) {
        let b = &mut self.backdrop;
        (b.job, b.reading) = (None, None);
        b.queue.clear();
        self.drop_unpainted_scenes();
    }

    /// Where each sentence of the text `a..b` starts, and whether the last one has ended. A
    /// sentence ends with its line, or with a full stop, question or exclamation mark or ellipsis
    /// (closing quotes may follow) and a space, unless it goes on in lower case, as after
    /// "Where is she?" in "\"Where is she?\" he asked."
    fn sentence_starts(&self, a: usize, b: usize) -> (Vec<usize>, bool) {
        let text = &self.doc.flow.text[self.doc.char_to_byte(a)..self.doc.char_to_byte(b)];
        let (mut starts, mut ended, mut by_line, mut closing) = (Vec::new(), true, true, false);
        for (i, c) in text.chars().enumerate() {
            if is_terminator(c) {
                (ended, by_line) = (true, true);
            } else if c.is_whitespace() || c == IMAGE_CHAR {
                ended |= closing;
            } else {
                if ended && (by_line || !c.is_lowercase()) {
                    starts.push(a + i);
                }
                (ended, by_line) = (false, false);
                let quote = matches!(c, '"' | '\'' | '\u{201d}' | '\u{2019}' | '\u{bb}' | ')');
                closing = matches!(c, '.' | '!' | '?' | '\u{2026}') || (closing && quote);
            }
        }
        (starts, ended)
    }

    /// Following the writing, a scene is opened where the sentence written in begins: the one the
    /// last edit began in, or the first after it if it began between sentences.
    fn open_at_edit(&mut self) {
        let p = self.doc.history.last_at().unwrap_or(self.caret).min(self.caret);
        let line = self.doc.para_start(p).0;
        let (before, between) = self.sentence_starts(line, p);
        let at = match before.last() {
            Some(&at) if !between => at,
            _ => self.sentence_starts(line, self.caret).0.into_iter().find(|&s| s >= p).unwrap_or(p),
        };
        let id = self.unpainted_scene(at);
        self.backdrop.open = Some(Open { scene: id, read: 0 });
    }

    /// A scene pinned at `at`, not yet painted.
    fn unpainted_scene(&mut self, at: usize) -> u64 {
        let id = self.doc.next_scene_id;
        self.doc.next_scene_id += 1;
        self.doc.scenes.push(Scene { id, at, versions: Vec::new(), shown: 0, subject: String::new(), hidden: false, people: Vec::new(), chosen: false });
        id
    }

    /// Have Claude read the sentences finished since the scene being written was last read, up to
    /// the caret, to say which begin a new scene. The scene's first sentence begins it unasked.
    fn read_new_sentences(&mut self) {
        let Some(open) = &self.backdrop.open else { return };
        let (id, read) = (open.scene, open.read);
        let Some(start) = self.doc.scenes.iter().find(|s| s.id == id).map(|s| s.at) else {
            self.backdrop.open = None;
            return;
        };
        if self.caret < start {
            return;
        }
        let (starts, ended) = self.sentence_starts(start, self.caret);
        let done = if ended { starts.len() } else { starts.len().saturating_sub(1) };
        let from = read.max(1);
        if done <= from {
            if let Some(o) = &mut self.backdrop.open {
                o.read = o.read.max(done);
            }
            return;
        }
        let end = |k: usize| starts.get(k + 1).copied().unwrap_or(self.caret);
        let scene = self.selected_passage(start, starts[from]).trim().to_owned();
        let sentences: Vec<String> = (from..done).map(|k| self.selected_passage(starts[k], end(k)).trim().to_owned()).collect();
        let pins: Vec<u64> = (from..done).map(|k| self.unpainted_scene(starts[k])).collect();
        let prompt = reading_prompt(&scene, &sentences);
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (ask, n, stop) = (prompt.clone(), sentences.len(), cancel.clone());
        crate::platform::spawn(move || {
            let _ = tx.send(read_sentences(&ask, n, &stop));
        });
        if let Some(o) = &mut self.backdrop.open {
            o.read = from;
        }
        self.backdrop.reading = Some(Reading { rx, cancel, pins, prompt });
    }

    /// Claude has read the new sentences (or could not, and then they go on the scene being
    /// written). At each that begins a new scene, the scene before it ends and is queued to be
    /// painted, with its text up to there; the new one is the scene being written.
    fn reading_done(&mut self, reading: Reading, firsts: Result<Vec<usize>, String>) {
        let firsts = firsts.unwrap_or_default();
        let pin_at = |app: &Self, id: u64| app.doc.scenes.iter().find(|s| s.id == id).map(|s| s.at);
        for &k in &firsts {
            let (Some(open), new) = (self.backdrop.open.take(), reading.pins[k]) else { break };
            if let (Some(a), Some(b)) = (pin_at(self, open.scene), pin_at(self, new)) {
                let subject = self.selected_passage(a, b).trim().to_owned();
                if let Some(s) = self.doc.scene_mut(open.scene) {
                    s.subject = subject;
                    self.backdrop.queue.push_back(open.scene);
                }
            }
            self.backdrop.open = Some(Open { scene: new, read: 0 });
        }
        let n = reading.pins.len();
        if let Some(o) = &mut self.backdrop.open {
            o.read = match firsts.last() {
                Some(&k) => n - k,
                None => o.read + n,
            };
        }
        drop(reading);
        self.drop_unpainted_scenes();
    }

    /// Take in what Claude said about the new sentences, once it has.
    fn take_reading(&mut self) {
        let Some(reading) = &self.backdrop.reading else { return };
        let firsts = match reading.rx.try_recv() {
            Ok(firsts) => firsts,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("the sentences were not read".into()),
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        };
        let reading = self.backdrop.reading.take().unwrap();
        self.reading_done(reading, firsts);
    }

    /// Take in the scene settings of a document just opened (its scenes came with the `Doc`). An
    /// older file's one drawing becomes a scene pinned to the story's start. Following the
    /// writing, what is already written counts as drawn: only writing on brings pictures.
    pub fn open_scene(&mut self, file: Option<SceneFile>) {
        let file = file.unwrap_or_default();
        let (visible, moving) = (!file.hidden, !file.still);
        self.backdrop = Backdrop { mode: self.backdrop.mode, description: file.description, visible, moving, ..Backdrop::default() };
        if let Some(svg) = file.svg.filter(|_| self.doc.scenes.is_empty()) {
            let id = self.doc.next_scene_id;
            self.doc.next_scene_id += 1;
            self.doc.scenes.push(Scene { id, at: 0, versions: vec![svg], shown: 0, subject: String::new(), hidden: false, people: Vec::new(), chosen: false });
        }
    }

    /// Following the writing: the first edit opens a scene at the sentence written in. After a
    /// pause in the writing, Claude reads the sentences finished since and says which begin a new
    /// scene; the scene before each is then painted, once. Scenes waiting are painted in turn.
    fn follow_writing(&mut self, ctx: &egui::Context, now: f64) {
        let version = self.doc.version;
        if self.backdrop.seen != Some(version) {
            let first = self.backdrop.seen.is_none();
            (self.backdrop.seen, self.backdrop.edited_at) = (Some(version), now);
            if !first && self.backdrop.open.is_none() {
                self.open_at_edit();
            }
        }
        self.take_reading();
        if self.backdrop.job.is_none()
            && let Some(id) = self.backdrop.queue.pop_front()
            && let Some(s) = self.doc.scenes.iter().find(|s| s.id == id)
        {
            let (subject, at) = (Subject::Passage(s.subject.clone()), s.at);
            self.start_scene(ctx, subject, at);
            return;
        }
        let b = &self.backdrop;
        if b.reading.is_some() || b.open.is_none() || b.read_at == Some(version) {
            return;
        }
        let wait = b.edited_at + PAUSE - now;
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        } else {
            self.backdrop.read_at = Some(version);
            self.read_new_sentences();
        }
    }

    /// Set where new scenes come from.
    pub fn set_scene_mode(&mut self, mode: Mode) {
        let b = &mut self.backdrop;
        if b.mode == mode {
            return;
        }
        b.mode = mode;
        // Following the writing starts with what is written next.
        (b.seen, b.read_at, b.open) = (None, None, None);
        b.queue.clear();
        self.stop_painting();
    }

    /// Take in finished paintings and renderings, keep the drawings near the page shown rendered,
    /// and fade between drawings on the page shown.
    pub fn update_backdrop(&mut self, ctx: &egui::Context) {
        self.recover_paintings(ctx);
        let now = ctx.input(|i| i.time);
        if self.backdrop.mode == Mode::Writing {
            self.follow_writing(ctx, now);
        }
        self.take_painting(ctx);
        while let Ok(r) = self.backdrop.rendered.1.try_recv() {
            match r.image {
                Ok(image) => {
                    self.backdrop.rendering.remove(&(r.key, r.thumb));
                    if r.thumb {
                        self.backdrop.thumbs.insert(r.key, ctx.load_texture("scene", image.still, TextureOptions::LINEAR));
                    } else {
                        self.backdrop.textures.insert(r.key, Drawn::load(ctx, image));
                    }
                }
                Err(e) => self.status = format!("- a scene could not be shown: {e}"),
            }
        }

        // The drawings of the pages near the one shown, and of the list, rendered ahead.
        let (lo, hi) = (self.target.min(self.pos as usize).saturating_sub(NEAR), (self.target.max(self.pos.ceil() as usize) + NEAR).min(self.last()));
        // Those at the pages' tops, and those that begin on them, for the caret to come to.
        let (from, to) = (self.doc.spans[lo].start, self.doc.spans[hi].end);
        let begin_on = self.doc.scenes.iter().filter(|s| (from..=to).contains(&s.at) && s.svg().is_some()).map(|s| (s.id, s.shown));
        // The caret's is kept too, wherever the pages are turned, to be there on coming back.
        let caret = self.doc.scene_at(self.caret).map(|s| (s.id, s.shown));
        let near: Vec<DrawingKey> = (lo..=hi).filter_map(|i| self.scene_key_of_page(i)).chain(begin_on).chain(caret).collect();
        for &key in &near {
            if let Some(svg) = self.doc.scenes.iter().find(|s| s.id == key.0).and_then(|s| s.versions.get(key.1)).cloned() {
                self.backdrop.render(ctx, key, &svg, false);
            }
        }
        if self.backdrop.list {
            let all: Vec<(DrawingKey, String)> = self.doc.scenes.iter().filter_map(|s| Some(((s.id, s.shown), s.svg()?.to_owned()))).collect();
            for (key, svg) in all {
                self.backdrop.render(ctx, key, &svg, true);
            }
        }

        // On the page shown, a new drawing (the caret moved into another scene's part, painted,
        // another version, hidden) fades in. Turned to, a page shows at once what it showed while
        // turning, which is the same.
        let wanted = self.scene_here().map(|s| (s.id, s.shown));
        let b = &mut self.backdrop;
        let ready = wanted.filter(|k| b.textures.contains_key(k));
        match b.on_target {
            Some((page, shown)) if page == self.target => {
                if shown != ready && (ready.is_some() || wanted.is_none()) {
                    b.fade = Some((shown, now));
                    b.on_target = Some((page, ready));
                }
            }
            _ => {
                b.on_target = Some((self.target, ready));
                b.fade = None;
            }
        }
        if b.fade.is_some_and(|(_, since)| now - since < FADE) {
            ctx.request_repaint();
        } else {
            b.fade = None;
        }
        // A picture that moves on the page shown is drawn again and again, while it can be seen.
        let shown = b.on_target.and_then(|t| t.1).into_iter().chain(b.fade.and_then(|f| f.0));
        let moves = b.visible && b.moving && shown.filter_map(|k| b.textures.get(&k)).any(Drawn::moves);
        if moves && !ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(MOVE_FRAME));
        }

        // Only the drawings near the page shown, and the ones fading, stay rendered.
        if b.textures.len() > near.len() + KEEP {
            let keep: HashSet<DrawingKey> = near.into_iter().chain(b.on_target.and_then(|t| t.1)).chain(b.fade.and_then(|f| f.0)).collect();
            b.textures.retain(|k, _| keep.contains(k));
        }
    }

    /// Can scenes be painted here: by Claude Code, or by Caprice's server for a writer signed in.
    pub fn can_paint(&self) -> bool {
        (self.claude || self.cloud.who.is_some()) && self.can_edit()
    }

    /// Caprice's server answered a painting: the picture is rendered as Claude's would be, or the
    /// server's reason goes to the writer.
    fn painted_online(&mut self, ctx: &egui::Context, answer: Answer, tx: Easel) {
        if let Some(left) = answer.header("X-Caprice-Scenes-Left").and_then(|n| n.parse().ok()) {
            self.backdrop.left = Some(left);
        }
        let said = String::from_utf8_lossy(&answer.body).trim().to_owned();
        let failed = match answer.status {
            200 => {
                let wake = crate::platform::Waker::new(ctx);
                crate::platform::spawn(move || {
                    let _ = tx.send(layered(&said, RENDER_WIDTH).map(|image| (said, image)));
                    wake.wake();
                });
                return;
            }
            0 => "no connection to Caprice's server".to_owned(),
            401 => "signed out: sign in again to paint scenes".to_owned(),
            503 if said.starts_with("No more scenes") => {
                self.backdrop.left = Some(0);
                said
            }
            _ if said.is_empty() => format!("Caprice's server answered {}", answer.status),
            _ => said,
        };
        let _ = tx.send(Err(failed));
    }

    /// A painting finished: it becomes its scene's newest version, shown.
    fn take_painting(&mut self, ctx: &egui::Context) {
        let answer = self.backdrop.job.as_ref().and_then(|j| j.online.as_ref()).and_then(|(rx, _)| match rx.try_recv() {
            Ok(answer) => Some(answer),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Answer::default()),
        });
        if let Some(answer) = answer
            && let Some((_, tx)) = self.backdrop.job.as_mut().and_then(|j| j.online.take())
        {
            self.painted_online(ctx, answer, tx);
        }
        let Some(job) = &self.backdrop.job else { return };
        let id = job.scene;
        match job.rx.try_recv() {
            Ok(Ok((svg, image))) => {
                let people = self.backdrop.job.as_mut().map(|j| std::mem::take(&mut j.people)).unwrap_or_default();
                let online_id = self.backdrop.job.as_mut().and_then(|j| j.online_id.take());
                self.backdrop.job = None;
                // Kept below (`keep_painting`); the server's copy is no longer needed.
                if let Some(painting) = online_id {
                    self.forget_painting(&painting);
                }
                if let Some(scene) = self.doc.scene_mut(id) {
                    scene.people = people;
                    scene.versions.push(svg);
                    scene.shown = scene.versions.len() - 1;
                    scene.hidden = false;
                    self.backdrop.textures.insert((id, scene.shown), Drawn::load(ctx, image));
                    self.backdrop.visible = true;
                    self.keep_painting();
                }
            }
            Ok(Err(e)) => {
                self.status = format!("- no picture: {e}");
                self.backdrop.job = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.backdrop.job = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        }
        self.drop_unpainted_scenes();
    }

    /// Tell Caprice's server that a picture it kept is in the story now. Nothing to wait for: if
    /// the request is lost, the picture is found again next time and is in the story already.
    fn forget_painting(&self, painting: &str) {
        if let (Some(server), Some(wake)) = (self.cloud.server.clone(), self.cloud.waker()) {
            let request = Request { method: "DELETE", path: format!("/api/scenes/{painting}"), headers: Vec::new(), body: None };
            server.send(request, channel().0, wake);
        }
    }

    /// Once for each story opened while signed in, where Caprice's server paints: the pictures
    /// that finished for it after the page went (reloaded, or another story opened while one was
    /// painted). Each goes to the scene pinned where it was asked for, a new one if there is none
    /// (an unpainted scene is not saved, R14), and is kept at once (`keep_painting`).
    fn recover_paintings(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.backdrop.recovering {
            let answer = match rx.try_recv() {
                Ok(answer) => answer,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Answer::default(),
            };
            self.backdrop.recovering = None;
            #[derive(serde::Deserialize)]
            struct Waiting {
                id: String,
                at: usize,
                svg: String,
            }
            let Ok(waiting) = serde_json::from_slice::<Vec<Waiting>>(&answer.body) else { return };
            // One still arriving in this very job is taken as it comes, not twice.
            let arriving = self.backdrop.job.as_ref().and_then(|j| j.online_id.clone());
            let mut found = false;
            for w in waiting.into_iter().filter(|w| Some(&w.id) != arriving.as_ref()) {
                let at = self.doc.para_start(w.at.min(self.doc.total_chars() - 1)).0;
                match self.doc.scenes.iter_mut().find(|s| s.at == at) {
                    Some(s) if !s.versions.contains(&w.svg) => {
                        s.versions.push(w.svg);
                        s.shown = s.versions.len() - 1;
                        s.hidden = false;
                    }
                    Some(_) => {}
                    None => {
                        let (end, _) = self.doc.term_from(at, self.doc.char_to_byte(at));
                        let id = self.doc.next_scene_id;
                        self.doc.next_scene_id += 1;
                        let subject = self.selected_passage(at, end);
                        self.doc.scenes.push(Scene { id, at, versions: vec![w.svg], shown: 0, subject, hidden: false, people: Vec::new(), chosen: false });
                    }
                }
                found = true;
                self.forget_painting(&w.id);
            }
            if found {
                self.backdrop.visible = true;
                self.keep_painting();
                self.status = "- a scene painted while you were away is back".into();
            }
            return;
        }
        let wanted = (!self.claude && self.cloud.who.is_some() && self.path.is_some() && self.can_edit()).then(|| self.story_id.clone());
        if wanted.is_none() || wanted == self.backdrop.recovered {
            return;
        }
        let (Some(server), Some(story)) = (self.cloud.server.clone(), wanted) else { return };
        let (tx, rx) = channel();
        let request = Request { method: "GET", path: format!("/api/scenes?story={story}"), headers: Vec::new(), body: None };
        server.send(request, tx, crate::platform::Waker::new(ctx));
        self.backdrop.recovering = Some(rx);
        self.backdrop.recovered = Some(story);
    }

    /// A painting is kept at once, not when the story would save itself: it was paid for, and
    /// Claude never paints the same one twice. A story with a file is saved (so it goes online
    /// soon after, R33); one without, in the browser, goes on the shelf under the name it
    /// suggests. A version is kept as well, so dropping the changes since saving can bring it back.
    fn keep_painting(&mut self) {
        if self.keep.autosave {
            match (self.path.is_some(), self.shelf.dir.clone()) {
                (true, _) => self.save_quietly(),
                (false, Some(dir)) => {
                    let path = crate::platform::free_name(&dir, &format!("{}.caprice", crate::shelf::file_name(&self.suggested_name())));
                    self.save_to(path);
                    if self.path.is_some() {
                        self.status = format!("- kept on the shelf as \u{201c}{}\u{201d}, with its scene", self.story_name());
                    }
                }
                (false, None) => {}
            }
        }
        self.keep_now();
    }

    /// The toolbar's Scene menu: follow the writing, describe a scene, the list of scenes, show or
    /// hide the pictures.
    pub fn scene_menu(&mut self, ui: &mut egui::Ui) {
        let count = self.doc.scenes.iter().filter(|s| !s.versions.is_empty()).count();
        let has_picture = self.scene_here().is_some();
        let mut peeks = self.doc.peeks;
        // The switch shows once someone can peek out: until then it would only give the surprise away.
        let can_peek = self.doc.cast.iter().any(|c| c.looks.iter().any(|l| l.peek.is_some()));
        let (can_paint, online, can_edit) = (self.can_paint(), !self.claude, self.can_edit());
        let b = &mut self.backdrop;
        let title = if b.job.is_some() { "Scene \u{b7} painting\u{2026}" } else { "Scene" };
        let (mut mode, mut describe, mut stop, mut save, mut cast) = (b.mode, false, false, false, false);
        let tip = "Pictures Claude paints faintly behind the pages, each for its part of the story";
        egui::containers::menu::MenuButton::new(title).ui(ui, |ui| {
            ui.set_min_width(230.0);
            // Not in the browser, for now: there it is the only way into following the writing.
            if cfg!(not(target_arch = "wasm32")) {
                let mut follow = mode == Mode::Writing;
                let follow_tip = "As you write, Claude groups your sentences into scenes and paints each once the next begins";
                if ui.checkbox(&mut follow, "Follow my writing").on_hover_text(follow_tip).changed() {
                    mode = if follow { Mode::Writing } else { Mode::Described };
                }
            }
            let sign_in = "Sign in (File \u{203a} Stories online) to have Claude paint scenes";
            if ui.add_enabled(can_paint, egui::Button::new("Describe the scene\u{2026}").frame(false)).on_disabled_hover_text(sign_in).clicked() {
                describe = true;
                ui.close();
            }
            // Caprice's server paints so many a day, for everyone together.
            if online && can_paint && let Some(left) = b.left {
                let left = match left {
                    0 => "No more scenes today".to_owned(),
                    1 => "1 scene left today".to_owned(),
                    n => format!("{n} scenes left today"),
                };
                ui.label(egui::RichText::new(left).small().color(pal().text_dim));
            }
            let list_tip = "Every scene painted, by where it is in the story";
            if ui.add_enabled(count > 0, egui::Button::new(format!("All scenes ({count})\u{2026}")).frame(false)).on_hover_text(list_tip).clicked() {
                b.list = true;
                ui.close();
            }
            let cast_tip = "The people of the story, painted the same in every scene that names them";
            if ui.add(egui::Button::new("Cast\u{2026}").frame(false)).on_hover_text(cast_tip).clicked() {
                cast = true;
                ui.close();
            }
            if can_peek && can_edit {
                let peek_tip = "Those of the cast with a peek drawn look out from behind the page every minute or two, while you are not writing";
                ui.checkbox(&mut peeks, "Cast peeks out").on_hover_text(peek_tip);
            }
            ui.separator();
            ui.add_enabled(count > 0, egui::Checkbox::new(&mut b.visible, "Show pictures"));
            let moving_tip = "Clouds drift, wings beat, smoke rises from chimneys, lights flicker: whatever Claude marked in the picture as moving";
            ui.add_enabled(count > 0 && b.visible, egui::Checkbox::new(&mut b.moving, "Pictures move")).on_hover_text(moving_tip);
            if ui.add_enabled(has_picture, egui::Button::new("Save picture as\u{2026}").frame(false)).clicked() {
                save = true;
                ui.close();
            }
            if b.drawing() && ui.add(egui::Button::new("Stop painting").frame(false)).clicked() {
                stop = true;
            }
        })
        .0
        .on_hover_text(tip);
        self.doc.peeks = peeks;
        if stop {
            self.stop_painting();
        }
        if describe {
            let b = &mut self.backdrop;
            b.panel = true;
            b.focus_field = true;
        }
        self.set_scene_mode(mode);
        if cast {
            self.cast_panel.open = true;
        }
        if save {
            self.save_scene_picture(&ui.ctx().clone());
        }
    }

    /// Write the picture shown now to a PNG (or, given a .svg name, an SVG) file the user picks.
    fn save_scene_picture(&mut self, ctx: &egui::Context) {
        if self.scene_here().is_none() {
            return;
        }
        let stem = self.path.as_ref().and_then(|p| p.file_stem()).map_or("Scene".to_owned(), |s| format!("{} scene", s.to_string_lossy()));
        let dialog = crate::platform::Dialog::new()
            .add_filter("PNG picture", &["png"])
            .add_filter("SVG drawing", &["svg"])
            .set_file_name(format!("{stem}.png"));
        self.ask_path(ctx, crate::fileio::DialogFor::ScenePicture, dialog);
    }

    pub fn save_scene_picture_to(&mut self, path: std::path::PathBuf) {
        let Some(svg) = self.scene_here().and_then(|s| s.svg()).map(str::to_owned) else { return };
        let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        let written = if is_svg {
            fs::write(&path, svg.as_bytes()).map_err(|e| e.to_string())
        } else {
            rasterize(&svg, RENDER_WIDTH).and_then(|image| {
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                let (w, h) = (image.size[0] as u32, image.size[1] as u32);
                let rgba = image::RgbaImage::from_raw(w, h, rgba).ok_or("the picture has no size")?;
                // Made in memory, then written as any file is (in the browser, downloaded).
                let mut png = std::io::Cursor::new(Vec::new());
                rgba.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
                fs::write(&path, png.into_inner()).map_err(|e| e.to_string())
            })
        };
        let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        self.status = match written {
            Ok(()) => format!("- picture saved as {name}"),
            Err(e) => format!("- picture not saved: {e}"),
        };
    }

    /// The dialog to describe the scene. It closes once the picture is asked for.
    pub fn scene_panel(&mut self, ctx: &egui::Context) {
        if !self.backdrop.panel {
            return;
        }
        let (mut open, mut draw) = (true, false);
        let (can_paint, online) = (self.can_paint(), !self.claude);
        let b = &mut self.backdrop;
        egui::Window::new("Describe the scene")
            .id(Id::new("scene_panel"))
            .open(&mut open)
            .default_width(crate::ui::window_width(ctx, 340.0))
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("What should Claude paint behind this part of the story?").color(pal().text_dim));
                let field = egui::TextEdit::multiline(&mut b.description)
                    .hint_text("A rural village in a valley between the mountains and the sea, at dusk\u{2026}")
                    .desired_rows(5)
                    .desired_width(f32::INFINITY);
                let field = ui.add(field);
                if std::mem::take(&mut b.focus_field) {
                    field.request_focus();
                }
                if can_paint && field.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
                    draw = true;
                }
                ui.add_space(4.0);
                let can = can_paint && !b.description.trim().is_empty();
                // What leaves the browser, said before it does.
                if online {
                    let sent = "What you describe, and how the people it names look, goes to Claude (Anthropic) to be painted.";
                    ui.label(egui::RichText::new(sent).small().color(pal().text_dim));
                }
                let tip = "Have Claude paint this scene, from the caret's paragraph on (Ctrl+Enter)";
                draw |= ui.add_enabled(can, egui::Button::new("Draw")).on_hover_text(tip).clicked();
            });
        if !open || draw {
            b.panel = false;
        }
        if draw && can_paint && !b.description.trim().is_empty() {
            self.set_scene_mode(Mode::Described);
            self.draw_backdrop(ctx);
        }
    }

    /// The list of scenes, in story order: each to go to, pin at the caret, step through its
    /// versions, show or hide, or delete; who of the cast it is painted with, to choose others and
    /// paint it again.
    pub fn scene_list(&mut self, ctx: &egui::Context) {
        if !self.backdrop.list {
            return;
        }
        let can_paint = self.can_paint();
        enum Do {
            GoTo(usize),
            PinHere(u64),
            Step(u64, isize),
            Hidden(u64, bool),
            Delete(u64),
            Person(u64, String, bool),
            PaintAgain(u64),
        }
        let mut action = None;
        let mut open = true;
        let mut scenes: Vec<&Scene> = self.doc.scenes.iter().filter(|s| !s.versions.is_empty()).collect();
        scenes.sort_by_key(|s| (s.at, s.id));
        let here = self.scene_here().map(|s| s.id);
        egui::Window::new("Scenes")
            .id(Id::new("scene_list"))
            .open(&mut open)
            .default_width(crate::ui::window_width(ctx, 380.0))
            .default_height(480.0)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    for s in &scenes {
                        let key = (s.id, s.shown);
                        ui.horizontal(|ui| {
                            let size = egui::vec2(THUMB_WIDTH * 0.6, THUMB_WIDTH * 0.6 * 1.414);
                            match self.backdrop.thumbs.get(&key) {
                                Some(t) => {
                                    let tint = if s.hidden { Color32::from_white_alpha(90) } else { Color32::WHITE };
                                    ui.add(egui::Image::new((t.id(), size)).tint(tint).bg_fill(Color32::WHITE));
                                }
                                None => {
                                    let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                    ui.painter().rect_filled(r, 2.0, Color32::from_gray(235));
                                }
                            }
                            ui.vertical(|ui| {
                                let page = self.doc.page_of(s.at) + 1;
                                let mark = if here == Some(s.id) { "  \u{b7} shown now" } else { "" };
                                ui.label(egui::RichText::new(format!("Page {page}{mark}")).size(12.0).color(pal().text_dim));
                                ui.label(self.scene_snippet(s.at)).on_hover_text(&s.subject);
                                if !self.doc.cast.is_empty() {
                                    ui.horizontal(|ui| {
                                        let with = if s.people.is_empty() { "With no one from the cast".to_owned() } else { format!("With {}", s.people.join(", ")) };
                                        ui.label(egui::RichText::new(with).size(12.0).color(pal().text_dim));
                                        let tip = "Choose who of the cast is in this scene, then paint it again";
                                        egui::containers::menu::MenuButton::new("People").ui(ui, |ui| {
                                            for c in self.doc.cast.iter().filter(|c| !c.name.trim().is_empty()) {
                                                let name = c.name.trim();
                                                let mut on = s.people.iter().any(|p| p == name);
                                                if ui.checkbox(&mut on, name).changed() {
                                                    action = Some(Do::Person(s.id, name.to_owned(), on));
                                                }
                                            }
                                        })
                                        .0
                                        .on_hover_text(tip);
                                    });
                                }
                                ui.horizontal(|ui| {
                                    if ui.small_button("Go to").on_hover_text("Turn to its place in the story").clicked() {
                                        action = Some(Do::GoTo(s.at));
                                    }
                                    if ui.small_button("Pin here").on_hover_text("Show it from the caret's paragraph on").clicked() {
                                        action = Some(Do::PinHere(s.id));
                                    }
                                    if s.versions.len() > 1 {
                                        if ui.add_enabled(s.shown > 0, egui::Button::new("\u{2039}").small()).clicked() {
                                            action = Some(Do::Step(s.id, -1));
                                        }
                                        ui.label(format!("{}/{}", s.shown + 1, s.versions.len()));
                                        if ui.add_enabled(s.shown + 1 < s.versions.len(), egui::Button::new("\u{203a}").small()).clicked() {
                                            action = Some(Do::Step(s.id, 1));
                                        }
                                    }
                                    let mut shows = !s.hidden;
                                    if ui.checkbox(&mut shows, "Show").changed() {
                                        action = Some(Do::Hidden(s.id, !shows));
                                    }
                                    let again = ui.add_enabled(can_paint, egui::Button::new("Paint again").small());
                                    if again.on_hover_text("Paint it again, as another version").clicked() {
                                        action = Some(Do::PaintAgain(s.id));
                                    }
                                    if ui.small_button("Delete").on_hover_text("Delete this scene and all its versions").clicked() {
                                        action = Some(Do::Delete(s.id));
                                    }
                                });
                            });
                        });
                        ui.separator();
                    }
                });
            });
        self.backdrop.list = open;
        // Another device holds the pen (R34): the list only goes to a scene.
        if !self.can_edit() && !matches!(action, Some(Do::GoTo(_))) {
            action = None;
        }
        let caret_para = self.doc.para_start(self.caret).0;
        match action {
            Some(Do::GoTo(at)) => self.set_caret(ctx, at, false),
            Some(Do::PinHere(id)) => {
                // Another scene pinned to that paragraph already gives way.
                self.doc.scenes.retain(|s| s.id == id || s.at != caret_para);
                if let Some(s) = self.doc.scene_mut(id) {
                    s.at = caret_para;
                }
            }
            Some(Do::Step(id, by)) => {
                if let Some(s) = self.doc.scene_mut(id) {
                    s.shown = s.shown.saturating_add_signed(by).min(s.versions.len() - 1);
                }
            }
            Some(Do::Hidden(id, hidden)) => {
                if let Some(s) = self.doc.scene_mut(id) {
                    s.hidden = hidden;
                }
            }
            Some(Do::Delete(id)) => self.doc.scenes.retain(|s| s.id != id),
            Some(Do::Person(id, name, on)) => {
                if let Some(s) = self.doc.scene_mut(id) {
                    s.people.retain(|p| *p != name);
                    if on {
                        s.people.push(name);
                    }
                    s.chosen = true;
                }
            }
            Some(Do::PaintAgain(id)) => {
                // With the people it shows, rather than looking for them again.
                if let Some(s) = self.doc.scenes.iter().find(|s| s.id == id) {
                    let (subject, at, people) = (Subject::Passage(s.subject.clone()), s.at, s.people.clone());
                    self.paint_with(ctx, subject, at, people);
                }
            }
            None => {}
        }
    }

    /// The first words of the paragraph a scene is pinned to.
    fn scene_snippet(&self, at: usize) -> String {
        let text: String = self.selected_passage(at, self.doc.total_chars()).lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_owned();
        let mut words = String::new();
        for w in text.split_whitespace() {
            if words.len() + w.len() > 60 {
                words.push('\u{2026}');
                break;
            }
            if !words.is_empty() {
                words.push(' ');
            }
            words.push_str(w);
        }
        if words.is_empty() { "(empty)".into() } else { words }
    }

    /// The scene behind page `i` at `rect` (on screen through `map`), at `shade` and `alpha`. On
    /// the page shown, a new drawing fades in over the one before.
    pub fn backdrop_shapes(&self, ctx: &egui::Context, i: usize, rect: Rect, map: &dyn Fn(Pos2) -> Pos2, shade: f32, alpha: f32) -> Vec<Shape> {
        let b = &self.backdrop;
        if !b.visible {
            return Vec::new();
        }
        let tint = |a: f32| {
            let v = (255.0 * shade).clamp(0.0, 255.0) as u8;
            Color32::from_rgb(v, v, v).gamma_multiply(a * alpha * OPACITY)
        };
        let now = ctx.input(|inp| inp.time);
        let time = b.moving.then_some(now);
        let shapes = |key: Option<DrawingKey>, a: f32| key.and_then(|k| b.textures.get(&k)).map(|d| d.shapes(rect, tint(a), map, time)).unwrap_or_default();
        match b.on_target {
            Some((page, shown)) if page == i => {
                let Some((from, since)) = b.fade else { return shapes(shown, 1.0) };
                let t = ((now - since) / FADE).clamp(0.0, 1.0) as f32;
                let t = t * t * (3.0 - 2.0 * t);
                shapes(from, 1.0 - t).into_iter().chain(shapes(shown, t)).collect()
            }
            _ => shapes(self.scene_key_of_page(i), 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drawing_is_found_and_rendered() {
        let reply = "```svg\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 60 85\"><circle cx=\"30\" cy=\"40\" r=\"20\" fill=\"#36c\"/></svg>\n```";
        let svg = extract_svg(reply).unwrap();
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        let image = rasterize(svg, RENDER_WIDTH).unwrap();
        assert_eq!(image.size, [1000, 1417]);
        let middle = image.pixels[708 * 1000 + 500];
        assert!(middle.b() > 150 && middle.a() == 255, "the circle is drawn: {middle:?}");
        assert_eq!(image.pixels[0].a(), 0, "the background stays transparent");
        assert!(extract_svg("no drawing here").is_none());
    }

    /// Asks the real Claude for a scene through the Claude Code CLI: `cargo test live_scene -- --ignored`.
    #[test]
    #[ignore]
    fn live_reading() {
        crate::claude::allow_live();
        let scene = "The harbour was full of boats. Gulls cried over the masts.";
        let sentences = ["\"Where is Mara?\" asked the fisherman.", "\"Up at the house,\" said the boy, pointing at the hill.",
            "Inside the old house, the fire was out.", "Mara lit a candle and sat down."].map(String::from);
        let firsts = read_sentences(&reading_prompt(scene, &sentences), sentences.len(), &AtomicBool::new(false)).unwrap();
        println!("new scenes begin at sentences {firsts:?} (from 0)");
        assert_eq!(firsts, vec![2], "the dialogue stays on the harbour, the house is a new scene");
    }

    #[test]
    #[ignore]
    fn live_scene() {
        crate::claude::allow_live();
        let passage = "Far away, a lone ship fought its way through a storm on the open sea.";
        let prompt = scene_prompt(&Subject::Passage(passage.into()), egui::vec2(595.0, 842.0), &[]);
        let (svg, painted) = draw_scene(&prompt, &AtomicBool::new(false)).unwrap().unwrap();
        println!("moving parts: {:?}", painted.layers.iter().map(|l| l.motion).collect::<Vec<_>>());
        let image = rasterize(&svg, RENDER_WIDTH).unwrap();
        let path = std::env::temp_dir().join("caprice-live-scene.png");
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8).unwrap();
        println!("scene saved to {}", path.display());
    }
}
