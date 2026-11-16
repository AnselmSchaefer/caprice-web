//! Making a theme with Claude (R36, the plan's step 5). The writer says what they would like on
//! the desk; Caprice's server has Claude write it as a program (`/api/theme-program`, the Worker's
//! `web/api/theme.js`), and it comes back as its source and its palette only. This browser builds
//! it (`platform::compiler`), checks it as every program is checked, and plays it off screen
//! (`eggs::preview`). What goes wrong on the way goes back to Claude without the writer seeing it:
//! the compiler's words, a program Caprice will not play, one that stops. One that works is shown
//! to Claude as pictures once, to be made better, and then to the writer, who keeps it or asks for
//! a change. A version that worked is never lost to one that did not (the owner's decisions for
//! step 5: a writer is never left with a broken theme).
//!
//! The compiler is started first, so a browser that cannot build says so before Claude is asked,
//! and it loads while Claude writes; it is ended once there is nothing more to build, for it holds
//! a gigabyte. Closing the window hides it: the theme goes on being made.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, TextureHandle, TextureOptions, vec2};
use serde::Deserialize;

use crate::App;
use crate::eggs::preview::{Preview, Previewed};
use crate::platform::compiler::{Built, Compiler, State};
use crate::platform::{Answer, Request, Waker};
use crate::theme::{Theme, pal};

/// The most of a writer's request, in bytes, as the server takes it (`THEME_REQUEST_BYTES`).
pub const MOST_REQUEST: usize = 2000;
/// The most of the compiler's words sent back, in bytes (the server takes 20 000).
const MOST_ERRORS: usize = 19_000;
/// Mistakes Claude is asked to fix for one theme, as the server allows (`THEME_ERRORS_ROUNDS`).
const FIXES: u8 = 3;
/// Times Claude is shown the pictures of a theme that works, to make it better: each costs about
/// as much as writing it did (THEME_PROGRAMS_PLAN.md, step 5's measurement).
const LOOKS: u8 = 1;
/// What the writer is told of where their words go.
pub const GOES_TO_CLAUDE: &str = "What you write here goes to Claude (Anthropic), through Caprice's server, to write your theme.";

/// What the server answers a round with: the theme's id there, its source and palette.
#[derive(Deserialize)]
struct Written {
    theme: String,
    source: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    about: String,
    #[serde(default)]
    colours: BTreeMap<String, String>,
}

/// A version Claude wrote: its source and palette, and its module once it is built.
#[derive(Clone)]
struct Version {
    source: String,
    name: String,
    about: String,
    colours: BTreeMap<String, String>,
    module: Vec<u8>,
    toolchain: String,
}

/// A version that built and played, with its pictures.
struct Good {
    version: Version,
    previewed: Previewed,
    textures: Vec<TextureHandle>,
}

/// The kinds of round, as the server names them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Begin,
    Errors,
    Frames,
    Change,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Begin => "begin",
            Kind::Errors => "errors",
            Kind::Frames => "frames",
            Kind::Change => "change",
        }
    }
}

/// Where the making of a theme is.
#[derive(Default)]
enum Step {
    /// The writer saying what they would like.
    #[default]
    Writing,
    /// The compiler starting, before Claude is asked: a round of this kind, with this body.
    Starting(Kind, Vec<u8>, Vec<(&'static str, String)>),
    /// Claude at a round.
    Waiting(Kind),
    Building,
    Picturing(Box<Preview>),
    /// A version that works, shown to the writer.
    Ready,
    /// Nothing came of it: why.
    Failed(String),
}

/// The "Make a theme with Claude" window and what it is doing.
#[derive(Default)]
pub struct Making {
    pub open: bool,
    pub request: String,
    pub change: String,
    step: Step,
    /// The theme's id on the server, once it is begun.
    theme: Option<String>,
    answer: Option<Receiver<Answer>>,
    compiler: Option<Compiler>,
    /// The version in hand: being built or played.
    trying: Option<Version>,
    good: Option<Good>,
    fixes_left: u8,
    looks_left: u8,
    /// What the writer should know beside the version shown (a change that did not work out).
    note: Option<String>,
}

impl Making {
    /// Whether Claude, the compiler or the preview is at work.
    pub fn busy(&self) -> bool {
        matches!(self.step, Step::Starting(..) | Step::Waiting(_) | Step::Building | Step::Picturing(_))
    }

    /// The kind of round Claude is at, if it is at one.
    #[cfg(test)]
    pub fn waiting(&self) -> Option<Kind> {
        match self.step {
            Step::Waiting(k) => Some(k),
            _ => None,
        }
    }
}

/// `text` cut to at most `most` bytes, on a character's edge.
fn cut(text: &str, most: usize) -> &str {
    let mut end = text.len().min(most);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// A round's pictures as the server takes them: how many (a byte), each one's length (four bytes,
/// little-endian), then the PNGs.
fn pictures_body(pngs: &[Vec<u8>]) -> Vec<u8> {
    let mut body = vec![pngs.len() as u8];
    for p in pngs {
        body.extend_from_slice(&(p.len() as u32).to_le_bytes());
    }
    for p in pngs {
        body.extend_from_slice(p);
    }
    body
}

impl App {
    /// Open the window, as it was left.
    pub fn make_theme(&mut self) {
        self.making.open = true;
    }

    /// Begin a theme from the writer's request: the compiler first, then Claude.
    fn begin_theme(&mut self, ctx: &egui::Context) {
        let request = cut(self.making.request.trim(), MOST_REQUEST).to_owned();
        let m = &mut self.making;
        m.theme = None;
        m.good = None;
        m.note = None;
        m.fixes_left = FIXES;
        m.looks_left = LOOKS;
        self.round(ctx, Kind::Begin, request.into_bytes(), Vec::new());
    }

    /// A round of `kind`: asked once the compiler has started (and not refused to).
    fn round(&mut self, ctx: &egui::Context, kind: Kind, body: Vec<u8>, headers: Vec<(&'static str, String)>) {
        if self.making.compiler.is_none() {
            self.making.compiler = Some(Compiler::start(Waker::new(ctx)));
        }
        self.making.step = Step::Starting(kind, body, headers);
    }

    fn send_round(&mut self, ctx: &egui::Context, kind: Kind, body: Vec<u8>, mut headers: Vec<(&'static str, String)>) {
        let Some(server) = self.cloud.server.clone() else {
            self.making.step = Step::Failed("Making a theme needs Caprice in the browser, signed in.".into());
            return;
        };
        let path = match (&self.making.theme, kind) {
            (Some(id), k) if k != Kind::Begin => {
                headers.push(("X-Caprice-Kind", kind.name().into()));
                format!("/api/theme-program/{id}")
            }
            _ => "/api/theme-program".into(),
        };
        if kind != Kind::Frames {
            headers.push(("Content-Type", "text/plain; charset=utf-8".into()));
        }
        let (tx, rx) = channel();
        server.send(Request { method: "POST", path, headers, body: Some(body) }, tx, Waker::new(ctx));
        self.making.answer = Some(rx);
        self.making.step = Step::Waiting(kind);
    }

    /// What went wrong goes back to Claude to fix, while it may be; else the theme stops at the
    /// last version that worked, or, with none, comes to nothing.
    fn fix(&mut self, ctx: &egui::Context, said: &str) {
        if self.making.fixes_left > 0 {
            self.making.fixes_left -= 1;
            self.round(ctx, Kind::Errors, cut(said, MOST_ERRORS).as_bytes().to_vec(), Vec::new());
        } else {
            self.give_up("Claude could not make this theme work. Try again, perhaps with something simpler.".into());
        }
    }

    /// Stop where it is: at the last version that worked, saying why, or with nothing.
    fn give_up(&mut self, why: String) {
        let m = &mut self.making;
        m.compiler = None;
        m.trying = None;
        m.answer = None;
        if m.good.is_some() {
            m.note = Some(format!("{why} This is the version before."));
            m.step = Step::Ready;
        } else {
            m.step = Step::Failed(why);
        }
    }

    /// Once a frame: the round answered, the version built, played, shown.
    pub fn tend_making(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        match &mut self.making.step {
            Step::Starting(..) => match self.making.compiler.as_mut().map(|c| c.state(now)) {
                Some(State::Failed { why, .. }) => self.give_up(why),
                // Opening: whether this browser can build is not known yet.
                Some(State::Loading { of: 0, .. }) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
                // Loading what it has to, or ready: Claude is asked meanwhile.
                _ => {
                    let Step::Starting(kind, body, headers) = std::mem::take(&mut self.making.step) else { return };
                    self.send_round(ctx, kind, body, headers);
                }
            },
            Step::Waiting(kind) => {
                let kind = *kind;
                let Some(answer) = self.making.answer.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
                self.making.answer = None;
                self.round_answered(kind, answer);
            }
            Step::Building => self.tend_making_build(ctx, now),
            Step::Picturing(preview) => match preview.tend(now) {
                Some(Ok(previewed)) => self.pictured(ctx, previewed),
                Some(Err(why)) => self.fix(ctx, &format!("It built, but Caprice stopped it while playing it: {why}.")),
                None => ctx.request_repaint_after(std::time::Duration::from_millis(50)),
            },
            Step::Writing | Step::Ready | Step::Failed(_) => {}
        }
    }

    /// Claude's answer to a round of `kind`: a version to build, or why there is none.
    fn round_answered(&mut self, kind: Kind, answer: Answer) {
        let said = String::from_utf8_lossy(&answer.body).trim().to_owned();
        let why = match answer.status {
            200 => match serde_json::from_slice::<Written>(&answer.body) {
                Ok(w) => {
                    self.making.theme = Some(w.theme);
                    let version = Version { source: w.source, name: w.name, about: w.about, colours: w.colours, module: Vec::new(), toolchain: String::new() };
                    if let Some(c) = &mut self.making.compiler {
                        c.build(&version.source);
                    }
                    self.making.trying = Some(version);
                    self.making.step = Step::Building;
                    return;
                }
                Err(_) => "Caprice's server answered with something that is not a theme.".to_owned(),
            },
            0 => "Caprice's server could not be reached. Check the connection and try again.".into(),
            401 => "You are signed out: sign in again (File › Stories online), then try again.".into(),
            _ if !said.is_empty() && said.len() < 200 => format!("{}.", said.trim_end_matches('.')),
            code => format!("Caprice's server could not make it ({code})."),
        };
        // A look at the pictures that cannot be had leaves the version that was shown.
        if kind == Kind::Frames && self.making.good.is_some() {
            self.making.compiler = None;
            self.making.step = Step::Ready;
            return;
        }
        self.give_up(why);
    }

    fn tend_making_build(&mut self, ctx: &egui::Context, now: f64) {
        let Some(c) = &mut self.making.compiler else { return };
        let state = c.state(now);
        match (c.take(), state) {
            (Some(Built::Module { module, toolchain, .. }), _) => match crate::eggs::program::check(&module) {
                Ok(()) => {
                    let Some(v) = &mut self.making.trying else { return };
                    v.module = module;
                    v.toolchain = toolchain;
                    let palette = crate::own::with_colours(Theme::Midnight.palette(), &v.colours).0;
                    self.making.step = Step::Picturing(Box::new(Preview::start(&v.module, palette, Waker::new(ctx), now)));
                }
                Err(why) => self.fix(ctx, &format!("It built, but Caprice will not play its program: {}.", why.join("; "))),
            },
            (Some(Built::Errors(said)), _) => self.fix(ctx, &said),
            (None, State::Failed { why, .. }) => self.give_up(why),
            (None, _) => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
        }
    }

    /// A version that played: kept as the one to show, and shown to Claude while it may look.
    fn pictured(&mut self, ctx: &egui::Context, previewed: Previewed) {
        let Some(version) = self.making.trying.take() else { return };
        let textures = previewed.pictures.iter().map(|p| ctx.load_texture(format!("making-{}", p.name), p.image.clone(), TextureOptions::LINEAR)).collect();
        let pngs: Vec<Vec<u8>> = previewed.pictures.iter().filter_map(|p| crate::eggs::preview::png(&p.image)).collect();
        let moments = previewed.pictures.iter().map(|p| p.name).collect::<Vec<_>>().join(",");
        let points = format!("{},{}", previewed.phone_points, previewed.computer_points);
        self.making.good = Some(Good { version, previewed, textures });
        if self.making.looks_left > 0 && pngs.len() == moments.split(',').count() {
            self.making.looks_left -= 1;
            self.round(ctx, Kind::Frames, pictures_body(&pngs), vec![("X-Caprice-Moments", moments), ("X-Caprice-Points", points)]);
        } else {
            self.making.compiler = None;
            self.making.step = Step::Ready;
        }
    }

    /// The version shown, put on the shelf and chosen.
    fn keep_made(&mut self, ctx: &egui::Context) {
        let Some(good) = self.making.good.take() else { return };
        let v = good.version;
        let manifest = crate::bundle::Manifest {
            world: crate::bundle::WORLD.into(), name: v.name.clone(), about: v.about, author: String::new(), colours: v.colours, toolchain: v.toolchain,
        };
        let bundle = crate::bundle::Bundle { manifest, module: v.module, source: Some(v.source), preview: good.previewed.gallery_picture() };
        let opened = crate::bundle::Opened { bundle, found: Vec::new(), notes: Vec::new() };
        let report = self.keep_theme(ctx, v.name, crate::own::Prepared::Program(opened));
        if report.added {
            self.making = crate::making::Making::default();
        } else {
            self.workshop.report = Some(report);
        }
    }

    /// The "Make a theme with Claude" window.
    pub fn making_window(&mut self, ctx: &egui::Context) {
        if !self.making.open {
            return;
        }
        let signed_in = self.cloud.who.is_some();
        let mut open = true;
        let mut act = None;
        let m = &mut self.making;
        let loading = match m.compiler.as_mut().map(|c| c.state(ctx.input(|i| i.time))) {
            Some(State::Loading { fetched, of }) if of > 0 => Some(format!("Loading the compiler: {} of {} MB, only this once", fetched.div_ceil(1 << 20), of.div_ceil(1 << 20))),
            _ => None,
        };
        egui::Window::new("Make a theme with Claude").id(egui::Id::new("theme_making")).open(&mut open).collapsible(false).resizable(false).default_width(480.0).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_max_width(500.0);
            match &m.step {
                Step::Writing => {
                    ui.label("Say what you would like on the desk beside your pages: a thing for each chapter, growing a part with each page you write.");
                    ui.add(egui::TextEdit::multiline(&mut m.request).char_limit(MOST_REQUEST).desired_rows(3).desired_width(f32::INFINITY)
                        .hint_text("A lighthouse on a cliff by the sea, a window lit for each page, a boat for each chapter"));
                    ui.label(egui::RichText::new(GOES_TO_CLAUDE).color(pal().text_dim).size(12.0));
                    if !signed_in {
                        ui.label(egui::RichText::new("Sign in first: Claude makes themes for writers signed in.").color(pal().accent));
                    }
                    ui.horizontal(|ui| {
                        if signed_in {
                            if ui.add_enabled(!m.request.trim().is_empty(), egui::Button::new("Make it")).clicked() {
                                act = Some(Act::Begin);
                            }
                        } else if ui.button("Sign in\u{2026}").clicked() {
                            act = Some(Act::SignIn);
                        }
                    });
                }
                Step::Starting(..) | Step::Waiting(_) | Step::Building | Step::Picturing(_) => {
                    let doing = match &m.step {
                        Step::Starting(..) => "Getting ready\u{2026}",
                        Step::Waiting(Kind::Begin) => "Claude is writing your theme. It takes some minutes: go on writing meanwhile.",
                        Step::Waiting(Kind::Errors) => "Claude is putting something right\u{2026}",
                        Step::Waiting(Kind::Frames) => "Claude is looking at it, to make it lovelier\u{2026}",
                        Step::Waiting(Kind::Change) => "Claude is making your change. It takes some minutes.",
                        Step::Building => "Building it\u{2026}",
                        _ => "Trying it out\u{2026}",
                    };
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(doing);
                    });
                    if let Some(l) = &loading {
                        ui.label(egui::RichText::new(l).color(pal().text_dim).size(12.0));
                    }
                    if let Some(g) = &m.good {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("So far:").color(pal().text_dim).size(12.0));
                        pictures(ui, g);
                    }
                }
                Step::Ready => {
                    if let Some(g) = &m.good {
                        ui.label(egui::RichText::new(&g.version.name).strong().size(16.0));
                        if !g.version.about.is_empty() {
                            ui.label(egui::RichText::new(&g.version.about).color(pal().text_dim));
                        }
                        pictures(ui, g);
                    }
                    if let Some(note) = &m.note {
                        ui.label(egui::RichText::new(note).color(pal().accent));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Keep it").clicked() {
                            act = Some(Act::Keep);
                        }
                        if ui.button("Start again").clicked() {
                            act = Some(Act::Again);
                        }
                    });
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::multiline(&mut m.change).char_limit(MOST_REQUEST).desired_rows(2).desired_width(f32::INFINITY).hint_text("Or say what to change"));
                    if ui.add_enabled(!m.change.trim().is_empty(), egui::Button::new("Change it")).clicked() {
                        act = Some(Act::Change);
                    }
                }
                Step::Failed(why) => {
                    ui.label(egui::RichText::new(why).color(pal().accent));
                    if ui.button("Try again").clicked() {
                        act = Some(Act::Again);
                    }
                }
            }
        });
        self.making.open &= open;
        match act {
            Some(Act::Begin) => self.begin_theme(ctx),
            Some(Act::SignIn) => self.cloud.open = true,
            Some(Act::Keep) => self.keep_made(ctx),
            Some(Act::Again) => {
                let request = std::mem::take(&mut self.making.request);
                self.making = Making { open: true, request, ..Making::default() };
            }
            Some(Act::Change) => {
                let said = cut(self.making.change.trim(), MOST_REQUEST).as_bytes().to_vec();
                self.making.change.clear();
                self.making.note = None;
                self.round(ctx, Kind::Change, said, Vec::new());
            }
            None => {}
        }
    }
}

/// What the writer did in the window.
enum Act {
    Begin,
    SignIn,
    Keep,
    Again,
    Change,
}

/// A version's pictures: three chapters written, large, and the others beside it, small.
fn pictures(ui: &mut egui::Ui, g: &Good) {
    let at = g.previewed.pictures.iter().position(|p| p.name == "written").unwrap_or(0);
    if let Some(t) = g.textures.get(at) {
        ui.add(egui::Image::new(t).fit_to_exact_size(vec2(480.0, 300.0)));
    }
    ui.horizontal(|ui| {
        for (k, t) in g.textures.iter().enumerate() {
            if k != at {
                ui.add(egui::Image::new(t).fit_to_exact_size(vec2(152.0, 95.0)));
            }
        }
    });
}
