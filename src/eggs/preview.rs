//! A theme program's pictures, made off screen (R36, the plan's step 5): the program played through
//! a story told in a few moments (begun, three chapters written, a chapter finishing, the carrier on
//! its way), each frame laid out as Caprice lays out the desk at the fitting zoom, read as Caprice
//! reads it (`program::read`), tessellated by egui as the screen's shapes are, and filled in here,
//! triangle by triangle, into a picture: the desk in the theme's colours, the program's shapes, the
//! page and the bars. They are what Claude is shown of a theme it wrote, and a theme's picture in
//! the gallery. The same frames count a theme's points where a writer reads: on a phone's desk and
//! on a computer's, zoomed out too.
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
/// Frames a second the story is told at.
const FPS: f64 = 10.0;

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
/// then the same desk on a phone and on a computer, there zoomed out too, to count its points.
fn story() -> Vec<Moment> {
    let at = |from: f64, seconds: f64, size: Vec2, zoom: f32, carried: bool| -> Vec<Frame> {
        (0..=(seconds * FPS) as usize).map(|k| Frame { time: from + k as f64 / FPS, size, zoom, carried }).collect()
    };
    let written = vec![14, 10, 6];
    let finishing = vec![14, 10, 6, 1];
    vec![
        Moment { name: "begun", picture: true, garden: vec![1], events: vec![Event::Fresh], frames: at(0.0, 2.0, PICTURE, 1.0, false) },
        Moment { name: "written", picture: true, garden: written.clone(), events: vec![Event::Fresh], frames: at(10.0, 2.0, PICTURE, 1.0, false) },
        Moment {
            name: "finishing", picture: true, garden: finishing.clone(),
            events: vec![Event::Begun(3), Event::Grown { chapter: 3, pages: 1 }], frames: at(20.0, 2.0, PICTURE, 1.0, false),
        },
        Moment { name: "carried", picture: true, garden: finishing.clone(), events: vec![], frames: at(30.0, 1.5, PICTURE, 1.0, true) },
        Moment { name: "phone", picture: false, garden: written.clone(), events: vec![Event::Fresh], frames: at(40.0, 0.5, PHONE, 1.0, false) },
        Moment { name: "computer", picture: false, garden: written.clone(), events: vec![Event::Fresh], frames: at(50.0, 0.5, COMPUTER, 1.0, false) },
        Moment { name: "computer zoomed out", picture: false, garden: written, events: vec![Event::Fresh], frames: at(60.0, 0.5, COMPUTER, ZOOMED_OUT, false) },
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
/// a computer's (zoomed out included), which Claude is told with the pictures (the plan's step 5).
#[cfg_attr(not(test), allow(dead_code))]
pub struct Previewed {
    pub pictures: Vec<Picture>,
    pub phone_points: usize,
    pub computer_points: usize,
}

impl Previewed {
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
    done: Option<Result<Previewed, String>>,
}

impl Preview {
    /// `module` (checked already) shown its story, drawn in `palette`'s colours.
    pub fn start(module: &[u8], palette: Palette, wake: Waker, now: f64) -> Preview {
        Preview {
            program: Program::load(module, wake, now), palette, story: story(), at: (0, 0), asked: false,
            ground: 0.0, asks_lift: false, pictures: Vec::new(), points: (0, 0), done: None,
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
                    self.done = Some(Ok(Previewed { pictures: std::mem::take(&mut self.pictures), phone_points, computer_points }));
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
        if moment.picture && f + 1 == moment.frames.len() {
            // On the screen: the desk, the page and the bar where they lie at this zoom.
            let (area, page, bar, _) = layout(frame.size);
            let zoom = egui::emath::TSTransform::new(anchor.to_vec2() * (1.0 - frame.zoom), frame.zoom);
            let mut shapes = desk(area, &self.palette);
            shapes.extend(drawn.desk.iter().cloned().map(|mut s| {
                s.transform(zoom);
                s
            }));
            shapes.extend(drawn.window.iter().cloned());
            shapes.extend(paper(page));
            shapes.extend(drawn.over.iter().cloned().map(|mut s| {
                s.transform(zoom);
                s
            }));
            if frame.carried {
                let title = carried(page, 0.0).title;
                shapes.push(Shape::rect_filled(title.translate(drawn.offset * frame.zoom), 2.0, INK.gamma_multiply(0.8)));
            }
            shapes.extend(docks(area, bar, &self.palette));
            self.pictures.push(Picture { name: moment.name, image: rasterize(frame.size, shapes) });
        }
        Ok(())
    }
}

/// The desk a frame is laid out on, unzoomed about the theme's ground as `draw_program` does: the
/// area, the page, the bar, the size of things, and the point zoomed about.
fn desk_of(frame: Frame, ground: f32) -> (Rect, Rect, Rect, f32, Pos2) {
    let (area, page, bar, unit) = layout(frame.size);
    let anchor = pos2(page.center().x, area.bottom() - (area.bottom() - bar.top()) - ground * unit);
    let z = frame.zoom;
    let unzoom = |r: Rect| Rect::from_min_max(anchor + (r.min - anchor) / z, anchor + (r.max - anchor) / z);
    (unzoom(area), unzoom(page), unzoom(bar), unit, anchor)
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
    let mut tess = epaint::Tessellator::new(1.0, epaint::TessellationOptions::default(), [1, 1], Vec::new());
    for shape in shapes {
        let mut mesh = epaint::Mesh::default();
        tess.tessellate_shape(shape, &mut mesh);
        for t in mesh.indices.as_chunks::<3>().0 {
            let v = t.map(|i| &mesh.vertices[i as usize]);
            fill(&mut px, w, h, v);
        }
    }
    let pixels = px.iter().map(|c| Color32::from_rgba_premultiplied(c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8)).collect();
    ColorImage::new([w, h], pixels)
}

/// One triangle, every pixel whose middle lies in it, its colour between its corners'. The middle is
/// taken a hair off the pixel's centre, so a pixel on an edge two triangles share falls in just one
/// of them, never in both (a see-through shape blended twice there) or neither.
fn fill(px: &mut [[f32; 4]], w: usize, h: usize, v: [&epaint::Vertex; 3]) {
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
            let dst = &mut px[y * w + x];
            let keep = 1.0 - src[3] / 255.0;
            for k in 0..4 {
                dst[k] = (src[k] + dst[k] * keep).min(255.0);
            }
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
        assert_eq!(names, ["begun", "written", "finishing"], "no carrier: the Snow brings no titles");
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
        // Its points, counted on a phone's desk and a computer's.
        assert!(snow.phone_points > 1000 && snow.computer_points > snow.phone_points, "{} {}", snow.phone_points, snow.computer_points);
        // The fireflies bring titles: the carrier's moment is pictured too.
        let fireflies = previewed("fireflies", include_bytes!("../../tests/fixtures/fireflies.wasm"), crate::theme::Theme::Midnight.palette());
        assert_eq!(fireflies.pictures.iter().map(|p| p.name).collect::<Vec<_>>(), ["begun", "written", "finishing", "carried"]);
        // The Vineyard, the theme of the most points: within the limit everywhere.
        let vineyard = previewed("vineyard", include_bytes!("../../tests/fixtures/vineyard.wasm"), crate::theme::Theme::Vineyard.palette());
        assert!(vineyard.computer_points > vineyard.phone_points && vineyard.computer_points < super::super::program::MOST_POINTS);
        eprintln!("points: snow {} / {}, vineyard {} / {}", snow.phone_points, snow.computer_points, vineyard.phone_points, vineyard.computer_points);
    }
}
