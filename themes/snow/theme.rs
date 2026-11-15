//! The Magic Snow, as a theme program: Caprice's own (`src/eggs/snow.rs`) written against the SDK,
//! drawing the same shapes (a test renders both and compares them frame by frame), and the example
//! a theme of the countryside can be made from (`tests/fixtures/snow.wasm`, by `themes/build.sh`).
//!
//! A snowy hillside at twilight lies on the desk: a few stars and the moon come out over hills with
//! pines on them, a frozen lake lies along the foot of the desk, under the page and either side of
//! it, the whole of it in the window, and snow falls softly in front of everything. Each chapter
//! has an igloo on the hillside beside the page, some near and big, some further up the hill,
//! smaller and bluer in the dusk. Each page of the chapter being written cuts a block of ice out of
//! the lake: a glint traces round it, it rises out of the water, floats up to the chapter's igloo
//! and becomes its next brick, course by course from the bottom up, and the hole freezes over. When
//! the next chapter begins, the igloo before is finished (the bricks it lacks drop in, its tunnel
//! is built) and a warm light comes on inside it, glowing through the doorway and the joints.
//! Nothing brings a title in this theme.
//!
//! Everything on the hillside is laid out as at the fitting zoom, from the ground under the page's
//! middle (`Input::ground`, 14 units above the foot of the desk: `Out::ground`), so Caprice zooms
//! it about the ground and the hillside comes nearer with the page, as through a camera. The lake
//! fits the window as it is at the fitting zoom (`Input::window`), so the whole of it shows, and
//! comes nearer with the rest when zoomed. The snow falls in the window (`Out::window`), never
//! zoomed.

use std::f32::consts::PI;

use theme_sdk::{Color, Event, Input, Layer, Mesh, Out, Point, Rect, Seeds, Tended, Theme, ease, host, pt, theme};

const SNOW: Color = Color::rgb(250, 252, 255);
const SNOW_SHADE: Color = Color::rgb(192, 204, 232);
const HILL: Color = Color::rgb(214, 222, 244);
const HILL_FAR: Color = Color::rgb(160, 172, 214);
const PINE: Color = Color::rgb(70, 92, 128);
const STAR: Color = Color::rgb(255, 250, 230);
const MOON: Color = Color::rgb(250, 246, 226);
/// The dusk that pales and blues what lies further up the hill.
const DUSK: Color = Color::rgb(120, 130, 182);
/// The lake: its ice far off, mirroring the dusk, and near, and the water under a hole cut in it.
const LAKE_FAR: Color = Color::rgb(132, 150, 204);
const LAKE_NEAR: Color = Color::rgb(196, 214, 242);
const WATER: Color = Color::rgb(30, 44, 92);
/// Ice cut and carved: in the light, clear, in shade and deep within.
const ICE_LIT: Color = Color::rgb(240, 250, 255);
const ICE: Color = Color::rgb(168, 212, 244);
const ICE_SHADE: Color = Color::rgb(92, 142, 206);
const ICE_DEEP: Color = Color::rgb(46, 86, 164);
const GLINT: Color = Color::rgb(250, 254, 255);

/// How far above the foot of the desk the ground is, in units: the lake lies along it, and
/// Caprice zooms about it.
const GROUND: f32 = 14.0;
/// A block's width, the height of its front and the depth of its top, in units.
const BLOCK: (f32, f32, f32) = (17.0, 10.0, 4.0);
/// Seconds a block takes to be cut, rise and float to its igloo, and an igloo to be finished and lit.
const HANG: f32 = 2.6;
const FINISH: f32 = 6.0;
/// How far the places reach from the page whatever the window, and how far up the hill they go,
/// in units; how many depths there are.
const REACH: f32 = 1800.0;
const RISE: f32 = 100.0;
const DEPTHS: usize = 5;
/// How big the igloos are beside the hillside.
const SIZE: f32 = 1.3;

/// How many bricks make an igloo, one for each page of its chapter: rows of these many from the
/// bottom up, the last its cap; how wide and how high its dome is, in units.
const BRICKS: usize = 16;
const COURSES: [usize; 5] = [5, 4, 3, 3, 1];
const DOME: (f32, f32) = (40.0, 34.0);
/// The light inside an igloo: at its brightest, and deeper, as through the doorway.
const WARM: Color = Color::rgb(255, 196, 110);
const EMBER: Color = Color::rgb(232, 120, 52);
/// The bricks: in the light, and in shade.
const BRICK: Color = Color::rgb(248, 251, 255);
const BRICK_SHADE: Color = Color::rgb(170, 188, 226);

/// A convex shape filled from colour to colour across it, `colors` at its `points`: one mesh.
fn painted(l: &mut Layer, points: &[Point], colors: &[Color]) {
    let mut mesh = Mesh::new();
    for (&p, &c) in points.iter().zip(colors) {
        mesh.vertex(p, c);
    }
    for i in 1..points.len() as u32 - 1 {
        mesh.triangle(0, i, i + 1);
    }
    l.mesh(mesh);
}

/// A point along the curve from `a` to `b` pulled towards `c`, at `s` from 0 to 1.
fn bend(a: Point, c: Point, b: Point, s: f32) -> Point {
    let m = |p: Point, q: Point| p + (q - p) * s;
    m(m(a, c), m(c, b))
}

/// The frozen lake as laid out at the fitting zoom: across the window, a little in from its
/// edges, so the whole of it shows, and just above the foot of the desk; `u` its unit.
struct Lake {
    left: f32,
    right: f32,
    ground: f32,
    u: f32,
}

impl Lake {
    fn of(window: Rect, ground: f32, u: f32) -> Lake {
        Lake { left: window.left() + 36.0 * u, right: window.right() - 36.0 * u, ground, u }
    }

    /// How far from its ends `x` is, from 0 at an end to 1 in its middle, rounded so that its
    /// shores meet in a curve.
    fn round(&self, x: f32) -> f32 {
        let d = (2.0 * x - self.left - self.right) / (self.right - self.left);
        (1.0 - d * d).max(0.0).powf(0.4)
    }

    /// The level its shores meet at, at its ends.
    fn waist(&self) -> f32 {
        self.ground - 32.0 * self.u
    }

    /// Its far shore at `x`, a little uneven.
    fn far(&self, x: f32) -> f32 {
        let (u, f, d) = (self.u, self.round(x), (x - self.left) / self.u);
        self.waist() - 34.0 * u * f + (2.0 * (d / 37.0).sin() + 1.5 * (d / 91.0 + 1.0).sin()) * u * f
    }

    /// Its near shore at `x`.
    fn near(&self, x: f32) -> f32 {
        self.waist() + 24.0 * self.u * self.round(x)
    }

    /// Where block `j` of chapter `k` is cut out of it, below `x` (kept well within it).
    fn cut_at(&self, seed: u64, k: usize, j: usize, x: f32) -> Point {
        let mut s = Seeds::of(seed, 99_700 + k * BRICKS + j);
        let u = self.u;
        let x = (x + s.next(-40.0, 40.0) * u).clamp(self.left + 70.0 * u, self.right - 70.0 * u);
        let (far, near) = (self.far(x) + 10.0 * u, self.near(x) - 8.0 * u);
        pt(x, far + (near - far) * s.next(0.2, 0.8))
    }

    /// The lake: its ice mirroring the dusk far off and paler near, the moon's light lying on it
    /// below `moon`, faint scratches in it, and snow heaped along both its shores.
    fn draw(&self, l: &mut Layer, seed: u64, moon: f32, time: f64) {
        let u = self.u;
        let n = 96;
        let at = |i: usize| self.left + (self.right - self.left) * i as f32 / n as f32;
        let mut mesh = Mesh::new();
        for i in 0..=n {
            let x = at(i);
            mesh.vertex(pt(x, self.far(x)), LAKE_FAR);
            mesh.vertex(pt(x, self.near(x)), LAKE_NEAR);
        }
        for i in 0..n as u32 {
            let (a, b) = (2 * i, 2 * i + 2);
            mesh.triangle(a, a + 1, b);
            mesh.triangle(b, a + 1, b + 1);
        }
        l.mesh(mesh);
        // The moon's light on the ice, shimmering a little.
        if moon > self.left + 60.0 * u && moon < self.right - 60.0 * u {
            let shine = 0.3 + 0.04 * (time as f32 * 0.8).sin();
            l.radial(pt(moon, (self.far(moon) + self.near(moon)) / 2.0), 46.0 * u, 16.0 * u, MOON, shine);
        }
        let mut s = Seeds::of(seed, 97_500);
        for _ in 0..40 {
            let x = self.left + (self.right - self.left) * s.next(0.1, 0.85);
            let y = self.far(x) + (self.near(x) - self.far(x)) * s.next(0.25, 0.8);
            let (len, tilt) = (s.next(10.0, 40.0) * u, s.next(-0.08, 0.08));
            l.line(pt(x, y), pt(x + len, y + len * tilt), 0.6 * u, ICE_LIT.see_through(s.next(0.15, 0.35)));
        }
        // Snow heaped along its shores, in shade underneath, smaller round its ends.
        for (shore, big) in [(0, 1.0), (1, 0.7)] {
            let mut x = self.left;
            while x < self.right {
                let f = self.round(x).max(0.3);
                let w = s.next(14.0, 30.0) * u * f;
                let y = if shore == 0 { self.far(x) } else { self.near(x) };
                l.oval(pt(x, y + 1.6 * u), pt(1.0, 0.0), w, s.next(2.5, 4.0) * u * big, SNOW_SHADE);
                l.oval(pt(x, y), pt(1.0, 0.0), w, s.next(2.5, 4.5) * u * big, SNOW);
                x += w * s.next(1.0, 1.6);
            }
        }
    }
}

/// A block of ice standing on `at` (the middle of its foot), `u` its size, faded to `alpha`, lit
/// up `bright` and blued `haze` by the dusk: its front shaded from the light on the left, its top
/// in the light, a clear edge and a streak of light in it.
fn block(l: &mut Layer, at: Point, u: f32, alpha: f32, bright: f32, haze: f32) {
    let (w, h, d) = (BLOCK.0 * u / 2.0, BLOCK.1 * u, BLOCK.2 * u);
    let c = |col: Color, a: f32| col.lerp(ICE_LIT, bright).lerp(DUSK, haze).see_through(a * alpha);
    let top = at.y - h;
    let front = [pt(at.x - w, top), pt(at.x + w, top), pt(at.x + w, at.y), pt(at.x - w, at.y)];
    let lid = [pt(at.x - w, top), pt(at.x - w + 1.5 * u, top - d), pt(at.x + w - 1.5 * u, top - d), pt(at.x + w, top)];
    painted(l, &front, &[c(ICE, 0.9), c(ICE_SHADE, 0.85), c(ICE_DEEP, 0.85), c(ICE_SHADE, 0.85)]);
    painted(l, &lid, &[c(ICE_LIT, 0.95), c(ICE_LIT, 0.95), c(ICE, 0.9), c(ICE, 0.9)]);
    l.line(front[0], front[1], 0.7 * u, c(ICE_LIT, 0.9));
    l.line(front[0], front[3], 0.6 * u, c(ICE_LIT, 0.6));
    l.line(pt(at.x - w + 2.5 * u, top + 2.0 * u), pt(at.x - w + 6.0 * u, at.y - 2.0 * u), 0.8 * u, c(Color::WHITE, 0.35));
}

/// A chapter's igloo: where it stands, its size, how much the dusk blues it, which side its
/// doorway is on (towards the page), and a number of its own that sets how its light flickers.
struct Igloo {
    foot: Point,
    u: f32,
    haze: f32,
    face: f32,
    phase: f32,
}

impl Igloo {
    /// Chapter `k`'s igloo at `place`, its doorway to `face`.
    fn of(seed: u64, k: usize, place: &Place, face: f32) -> Igloo {
        Igloo { foot: place.foot, u: place.size, haze: place.haze, face, phase: Seeds::of(seed, 99_500 + k).next(0.0, 40.0) }
    }

    fn hazed(&self, c: Color) -> Color {
        c.lerp(DUSK, self.haze)
    }

    /// The point `(x, y)` of the igloo, in its units from its foot, its doorway turned to `face`.
    fn point(&self, (x, y): (f32, f32)) -> Point {
        self.foot + pt(x * self.face, y) * self.u
    }

    /// A point of the dome, `phi` of the way up it (an angle) and `s` across (from -1 to 1).
    fn dome(phi: f32, s: f32) -> (f32, f32) {
        (DOME.0 * phi.cos() * s, -DOME.1 * phi.sin())
    }

    /// Brick `j`, in the igloo's units: a band of its course, narrower towards the sides as the
    /// dome turns away, with its outer edge bowed out to the dome's round; the cap is the top of it.
    fn brick(j: usize) -> Vec<(f32, f32)> {
        let (mut row, mut i) = (0, j.min(BRICKS - 1));
        while row + 1 < COURSES.len() && i >= COURSES[row] {
            i -= COURSES[row];
            row += 1;
        }
        let step = PI / 2.0 / COURSES.len() as f32;
        let (lo, hi) = (row as f32 * step, (row + 1) as f32 * step);
        if row + 1 == COURSES.len() {
            return (0..=8).map(|m| PI * m as f32 / 8.0).map(|a| (DOME.0 * lo.cos() * -a.cos(), -DOME.1 * (lo.sin() + (1.0 - lo.sin()) * a.sin()))).collect();
        }
        let n = COURSES[row] as f32;
        let across = |m: f32| -(PI * m / n).cos();
        let (a, b) = (across(i as f32), across(i as f32 + 1.0));
        let mut out = vec![Self::dome(lo, a), Self::dome(lo, b)];
        if i + 1 == COURSES[row] {
            out.push(Self::dome((lo + hi) / 2.0, 1.0));
        }
        out.extend([Self::dome(hi, b), Self::dome(hi, a)]);
        if i == 0 {
            out.push(Self::dome((lo + hi) / 2.0, -1.0));
        }
        out
    }

    /// Where brick `j` sits: the middle of its foot, where the block that becomes it lands.
    fn slot(&self, j: usize) -> Point {
        let b = Self::brick(j);
        let x = b.iter().map(|p| p.0).sum::<f32>() / b.len() as f32;
        let y = b.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        self.point((x, y))
    }

    /// One brick, `alpha` of the way there, lowered from `drop` units above, with the light inside
    /// showing in the joint round it as `lit`: the joint is the brick at full size, the brick itself
    /// a little within it, shaded from the light on the left and the top.
    fn draw_brick(&self, l: &mut Layer, j: usize, alpha: f32, drop: f32, lit: f32) {
        let poly: Vec<(f32, f32)> = Self::brick(j).into_iter().map(|(x, y)| (x, y - drop)).collect();
        let n = poly.len() as f32;
        let c = poly.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
        let joint = self.hazed(BRICK_SHADE.lerp(WARM, lit));
        l.polygon(poly.iter().map(|&p| self.point(p)).collect(), joint.see_through(alpha));
        let inset = |(x, y): (f32, f32)| {
            let (dx, dy) = (x - c.0, y - c.1);
            let l = (dx * dx + dy * dy).sqrt().max(0.01);
            let k = (1.0 - 1.3 / l).max(0.0);
            (c.0 + dx * k, c.1 + dy * k)
        };
        let shade = |(x, y): (f32, f32)| {
            let l = (0.55 - 0.45 * x * self.face / DOME.0 + 0.35 * (-y / DOME.1 - 0.5)).clamp(0.0, 1.0);
            self.hazed(BRICK_SHADE.lerp(BRICK, l).lerp(WARM, 0.12 * lit)).see_through(alpha)
        };
        let inner: Vec<(f32, f32)> = poly.iter().map(|&p| inset(p)).collect();
        let mut mesh = Mesh::new();
        mesh.vertex(self.point(inset(c)), shade(c));
        for &p in &inner {
            mesh.vertex(self.point(p), shade(p));
        }
        let m = inner.len() as u32;
        for i in 1..=m {
            mesh.triangle(0, i, i % m + 1);
        }
        l.mesh(mesh);
    }

    /// The igloo with `bricks` laid (whole ones, and the newest coming in as its block lands),
    /// `finished` from 0 to 1 once its chapter is done: the bricks it lacks drop in one after
    /// another, its tunnel is built, and a light comes on inside, glowing through the doorway and
    /// the joints and on the snow before it, flickering a little.
    fn draw(&self, l: &mut Layer, bricks: f32, finished: f32, time: f64) {
        let (t, u, q) = (time as f32, self.u, finished);
        let laid = (bricks.floor() as usize).min(BRICKS);
        let lit = ease((q - 0.65) / 0.25) * (0.9 + 0.1 * (t * 5.0 + self.phase).sin() * (t * 3.3).sin());
        if laid > 0 || q > 0.0 {
            l.radial(self.foot + pt(8.0, -1.0) * u, DOME.0 * 1.3 * u, 7.0 * u, self.hazed(SNOW_SHADE), 0.8);
        }
        if lit > 0.0 {
            l.radial(self.point((0.0, -DOME.1 * 0.5)), DOME.0 * 2.0 * u, DOME.1 * 0.9 * u, self.hazed(WARM), 0.22 * lit);
        }
        for j in 0..BRICKS {
            if j < laid {
                self.draw_brick(l, j, 1.0, 0.0, lit);
            } else if j == laid && bricks.fract() > 0.9 && q == 0.0 {
                // The newest, as its block lands.
                self.draw_brick(l, j, (bricks.fract() - 0.9) / 0.1, 0.0, 0.0);
            } else if q > 0.0 {
                // Those it lacks, once its chapter is done, dropping in one after another.
                let at = 0.45 * (j - laid) as f32 / (BRICKS - laid).max(1) as f32;
                let p = ease((q - at) / 0.1);
                if p > 0.0 {
                    self.draw_brick(l, j, p, 18.0 * (1.0 - p), lit);
                    if p < 1.0 {
                        let land = self.slot(j);
                        l.radial(land - pt(0.0, 4.0 * u), 12.0 * u, 8.0 * u, GLINT, 0.8 * p);
                    }
                }
            }
        }
        let tunnel = ease((q - 0.5) / 0.12);
        if tunnel > 0.0 {
            // The tunnel before the doorway, on the page's side, rising from the snow.
            let (cx, rx, ry) = (DOME.0 * 0.62, 15.0, 18.0 * tunnel);
            let arch = |rx: f32, ry: f32| (0..=12).map(|m| PI * m as f32 / 12.0).map(|a| self.point((cx - rx * a.cos(), -ry * a.sin()))).collect::<Vec<_>>();
            let outer = arch(rx, ry);
            let colors: Vec<Color> = (0..outer.len()).map(|m| self.hazed(BRICK_SHADE.lerp(BRICK, 0.35 + 0.5 * (m as f32 / 12.0))).see_through(tunnel)).collect();
            let mut mesh = Mesh::new();
            let base = self.point((cx, 0.0));
            mesh.vertex(base, self.hazed(BRICK_SHADE.lerp(BRICK, 0.6)).see_through(tunnel));
            for (&p, &c) in outer.iter().zip(&colors) {
                mesh.vertex(p, c);
            }
            for m in 1..outer.len() as u32 {
                mesh.triangle(0, m, m + 1);
            }
            l.mesh(mesh);
            l.path(arch(rx - 3.0, ry - 3.0 * tunnel), false, 0.8 * u, self.hazed(BRICK_SHADE.lerp(WARM, lit)).see_through(tunnel));
            // The doorway, dark until the light comes on, then warm from within.
            let door = arch(8.0, 12.0 * tunnel);
            let mut mesh = Mesh::new();
            let dark = self.hazed(WATER);
            mesh.vertex(base, dark.lerp(WARM, lit));
            for &p in &door {
                mesh.vertex(p, dark.lerp(EMBER, 0.85 * lit));
            }
            for m in 1..door.len() as u32 {
                mesh.triangle(0, m, m + 1);
            }
            l.mesh(mesh);
            if lit > 0.0 {
                l.radial(base + pt(self.face * 4.0, 2.0) * u, 30.0 * u, 7.0 * u, self.hazed(WARM), 0.45 * lit);
            }
        }
    }

    /// Block `j` on its way, `g` of the way (from 0 to 1), cut out of the lake at `from` where its
    /// size is `lake_u`: a glint traces round it in the ice, it rises out of the dark water, floats
    /// up to its place in the igloo, becomes the brick there as it lands, and the hole freezes
    /// over. The lake's part of it goes into `lake`, the block's into `out`.
    fn coming(&self, lake: &mut Layer, out: &mut Layer, j: usize, g: f32, from: Point, lake_u: f32, time: f64) {
        let (w, d) = (BLOCK.0 * lake_u / 2.0, 3.0 * lake_u);
        let corners = [from + pt(-w, -d), from + pt(w, -d), from + pt(w, d), from + pt(-w, d)];
        if g < 0.3 {
            // The cut, traced round.
            let along = g / 0.3 * 4.0;
            for i in 0..4 {
                let s = (along - i as f32).clamp(0.0, 1.0);
                if s > 0.0 {
                    let (a, b) = (corners[i], corners[(i + 1) % 4]);
                    lake.line(a, a + (b - a) * s, 0.9 * lake_u, GLINT.see_through(0.9));
                    if s < 1.0 {
                        lake.radial(a + (b - a) * s, 9.0 * lake_u, 9.0 * lake_u, GLINT, 0.9);
                    }
                }
            }
            return;
        }
        let open = 1.0 - ease((g - 0.55) / 0.45);
        painted(lake, &corners, &[WATER.see_through(0.9 * open); 4]);
        lake.path(corners.to_vec(), true, 0.6 * lake_u, ICE_LIT.see_through(0.6 * open));
        let lifted = from + pt(0.0, d - 14.0 * lake_u);
        let (at, size, haze) = if g < 0.45 {
            (from + pt(0.0, d) + (lifted - from - pt(0.0, d)) * ease((g - 0.3) / 0.15), lake_u, 0.0)
        } else {
            let s = ease((g - 0.45) / 0.55);
            let to = self.slot(j);
            let over = pt((lifted.x + to.x) / 2.0, lifted.y.min(to.y) - 50.0 * self.u - 0.15 * lifted.distance(to));
            let bob = (s * PI).sin() * (time as f32 * 3.0).sin() * 2.0 * self.u;
            // Light trailing after it.
            for i in 1..6 {
                let back = (s - 0.025 * i as f32).max(0.0);
                out.circle(bend(lifted, over, to, back) - pt(0.0, 5.0 * self.u), 1.3 * self.u, GLINT.see_through(0.7 * (1.0 - i as f32 / 6.0) * (1.0 - s)));
            }
            (bend(lifted, over, to, s) + pt(0.0, bob), lake_u + (self.u * 0.8 - lake_u) * s, self.haze * s)
        };
        block(out, at, size, 1.0 - ((g - 0.9) / 0.1).clamp(0.0, 1.0), 0.15, haze);
        if g > 0.9 {
            out.radial(at - pt(0.0, 6.0 * size), 16.0 * size, 12.0 * size, GLINT, 0.9 * (1.0 - (g - 0.9) / 0.1));
        }
    }
}

/// A place on the hillside a chapter's igloo can stand: its foot, its size, how much the dusk blues
/// it, and how much room it has across.
#[derive(Clone, Copy)]
struct Place {
    foot: Point,
    size: f32,
    haze: f32,
    w: f32,
}

impl Place {
    /// Whether the igloo there is wholly on the desk, beside the page.
    fn in_view(&self, area: Rect, page: Rect) -> bool {
        let r = Rect::from_min_max(self.foot + pt(-self.w / 2.0, -40.0 * self.size), self.foot + pt(self.w / 2.0, 0.0));
        area.contains_rect(r) && !r.overlaps_across(page)
    }
}

/// The places on the hillside beyond the lake, in the order chapters take them: either side of
/// the page in turn, spread about rather than in rows, nearer the page first but at every depth,
/// near and big or further up the hill and smaller, each a little off a grid so they do not line
/// up. They come from the story and reach as far from the page whatever the window, so a place
/// never moves when it changes; those beyond the desk are out of sight.
fn places(seed: u64, page: Rect, ground: f32, u: f32) -> Vec<Place> {
    let mut s = Seeds::of(seed, 99_000);
    let mut out = Vec::new();
    for level in 0..DEPTHS {
        let depth = level as f32 / (DEPTHS - 1) as f32;
        let k = 1.0 - 0.5 * depth;
        let (k, step) = (k * SIZE, 115.0 * u * k * SIZE);
        let y = ground - 82.0 * u - depth * RISE * u;
        let near = 70.0 * u * k;
        let mut j = 0.0;
        while near + step * (j + 1.5) <= REACH * u {
            for x in [-1.0f32, 1.0] {
                let d = near + step * (j + 0.5 * (level % 2) as f32 + s.next(0.0, 0.3));
                let foot = pt(if x < 0.0 { page.left() - d } else { page.right() + d }, y + s.next(-6.0, 6.0) * u);
                let key = d + s.next(0.0, 380.0) * u + level as f32 * 25.0 * u;
                // Not almost straight behind a place nearer down the hill: its igloo would hide
                // behind that one's (a story in ten had one so).
                if out.iter().all(|(_, q): &(f32, Place)| q.foot.distance(foot) > 20.0 * u * k) {
                    out.push((key, Place { foot, size: u * k, haze: 0.5 * depth, w: 100.0 * u * k }));
                }
            }
            j += 1.0;
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Taken from either side of the page in turn, so a few chapters are not all on one side.
    let (mut left, mut right): (Vec<Place>, Vec<Place>) = out.into_iter().map(|(_, p)| p).partition(|p| p.foot.x < page.center().x);
    if s.next(0.0, 1.0) < 0.5 {
        std::mem::swap(&mut left, &mut right);
    }
    let mut taken = Vec::with_capacity(left.len() + right.len());
    let (mut a, mut b) = (left.into_iter(), right.into_iter());
    loop {
        match (a.next(), b.next()) {
            (None, None) => break,
            (x, y) => taken.extend(x.into_iter().chain(y)),
        }
    }
    taken
}

/// A hill at `x`, `w` wide, `high` at most, its foot at `y`: with pines on the far ones (`pines`),
/// snow on their boughs.
#[allow(clippy::too_many_arguments)]
fn hill(l: &mut Layer, s: &mut Seeds, x: f32, w: f32, y: f32, high: f32, u: f32, color: Color, pines: bool) {
    let h = s.next(0.5, 1.0) * high * u;
    if pines {
        for _ in 0..(s.next(2.0, 6.0) as usize) {
            let base = pt(x + s.next(-0.6, 0.6) * w, y - h * s.next(0.2, 0.7));
            let tall = s.next(40.0, 85.0) * u;
            for k in 0..3 {
                let (top, wide) = (base.y - tall * (0.35 + 0.3 * k as f32), tall * (0.42 - 0.1 * k as f32));
                let bottom = top + tall * 0.42;
                l.solid(vec![pt(base.x, top), pt(base.x + wide, bottom), pt(base.x - wide, bottom)], PINE);
                l.solid(vec![pt(base.x, top), pt(base.x + wide * 0.4, top + tall * 0.16), pt(base.x - wide * 0.4, top + tall * 0.16)], HILL);
            }
        }
    }
    l.oval(pt(x, y), pt(1.0, 0.0), w * 0.6, h, color);
}

/// The hillside behind the igloos: stars coming out and the moon risen, hills far off and nearer
/// with pines on them, and the snow reaching up to them. All of it is laid out from `middle`
/// (across) and `ground` (up), as far as `REACH` either side, whatever part of it shows in `area`:
/// only the plain fills below the hills reach to `area`'s edges. So zooming, which shows more or
/// less of it, only makes it bigger or smaller and never moves one hill against another.
fn hillside(l: &mut Layer, seed: u64, area: Rect, middle: f32, ground: f32, u: f32, time: f64) {
    let t = time as f32;
    let horizon = ground - 230.0 * u;
    let (from, to) = (middle - REACH * u, middle + REACH * u);
    for i in 0..160 {
        let mut s = Seeds::of(seed, 96_000 + i);
        let at = pt(from + (to - from) * s.next(0.0, 1.0), horizon - s.next(40.0, 760.0) * u);
        let a = 0.5 + 0.5 * (t * s.next(0.6, 1.8) + s.next(0.0, 6.0)).sin();
        l.circle(at, s.next(0.5, 1.4) * u, STAR.see_through(0.3 + 0.5 * a));
    }
    let mut s = Seeds::of(seed, 97_000);
    let moon = moon(seed, middle, ground, u);
    s.next(0.0, 1.0);
    l.radial(moon, 110.0 * u, 110.0 * u, MOON, 0.22);
    l.circle(moon, 22.0 * u, MOON);
    for (layer, (color, y, high)) in [(HILL_FAR, horizon, 90.0), (HILL, horizon + 60.0 * u, 60.0)].into_iter().enumerate() {
        let pines = color == HILL_FAR;
        let mut x = from;
        while x < to {
            let w = s.next(180.0, 360.0) * u;
            hill(l, &mut s, x, w, y, high, u, color, pines);
            x += w * s.next(0.6, 0.9);
        }
        // Beyond `REACH`, if more shows (the page pushed to one side), more hills, from seeds of their
        // own, so those within it are the same whatever shows.
        let mut more = Seeds::of(seed, 97_100 + layer);
        while x < area.right() + 200.0 * u {
            let w = more.next(180.0, 360.0) * u;
            hill(l, &mut more, x, w, y, high, u, color, pines);
            x += w * more.next(0.6, 0.9);
        }
        let mut more = Seeds::of(seed, 97_200 + layer);
        let mut x = from;
        while x > area.left() - 200.0 * u {
            let w = more.next(180.0, 360.0) * u;
            x -= w * more.next(0.6, 0.9);
            hill(l, &mut more, x, w, y, high, u, color, pines);
        }
        l.polygon(Rect::from_min_max(pt(area.left(), y), pt(area.right(), area.bottom())).corners(), color);
    }
    // The snow, bluer up the hill.
    let mut mesh = Mesh::new();
    let top = horizon + 80.0 * u;
    mesh.vertex(pt(area.left(), top), HILL);
    mesh.vertex(pt(area.right(), top), HILL);
    mesh.vertex(pt(area.right(), ground), SNOW);
    mesh.vertex(pt(area.left(), ground), SNOW);
    mesh.triangle(0, 1, 2);
    mesh.triangle(0, 2, 3);
    l.mesh(mesh);
    l.polygon(Rect::from_min_max(pt(area.left(), ground), area.max).corners(), SNOW);
}

/// The moon, off to one side of `middle`, where the page does not hide it.
fn moon(seed: u64, middle: f32, ground: f32, u: f32) -> Point {
    let mut s = Seeds::of(seed, 97_000);
    pt(middle + if s.next(0.0, 1.0) < 0.5 { -490.0 } else { 490.0 } * u, ground - 620.0 * u)
}

/// Snow falling softly over everything, each flake drifting from side to side. It falls in the
/// window, not on the hillside: it is never zoomed, and how far a flake has fallen is a share of the
/// window's height, so a window made taller only stretches its way down. (Its time multiplied by a
/// size that changed made every flake leap.)
fn snowfall(l: &mut Layer, seed: u64, window: Rect, u: f32, time: f64) {
    let t = time as f32;
    for i in 0..140 {
        let mut s = Seeds::of(seed, 98_000 + i);
        let near = s.next(0.0, 1.0);
        let y = window.top() + window.height() * (s.next(0.0, 1.0) + t * (12.0 + 26.0 * near) / 900.0).rem_euclid(1.0);
        let x = window.left() + window.width() * s.next(0.0, 1.0) + (t * s.next(0.3, 0.8) + i as f32).sin() * 14.0 * u;
        l.circle(pt(x, y), (0.8 + 1.8 * near) * u, SNOW.see_through(0.5 + 0.4 * near));
    }
}

/// What the Snow keeps while a story is shown: the igloos as they are built (`Tended`).
struct Snow {
    ice: Tended,
}

impl Theme for Snow {
    fn new() -> Self {
        Snow { ice: Tended::default() }
    }

    fn event(&mut self, event: &Event) {
        // Picked, or a story shown: everything as it stands, nothing growing.
        if let Event::Fresh = event {
            self.ice.forget();
        }
    }

    fn frame(&mut self, input: &Input, out: &mut Out) {
        let (t, u, area, page) = (input.time, input.unit, input.area, input.page);
        let (seed, garden) = (host::story_seed(), host::garden());
        let n = garden.len().max(1);
        out.ground = GROUND;
        let (middle, ground) = (input.ground.x, input.ground.y);
        // While a block is on its way or an igloo is being finished, every frame; else the snow
        // falls and the lights flicker.
        out.next = if self.ice.tend(seed, &garden, BRICKS, HANG, FINISH, t) { 0 } else { 40 };
        let lake = Lake::of(input.window, ground, u);
        hillside(&mut out.desk, seed, area, middle, ground, u, t);
        lake.draw(&mut out.desk, seed, moon(seed, middle, ground, u).x, t);
        let places = places(seed, page, ground, u);
        let finishing = self.ice.finishing(t, FINISH);
        // Those further up the hill first, so the nearer stand in front.
        let mut order: Vec<(usize, Place)> = (0..n).map(|k| (k, places[self.ice.place(k, places.len(), |p| places[p].in_view(area, page))])).collect();
        order.sort_by(|a, b| a.1.foot.y.total_cmp(&b.1.foot.y).then(a.0.cmp(&b.0)));
        let (mut cut, mut carried) = (Layer::default(), Layer::default());
        for (k, place) in order {
            let face = if place.foot.x < page.center().x { 1.0 } else { -1.0 };
            let bricks = self.ice.grown(&garden, k, BRICKS);
            let igloo = Igloo::of(seed, k, &place, face);
            let finished = match finishing {
                Some((m, q)) if m == k => q,
                _ if k == n - 1 => 0.0,
                _ => 1.0,
            };
            igloo.draw(&mut out.desk, bricks, finished, t);
            if k == n - 1 && bricks.fract() > 0.0 {
                // The block on its way, cut out of the lake below the place.
                let from = lake.cut_at(seed, k, bricks as usize, place.foot.x);
                igloo.coming(&mut cut, &mut carried, bricks as usize, bricks.fract(), from, u, t);
            }
        }
        out.desk.append(cut);
        out.desk.append(carried);
        snowfall(&mut out.window, seed, input.window, u, t);
    }
}

theme!(Snow);
