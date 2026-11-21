//! A theme program's pictures, made off screen (R36, the plan's step 5): the program played through
//! a story told in a few moments (begun, three chapters written, a chapter finishing, the carrier on
//! its way), each frame laid out as Caprice lays out the desk at the fitting zoom, read as Caprice
//! reads it (`program::read`), tessellated by egui as the screen's shapes are, and filled in here,
//! triangle by triangle, into a picture: the desk in the theme's colours, the program's shapes, the
//! page and the bars. They are what Claude is shown of a theme it wrote, and a theme's picture in
//! the gallery. The same frames count a theme's points where a writer reads: on a phone's desk and
//! on a computer's, zoomed out too. A theme drawn zoomed out as far as a writer zooms is pictured
//! as well, with how much of the window's edges its own shapes cover there and at the fitting zoom:
//! a sky stretched over the desk instead of what shows stops short of them (`Previewed::bare`).
//! And one moment is drawn again a step out and a step in, to find what slides as the writer zooms:
//! anything laid out from what shows rather than from the desk (`Previewed::slides`). Last, the desk
//! is pictured with no page on it, as the welcome shows it and as the writer sees it who drags the
//! page aside: a world cut off where the page's edge lies is seen there (the Lighthouse's cliff ended
//! in a straight edge under it), and never in the pictures with the page.
//!
//! The program runs where every program theme runs (`platform::Program`: a worker in the browser,
//! played natively under the tests, nowhere on the desktop), so a preview is asked for and taken
//! when it is done, never waited for.

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Shape, Stroke, Vec2, epaint, pos2, vec2};

use super::program::{Carried, Desk, Event, input_bytes, event_bytes, read, story_seed};
use super::{Carrier, pose_of};
use crate::platform::Waker;
use crate::platform::program::{Ask, Program};
use crate::theme::{INK, PAPER, Palette};
use crate::ui::{DOCK_GAP, DOCK_H, DOCK_MAX_W, RESERVED};

/// The pictures' window: a computer's, small enough for Claude to see whole (it takes 1568 points
/// on the long side), wide enough to show the desk on both sides of the page.
pub const PICTURE: Vec2 = vec2(1280.0, 800.0);
/// The desks a theme's points are counted on: a phone held upright, and a computer's.
pub const PHONE: Vec2 = vec2(412.0, 915.0);
pub const COMPUTER: Vec2 = vec2(1920.0, 1080.0);
/// How far a writer zooms out at most: half the fitting zoom (`view::LEAST`).
const ZOOMED_OUT: f32 = crate::view::LEAST;
/// A4, in points, as a new story's pages are.
const PAPER_SIZE: Vec2 = vec2(595.0, 842.0);
/// The moment pictured with no page on the desk, nor the bars: the whole world, as the welcome shows
/// it behind its cards and as a writer sees it who drags the page aside.
pub const BARE: &str = "without the page";
/// Frames a second the story is told at.
const FPS: f64 = 10.0;
/// How much of the window's edges a theme's shapes must cover to count as reaching them.
const COVERED: f32 = 0.95;
/// The zooms the steady moment is drawn at again: a step out and a step in. Each moves the edge of
/// what shows by some units, and what is stepped along from that edge by a step that divides them
/// comes back to the same places (at 0.9 the edge moved 80, and grass stepped by 4 or 5 was not seen
/// to move; at 0.93 alone, grass stepped by 3 moved a fifth of a unit). Of these two, one or the other
/// moves anything stepped by 1 to 40 units by a third of a unit or more (by a third of its step,
/// below 2).
const STEADY_ZOOMS: [f32; 2] = [0.93, 1.08];
/// How many times larger than the window the two drawings of the steady moment are compared, so
/// that what moves by a unit or two is seen; the outline of a fill drifting by less is not.
const STEADY_SCALE: f32 = 2.0;
/// How many of the compared pixels may move before a theme is said to slide. Caprice's own themes
/// move none; the gallery's Magic Lighthouse moved 725 with its lighthouse on the right, where the
/// grass on its cliff shuffled by a unit or so, and 1472 on the left, where it streamed along the top.
const MOVED: usize = 32;

/// The desk as Caprice lays it out for a window of `size` at the fitting zoom, the page bar along
/// its foot (`App::page_rect`, `App::fitting_zoom`, `App::dock_rect`), and how big things are on it
/// (`Eggs::unit`): the window, the page and the bar.
pub fn layout(size: Vec2) -> (Rect, Rect, Rect, f32) {
    let area = Rect::from_min_size(Pos2::ZERO, size);
    let view = Rect::from_min_max(pos2(0.0, RESERVED), pos2(size.x, size.y - RESERVED));
    let zoom = crate::view::fit(view, PAPER_SIZE);
    let page_size = PAPER_SIZE * zoom;
    let top = if view.width() < crate::view::NARROW && page_size.y > view.height() - 16.0 { view.top() + 8.0 } else { view.center().y - page_size.y / 2.0 };
    let page = Rect::from_min_size(pos2(view.center().x - page_size.x / 2.0, top), page_size);
    let bar = Rect::from_center_size(pos2(area.center().x, size.y - DOCK_GAP - DOCK_H / 2.0), vec2((size.x - 48.0).clamp(0.0, DOCK_MAX_W), DOCK_H));
    (area, page, bar, (size.y / 900.0).clamp(0.6, 1.6))
}

/// What is shown at a frame: the window, how far zoomed in or out from the fitting zoom, and
/// whether the carrier is at work.
#[derive(Clone, Copy, Debug)]
struct Frame {
    time: f64,
    size: Vec2,
    zoom: f32,
    carried: bool,
}

/// A moment of the story: what happened, how the garden stands, then the frames shown, the last
/// of them kept as a picture (`name`) or only counted.
struct Moment {
    name: &'static str,
    picture: bool,
    garden: Vec<u32>,
    events: Vec<Event>,
    frames: Vec<Frame>,
}

/// The story a preview tells: begun; three chapters written (shown afresh, as when the theme is
/// picked); the fourth begun, the third finishing; a title carried, if the theme carries titles;
/// three chapters written, zoomed out as far as a writer zooms; the same, held still, its last frame
/// drawn again a step out and a step in (`STEADY_ZOOMS`) at the same moment; then the same desk on a phone
/// and on a computer, there zoomed out too, to count its points; and last, three chapters written with
/// no page on the desk (`BARE`).
fn story() -> Vec<Moment> {
    let at = |from: f64, seconds: f64, size: Vec2, zoom: f32, carried: bool| -> Vec<Frame> {
        (0..=(seconds * FPS) as usize).map(|k| Frame { time: from + k as f64 / FPS, size, zoom, carried }).collect()
    };
    let written = vec![14, 10, 6];
    let mut steady = at(37.0, 1.0, PICTURE, 1.0, false);
    let last = steady[steady.len() - 1];
    steady.extend(STEADY_ZOOMS.map(|zoom| Frame { zoom, ..last }));
    let finishing = vec![14, 10, 6, 1];
    vec![
        Moment { name: "begun", picture: true, garden: vec![1], events: vec![Event::Fresh], frames: at(0.0, 2.0, PICTURE, 1.0, false) },
        Moment { name: "written", picture: true, garden: written.clone(), events: vec![Event::Fresh], frames: at(10.0, 2.0, PICTURE, 1.0, false) },
        Moment {
            name: "finishing", picture: true, garden: finishing.clone(),
            events: vec![Event::Begun(3), Event::Grown { chapter: 3, pages: 1 }], frames: at(20.0, 2.0, PICTURE, 1.0, false),
        },
        Moment { name: "carried", picture: true, garden: finishing.clone(), events: vec![], frames: at(30.0, 1.5, PICTURE, 1.0, true) },
        Moment { name: "zoomed out", picture: true, garden: written.clone(), events: vec![Event::Fresh], frames: at(35.0, 1.0, PICTURE, ZOOMED_OUT, false) },
        Moment { name: "steady", picture: false, garden: written.clone(), events: vec![Event::Fresh], frames: steady },
        Moment { name: "phone", picture: false, garden: written.clone(), events: vec![Event::Fresh], frames: at(40.0, 0.5, PHONE, 1.0, false) },
        Moment { name: "computer", picture: false, garden: written.clone(), events: vec![Event::Fresh], frames: at(50.0, 0.5, COMPUTER, 1.0, false) },
        Moment { name: "computer zoomed out", picture: false, garden: written.clone(), events: vec![Event::Fresh], frames: at(60.0, 0.5, COMPUTER, ZOOMED_OUT, false) },
        Moment { name: BARE, picture: true, garden: written, events: vec![Event::Fresh], frames: at(70.0, 1.0, PICTURE, 1.0, false) },
    ]
}

/// The carrier half-way through its course, from the left, over a title at the page's top.
fn carried(page: Rect, elapsed: f32) -> Carried {
    let title = Rect::from_min_size(page.min + vec2(page.width() * 0.2, page.height() * 0.12), vec2(page.width() * 0.6, page.height() * 0.04));
    Carried { pose: pose_of(Carrier::Program { right: false }, elapsed, None), from_left: true, title }
}

/// A picture of a moment, and the most points it drew in any frame.
pub struct Picture {
    pub name: &'static str,
    pub image: ColorImage,
}

/// What a preview came to: its pictures, and the most points a frame drew on a phone's desk and on
/// a computer's (zoomed out included), which Claude is told with the pictures (the plan's step 5);
/// and how much of the window's edges its own shapes cover, at the fitting zoom and zoomed out.
#[cfg_attr(not(test), allow(dead_code))]
pub struct Previewed {
    pub pictures: Vec<Picture>,
    pub phone_points: usize,
    pub computer_points: usize,
    pub edges: (f32, f32),
    pub slid: Slid,
}

/// What moved on the desk between the steady moment's drawings, the same moment at three zooms:
/// how many of the pixels compared, and the rectangle around them, in points on the desk.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slid {
    pub pixels: usize,
    pub around: Option<Rect>,
}

impl Previewed {
    /// What Claude is told when the theme draws its own sky and ground to the window's edges at the
    /// fitting zoom but stops short of them zoomed out: stretched over `Input::area`, the desk at the
    /// fitting zoom, instead of over `Input::shown`, what the window shows (the gallery's Magic
    /// Castle and Lighthouse did). A theme that leaves the desk's own colours showing at the fitting
    /// zoom too is left alone: it draws no sky of its own, as the guide allows.
    pub fn bare(&self) -> Option<String> {
        let (fitted, out) = self.edges;
        let percent = |k: f32| (k * 100.0).round() as u32;
        (fitted >= COVERED && out < COVERED).then(|| format!(
            "It plays, but zoomed out (as far as a writer zooms, half the fitting zoom) its own sky and ground stop short \
            of the window's edges, and the desk's plain colours show around them like a frame: its shapes cover {}% of \
            the window's edges there, against {}% at the fitting zoom. Stretch every fill that must reach the edges (the \
            sky, the ground, the water, a field) over `Input::shown`, not `Input::area`; keep placing things from \
            `area`, the page and the ground.", percent(out), percent(fitted)))
    }

    /// What Claude is told when part of the theme slides as the writer zooms: drawn at the same
    /// moment at two zooms, its shapes on the desk are not where they were. Something is laid out
    /// from `Input::shown` (the gallery's Magic Lighthouse laid the grass on its cliff from the
    /// window's edge, and it streamed along the clifftop while the writer zoomed). A fill stretched
    /// over `shown` is left alone: it changes only gradually, which the comparison does not count.
    pub fn slides(&self) -> Option<String> {
        let around = self.slid.around.filter(|_| self.slid.pixels >= MOVED)?;
        let (area, page, _, _) = layout(PICTURE);
        Some(format!(
            "It plays, but part of it slides as the writer zooms. Drawn at the same moment at the fitting zoom and \
            a step out and in ({} and {}), the shapes between x {} and {} and y {} and {} on the desk (a desk \
            {} by {}, the page from x {} to {}) are not where they were: something there is laid out from \
            `Input::shown`, which changes with every step of the zoom. Use `shown` only to stretch fills to the \
            window's edges. Lay everything else out from fixed points (`area`, the page, the ground, the things you \
            placed), and that includes what lies along such a fill (grass on a ground, foam along a sea, posts along \
            a shore) and the points a fill's outline is sampled at: step them from a fixed point (`x = start + k * step`), \
            never from where `shown` begins.",
            STEADY_ZOOMS[0], STEADY_ZOOMS[1], around.left().round(), around.right().round(), around.top().round(), around.bottom().round(),
            area.width(), area.height(), page.left().round(), page.right().round()))
    }

    /// The gallery's picture of the theme: three chapters written, as wide as the gallery keeps them.
    pub fn gallery_picture(&self) -> Option<ColorImage> {
        let written = self.pictures.iter().find(|p| p.name == "written")?;
        Some(shrunk(&written.image, crate::bundle::PREVIEW_WIDTH))
    }
}

/// A picture made `width` pixels wide, its height in proportion.
pub fn shrunk(image: &ColorImage, width: u32) -> ColorImage {
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let Some(img) = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, rgba) else { return image.clone() };
    let height = (image.size[1] as u64 * u64::from(width) / image.size[0].max(1) as u64) as u32;
    let small = image::imageops::resize(&img, width, height.max(1), image::imageops::FilterType::Triangle);
    ColorImage::from_rgba_unmultiplied([small.width() as usize, small.height() as usize], small.as_raw())
}

/// A program being shown its story, frame after frame.
pub struct Preview {
    program: Program,
    palette: Palette,
    story: Vec<Moment>,
    /// The moment and the frame asked for last.
    at: (usize, usize),
    asked: bool,
    /// The ground it last named, the zoom it is drawn at, and whether it carries titles.
    ground: f32,
    asks_lift: bool,
    pictures: Vec<Picture>,
    points: (usize, usize),
    edges: (f32, f32),
    /// The steady moment's first drawing, held to compare the others with, and what moved.
    held: Option<Held>,
    slid: Slid,
    done: Option<Result<Previewed, String>>,
}

impl Preview {
    /// `module` (checked already) shown its story, drawn in `palette`'s colours.
    pub fn start(module: &[u8], palette: Palette, wake: Waker, now: f64) -> Preview {
        Preview {
            program: Program::load(module, wake, now), palette, story: story(), at: (0, 0), asked: false,
            ground: 0.0, asks_lift: false, pictures: Vec::new(), points: (0, 0), edges: (0.0, 0.0), held: None,
            slid: Slid::default(), done: None,
        }
    }

    /// Looked at, at `now`: the next frame asked for when the last has come; the pictures, or why
    /// there are none, once it is done.
    pub fn tend(&mut self, now: f64) -> Option<Result<Previewed, String>> {
        while self.done.is_none() {
            if let Some(why) = self.program.stopped(now) {
                self.done = Some(Err(why));
                break;
            }
            if self.asked {
                let Some(words) = self.program.take() else { break };
                self.asked = false;
                if let Err(why) = self.heard(&words) {
                    self.done = Some(Err(why));
                    break;
                }
                if !self.next() {
                    let (phone_points, computer_points) = self.points;
                    let (pictures, edges, slid) = (std::mem::take(&mut self.pictures), self.edges, self.slid);
                    self.done = Some(Ok(Previewed { pictures, phone_points, computer_points, edges, slid }));
                    break;
                }
            }
            if !self.ask(now) {
                break;
            }
        }
        self.done.take()
    }

    /// On to the next frame; whether there is one. The carrier's moment is passed over by a theme
    /// that does not carry titles.
    fn next(&mut self) -> bool {
        let (m, f) = self.at;
        self.at = if f + 1 < self.story[m].frames.len() { (m, f + 1) } else { (m + 1, 0) };
        while self.at.0 < self.story.len() && self.story[self.at.0].frames[0].carried && !self.asks_lift {
            self.at = (self.at.0 + 1, 0);
        }
        self.at.0 < self.story.len()
    }

    fn ask(&mut self, now: f64) -> bool {
        let (m, f) = self.at;
        let moment = &self.story[m];
        let frame = moment.frames[f];
        let (area, page, bar, unit, anchor) = desk_of(frame, self.ground);
        let carried = frame.carried.then(|| carried(page, (frame.time - moment.frames[0].time) as f32 + 1.2));
        let ask = Ask {
            input: input_bytes(frame.time, &Desk { area, page, bar: Some(bar), zoom: frame.zoom, unit, window: Rect::from_min_size(Pos2::ZERO, frame.size), ground: anchor }, carried),
            events: if f == 0 { event_bytes(&moment.events) } else { Vec::new() },
            seed: story_seed("caprice-preview"),
            garden: moment.garden.clone(),
            pages: moment.garden.iter().sum(),
            lift: true,
        };
        self.asked = self.program.ask(&ask, now);
        self.asked
    }

    /// A frame's words: read, counted, and drawn if it is a moment's last.
    fn heard(&mut self, words: &[u32]) -> Result<(), String> {
        let drawn = read(words).map_err(|e| match e {
            super::program::Unread::Over(points) => format!("it drew {points} points a frame, more than the {} a theme may", super::program::MOST_POINTS),
            super::program::Unread::Bad(why) => why.to_string(),
        })?;
        let (m, f) = self.at;
        let moment = &self.story[m];
        let frame = moment.frames[f];
        let counted = if frame.size == PHONE { &mut self.points.0 } else { &mut self.points.1 };
        if frame.size != PICTURE {
            *counted = (*counted).max(drawn.points);
        }
        self.asks_lift |= drawn.asked;
        let anchor = desk_of(frame, self.ground).4;
        self.ground = drawn.ground;
        let again = STEADY_ZOOMS.len();
        if moment.name == "steady" && f + again + 1 >= moment.frames.len() {
            // The desk's shapes as the program lays them out, as at the fitting zoom, before Caprice
            // zooms them: at the same moment, a theme that stays put draws the same each time, where
            // both drawings show the desk (zoomed in, less of it).
            if f + again + 1 == moment.frames.len() {
                self.held = Some(Held { shapes: drawn.desk.clone(), drawn: Drawing::of(layout(frame.size).0, 1.0, &drawn.desk) });
            } else if let Some(held) = &self.held {
                let back = |p: Pos2| anchor + (p - anchor) / frame.zoom;
                let shown = Rect::from_min_max(back(Pos2::ZERO), back(frame.size.to_pos2()));
                let moved = slid(held, shown, &drawn.desk);
                let around = match (self.slid.around, moved.around) {
                    (Some(a), Some(b)) => Some(a.union(b)),
                    (a, b) => a.or(b),
                };
                self.slid = Slid { pixels: self.slid.pixels + moved.pixels, around };
            }
        }
        if moment.picture && f + 1 == moment.frames.len() {
            // On the screen: the desk, the page and the bar where they lie at this zoom (the page
            // zoomed with the desk, about the theme's ground under its middle).
            let (area, page, bar, _) = layout(frame.size);
            let zoom = egui::emath::TSTransform::new(anchor.to_vec2() * (1.0 - frame.zoom), frame.zoom);
            let own: Vec<Shape> = drawn.desk.iter().cloned().map(|mut s| {
                s.transform(zoom);
                s
            }).chain(drawn.window.iter().cloned()).collect();
            match moment.name {
                "written" => self.edges.0 = edges_covered(frame.size, &own),
                "zoomed out" => self.edges.1 = edges_covered(frame.size, &own),
                _ => {}
            }
            let mut shapes = desk(area, &self.palette);
            shapes.extend(own);
            let whole = moment.name == BARE;
            if !whole {
                shapes.extend(paper(zoom.mul_rect(page)));
            }
            shapes.extend(drawn.over.iter().cloned().map(|mut s| {
                s.transform(zoom);
                s
            }));
            if frame.carried {
                let title = carried(page, 0.0).title;
                shapes.push(Shape::rect_filled(title.translate(drawn.offset * frame.zoom), 2.0, INK.gamma_multiply(0.8)));
            }
            if !whole {
                shapes.extend(docks(area, bar, &self.palette));
            }
            self.pictures.push(Picture { name: moment.name, image: rasterize(frame.size, shapes) });
        }
        Ok(())
    }
}

/// The desk a frame is laid out on, as `draw_program` tells it: the area, the page and the bar as
/// at the fitting zoom (the page in the middle, zoomed about the theme's ground under it), the size
/// of things, and the point zoomed about.
fn desk_of(frame: Frame, ground: f32) -> (Rect, Rect, Rect, f32, Pos2) {
    let (area, page, bar, unit) = layout(frame.size);
    let anchor = pos2(page.center().x, area.bottom() - (area.bottom() - bar.top()) - ground * unit);
    (area, page, bar, unit, anchor)
}

/// How much of the window's edges `shapes` cover on a window of `size`: points along its four sides,
/// each covered when what is drawn over it is all but opaque, blended as `fill` blends.
fn edges_covered(size: Vec2, shapes: &[Shape]) -> f32 {
    const ALONG: usize = 40;
    let (w, h) = (size.x - 0.5, size.y - 0.5);
    let step = |k: usize, n: f32| 0.5 + (n - 1.0) * (k as f32 + 0.5) / ALONG as f32;
    let points: Vec<Pos2> = (0..ALONG).flat_map(|k| {
        let (x, y) = (step(k, size.x), step(k, size.y));
        [pos2(0.5, y), pos2(w, y), pos2(x, 0.5), pos2(x, h)]
    }).collect();
    let mut alpha = vec![0f32; points.len()];
    let mut tess = epaint::Tessellator::new(1.0, epaint::TessellationOptions::default(), [1, 1], Vec::new());
    for shape in shapes {
        let mut mesh = epaint::Mesh::default();
        tess.tessellate_shape(shape.clone(), &mut mesh);
        for t in mesh.indices.as_chunks::<3>().0 {
            let v = t.map(|i| &mesh.vertices[i as usize]);
            let (lo, hi) = (v[0].pos.min(v[1].pos).min(v[2].pos), v[0].pos.max(v[1].pos).max(v[2].pos));
            // Only a triangle that reaches a side can cover a point on it.
            if lo.x > 0.5 && hi.x < w && lo.y > 0.5 && hi.y < h {
                continue;
            }
            let edge = |a: Pos2, b: Pos2, p: Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let area = edge(v[0].pos, v[1].pos, v[2].pos);
            if area.abs() < 1e-9 || !area.is_finite() {
                continue;
            }
            for (p, a) in points.iter().zip(alpha.iter_mut()) {
                if p.x < lo.x || p.x > hi.x || p.y < lo.y || p.y > hi.y {
                    continue;
                }
                let k = [edge(v[1].pos, v[2].pos, *p) / area, edge(v[2].pos, v[0].pos, *p) / area, edge(v[0].pos, v[1].pos, *p) / area];
                if k.iter().any(|&k| k < 0.0) {
                    continue;
                }
                let src = (k[0] * f32::from(v[0].color.a()) + k[1] * f32::from(v[1].color.a()) + k[2] * f32::from(v[2].color.a())) / 255.0;
                *a += src * (1.0 - *a);
            }
        }
    }
    alpha.iter().filter(|&&a| a >= 0.9).count() as f32 / points.len() as f32
}

/// The desk under a program's shapes: the palette's two colours, as `theme::paint_desk` lays them.
fn desk(area: Rect, p: &Palette) -> Vec<Shape> {
    let mut mesh = epaint::Mesh::default();
    for (at, c) in [(area.left_top(), p.desk_top), (area.right_top(), p.desk_top), (area.left_bottom(), p.desk), (area.right_bottom(), p.desk)] {
        mesh.colored_vertex(at, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    vec![Shape::mesh(mesh)]
}

/// The page, and lines of text on it, as a story's first page is seen from a little way off.
fn paper(page: Rect) -> Vec<Shape> {
    let mut shapes = vec![
        Shape::rect_filled(page.translate(vec2(0.0, 3.0)).expand(2.0), 3.0, Color32::from_black_alpha(60)),
        Shape::rect_filled(page, 1.0, PAPER),
    ];
    let (left, width, step) = (page.left() + page.width() * 0.12, page.width() * 0.76, page.height() * 0.032);
    let mut y = page.top() + page.height() * 0.22;
    let mut k = 0usize;
    while y < page.bottom() - page.height() * 0.1 {
        // Lines of their own lengths, a short one ending each paragraph.
        let w = if k % 7 == 6 { width * 0.45 } else { width * (0.92 + 0.08 * ((k * 37 % 11) as f32 / 10.0)) };
        shapes.push(Shape::rect_filled(Rect::from_min_size(pos2(left, y), vec2(w, step * 0.32)), 0.0, INK.gamma_multiply(0.22)));
        (y, k) = (y + step, k + 1);
    }
    shapes
}

/// The toolbar's dock and the page bar, in the palette's colours, empty.
fn docks(area: Rect, bar: Rect, p: &Palette) -> Vec<Shape> {
    let top = Rect::from_center_size(pos2(area.center().x, DOCK_GAP + DOCK_H / 2.0), vec2(bar.width(), DOCK_H));
    [top, bar].into_iter().map(|r| Shape::Rect(epaint::RectShape::new(r, DOCK_H / 2.0 - 8.0, p.dock, Stroke::new(1.0, p.dock_edge), egui::StrokeKind::Inside))).collect()
}

/// `shapes` filled into a picture of `size` points, a pixel each: tessellated as egui tessellates
/// the screen's (its feathered edges are the smoothing), each triangle's colours blended over what
/// lies under it as a GPU blends egui's, premultiplied.
pub fn rasterize(size: Vec2, shapes: Vec<Shape>) -> ColorImage {
    let (w, h) = (size.x as usize, size.y as usize);
    let mut px = vec![[0f32; 4]; w * h];
    paint(&mut px, w, h, shapes);
    let pixels = px.iter().map(|c| Color32::from_rgba_premultiplied(c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8)).collect();
    ColorImage::new([w, h], pixels)
}

/// `shapes` tessellated and filled into `px`, a picture `w` pixels wide and `h` high.
fn paint<P: Blend>(px: &mut [P], w: usize, h: usize, shapes: impl IntoIterator<Item = Shape>) {
    let mut tess = epaint::Tessellator::new(1.0, epaint::TessellationOptions::default(), [1, 1], Vec::new());
    for shape in shapes {
        let mut mesh = epaint::Mesh::default();
        tess.tessellate_shape(shape, &mut mesh);
        for t in mesh.indices.as_chunks::<3>().0 {
            let v = t.map(|i| &mesh.vertices[i as usize]);
            fill(px, w, h, v);
        }
    }
}

/// A pixel a colour is blended over, premultiplied: kept as numbers for the pictures, as bytes
/// where only two drawings are compared, which then take a quarter of the room.
trait Blend {
    fn blend(&mut self, src: [f32; 4]);
}

impl Blend for [f32; 4] {
    fn blend(&mut self, src: [f32; 4]) {
        let keep = 1.0 - src[3] / 255.0;
        for k in 0..4 {
            self[k] = (src[k] + self[k] * keep).min(255.0);
        }
    }
}

impl Blend for [u8; 4] {
    fn blend(&mut self, src: [f32; 4]) {
        let keep = 1.0 - src[3] / 255.0;
        for k in 0..4 {
            self[k] = (src[k] + f32::from(self[k]) * keep).round().min(255.0) as u8;
        }
    }
}

/// The desk's shapes as a program laid them out, filled `scale` times their size over black:
/// `region` of the desk, in bytes.
struct Drawing {
    region: Rect,
    scale: f32,
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
}

impl Drawing {
    fn of(region: Rect, scale: f32, shapes: &[Shape]) -> Drawing {
        let (w, h) = ((region.width() * scale).round() as usize, (region.height() * scale).round() as usize);
        let mut px = vec![[0u8; 4]; w * h];
        let zoom = egui::emath::TSTransform::new(-region.min.to_vec2() * scale, scale);
        paint(&mut px, w, h, shapes.iter().cloned().map(|mut s| {
            s.transform(zoom);
            s
        }));
        Drawing { region, scale, w, h, px }
    }

    /// The part of it within `r`, on its whole pixels; none if that is not two pixels each way.
    fn within(&self, r: Rect) -> Option<Drawing> {
        let at = |v: f32, lo: f32, n: usize, round: fn(f32) -> f32| (round((v - lo) * self.scale).max(0.0) as usize).min(n);
        let o = self.region;
        let (x0, y0) = (at(r.left(), o.left(), self.w, f32::ceil), at(r.top(), o.top(), self.h, f32::ceil));
        let (x1, y1) = (at(r.right(), o.left(), self.w, f32::floor), at(r.bottom(), o.top(), self.h, f32::floor));
        if x1 < x0 + 2 || y1 < y0 + 2 {
            return None;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let region = Rect::from_min_size(o.min + vec2(x0 as f32, y0 as f32) / self.scale, vec2(w as f32, h as f32) / self.scale);
        let px = (y0..y1).flat_map(|y| self.px[y * self.w + x0..y * self.w + x1].iter().copied()).collect();
        Some(Drawing { region, scale: self.scale, w, h, px })
    }
}

/// The steady moment's first drawing: its shapes, and the desk filled at its own size.
struct Held {
    shapes: Vec<Shape>,
    drawn: Drawing,
}

/// What moved between two drawings of the desk at the same moment, `before` (drawn already) and
/// `after`, each laid out as at the fitting zoom, compared where both show (`shown`, what `after`
/// showed). A fill stretched over what shows changes its colours gradually, which the difference
/// less its average nearby takes away (`sharp`); first at the desk's own size, to find where
/// anything differs sharply at all (in a theme that stays put, nowhere), then there alone at
/// `STEADY_SCALE`, where an outline drifting by less than a pixel leaves each pixel among its
/// neighbours' colours in the other drawing. What is left moved.
fn slid(before: &Held, shown: Rect, after: &[Shape]) -> Slid {
    const NEAR: f32 = 12.0;
    const SLACK: i32 = 4;
    // At the desk's own size, any sharp difference at all is looked at closely.
    const FOUND: f32 = 6.0;
    let Some(a) = before.drawn.within(shown) else { return Slid::default() };
    let b = Drawing::of(a.region, 1.0, after);
    let Some(found) = sharp(&a, &b, FOUND, None).1 else { return Slid::default() };
    // A little around it, for the averages nearby and the neighbours.
    let close = found.expand(8.0).intersect(a.region);
    let (a, b) = (Drawing::of(close, STEADY_SCALE, &before.shapes), Drawing::of(close, STEADY_SCALE, after));
    let (pixels, around) = sharp(&a, &b, NEAR, Some(SLACK));
    Slid { pixels, around }
}

/// The pixels of `b` (the same region as `a`, at the same size) far from `a`'s: their colour's
/// difference farther than `near` from the average difference around them, and, given a `slack`, not
/// among the colours of their neighbours in the other drawing, give or take it. How many, and the
/// rectangle around them on the desk.
fn sharp(a: &Drawing, b: &Drawing, near: f32, slack: Option<i32>) -> (usize, Option<Rect>) {
    const BLOCK: usize = 16;
    let (w, h, region, scale, (a, b)) = (a.w, a.h, a.region, a.scale, (&a.px, &b.px));
    let diff = |i: usize| -> [i32; 3] { std::array::from_fn(|k| i32::from(a[i][k]) - i32::from(b[i][k])) };
    // The average difference in each block of pixels, then between the blocks' middles at each pixel.
    let (bw, bh) = (w.div_ceil(BLOCK), h.div_ceil(BLOCK));
    let mut mean = vec![[0f32; 3]; bw * bh];
    for (j, m) in mean.iter_mut().enumerate() {
        let (bx, by) = (j % bw, j / bw);
        let (xs, ys) = (bx * BLOCK..((bx + 1) * BLOCK).min(w), by * BLOCK..((by + 1) * BLOCK).min(h));
        let n = (xs.len() * ys.len()) as f32;
        for y in ys {
            for x in xs.clone() {
                let d = diff(y * w + x);
                for k in 0..3 {
                    m[k] += d[k] as f32 / n;
                }
            }
        }
    }
    let around = |x: usize, y: usize| -> [f32; 3] {
        let f = |p: usize, n: usize| ((p as f32 + 0.5) / BLOCK as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let (fx, fy) = (f(x, bw), f(y, bh));
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        std::array::from_fn(|k| {
            let top = mean[y0 * bw + x0][k] * (1.0 - tx) + mean[y0 * bw + x1][k] * tx;
            let low = mean[y1 * bw + x0][k] * (1.0 - tx) + mean[y1 * bw + x1][k] * tx;
            top * (1.0 - ty) + low * ty
        })
    };
    // Whether the pixel at `x`, `y` in `one` has a colour among those around it in `other`.
    let among = |one: &[[u8; 4]], other: &[[u8; 4]], x: usize, y: usize, slack: i32| {
        let c = one[y * w + x];
        (0..3).all(|k| {
            let (mut lo, mut hi) = (255, 0);
            for yy in y.saturating_sub(1)..(y + 2).min(h) {
                for xx in x.saturating_sub(1)..(x + 2).min(w) {
                    let v = i32::from(other[yy * w + xx][k]);
                    (lo, hi) = (lo.min(v), hi.max(v));
                }
            }
            (lo - slack..=hi + slack).contains(&i32::from(c[k]))
        })
    };
    let (mut moved, mut lo, mut hi) = (0usize, (usize::MAX, usize::MAX), (0, 0));
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if a[i] == b[i] {
                continue;
            }
            let (d, m) = (diff(i), around(x, y));
            if (0..3).all(|k| (d[k] as f32 - m[k]).abs() <= near) || slack.is_some_and(|s| among(a, b, x, y, s) && among(b, a, x, y, s)) {
                continue;
            }
            moved += 1;
            (lo, hi) = ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x + 1), hi.1.max(y + 1)));
        }
    }
    let on_desk = |(x, y): (usize, usize)| region.min + vec2(x as f32, y as f32) / scale;
    (moved, (moved > 0).then(|| Rect::from_min_max(on_desk(lo), on_desk(hi))))
}

/// One triangle, every pixel whose middle lies in it, its colour between its corners'. The middle is
/// taken a hair off the pixel's centre, so a pixel on an edge two triangles share falls in just one
/// of them, never in both (a see-through shape blended twice there) or neither.
fn fill<P: Blend>(px: &mut [P], w: usize, h: usize, v: [&epaint::Vertex; 3]) {
    let [a, b, c] = v.map(|v| (f64::from(v.pos.x), f64::from(v.pos.y)));
    let edge = |(ax, ay): (f64, f64), (bx, by): (f64, f64), (px, py): (f64, f64)| (bx - ax) * (py - ay) - (by - ay) * (px - ax);
    let area = edge(a, b, c);
    if area.abs() < 1e-9 || !area.is_finite() {
        return;
    }
    let lo_hi = |k: fn(&(f64, f64)) -> f64| ([a, b, c].iter().map(k).fold(f64::MAX, f64::min), [a, b, c].iter().map(k).fold(f64::MIN, f64::max));
    let ((x0, x1), (y0, y1)) = (lo_hi(|p| p.0), lo_hi(|p| p.1));
    let span = |lo: f64, hi: f64, n: usize| ((lo.floor().max(0.0) as usize).min(n), (hi.ceil().max(0.0) as usize).min(n));
    let ((x0, x1), (y0, y1)) = (span(x0, x1, w), span(y0, y1, h));
    let colours = v.map(|v| v.color.to_array().map(f32::from));
    for y in y0..y1 {
        for x in x0..x1 {
            let p = (x as f64 + 0.5 + 1.0 / 1024.0, y as f64 + 0.5 + 1.0 / 4096.0);
            let (wa, wb, wc) = (edge(b, c, p) / area, edge(c, a, p) / area, edge(a, b, p) / area);
            if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                continue;
            }
            let (wa, wb, wc) = (wa as f32, wb as f32, wc as f32);
            let src: [f32; 4] = std::array::from_fn(|k| colours[0][k] * wa + colours[1][k] * wb + colours[2][k] * wc);
            px[y * w + x].blend(src);
        }
    }
}

/// A picture as a PNG's bytes.
pub fn png(image: &ColorImage) -> Option<Vec<u8>> {
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let img = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, rgba)?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    /// The preview of `module` played to its end (under the tests a theme answers at once).
    fn previewed(name: &str, module: &[u8], palette: Palette) -> Previewed {
        let ctx = egui::Context::default();
        let mut p = Preview::start(module, palette, Waker::new(&ctx), 0.0);
        let done = p.tend(0.0).expect("done at once, played natively");
        let previewed = done.unwrap_or_else(|why| panic!("{why}"));
        // CAPRICE_PREVIEW_OUT=dir keeps the pictures, to look at.
        if let Some(dir) = std::env::var_os("CAPRICE_PREVIEW_OUT") {
            for p in &previewed.pictures {
                std::fs::write(std::path::Path::new(&dir).join(format!("{name}-{}.png", p.name)), png(&p.image).unwrap()).unwrap();
            }
        }
        previewed
    }

    fn at(image: &ColorImage, p: Pos2) -> Color32 {
        image.pixels[p.y as usize * image.size[0] + p.x as usize]
    }

    #[test]
    fn a_theme_is_shown_its_story_and_pictured_as_the_desk_looks_with_the_page_on_it() {
        let snow = previewed("snow", include_bytes!("../../tests/fixtures/snow.wasm"), crate::theme::Theme::Snow.palette());
        let names: Vec<_> = snow.pictures.iter().map(|p| p.name).collect();
        assert_eq!(names, ["begun", "written", "finishing", "zoomed out", BARE], "no carrier: the Snow brings no titles");
        let written = &snow.pictures[1].image;
        assert_eq!(written.size, [1280, 800]);
        let (area, page, bar, _) = layout(PICTURE);
        // The page white, the dock in the Snow's colour, the sky at the top its dusk, and the snowy
        // hillside low on the desk beside the page, pale and blue.
        assert_eq!(at(written, page.center()), PAPER);
        assert!(at(written, pos2(bar.left() + 30.0, bar.center().y)) == crate::theme::Theme::Snow.palette().dock);
        let sky = at(written, pos2(20.0, 90.0));
        assert!(sky.b() > sky.r() && sky.b() < 140, "the dusk: {sky:?}");
        let hill = at(written, pos2(40.0, area.bottom() - 120.0));
        assert!(hill.b() > 200 && hill.b() >= hill.r(), "the hillside: {hill:?}");
        // And the same desk with no page on it, nor the bars: the hillside goes on where the page lay.
        let bare = &snow.pictures[4].image;
        assert_ne!(at(bare, page.center()), PAPER);
        assert_ne!(at(bare, pos2(bar.left() + 30.0, bar.center().y)), crate::theme::Theme::Snow.palette().dock);
        // Its points, counted on a phone's desk and a computer's.
        assert!(snow.phone_points > 1000 && snow.computer_points > snow.phone_points, "{} {}", snow.phone_points, snow.computer_points);
        // The fireflies bring titles: the carrier's moment is pictured too.
        let fireflies = previewed("fireflies", include_bytes!("../../tests/fixtures/fireflies.wasm"), crate::theme::Theme::Midnight.palette());
        assert_eq!(fireflies.pictures.iter().map(|p| p.name).collect::<Vec<_>>(), ["begun", "written", "finishing", "carried", "zoomed out", BARE]);
        // The Vineyard, the theme of the most points: within the limit everywhere.
        let vineyard = previewed("vineyard", include_bytes!("../../tests/fixtures/vineyard.wasm"), crate::theme::Theme::Vineyard.palette());
        assert!(vineyard.computer_points > vineyard.phone_points && vineyard.computer_points < super::super::program::MOST_POINTS);
        eprintln!("points: snow {} / {}, vineyard {} / {}", snow.phone_points, snow.computer_points, vineyard.phone_points, vineyard.computer_points);
        // Each fills what must reach the window's edges over what shows: none is told it stops short.
        for (name, p) in [("snow", &snow), ("fireflies", &fireflies), ("vineyard", &vineyard)] {
            assert_eq!(p.bare(), None, "{name}: {:?}", p.edges);
        }
        // And nothing of theirs slides as the writer zooms.
        for (name, p) in [("snow", &snow), ("fireflies", &fireflies), ("vineyard", &vineyard)] {
            assert_eq!(p.slid, Slid::default(), "{name}");
        }
    }

    /// A sky from top to foot, stretched over `area` (the desk at the fitting zoom) or over
    /// `shown` (what the window shows), and a tower placed from `area` beside the page.
    struct Sky<const SHOWN: bool>;

    impl<const SHOWN: bool> theme_sdk::Theme for Sky<SHOWN> {
        fn new() -> Self {
            Sky
        }

        fn frame(&mut self, input: &theme_sdk::Input, out: &mut theme_sdk::Out) {
            let r = if SHOWN { input.shown } else { input.area };
            let mut sky = theme_sdk::Mesh::new();
            for (at, c) in [(r.min, (40, 30, 90)), (theme_sdk::pt(r.max.x, r.min.y), (40, 30, 90)), (r.max, (200, 120, 140)), (theme_sdk::pt(r.min.x, r.max.y), (200, 120, 140))] {
                sky.vertex(at, theme_sdk::Color::rgb(c.0, c.1, c.2));
            }
            sky.triangle(0, 1, 2);
            sky.triangle(0, 2, 3);
            out.desk.mesh(sky);
            let u = input.unit;
            out.desk.circle(theme_sdk::pt(input.area.right() - 150.0 * u, input.ground.y - 80.0 * u), 30.0 * u, theme_sdk::Color::rgb(200, 200, 210));
        }
    }

    fn sky_module(memory: u32) -> Vec<u8> {
        wat::parse_str(format!(r#"(module (memory (export "memory") {memory} 512)
            (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 600000)
            (func (export "events") (param i32 i32))
            (func (export "frame") (param i32) (result i32) i32.const 1024)
            (func (export "cabi_post_frame") (param i32)))"#)).unwrap()
    }

    #[test]
    fn a_theme_whose_sky_stops_short_of_the_window_zoomed_out_is_told_so() {
        use crate::platform::program::{Stand, stand_in};
        // Stretched over the desk at the fitting zoom, a sky reaches the window's edges there and
        // leaves a frame of the desk's own colours around it zoomed out; over what shows, it reaches
        // them at every zoom.
        let (short, whole) = (sky_module(13), sky_module(14));
        stand_in(&short, || Stand::Theme(Box::new(theme_sdk::Running::<Sky<false>>::new())));
        stand_in(&whole, || Stand::Theme(Box::new(theme_sdk::Running::<Sky<true>>::new())));
        let palette = crate::theme::Theme::Midnight.palette();
        let short = previewed("sky-over-area", &short, palette);
        let zoomed = &short.pictures.iter().find(|p| p.name == "zoomed out").expect("pictured zoomed out").image;
        let plain = rasterize(PICTURE, desk(layout(PICTURE).0, &palette));
        assert_eq!(at(zoomed, pos2(4.0, 300.0)), at(&plain, pos2(4.0, 300.0)), "the desk's own colours at the edge");
        let said = short.bare().expect("told it stops short");
        assert!(said.contains("Input::shown") && said.contains("100% at the fitting zoom"), "{said}");
        let whole = previewed("sky-over-shown", &whole, palette);
        assert_eq!(whole.edges, (1.0, 1.0));
        assert_eq!(whole.bare(), None);
    }

    /// A sky stretched over what shows, and a cliff from the page's right edge to the window's: a
    /// world drawn only where it is seen beside the page, as the Lighthouse's cliff was.
    struct Cliff;

    impl theme_sdk::Theme for Cliff {
        fn new() -> Self {
            Cliff
        }

        fn frame(&mut self, input: &theme_sdk::Input, out: &mut theme_sdk::Out) {
            out.desk.solid(input.shown.corners(), theme_sdk::Color::rgb(40, 30, 90));
            let top = input.page.center().y - 20.0;
            let cliff = theme_sdk::Rect::from_min_max(theme_sdk::pt(input.page.right(), top), theme_sdk::pt(input.shown.right(), input.shown.bottom()));
            out.desk.solid(cliff.corners(), theme_sdk::Color::rgb(70, 60, 60));
        }
    }

    #[test]
    fn a_world_cut_off_under_the_page_is_pictured_without_the_page_where_the_cut_shows() {
        use crate::platform::program::{Stand, stand_in};
        let module = sky_module(17);
        stand_in(&module, || Stand::Theme(Box::new(theme_sdk::Running::<Cliff>::new())));
        let cliff = previewed("cliff", &module, crate::theme::Theme::Midnight.palette());
        let picture = |name: &str| &cliff.pictures.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("pictured {name}")).image;
        let (_, page, _, _) = layout(PICTURE);
        let (beside, under) = (pos2(page.right() + 40.0, page.center().y + 40.0), pos2(page.right() - 40.0, page.center().y + 40.0));
        let rock = Color32::from_rgb(70, 60, 60);
        // With the page on the desk, the cliff is seen only beside it, and nothing looks amiss.
        let written = picture("written");
        assert_eq!((at(written, beside), at(written, under)), (rock, PAPER));
        // Without it, the sky shows where the cliff stops at the page's edge.
        let bare = picture(BARE);
        assert_eq!(at(bare, beside), rock);
        assert_eq!(at(bare, under), Color32::from_rgb(40, 30, 90), "the cut in plain sight");
        // It is the last picture, and the gallery's picture is still three chapters written.
        assert_eq!(cliff.pictures.last().map(|p| p.name), Some(BARE));
        assert_eq!(cliff.gallery_picture().map(|p| p.size), Some(shrunk(written, crate::bundle::PREVIEW_WIDTH).size));
    }

    /// A sky stretched over what shows, its colours changing half way down it (so they change as the
    /// writer zooms, gradually), a field along the foot, and grass on the field every four units:
    /// from where `shown` begins (it streams sideways with the zoom), or stepped from a fixed point.
    struct Field<const FROM_SHOWN: bool>;

    impl<const FROM_SHOWN: bool> theme_sdk::Theme for Field<FROM_SHOWN> {
        fn new() -> Self {
            Field
        }

        fn frame(&mut self, input: &theme_sdk::Input, out: &mut theme_sdk::Out) {
            use theme_sdk::{Color, pt};
            let (s, u, foot) = (input.shown, input.unit, input.ground.y);
            let mut sky = theme_sdk::Mesh::new();
            for (y, c) in [(s.top(), Color::rgb(20, 20, 60)), ((s.top() + foot) / 2.0, Color::rgb(90, 70, 130)), (foot, Color::rgb(220, 150, 120))] {
                sky.vertex(pt(s.left(), y), c);
                sky.vertex(pt(s.right(), y), c);
            }
            for a in [0, 2] {
                sky.triangle(a, a + 1, a + 3);
                sky.triangle(a, a + 3, a + 2);
            }
            out.desk.mesh(sky);
            let field = theme_sdk::Rect::from_min_max(pt(s.left(), foot), pt(s.right(), s.bottom().max(foot + 1.0)));
            out.desk.solid(field.corners(), Color::rgb(40, 70, 40));
            let step = 4.0 * u;
            let mut x = if FROM_SHOWN { s.left() } else { input.area.left() - ((input.area.left() - s.left()) / step).ceil() * step };
            while x < s.right() {
                out.desk.line(pt(x, foot), pt(x + u, foot - 7.0 * u), 1.2 * u, Color::rgb(150, 220, 110));
                x += step;
            }
        }
    }

    #[test]
    fn a_theme_whose_grass_is_laid_from_what_shows_is_told_that_it_slides_as_the_writer_zooms() {
        use crate::platform::program::{Stand, stand_in};
        let (sliding, fixed) = (sky_module(15), sky_module(16));
        stand_in(&sliding, || Stand::Theme(Box::new(theme_sdk::Running::<Field<true>>::new())));
        stand_in(&fixed, || Stand::Theme(Box::new(theme_sdk::Running::<Field<false>>::new())));
        let palette = crate::theme::Theme::Midnight.palette();
        // Stepped from a fixed point, the grass stays where it is, and the sky's colours, moving
        // gradually with the zoom, are not counted.
        let fixed = previewed("grass-fixed", &fixed, palette);
        assert_eq!(fixed.slid, Slid::default());
        assert_eq!(fixed.slides(), None);
        // From where what shows begins, every blade is somewhere else: along the field, the width
        // of the window.
        let sliding = previewed("grass-from-shown", &sliding, palette);
        let around = sliding.slid.around.expect("it moved");
        let ground = desk_of(Frame { time: 0.0, size: PICTURE, zoom: 1.0, carried: false }, 0.0).4.y;
        assert!(around.width() > PICTURE.x * 0.9 && around.top() > ground - 10.0 && around.bottom() < ground + 2.0, "{around:?}");
        let said = sliding.slides().expect("told it slides");
        assert!(said.contains("slides as the writer zooms") && said.contains("`Input::shown`") && said.contains("grass on a ground"), "{said}");
    }
}
