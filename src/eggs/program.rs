//! A theme that is a program (R36): one WebAssembly module, written against `theme-sdk/` and the
//! WIT world `caprice:theme@1.0.0`, that draws the desk as the magic themes do. Caprice checks it
//! on every load (`check`: what it imports and exports, its memory, and that it uses nothing of
//! WebAssembly beyond a fixed set), runs it apart from the app (`platform::Program`: a worker of
//! its own in the browser), tells it what happened (the story shown, a chapter grown or begun),
//! and each frame asks it for shapes. It draws the newest it has, never waiting, so a program's
//! shapes are a frame behind; zoomed here, all at once, as the Magic Snow and the Vineyard are
//! (the program lays out as at the fitting zoom), about the ground it names; what it puts in the
//! window (as snow falling) is never zoomed. Its carrier brings new chapter titles, flown by
//! Caprice's course (`pose_of`), drawn and offset by the program.
//!
//! Like the other magic themes it lies on the desk behind every page, and its carrier over the
//! settled page only: nothing of it is printed or saved, and nothing of it moves the text (R30).
//! Program themes play in the browser only: on the desktop the usual desk shows.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use super::{Carrier, Pose, pose_of};
use crate::App;
use crate::platform::program::{Ask, HEAD, HOST, INPUT_LEN, Program};
use crate::theme::Theme;

/// The largest module, and the most memory it may declare (in pages of 64 KiB: 32 MiB).
pub const MOST_BYTES: usize = 1 << 20;
pub const MOST_PAGES: u64 = 512;
/// The most points a frame may draw (a polygon's points and a mesh's vertices; a circle or a line
/// is one): twice the Magic Snow's, which Caprice's own frame tessellates in about 7 ms at a
/// quarter of this machine's speed. A frame over it is not drawn; `OVER_FRAMES` running stop it.
pub const MOST_POINTS: usize = 40_000;
pub const OVER_FRAMES: usize = 3;

/// What a theme may import (from `HOST`) and export, as the canonical ABI flattens the world's
/// functions: their parameters and results.
use wasmparser::ValType::{I32, I64};
const IMPORTS: [(&str, &[wasmparser::ValType], &[wasmparser::ValType]); 4] =
    [("story-seed", &[], &[I64]), ("garden", &[I32], &[]), ("page-count", &[], &[I32]), ("ask-lift", &[], &[I32])];
const EXPORTS: [(&str, &[wasmparser::ValType], &[wasmparser::ValType]); 4] =
    [("events", &[I32, I32], &[]), ("frame", &[I32], &[I32]), ("cabi_post_frame", &[I32], &[]), ("cabi_realloc", &[I32, I32, I32, I32], &[I32])];

/// The WebAssembly a theme may use, for good: what every engine runs (WebAssembly 2.0 without
/// SIMD, and what `wasm32-unknown-unknown` turns on by default). A toolchain that turns on more
/// makes a new version of the world, never a change to this one.
fn features() -> wasmparser::WasmFeatures {
    use wasmparser::WasmFeatures as F;
    F::WASM2.difference(F::SIMD)
}

/// Whether `module` may be run as a theme, and if not, each reason why, in words.
pub fn check(module: &[u8]) -> Result<(), Vec<String>> {
    use wasmparser::{CompositeInnerType, ExternalKind, Parser, Payload, TypeRef, Validator};
    if module.len() > MOST_BYTES {
        return Err(vec![format!("it is {} KB, more than the {} KB a theme may be", module.len() / 1024, MOST_BYTES / 1024)]);
    }
    if let Err(e) = Validator::new_with_features(features()).validate_all(module) {
        return Err(vec![format!("it is not WebAssembly that every browser runs: {}", e.message())]);
    }
    let mut why = Vec::new();
    // Each function's type, imported ones first.
    let (mut types, mut funcs, mut exported, mut memories) = (Vec::new(), Vec::new(), Vec::new(), 0);
    for payload in Parser::new(0).parse_all(module) {
        let Ok(payload) = payload else { return Err(vec!["it could not be read".into()]) };
        match payload {
            Payload::TypeSection(r) => {
                for group in r.into_iter().flatten() {
                    for ty in group.into_types() {
                        types.push(match ty.composite_type.inner {
                            CompositeInnerType::Func(f) => Some((f.params().to_vec(), f.results().to_vec())),
                            _ => None,
                        });
                    }
                }
            }
            Payload::ImportSection(r) => {
                for import in r.into_iter().flatten() {
                    let wanted = IMPORTS.iter().find(|i| import.module == HOST && i.0 == import.name);
                    match (import.ty, wanted) {
                        (TypeRef::Func(t), Some((name, params, results))) => {
                            if types.get(t as usize).cloned().flatten() != Some((params.to_vec(), results.to_vec())) {
                                why.push(format!("it asks Caprice for {name} in another way than Caprice answers"));
                            }
                            funcs.push(t);
                        }
                        (TypeRef::Func(t), None) => {
                            why.push(format!("it asks for {} of {}, which Caprice does not give", import.name, import.module));
                            funcs.push(t);
                        }
                        _ => why.push(format!("it asks for {} of {}, which is not a function Caprice gives", import.name, import.module)),
                    }
                }
            }
            Payload::FunctionSection(r) => funcs.extend(r.into_iter().flatten()),
            Payload::MemorySection(r) => {
                for m in r.into_iter().flatten() {
                    memories += 1;
                    match m.maximum {
                        _ if m.shared || m.memory64 => why.push("its memory is shared or 64-bit".into()),
                        None => why.push("it does not say how much memory it may take".into()),
                        Some(most) if most > MOST_PAGES => why.push(format!("it may take {} MB of memory, more than the {} MB a theme may", most / 16, MOST_PAGES / 16)),
                        _ => {}
                    }
                }
            }
            Payload::ExportSection(r) => {
                for export in r.into_iter().flatten() {
                    exported.push(export.name.to_string());
                    let wanted = EXPORTS.iter().find(|e| e.0 == export.name);
                    match (export.kind, wanted) {
                        (ExternalKind::Func, Some((name, params, results))) => {
                            let t = funcs.get(export.index as usize).and_then(|&t| types.get(t as usize).cloned().flatten());
                            if t != Some((params.to_vec(), results.to_vec())) {
                                why.push(format!("its {name} is not what Caprice calls"));
                            }
                        }
                        (ExternalKind::Memory, _) if export.name == "memory" => {}
                        _ => why.push(format!("it gives {}, which Caprice does not use", export.name)),
                    }
                }
            }
            _ => {}
        }
    }
    if memories != 1 {
        why.push("it does not have a memory of its own".into());
    }
    for name in EXPORTS.iter().map(|e| e.0).chain(["memory"]) {
        if !exported.iter().any(|e| e == name) {
            why.push(format!("it does not give {name}, which Caprice calls"));
        }
    }
    if why.is_empty() { Ok(()) } else { Err(why) }
}

/// What Caprice tells a program before a frame (the world's `event`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Fresh,
    Grown { chapter: u32, pages: u32 },
    Begun(u32),
    Clicked(Pos2),
}

/// The events as the canonical ABI lays out `list<event>`'s elements: 12 bytes each, the case in
/// the first, its payload from the fifth.
pub fn event_bytes(events: &[Event]) -> Vec<u8> {
    let mut b = vec![0u8; events.len() * 12];
    for (e, at) in events.iter().zip(b.chunks_mut(12)) {
        let (case, x, y) = match *e {
            Event::Fresh => (0, 0, 0),
            Event::Grown { chapter, pages } => (1, chapter, pages),
            Event::Begun(k) => (2, k, 0),
            Event::Clicked(p) => (3, p.x.to_bits(), p.y.to_bits()),
        };
        at[0] = case;
        at[4..8].copy_from_slice(&x.to_le_bytes());
        at[8..12].copy_from_slice(&y.to_le_bytes());
    }
    b
}

/// The carrier at work, as a program is told of it: its pose, whether it came from the left, and
/// the title's place.
#[derive(Clone, Copy, Debug)]
pub struct Carried {
    pub pose: Pose,
    pub from_left: bool,
    pub title: Rect,
}

/// The desk as a program is told of it (the world's `desk`): as much of it as shows, the page and
/// the bar, laid out as at the fitting zoom; the zoom and the size of things; the desk on the
/// screen; and the point it is zoomed about.
#[derive(Clone, Copy, Debug)]
pub struct Desk {
    pub area: Rect,
    pub page: Rect,
    pub bar: Option<Rect>,
    pub zoom: f32,
    pub unit: f32,
    pub window: Rect,
    pub ground: Pos2,
}

/// The frame's input as the canonical ABI lays out `frame-input` (144 bytes): the time, the desk
/// (its area, the page, the bar if it shows, the zoom, the size of things, the desk on the screen
/// and the ground), and the carrier.
pub fn input_bytes(time: f64, desk: &Desk, carried: Option<Carried>) -> [u8; INPUT_LEN] {
    let Desk { area, page, bar, zoom, unit, window, ground } = *desk;
    let mut b = [0u8; INPUT_LEN];
    let mut put = |at: usize, v: f32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    let rect = |put: &mut dyn FnMut(usize, f32), at: usize, r: Rect| {
        for (i, v) in [r.min.x, r.min.y, r.max.x, r.max.y].into_iter().enumerate() {
            put(at + 4 * i, v);
        }
    };
    rect(&mut put, 8, area);
    rect(&mut put, 24, page);
    if let Some(bar) = bar {
        rect(&mut put, 44, bar);
    }
    put(60, zoom);
    put(64, unit);
    rect(&mut put, 68, window);
    put(84, ground.x);
    put(88, ground.y);
    if let Some(c) = carried {
        let p = c.pose;
        for (at, v) in [(96, p.enter), (100, p.run), (104, p.lower), (112, p.hook), (116, p.look), (120, p.t)] {
            put(at, v);
        }
        rect(&mut put, 128, c.title);
    }
    b[0..8].copy_from_slice(&time.to_le_bytes());
    b[40] = u8::from(bar.is_some());
    if let Some(c) = carried {
        (b[92], b[108], b[124]) = (1, u8::from(c.pose.hooked), u8::from(c.from_left));
    }
    b
}

/// A story's seed for its themes: a hash of its id, never the id, the same in every browser.
pub fn story_seed(story: &str) -> u64 {
    // FNV-1a: fixed for good, unlike the standard library's hasher.
    story.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A frame's words read: its shapes as egui's (on the desk, in the window, over the page), how
/// many points, when it wants the next frame, the carried title's offset, whether it asked to carry
/// titles, and how high its ground is above the foot of the desk, in units.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub desk: Vec<Shape>,
    pub window: Vec<Shape>,
    pub over: Vec<Shape>,
    pub next: u32,
    pub offset: Vec2,
    pub asked: bool,
    pub ground: f32,
    pub points: usize,
}

/// Why a frame's words could not be drawn.
#[derive(Debug, PartialEq)]
pub enum Unread {
    /// More points than a frame may have.
    Over(usize),
    Bad(&'static str),
}

/// The words a program's host made (`platform::program::Words`) read into egui's shapes, each
/// checked: nothing that is not a finite number is drawn, nor a mesh with indices beyond it. Over
/// `MOST_POINTS` it stops reading.
pub fn read(w: &[u32]) -> Result<Drawn, Unread> {
    let get = |i: usize| w.get(i).copied().ok_or(Unread::Bad("its output is cut short"));
    let f = |i: usize| get(i).map(f32::from_bits);
    let color = |c: u32| {
        let [r, g, b, a] = c.to_le_bytes();
        Color32::from_rgba_premultiplied(r, g, b, a)
    };
    let fine = |v: &[f32]| v.iter().all(|x| x.is_finite());
    let (desk_n, window_n, over_n) = (get(0)? as usize, get(1)? as usize, get(2)? as usize);
    let mut d = Drawn { next: get(3)?, offset: vec2(f(4)?, f(5)?), asked: get(6)? == 1, ground: f(7)?, ..Drawn::default() };
    if !fine(&[d.offset.x, d.offset.y]) {
        d.offset = Vec2::ZERO;
    }
    if !d.ground.is_finite() {
        d.ground = 0.0;
    }
    let mut i = HEAD;
    for k in 0..desk_n + window_n + over_n {
        let kind = get(i)?;
        let shape = match kind {
            1 => {
                let (c, x, y, r) = (get(i + 1)?, f(i + 2)?, f(i + 3)?, f(i + 4)?);
                (i, d.points) = (i + 5, d.points + 1);
                fine(&[x, y, r]).then(|| Shape::circle_filled(pos2(x, y), r.max(0.0), color(c)))
            }
            2 => {
                let (c, n) = (get(i + 1)?, get(i + 2)? as usize);
                if w.len() < i + 3 + 2 * n {
                    return Err(Unread::Bad("its output is cut short"));
                }
                let points: Vec<Pos2> = (0..n).map(|j| pos2(f32::from_bits(w[i + 3 + 2 * j]), f32::from_bits(w[i + 4 + 2 * j]))).collect();
                (i, d.points) = (i + 3 + 2 * n, d.points + n);
                (n >= 3 && points.iter().all(|p| p.is_finite())).then(|| Shape::convex_polygon(points, color(c), Stroke::NONE))
            }
            3 => {
                let (c, width, ax, ay, bx, by) = (get(i + 1)?, f(i + 2)?, f(i + 3)?, f(i + 4)?, f(i + 5)?, f(i + 6)?);
                (i, d.points) = (i + 7, d.points + 1);
                fine(&[width, ax, ay, bx, by]).then(|| Shape::line_segment([pos2(ax, ay), pos2(bx, by)], Stroke::new(width.max(0.0), color(c))))
            }
            4 => {
                let (nv, ni) = (get(i + 1)? as usize, get(i + 2)? as usize);
                let end = i + 3 + 3 * nv + ni;
                if w.len() < end {
                    return Err(Unread::Bad("its output is cut short"));
                }
                let mut mesh = egui::epaint::Mesh::default();
                for v in w[i + 3..i + 3 + 3 * nv].chunks(3) {
                    mesh.colored_vertex(pos2(f32::from_bits(v[0]), f32::from_bits(v[1])), color(v[2]));
                }
                mesh.indices.extend_from_slice(&w[i + 3 + 3 * nv..end]);
                (i, d.points) = (end, d.points + nv);
                let whole = ni % 3 == 0 && mesh.indices.iter().all(|&j| (j as usize) < nv) && mesh.vertices.iter().all(|v| v.pos.is_finite());
                whole.then(|| Shape::mesh(mesh))
            }
            5 => {
                let (c, width, closed, n) = (get(i + 1)?, f(i + 2)?, get(i + 3)? == 1, get(i + 4)? as usize);
                if w.len() < i + 5 + 2 * n {
                    return Err(Unread::Bad("its output is cut short"));
                }
                let points: Vec<Pos2> = (0..n).map(|j| pos2(f32::from_bits(w[i + 5 + 2 * j]), f32::from_bits(w[i + 6 + 2 * j]))).collect();
                (i, d.points) = (i + 5 + 2 * n, d.points + n);
                let stroke = Stroke::new(width.max(0.0), color(c));
                let drawn = n >= 2 && width.is_finite() && points.iter().all(|p| p.is_finite());
                drawn.then(|| if closed { Shape::closed_line(points, stroke) } else { Shape::line(points, stroke) })
            }
            _ => return Err(Unread::Bad("a shape of no kind")),
        };
        if d.points > MOST_POINTS {
            return Err(Unread::Over(d.points));
        }
        if let Some(s) = shape {
            match k {
                _ if k < desk_n => d.desk.push(s),
                _ if k < desk_n + window_n => d.window.push(s),
                _ => d.over.push(s),
            }
        }
    }
    Ok(d)
}

/// What a program theme drew last frame, for tests: the theme, how many shapes on the desk, in the
/// window and over the page, how many points, how many frames it has answered, and why it was
/// stopped, if it was.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    pub id: String,
    pub desk: usize,
    pub window: usize,
    pub over: usize,
    pub points: usize,
    pub frames: usize,
    pub stopped: Option<String>,
}

/// The program theme shown: its program, what it was last told, its newest frame.
#[derive(Default)]
pub struct Shown {
    /// The theme loaded, and its program.
    id: Option<String>,
    program: Option<Program>,
    /// The story it was last shown for, and that story's garden as it was last told.
    story: Option<String>,
    garden: Vec<u32>,
    /// What is still to be told, with the next frame asked for.
    events: Vec<Event>,
    drawn: Drawn,
    frames: usize,
    over: usize,
    /// Why it is stopped, once known; said in the status line once.
    stopped: Option<String>,
    said: bool,
    /// The zoom its shapes are drawn at, and about where.
    zoom: egui::emath::TSTransform,
    /// For tests: what it was drawn last frame, and everything it was told, in order.
    pub seen: Option<Seen>,
    #[cfg(test)]
    pub told: Vec<Event>,
}

impl Shown {
    /// Nothing kept for another theme: its program ended, and picked again it starts afresh.
    pub fn forget(&mut self) {
        if self.id.is_some() {
            *self = Shown::default();
        }
    }

    /// Told to show the story as it stands (`Eggs::regrown`).
    pub fn regrown(&mut self) {
        if self.program.is_some() {
            self.events.push(Event::Fresh);
        }
    }

    /// Its newest frame, as read.
    #[cfg(test)]
    pub fn drawn(&self) -> &Drawn {
        &self.drawn
    }

    /// Whether it brings new chapter titles: it asked to, and is not stopped.
    pub fn carries(&self) -> bool {
        self.drawn.asked && self.stopped.is_none()
    }

    /// The carried title's offset from its place, as drawn.
    pub fn title_offset(&self) -> Vec2 {
        self.drawn.offset * self.zoom.scaling
    }

    /// Its shapes over the page, for the carrier.
    pub fn over(&self) -> Vec<Shape> {
        self.drawn.over.iter().cloned().map(|mut s| {
            s.transform(self.zoom);
            s
        }).collect()
    }

    /// What the garden's change since it was last told says happened: a chapter begun for each new
    /// one, then every chapter with more pages than before.
    fn tell(&mut self, garden: &[u32]) {
        for k in self.garden.len().max(1)..garden.len() {
            self.events.push(Event::Begun(k as u32));
        }
        for (k, &pages) in garden.iter().enumerate() {
            if pages > self.garden.get(k).copied().unwrap_or(0) {
                self.events.push(Event::Grown { chapter: k as u32, pages });
            }
        }
        self.garden = garden.to_vec();
    }
}

impl App {
    /// The writer's own theme `id` that is a program, on the desk behind the pages lying at `page`.
    pub fn draw_program(&mut self, ui: &egui::Ui, area: Rect, page: Rect, id: &str) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let p = &mut self.eggs.program;
        p.seen = None;
        if p.id.as_deref() != Some(id) {
            *p = Shown { id: Some(id.to_owned()), ..Shown::default() };
            match self.themes.own.module(id).ok_or_else(|| vec!["its program is missing".to_string()]).and_then(|m| check(&m).map(|_| m)) {
                Ok(module) => p.program = Some(Program::load(&module, crate::platform::Waker::new(&ctx), now)),
                Err(why) => p.stopped = Some(why.join("; ")),
            }
        }
        // A story shown afresh, or another, is told as it stands.
        if p.story.as_deref() != Some(self.story_id.as_str()) {
            (p.story, p.garden) = (Some(self.story_id.clone()), self.doc.garden.clone());
            p.events.push(Event::Fresh);
        }
        p.tell(&self.doc.garden.clone());
        let p = &mut self.eggs.program;
        if let Some(program) = &mut p.program {
            if let Some(why) = program.stopped(now) {
                p.stopped.get_or_insert(why);
            }
            if let Some(words) = program.take() {
                match read(&words) {
                    Ok(d) => {
                        (p.drawn, p.over, p.frames) = (d, 0, p.frames + 1);
                    }
                    Err(Unread::Over(points)) => {
                        p.over += 1;
                        if p.over >= OVER_FRAMES {
                            let why = format!("it drew {points} points a frame, more than the {MOST_POINTS} a theme may");
                            program.stop(why.clone());
                            p.stopped.get_or_insert(why);
                        }
                    }
                    Err(Unread::Bad(why)) => {
                        program.stop(why.to_string());
                        p.stopped.get_or_insert(why.to_string());
                    }
                }
            }
        }
        if let Some(why) = &self.eggs.program.stopped {
            // Stopped: the usual desk, and a word why, once.
            if !self.eggs.program.said {
                self.status = format!("- the theme was stopped: {why}");
                self.eggs.program.said = true;
            }
            self.eggs.program.program = None;
            self.eggs.program.drawn = Drawn::default();
            self.eggs.program.seen = Some(Seen { id: id.to_owned(), stopped: Some(why.clone()), ..Seen::default() });
            return;
        }

        // Laid out as at the fitting zoom, zoomed about the theme's ground under the page's middle:
        // as high above the foot of the desk (the bar's top, when the bar lies along it) as it last
        // said, so what stands there stays where it is on the screen.
        let z = (self.zoom / self.fitting_zoom(area)).clamp(0.5, 4.0);
        let unit = self.eggs.unit(&ctx, area, now);
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        let anchor = pos2(page.center().x, area.bottom() - hidden - self.eggs.program.drawn.ground * unit);
        let unzoom = |r: Rect| Rect::from_min_max(anchor + (r.min - anchor) / z, anchor + (r.max - anchor) / z);
        let bar = Some(unzoom(self.bar_rect(area, self.bar.side)));
        let carried = self.eggs.lift.filter(|l| matches!(l.by, Carrier::Program { .. })).zip(self.eggs.place).map(|(lift, place)| {
            let pose = pose_of(lift.by, (now - lift.started) as f32, lift.duck.map(|d| (d - lift.started) as f32));
            let Carrier::Program { right } = lift.by else { unreachable!() };
            Carried { pose, from_left: !right, title: unzoom(place) }
        });
        // A click on the desk, below the toolbar, beside the page and the bar, with no window over
        // it, is the theme's.
        let clicked = ctx.input(|i| i.pointer.primary_clicked().then(|| i.pointer.interact_pos()).flatten()).filter(|&at| {
            area.contains(at) && at.y > self.tools_bottom && !page.contains(at) && !self.bar_rect(area, self.bar.side).contains(at)
                && ctx.layer_id_at(at).is_none_or(|l| l.order == egui::Order::Background)
        });
        if let Some(at) = clicked {
            self.eggs.program.events.push(Event::Clicked(anchor + (at - anchor) / z));
        }
        let lift = self.eggs.odds.is_none_or(|o| o > 0.0);
        let p = &mut self.eggs.program;
        p.zoom = egui::emath::TSTransform::new(anchor.to_vec2() * (1.0 - z), z);
        let ask = Ask {
            input: input_bytes(now, &Desk { area: unzoom(area), page: unzoom(page), bar, zoom: z, unit, window: area, ground: anchor }, carried),
            events: event_bytes(&p.events),
            seed: story_seed(&self.story_id),
            garden: self.doc.garden.clone(),
            pages: self.doc.pages() as u32,
            lift,
        };
        if let Some(program) = &mut p.program
            && program.ask(&ask, now)
        {
            #[cfg(test)]
            p.told.extend_from_slice(&p.events);
            p.events.clear();
        }
        let zoom = p.zoom;
        let d = &p.drawn;
        p.seen = Some(Seen { id: id.to_owned(), desk: d.desk.len(), window: d.window.len(), over: d.over.len(), points: d.points, frames: p.frames, stopped: None });
        // The desk zoomed, then what is in the window as it is.
        let drawn: Vec<Shape> = d.desk.iter().cloned().map(|mut s| {
            s.transform(zoom);
            s
        }).chain(d.window.iter().cloned()).collect();
        let next = if p.frames == 0 { 16 } else { u64::from(p.drawn.next).clamp(16, 1000) };
        self.eggs.paint(ui.painter(), drawn);
        // Its answer wakes the frame when it comes (`platform::Waker`); meanwhile, as it asks.
        ctx.request_repaint_after(std::time::Duration::from_millis(next));
    }
}

/// Is the theme chosen one of the writer's own that is a program?
pub fn program_theme(app: &App) -> Option<String> {
    match (app.themes.theme, &app.themes.own_id) {
        (Theme::Own, Some(id)) if app.themes.own.get(id).is_some_and(|e| e.program) => Some(id.clone()),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use crate::platform::program::Words;

    const FIREFLIES: &[u8] = include_bytes!("../../tests/fixtures/fireflies.wasm");
    const ECHO: &[u8] = include_bytes!("../../tests/fixtures/echo.wasm");

    /// A module in the text format.
    fn wat(text: &str) -> Vec<u8> {
        wat::parse_str(text).unwrap()
    }

    /// The world's exports, written by hand: `frame` gives back what `out` holds at 1024.
    fn module(imports: &str, memory: &str, frame: &str) -> Vec<u8> {
        wat(&format!(r#"(module {imports}
            (memory (export "memory") {memory})
            (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 8192)
            (func (export "events") (param i32 i32))
            (func (export "frame") (param i32) (result i32) {frame} i32.const 1024)
            (func (export "cabi_post_frame") (param i32)))"#))
    }

    fn refused(module: &[u8]) -> String {
        check(module).expect_err("refused").join("; ")
    }

    #[test]
    fn the_sdk_s_themes_pass_the_check_as_they_are_built() {
        assert_eq!(check(FIREFLIES), Ok(()));
        assert_eq!(check(ECHO), Ok(()));
        assert_eq!(check(&module("", "1 512", "")), Ok(()), "the world written by hand");
    }

    #[test]
    fn a_module_that_asks_for_anything_but_caprice_s_host_is_refused_saying_why() {
        let why = refused(&module(r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#, "1 512", ""));
        assert!(why.contains("fd_write of wasi_snapshot_preview1, which Caprice does not give"), "{why}");
        let why = refused(&module(r#"(import "caprice:theme/host@1.0.0" "garden" (func (result i32)))"#, "1 512", ""));
        assert!(why.contains("garden in another way"), "{why}");
        let why = refused(&wat(r#"(module (import "env" "memory" (memory 1 512)))"#));
        assert!(why.contains("not a function") && why.contains("does not give frame"), "{why}");
        let why = refused(&wat(r#"(module (memory (export "memory") 1 512) (func (export "frame") (param i32 i32)) (func (export "grow")))"#));
        assert!(why.contains("its frame is not what Caprice calls") && why.contains("it gives grow") && why.contains("does not give events"), "{why}");
    }

    #[test]
    fn a_module_using_webassembly_beyond_the_fixed_set_is_refused() {
        let simd = refused(&module("", "1 512", "v128.const i32x4 0 0 0 0 drop"));
        assert!(simd.contains("every browser runs") && simd.contains("SIMD"), "{simd}");
        let shared = refused(&module("", "1 512 shared", ""));
        assert!(shared.contains("every browser runs"), "{shared}");
        let tail = refused(&wat(r#"(module (func $f return_call $f))"#));
        assert!(tail.contains("tail call"), "{tail}");
        assert!(refused(&vec![0; MOST_BYTES + 1]).contains("1024 KB a theme may be"));
    }

    #[test]
    fn a_module_that_may_take_too_much_memory_is_refused() {
        assert!(refused(&module("", "1", "")).contains("does not say how much memory"));
        assert!(refused(&module("", "1 1024", "")).contains("64 MB of memory, more than the 32 MB"));
    }

    /// Words with `points` points in one polygon behind the page.
    fn polygon_words(points: usize) -> Words {
        let mut w = vec![1, 0, 0, 0, 0, 0, 0, 0, 2, 0xff00_00ff, points as u32];
        w.extend((0..points).flat_map(|k| [(k as f32).to_bits(), 1f32.to_bits()]));
        w
    }

    #[test]
    fn a_frame_over_the_points_a_theme_may_draw_is_not_read() {
        assert_eq!(read(&polygon_words(MOST_POINTS)).unwrap().points, MOST_POINTS);
        assert_eq!(read(&polygon_words(MOST_POINTS + 1)).unwrap_err(), Unread::Over(MOST_POINTS + 1));
    }

    #[test]
    fn what_cannot_be_drawn_is_left_out_and_words_cut_short_are_refused() {
        let nan = f32::NAN.to_bits();
        // A circle at no place, a mesh with an index beyond it, a polygon of two points, a path of
        // one, a path as wide as no number; and a line; and a ground at no height.
        let mut w = vec![6, 0, 0, 0, 0, 0, 0, nan, 1, 0, nan, 0, 0];
        w.extend([4, 1, 3, 0, 0, 0, 0, 0, 7]);
        w.extend([2, 0, 2, 0, 0, 0, 0]);
        w.extend([5, 0, 1f32.to_bits(), 1, 1, 0, 0]);
        w.extend([5, 0, nan, 0, 2, 0, 0, 0, 0]);
        w.extend([3, 0, 1f32.to_bits(), 0, 0, 0, 0]);
        let d = read(&w).unwrap();
        assert_eq!(d.desk.len(), 1, "only the line");
        assert!(matches!(d.desk[0], Shape::LineSegment { .. }));
        assert_eq!(d.ground, 0.0, "the ground at the foot");
        assert_eq!(read(&w[..w.len() - 1]).unwrap_err(), Unread::Bad("its output is cut short"));
    }

    /// What the echo theme is asked in the tests: every field set to something it can be told by.
    fn echo_ask() -> Ask {
        let r = |a: f32, b: f32, c: f32, d: f32| Rect::from_min_max(pos2(a, b), pos2(c, d));
        let pose = Pose { enter: 1.0, run: 0.25, lower: 0.5, hooked: true, hook: 0.75, done: false, look: 2.0, t: 3.5 };
        let carried = Carried { pose, from_left: true, title: r(300.0, 120.0, 380.0, 140.0) };
        let events = [Event::Fresh, Event::Grown { chapter: 2, pages: 9 }, Event::Begun(3), Event::Clicked(pos2(4.5, 6.5))];
        let desk = Desk {
            area: r(0.0, 40.0, 1400.0, 900.0), page: r(450.0, 60.0, 950.0, 880.0), bar: Some(r(200.0, 860.0, 1200.0, 890.0)), zoom: 1.5, unit: 0.8,
            window: r(-350.0, -180.0, 1750.0, 1110.0), ground: pos2(700.0, 850.0),
        };
        Ask {
            input: input_bytes(12.5, &desk, Some(carried)),
            events: event_bytes(&events),
            seed: 0x0000_1234_0000_5678,
            garden: vec![3, 5, 9],
            pages: 17,
            lift: true,
        }
    }

    /// `module` asked once, under wasmi, and its words.
    fn run_once(module: &[u8], ask: &Ask) -> Words {
        let mut program = Program::load(module, crate::platform::Waker::new(&egui::Context::default()), 0.0);
        assert!(program.ask(ask, 0.0));
        assert_eq!(program.stopped(0.0), None);
        program.take().expect("an answer")
    }

    #[test]
    fn a_theme_is_told_every_field_of_its_input_and_each_event_as_written() {
        let d = read(&run_once(ECHO, &echo_ask())).unwrap();
        let circles: Vec<(u8, Pos2, f32)> = d.desk.iter().chain(&d.window).chain(&d.over).filter_map(|s| match s {
            Shape::Circle(c) => Some((c.fill.r(), c.center, c.radius)),
            Shape::Path(_) => None,
            _ => panic!("only circles and a path"),
        }).collect();
        let at = |k: u8| circles.iter().filter(|c| c.0 == k).map(|c| (c.1, c.2)).collect::<Vec<_>>();
        assert_eq!(at(1), [(pos2(0.0, 40.0), 1400.0)], "the desk");
        assert_eq!(at(2), [(pos2(450.0, 60.0), 500.0)], "the page");
        assert_eq!(at(3), [(pos2(1.5, 0.8), 1.0)], "the zoom and the size of things");
        assert_eq!(at(4), [(pos2(200.0, 860.0), 1000.0)], "the bar");
        assert_eq!(at(5), [(Pos2::ZERO, 12.5)], "the time");
        assert_eq!(at(14), [(pos2(-350.0, -180.0), 2100.0)], "the desk on the screen");
        assert_eq!(at(15), [(pos2(700.0, 850.0), 1.0)], "the ground");
        assert_eq!(d.window.len(), 1, "in the window");
        assert_eq!(at(30), [(pos2(700.0, 850.0), 1.0)], "in the window, at the ground");
        let path = d.desk.iter().find_map(|s| match s {
            Shape::Path(p) => Some(p.clone()),
            _ => None,
        }).expect("a path");
        assert_eq!(path.points, [pos2(450.0, 60.0), pos2(950.0, 60.0), pos2(950.0, 880.0), pos2(450.0, 880.0)], "round the page");
        assert!(path.closed && path.fill == Color32::TRANSPARENT, "a line closed, not a fill");
        assert_eq!(path.stroke.width, 1.5, "as wide as the zoom");
        assert_eq!(d.ground, 3.0, "its ground");
        assert_eq!(at(6), [(pos2(0x5678 as f32, 0x1234 as f32), 1.0)], "the seed");
        assert_eq!(at(7), [(pos2(17.0, 3.0), 1.0)], "its pages and the garden's chapters");
        assert_eq!(at(8), [(pos2(0.0, 3.0), 1.0), (pos2(1.0, 5.0), 1.0), (pos2(2.0, 9.0), 1.0)], "the garden");
        assert_eq!((at(10), at(11), at(12), at(13)), (vec![(Pos2::ZERO, 1.0)], vec![(pos2(2.0, 9.0), 1.0)], vec![(pos2(3.0, 0.0), 1.0)], vec![(pos2(4.5, 6.5), 1.0)]), "the events");
        assert_eq!(at(20), [(pos2(1.0, 0.25), 1.0)], "entered, running");
        assert_eq!(at(21), [(pos2(0.5, 0.75), 1.0)], "lowered, its hook");
        assert_eq!(at(22), [(pos2(2.0, 3.5), 1.0)], "where it looks, its seconds");
        assert_eq!(at(23), [(pos2(1.0, 1.0), 1.0)], "hooked, from the left");
        assert_eq!(at(24), [(pos2(300.0, 120.0), 80.0)], "the title's place");
        assert_eq!((d.offset, d.next, d.asked), (vec2(5.0, 7.0), 250, true), "the offset, the next frame and the ask to carry");
    }

    /// The echo theme's words for `echo_ask`, kept for `web/theme-worker.test.mjs`, which checks
    /// that the browser's host makes the same: CAPRICE_FIXTURES=1 writes it again.
    #[test]
    fn the_browser_s_host_is_given_the_same_words_to_make_as_the_tests_host() {
        let ask = echo_ask();
        let words = run_once(ECHO, &ask);
        let kept = serde_json::json!({
            "input": ask.input.to_vec(), "events": ask.events, "seed": ask.seed.to_string(), "garden": ask.garden,
            "pages": ask.pages, "lift": ask.lift, "words": words,
        });
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/echo.words.json");
        if std::env::var_os("CAPRICE_FIXTURES").is_some() {
            std::fs::write(&file, serde_json::to_string(&kept).unwrap() + "\n").unwrap();
        }
        let was: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(was == kept, "tests/fixtures/echo.words.json is not what the echo theme draws now");
    }

    #[test]
    fn a_frame_that_never_ends_or_traps_stops_the_theme_saying_why() {
        let ask = echo_ask();
        let mut spin = Program::load(&module("", "1 512", "(loop br 0)"), crate::platform::Waker::new(&egui::Context::default()), 0.0);
        spin.ask(&ask, 0.0);
        assert_eq!(spin.stopped(0.0).as_deref(), Some("it ran too long"));
        assert!(!spin.ask(&ask, 0.0) && spin.take().is_none(), "never asked again");
        let mut trap = Program::load(&module("", "1 512", "unreachable"), crate::platform::Waker::new(&egui::Context::default()), 0.0);
        trap.ask(&ask, 0.0);
        assert!(trap.stopped(0.0).is_some_and(|w| w.starts_with("it failed")), "{:?}", trap.stopped(0.0));
    }

    #[test]
    fn the_theme_that_never_answers_is_the_module_its_text_says() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let made = wat::parse_file(dir.join("spin.wat")).unwrap();
        if std::env::var_os("CAPRICE_FIXTURES").is_some() {
            std::fs::write(dir.join("spin.wasm"), &made).unwrap();
        }
        assert!(std::fs::read(dir.join("spin.wasm")).unwrap() == made, "tests/fixtures/spin.wasm is not spin.wat: CAPRICE_FIXTURES=1 makes it again");
        assert_eq!(check(&made), Ok(()));
    }

    #[test]
    fn the_sdk_seeds_and_tends_as_caprice_s_own_themes_do() {
        let (mut ours, mut sdk) = (super::super::Seeds(0x5EED_1234), theme_sdk::Seeds::from_state(0x5EED_1234));
        for _ in 0..1000 {
            assert_eq!(ours.next(-3.0, 7.0), sdk.next(-3.0, 7.0));
        }
        // A built-in theme's numbers for a story are a program's for that story's seed.
        for story in ["a story", "", "Ünïcode ✓", "0f3c9a1e-7b2d-4c55-9e10-2a6b8d4f1c37"] {
            for k in [0, 1, 99_000, 135_000 + 9, usize::from(u16::MAX) * 70_000] {
                let (mut ours, mut sdk) = (super::super::Seeds::of(story, k), theme_sdk::Seeds::of(story_seed(story), k));
                for _ in 0..20 {
                    assert_eq!(ours.next(0.0, 1.0), sdk.next(0.0, 1.0), "{story} {k}");
                }
            }
        }
        let (mut ours, mut sdk) = (super::super::Tended::default(), theme_sdk::Tended::default());
        let gardens: [&[u32]; 6] = [&[2], &[3], &[3], &[3, 1], &[3, 2], &[3, 2, 7]];
        let mut now = 0.0;
        for garden in gardens.iter().flat_map(|g| std::iter::repeat_n(*g, 40)) {
            now += 1.0 / 60.0;
            assert_eq!(ours.tend("a story", garden, 8, 1.2, 1.6, now), sdk.tend(7, garden, 8, 1.2, 1.6, now));
            for k in 0..garden.len() {
                assert_eq!(ours.grown(garden, k, 8), sdk.grown(garden, k, 8));
                assert_eq!(ours.place(k, 5, |p| p > 1), sdk.place(k, 5, |p| p > 1));
            }
            assert_eq!(ours.finishing(now, 1.6), sdk.finishing(now, 1.6));
        }
    }
}
