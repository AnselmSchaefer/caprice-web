//! Everything a theme program needs to draw on Caprice's desk (ARCHITECTURE.md R36). A theme is
//! one Rust file: a type that is a `Theme`, and `theme!` naming it. Caprice tells it what happened
//! (`Theme::event`: the story just shown, a chapter grown or begun) and asks it for a frame
//! (`Theme::frame`); it answers with shapes, behind the pages and, while a title is carried, over
//! the page. It may ask Caprice a few things (`host`), never change anything.
//!
//! What it draws are egui's shapes, built as egui builds them: circles, convex polygons, lines,
//! paths and meshes of coloured triangles. No text, no pictures. It lays everything out for the
//! desk at the fitting zoom (`Input::area`, `Input::page`); Caprice zooms it all at once, about the
//! theme's ground (`Out::ground`), which stays where it is on the screen. What is in the window
//! rather than on the desk (snow falling) goes in `Out::window`, laid out on `Input::window` and
//! never zoomed.
//!
//! Polygons cost the most to draw (egui smooths every edge, doubling its points): a gradient or a
//! field of many small fills is cheaper as one mesh. A frame may hold 40 000 points (a polygon's
//! points and a mesh's vertices; a circle or a line is one), twice what the Magic Snow draws; over
//! that it is not drawn, and a theme over it frame after frame is stopped.
//!
//! The crate needs nothing but `std`, and builds with the bindings it carries (made ahead of time
//! from `wit/theme.wit`), so a theme's build is this crate and one file.

#[doc(hidden)]
#[allow(clippy::all, dead_code, unused_imports, unused_unsafe)]
#[rustfmt::skip]
pub mod bindings;
#[doc(hidden)]
pub mod rt;

use std::f32::consts::PI;
use std::ops::{Add, Mul, Neg, Sub};

pub use bindings::caprice::theme::types::{Carrier, Circle, Event, Line, Mesh, Pages, Path, Point, Polygon, Pose, Rect, Shape, Vertex};

/// What a theme may ask of Caprice, at any time.
pub mod host {
    pub use crate::bindings::caprice::theme::host::{ask_lift, garden, page_count, story_seed};
}

/// A theme: made once when it is loaded, told what happened, asked for frames.
pub trait Theme: 'static {
    fn new() -> Self;
    /// What happened since the last frame, in order, before it is asked for the next.
    fn event(&mut self, event: &Event) {
        let _ = event;
    }
    /// The frame for `input`, drawn into `out`.
    fn frame(&mut self, input: &Input, out: &mut Out);
}

/// Makes the type a theme: `theme!(Fireflies);` once, in the theme's file.
#[macro_export]
macro_rules! theme {
    ($ty:ty) => {
        /// The canonical ABI's allocator, exported for Caprice to give the theme its input.
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn cabi_realloc(old: *mut u8, old_len: usize, align: usize, new_len: usize) -> *mut u8 {
            unsafe { $crate::rt::realloc(old, old_len, align, new_len) }
        }

        const _: () = {
            thread_local!(static THEME: ::std::cell::RefCell<$crate::Running<$ty>> = ::std::cell::RefCell::new($crate::Running::new()));

            struct Exports;

            impl $crate::bindings::Guest for Exports {
                fn events(events: ::std::vec::Vec<$crate::Event>) {
                    THEME.with_borrow_mut(|t| t.events(&events));
                }

                fn frame(input: $crate::bindings::FrameInput) -> $crate::bindings::FrameOutput {
                    THEME.with_borrow_mut(|t| t.frame(input))
                }
            }

            $crate::__export_theme_impl!(Exports with_types_in $crate::bindings);
        };
    };
}

/// A theme as it runs, and the output it draws into, kept between frames so its lists keep their
/// room. Used by `theme!`.
#[doc(hidden)]
pub struct Running<T: Theme> {
    theme: Option<T>,
    out: Out,
}

impl<T: Theme> Default for Running<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Theme> Running<T> {
    pub fn new() -> Self {
        Running { theme: None, out: Out::default() }
    }

    pub fn events(&mut self, events: &[Event]) {
        let theme = self.theme.get_or_insert_with(T::new);
        for e in events {
            theme.event(e);
        }
    }

    pub fn frame(&mut self, input: bindings::FrameInput) -> bindings::FrameOutput {
        let input = Input::from(input);
        let out = &mut self.out;
        out.desk.0.clear();
        out.window.0.clear();
        out.over.0.clear();
        (out.next, out.title_offset, out.ground) = (0, Point::ZERO, 0.0);
        let theme = self.theme.get_or_insert_with(T::new);
        theme.frame(&input, &mut self.out);
        let out = &mut self.out;
        // The lists go to Caprice, which reads them and has them freed after (`cabi_post_frame`).
        bindings::FrameOutput {
            desk: std::mem::take(&mut out.desk.0),
            window: std::mem::take(&mut out.window.0),
            over: std::mem::take(&mut out.over.0),
            next: out.next,
            title_offset: out.title_offset,
            ground: out.ground,
        }
    }
}

/// A frame's input: the time, the desk, and the carrier if a title is being carried.
#[derive(Clone, Copy, Debug)]
pub struct Input {
    /// Seconds, from an arbitrary start; never an `f32` (it would stutter after a day).
    pub time: f64,
    /// The desk and the page as they lie at the fitting zoom.
    pub area: Rect,
    pub page: Rect,
    /// The page bar, if it shows: anything growing up from the desk's foot keeps above it.
    pub bar: Option<Rect>,
    /// How far the writer has zoomed in on the page (1 at the fitting zoom). Caprice zooms what is
    /// drawn itself, about `ground`: a theme lays out as at 1.
    pub zoom: f32,
    /// The size of things: 1 on a desk 900 high, followed over a moment when the window changes.
    pub unit: f32,
    /// The desk as it lies on the screen, at every zoom: what `Out::window` is laid out on.
    pub window: Rect,
    /// The point Caprice zooms about: under the page's middle, as high as the theme's ground
    /// (`Out::ground`). It stays where it is on the screen, so what stands there stays too.
    pub ground: Point,
    pub carrier: Option<Carrier>,
}

impl From<bindings::FrameInput> for Input {
    fn from(i: bindings::FrameInput) -> Input {
        let d = i.desk;
        Input { time: i.time, area: d.area, page: d.page, bar: d.bar, zoom: d.zoom, unit: d.unit, window: d.window, ground: d.ground, carrier: i.carrier }
    }
}

/// What a frame draws: shapes behind every page (on the desk, zoomed; then in the window, not),
/// shapes over the settled page while a title is carried, when the next frame is wanted, how far
/// the carried title is from its place, and where the theme's ground is.
#[derive(Clone, Debug, Default)]
pub struct Out {
    pub desk: Layer,
    pub window: Layer,
    pub over: Layer,
    /// Milliseconds until the next frame is wanted: 0 (the default) while anything moves.
    pub next: u32,
    pub title_offset: Point,
    /// How far above the foot of the desk (the top of the page bar when it lies along it) the
    /// theme's ground is, in units (0, the default, is the foot itself). Caprice zooms about it
    /// from the next frame on, and says where it is in `Input::ground`.
    pub ground: f32,
}

/// Shapes in the order they are drawn, the first furthest back.
#[derive(Clone, Debug, Default)]
pub struct Layer(pub Vec<Shape>);

impl Layer {
    pub fn circle(&mut self, at: Point, r: f32, color: Color) {
        self.0.push(Shape::Circle(Circle { at, r, color: color.0 }));
    }

    /// A filled convex polygon (egui fills only convex ones), its points in order round it.
    pub fn polygon(&mut self, points: Vec<Point>, color: Color) {
        self.0.push(Shape::Polygon(Polygon { points, color: color.0 }));
    }

    pub fn line(&mut self, a: Point, b: Point, width: f32, color: Color) {
        self.0.push(Shape::Line(Line { a, b, width, color: color.0 }));
    }

    /// A line through `points`, joined where it bends, and back to the first if `closed`: unlike
    /// lines one after another, it shows no knots where they meet when it is see-through.
    pub fn path(&mut self, points: Vec<Point>, closed: bool, width: f32, color: Color) {
        self.0.push(Shape::Path(Path { points, closed, width, color: color.0 }));
    }

    /// A filled convex shape that egui draws cleanly, as Caprice's own themes fill one. egui
    /// pushes a shape's soft edge out along each corner, the further the sharper the corner: a
    /// corner folded nearly flat draws a spike right across the window. So points on top of each
    /// other are merged, corners sharper than about 25 degrees cut by a hair, and a sliver too thin
    /// to see is left out.
    pub fn solid(&mut self, points: Vec<Point>, color: Color) {
        let mut ps: Vec<Point> = Vec::new();
        for p in points {
            if ps.last().is_none_or(|q| q.distance(p) > 0.05) {
                ps.push(p);
            }
        }
        while ps.len() > 1 && ps[0].distance(ps[ps.len() - 1]) <= 0.05 {
            ps.pop();
        }
        let n = ps.len();
        if n < 3 {
            return;
        }
        let area = (0..n).map(|i| ps[i].y * ps[(i + 1) % n].x + -ps[i].x * ps[(i + 1) % n].y).sum::<f32>().abs() / 2.0;
        let longest = (0..n).map(|i| ps[i].distance(ps[(i + 1) % n])).fold(0.0, f32::max);
        if area / longest < 0.3 {
            return;
        }
        let mut out = Vec::with_capacity(2 * n);
        for i in 0..n {
            let (before, p, after) = (ps[(i + n - 1) % n], ps[i], ps[(i + 1) % n]);
            let (a, b) = ((before - p).normalized(), (after - p).normalized());
            if a.dot(b) > 0.9 {
                let cut = 0.4f32.min(p.distance(before) * 0.3).min(p.distance(after) * 0.3);
                out.extend([p + a * cut, p + b * cut]);
            } else {
                out.push(p);
            }
        }
        self.polygon(out, color);
    }

    /// An oval around `c`, `a` long along `along` (a way of length 1) and `b` across it.
    pub fn oval(&mut self, c: Point, along: Point, a: f32, b: f32, color: Color) {
        let across = pt(-along.y, along.x);
        let points = (0..20).map(|k| {
            let r = k as f32 / 20.0 * 2.0 * PI;
            c + along * a * r.cos() + across * b * r.sin()
        });
        self.solid(points.collect(), color);
    }

    /// Light about `at`, `r` across and `h` high, `alpha` at its middle and fading smoothly to
    /// nothing at its edge: one mesh, so it shows no rings (Caprice's own themes' glow).
    pub fn radial(&mut self, at: Point, r: f32, h: f32, color: Color, alpha: f32) {
        let mut mesh = Mesh::new();
        mesh.vertex(at, color.see_through(alpha));
        let rings = [(0.35, 0.55), (0.7, 0.18), (1.0, 0.0)];
        let n = 32;
        for &(f, a) in &rings {
            for i in 0..n {
                let ang = i as f32 / n as f32 * 2.0 * PI;
                mesh.vertex(at + pt(ang.cos() * r * f, ang.sin() * h * f), color.see_through(alpha * a));
            }
        }
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.triangle(0, 1 + i, 1 + j);
            for ring in 0..rings.len() as u32 - 1 {
                let (a, b) = (1 + ring * n, 1 + (ring + 1) * n);
                mesh.triangle(a + i, b + i, a + j);
                mesh.triangle(a + j, b + i, b + j);
            }
        }
        self.mesh(mesh);
    }

    /// Another layer's shapes after this one's.
    pub fn append(&mut self, other: Layer) {
        self.0.extend(other.0);
    }

    pub fn mesh(&mut self, mesh: Mesh) {
        if !mesh.indices.is_empty() {
            self.0.push(Shape::Mesh(mesh));
        }
    }

    /// How many points the layer holds, as Caprice counts them against its limit.
    pub fn points(&self) -> usize {
        self.0.iter().map(|s| match s {
            Shape::Polygon(p) => p.points.len(),
            Shape::Mesh(m) => m.vertices.len(),
            _ => 1,
        }).sum()
    }
}

impl Mesh {
    pub fn new() -> Mesh {
        Mesh { vertices: Vec::new(), indices: Vec::new() }
    }

    /// A vertex added, and its index.
    pub fn vertex(&mut self, at: Point, color: Color) -> u32 {
        self.vertices.push(Vertex { at, color: color.0 });
        self.vertices.len() as u32 - 1
    }

    pub fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Mesh::new()
    }
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, to: Point) -> f32 {
        (self - to).length()
    }

    /// The same way, of length 1 (unless it is none).
    pub fn normalized(self) -> Point {
        let len = self.length();
        if len <= 0.0 { self } else { pt(self.x / len, self.y / len) }
    }

    pub fn dot(self, o: Point) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// The way `angle` radians round from the right, clockwise on the screen (egui's `Vec2::angled`).
    pub fn angled(angle: f32) -> Point {
        let (sin, cos) = angle.sin_cos();
        Point { x: cos, y: sin }
    }

    /// From here to `to`, `t` of the way.
    pub fn lerp(self, to: Point, t: f32) -> Point {
        self + (to - self) * t
    }
}

impl Default for Point {
    fn default() -> Self {
        Point::ZERO
    }
}

/// A point: `pt(x, y)`.
pub const fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

impl Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        pt(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        pt(self.x - o.x, self.y - o.y)
    }
}

impl Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        pt(-self.x, -self.y)
    }
}

impl Mul<f32> for Point {
    type Output = Point;
    fn mul(self, s: f32) -> Point {
        pt(self.x * s, self.y * s)
    }
}

impl Rect {
    pub const fn from_min_max(min: Point, max: Point) -> Rect {
        Rect { min, max }
    }

    pub fn left(&self) -> f32 {
        self.min.x
    }
    pub fn right(&self) -> f32 {
        self.max.x
    }
    pub fn top(&self) -> f32 {
        self.min.y
    }
    pub fn bottom(&self) -> f32 {
        self.max.y
    }
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    pub fn center(&self) -> Point {
        pt((self.min.x + self.max.x) / 2.0, (self.min.y + self.max.y) / 2.0)
    }
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn contains_rect(&self, r: Rect) -> bool {
        self.contains(r.min) && self.contains(r.max)
    }
    /// Whether the two overlap across (their spans of x meet).
    pub fn overlaps_across(&self, r: Rect) -> bool {
        self.min.x <= r.max.x && r.min.x <= self.max.x
    }
    /// Its four corners, round from the top left: to fill it as a polygon.
    pub fn corners(&self) -> Vec<Point> {
        vec![self.min, pt(self.max.x, self.min.y), self.max, pt(self.min.x, self.max.y)]
    }
}

/// A colour as egui keeps it: sRGB, premultiplied by its alpha, red in the lowest byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub const TRANSPARENT: Color = Color(0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::premultiplied(r, g, b, 255)
    }

    pub const fn premultiplied(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color(u32::from_le_bytes([r, g, b, a]))
    }

    /// The parts, premultiplied: red, green, blue, alpha.
    pub const fn parts(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// Fainter by `factor` (0 to 1): egui's `Color32::gamma_multiply`, which every built-in theme
    /// fades with.
    pub fn gamma_multiply(self, factor: f32) -> Color {
        let f = factor.clamp(0.0, 1.0);
        let [r, g, b, a] = self.parts();
        let m = |c: u8| (c as f32 * f + 0.5) as u8;
        Color::premultiplied(m(r), m(g), m(b), m(a))
    }

    /// From this to `to`, `t` of the way (egui's `lerp_to_gamma`, which Caprice's own themes mix with).
    pub fn lerp(self, to: Color, t: f32) -> Color {
        let (a, b, t) = (self.parts(), to.parts(), t.clamp(0.0, 1.0));
        let m = |i: usize| ((1.0 - t) * a[i] as f32 + t * b[i] as f32 + 0.5) as u8;
        Color::premultiplied(m(0), m(1), m(2), m(3))
    }

    /// This colour, made see-through by `alpha` (0 to 1) however see-through it was: as Caprice's
    /// own themes make their glows and shadows (egui's `from_rgba_unmultiplied`).
    pub fn see_through(self, alpha: f32) -> Color {
        let [r, g, b, _] = self.parts();
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let m = |c: u8| {
            let p = c as u16 * a as u16 + 128;
            ((p + (p >> 8)) >> 8) as u8
        };
        match a {
            0 => Color::TRANSPARENT,
            255 => Color::rgb(r, g, b),
            a => Color::premultiplied(m(r), m(g), m(b), a),
        }
    }
}

/// A little generator of numbers, the same for the same seed: Caprice's own (`eggs::Seeds`), so
/// a theme ported from Caprice draws as it did, seeded by the story's seed (`host::story_seed`)
/// and a number of the theme's choosing.
#[derive(Clone, Debug)]
pub struct Seeds(u64);

impl Seeds {
    pub fn of(seed: u64, k: usize) -> Seeds {
        // splitmix64: every seed and number gives a number of its own, the same on every machine.
        let mut z = seed ^ (k as u64).wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Seeds((z ^ (z >> 31)) | 1)
    }

    /// From the generator's state itself: for the test that it is Caprice's.
    #[doc(hidden)]
    pub fn from_state(state: u64) -> Seeds {
        Seeds(state)
    }

    /// The generator's state: for the tests that seed Caprice's own themes as a ported one is.
    #[doc(hidden)]
    pub fn state(&self) -> u64 {
        self.0
    }

    /// A number from `lo` to `hi`.
    pub fn next(&mut self, lo: f32, hi: f32) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        lo + (hi - lo) * (self.0 % 10_000) as f32 / 10_000.0
    }
}

/// `s` from 0 to 1 eased in and out (Caprice's `ease`).
pub fn ease(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

/// How far `s` has gone from `a` to `b`, eased (Caprice's `between`).
pub fn between(s: f32, a: f32, b: f32) -> f32 {
    ease((s - a) / (b - a))
}

/// `shown` gone `dt` seconds of the way to `target`: most of it in a third of a second, never past
/// it, and there once within a hair (Caprice's `toward`).
pub fn toward(shown: f32, target: f32, dt: f32) -> f32 {
    let next = shown + (target - shown) * (1.0 - (-dt / 0.1).exp());
    if (next - target).abs() < 1e-3 { target } else { next }
}

/// What a theme growing a thing a chapter keeps, as Caprice's own themes do (`eggs::Tended`): the
/// story and the chapter growing, how far it has grown (a step a page, the last perhaps in part),
/// the one before finishing when the next chapter begins, and the place each chapter's stands in,
/// given when it is first drawn and kept, so a window made narrower moves none.
#[derive(Clone, Debug, Default)]
pub struct Tended {
    shown_for: Option<(u64, usize)>,
    places: Vec<Option<usize>>,
    grown: f32,
    grown_at: f64,
    finish: Option<(f64, usize)>,
}

impl Tended {
    /// Brought up to `now` for the story `seed` whose chapters have had `garden` pages: the newest
    /// chapter's grows to as many steps, `most` at most, `step` seconds each; the one before it
    /// finishes over `over` seconds once the next begins. Whether anything is on the move.
    pub fn tend(&mut self, seed: u64, garden: &[u32], most: usize, step: f32, over: f32, now: f64) -> bool {
        let newest = garden.len().max(1) - 1;
        let target = garden.get(newest).map_or(0, |&p| (p as usize).min(most)) as f32;
        if self.shown_for.is_none_or(|(was, _)| was != seed) {
            // A story just shown comes with it all grown.
            (self.shown_for, self.grown, self.finish) = (Some((seed, newest)), target, None);
            self.places.clear();
        } else if let Some(was) = self.shown_for.map(|f| f.1).filter(|&was| was < newest) {
            (self.shown_for, self.grown, self.finish) = (Some((seed, newest)), 0.0, Some((now, was)));
        }
        if self.finish.is_some_and(|(at, _)| now - at >= over as f64) {
            self.finish = None;
        }
        let dt = (now - self.grown_at).clamp(0.0, 0.1) as f32;
        self.grown_at = now;
        if self.grown < target {
            self.grown = (self.grown + (target - self.grown).max(3.0) / 3.0 / step * dt).min(target);
        }
        self.finish.is_some() || self.grown < target
    }

    /// How many steps chapter `k`'s has grown, the newest's perhaps in part.
    pub fn grown(&self, garden: &[u32], k: usize, most: usize) -> f32 {
        if k + 1 >= garden.len().max(1) { self.grown } else { garden.get(k).map_or(0, |&p| (p as usize).min(most)) as f32 }
    }

    /// The chapter finishing, if one is, and how far, from 0 to 1 over `over` seconds.
    pub fn finishing(&self, now: f64, over: f32) -> Option<(usize, f32)> {
        self.finish.map(|(at, k)| (k, ((now - at) as f32 / over).clamp(0.0, 1.0)))
    }

    /// The place chapter `k`'s stands in, of `count`: the first free one `in_view` when it is first
    /// asked for, else the first free.
    pub fn place(&mut self, k: usize, count: usize, in_view: impl Fn(usize) -> bool) -> usize {
        self.places.resize(self.places.len().max(k + 1), None);
        if self.places[k].is_none() {
            let free = |p: &usize| !self.places.contains(&Some(*p));
            self.places[k] = Some((0..count).filter(free).find(|&p| in_view(p)).or_else(|| (0..count).find(free)).unwrap_or(0));
        }
        self.places[k].unwrap_or(0).min(count.max(1) - 1)
    }

    /// Forgotten (Caprice said `fresh`): the next frame shows it all as it stands.
    pub fn forget(&mut self) {
        (self.shown_for, self.finish) = (None, None);
    }
}
