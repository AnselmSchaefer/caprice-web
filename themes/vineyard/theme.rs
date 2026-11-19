//! The Magic Vineyard, as a theme program: Caprice's own (`src/eggs/vineyard.rs`, and the country it
//! lies in, `src/eggs/country.rs`) written against the SDK, drawing the same shapes (a test renders
//! both and compares them frame by frame), and the second example a theme is made from, beside the
//! Snow (`tests/fixtures/vineyard.wasm`, by `themes/build.sh`).
//!
//! A vineyard in the country on a golden autumn afternoon lies on the desk: the sun low to one side,
//! soft clouds drifting slowly over rolling hills in the haze, cypresses and a farmhouse on them,
//! rows of vines along the far hills, and rows of vines on the slope beside the page, their leaves
//! turning. Each chapter has a stretch of a row beside the page, and each page of the chapter being
//! written grows a bunch of grapes on it, small and green at first, swelling and ripening to a dusky
//! purple. When the next chapter begins, the stretch before turns: its leaves go gold and red, a few
//! of them drift down to lie in the grass, and its grapes darken to the ripest purple; they stay, so
//! each chapter's stretch keeps its grapes. Now and then a murmuration of starlings swoops and folds
//! across the sky. Nothing brings a title in this theme.
//!
//! Unlike the Snow it draws its own sky, and puts nothing in the window: everything, the sky and the
//! clouds too, is laid out as at the fitting zoom from the ground under the page's middle
//! (`Input::ground`, 24 units above the foot of the desk: `Out::ground`), beside the page as it
//! would be then, so Caprice zooms the whole of it at once and nothing in it can zoom differently
//! from the rest. The country is laid out from that middle as far as `REACH` either side, never from
//! what shows, so zooming only makes it bigger or smaller and moves nothing; only its plain fills
//! reach the edges of what shows (`Input::shown`).
//!
//! The leaves are thousands: they are meshes, a few for each row, never polygons, which egui
//! smooths at every edge.

use std::f32::consts::PI;

use theme_sdk::{Color, Event, Input, Layer, Mesh, Out, Point, Rect, Seeds, Tended, Theme, between, ease, host, pt, theme};

/// The sky, the sun and the clouds, lit warm from below the sun.
const SKY_TOP: Color = Color::rgb(118, 164, 210);
const SKY_LOW: Color = Color::rgb(244, 226, 194);
const SUN: Color = Color::rgb(255, 246, 222);
const SUN_GLOW: Color = Color::rgb(255, 214, 140);
const CLOUD_LIT: Color = Color::rgb(255, 250, 240);
const CLOUD_SHADE: Color = Color::rgb(206, 194, 200);
/// The air that pales what lies further off.
const HAZE: Color = Color::rgb(222, 212, 194);
/// The hills far off, nearer and nearest; the light; the cypresses and the farmhouse; and shadow.
const HILL_FAR: Color = Color::rgb(118, 138, 148);
const HILL_MID: Color = Color::rgb(178, 164, 104);
const HILL_NEAR: Color = Color::rgb(196, 170, 96);
const SUNLIT: Color = Color::rgb(255, 232, 170);
const CYPRESS: Color = Color::rgb(40, 58, 34);
const STONE: Color = Color::rgb(230, 210, 172);
const ROOF: Color = Color::rgb(176, 88, 58);
const SHADOW: Color = Color::rgb(70, 52, 30);

/// The slope, darker towards the writer, and the rows of vines along the far hills.
const FIELD: Color = Color::rgb(164, 140, 80);
const FIELD_LOW: Color = Color::rgb(150, 124, 70);
const ROWS_FAR: Color = Color::rgb(96, 102, 56);
/// The vines: their leaves, a fresh green going yellow here and there, lit by the sun and deep in
/// shade; the clear colours they turn, each leaf one of them; their shoots and old wood; the posts
/// and the wire.
const LEAF_GREEN: Color = Color::rgb(76, 124, 40);
const LEAF_LIT: Color = Color::rgb(150, 184, 62);
const LEAF_YELLOW: Color = Color::rgb(214, 198, 78);
const TURNED: [Color; 4] = [Color::rgb(170, 26, 46), Color::rgb(212, 56, 34), Color::rgb(234, 120, 30), Color::rgb(242, 188, 50)];
const CANE: Color = Color::rgb(132, 120, 60);
const CORDON: Color = Color::rgb(104, 78, 52);
const TRUNK: Color = Color::rgb(92, 68, 48);
const POST: Color = Color::rgb(128, 104, 78);
const WIRE: Color = Color::rgb(150, 146, 136);
/// The grapes: green while they swell, dusky purple ripe, darker still at the last, with the bloom
/// on them.
const UNRIPE: Color = Color::rgb(150, 172, 84);
const UNRIPE_DARK: Color = Color::rgb(92, 116, 48);
const GRAPE: Color = Color::rgb(92, 34, 92);
const GRAPE_DARK: Color = Color::rgb(40, 18, 44);
const RIPEST: Color = Color::rgb(56, 18, 58);
const BLOOM: Color = Color::rgb(188, 172, 210);
const STALK: Color = Color::rgb(110, 96, 50);
/// The starlings, far off against the sky.
const STARLING: Color = Color::rgb(52, 46, 54);

/// How far above the foot of the desk the ground is, in units: the nearest row stands on it, and
/// Caprice zooms about it.
const GROUND: f32 = 24.0;
/// How far the country reaches from the page's middle whatever the window, and how far up the slope
/// in front goes, in units.
const REACH: f32 = 1800.0;
const RISE: f32 = 260.0;
/// How many bunches a stretch holds at most, one for each page of its chapter.
const BUNCHES: usize = 12;
/// Seconds a bunch takes to swell and ripen; and from a chapter beginning until the stretch before
/// has turned and its leaves have fallen.
const RIPEN: f32 = 2.5;
const TURN: f32 = 16.0;
/// How many rows there are on the slope, and on how many of the nearest the chapters' stretches lie.
const DEPTHS: usize = 5;
const PLOTS: usize = 3;
/// How tall a row of vines is on the nearest ground, and how long a chapter's stretch is there, in units.
const TALL: f32 = 80.0;
const SPACE: f32 = 150.0;
/// The order the bunches come in along a stretch, spread over it rather than from one end.
const ORDER: [usize; BUNCHES] = [5, 2, 9, 0, 7, 3, 11, 1, 6, 10, 4, 8];
/// How many leaves fall from a stretch as it turns.
const FALLING: usize = 12;
/// Seconds between one murmuration of starlings and the next, how long one lasts, and how many birds.
const FLOCK_EVERY: f32 = 90.0;
const FLOCK: f32 = 24.0;
const BIRDS: usize = 160;

/// The land the vineyard lies on: laid out from `middle` (across) and `ground` (where the nearest
/// row stands); `u` its unit.
#[derive(Clone, Copy)]
struct Land {
    middle: f32,
    ground: f32,
    u: f32,
}

/// A tongue of fire from `at` along `dir`, `len` long (a cypress): a lens, which is convex.
fn flame(l: &mut Layer, at: Point, dir: Point, len: f32, width: f32, color: Color) {
    let side = pt(-dir.y, dir.x) * width;
    let points = (0..=16).map(|k| {
        let (s, w) = if k <= 8 { (k as f32 / 8.0, 1.0) } else { (2.0 - k as f32 / 8.0, -1.0) };
        at + dir * len * s + side * w * (s * PI).sin()
    });
    l.solid(points.collect(), color);
}

/// A body tapering along `spine`, `widths` thick at its points (a vine's trunk): a convex piece for
/// each stretch and a round at each joint, so it bends smoothly.
fn ribbon(l: &mut Layer, spine: &[Point], widths: &[f32], color: Color) {
    for (k, w) in spine.windows(2).enumerate() {
        let n = (w[1] - w[0]).normalized().rot90();
        let (a, b) = (widths[k] / 2.0, widths[k + 1] / 2.0);
        l.solid(vec![w[0] + n * a, w[1] + n * b, w[1] - n * b, w[0] - n * a], color);
    }
    for (&p, &w) in spine.iter().zip(widths) {
        l.circle(p, w / 2.0, color);
    }
}

/// A soft puff of cloud about `at`, `r` across and `h` high, `alpha` at its middle and fading to
/// nothing at its edge: like `Layer::radial`, with fewer points, as a sky holds many.
fn puff(l: &mut Layer, at: Point, r: f32, h: f32, color: Color, alpha: f32) {
    let mut mesh = Mesh::new();
    mesh.vertex(at, color.see_through(alpha));
    let n = 14;
    for (f, a) in [(0.55, 0.5), (1.0, 0.0)] {
        for i in 0..n {
            let ang = i as f32 / n as f32 * 2.0 * PI;
            mesh.vertex(at + pt(ang.cos() * r * f, ang.sin() * h * f), color.see_through(alpha * a));
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.triangle(0, 1 + i, 1 + j);
        mesh.triangle(1 + i, 1 + n + i, 1 + j);
        mesh.triangle(1 + j, 1 + n + i, 1 + n + j);
    }
    l.mesh(mesh);
}

/// A hill's height at `x`: rolling, from three waves of its own.
fn hill(waves: &[(f32, f32, f32)], base: f32, x: f32, middle: f32, u: f32) -> f32 {
    base - waves.iter().map(|&(a, wl, ph)| a * u * (0.5 + 0.5 * ((x - middle) / (wl * u) + ph).sin())).sum::<f32>()
}

/// A hill behind the slope: its colour, the height of its foot, how high it rolls, how much the haze
/// pales it, how far apart the stripes of the fields along it are (none on the far ones), and its waves.
struct Hill {
    color: Color,
    base: f32,
    high: f32,
    haze: f32,
    rows: f32,
    waves: Vec<(f32, f32, f32)>,
}

/// The hills behind the slope, from the furthest.
fn hills(seed: u64, land: &Land) -> Vec<Hill> {
    let u = land.u;
    let top = land.ground - RISE * u;
    let horizon = top - 150.0 * u;
    let mut s = Seeds::of(seed, 134_000);
    [
        (HILL_FAR, horizon - 10.0 * u, 120.0, 0.72, 0.0),
        (HILL_FAR.lerp(HILL_MID, 0.4), horizon + 40.0 * u, 100.0, 0.55, 0.0),
        (HILL_MID, top - 60.0 * u, 80.0, 0.32, 6.0),
        (HILL_NEAR, top - 20.0 * u, 50.0, 0.14, 8.0),
    ].into_iter().map(|(color, base, high, haze, rows)| {
        let waves = (0..3).map(|w| (high * s.next(0.25, 0.5), s.next(70.0, 160.0) * (1.0 + 1.5 * w as f32), s.next(0.0, 6.3))).collect();
        Hill { color, base, high, haze, rows, waves }
    }).collect()
}

/// Where the farmhouse stands on the nearer hills.
fn farm(seed: u64, land: &Land) -> Point {
    let mut s = Seeds::of(seed, 137_000);
    let side = if s.next(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
    let x = land.middle + side * s.next(480.0, 640.0) * land.u;
    let h = &hills(seed, land)[2];
    pt(x, hill(&h.waves, h.base, x, land.middle, land.u) + 6.0 * land.u)
}

/// How far below its hill's crest line `r` of the lines along it lies, the lines `rows` apart at the
/// top and further apart lower down, as nearer.
fn rows_apart(rows: f32, r: usize, u: f32) -> f32 {
    rows * u * r as f32 * (1.0 + 0.15 * r as f32)
}

/// The country behind the slope: the sky, warmer low down, the low sun to one side and its glow,
/// soft clouds drifting slowly, and hills far off and nearer, paler in the haze, the nearer with
/// lines along their slopes (`lines`: each one's colour and width, from its number and how much the
/// haze pales it, if it is drawn), cypresses and a farmhouse; the nearest hill's foot `field`'s
/// colour, the slope's; then what else flies over them (`company`). All of it is laid out from the
/// land's middle and its ground, as far as `REACH` either side, whatever part of it shows in `area`:
/// only the plain fills reach to `area`'s edges.
#[allow(clippy::too_many_arguments)]
fn country(l: &mut Layer, seed: u64, area: Rect, land: &Land, time: f64, field: Color, lines: &dyn Fn(usize, f32) -> Option<(Color, f32)>, company: &dyn Fn(&mut Layer)) {
    let (u, middle, ground) = (land.u, land.middle, land.ground);
    let t = time as f32;
    let top_of_slope = ground - RISE * u;
    let horizon = top_of_slope - 150.0 * u;
    let mut sky = Mesh::new();
    sky.vertex(area.min, SKY_TOP);
    sky.vertex(pt(area.right(), area.top()), SKY_TOP);
    sky.vertex(pt(area.right(), horizon), SKY_LOW);
    sky.vertex(pt(area.left(), horizon), SKY_LOW);
    sky.triangle(0, 1, 2);
    sky.triangle(0, 2, 3);
    l.mesh(sky);
    l.polygon(Rect::from_min_max(pt(area.left(), horizon), area.max).corners(), SKY_LOW);
    let mut s = Seeds::of(seed, 136_000);
    // The sun low on the side the light comes from.
    let sun = pt(middle - 700.0 * u, horizon - 230.0 * u);
    l.radial(sun, 620.0 * u, 420.0 * u, SUN_GLOW, 0.5);
    l.radial(sun, 130.0 * u, 130.0 * u, SUN, 0.75);
    l.circle(sun, 22.0 * u, SUN);
    // Clouds, soft, of puffs lit above and shaded below, drifting slowly the same way; how far is a
    // share of the way across, never the time multiplied by a size.
    for i in 0..10 {
        let mut c = Seeds::of(seed, 135_000 + i);
        let share = (c.next(0.0, 1.0) + t * c.next(2.0, 4.0) / (2.0 * REACH)).rem_euclid(1.0);
        let at = pt(middle - REACH * u + 2.0 * REACH * u * share, horizon - c.next(60.0, 380.0) * u);
        let w = c.next(150.0, 320.0) * u;
        let puffs = c.next(6.0, 11.0) as usize;
        let mut lit = Vec::new();
        for _ in 0..puffs {
            let f = c.next(-1.0, 1.0);
            let r = w * c.next(0.12, 0.22) * (1.0 - 0.4 * f.abs());
            let p = at + pt(f * w * 0.5, -(1.0 - f * f) * w * c.next(0.05, 0.16));
            puff(l, p + pt(0.0, r * 0.35), r * 1.25, r * 0.7, CLOUD_SHADE, 0.55);
            lit.push((p, r));
        }
        for (p, r) in lit {
            puff(l, p + pt(-r * 0.1, -r * 0.12), r * 1.1, r * 0.85, CLOUD_LIT, 0.8);
        }
    }
    // The hills, from the furthest: each a smooth fill from its crest down, lit where it faces the
    // sun, the nearer with fields mown in stripes.
    let from = middle - REACH * u;
    let grid = 16.0 * u;
    // As far as `REACH` either side, and further if more shows (the page pushed to one side), on the
    // same points, so the hills within it are the same whatever shows.
    let first = ((area.left() - from) / grid).floor().min(0.0) as i64 - 1;
    let last = ((area.right() - from) / grid).ceil().max(2.0 * REACH * u / grid) as i64 + 1;
    let xs: Vec<f32> = (first..=last).map(|i| from + i as f32 * grid).collect();
    let n = xs.len() - 1;
    let all = hills(seed, land);
    for (j, h) in all.iter().enumerate() {
        let (base, high, haze, rows) = (h.base, h.high, h.haze, h.rows);
        let y = |x: f32| hill(&h.waves, base, x, middle, u);
        let color = h.color.lerp(HAZE, haze);
        let low = if j == all.len() - 1 { field } else { color.lerp(HAZE, 0.5) };
        let mut mesh = Mesh::new();
        for &x in &xs {
            let slope = (y(x + grid) - y(x - grid)) / (2.0 * grid);
            let lit = (0.5 - slope * 4.0).clamp(0.0, 1.0);
            mesh.vertex(pt(x, y(x)), color.lerp(SUNLIT.lerp(HAZE, haze), 0.35 * lit));
            mesh.vertex(pt(x, y(x) + high * u * 1.2), low);
            mesh.vertex(pt(x, ground), low);
        }
        for i in 0..n as u32 {
            let (a, b) = (3 * i, 3 * i + 3);
            mesh.triangle(a, a + 1, b);
            mesh.triangle(b, a + 1, b + 1);
            mesh.triangle(a + 1, a + 2, b + 1);
            mesh.triangle(b + 1, a + 2, b + 2);
        }
        l.mesh(mesh);
        if rows > 0.0 {
            for r in 1..9 {
                let Some((ink, wide)) = lines(r, haze) else { continue };
                let down = rows_apart(rows, r, u);
                l.path(xs.iter().map(|&x| pt(x, y(x) + down)).collect(), false, wide * u, ink);
            }
        }
        if j == 2 {
            // A farmhouse of pale stone with a red roof, cypresses by it and along the crests.
            let at = farm(seed, land);
            let (x, foot) = (at.x, at.y);
            let hz = |c: Color| c.lerp(HAZE, 0.35);
            let (w, tall) = (40.0 * u, 20.0 * u);
            l.polygon(Rect::from_min_max(pt(x - w / 2.0, foot - tall), pt(x + w / 2.0, foot)).corners(), hz(STONE));
            l.solid(vec![pt(x + w / 2.0, foot - tall), pt(x + w / 2.0 + 14.0 * u, foot - tall - 4.0 * u), pt(x + w / 2.0 + 14.0 * u, foot - 3.0 * u), pt(x + w / 2.0, foot)], hz(STONE.lerp(SHADOW, 0.25)));
            l.solid(vec![pt(x - w / 2.0 - 3.0 * u, foot - tall), pt(x + w / 2.0 + 3.0 * u, foot - tall), pt(x + w / 2.0 - 4.0 * u, foot - tall - 9.0 * u), pt(x - w / 2.0 + 4.0 * u, foot - tall - 9.0 * u)], hz(ROOF));
            l.solid(vec![pt(x + w / 2.0 + 3.0 * u, foot - tall), pt(x + w / 2.0 + 15.0 * u, foot - tall - 4.0 * u), pt(x + w / 2.0 - 4.0 * u, foot - tall - 9.0 * u)], hz(ROOF.lerp(SHADOW, 0.3)));
            for k in 0..3 {
                let wx = x - w / 2.0 + (8.0 + 12.0 * k as f32) * u;
                let corner = pt(wx, foot - tall + 6.0 * u);
                l.polygon(Rect::from_min_max(corner, corner + pt(3.5 * u, 5.0 * u)).corners(), hz(SHADOW.lerp(STONE, 0.2)));
            }
            for i in 0..14 {
                let near = i < 2;
                let cx = if near { x + (if i == 0 { -w * 0.75 } else { w * 0.85 }) } else { middle + s.next(-1.0, 1.0) * REACH * u };
                let tall = s.next(28.0, 50.0) * u * if near { 1.2 } else { 1.0 };
                let base = pt(cx, y(cx) + 3.0 * u);
                let dark = CYPRESS.lerp(HAZE, 0.38);
                flame(l, base, pt(0.0, -1.0), tall, tall * 0.12, dark);
                flame(l, base + pt(-tall * 0.03, -tall * 0.05), pt(0.0, -1.0), tall * 0.85, tall * 0.05, dark.lerp(SUNLIT, 0.25));
            }
        }
    }
    company(l);
}

/// The rows on the slope, laid out on the land, each waving gently over it, its own way.
struct Rows {
    land: Land,
    phase: [f32; DEPTHS],
}

impl Rows {
    fn new(seed: u64, land: Land) -> Rows {
        let mut s = Seeds::of(seed, 130_000);
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
    fn row(&self, l: usize, x: f32) -> f32 {
        let (u, k) = (self.land.u, Rows::k(l));
        self.land.ground - Rows::depth(l) * RISE * u + 5.0 * u * k * ((x - self.land.middle) / (560.0 * u) + self.phase[l]).sin()
    }

    /// How tall the vines are on row `l`.
    fn tall(&self, l: usize) -> f32 {
        TALL * self.land.u * Rows::k(l)
    }
}

/// A stretch of a row a chapter's grapes grow on: its row, from where to where across, and the side
/// of the page it is on (-1 the left).
#[derive(Clone, Copy)]
struct Place {
    level: usize,
    from: f32,
    to: f32,
    out: f32,
}

impl Place {
    /// Around its vines and the leaves fallen in front of them.
    fn bounds(&self, rows: &Rows) -> Rect {
        let h = rows.tall(self.level);
        let y = rows.row(self.level, (self.from + self.to) / 2.0);
        Rect::from_min_max(pt(self.from, y - 0.95 * h), pt(self.to, y + 0.3 * h))
    }

    /// Whether it is wholly on the desk, beside the page.
    fn in_view(&self, rows: &Rows, area: Rect, page: Rect) -> bool {
        let r = self.bounds(rows);
        area.contains_rect(r) && !r.overlaps_across(page)
    }

    /// How far `x` is within it, fading over its last few units at each end.
    fn within(&self, x: f32, u: f32) -> f32 {
        let edge = 14.0 * u;
        ((x - self.from) / edge).min((self.to - x) / edge).clamp(0.0, 1.0)
    }
}

/// The stretches on the nearest rows, in the order chapters take them: nearer the page first but on
/// every row, each a little off a grid so they do not line up. They come from the story and reach
/// as far from the page whatever the window, so a stretch never moves when it changes; those beyond
/// the desk are out of sight.
fn places(seed: u64, page: Rect, rows: &Rows) -> Vec<Place> {
    let mut s = Seeds::of(seed, 131_000);
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
/// at the top of its stalk; how long it is; and the numbers of its own for its berries.
fn bunches(seed: u64, k: usize, place: &Place, rows: &Rows) -> Vec<(Point, f32, Seeds)> {
    let mut s = Seeds::of(seed, 132_000 + k);
    let h = rows.tall(place.level);
    ORDER.iter().map(|&slot| {
        let x = place.from + (place.to - place.from) * (slot as f32 + 0.5 + s.next(-0.25, 0.25)) / BUNCHES as f32;
        let top = pt(x, rows.row(place.level, x) - h * s.next(0.47, 0.53));
        (top, h * s.next(0.19, 0.23), s.clone())
    }).collect()
}

/// A bunch of grapes hanging from `top`, `size` long once grown, `grown` from 0 (just set) to 1 (full
/// and ripe), and `last` how far it has darkened to the ripest purple, its bloom thicker: it swells
/// green first, then ripens. Each berry round, darker below and lit from the sun's side.
#[allow(clippy::too_many_arguments)]
fn bunch(l: &mut Layer, top: Point, size: f32, grown: f32, last: f32, mut s: Seeds, haze: f32) {
    let swell = ease(grown / 0.45);
    let ripe = ease((grown - 0.35) / 0.65);
    let size = size * (0.35 + 0.65 * swell);
    let hz = |c: Color| c.lerp(HAZE, haze);
    let dark = hz(UNRIPE_DARK.lerp(GRAPE_DARK, ripe).lerp(GRAPE_DARK.lerp(Color::BLACK, 0.3), last));
    let mid = hz(UNRIPE.lerp(GRAPE, ripe).lerp(RIPEST, last));
    let r = size * 0.11;
    l.line(top + pt(0.0, -size * 0.12), top + pt(0.0, size * 0.1), size * 0.05, hz(STALK));
    let lean = s.next(-0.12, 0.12);
    const ROWS: [usize; 7] = [4, 4, 4, 3, 3, 2, 1];
    for (i, &n) in ROWS.iter().enumerate() {
        let f = i as f32 / (ROWS.len() - 1) as f32;
        let w = size * 0.36 * (1.0 - 0.7 * f);
        let y = top.y + size * (0.14 + 0.13 * i as f32);
        for j in 0..n {
            let along = if n == 1 { 0.0 } else { j as f32 / (n - 1) as f32 * 2.0 - 1.0 };
            let c = pt(top.x + along * w * 0.75 + lean * size * f + s.next(-0.3, 0.3) * r, y + s.next(-0.3, 0.3) * r);
            let r = r * s.next(0.9, 1.1);
            l.circle(c, r, dark);
            l.circle(c + pt(-0.18, -0.2) * r, r * 0.76, mid);
            let shine = if ripe > 0.5 { hz(BLOOM).see_through(0.55 + 0.25 * last) } else { Color::rgb(226, 236, 170).see_through(0.6) };
            l.circle(c + pt(-0.4, -0.42) * r, r * (0.26 + 0.1 * last), shine);
        }
    }
}

/// `c` made lighter (above 1) or darker (below), its hue kept: shade darkens a turned leaf to a
/// deeper red, where mixing in a dark green made it brown.
fn shade(c: Color, f: f32) -> Color {
    let [r, g, b, a] = c.parts();
    let m = |v: u8| (v as f32 * f).round().clamp(0.0, 255.0) as u8;
    Color::premultiplied(m(r), m(g), m(b), a)
}

/// A vine leaf added to `mesh`: its stalk at `at`, `r` from there to the tip of its middle lobe,
/// pointing `towards`, coloured `color`, and squashed to `flat` of its height when it lies on the
/// ground: five pointed lobes with rounded bays between them and a notch where the stalk meets it,
/// lighter on the sun's side, darker at its heart; and its veins, if `veins`.
fn vine_leaf(mesh: &mut Mesh, at: Point, r: f32, towards: f32, color: Color, flat: f32, veins: bool) {
    // Each lobe's way from the heart, from the middle one, and how long it is.
    const LOBES: [(f32, f32); 5] = [(-1.8, 0.5), (-0.95, 0.82), (0.0, 1.0), (0.95, 0.82), (1.8, 0.5)];
    let place = |a: f32, d: f32| {
        let v = Point::angled(towards + a) * d;
        let heart = Point::angled(towards) * r * 0.36;
        at + pt(heart.x + v.x, (heart.y + v.y) * flat)
    };
    let light = pt(-0.55, -0.83);
    let lit = |a: f32| shade(color, 0.9 + 0.24 * Point::angled(towards + a).dot(light));
    let heart = mesh.vertex(place(0.0, 0.0), shade(color, 0.74));
    let mut rim = vec![(PI, 0.16 * r)];
    for (i, &(a, long)) in LOBES.iter().enumerate() {
        let bay = if i == 0 { a - 0.55 } else { (a + LOBES[i - 1].0) / 2.0 };
        rim.push((bay, if i == 0 { 0.3 } else { 0.42 } * r));
        let reach = long * 0.64 * r;
        rim.extend([(a - 0.15, reach * 0.76), (a, reach), (a + 0.15, reach * 0.76)]);
    }
    rim.push((LOBES[4].0 + 0.55, 0.3 * r));
    for &(a, d) in &rim {
        mesh.vertex(place(a, d), lit(a));
    }
    let n = rim.len() as u32;
    for i in 0..n {
        mesh.triangle(heart, heart + 1 + i, heart + 1 + (i + 1) % n);
    }
    if veins {
        let vein = color.lerp(LEAF_YELLOW, 0.3);
        for &(a, long) in &LOBES {
            let (from, to) = (place(a, 0.0), place(a, long * 0.55 * r));
            let side = (to - from).normalized().rot90() * r * 0.025;
            let i = mesh.vertex(from + side, vein);
            mesh.vertex(from - side, vein);
            mesh.vertex(to, vein);
            mesh.triangle(i, i + 1, i + 2);
        }
    }
}

/// The colour a leaf turns, one of the clear autumn colours, by where it hangs (`x` from the land's
/// middle, as the page moves when zooming) and a number of its own.
fn autumn(x: f32, u: f32, own: f32) -> Color {
    let m = 0.5 + 0.5 * (x / (37.0 * u)).sin() * (x / (11.0 * u) + 1.0).cos();
    TURNED[((m * 3.0 + own) as usize).min(TURNED.len() - 1)]
}

/// A leaf `p` of the way from its `green` to the autumn colour it turns, `to`: through gold, as a
/// leaf goes, never through the brown a straight mix of green and red passes.
fn turning(green: Color, to: Color, p: f32) -> Color {
    let p = p.clamp(0.0, 1.0);
    if p < 0.5 { green.lerp(LEAF_YELLOW, p * 2.0) } else { LEAF_YELLOW.lerp(to, p * 2.0 - 1.0) }
}

/// How far a leaf has turned, when its row has turned `turn`: each in its own moment, `own` from 0
/// to 1, slowly, rather than all at once or each in an instant.
fn turned_leaf(turn: f32, own: f32) -> f32 {
    ((turn - own * 0.6) / 0.4).clamp(0.0, 1.0)
}

/// A piece of wood from `a` to `b`, `wa` and `wb` wide there, lit on one side: two triangles of
/// `mesh`.
fn stick(mesh: &mut Mesh, a: Point, b: Point, wa: f32, wb: f32, c: Color) {
    let side = (b - a).normalized().rot90();
    let i = mesh.vertex(a + side * wa, c);
    mesh.vertex(a - side * wa, shade(c, 0.75));
    mesh.vertex(b - side * wb, shade(c, 0.75));
    mesh.vertex(b + side * wb, c);
    mesh.triangle(i, i + 1, i + 2);
    mesh.triangle(i, i + 2, i + 3);
}

/// A row of vines `l`, across what shows of `area`: its shadow on the ground towards the writer, the
/// old trunks up to the wire, the leaves turning gold and rust, lit from above where the sun comes
/// through them, the posts, and dry grass in front. Where `turned` says (from 0 to 1, by where along
/// the row), its leaves have turned gold and red, and fewer are left. Its pieces come from where they
/// stand, so whatever shows, each is the same.
fn vines(out: &mut Layer, seed: u64, l: usize, rows: &Rows, area: Rect, turned: &dyn Fn(f32) -> f32) {
    let (k, u, middle) = (Rows::k(l), rows.land.u, rows.land.middle);
    let h = rows.tall(l);
    let hz = |c: Color| c.lerp(HAZE, Rows::haze(l));
    let start = middle - REACH * u;
    let step = 0.5 * h;
    // What shows of it, however far from the page (pushed to one side, the page leaves a lot showing).
    let first = ((area.left() - start) / step).floor() as i64 - 1;
    let last = ((area.right() - start) / step).ceil() as i64 + 1;
    if first >= last {
        return;
    }
    let x_of = |i: i64| start + i as f32 * step;
    // Each piece's own seeds, from where it stands along the row.
    let seeds = |i: i64, what: usize| {
        let at = if i >= 0 { i as usize } else { 50_000_000 + i.unsigned_abs() as usize };
        Seeds::of(seed, 140_000 + l * 100_000 + at * 4 + what)
    };
    // Here and there along a row the leaves are going yellow.
    let colour = |x: f32| {
        let m = 0.5 + 0.5 * ((x - middle) / (130.0 * u) + l as f32).sin() * ((x - middle) / (53.0 * u) + 2.0 * l as f32).cos();
        LEAF_GREEN.lerp(LEAF_YELLOW, 0.1 + 0.6 * m)
    };
    // The shadow, falling towards the writer.
    let mut dusk = Mesh::new();
    for i in first..=last {
        let x = x_of(i);
        let y = rows.row(l, x);
        dusk.vertex(pt(x, y - 0.04 * h), hz(SHADOW).see_through(0.34));
        dusk.vertex(pt(x, y + 0.16 * h), hz(SHADOW).see_through(0.0));
    }
    for j in 0..(last - first) as u32 {
        let (a, b) = (2 * j, 2 * j + 2);
        dusk.triangle(a, a + 1, b);
        dusk.triangle(b, a + 1, b + 1);
    }
    out.mesh(dusk);
    // The trunks, gnarled, and the wire they are tied to.
    for i in first..=last {
        let mut s = seeds(i, 0);
        let x = x_of(i) + s.next(-0.06, 0.06) * h;
        let y = rows.row(l, x);
        let spine = [pt(x, y), pt(x + s.next(-0.03, 0.03) * h, y - 0.2 * h), pt(x + s.next(-0.04, 0.04) * h, y - 0.42 * h)];
        ribbon(out, &spine, &[0.04 * h, 0.032 * h, 0.028 * h], hz(TRUNK.lerp(FIELD, 0.15)));
    }
    let wire: Vec<Point> = (first..=last).map(|i| pt(x_of(i), rows.row(l, x_of(i)) - 0.42 * h)).collect();
    out.path(wire, false, (0.8 * k * u).max(0.5), hz(WIRE));
    // Behind the leaves, the shade deep within the vine, so it is dense where they overlap.
    let mut body = Mesh::new();
    let fine = 3;
    let n = (last - first) * fine;
    for j in 0..=n {
        let x = x_of(first) + j as f32 * step / fine as f32;
        let y = rows.row(l, x);
        let d = x - middle;
        let lump = 0.06 * (d / (14.0 * k * u)).sin() + 0.04 * (d / (37.0 * k * u) + 1.0).sin() - 0.04 * turned(x);
        // Its foot hangs lower here and there, and fades: a straight dark edge looked cut.
        let hem = 0.05 * (d / (9.0 * k * u) + 0.7).sin() + 0.03 * (d / (3.7 * k * u) + l as f32).sin() + 0.02;
        // Where it has turned, its depths are a warm rust, lit through the thinner leaves: darkening
        // a deep red made them all but black.
        let (c, lit) = (turning(colour(x), TURNED[2], 0.75 * turned(x)), 1.0 + 0.45 * turned(x));
        body.vertex(pt(x, y - (0.78 + lump) * h), hz(shade(c, 0.62 * lit)));
        body.vertex(pt(x, y - 0.56 * h), hz(shade(c, 0.4 * lit)));
        body.vertex(pt(x, y - (0.42 - hem) * h), hz(shade(c, 0.36 * lit)).see_through(0.3));
    }
    for j in 0..n as u32 {
        let (a, b) = (3 * j, 3 * j + 3);
        for r in 0..2 {
            body.triangle(a + r, a + r + 1, b + r);
            body.triangle(b + r, a + r + 1, b + r + 1);
        }
    }
    out.mesh(body);
    // The vine itself: its cordon along the wire, shoots rising from it, and leaves along each shoot
    // on short stalks, turning one by one where it has turned and fewer of them left; some further
    // in, in shade. Meshes, as there are thousands of leaves.
    // Veins on the nearest row only, and fewer, smaller-seeming leaves further up, where a leaf is a
    // few points across: the same look for far less to draw.
    let (near, far) = (l == 0, k < 0.75);
    let (mut wood, mut back, mut front) = (Mesh::new(), Mesh::new(), Mesh::new());
    for i in first..=last {
        let mut s = seeds(i, 1);
        let x0 = x_of(i);
        let wire = |x: f32| pt(x, rows.row(l, x) - 0.42 * h);
        // The old wood along the wire sags and bends between the trunks; drawn straight, it ruled a line.
        let bend = |x: f32| wire(x) + pt(0.0, 0.012 * h * ((x - middle) / (0.23 * step)).sin() * ((x - middle) / (0.71 * step) + 1.0).sin());
        for q in 0..3 {
            let (a, b) = (x0 + step * q as f32 / 3.0, x0 + step * (q + 1) as f32 / 3.0);
            stick(&mut wood, bend(a), bend(b), 0.013 * h, 0.013 * h, hz(CORDON));
        }
        let shoots = if far { 2 } else { 4 };
        for j in 0..shoots {
            let bx = x0 + (j as f32 + s.next(0.1, 0.9)) / shoots as f32 * step;
            let base = wire(bx);
            let (tall, lean, droop) = (s.next(0.36, 0.5) * h, s.next(-0.08, 0.08) * h, s.next(-0.06, 0.06) * h);
            let at = |t: f32| base + pt(lean * t + droop * t * t, -tall * t);
            stick(&mut wood, at(0.0), at(0.5), 0.008 * h, 0.006 * h, hz(CANE));
            stick(&mut wood, at(0.5), at(1.0), 0.006 * h, 0.004 * h, hz(CANE));
            let leaves = if far { 3 } else { 5 };
            for q in 0..leaves {
                let t = 0.18 + 0.82 * (q as f32 + s.next(0.0, 0.6)) / leaves as f32;
                let side = if q % 2 == 0 { -1.0 } else { 1.0 };
                let leaf = at(t) + pt(side * s.next(0.03, 0.07) * h, s.next(-0.02, 0.03) * h);
                // Of every size and every way, a few hanging down, as leaves are.
                let droops = s.next(0.0, 1.0) < 0.15;
                let towards = if droops { PI / 2.0 + s.next(-0.6, 0.6) } else { -PI / 2.0 + side * s.next(0.0, 1.3) + s.next(-0.3, 0.3) };
                let (r, gone, flips, own) = (s.next(0.1, 0.17) * h, s.next(0.0, 1.0), s.next(0.0, 1.0), s.next(0.0, 1.2));
                let (inside, yellow) = (s.next(0.0, 1.0) < 0.4, s.next(0.0, 1.0));
                let turn = turned(leaf.x);
                // Lighter at the top where the sun is, each leaf a green of its own.
                let green = LEAF_GREEN.lerp(LEAF_LIT, (0.15 + 0.35 * t + s.next(-0.2, 0.25)).clamp(0.0, 0.75));
                // A few fall as the row turns; most stay on, so it is never bare.
                if gone < 0.12 * turn {
                    continue;
                }
                let green = green.lerp(LEAF_YELLOW, (colour(leaf.x).parts()[0] as f32 / 255.0 - 0.45).max(0.0) * yellow);
                let c = turning(green, autumn(leaf.x - middle, u, own), turned_leaf(turn, flips));
                stick(&mut wood, at(t), leaf, 0.004 * h, 0.003 * h, hz(CANE));
                if inside {
                    vine_leaf(&mut back, leaf, r * 0.9, towards, hz(shade(c, 0.68)), 1.0, false);
                } else {
                    vine_leaf(&mut front, leaf, r, towards, hz(c), 1.0, near);
                }
            }
        }
        // Leaves hang over the wire and below it, a few here and none there, so the vine's foot is
        // ragged, as a real one's is, and never a clean line along the wire.
        let mut s = seeds(i, 3);
        for _ in 0..if far { 3 } else { 6 } {
            let x = x0 + s.next(0.0, 1.0) * step;
            let leaf = wire(x) + pt(s.next(-0.03, 0.03) * h, s.next(-0.07, 0.03) * h);
            let towards = PI / 2.0 + s.next(-1.0, 1.0);
            let (r, gone, flips, own, inside, bare) = (s.next(0.08, 0.15) * h, s.next(0.0, 1.0), s.next(0.0, 1.0), s.next(0.0, 1.2), s.next(0.0, 1.0) < 0.4, s.next(0.0, 1.0) < 0.3);
            let turn = turned(x);
            // In the vine's shade, a darker green than the leaves the sun reaches.
            let green = LEAF_GREEN.lerp(LEAF_LIT, s.next(0.0, 0.25));
            if bare || gone < 0.12 * turn {
                continue;
            }
            let c = turning(green, autumn(x - middle, u, own), turned_leaf(turn, flips));
            stick(&mut wood, bend(x), leaf, 0.004 * h, 0.003 * h, hz(CANE));
            if inside {
                vine_leaf(&mut back, leaf, r * 0.9, towards, hz(shade(c, 0.68)), 1.0, false);
            } else {
                vine_leaf(&mut front, leaf, r, towards, hz(shade(c, 0.85)), 1.0, near);
            }
        }
    }
    out.mesh(back);
    out.mesh(wood);
    out.mesh(front);
    // The posts, every few trunks, in front of the leaves.
    for i in (first..=last).filter(|i| i.rem_euclid(6) == 3) {
        let x = x_of(i) + 0.5 * step;
        let y = rows.row(l, x);
        out.line(pt(x, y + 0.01 * h), pt(x, y - 0.94 * h), 0.035 * h, hz(POST));
        out.line(pt(x - 0.01 * h, y), pt(x - 0.01 * h, y - 0.94 * h), 0.012 * h, hz(POST.lerp(SUNLIT, 0.3)));
    }
    // Dry grass on the ground in front of it, and a little green where the vines shade it.
    for i in first..=last {
        let mut s = seeds(i, 2);
        for _ in 0..3 {
            let x = x_of(i) + s.next(-0.5, 0.5) * step;
            let y = rows.row(l, x) + s.next(0.0, 0.3) * h;
            let c = if s.next(0.0, 1.0) < 0.3 { LEAF_GREEN.lerp(FIELD, 0.5) } else { FIELD.lerp(SUNLIT, s.next(0.1, 0.5)) };
            for b in 0..2 {
                let lean = (b as f32 - 0.5) * 0.8 + s.next(-0.2, 0.2);
                let tip = pt(x + lean * 0.04 * h, y - s.next(0.03, 0.06) * h);
                out.line(pt(x + b as f32 * 0.008 * h, y), tip, 0.008 * h, hz(c));
            }
        }
    }
}

/// The leaves falling from chapter `k`'s stretch as it turns, `t` seconds after the next chapter
/// began: each lets go in its own moment, drifts down, swaying and turning over as it goes, and
/// comes to lie in the grass in front of the row. Each one's number, where it is, how far it is
/// turned, and whether it lies on the ground; none before it lets go.
fn falling(seed: u64, k: usize, place: &Place, rows: &Rows, t: f32) -> Vec<(usize, Point, f32, bool)> {
    let h = rows.tall(place.level);
    (0..FALLING).filter_map(|i| {
        let mut s = Seeds::of(seed, 165_000 + k * 64 + i);
        let x = place.from + (place.to - place.from) * s.next(0.05, 0.95);
        let from = pt(x, rows.row(place.level, x) - h * s.next(0.5, 0.85));
        let to = pt(x + place.out * s.next(0.0, 0.5) * h, rows.row(place.level, x) + h * s.next(0.04, 0.26));
        let (start, long) = (1.0 + (TURN - 5.0) * i as f32 / FALLING as f32 + s.next(0.0, 0.6), s.next(2.8, 3.8));
        let f = (t - start) / long;
        if f < 0.0 {
            return None;
        }
        let f = f.min(1.0);
        let sway = (f * PI * s.next(2.0, 3.5)).sin() * 0.18 * h * (1.0 - f);
        let at = pt(from.x + (to.x - from.x) * f + sway, from.y + (to.y - from.y) * ease(f));
        Some((i, at, s.next(0.0, 6.0) + f * s.next(4.0, 9.0), f >= 1.0))
    }).collect()
}

/// A starling murmuration at `time`, if one is in the sky: a minute or two apart it comes in from
/// one side over the hills, its flock stretching, folding and turning as it swoops across, and is
/// gone at the other side. Where each bird is, and how far through its wingbeat. The flock's shape
/// and course are functions of the time since it came, so a bird never jumps.
fn starlings(seed: u64, land: &Land, time: f64) -> Vec<(Point, f32)> {
    let since = time.rem_euclid(FLOCK_EVERY as f64) as f32 - (FLOCK_EVERY - FLOCK);
    if !(0.0..FLOCK).contains(&since) {
        return Vec::new();
    }
    let u = land.u;
    let round = (time / FLOCK_EVERY as f64).floor() as usize;
    let mut s = Seeds::of(seed, 166_000 + round);
    let dir = if round.is_multiple_of(2) { 1.0 } else { -1.0 };
    let q = since / FLOCK;
    let horizon = land.ground - RISE * u - 150.0 * u;
    let centre = pt(land.middle + dir * (q * 2.0 - 1.0) * 950.0 * u, horizon - (s.next(70.0, 120.0) + 45.0 * (q * PI * 1.6).sin()) * u);
    // Thinner as it comes in and goes, so it gathers and disperses rather than appearing.
    let gathered = (q / 0.12).min((1.0 - q) / 0.12).clamp(0.0, 1.0);
    let t = since;
    let size = (90.0 + 40.0 * (t * 0.27).sin()) * u;
    let (turn, wide, high) = (0.6 * (t * 0.35).sin(), 1.0 + 0.7 * (t * 0.5).sin(), 0.45 + 0.3 * (t * 0.43).cos());
    (0..BIRDS).filter_map(|i| {
        let mut b = Seeds::of(seed, 167_000 + i);
        let (a, r) = (b.next(0.0, 2.0 * PI), b.next(0.0, 1.0).sqrt());
        if b.next(0.0, 1.0) > gathered {
            return None;
        }
        let (x, y) = (a.cos() * r * wide, a.sin() * r * high + 0.35 * (3.0 * a.cos() * r + t * 1.3).sin());
        let (c, sn) = (turn.cos(), turn.sin());
        let at = centre + pt(x * c - y * sn, x * sn + y * c) * size;
        Some((at, (t * 11.0 + b.next(0.0, 6.0)).sin()))
    }).collect()
}

/// The starlings as one mesh: each a tiny dark bird, its wings up or down as it beats them.
fn flock(l: &mut Layer, birds: &[(Point, f32)], u: f32) {
    let mut mesh = Mesh::new();
    for &(at, beat) in birds {
        let (span, lift) = (1.8 * u, beat * 0.9 * u);
        let i = mesh.vertex(at + pt(-span, -lift), STARLING);
        mesh.vertex(at + pt(0.0, -0.35 * u), STARLING);
        mesh.vertex(at + pt(span, -lift), STARLING);
        mesh.vertex(at + pt(0.0, 0.45 * u), STARLING);
        mesh.triangle(i, i + 1, i + 3);
        mesh.triangle(i + 1, i + 2, i + 3);
    }
    l.mesh(mesh);
}

/// The land behind the vines: the country, with rows of vines along the nearer hills and, now and
/// then, the starlings in the sky; then the slope the rows stand on, darker towards the writer.
/// Only its fills reach to `area`'s edges.
fn landscape(l: &mut Layer, seed: u64, area: Rect, land: &Land, time: f64) {
    let lines = |_: usize, haze: f32| Some((ROWS_FAR.lerp(HAZE, haze).see_through(0.55), 0.9 * (1.0 - haze)));
    let birds = |l: &mut Layer| {
        let flying = starlings(seed, land, time);
        if !flying.is_empty() {
            flock(l, &flying, land.u);
        }
    };
    country(l, seed, area, land, time, FIELD, &lines, &birds);
    let (u, ground) = (land.u, land.ground);
    let top = ground - RISE * u - 10.0 * u;
    let mut field = Mesh::new();
    field.vertex(pt(area.left(), top), FIELD.lerp(HAZE, 0.15));
    field.vertex(pt(area.right(), top), FIELD.lerp(HAZE, 0.15));
    field.vertex(pt(area.right(), ground), FIELD_LOW);
    field.vertex(pt(area.left(), ground), FIELD_LOW);
    field.triangle(0, 1, 2);
    field.triangle(0, 2, 3);
    l.mesh(field);
    l.polygon(Rect::from_min_max(pt(area.left(), ground), area.max).corners(), FIELD_LOW);
}

/// What the Vineyard keeps while a story is shown: the stretches as their grapes grow (`Tended`).
struct Vineyard {
    vines: Tended,
}

impl Theme for Vineyard {
    fn new() -> Self {
        Vineyard { vines: Tended::default() }
    }

    fn event(&mut self, event: &Event) {
        // Picked, or a story shown: everything as it stands, nothing growing.
        if let Event::Fresh = event {
            self.vines.forget();
        }
    }

    fn frame(&mut self, input: &Input, out: &mut Out) {
        // The plain fills reach the edges of what shows, and a stretch is planted where it is seen.
        let (now, u, shown, page) = (input.time, input.unit, input.shown, input.page);
        let (seed, garden) = (host::story_seed(), host::garden());
        let n = garden.len().max(1);
        out.ground = GROUND;
        // While a bunch swells or a stretch turns, every frame; else the clouds drift and the
        // starlings fly.
        out.next = if self.vines.tend(seed, &garden, BUNCHES, RIPEN, TURN, now) { 0 } else { 40 };
        let land = Land { middle: input.ground.x, ground: input.ground.y, u };
        let rows = Rows::new(seed, land);
        let l = &mut out.desk;
        landscape(l, seed, shown, &land, now);
        let all = places(seed, page, &rows);
        let placed: Vec<Place> = (0..n).map(|k| all[self.vines.place(k, all.len(), |p| all[p].in_view(&rows, shown, page))]).collect();
        let turning = self.vines.finishing(now, TURN);
        // How far each chapter's stretch has turned: not the newest, the one before as it turns,
        // every other all the way.
        let turned: Vec<f32> = (0..n).map(|k| match turning {
            _ if k + 1 >= n => 0.0,
            Some((m, q)) if m == k => between(q * TURN, 0.0, TURN * 0.7),
            _ => 1.0,
        }).collect();
        // Row by row from the furthest up the slope: its vines, turned where its stretches have, the
        // grapes on its stretches, and the leaves fallen in front of it.
        for row in (0..DEPTHS).rev() {
            let on = |x: f32| placed.iter().zip(&turned).filter(|(p, _)| p.level == row).map(|(p, &a)| a * p.within(x, u)).fold(0.0, f32::max);
            vines(l, seed, row, &rows, shown, &on);
            for (k, place) in placed.iter().enumerate().filter(|(_, p)| p.level == row) {
                let all = bunches(seed, k, place, &rows);
                let pages = garden.get(k).map_or(1, |&p| (p as usize).min(BUNCHES));
                let (hanging, last): (Vec<(usize, f32)>, f32) = if k + 1 >= n {
                    let grown = self.vines.grown(&garden, k, BUNCHES);
                    ((0..grown.ceil() as usize).map(|j| (j, (grown - j as f32).clamp(0.0, 1.0))).collect(), 0.0)
                } else {
                    ((0..pages).map(|j| (j, 1.0)).collect(), turned[k])
                };
                for &(j, grown) in &hanging {
                    let (top, size, seeds) = all[j].clone();
                    bunch(l, top, size, grown, last, seeds, Rows::haze(row));
                }
                if k + 1 < n {
                    let t = match turning {
                        Some((m, q)) if m == k => q * TURN,
                        _ => TURN,
                    };
                    let r = 0.065 * rows.tall(row);
                    let mut mesh = Mesh::new();
                    for (i, at, turn, lying) in falling(seed, k, place, &rows, t) {
                        let c = autumn(at.x - land.middle, u, (i % 4) as f32 * 0.3).lerp(HAZE, Rows::haze(row));
                        // Turning over as it falls, it shows narrower; lying, it is flat on the grass.
                        let (flat, c) = if lying { (0.4, shade(c, 0.85)) } else { (0.25 + 0.75 * (turn * 1.7).sin().abs(), c) };
                        vine_leaf(&mut mesh, at, r, turn, c, flat, false);
                    }
                    l.mesh(mesh);
                }
            }
        }
    }
}

theme!(Vineyard);
