//! The Magic Vineyard. A vineyard in the country (`country`) on a golden autumn afternoon lies on the
//! desk: rows of vines along the far hills, and rows of vines on the slope beside the page, their
//! leaves turning. Each chapter has a stretch of a row beside the page, and each page of the chapter
//! being written grows a bunch of grapes on it, small and green at first, swelling and ripening to a
//! dusky purple. When the next chapter begins, the stretch before turns: its leaves go gold and red,
//! many of them drift down to lie in the grass, and its grapes darken to the ripest purple; they
//! stay, so each chapter's stretch keeps its grapes. Now and then a murmuration of starlings swoops
//! and folds across the sky. No one is in it, and nothing brings a title in this theme.
//!
//! Like everything in `eggs.rs` it lies on the desk behind the pages and is drawn from shapes as a
//! function of the story and of time; nothing is printed or saved, and nothing new is kept: the
//! stretches and their grapes are counted from `Doc::garden`. It is all laid out as at the fitting
//! zoom and then zoomed as one about the foot of the desk under the page's middle, so zooming
//! brings the whole vineyard nearer, as through a camera.

use super::*;
use super::country::*;

/// The slope, darker towards the writer, and the rows of vines along the far hills.
const FIELD: Color32 = Color32::from_rgb(164, 140, 80);
const FIELD_LOW: Color32 = Color32::from_rgb(150, 124, 70);
const ROWS_FAR: Color32 = Color32::from_rgb(96, 102, 56);
/// The vines: their leaves turning, and turned, their old wood, the posts and the wire.
const LEAF_GREEN: Color32 = Color32::from_rgb(98, 124, 46);
const LEAF_YELLOW: Color32 = Color32::from_rgb(208, 172, 62);
const LEAF_RUST: Color32 = Color32::from_rgb(178, 92, 42);
const LEAF_DARK: Color32 = Color32::from_rgb(46, 60, 28);
const AUTUMN_RED: Color32 = Color32::from_rgb(178, 50, 36);
const AUTUMN_GOLD: Color32 = Color32::from_rgb(230, 168, 48);
const TRUNK: Color32 = Color32::from_rgb(92, 68, 48);
const POST: Color32 = Color32::from_rgb(128, 104, 78);
const WIRE: Color32 = Color32::from_rgb(150, 146, 136);
/// The grapes: green while they swell, dusky purple ripe, darker still at the last, with the bloom
/// on them.
const UNRIPE: Color32 = Color32::from_rgb(150, 172, 84);
const UNRIPE_DARK: Color32 = Color32::from_rgb(92, 116, 48);
pub const GRAPE: Color32 = Color32::from_rgb(88, 38, 82);
const GRAPE_DARK: Color32 = Color32::from_rgb(40, 18, 44);
const RIPEST: Color32 = Color32::from_rgb(56, 18, 58);
const BLOOM: Color32 = Color32::from_rgb(178, 160, 192);
const STALK: Color32 = Color32::from_rgb(110, 96, 50);
/// The starlings, far off against the sky.
pub const STARLING: Color32 = Color32::from_rgb(52, 46, 54);

/// How many bunches a stretch holds at most, one for each page of its chapter.
pub const BUNCHES: usize = 12;
/// Seconds a bunch takes to swell and ripen; and from a chapter beginning until the stretch before
/// has turned and its leaves have fallen.
pub const RIPEN: f32 = 2.5;
pub const TURN: f32 = 10.0;
/// How many rows there are on the slope, and on how many of the nearest the chapters' stretches lie.
const DEPTHS: usize = 5;
const PLOTS: usize = 3;
/// How tall a row of vines is on the nearest ground, and how long a chapter's stretch is there, in units.
const TALL: f32 = 80.0;
const SPACE: f32 = 150.0;
/// The order the bunches come in along a stretch, spread over it rather than from one end.
const ORDER: [usize; BUNCHES] = [5, 2, 9, 0, 7, 3, 11, 1, 6, 10, 4, 8];
/// How many leaves fall from a stretch as it turns.
const FALLING: usize = 40;
/// Seconds between one murmuration of starlings and the next, how long one lasts, and how many birds.
pub const FLOCK_EVERY: f32 = 90.0;
pub const FLOCK: f32 = 24.0;
const BIRDS: usize = 160;

/// The rows on the slope, laid out on the land, each waving gently over it, its own way.
#[derive(Clone, Copy, Debug)]
pub struct Rows {
    land: Land,
    phase: [f32; DEPTHS],
}

impl Rows {
    pub fn new(story: &str, land: Land) -> Rows {
        let mut s = Seeds::of(story, 130_000);
        Rows { land, phase: std::array::from_fn(|_| s.next(0.0, 2.0 * PI)) }
    }

    /// How far up the slope row `l` lies, from 0 (the nearest) to 1.
    fn depth(l: usize) -> f32 {
        l as f32 / (DEPTHS - 1) as f32
    }

    /// The size of things on row `l`, 1 on the nearest; and how much the haze pales it.
    fn k(l: usize) -> f32 {
        1.0 - 0.42 * Rows::depth(l)
    }

    fn haze(l: usize) -> f32 {
        0.3 * Rows::depth(l)
    }

    /// Where row `l` stands on the ground at `x`, and the grass in front of it.
    pub fn row(&self, l: usize, x: f32) -> f32 {
        let (u, k) = (self.land.u, Rows::k(l));
        self.land.ground - Rows::depth(l) * RISE * u + 5.0 * u * k * ((x - self.land.middle) / (560.0 * u) + self.phase[l]).sin()
    }

    /// How tall the vines are on row `l`.
    pub fn tall(&self, l: usize) -> f32 {
        TALL * self.land.u * Rows::k(l)
    }
}

/// A stretch of a row a chapter's grapes grow on: its row, from where to where across, and the side
/// of the page it is on (-1 the left).
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub level: usize,
    pub from: f32,
    pub to: f32,
    pub out: f32,
}

impl Place {
    /// Around its vines and the leaves fallen in front of them.
    pub fn bounds(&self, rows: &Rows) -> Rect {
        let h = rows.tall(self.level);
        let y = rows.row(self.level, (self.from + self.to) / 2.0);
        Rect::from_min_max(pos2(self.from, y - 0.95 * h), pos2(self.to, y + 0.3 * h))
    }

    /// Whether it is wholly on the desk, beside the page.
    pub fn in_view(&self, rows: &Rows, area: Rect, page: Rect) -> bool {
        let r = self.bounds(rows);
        area.contains_rect(r) && !r.x_range().intersects(page.x_range())
    }

    /// How far `x` is within it, fading over its last few units at each end.
    fn within(&self, x: f32, u: f32) -> f32 {
        let edge = 14.0 * u;
        ((x - self.from) / edge).min((self.to - x) / edge).clamp(0.0, 1.0)
    }
}

/// The stretches on the nearest rows, in the order chapters take them: nearer the page first but on
/// every row, each a little off a grid so they do not line up. They come from `story` and reach as
/// far from the page whatever the window, so a stretch never moves when it changes; those beyond the
/// desk are out of sight.
pub fn places(story: &str, page: Rect, rows: &Rows) -> Vec<Place> {
    let mut s = Seeds::of(story, 131_000);
    let u = rows.land.u;
    let mut out = Vec::new();
    for level in 0..PLOTS {
        let k = Rows::k(level);
        let w = SPACE * u * k;
        let step = w + 0.9 * TALL * u * k;
        let mut j = 0.0;
        while 40.0 * u * k + step * (j + 1.5) <= REACH * u {
            for side in [-1.0f32, 1.0] {
                let d = 40.0 * u * k + step * (j + 0.5 * (level % 2) as f32 + s.next(0.0, 0.1));
                let (from, to) = if side < 0.0 { (page.left() - d - w, page.left() - d) } else { (page.right() + d, page.right() + d + w) };
                let key = d + s.next(0.0, 320.0) * u + level as f32 * 40.0 * u;
                out.push((key, Place { level, from, to, out: side }));
            }
            j += 1.0;
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, p)| p).collect()
}

/// Where each bunch of chapter `k`'s stretch hangs, in the order they come: just under the leaves,
/// at the top of its stalk; how long it is; and a seed of its own for its berries.
pub fn bunches(story: &str, k: usize, place: &Place, rows: &Rows) -> Vec<(Pos2, f32, u64)> {
    let mut s = Seeds::of(story, 132_000 + k);
    let h = rows.tall(place.level);
    ORDER.iter().map(|&slot| {
        let x = place.from + (place.to - place.from) * (slot as f32 + 0.5 + s.next(-0.25, 0.25)) / BUNCHES as f32;
        let top = pos2(x, rows.row(place.level, x) - h * s.next(0.47, 0.53));
        (top, h * s.next(0.19, 0.23), s.0)
    }).collect()
}

/// A bunch of grapes hanging from `top`, `size` long once grown, `grown` from 0 (just set) to 1 (full
/// and ripe), and `last` how far it has darkened to the ripest purple, its bloom thicker: it swells
/// green first, then ripens. Each berry round, darker below and lit from the sun's side.
pub fn bunch(top: Pos2, size: f32, grown: f32, last: f32, seed: u64, haze: f32) -> Vec<Shape> {
    let mut s = Seeds(seed);
    let swell = ease(grown / 0.45);
    let ripe = ease((grown - 0.35) / 0.65);
    let size = size * (0.35 + 0.65 * swell);
    let hz = |c: Color32| c.lerp_to_gamma(HAZE, haze);
    let dark = hz(UNRIPE_DARK.lerp_to_gamma(GRAPE_DARK, ripe).lerp_to_gamma(GRAPE_DARK.lerp_to_gamma(Color32::BLACK, 0.3), last));
    let mid = hz(UNRIPE.lerp_to_gamma(GRAPE, ripe).lerp_to_gamma(RIPEST, last));
    let r = size * 0.11;
    let mut out = vec![Shape::line_segment([top + vec2(0.0, -size * 0.12), top + vec2(0.0, size * 0.1)], Stroke::new(size * 0.05, hz(STALK)))];
    let lean = s.next(-0.12, 0.12);
    const ROWS: [usize; 7] = [4, 4, 4, 3, 3, 2, 1];
    for (i, &n) in ROWS.iter().enumerate() {
        let f = i as f32 / (ROWS.len() - 1) as f32;
        let w = size * 0.36 * (1.0 - 0.7 * f);
        let y = top.y + size * (0.14 + 0.13 * i as f32);
        for j in 0..n {
            let along = if n == 1 { 0.0 } else { j as f32 / (n - 1) as f32 * 2.0 - 1.0 };
            let c = pos2(top.x + along * w * 0.75 + lean * size * f + s.next(-0.3, 0.3) * r, y + s.next(-0.3, 0.3) * r);
            let r = r * s.next(0.9, 1.1);
            out.push(Shape::circle_filled(c, r, dark));
            out.push(Shape::circle_filled(c + vec2(-0.18, -0.2) * r, r * 0.76, mid));
            let shine = if ripe > 0.5 { glow(hz(BLOOM), 0.55 + 0.25 * last) } else { glow(Color32::from_rgb(226, 236, 170), 0.6) };
            out.push(Shape::circle_filled(c + vec2(-0.4, -0.42) * r, r * (0.26 + 0.1 * last), shine));
        }
    }
    out
}

/// A vine leaf at `at`, `r` across, turned `turn`: a broad middle lobe and two at its sides.
fn leaf(at: Pos2, r: f32, turn: f32, color: Color32) -> Vec<Shape> {
    let lobe = |a: f32| at + Vec2::angled(a + turn) * r * 0.75;
    vec![
        Shape::circle_filled(lobe(-2.4), r * 0.75, color.lerp_to_gamma(LEAF_DARK, 0.12)),
        Shape::circle_filled(lobe(-0.7), r * 0.75, color.lerp_to_gamma(LEAF_DARK, 0.12)),
        Shape::circle_filled(at + Vec2::angled(-PI / 2.0 + turn) * r * 0.35, r, color.lerp_to_gamma(SUNLIT, 0.08)),
    ]
}

/// A leaf turned, gold or red, from where it falls along a row.
fn autumn(x: f32, u: f32) -> Color32 {
    let m = 0.5 + 0.5 * (x / (37.0 * u)).sin() * (x / (11.0 * u) + 1.0).cos();
    AUTUMN_GOLD.lerp_to_gamma(AUTUMN_RED, m).lerp_to_gamma(LEAF_RUST, 0.2)
}

/// A row of vines `l`, across what shows of `area`: its shadow on the ground towards the writer, the
/// old trunks up to the wire, the leaves turning gold and rust, lit from above where the sun comes
/// through them, the posts, and dry grass in front. Where `turned` says (from 0 to 1, by where along
/// the row), its leaves have turned gold and red, and fewer are left. Its pieces come from where they
/// stand, so whatever shows, each is the same.
fn vines(story: &str, l: usize, rows: &Rows, area: Rect, turned: &dyn Fn(f32) -> f32) -> Vec<Shape> {
    let (k, u, middle) = (Rows::k(l), rows.land.u, rows.land.middle);
    let h = rows.tall(l);
    let hz = |c: Color32| c.lerp_to_gamma(HAZE, Rows::haze(l));
    let start = middle - REACH * u;
    let step = 0.5 * h;
    // What shows of it, however far from the page (pushed to one side, the page leaves a lot showing).
    let first = ((area.left() - start) / step).floor() as i64 - 1;
    let last = ((area.right() - start) / step).ceil() as i64 + 1;
    let mut out = Vec::new();
    if first >= last {
        return out;
    }
    let x_of = |i: i64| start + i as f32 * step;
    // Each piece's own seeds, from where it stands along the row.
    let seeds = |i: i64, what: usize| {
        let at = if i >= 0 { i as usize } else { 50_000_000 + i.unsigned_abs() as usize };
        Seeds::of(story, 140_000 + l * 100_000 + at * 4 + what)
    };
    let colour = |x: f32| {
        let m = 0.5 + 0.5 * ((x - middle) / (130.0 * u) + l as f32).sin() * ((x - middle) / (53.0 * u) + 2.0 * l as f32).cos();
        let c = LEAF_GREEN.lerp_to_gamma(LEAF_YELLOW, 0.2 + 0.7 * m);
        let c = if m > 0.82 { c.lerp_to_gamma(LEAF_RUST, (m - 0.82) * 4.0) } else { c };
        c.lerp_to_gamma(autumn(x, u), turned(x))
    };
    // The shadow, falling towards the writer.
    let mut shade = egui::epaint::Mesh::default();
    for i in first..=last {
        let x = x_of(i);
        let y = rows.row(l, x);
        shade.colored_vertex(pos2(x, y - 0.04 * h), glow(hz(SHADOW), 0.34));
        shade.colored_vertex(pos2(x, y + 0.16 * h), glow(hz(SHADOW), 0.0));
    }
    for j in 0..(last - first) as u32 {
        let (a, b) = (2 * j, 2 * j + 2);
        shade.add_triangle(a, a + 1, b);
        shade.add_triangle(b, a + 1, b + 1);
    }
    out.push(Shape::mesh(shade));
    // The trunks, gnarled, and the wire they are tied to.
    for i in first..=last {
        let mut s = seeds(i, 0);
        let x = x_of(i) + s.next(-0.06, 0.06) * h;
        let y = rows.row(l, x);
        let spine = [pos2(x, y), pos2(x + s.next(-0.03, 0.03) * h, y - 0.2 * h), pos2(x + s.next(-0.04, 0.04) * h, y - 0.42 * h)];
        out.extend(ribbon(&spine, &[0.04 * h, 0.032 * h, 0.028 * h], hz(TRUNK.lerp_to_gamma(FIELD, 0.15))));
    }
    let wire: Vec<Pos2> = (first..=last).map(|i| pos2(x_of(i), rows.row(l, x_of(i)) - 0.42 * h)).collect();
    out.push(Shape::line(wire, Stroke::new((0.8 * k * u).max(0.5), hz(WIRE))));
    // The leaves: one soft body lit on top and shaded deep within, its edges leafy, then leaves over it.
    let mut body = egui::epaint::Mesh::default();
    let fine = 3;
    let n = (last - first) * fine;
    for j in 0..=n {
        let x = x_of(first) + j as f32 * step / fine as f32;
        let y = rows.row(l, x);
        let d = x - middle;
        let thin = 0.12 * turned(x);
        let lump = 0.07 * (d / (14.0 * k * u)).sin() + 0.05 * (d / (37.0 * k * u) + 1.0).sin() + 0.03 * (d / (5.0 * k * u) + 2.0).sin() - thin;
        let hem = 0.05 * (d / (11.0 * k * u)).sin() + 0.03 * (d / (4.3 * k * u) + 1.0).sin();
        let c = colour(x);
        let deep = 0.55 + 0.25 * (d / (7.0 * k * u) + 0.5).sin() * (d / (19.0 * k * u)).sin();
        body.colored_vertex(pos2(x, y - (0.84 + lump) * h), hz(c.lerp_to_gamma(SUNLIT, 0.3)));
        body.colored_vertex(pos2(x, y - (0.6 + 0.3 * hem) * h), hz(c.lerp_to_gamma(LEAF_DARK, deep)));
        body.colored_vertex(pos2(x, y - (0.4 - hem) * h), hz(c.lerp_to_gamma(LEAF_DARK, 0.6)));
    }
    for j in 0..n as u32 {
        let (a, b) = (3 * j, 3 * j + 3);
        for r in 0..2 {
            body.add_triangle(a + r, a + r + 1, b + r);
            body.add_triangle(b + r, a + r + 1, b + r + 1);
        }
    }
    out.push(Shape::mesh(body));
    for i in first..=last {
        let mut s = seeds(i, 1);
        // Most of them at its edges, so its outline is of leaves; fewer where it has turned.
        for j in 0..6 {
            let x = x_of(i) + s.next(-0.5, 0.5) * step;
            let y = rows.row(l, x);
            let up = match j % 3 {
                0 => s.next(0.82, 1.0),
                1 => s.next(0.0, 0.15),
                _ => s.next(0.1, 0.9),
            };
            let at = pos2(x, y - (0.36 + 0.52 * up) * h);
            let c = colour(x + s.next(-30.0, 30.0) * u).lerp_to_gamma(LEAF_DARK, 0.45 * (1.0 - up) + s.next(0.0, 0.15));
            let c = if up > 0.7 { c.lerp_to_gamma(SUNLIT, 0.3 * (up - 0.7) / 0.3) } else { c };
            let (r, turn, gone) = (s.next(0.03, 0.045) * h, s.next(-0.5, 0.5), s.next(0.0, 1.0));
            if gone >= 0.45 * turned(x) {
                out.extend(leaf(at, r, turn, hz(c)));
            }
        }
    }
    // The posts, every few trunks, in front of the leaves.
    for i in (first..=last).filter(|i| i.rem_euclid(6) == 3) {
        let x = x_of(i) + 0.5 * step;
        let y = rows.row(l, x);
        out.push(Shape::line_segment([pos2(x, y + 0.01 * h), pos2(x, y - 0.94 * h)], Stroke::new(0.035 * h, hz(POST))));
        out.push(Shape::line_segment([pos2(x - 0.01 * h, y), pos2(x - 0.01 * h, y - 0.94 * h)], Stroke::new(0.012 * h, hz(POST.lerp_to_gamma(SUNLIT, 0.3)))));
    }
    // Dry grass on the ground in front of it, and a little green where the vines shade it.
    for i in first..=last {
        let mut s = seeds(i, 2);
        for _ in 0..3 {
            let x = x_of(i) + s.next(-0.5, 0.5) * step;
            let y = rows.row(l, x) + s.next(0.0, 0.3) * h;
            let c = if s.next(0.0, 1.0) < 0.3 { LEAF_GREEN.lerp_to_gamma(FIELD, 0.5) } else { FIELD.lerp_to_gamma(SUNLIT, s.next(0.1, 0.5)) };
            for b in 0..2 {
                let lean = (b as f32 - 0.5) * 0.8 + s.next(-0.2, 0.2);
                let tip = pos2(x + lean * 0.04 * h, y - s.next(0.03, 0.06) * h);
                out.push(Shape::line_segment([pos2(x + b as f32 * 0.008 * h, y), tip], Stroke::new(0.008 * h, hz(c))));
            }
        }
    }
    out
}

/// The leaves falling from chapter `k`'s stretch as it turns, `t` seconds after the next chapter
/// began: each lets go in its own moment, drifts down, swaying and turning over as it goes, and
/// comes to lie in the grass in front of the row. Each one's number, where it is, how far it is
/// turned, and whether it lies on the ground; none before it lets go.
pub fn falling(story: &str, k: usize, place: &Place, rows: &Rows, t: f32) -> Vec<(usize, Pos2, f32, bool)> {
    let h = rows.tall(place.level);
    (0..FALLING).filter_map(|i| {
        let mut s = Seeds::of(story, 165_000 + k * 64 + i);
        let x = place.from + (place.to - place.from) * s.next(0.05, 0.95);
        let from = pos2(x, rows.row(place.level, x) - h * s.next(0.5, 0.85));
        let to = pos2(x + place.out * s.next(0.0, 0.5) * h, rows.row(place.level, x) + h * s.next(0.04, 0.26));
        let (start, long) = (1.0 + (TURN - 5.0) * i as f32 / FALLING as f32 + s.next(0.0, 0.6), s.next(2.8, 3.8));
        let f = (t - start) / long;
        if f < 0.0 {
            return None;
        }
        let f = f.min(1.0);
        let sway = (f * PI * s.next(2.0, 3.5)).sin() * 0.18 * h * (1.0 - f);
        let at = pos2(from.x + (to.x - from.x) * f + sway, from.y + (to.y - from.y) * ease(f));
        Some((i, at, s.next(0.0, 6.0) + f * s.next(4.0, 9.0), f >= 1.0))
    }).collect()
}

/// A starling murmuration at `time`, if one is in the sky: a minute or two apart it comes in from
/// one side over the hills, its flock stretching, folding and turning as it swoops across, and is
/// gone at the other side. Where each bird is, and how far through its wingbeat. The flock's shape
/// and course are functions of the time since it came, so a bird never jumps.
pub fn starlings(story: &str, land: &Land, time: f64) -> Vec<(Pos2, f32)> {
    let since = time.rem_euclid(FLOCK_EVERY as f64) as f32 - (FLOCK_EVERY - FLOCK);
    if !(0.0..FLOCK).contains(&since) {
        return Vec::new();
    }
    let u = land.u;
    let round = (time / FLOCK_EVERY as f64).floor() as usize;
    let mut s = Seeds::of(story, 166_000 + round);
    let dir = if round.is_multiple_of(2) { 1.0 } else { -1.0 };
    let q = since / FLOCK;
    let horizon = land.ground - RISE * u - 150.0 * u;
    let centre = pos2(land.middle + dir * (q * 2.0 - 1.0) * 950.0 * u, horizon - (s.next(70.0, 120.0) + 45.0 * (q * PI * 1.6).sin()) * u);
    // Thinner as it comes in and goes, so it gathers and disperses rather than appearing.
    let gathered = (q / 0.12).min((1.0 - q) / 0.12).clamp(0.0, 1.0);
    let t = since;
    let size = (90.0 + 40.0 * (t * 0.27).sin()) * u;
    let (turn, wide, high) = (0.6 * (t * 0.35).sin(), 1.0 + 0.7 * (t * 0.5).sin(), 0.45 + 0.3 * (t * 0.43).cos());
    (0..BIRDS).filter_map(|i| {
        let mut b = Seeds::of(story, 167_000 + i);
        let (a, r) = (b.next(0.0, 2.0 * PI), b.next(0.0, 1.0).sqrt());
        if b.next(0.0, 1.0) > gathered {
            return None;
        }
        let (x, y) = (a.cos() * r * wide, a.sin() * r * high + 0.35 * (3.0 * a.cos() * r + t * 1.3).sin());
        let (c, sn) = (turn.cos(), turn.sin());
        let at = centre + vec2(x * c - y * sn, x * sn + y * c) * size;
        Some((at, (t * 11.0 + b.next(0.0, 6.0)).sin()))
    }).collect()
}

/// The starlings as one mesh: each a tiny dark bird, its wings up or down as it beats them.
fn flock(birds: &[(Pos2, f32)], u: f32) -> Shape {
    let mut mesh = egui::epaint::Mesh::default();
    for &(at, beat) in birds {
        let i = mesh.vertices.len() as u32;
        let (span, lift) = (1.8 * u, beat * 0.9 * u);
        mesh.colored_vertex(at + vec2(-span, -lift), STARLING);
        mesh.colored_vertex(at + vec2(0.0, -0.35 * u), STARLING);
        mesh.colored_vertex(at + vec2(span, -lift), STARLING);
        mesh.colored_vertex(at + vec2(0.0, 0.45 * u), STARLING);
        mesh.add_triangle(i, i + 1, i + 3);
        mesh.add_triangle(i + 1, i + 2, i + 3);
    }
    Shape::mesh(mesh)
}

/// The land behind the vines: the country (`country`), with rows of vines along the nearer hills and,
/// now and then, the starlings in the sky; then the slope the rows stand on, darker towards the
/// writer. Only its fills reach to `area`'s edges.
pub fn landscape(story: &str, area: Rect, land: &Land, time: f64) -> Vec<Shape> {
    let lines = |_: usize, haze: f32| Some((glow(ROWS_FAR.lerp_to_gamma(HAZE, haze), 0.55), 0.9 * (1.0 - haze)));
    let birds = |out: &mut Vec<Shape>| {
        let flying = starlings(story, land, time);
        if !flying.is_empty() {
            out.push(flock(&flying, land.u));
        }
    };
    let mut out = country(story, area, land, time, FIELD, &lines, &birds);
    let (u, ground) = (land.u, land.ground);
    let top = ground - RISE * u - 10.0 * u;
    let mut field = egui::epaint::Mesh::default();
    field.colored_vertex(pos2(area.left(), top), FIELD.lerp_to_gamma(HAZE, 0.15));
    field.colored_vertex(pos2(area.right(), top), FIELD.lerp_to_gamma(HAZE, 0.15));
    field.colored_vertex(pos2(area.right(), ground), FIELD_LOW);
    field.colored_vertex(pos2(area.left(), ground), FIELD_LOW);
    field.add_triangle(0, 1, 2);
    field.add_triangle(0, 2, 3);
    out.push(Shape::mesh(field));
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), ground), area.max), 0.0, FIELD_LOW));
    out
}

impl App {
    /// The vineyard on the desk, behind the pages lying at `page`, in the Magic Vineyard only: a
    /// stretch of vines for each chapter, a bunch for each page it has had (`Doc::garden`), the
    /// stretch before turning gold and red when a chapter begins.
    pub fn draw_vineyard(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.vineyard = None;
        if self.themes.theme != Theme::Vineyard {
            self.eggs.vines.forget();
            return;
        }
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let garden = &self.doc.garden;
        let n = garden.len().max(1);
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        let z = (self.zoom / self.fitting_zoom(area)).clamp(0.5, 4.0);
        let e = &mut self.eggs;
        if e.vines.tend(&self.story_id, garden, BUNCHES, RIPEN, TURN, now) {
            ctx.request_repaint();
        } else {
            // The clouds drift and the starlings fly.
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        let u = e.unit(&ctx, area, now);
        let story = &self.story_id;
        let ground = area.bottom() - hidden - 24.0 * u;
        // Everything is laid out as at the fitting zoom over as much of the desk as shows once
        // zoomed, beside the page as it would be, then zoomed about the foot of the desk under the
        // page's middle.
        let anchor = pos2(page.center().x, ground);
        let unzoom = |r: Rect| Rect::from_min_max(anchor + (r.min - anchor) / z, anchor + (r.max - anchor) / z);
        let (seen_area, page) = (unzoom(area), unzoom(page));
        let zoom = egui::emath::TSTransform::new(anchor.to_vec2() * (1.0 - z), z);
        let land = Land { middle: anchor.x, ground, u };
        let rows = Rows::new(story, land);
        let mut shapes = landscape(story, seen_area, &land, now);
        let all = places(story, page, &rows);
        let placed: Vec<Place> = (0..n).map(|k| all[e.vines.place(k, all.len(), |p| all[p].in_view(&rows, seen_area, page))]).collect();
        let turning = e.vines.finishing(now, TURN);
        let mut seen = Seen { plots: vec![(Rect::NOTHING, 0, 0.0); n], turning: turning.map(|t| t.0), ..Seen::default() };
        seen.birds = starlings(story, &land, now).len();
        // How far each chapter's stretch has turned: not the newest, the one before as it turns,
        // every other all the way.
        let turned: Vec<f32> = (0..n).map(|k| match turning {
            _ if k + 1 >= n => 0.0,
            Some((m, q)) if m == k => between(q * TURN, 0.0, TURN * 0.7),
            _ => 1.0,
        }).collect();
        // Row by row from the furthest up the slope: its vines, turned where its stretches have, the
        // grapes on its stretches, and the leaves fallen in front of it.
        for l in (0..DEPTHS).rev() {
            let on = |x: f32| placed.iter().zip(&turned).filter(|(p, _)| p.level == l).map(|(p, &a)| a * p.within(x, u)).fold(0.0, f32::max);
            shapes.extend(vines(story, l, &rows, seen_area, &on));
            for (k, place) in placed.iter().enumerate().filter(|(_, p)| p.level == l) {
                let all = bunches(story, k, place, &rows);
                let pages = garden.get(k).map_or(1, |&p| (p as usize).min(BUNCHES));
                let (hanging, last): (Vec<(usize, f32)>, f32) = if k + 1 >= n {
                    let grown = e.vines.grown(garden, k, BUNCHES);
                    ((0..grown.ceil() as usize).map(|j| (j, (grown - j as f32).clamp(0.0, 1.0))).collect(), 0.0)
                } else {
                    ((0..pages).map(|j| (j, 1.0)).collect(), turned[k])
                };
                for &(j, grown) in &hanging {
                    let (top, size, seed) = all[j];
                    shapes.extend(bunch(top, size, grown, last, seed, Rows::haze(l)));
                }
                if k + 1 < n {
                    let t = match turning {
                        Some((m, q)) if m == k => q * TURN,
                        _ => TURN,
                    };
                    let leaves = falling(story, k, place, &rows, t);
                    let r = 0.04 * rows.tall(l);
                    for &(_, at, turn, lying) in &leaves {
                        let c = autumn(at.x * 3.0, u).lerp_to_gamma(HAZE, Rows::haze(l));
                        if lying {
                            shapes.push(oval(at, Vec2::angled(turn), r * 1.1, r * 0.45, c.lerp_to_gamma(SHADOW, 0.15)));
                        } else {
                            shapes.push(oval(at, Vec2::angled(turn), r * 1.1, r * (0.3 + 0.6 * (turn * 1.7).sin().abs()), c));
                        }
                    }
                    seen.falling += leaves.iter().filter(|f| !f.3).count();
                    seen.fallen += leaves.iter().filter(|f| f.3).count();
                }
                seen.plots[k] = (zoom.mul_rect(place.bounds(&rows)), hanging.iter().filter(|b| b.1 >= 1.0).count(), turned[k]);
            }
        }
        ui.painter().extend(shapes.into_iter().map(|mut s| {
            s.transform(zoom);
            s
        }));
        self.eggs.vineyard = Some(seen);
    }
}

/// The vineyard drawn last frame, for tests: each chapter's stretch, around it, with how many ripe
/// bunches hang on it and how far it has turned; the chapter turning, if one is; how many leaves are
/// falling and how many lie fallen; and how many starlings are in the sky.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub plots: Vec<(Rect, usize, f32)>,
    pub turning: Option<usize>,
    pub falling: usize,
    pub fallen: usize,
    pub birds: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))
    }

    fn page() -> Rect {
        Rect::from_min_size(pos2(450.0, 60.0), vec2(500.0, 700.0))
    }

    fn land() -> Land {
        Land { middle: 700.0, ground: 880.0, u: 1.0 }
    }

    fn rows() -> Rows {
        Rows::new("a story", land())
    }

    #[test]
    fn the_vineyard_is_there_from_the_start_the_same_for_the_same_story() {
        let shapes = |story: &str| [landscape(story, desk(), &land(), 0.0), vines(story, 0, &Rows::new(story, land()), desk(), &|_| 0.0)].concat();
        assert!(shapes("a story").len() > 200, "sky, sun, clouds, hills and vines: {}", shapes("a story").len());
        let at = |story: &str| shapes(story).iter().map(|s| s.visual_bounding_rect().center()).collect::<Vec<_>>();
        assert_eq!(at("a story"), at("a story"));
        assert_ne!(at("a story"), at("another story"));
    }

    #[test]
    fn the_rows_go_on_to_the_edges_of_whatever_shows_however_far_from_the_page() {
        for far in [-4000.0, 4000.0] {
            let area = desk().translate(vec2(far, 0.0));
            let row = vines("a story", 0, &rows(), area, &|_| 0.0).iter().map(|s| s.visual_bounding_rect()).fold(Rect::NOTHING, |a, r| a | r);
            assert!(row.left() <= area.left() && row.right() >= area.right(), "{far}: {row:?}");
        }
    }

    #[test]
    fn the_landscape_is_laid_out_the_same_whatever_part_of_it_shows() {
        let laid = |area: Rect| landscape("a story", area, &land(), 3.0).into_iter().filter(|s| !matches!(s, Shape::Rect(_) | Shape::Mesh(_))).map(|s| s.visual_bounding_rect()).collect::<Vec<_>>();
        assert_eq!(laid(desk()), laid(desk().expand2(vec2(500.0, 300.0))));
        assert_eq!(laid(desk()), laid(desk().shrink(200.0)));
    }

    #[test]
    fn a_bunch_swells_green_then_ripens_purple_and_darkens_at_the_last() {
        let b = |grown: f32, last: f32| bunch(pos2(100.0, 100.0), 14.0, grown, last, 3, 0.0);
        let around = |s: &[Shape]| s.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect());
        assert!(around(&b(0.0, 0.0)).height() < around(&b(1.0, 0.0)).height() * 0.6, "small at first");
        let berry = |s: &[Shape]| s.iter().find_map(|s| match s {
            Shape::Circle(c) if c.radius > 1.0 && c.fill.a() == 255 && c.radius < 1.5 => Some(c.fill),
            _ => None,
        }).unwrap();
        let (green, ripe, ripest) = (berry(&b(0.3, 0.0)), berry(&b(1.0, 0.0)), berry(&b(1.0, 1.0)));
        assert!(green.g() > green.r() && ripe.r() > ripe.g() && ripe.b() > ripe.g(), "green {green:?}, then purple {ripe:?}");
        assert!(ripest.r() < ripe.r() && ripest.g() <= ripe.g(), "darker at the last: {ripest:?}");
    }

    #[test]
    fn a_turned_stretch_has_gold_and_red_leaves_and_fewer_of_them() {
        let place = places("a story", page(), &rows())[0];
        let r = rows();
        let area = Rect::from_x_y_ranges(place.from..=place.to, 0.0..=900.0);
        let leaves = |a: f32| vines("a story", place.level, &r, area, &|x| a * place.within(x, 1.0)).into_iter().filter(|s| matches!(s, Shape::Circle(_))).collect::<Vec<_>>();
        let red = |s: &[Shape]| s.iter().filter(|s| matches!(s, Shape::Circle(c) if c.fill.r() > c.fill.g() + 40)).count();
        let (green, turned) = (leaves(0.0), leaves(1.0));
        assert!(turned.len() < green.len() * 9 / 10, "fewer: {} then {}", green.len(), turned.len());
        assert!(red(&turned) > red(&green) + 20, "gold and red: {} then {}", red(&green), red(&turned));
    }

    #[test]
    fn the_leaves_drift_down_one_after_another_and_lie_in_the_grass() {
        let r = rows();
        let place = places("a story", page(), &r)[0];
        assert!(falling("a story", 0, &place, &r, 0.0).is_empty(), "none at first");
        let mut was: Vec<(usize, Pos2, f32, bool)> = Vec::new();
        for i in 0..=(TURN * 30.0) as usize {
            let now = falling("a story", 0, &place, &r, i as f32 / 30.0);
            assert!(now.len() >= was.len() && now.len() <= was.len() + 3, "one after another");
            for a in &was {
                let b = now.iter().find(|b| b.0 == a.0).expect("still there");
                assert!(b.1.y >= a.1.y - 0.01 && a.1.distance(b.1) < 3.0, "down, gently: {a:?} {b:?}");
            }
            was = now;
        }
        assert!(was.len() == FALLING && was.iter().all(|l| l.3 && l.1.y > r.row(place.level, l.1.x)), "all lying in front of the row");
    }

    #[test]
    fn now_and_then_a_murmuration_of_starlings_swoops_across_the_sky() {
        let at = |t: f64| starlings("a story", &land(), t);
        let mut came = 0;
        for i in 0..(FLOCK_EVERY * 2.0) as usize {
            if !at(i as f64).is_empty() {
                came += 1;
            }
        }
        assert!((came as f32 - 2.0 * FLOCK).abs() <= 2.0, "a while in each round: {came}");
        let t = (FLOCK_EVERY - FLOCK / 2.0) as f64;
        let (a, b) = (at(t), at(t + 1.0 / 60.0));
        assert!(a.len() > BIRDS / 2, "a flock: {}", a.len());
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(&b) {
            assert!(p.0.distance(q.0) < 4.0, "flying, not jumping: {p:?} {q:?}");
            assert!(p.0.y < land().ground - RISE, "in the sky: {p:?}");
        }
        let thin = at((FLOCK_EVERY - FLOCK + 0.5) as f64).len();
        assert!(thin < a.len() / 2, "gathering as it comes: {thin}");
    }

    #[test]
    fn the_stretches_lie_beside_the_page_nearer_first_on_the_nearest_rows() {
        let r = rows();
        let all = places("a story", page(), &r);
        assert!(all.iter().all(|p| p.level < PLOTS && !p.bounds(&r).x_range().intersects(page().x_range())));
        assert!(all.iter().filter(|p| p.in_view(&r, desk(), page())).count() >= 6);
        assert!((0..PLOTS).all(|l| all[..8].iter().any(|p| p.level == l)), "every row near the page");
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert!(a.level != b.level || !a.bounds(&r).intersects(b.bounds(&r)), "none on another: {a:?} {b:?}");
            }
        }
        let b = bunches("a story", 0, &all[0], &r);
        assert_eq!(b.len(), BUNCHES);
        assert!(b.iter().all(|b| (all[0].from..all[0].to).contains(&b.0.x)), "on its stretch");
    }
}
