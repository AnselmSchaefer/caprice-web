//! A theme of the writer's own on the desk (`own.rs` reads it). Its picture covers the whole desk
//! whatever the window's shape, cut to fit and held at its foot, its marked parts moving
//! (`motion.rs`). For each chapter a piece stands beside the pages, a stage more for each page it
//! has had, each coming out of a little light; when the next chapter begins, it is finished at
//! once, as a ship is: the stages it had no pages for come in with its finish.
//! Its carrier brings a new chapter title in the way the balloon does (`rig`), mirrored to face
//! the way it flies.
//!
//! Like the other magic themes it lies on the desk behind every page, and the carrier over the
//! settled page only: nothing of it is printed or saved, and nothing of it moves the text (R30,
//! R34). The picture and its parts are rendered off the frame, at the size they are shown (no
//! larger than a browser's GPU takes), and again only when that changes by much.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use eframe::egui::{self, Color32, Pos2, Rect, Shape, pos2, vec2};

use super::{Carrier, LIGHT, Pose, Seeds, TALL, Tended, rig};
use crate::App;
use crate::motion::{Drawn, Layered, layered};
use crate::own::{Arrive, Each, Faces, Finish, Layout, Pack};
use crate::theme::Theme;

/// Seconds for a stage to come, and for a piece to finish once the next chapter begins.
pub const STEP: f32 = 1.2;
pub const FINISH: f32 = 1.6;
/// The largest a picture is rendered either way, in pixels: browsers' GPUs take no more.
const MOST_PIXELS: f32 = 4096.0;
/// Seconds the desk's picture takes to come in once it is rendered.
const COME_IN: f64 = 0.6;
/// How much nearer the picture comes as the writer zooms in on the page (the pieces with it): a
/// little, as a far landscape would.
const NEARER: f32 = 0.35;
/// How wide the carrier is rendered.
const CARRIER_PIXELS: f32 = 640.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum Which {
    Desk,
    Stage(usize, usize),
    Done(usize),
    Carrier,
}

struct Rendered {
    id: String,
    which: Which,
    width: u32,
    image: Result<Layered, String>,
    bounds: Rect,
}

/// A part rendered: its textures, how wide, and where in it anything is drawn (shares of its size).
struct Slot {
    drawn: Drawn,
    width: u32,
    bounds: Rect,
}

/// The theme of one's own as it is shown: read, its parts rendered, its pieces growing.
pub struct Shown {
    pack: Option<(String, Arc<Pack>)>,
    reading: Option<(String, Receiver<Result<Pack, String>>)>,
    slots: HashMap<Which, Slot>,
    /// The width each part was last asked for, and when.
    asked: HashMap<Which, (u32, f64)>,
    rendered: (Sender<Rendered>, Receiver<Rendered>),
    /// When the desk's picture first showed, to come in.
    since: Option<f64>,
    grow: Tended,
    /// What could not be shown, to say once.
    pub failed: Option<String>,
    /// What was drawn last frame, for tests.
    pub seen: Option<Seen>,
}

impl Default for Shown {
    fn default() -> Self {
        Self { pack: None, reading: None, slots: HashMap::new(), asked: HashMap::new(), rendered: channel(), since: None, grow: Tended::default(), failed: None, seen: None }
    }
}

/// What a theme of one's own drew last frame, for tests: where its picture lies, whether it is
/// rendered yet, and how many of its parts move; each chapter's piece, around it, with how many stages
/// show (all of them once it is finished) and how far finished; and the light of a stage coming,
/// if one is.
#[derive(Clone, Debug)]
pub struct Seen {
    pub desk: Rect,
    pub shown: bool,
    pub moving: usize,
    pub pieces: Vec<(Rect, usize, f32)>,
    pub coming: Option<Pos2>,
    /// How many stages lie fallen on the ground, and how many pieces move with the part of the
    /// desk they lie on.
    pub fallen: usize,
    pub riding: usize,
}

/// Where a piece stands: its foot, how large (1 at the front), and whether right of the page.
#[derive(Clone, Copy, Debug)]
struct Place {
    foot: Pos2,
    scale: f32,
    right: bool,
}

impl Place {
    fn in_view(&self, area: Rect, page: Rect, half: f32) -> bool {
        let x = self.foot.x;
        x - half > area.left() && x + half < area.right() && (x + half < page.left() || x - half > page.right())
    }
}

/// The places pieces `tall` high and `wide` wide (at the front) stand in on `ground` beside `page`.
/// In a row, taking turns left and right of the page and out from it, then in rows behind, smaller
/// and higher; scattered, about the lower part of the desk, smaller the higher they stand.
fn places(layout: Layout, story: &str, area: Rect, page: Rect, ground: f32, tall: f32, wide: f32) -> Vec<Place> {
    let mut out = Vec::new();
    match layout {
        // Where they are drawn, `draw_own` places them itself; rows, should it ask.
        Layout::Row | Layout::Drawn => {
            for row in 0..3 {
                let scale = 0.78f32.powi(row);
                let (w, foot) = (wide * scale * 1.08, ground - row as f32 * tall * 0.2);
                let gap = tall * 0.04;
                let fits = |room: f32| ((room - gap) / w).floor().max(1.0) as usize;
                let (left, right) = (fits(page.left() - area.left()), fits(area.right() - page.right()));
                for j in 0..left.max(right) {
                    if j < left {
                        out.push(Place { foot: pos2(page.left() - gap - w * (j as f32 + 0.5), foot), scale, right: false });
                    }
                    if j < right {
                        out.push(Place { foot: pos2(page.right() + gap + w * (j as f32 + 0.5), foot), scale, right: true });
                    }
                }
            }
        }
        Layout::Scatter => {
            let top = ground - (area.height() * 0.32).min(tall * 1.6);
            for i in 0..36 {
                let mut s = Seeds::of(story, 77_000 + i);
                let depth = s.next(0.0, 1.0);
                let scale = 0.55 + 0.45 * depth;
                let y = top + (ground - top) * depth;
                let right = i % 2 == 1;
                let half = wide * scale / 2.0;
                let (from, to) = if right { (page.right() + half, area.right() - half) } else { (area.left() + half, page.left() - half) };
                let x = if to > from { s.next(from, to) } else { from };
                out.push(Place { foot: pos2(x, y), scale, right });
            }
        }
    }
    if out.is_empty() {
        out.push(Place { foot: pos2(page.left() - wide / 2.0, ground), scale: 1.0, right: false });
    }
    out
}

/// Where in an image anything is drawn, as shares of its size.
fn drawn_bounds(image: &egui::ColorImage) -> Rect {
    let [w, h] = image.size;
    let mut r = Rect::NOTHING;
    for y in 0..h {
        for x in 0..w {
            if image.pixels[y * w + x].a() > 8 {
                r.extend_with(pos2(x as f32, y as f32));
            }
        }
    }
    if r.is_positive() { Rect::from_min_max(pos2(r.left() / w as f32, r.top() / h as f32), pos2((r.right() + 1.0) / w as f32, (r.bottom() + 1.0) / h as f32)) } else { Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)) }
}

/// The rectangle a picture of `size` covers `area` with, held by `anchor`, and brought `near` (1 as
/// it is) about the foot of the desk under `middle`.
fn cover(area: Rect, size: egui::Vec2, anchor: crate::own::Anchor, near: f32, middle: f32) -> Rect {
    let s = (area.width() / size.x).max(area.height() / size.y);
    let (w, h) = (size.x * s, size.y * s);
    let top = match anchor {
        crate::own::Anchor::Bottom => area.bottom() - h,
        crate::own::Anchor::Middle => area.center().y - h / 2.0,
        crate::own::Anchor::Top => area.top(),
    };
    let r = Rect::from_min_size(pos2(area.center().x - w / 2.0, top), vec2(w, h));
    // Nearer about a point on the desk, so it still covers all of it.
    let about = pos2(middle.clamp(area.left(), area.right()), area.bottom());
    Rect::from_min_max(about + (r.min - about) * near, about + (r.max - about) * near)
}

impl Shown {
    /// Nothing kept for another theme: picked again, it is read and grown again as it stands.
    fn forget(&mut self) {
        if self.pack.is_some() || self.reading.is_some() || !self.slots.is_empty() {
            *self = Shown::default();
        }
        self.grow.forget();
    }

    /// The tending of the pieces forgotten, so the story shows as it stands (`Eggs::regrown`).
    pub fn regrown(&mut self) {
        self.grow.forget();
    }

    /// Is the carrier there to bring a title?
    pub fn carrier_ready(&self) -> bool {
        self.slots.contains_key(&Which::Carrier)
    }

    /// The theme `id` read, from `shelf`, off the frame.
    fn read(&mut self, ctx: &egui::Context, id: &str, shelf: &crate::own::Shelf) {
        if self.pack.as_ref().is_some_and(|p| p.0 == id) {
            return;
        }
        if let Some((reading, rx)) = &self.reading
            && reading == id
        {
            match rx.try_recv() {
                Ok(Ok(pack)) => {
                    *self = Shown { pack: Some((id.to_owned(), Arc::new(pack))), ..Shown::default() };
                }
                Ok(Err(e)) => {
                    self.failed = Some(e);
                    self.reading = None;
                    self.pack = None;
                }
                Err(TryRecvError::Disconnected) => self.reading = None,
                Err(TryRecvError::Empty) => {}
            }
            return;
        }
        if self.failed.is_some() && self.reading.is_none() && self.pack.is_none() {
            return;
        }
        let Some(svg) = shelf.svg(id) else {
            self.failed = Some("its drawing is missing".into());
            return;
        };
        let (tx, rx) = channel();
        let wake = crate::platform::Waker::new(ctx);
        crate::platform::spawn(move || {
            let _ = tx.send(Pack::read(svg));
            wake.wake();
        });
        self.reading = Some((id.to_owned(), rx));
    }

    /// The parts rendered meanwhile, for the theme shown.
    fn receive(&mut self, ctx: &egui::Context) {
        let Some((id, _)) = &self.pack else { return };
        while let Ok(r) = self.rendered.1.try_recv() {
            if &r.id != id {
                continue;
            }
            if self.asked.get(&r.which).is_some_and(|a| a.0 == r.width) {
                self.asked.remove(&r.which);
            }
            match r.image {
                Ok(image) => {
                    self.slots.insert(r.which, Slot { drawn: Drawn::load(ctx, image), width: r.width, bounds: r.bounds });
                }
                Err(e) => self.failed = Some(e),
            }
        }
    }

    /// Part `which` rendered about `width` pixels wide, off the frame, unless it is already near
    /// enough that size or on its way.
    fn want(&mut self, ctx: &egui::Context, which: Which, width: f32, now: f64) {
        let Some((id, pack)) = &self.pack else { return };
        let width = ((width / 64.0).ceil() * 64.0).clamp(64.0, MOST_PIXELS) as u32;
        if let Some(s) = self.slots.get(&which)
            && width <= s.width + s.width / 4
            && width * 2 >= s.width
        {
            return;
        }
        if self.asked.get(&which).is_some_and(|&(w, at)| w == width || now - at < 0.4) {
            return;
        }
        let svg = match which {
            Which::Desk => pack.desk.svg.clone(),
            Which::Stage(p, s) => pack.pieces[p].stages[s].svg.clone(),
            Which::Done(p) => match &pack.pieces[p].done {
                Some(d) => d.svg.clone(),
                None => return,
            },
            Which::Carrier => match &pack.carrier {
                Some(c) => c.art.svg.clone(),
                None => return,
            },
        };
        self.asked.insert(which, (width, now));
        let (tx, wake, id) = (self.rendered.0.clone(), crate::platform::Waker::new(ctx), id.clone());
        crate::platform::spawn(move || {
            let image = layered(&svg, width as f32);
            let bounds = image.as_ref().map_or(Rect::NOTHING, |i| drawn_bounds(&i.still));
            let _ = tx.send(Rendered { id, which, width, image, bounds });
            wake.wake();
        });
    }

    /// The carrier at `p`, bringing a title lying at `place` on a page lying at `page`, at `now`.
    pub fn carrier(&self, by: Carrier, p: Pose, place: Rect, page: Rect, desk: Rect, now: f64) -> Vec<Shape> {
        let (Some((_, pack)), Some(slot), Carrier::Own { right }) = (&self.pack, self.slots.get(&Which::Carrier), by) else { return Vec::new() };
        let Some(c) = &pack.carrier else { return Vec::new() };
        let r = rig(by, p, place, page, desk);
        let h = r.size * TALL * pack.meta.carrier.size.clamp(0.3, 3.0);
        let w = h * c.art.size.x / c.art.size.y;
        let rect = Rect::from_min_size(r.hook - vec2(c.hook.x * w, c.hook.y * h), vec2(w, h));
        // From the right it flies to the left; mirrored about where it holds the title.
        let mirror = right != (pack.meta.carrier.faces == Faces::Left);
        let x = r.hook.x;
        slot.drawn.shapes(rect, Color32::WHITE, &|q| if mirror { pos2(2.0 * x - q.x, q.y) } else { q }, Some(now))
    }
}

/// A piece of `stages` drawn over `rect` (mirrored if `mirror`), `grown` so far and `finished` so
/// far, its stages coming as `arrive` says, a fall from `drop` above, at `now`, all of it `moved`
/// with the part of the desk it lies on; and how many stages show whole. Where a stage is coming
/// goes in `coming`.
#[allow(clippy::too_many_arguments)]
fn piece_shapes(slots: &HashMap<Which, Slot>, p: usize, stages: usize, rect: Rect, mirror: bool, grown: f32, finished: f32, arrive: Arrive, drop: f32, now: f64, moved: &dyn Fn(Pos2) -> Pos2, coming: &mut Option<Pos2>) -> (Vec<Shape>, usize) {
    let flip = |q: Pos2| moved(if mirror { pos2(2.0 * rect.center().x - q.x, q.y) } else { q });
    let size = rect.width().max(rect.height());
    let (mut out, mut shown) = (Vec::new(), 0);
    for s in 0..stages {
        // A chapter of fewer pages than stages is finished at once when the next begins, as a
        // ship is: the stages it had no pages for come in with its finish.
        let late = s as f32 >= grown.ceil();
        let a = if late { finished } else { (grown - s as f32).clamp(0.0, 1.0) };
        shown += usize::from(a >= 1.0);
        let Some(slot) = slots.get(&Which::Stage(p, s)).filter(|_| a > 0.0) else { continue };
        let b = slot.bounds;
        let from = flip(rect.lerp_inside(b.center().to_vec2()));
        let time = Some(now);
        if a >= 1.0 || late {
            out.extend(slot.drawn.shapes(rect, Color32::WHITE.gamma_multiply(a), &flip, time));
            continue;
        }
        *coming = Some(from);
        match arrive {
            Arrive::Light => {
                // Out of its light, growing a little into its place.
                let k = 0.85 + 0.15 * a;
                out.extend(slot.drawn.shapes(rect, Color32::WHITE.gamma_multiply(a), &|q| from + (flip(q) - from) * k, time));
                out.extend(light(from, size * 0.18, (1.0 - a) * (a * 4.0).min(1.0)));
            }
            Arrive::Fall => {
                // Faster and faster from above, then a little bounce where it lands.
                let y = if a < 0.7 {
                    let t = a / 0.7;
                    -drop * (1.0 - t * t)
                } else {
                    let t = (a - 0.7) / 0.3;
                    -size * 0.06 * (std::f32::consts::PI * t).sin() * (1.0 - t)
                };
                *coming = Some(from + vec2(0.0, y));
                out.extend(slot.drawn.shapes(rect, Color32::WHITE.gamma_multiply((a * 8.0).min(1.0)), &|q| flip(q) + vec2(0.0, y), time));
            }
            Arrive::Grow => {
                // Up out of the ground from its foot.
                let foot = flip(rect.lerp_inside(egui::vec2(b.center().x, b.max.y)));
                let k = a * a * (3.0 - 2.0 * a);
                out.extend(slot.drawn.shapes(rect, Color32::WHITE.gamma_multiply((a * 3.0).min(1.0)), &|q| foot + (flip(q) - foot) * k, time));
            }
        }
    }
    if finished > 0.0
        && let Some(slot) = slots.get(&Which::Done(p))
    {
        out.extend(slot.drawn.shapes(rect, Color32::WHITE.gamma_multiply(finished), &flip, Some(now)));
    }
    (out, shown)
}

/// The piece chapter `k` of the story has, of `count`: each in turn, or one at random, never the one
/// the chapter before had; the same for the same story every time.
fn piece_for(order: crate::own::Order, story: &str, k: usize, count: usize) -> usize {
    if order == crate::own::Order::Turn || count < 2 {
        return k % count.max(1);
    }
    let mut before = usize::MAX;
    for j in 0..=k {
        let mut pick = (Seeds::of(story, 55_000 + j).next(0.0, count as f32) as usize).min(count - 1);
        if pick == before {
            pick = (pick + 1) % count;
        }
        before = pick;
    }
    before
}

/// The first `count` stages of piece `p` (lying at `rect`) fallen to `ground` for chapter `k`,
/// `q` of the way: one after another, faster and faster, each landing about the foot of where it
/// hung, turned its own way, the same for the same story. How many lie there goes in `lying`.
#[allow(clippy::too_many_arguments)]
fn fallen(slots: &HashMap<Which, Slot>, p: usize, count: usize, rect: Rect, ground: f32, story: &str, k: usize, q: f32, wide: f32, now: f64, lying: &mut usize) -> Vec<Shape> {
    let mut out = Vec::new();
    for s in 0..count {
        let Some(slot) = slots.get(&Which::Stage(p, s)) else { continue };
        let b = slot.bounds;
        let c = rect.lerp_inside(b.center().to_vec2());
        let half = b.height() * rect.height() / 2.0;
        let mut r = Seeds::of(story, 66_000 + k * 31 + s);
        let (dx, depth, turn) = (r.next(-1.0, 1.0) * wide * 0.02, r.next(0.0, 1.0) * wide * 0.008, r.next(-1.2, 1.2));
        let t = ((q - s as f32 / count as f32 * 0.45) / 0.55).clamp(0.0, 1.0);
        let at = c + vec2(dx * t, (ground + depth - half - c.y).max(0.0) * t * t);
        *lying += usize::from(t >= 1.0);
        let (sin, cos) = (turn * t).sin_cos();
        out.extend(slot.drawn.shapes(rect, Color32::WHITE, &|v| {
            let d = v - c;
            at + vec2(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
        }, Some(now)));
    }
    out
}

/// A soft light at `at`, `r` across, as strong as `a`.
fn light(at: Pos2, r: f32, a: f32) -> Vec<Shape> {
    (0..4).map(|k| Shape::circle_filled(at, r * (1.0 - k as f32 * 0.22), LIGHT.gamma_multiply(a * (0.12 + 0.2 * k as f32)))).collect()
}

impl App {
    /// The writer's own theme on the desk, behind the pages lying at `page`: its picture, and for
    /// each chapter a piece of as many stages as it has had pages (`Doc::garden`), the one before
    /// the newest finished.
    pub fn draw_own(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.own.seen = None;
        let id = match (self.themes.theme, &self.themes.own_id) {
            (Theme::Own, Some(id)) => id.clone(),
            _ => {
                self.eggs.own.forget();
                return;
            }
        };
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        let z = (self.zoom / self.fitting_zoom(area)).clamp(0.5, 4.0);
        let o = &mut self.eggs.own;
        o.read(&ctx, &id, &self.themes.own);
        o.receive(&ctx);
        if let Some(e) = o.failed.take() {
            self.status = format!("- the theme could not be shown: {e}");
        }
        let o = &mut self.eggs.own;
        let Some((_, pack)) = o.pack.clone() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        let ppp = ctx.pixels_per_point();
        let cover = cover(area, pack.desk.size, pack.meta.anchor, 1.0 + (z.max(1.0) - 1.0) * NEARER, page.center().x);
        let aspect = pack.desk.size.x / pack.desk.size.y;
        o.want(&ctx, Which::Desk, (cover.width() * ppp).min(MOST_PIXELS * aspect), now);
        if pack.carrier.is_some() {
            o.want(&ctx, Which::Carrier, CARRIER_PIXELS, now);
        }
        let painter = ui.painter();
        let mut seen = Seen { desk: cover, shown: false, moving: 0, pieces: Vec::new(), coming: None, fallen: 0, riding: 0 };
        let mut busy = false;
        if let Some(slot) = o.slots.get(&Which::Desk) {
            let since = *o.since.get_or_insert(now);
            let a = ((now - since) / COME_IN).clamp(0.0, 1.0) as f32;
            busy |= a < 1.0;
            painter.extend(slot.drawn.shapes(cover, Color32::WHITE.gamma_multiply(a * a * (3.0 - 2.0 * a)), &|p| p, Some(now)));
            (seen.shown, seen.moving) = (true, slot.drawn.parts());
        }

        if !pack.pieces.is_empty() {
            let garden = &self.doc.garden;
            let g = &pack.meta.growth;
            let (arrive, most) = (g.arrive, pack.pieces.iter().map(|p| p.stages.len()).max().unwrap_or(1));
            if g.layout == Layout::Drawn {
                for (p, piece) in pack.pieces.iter().enumerate() {
                    let width = cover.width() * piece.at.width() * ppp;
                    for s in 0..piece.stages.len() {
                        o.want(&ctx, Which::Stage(p, s), width, now);
                    }
                    if piece.done.is_some() {
                        o.want(&ctx, Which::Done(p), width, now);
                    }
                }
                let in_place = |piece: &crate::own::Piece| Rect::from_min_max(cover.lerp_inside(piece.at.min.to_vec2()), cover.lerp_inside(piece.at.max.to_vec2()));
                // What is drawn on a moving part of the desk (a coconut on a palm) moves with it.
                let desk = o.slots.get(&Which::Desk).map(|d| &d.drawn);
                let follow = |piece: &crate::own::Piece, riding: &mut usize| -> Box<dyn Fn(Pos2) -> Pos2> {
                    match desk.and_then(|d| Some((d, d.holding(piece.at.center())?))) {
                        Some((d, i)) => {
                            *riding += 1;
                            Box::new(d.moved(i, cover, now))
                        }
                        None => Box::new(|q| q),
                    }
                };
                // From about as high as a tree's crown above the ground.
                let drop = area.height() * 0.3;
                if g.each == Each::Story {
                    // Each piece where it is drawn, growing with every page of the story: those before
                    // the first chapter title too, which no chapter of the garden counts. It never
                    // shrinks while the story is open (`Tended`).
                    let pages = [garden.iter().sum::<u32>().max(self.doc.pages() as u32).max(1)];
                    busy |= o.grow.tend(&self.story_id, &pages, most, STEP, FINISH, now);
                    let grown = o.grow.grown(&pages, 0, most);
                    for (p, piece) in pack.pieces.iter().enumerate() {
                        let (rect, stages) = (in_place(piece), piece.stages.len());
                        let grown = grown.min(stages as f32);
                        // Its finish comes with its last stage.
                        let finished = (grown - (stages as f32 - 1.0)).clamp(0.0, 1.0);
                        let moved = follow(piece, &mut seen.riding);
                        let (shapes, shown) = piece_shapes(&o.slots, p, stages, rect, false, grown, finished, arrive, drop, now, &*moved, &mut seen.coming);
                        painter.extend(shapes);
                        seen.pieces.push((rect, shown, finished));
                    }
                } else {
                    // Each chapter's piece where it is drawn: a stage a page, and when the next
                    // chapter begins, its finish.
                    let n = garden.len().max(1);
                    let story = self.story_id.clone();
                    let piece_of = |k: usize| piece_for(g.order, &story, k, pack.pieces.len());
                    busy |= o.grow.tend(&self.story_id, garden, pack.pieces[piece_of(n - 1)].stages.len(), STEP, FINISH, now);
                    let finishing = o.grow.finishing(now, FINISH);
                    let floor = cover.top() + cover.height() * 0.9;
                    let ground = pack.ground.map_or(floor, |s| cover.top() + s * cover.height()).min(area.bottom() - hidden);
                    seen.pieces = vec![(Rect::NOTHING, 0, 0.0); n];
                    for k in 0..n {
                        let p = piece_of(k);
                        let piece = &pack.pieces[p];
                        let (rect, stages) = (in_place(piece), piece.stages.len());
                        let grown = o.grow.grown(garden, k, stages);
                        let finished = match finishing {
                            Some((m, q)) if m == k => q,
                            _ if k + 1 >= n => 0.0,
                            _ => 1.0,
                        };
                        let moved = follow(piece, &mut seen.riding);
                        if g.finish == Finish::Fall && finished > 0.0 {
                            // Off the tree, onto the ground, and lying there for good.
                            let count = (grown.ceil() as usize).min(stages);
                            painter.extend(fallen(&o.slots, p, count, rect, ground, &story, k, finished, cover.width(), now, &mut seen.fallen));
                            if let Some(done) = o.slots.get(&Which::Done(p)) {
                                painter.extend(done.drawn.shapes(rect, Color32::WHITE.gamma_multiply(finished), &*moved, Some(now)));
                            }
                            seen.pieces[k] = (rect, 0, finished);
                            continue;
                        }
                        // A piece another chapter after grows on is that chapter's now.
                        if finished > 0.0 && (k + 1..n).any(|j| piece_of(j) == p) {
                            continue;
                        }
                        let (shapes, shown) = piece_shapes(&o.slots, p, stages, rect, false, grown, finished, arrive, drop, now, &*moved, &mut seen.coming);
                        painter.extend(shapes);
                        seen.pieces[k] = (rect, shown, finished);
                    }
                }
            } else {
                let n = garden.len().max(1);
                let story = self.story_id.clone();
                let piece_of = |k: usize| piece_for(g.order, &story, k, pack.pieces.len());
                let newest = &pack.pieces[piece_of(n - 1)];
                busy |= o.grow.tend(&self.story_id, garden, newest.stages.len(), STEP, FINISH, now);
                let floor = area.bottom() - hidden - area.height() * 0.025;
                let ground = pack.ground.map_or(floor, |s| (cover.top() + s * cover.height()).min(area.bottom() - hidden));
                let tall = g.size.clamp(0.05, 0.8) * cover.height();
                let widest = pack.pieces.iter().map(|p| p.size.x / p.size.y).fold(0.1, f32::max) * tall;
                let all = places(g.layout, &self.story_id, area, page, ground, tall, widest);
                let mut order: Vec<(usize, Place)> = (0..n).map(|k| (k, all[o.grow.place(k, all.len(), |p| all[p].in_view(area, page, widest * all[p].scale / 2.0))])).collect();
                order.sort_by(|a, b| a.1.foot.y.total_cmp(&b.1.foot.y).then(a.0.cmp(&b.0)));
                // Every stage of every piece, as wide as the front row shows them.
                for (p, piece) in pack.pieces.iter().enumerate() {
                    let px = tall * piece.size.x / piece.size.y * ppp * 1.2;
                    for s in 0..piece.stages.len() {
                        o.want(&ctx, Which::Stage(p, s), px, now);
                    }
                    if piece.done.is_some() {
                        o.want(&ctx, Which::Done(p), px, now);
                    }
                }
                let finishing = o.grow.finishing(now, FINISH);
                seen.pieces = vec![(Rect::NOTHING, 0, 0.0); n];
                for (k, place) in order {
                    let p = piece_of(k);
                    let piece = &pack.pieces[p];
                    let stages = piece.stages.len();
                    let h = tall * place.scale;
                    let w = h * piece.size.x / piece.size.y;
                    let rect = Rect::from_min_max(pos2(place.foot.x - w / 2.0, place.foot.y - h), pos2(place.foot.x + w / 2.0, place.foot.y));
                    let grown = o.grow.grown(garden, k, stages);
                    let finished = match finishing {
                        Some((m, q)) if m == k => q,
                        _ if k + 1 >= n => 0.0,
                        _ => 1.0,
                    };
                    let (shapes, shown) = piece_shapes(&o.slots, p, stages, rect, g.mirror && place.right, grown, finished, arrive, h * 1.6, now, &|q| q, &mut seen.coming);
                    painter.extend(shapes);
                    if finished > 0.0 && finished < 1.0 {
                        // A ring of sparks going out from the piece as it is finished.
                        let c = rect.center();
                        for j in 0..12 {
                            let d = egui::Vec2::angled(j as f32 / 12.0 * std::f32::consts::TAU);
                            painter.add(Shape::circle_filled(c + d * w.max(h) * (0.2 + 0.5 * finished), w.max(h) * 0.012, LIGHT.gamma_multiply(1.0 - finished)));
                        }
                    }
                    seen.pieces[k] = (rect, shown, finished);
                }
            }
        }
        if busy {
            ctx.request_repaint();
        } else {
            // The picture moves.
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        self.eggs.own.seen = Some(seen);
    }
}
