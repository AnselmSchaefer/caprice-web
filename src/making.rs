//! Making a theme with Claude (R36, the plan's step 5). The writer says what they would like on
//! the desk; Caprice's server has Claude write it as a program (`/api/theme-program`, the Worker's
//! `web/api/theme.js`), and it comes back as its source and its palette only. This browser builds
//! it (`platform::compiler`), checks it as every program is checked, and plays it off screen
//! (`eggs::preview`). What goes wrong on the way goes back to Claude without the writer seeing it:
//! the compiler's words (`errors`); a program Caprice will not play, one that stops while playing
//! the preview's story, one that draws more than `gallery::BUDGET`, one whose own sky stops short of
//! the window's edges zoomed out (`Previewed::bare`), one with parts that slide as the writer zooms
//! (`Previewed::slides`) (`checks`; the last two go with the pictures instead, while Claude is still
//! to see them). One that works is shown
//! to Claude as pictures once, to be made better (unless the server says the theme has cost enough
//! already), and then to the writer, who keeps it or asks for a change. A version that worked is never lost to one that did not (the owner's decisions for
//! step 5: a writer is never left with a broken theme).
//!
//! The compiler is started first, so a browser that cannot build says so before Claude is asked,
//! and it loads while Claude writes; it is ended once there is nothing more to build, for it holds
//! a gigabyte. Closing the window hides it: the theme goes on being made.
//!
//! The writer never reads Claude's name here (the one line on where their words go names
//! Anthropic). A theme takes many minutes, nearly all of them Claude writing, so the window counts
//! the steps of making it and the time so far, and says plainly what went wrong: the network
//! lost, the server out of reach or saying no. While Claude writes the server says every few
//! seconds that it is still at it (`web/api/theme.js`, `keptAlive`); a minute without word is a
//! connection lost.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, TextureHandle, TextureOptions, vec2};
use serde::Deserialize;

use crate::App;
use crate::eggs::preview::{Preview, Previewed};
use crate::gallery::BUDGET;
use crate::platform::compiler::{Built, Compiler, State};
use crate::platform::{Answer, Request, Waker};
use crate::theme::{Theme, pal};

/// The most of a writer's request, in bytes, as the server takes it (`THEME_REQUEST_BYTES`).
pub const MOST_REQUEST: usize = 2000;
/// The most of the compiler's words sent back, in bytes (the server takes 20 000).
const MOST_ERRORS: usize = 19_000;
/// Mistakes Claude is asked to fix for one theme, as the server allows (`THEME_ERRORS_ROUNDS`).
const FIXES: u8 = 3;
/// Times Claude is shown the pictures of a theme that works, to make it better, at most: each costs
/// about as much as writing it did (THEME_PROGRAMS_PLAN.md, step 5's measurement). The server's
/// answers say whether it may (`left.frames`).
const LOOKS: u8 = 1;
/// What the writer is told of where their words go.
pub const GOES_OUT: &str = "What you write here goes through Caprice's server to an AI model by Anthropic, which writes your theme.";
/// What the writer is told in the sketch view.
pub const SKETCH_GOES_OUT: &str = "What you draw and write here goes through Caprice's server to an AI model by Anthropic, which writes your theme.";
/// What the writer is told of what a theme costs, while Caprice gives them away (the owner's call,
/// 2026-11-19): the themes kept meanwhile stay the writer's whatever comes later.
pub const FREE: &str = "Free while Caprice is new: a theme you keep now stays yours.";
/// What the writer is told a theme takes, in minutes: two rounds of writing, the world laid out and
/// then made finished, about seven to ten minutes each at effort medium (THEME_PROGRAMS_PLAN.md,
/// step 5), and the building and trying between.
pub const UP_TO_MINUTES: u32 = 25;
/// Seconds a round of writing usually takes: how far the bar moves within its step meanwhile.
const TYPICAL_ROUND: f64 = 480.0;
/// Seconds without word from the server while Claude writes before the window says so, and before
/// the connection is taken for lost: it says it is at work every 15 seconds (`BEAT_MS`).
const QUIET: f64 = 25.0;
const LOST: f64 = 60.0;
/// What the writer is told when the theme was lost with the connection.
pub const LOST_NETWORK: &str = "Lost network. The theme could not be finished: check the connection, then try again.";
pub const LOST_SERVER: &str = "Lost the connection to Caprice's server. The theme could not be finished: try again in a moment.";
/// What the writer is told when no version would build and play.
pub const NEVER_WORKED: &str = "The theme could not be made to work. Start again, perhaps with something simpler.";

/// What Claude is told of a version that plays but draws more points a frame than `BUDGET`.
fn over_budget(phone: usize, computer: usize) -> String {
    format!("It plays, but draws too much: at most {phone} points a frame on a phone's desk and {computer} on a computer's \
        (zoomed out included), where a theme keeps within {} and {}. Draw less: fewer shapes, fewer points in each, \
        many small fills as one mesh.", BUDGET.0, BUDGET.1)
}

/// What the server answers a round that went wrong with, once it had begun answering (it sends
/// spaces while Claude writes, so its status was sent before it knew).
#[derive(Deserialize)]
struct Refused {
    error: u16,
    message: String,
}

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
    /// The rounds the server still allows the theme. Only the looks at its pictures matter here:
    /// none once the theme has cost enough already (`THEME_LOOK_MICRO`), and then it is shown as it
    /// is. Read loosely, so that a shape the app does not know never loses the theme.
    #[serde(default)]
    left: serde_json::Value,
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
    /// A theme begun from a hand-drawn sketch (desktop only).
    Sketch,
    Errors,
    /// It built, but did not pass what every theme must: the load check, playing the preview's
    /// story through, the budget of points (the owner's decisions for step 5).
    Checks,
    Frames,
    Change,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Begin => "begin",
            Kind::Sketch => "sketch",
            Kind::Errors => "errors",
            Kind::Checks => "checks",
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

/// The steps from a request or a change to a version shown, as the writer is told them.
#[derive(Default, Clone, Copy)]
struct Run {
    /// When it began, and when the server last said it was at work (the window's clock, seconds).
    began: f64,
    heard: f64,
    /// How many steps it has, and how many looks at the pictures were left when it began (each
    /// adds a round of writing, a build and a try). A look the server allows no more is taken off
    /// both, so that the steps never promise more than will come.
    steps: u8,
    looks: u8,
    /// The step shown, never going back (a mistake put right stays at the step it was found in),
    /// and when it began.
    shown: u8,
    since: f64,
}

/// The "Make a theme" window and what it is doing.
#[derive(Default)]
pub struct Making {
    pub open: bool,
    pub request: String,
    pub change: String,
    /// Desktop-only hand-drawn sketch canvas and whether the writer switched to it.
    pub sketch: crate::sketch::Sketch,
    pub sketching: bool,
    step: Step,
    /// The theme's id on the server, once it is begun.
    theme: Option<String>,
    answer: Option<Receiver<Answer>>,
    /// Told each time the server says it is still writing.
    heard: Option<Receiver<()>>,
    run: Run,
    compiler: Option<Compiler>,
    /// The version in hand: being built or played.
    trying: Option<Version>,
    good: Option<Good>,
    fixes_left: u8,
    looks_left: u8,
    /// Whether the server would have Claude look at the pictures of the version in hand.
    may_look: bool,
    /// What the writer should know beside the version shown (a change that did not work out).
    note: Option<String>,
}

impl Making {
    /// Whether the writer is currently drawing on the desktop sketch sheet.
    pub fn is_sketching(&self, holding: crate::ui::Held) -> bool {
        self.open && self.sketching && holding == crate::ui::Held::Desk && matches!(self.step, Step::Writing)
    }

    /// Whether Claude, the compiler or the preview is at work.
    pub fn busy(&self) -> bool {
        matches!(self.step, Step::Starting(..) | Step::Waiting(_) | Step::Building | Step::Picturing(_))
    }

    /// The step it is at, of how many, and what it is doing, while it is busy.
    pub fn step(&self) -> Option<(u8, u8, &'static str)> {
        let pass = self.run.looks.saturating_sub(self.looks_left) > 0;
        let doing = match &self.step {
            Step::Starting(Kind::Begin | Kind::Sketch | Kind::Change, ..) => "Getting ready",
            Step::Waiting(Kind::Begin | Kind::Sketch) => "Writing the theme",
            Step::Waiting(Kind::Change) => "Making your change",
            Step::Starting(Kind::Frames, ..) | Step::Waiting(Kind::Frames) => "Looking it over, to make it lovelier",
            Step::Starting(Kind::Errors | Kind::Checks, ..) | Step::Waiting(Kind::Errors | Kind::Checks) => "Putting a mistake right",
            Step::Building if pass => "Building the better version",
            Step::Building => "Building it",
            Step::Picturing(_) if pass => "Trying the better version",
            Step::Picturing(_) => "Trying it out",
            _ => return None,
        };
        Some((self.run.shown, self.run.steps, doing))
    }

    /// The step the work is at now, by what it is doing; a mistake being put right is at the step
    /// it was found in.
    fn step_now(&self) -> u8 {
        let pass = self.run.looks.saturating_sub(self.looks_left);
        let at = |phase: u8| phase + 3 * pass;
        match &self.step {
            Step::Starting(Kind::Begin | Kind::Sketch | Kind::Change, ..) => 1,
            Step::Starting(Kind::Frames, ..) | Step::Waiting(Kind::Begin | Kind::Sketch | Kind::Change | Kind::Frames) => at(2),
            Step::Building => at(3),
            Step::Picturing(_) => at(4),
            _ => self.run.shown,
        }
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
/// little-endian), then the PNGs, then a trailing UTF-8 payload (what Caprice's checks found for
/// `frames`, or the sketch's JSON for `sketch`).
fn pictures_body(pngs: &[Vec<u8>], tail: &[u8]) -> Vec<u8> {
    let mut body = vec![pngs.len() as u8];
    for p in pngs {
        body.extend_from_slice(&(p.len() as u32).to_le_bytes());
    }
    for p in pngs {
        body.extend_from_slice(p);
    }
    body.extend_from_slice(tail);
    body
}

impl App {
    /// Open the window, as it was left.
    pub fn make_theme(&mut self) {
        self.making.open = true;
    }

    /// Begin a theme from the writer's request: the compiler first, then Claude. Every theme is
    /// given the same room for now: the server's rich tier, and its weighing of a request to suggest
    /// it (`/api/theme-estimate`), are not offered (the owner's call, 2026-11-20).
    fn begin_theme(&mut self, ctx: &egui::Context) {
        let request = cut(self.making.request.trim(), MOST_REQUEST).to_owned();
        let m = &mut self.making;
        m.theme = None;
        m.good = None;
        m.note = None;
        m.fixes_left = FIXES;
        m.looks_left = LOOKS;
        self.start_run(ctx);
        self.round(ctx, Kind::Begin, request.into_bytes(), Vec::new());
    }

    /// Begin a theme from the hand-drawn sketch (plus optional words): PNGs and strokes go to
    /// `/api/theme-program` with `X-Caprice-Kind: sketch`.
    fn begin_sketch(&mut self, ctx: &egui::Context) {
        let words = cut(self.making.request.trim(), MOST_REQUEST).to_owned();
        let Some((pngs, json)) = self.making.sketch.export(&words) else { return };
        let body = pictures_body(&pngs, json.as_bytes());
        let m = &mut self.making;
        m.theme = None;
        m.good = None;
        m.note = None;
        m.fixes_left = FIXES;
        m.looks_left = LOOKS;
        self.start_run(ctx);
        self.round(ctx, Kind::Sketch, body, Vec::new());
    }

    /// The steps counted from here: a request, or a change asked for.
    fn start_run(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let looks = self.making.looks_left;
        self.making.run = Run { began: now, heard: now, steps: 4 + 3 * looks, looks, shown: 1, since: now };
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
            (Some(id), k) if !matches!(k, Kind::Begin | Kind::Sketch) => {
                headers.push(("X-Caprice-Kind", kind.name().into()));
                format!("/api/theme-program/{id}")
            }
            _ => {
                if kind == Kind::Sketch {
                    headers.push(("X-Caprice-Kind", kind.name().into()));
                }
                "/api/theme-program".into()
            }
        };
        if !matches!(kind, Kind::Frames | Kind::Sketch) {
            headers.push(("Content-Type", "text/plain; charset=utf-8".into()));
        }
        let (tx, rx) = channel();
        let (told, heard) = channel();
        server.send_long(Request { method: "POST", path, headers, body: Some(body) }, tx, told, Waker::new(ctx));
        self.making.answer = Some(rx);
        self.making.heard = Some(heard);
        self.making.run.heard = ctx.input(|i| i.time);
        self.making.step = Step::Waiting(kind);
    }

    /// What went wrong goes back to Claude to fix, while it may be (the compiler's words as
    /// `errors`, what Caprice found of a program that built as `checks`, both counted as fixes);
    /// else the theme stops at the last version that worked, or, with none, comes to nothing.
    fn fix(&mut self, ctx: &egui::Context, kind: Kind, said: &str) {
        if self.making.fixes_left > 0 {
            self.making.fixes_left -= 1;
            self.round(ctx, kind, cut(said, MOST_ERRORS).as_bytes().to_vec(), Vec::new());
        } else {
            self.give_up(NEVER_WORKED.into());
        }
    }

    /// Stop where it is: at the last version that worked, saying why, or with nothing.
    fn give_up(&mut self, why: String) {
        let m = &mut self.making;
        m.compiler = None;
        m.trying = None;
        m.answer = None;
        m.heard = None;
        if m.good.is_some() {
            m.note = Some(format!("{why} This is the version before."));
            m.step = Step::Ready;
        } else {
            m.step = Step::Failed(why);
        }
    }

    /// Whether this device has a network, as far as Caprice can tell.
    fn online(&self) -> bool {
        self.cloud.server.as_ref().is_none_or(|s| s.online())
    }

    /// Once a frame: the round answered, the version built, played, shown.
    pub fn tend_making(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let at = self.making.step_now().min(self.making.run.steps.max(1));
        if at > self.making.run.shown {
            (self.making.run.shown, self.making.run.since) = (at, now);
        }
        match &mut self.making.step {
            Step::Starting(..) => match self.making.compiler.as_mut().map(|c| c.state(now)) {
                Some(State::Failed { why, unsupported }) => {
                    let why = if unsupported {
                        why
                    } else if !self.online() {
                        LOST_NETWORK.into()
                    } else {
                        format!("The theme could not be built in this browser ({why}). Try again.")
                    };
                    self.give_up(why)
                }
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
                if self.making.heard.as_ref().is_some_and(|h| h.try_iter().count() > 0) {
                    self.making.run.heard = now;
                }
                let Some(answer) = self.making.answer.as_ref().and_then(|rx| rx.try_recv().ok()) else {
                    // A minute without word: the connection is gone, and the answer with it.
                    if now - self.making.run.heard > LOST {
                        let lost = if self.online() { LOST_SERVER } else { LOST_NETWORK };
                        self.round_lost(kind, lost.into());
                    }
                    return;
                };
                self.making.answer = None;
                self.making.heard = None;
                self.round_answered(kind, answer);
            }
            Step::Building => self.tend_making_build(ctx, now),
            Step::Picturing(preview) => match preview.tend(now) {
                Some(Ok(previewed)) => self.pictured(ctx, previewed),
                Some(Err(why)) => self.fix(ctx, Kind::Checks, &format!("Caprice stopped it while playing the story: {why}.")),
                None => ctx.request_repaint_after(std::time::Duration::from_millis(50)),
            },
            Step::Writing | Step::Ready | Step::Failed(_) => {}
        }
    }

    /// Claude's answer to a round of `kind`: a version to build, or why there is none.
    fn round_answered(&mut self, kind: Kind, answer: Answer) {
        let said = String::from_utf8_lossy(&answer.body).trim().to_owned();
        let words = |said: &str| format!("{}.", said.trim_end_matches('.'));
        // Gone wrong after the server had begun answering: its status is in the body.
        let refused = serde_json::from_slice::<Refused>(&answer.body).ok().filter(|_| answer.status == 200);
        let (status, said) = refused.map_or((answer.status, said), |r| (r.error, r.message));
        let why = match status {
            200 => match serde_json::from_slice::<Written>(&answer.body) {
                Ok(w) => {
                    self.making.theme = Some(w.theme);
                    self.making.may_look = w.left.get("frames").and_then(serde_json::Value::as_u64) != Some(0);
                    if !self.making.may_look {
                        let m = &mut self.making;
                        let unseen = m.looks_left;
                        m.run.looks -= unseen;
                        m.run.steps -= 3 * unseen;
                        m.looks_left = 0;
                    }
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
            0 if !self.online() => LOST_NETWORK.into(),
            0 => LOST_SERVER.into(),
            401 => "You are signed out: sign in again (File \u{203a} Stories online), then try again.".into(),
            _ if !said.is_empty() && said.len() < 200 => words(&said),
            code => format!("Caprice's server could not make it ({code})."),
        };
        self.round_lost(kind, why);
    }

    /// A round of `kind` that came to nothing, for `why`.
    fn round_lost(&mut self, kind: Kind, why: String) {
        self.making.answer = None;
        self.making.heard = None;
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
                Err(why) => self.fix(ctx, Kind::Checks, &format!("Caprice will not play its program: {}.", why.join("; "))),
            },
            (Some(Built::Errors(said)), _) => self.fix(ctx, Kind::Errors, &said),
            (None, State::Failed { why, .. }) => self.give_up(why),
            (None, _) => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
        }
    }

    /// A version that played: kept as the one to show, and shown to Claude while it may look.
    fn pictured(&mut self, ctx: &egui::Context, previewed: Previewed) {
        let Some(version) = self.making.trying.take() else { return };
        let (phone, computer) = (previewed.phone_points, previewed.computer_points);
        let within = |phone: usize, computer: usize| phone <= BUDGET.0 && computer <= BUDGET.1;
        // One that draws more than the budget is a near miss: kept to fall back on only while
        // there is no version within it, and sent back to be made lighter while fixes are left.
        let keep = within(phone, computer) || !self.making.good.as_ref().is_some_and(|g| within(g.previewed.phone_points, g.previewed.computer_points));
        // Its sky short of the window's edges zoomed out, and what slides as the writer zooms.
        let looks: Vec<String> = [previewed.bare(), previewed.slides()].into_iter().flatten().collect();
        let pngs: Vec<Vec<u8>> = previewed.pictures.iter().filter_map(|p| crate::eggs::preview::png(&p.image)).collect();
        let moments = previewed.pictures.iter().map(|p| p.name).collect::<Vec<_>>().join(",");
        if keep {
            let textures = previewed.pictures.iter().map(|p| ctx.load_texture(format!("making-{}", p.name), p.image.clone(), TextureOptions::LINEAR)).collect();
            self.making.good = Some(Good { version, previewed, textures });
        }
        if !within(phone, computer) {
            if self.making.fixes_left > 0 {
                self.fix(ctx, Kind::Checks, &over_budget(phone, computer));
                return;
            }
            if !keep {
                self.give_up("The newest version draws more than a writer's desk can carry.".into());
                return;
            }
        }
        // What is wrong with how it looks zoomed goes with the pictures when Claude is still to see
        // them: one round, which mends what is wrong first and then makes it finished, rather than
        // a round of its own before the look. Without a look to come it is sent back while fixes
        // are left; then it is shown as it is (the picture zoomed out among the others), and kept
        // if the writer likes it.
        let look = self.making.looks_left > 0 && self.making.may_look && pngs.len() == moments.split(',').count();
        if !looks.is_empty() && !look && self.making.fixes_left > 0 {
            self.fix(ctx, Kind::Checks, &looks.join("\n\n"));
            return;
        }
        let points = format!("{phone},{computer}");
        if look {
            self.making.looks_left -= 1;
            let found = looks.join("\n\n");
            let body = pictures_body(&pngs, cut(&found, MOST_ERRORS).as_bytes());
            self.round(ctx, Kind::Frames, body, vec![("X-Caprice-Moments", moments), ("X-Caprice-Points", points)]);
        } else {
            self.making.compiler = None;
            self.making.step = Step::Ready;
        }
    }

    /// The version shown, put on the shelf and chosen.
    fn keep_made(&mut self, ctx: &egui::Context) {
        let Some(good) = self.making.good.take() else { return };
        // The server counts it as kept (what themes cost is measured by, and billing will hang on);
        // nothing waits for its answer.
        if let (Some(server), Some(id)) = (self.cloud.server.clone(), &self.making.theme) {
            let (tx, _) = channel::<Answer>();
            server.send(Request { method: "POST", path: format!("/api/theme-program/{id}/kept"), headers: Vec::new(), body: None }, tx, Waker::new(ctx));
        }
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

    /// The "Make a theme" window.
    pub fn making_window(&mut self, ctx: &egui::Context) {
        if !self.making.open {
            return;
        }
        let now = ctx.input(|i| i.time);
        let online = self.online();
        let signed_in = self.cloud.who.is_some();
        let mut open = true;
        let mut act = None;
        let m = &mut self.making;
        let loading = match m.compiler.as_mut().map(|c| c.state(ctx.input(|i| i.time))) {
            Some(State::Loading { fetched, of }) if of > 0 => {
                Some(format!("Loading what builds themes: {} of {} MB, only this once", fetched.div_ceil(1 << 20), of.div_ceil(1 << 20)))
            }
            _ => None,
        };
        let desktop = self.holding == crate::ui::Held::Desk;
        let sketching = m.is_sketching(self.holding);
        let sheet = if sketching {
            let content = ctx.content_rect();
            let avail_w = (content.width() - 48.0).clamp(360.0, 960.0);
            let avail_h = (content.height() - 230.0).max(225.0);
            let aspect = crate::sketch::W as f32 / crate::sketch::H as f32;
            let w = avail_w.min(avail_h * aspect);
            vec2(w, w / aspect)
        } else {
            vec2(0.0, 0.0)
        };
        let win_w = if sketching { sheet.x + 16.0 } else { 480.0 };
        let max_w = if sketching { sheet.x + 16.0 } else { 500.0 };
        let mut closed = false;
        egui::Window::new("Make a theme").id(egui::Id::new("theme_making")).open(&mut open).collapsible(false).resizable(false).default_width(win_w).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            closed = crate::ui::finger_close(ui, egui::Id::new("theme_making"));
            ui.set_max_width(max_w);
            match &m.step {
                Step::Writing if sketching => {
                    m.sketch.show(ui, sheet);
                    ui.add_space(6.0);
                    ui.add(egui::TextEdit::singleline(&mut m.request).char_limit(MOST_REQUEST).desired_width(sheet.x)
                        .hint_text("What is it? (optional)  e.g. A lighthouse on a cliff, a window lit for each page"));
                    ui.label(egui::RichText::new(SKETCH_GOES_OUT).color(pal().text_dim).size(12.0));
                    ui.label(egui::RichText::new(FREE).color(pal().text_dim).size(12.0));
                    if !signed_in {
                        ui.label(egui::RichText::new("Sign in first: themes are made for writers signed in.").color(pal().accent));
                    }
                    ui.horizontal(|ui| {
                        if signed_in {
                            if ui.add_enabled(!m.sketch.is_empty(), egui::Button::new("Make it")).clicked() {
                                act = Some(Act::BeginSketch);
                            }
                        } else if ui.button("Sign in\u{2026}").clicked() {
                            act = Some(Act::SignIn);
                        }
                        if ui.button("Describe in words").clicked() {
                            m.sketching = false;
                        }
                    });
                }
                Step::Writing => {
                    ui.label("Say what you would like on the desk beside your pages: a thing for each chapter, growing a part with each page you write.");
                    ui.add(egui::TextEdit::multiline(&mut m.request).char_limit(MOST_REQUEST).desired_rows(3).desired_width(f32::INFINITY)
                        .hint_text("A lighthouse on a cliff by the sea, a window lit for each page, a boat for each chapter"));
                    ui.label(egui::RichText::new(GOES_OUT).color(pal().text_dim).size(12.0));
                    ui.label(egui::RichText::new(FREE).color(pal().text_dim).size(12.0));
                    if !signed_in {
                        ui.label(egui::RichText::new("Sign in first: themes are made for writers signed in.").color(pal().accent));
                    }
                    ui.horizontal(|ui| {
                        if signed_in {
                            if ui.add_enabled(!m.request.trim().is_empty(), egui::Button::new("Make it")).clicked() {
                                act = Some(Act::Begin);
                            }
                        } else if ui.button("Sign in\u{2026}").clicked() {
                            act = Some(Act::SignIn);
                        }
                        if desktop && ui.button("Draw a sketch\u{2026}").on_hover_text("Draw the desk by hand: four inks say when each part moves").clicked() {
                            m.sketching = true;
                        }
                    });
                }
                Step::Starting(..) | Step::Waiting(_) | Step::Building | Step::Picturing(_) => {
                    ui.label(format!("We prepare the theme (up to {UP_TO_MINUTES} minutes) and show it once it is done. Go on writing meanwhile: closing this window does not stop it."));
                    ui.add_space(4.0);
                    if let Some((at, of, doing)) = m.step() {
                        let r = m.run;
                        // Within a round of writing the bar moves by the time it usually takes,
                        // never past most of its step: the steps between are seconds.
                        let within = if matches!(m.step, Step::Waiting(_)) { ((now - r.since) / TYPICAL_ROUND).min(0.9) } else { 0.0 };
                        let done = (f64::from(at.saturating_sub(1)) + within) / f64::from(of.max(1));
                        ui.add(egui::ProgressBar::new(done as f32).text(format!("Step {at} of {of}")));
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!("{doing}\u{2026}"));
                            ui.label(egui::RichText::new(took(now - r.began)).color(pal().text_dim));
                        });
                        let quiet = now - r.heard;
                        if !online {
                            ui.label(egui::RichText::new("Lost network. Waiting for it to come back\u{2026}").color(pal().accent));
                        } else if matches!(m.step, Step::Waiting(_)) && quiet > QUIET {
                            ui.label(egui::RichText::new(format!("No word from Caprice's server for {}: the connection may be lost.", took(quiet))).color(pal().accent));
                        } else if now - r.began > f64::from(UP_TO_MINUTES) * 60.0 {
                            ui.label(egui::RichText::new("This is taking longer than usual, but it is still going.").color(pal().text_dim));
                        }
                    }
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
                    ui.label(egui::RichText::new(FREE).color(pal().text_dim).size(12.0));
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::multiline(&mut m.change).char_limit(MOST_REQUEST).desired_rows(2).desired_width(f32::INFINITY).hint_text("Or say what to change"));
                    if ui.add_enabled(!m.change.trim().is_empty(), egui::Button::new("Change it")).clicked() {
                        act = Some(Act::Change);
                    }
                }
                Step::Failed(why) => {
                    ui.label(egui::RichText::new(why).color(pal().accent));
                    if ui.button("Start again").clicked() {
                        act = Some(Act::Again);
                    }
                }
            }
        });
        open &= !closed;
        self.making.open &= open;
        // The clock and the bar move on while it works.
        if self.making.busy() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        match act {
            Some(Act::Begin) => self.begin_theme(ctx),
            Some(Act::BeginSketch) => self.begin_sketch(ctx),
            Some(Act::SignIn) => self.cloud.open = true,
            Some(Act::Keep) => self.keep_made(ctx),
            Some(Act::Again) => {
                let request = std::mem::take(&mut self.making.request);
                let sketch = std::mem::take(&mut self.making.sketch);
                let sketching = self.making.sketching;
                self.making = Making { open: true, request, sketch, sketching, ..Making::default() };
            }
            Some(Act::Change) => {
                let said = cut(self.making.change.trim(), MOST_REQUEST).as_bytes().to_vec();
                self.making.change.clear();
                self.making.note = None;
                self.start_run(ctx);
                self.round(ctx, Kind::Change, said, Vec::new());
            }
            None => {}
        }
    }
}

/// What the writer did in the window.
enum Act {
    Begin,
    BeginSketch,
    SignIn,
    Keep,
    Again,
    Change,
}

/// How long something has taken, as the writer reads it: "40 s", "3 min 12 s".
fn took(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s < 60 { format!("{s} s") } else { format!("{} min {:02} s", s / 60, s % 60) }
}

/// A version's pictures: three chapters written, large, and the others under it, small, in one row
/// as wide as the large one (five of them with the carrier's and the one without the page).
fn pictures(ui: &mut egui::Ui, g: &Good) {
    let at = g.previewed.pictures.iter().position(|p| p.name == "written").unwrap_or(0);
    if let Some(t) = g.textures.get(at) {
        ui.add(egui::Image::new(t).fit_to_exact_size(vec2(480.0, 300.0)));
    }
    let others = g.textures.len().saturating_sub(1).max(1) as f32;
    let gap = ui.spacing().item_spacing.x;
    let w = ((480.0 - gap * (others - 1.0)) / others).min(152.0);
    ui.horizontal(|ui| {
        for (k, t) in g.textures.iter().enumerate() {
            if k != at {
                ui.add(egui::Image::new(t).fit_to_exact_size(vec2(w, w * 0.625)));
            }
        }
    });
}
