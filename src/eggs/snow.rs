//! The Magic Snow. A snowy hillside at twilight lies on the desk: a few stars and the moon come out
//! over hills with pines on them, a frozen lake lies along the foot of the desk, under the page and
//! either side of it, the whole of it in the window, and snow falls softly in front of everything.
//! Each chapter has an igloo on the hillside beside the page, some near and big, some further up
//! the hill, smaller and bluer in the dusk. Each page of the chapter being written cuts a block of
//! ice out of the lake: a glint traces round it, it rises out of the water, floats up to the
//! chapter's igloo and becomes its next brick, course by course from the bottom up, and the hole
//! freezes over. When the next chapter begins, the igloo before is finished (the bricks it lacks
//! drop in, its tunnel is built) and a warm light comes on inside it, glowing through the doorway
//! and the joints. Nothing brings a title in this theme.
//!
//! Like everything in `eggs.rs` it lies on the desk behind the pages and is drawn from shapes as a
//! function of the story and of time; nothing is printed or saved, and nothing new is kept: the
//! bricks and the igloos are counted from `Doc::garden`.

use super::*;

pub const SNOW: Color32 = Color32::from_rgb(250, 252, 255);
const SNOW_SHADE: Color32 = Color32::from_rgb(192, 204, 232);
const HILL: Color32 = Color32::from_rgb(214, 222, 244);
const HILL_FAR: Color32 = Color32::from_rgb(160, 172, 214);
const PINE: Color32 = Color32::from_rgb(70, 92, 128);
const STAR: Color32 = Color32::from_rgb(255, 250, 230);
pub const MOON: Color32 = Color32::from_rgb(250, 246, 226);
/// The dusk that pales and blues what lies further up the hill.
const DUSK: Color32 = Color32::from_rgb(120, 130, 182);
/// The lake: its ice far off, mirroring the dusk, and near, and the water under a hole cut in it.
pub const LAKE_FAR: Color32 = Color32::from_rgb(132, 150, 204);
const LAKE_NEAR: Color32 = Color32::from_rgb(196, 214, 242);
const WATER: Color32 = Color32::from_rgb(30, 44, 92);
/// Ice cut and carved: in the light, clear, in shade and deep within.
pub const ICE_LIT: Color32 = Color32::from_rgb(240, 250, 255);
pub const ICE: Color32 = Color32::from_rgb(168, 212, 244);
const ICE_SHADE: Color32 = Color32::from_rgb(92, 142, 206);
const ICE_DEEP: Color32 = Color32::from_rgb(46, 86, 164);
const GLINT: Color32 = Color32::from_rgb(250, 254, 255);

/// A block's width, the height of its front and the depth of its top, in units.
const BLOCK: (f32, f32, f32) = (17.0, 10.0, 4.0);
/// Seconds a block takes to be cut, rise and float to its igloo, and an igloo to be finished and lit.
pub const HANG: f32 = 2.6;
pub const FINISH: f32 = 6.0;
/// How far the places reach from the page whatever the window, and how far up the hill they go,
/// in units; how many depths there are.
const REACH: f32 = 1800.0;
const RISE: f32 = 100.0;
const DEPTHS: usize = 5;
/// How big the igloos are beside the hillside.
const SIZE: f32 = 1.3;

/// A convex shape filled from colour to colour across it, `colors` at its `points`: one mesh.
fn painted(points: &[Pos2], colors: &[Color32]) -> Shape {
    let mut mesh = egui::epaint::Mesh::default();
    for (&p, &c) in points.iter().zip(colors) {
        mesh.colored_vertex(p, c);
    }
    for i in 1..points.len() as u32 - 1 {
        mesh.add_triangle(0, i, i + 1);
    }
    Shape::mesh(mesh)
}

/// The frozen lake as laid out at the fitting zoom: across the window, a little in from its
/// edges, so the whole of it shows, and just above the foot of the desk; `u` its unit.
#[derive(Clone, Copy, Debug)]
pub struct Lake {
    pub left: f32,
    pub right: f32,
    ground: f32,
    u: f32,
}

impl Lake {
    pub fn of(area: Rect, ground: f32, u: f32) -> Lake {
        Lake { left: area.left() + 36.0 * u, right: area.right() - 36.0 * u, ground, u }
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
    pub fn far(&self, x: f32) -> f32 {
        let (u, f, d) = (self.u, self.round(x), (x - self.left) / self.u);
        self.waist() - 34.0 * u * f + (2.0 * (d / 37.0).sin() + 1.5 * (d / 91.0 + 1.0).sin()) * u * f
    }

    /// Its near shore at `x`.
    pub fn near(&self, x: f32) -> f32 {
        self.waist() + 24.0 * self.u * self.round(x)
    }

    /// Where block `j` of chapter `k` is cut out of it, below `x` (kept well within it).
    fn cut_at(&self, story: &str, k: usize, j: usize, x: f32) -> Pos2 {
        let mut s = Seeds::of(story, 99_700 + k * BRICKS + j);
        let u = self.u;
        let x = (x + s.next(-40.0, 40.0) * u).clamp(self.left + 70.0 * u, self.right - 70.0 * u);
        let (far, near) = (self.far(x) + 10.0 * u, self.near(x) - 8.0 * u);
        pos2(x, far + (near - far) * s.next(0.2, 0.8))
    }

    /// The lake: its ice mirroring the dusk far off and paler near, the moon's light lying on it
    /// below `moon`, faint scratches in it, and snow heaped along both its shores.
    fn draw(&self, story: &str, moon: f32, time: f64) -> Vec<Shape> {
        let mut out = Vec::new();
        let u = self.u;
        let n = 96;
        let at = |i: usize| self.left + (self.right - self.left) * i as f32 / n as f32;
        let mut mesh = egui::epaint::Mesh::default();
        for i in 0..=n {
            let x = at(i);
            mesh.colored_vertex(pos2(x, self.far(x)), LAKE_FAR);
            mesh.colored_vertex(pos2(x, self.near(x)), LAKE_NEAR);
        }
        for i in 0..n as u32 {
            let (a, b) = (2 * i, 2 * i + 2);
            mesh.add_triangle(a, a + 1, b);
            mesh.add_triangle(b, a + 1, b + 1);
        }
        out.push(Shape::mesh(mesh));
        // The moon's light on the ice, shimmering a little.
        if moon > self.left + 60.0 * u && moon < self.right - 60.0 * u {
            let shine = 0.3 + 0.04 * (time as f32 * 0.8).sin();
            out.push(radial(pos2(moon, (self.far(moon) + self.near(moon)) / 2.0), 46.0 * u, 16.0 * u, MOON, shine));
        }
        let mut s = Seeds::of(story, 97_500);
        for _ in 0..40 {
            let x = self.left + (self.right - self.left) * s.next(0.1, 0.85);
            let y = self.far(x) + (self.near(x) - self.far(x)) * s.next(0.25, 0.8);
            let (len, tilt) = (s.next(10.0, 40.0) * u, s.next(-0.08, 0.08));
            out.push(Shape::line_segment([pos2(x, y), pos2(x + len, y + len * tilt)], Stroke::new(0.6 * u, glow(ICE_LIT, s.next(0.15, 0.35)))));
        }
        // Snow heaped along its shores, in shade underneath, smaller round its ends.
        for (shore, big) in [(0, 1.0), (1, 0.7)] {
            let mut x = self.left;
            while x < self.right {
                let f = self.round(x).max(0.3);
                let w = s.next(14.0, 30.0) * u * f;
                let y = if shore == 0 { self.far(x) } else { self.near(x) };
                out.push(oval(pos2(x, y + 1.6 * u), vec2(1.0, 0.0), w, s.next(2.5, 4.0) * u * big, SNOW_SHADE));
                out.push(oval(pos2(x, y), vec2(1.0, 0.0), w, s.next(2.5, 4.5) * u * big, SNOW));
                x += w * s.next(1.0, 1.6);
            }
        }
        out
    }
}

/// A block of ice standing on `at` (the middle of its foot), `u` its size, faded to `alpha`, lit
/// up `bright` and blued `haze` by the dusk: its front shaded from the light on the left, its top
/// in the light, a clear edge and a streak of light in it.
fn block(at: Pos2, u: f32, alpha: f32, bright: f32, haze: f32) -> Vec<Shape> {
    let (w, h, d) = (BLOCK.0 * u / 2.0, BLOCK.1 * u, BLOCK.2 * u);
    let c = |col: Color32, a: f32| glow(col.lerp_to_gamma(ICE_LIT, bright).lerp_to_gamma(DUSK, haze), a * alpha);
    let top = at.y - h;
    let front = [pos2(at.x - w, top), pos2(at.x + w, top), pos2(at.x + w, at.y), pos2(at.x - w, at.y)];
    let lid = [pos2(at.x - w, top), pos2(at.x - w + 1.5 * u, top - d), pos2(at.x + w - 1.5 * u, top - d), pos2(at.x + w, top)];
    vec![
        painted(&front, &[c(ICE, 0.9), c(ICE_SHADE, 0.85), c(ICE_DEEP, 0.85), c(ICE_SHADE, 0.85)]),
        painted(&lid, &[c(ICE_LIT, 0.95), c(ICE_LIT, 0.95), c(ICE, 0.9), c(ICE, 0.9)]),
        Shape::line_segment([front[0], front[1]], Stroke::new(0.7 * u, c(ICE_LIT, 0.9))),
        Shape::line_segment([front[0], front[3]], Stroke::new(0.6 * u, c(ICE_LIT, 0.6))),
        Shape::line_segment([pos2(at.x - w + 2.5 * u, top + 2.0 * u), pos2(at.x - w + 6.0 * u, at.y - 2.0 * u)], Stroke::new(0.8 * u, c(Color32::WHITE, 0.35))),
    ]
}

/// How many bricks make an igloo, one for each page of its chapter: rows of these many from the
/// bottom up, the last its cap; how wide and how high its dome is, in units.
pub const BRICKS: usize = 16;
const COURSES: [usize; 5] = [5, 4, 3, 3, 1];
const DOME: (f32, f32) = (40.0, 34.0);
/// The light inside an igloo: at its brightest, and deeper, as through the doorway.
pub const WARM: Color32 = Color32::from_rgb(255, 196, 110);
const EMBER: Color32 = Color32::from_rgb(232, 120, 52);
/// The bricks: in the light, and in shade.
const BRICK: Color32 = Color32::from_rgb(248, 251, 255);
const BRICK_SHADE: Color32 = Color32::from_rgb(170, 188, 226);

/// A chapter's igloo: where it stands, its size, how much the dusk blues it, which side its
/// doorway is on (towards the page), and a number of its own that sets how its light flickers.
#[derive(Clone, Debug)]
pub struct Igloo {
    pub foot: Pos2,
    u: f32,
    haze: f32,
    face: f32,
    phase: f32,
}

impl Igloo {
    /// Chapter `k`'s igloo at `place`, its doorway to `face`.
    pub fn of(story: &str, k: usize, place: &Place, face: f32) -> Igloo {
        Igloo { foot: place.foot, u: place.size, haze: place.haze, face, phase: Seeds::of(story, 99_500 + k).next(0.0, 40.0) }
    }

    fn hazed(&self, c: Color32) -> Color32 {
        c.lerp_to_gamma(DUSK, self.haze)
    }

    /// The point `(x, y)` of the igloo, in its units from its foot, its doorway turned to `face`.
    fn point(&self, (x, y): (f32, f32)) -> Pos2 {
        self.foot + vec2(x * self.face, y) * self.u
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
    pub fn slot(&self, j: usize) -> Pos2 {
        let b = Self::brick(j);
        let x = b.iter().map(|p| p.0).sum::<f32>() / b.len() as f32;
        let y = b.iter().map(|p| p.1).fold(f32::MIN, f32::max);
        self.point((x, y))
    }

    /// Around the igloo built, its tunnel and all.
    pub fn bounds(&self) -> Rect {
        Rect::from_two_pos(self.point((-DOME.0, -DOME.1)), self.point((DOME.0 + 6.0, 0.0)))
    }

    /// Around the first `bricks` bricks (the first, if none).
    pub fn built_bounds(&self, bricks: usize) -> Rect {
        (0..bricks.clamp(1, BRICKS)).flat_map(Self::brick).fold(Rect::NOTHING, |r, p| r | Rect::from_pos(self.point(p)))
    }

    /// One brick, `alpha` of the way there, lowered from `drop` units above, with the light inside
    /// showing in the joint round it as `lit`: the joint is the brick at full size, the brick itself
    /// a little within it, shaded from the light on the left and the top.
    fn draw_brick(&self, j: usize, alpha: f32, drop: f32, lit: f32) -> Vec<Shape> {
        let poly: Vec<(f32, f32)> = Self::brick(j).into_iter().map(|(x, y)| (x, y - drop)).collect();
        let n = poly.len() as f32;
        let c = poly.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
        let joint = self.hazed(BRICK_SHADE.lerp_to_gamma(WARM, lit));
        let mut out = vec![Shape::convex_polygon(poly.iter().map(|&p| self.point(p)).collect(), glow(joint, alpha), Stroke::NONE)];
        let inset = |(x, y): (f32, f32)| {
            let (dx, dy) = (x - c.0, y - c.1);
            let l = (dx * dx + dy * dy).sqrt().max(0.01);
            let k = (1.0 - 1.3 / l).max(0.0);
            (c.0 + dx * k, c.1 + dy * k)
        };
        let shade = |(x, y): (f32, f32)| {
            let l = (0.55 - 0.45 * x * self.face / DOME.0 + 0.35 * (-y / DOME.1 - 0.5)).clamp(0.0, 1.0);
            glow(self.hazed(BRICK_SHADE.lerp_to_gamma(BRICK, l).lerp_to_gamma(WARM, 0.12 * lit)), alpha)
        };
        let inner: Vec<(f32, f32)> = poly.iter().map(|&p| inset(p)).collect();
        let mut mesh = egui::epaint::Mesh::default();
        mesh.colored_vertex(self.point(inset(c)), shade(c));
        for &p in &inner {
            mesh.colored_vertex(self.point(p), shade(p));
        }
        let m = inner.len() as u32;
        for i in 1..=m {
            mesh.add_triangle(0, i, i % m + 1);
        }
        out.push(Shape::mesh(mesh));
        out
    }

    /// The igloo with `bricks` laid (whole ones, and the newest coming in as its block lands),
    /// `finished` from 0 to 1 once its chapter is done: the bricks it lacks drop in one after
    /// another, its tunnel is built, and a light comes on inside, glowing through the doorway and
    /// the joints and on the snow before it, flickering a little.
    pub fn draw(&self, bricks: f32, finished: f32, time: f64) -> Vec<Shape> {
        let (t, u, q) = (time as f32, self.u, finished);
        let mut out = Vec::new();
        let laid = (bricks.floor() as usize).min(BRICKS);
        let lit = ease((q - 0.65) / 0.25) * (0.9 + 0.1 * (t * 5.0 + self.phase).sin() * (t * 3.3).sin());
        if laid > 0 || q > 0.0 {
            out.push(radial(self.foot + vec2(8.0, -1.0) * u, DOME.0 * 1.3 * u, 7.0 * u, self.hazed(SNOW_SHADE), 0.8));
        }
        if lit > 0.0 {
            out.push(radial(self.point((0.0, -DOME.1 * 0.5)), DOME.0 * 2.0 * u, DOME.1 * 0.9 * u, self.hazed(WARM), 0.22 * lit));
        }
        for j in 0..BRICKS {
            if j < laid {
                out.extend(self.draw_brick(j, 1.0, 0.0, lit));
            } else if j == laid && bricks.fract() > 0.9 && q == 0.0 {
                // The newest, as its block lands.
                out.extend(self.draw_brick(j, (bricks.fract() - 0.9) / 0.1, 0.0, 0.0));
            } else if q > 0.0 {
                // Those it lacks, once its chapter is done, dropping in one after another.
                let at = 0.45 * (j - laid) as f32 / (BRICKS - laid).max(1) as f32;
                let p = ease((q - at) / 0.1);
                if p > 0.0 {
                    out.extend(self.draw_brick(j, p, 18.0 * (1.0 - p), lit));
                    if p < 1.0 {
                        let land = self.slot(j);
                        out.push(radial(land - vec2(0.0, 4.0 * u), 12.0 * u, 8.0 * u, GLINT, 0.8 * p));
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
            let colors: Vec<Color32> = outer.iter().enumerate().map(|(m, _)| glow(self.hazed(BRICK_SHADE.lerp_to_gamma(BRICK, 0.35 + 0.5 * (m as f32 / 12.0))), tunnel)).collect();
            let mut mesh = egui::epaint::Mesh::default();
            let base = self.point((cx, 0.0));
            mesh.colored_vertex(base, glow(self.hazed(BRICK_SHADE.lerp_to_gamma(BRICK, 0.6)), tunnel));
            for (&p, &c) in outer.iter().zip(&colors) {
                mesh.colored_vertex(p, c);
            }
            for m in 1..outer.len() as u32 {
                mesh.add_triangle(0, m, m + 1);
            }
            out.push(Shape::mesh(mesh));
            out.push(Shape::line(arch(rx - 3.0, ry - 3.0 * tunnel), Stroke::new(0.8 * u, glow(self.hazed(BRICK_SHADE.lerp_to_gamma(WARM, lit)), tunnel))));
            // The doorway, dark until the light comes on, then warm from within.
            let door = arch(8.0, 12.0 * tunnel);
            let mut mesh = egui::epaint::Mesh::default();
            let dark = self.hazed(WATER);
            mesh.colored_vertex(base, dark.lerp_to_gamma(WARM, lit));
            for &p in &door {
                mesh.colored_vertex(p, dark.lerp_to_gamma(EMBER, 0.85 * lit));
            }
            for m in 1..door.len() as u32 {
                mesh.add_triangle(0, m, m + 1);
            }
            out.push(Shape::mesh(mesh));
            if lit > 0.0 {
                out.push(radial(base + vec2(self.face * 4.0, 2.0) * u, 30.0 * u, 7.0 * u, self.hazed(WARM), 0.45 * lit));
            }
        }
        out
    }

    /// Block `j` on its way, `g` of the way (from 0 to 1), cut out of the lake at `from` where its
    /// size is `lake_u`: a glint traces round it in the ice, it rises out of the dark water, floats
    /// up to its place in the igloo, becomes the brick there as it lands, and the hole freezes
    /// over. The lake's part of it, the block's, and where the block is.
    pub fn coming(&self, j: usize, g: f32, from: Pos2, lake_u: f32, time: f64) -> (Vec<Shape>, Vec<Shape>, Pos2) {
        let (mut lake, mut out) = (Vec::new(), Vec::new());
        let (w, d) = (BLOCK.0 * lake_u / 2.0, 3.0 * lake_u);
        let corners = [from + vec2(-w, -d), from + vec2(w, -d), from + vec2(w, d), from + vec2(-w, d)];
        if g < 0.3 {
            // The cut, traced round.
            let along = g / 0.3 * 4.0;
            for i in 0..4 {
                let s = (along - i as f32).clamp(0.0, 1.0);
                if s > 0.0 {
                    let (a, b) = (corners[i], corners[(i + 1) % 4]);
                    lake.push(Shape::line_segment([a, a + (b - a) * s], Stroke::new(0.9 * lake_u, glow(GLINT, 0.9))));
                    if s < 1.0 {
                        lake.push(radial(a + (b - a) * s, 9.0 * lake_u, 9.0 * lake_u, GLINT, 0.9));
                    }
                }
            }
            return (lake, out, from);
        }
        let open = 1.0 - ease((g - 0.55) / 0.45);
        lake.push(painted(&corners, &[glow(WATER, 0.9 * open); 4]));
        lake.push(Shape::closed_line(corners.to_vec(), Stroke::new(0.6 * lake_u, glow(ICE_LIT, 0.6 * open))));
        let lifted = from + vec2(0.0, d - 14.0 * lake_u);
        let (at, size, haze) = if g < 0.45 {
            (from + vec2(0.0, d) + (lifted - from - vec2(0.0, d)) * ease((g - 0.3) / 0.15), lake_u, 0.0)
        } else {
            let s = ease((g - 0.45) / 0.55);
            let to = self.slot(j);
            let over = pos2((lifted.x + to.x) / 2.0, lifted.y.min(to.y) - 50.0 * self.u - 0.15 * lifted.distance(to));
            let bob = (s * PI).sin() * (time as f32 * 3.0).sin() * 2.0 * self.u;
            // Light trailing after it.
            for i in 1..6 {
                let back = (s - 0.025 * i as f32).max(0.0);
                out.push(Shape::circle_filled(bend(lifted, over, to, back) - vec2(0.0, 5.0 * self.u), 1.3 * self.u, glow(GLINT, 0.7 * (1.0 - i as f32 / 6.0) * (1.0 - s))));
            }
            (bend(lifted, over, to, s) + vec2(0.0, bob), lake_u + (self.u * 0.8 - lake_u) * s, self.haze * s)
        };
        out.extend(block(at, size, 1.0 - ((g - 0.9) / 0.1).clamp(0.0, 1.0), 0.15, haze));
        if g > 0.9 {
            out.push(radial(at - vec2(0.0, 6.0 * size), 16.0 * size, 12.0 * size, GLINT, 0.9 * (1.0 - (g - 0.9) / 0.1)));
        }
        (lake, out, at)
    }
}

/// A place on the hillside a chapter's igloo can stand: its foot, its size, how much the dusk blues
/// it, and how much room it has across.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub foot: Pos2,
    pub size: f32,
    pub haze: f32,
    pub w: f32,
}

impl Place {
    /// Whether the igloo there is wholly on the desk, beside the page.
    pub fn in_view(&self, area: Rect, page: Rect) -> bool {
        let r = Rect::from_min_max(self.foot + vec2(-self.w / 2.0, -40.0 * self.size), self.foot + vec2(self.w / 2.0, 0.0));
        area.contains_rect(r) && !r.x_range().intersects(page.x_range())
    }
}

/// The places on the hillside beyond the lake, in the order chapters take them: either side of
/// the page in turn, spread about rather than in rows, nearer the page first but at every depth, near and big or further up the
/// hill and smaller, each a little off a grid so they do not line up. They come from `story` and
/// reach as far from the page whatever the window, so a place never moves when it changes; those
/// beyond the desk are out of sight.
pub fn places(story: &str, page: Rect, ground: f32, u: f32) -> Vec<Place> {
    let mut s = Seeds::of(story, 99_000);
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
                let foot = pos2(if x < 0.0 { page.left() - d } else { page.right() + d }, y + s.next(-6.0, 6.0) * u);
                let key = d + s.next(0.0, 380.0) * u + level as f32 * 25.0 * u;
                out.push((key, Place { foot, size: u * k, haze: 0.5 * depth, w: 100.0 * u * k }));
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

/// The hillside behind the igloos: stars coming out and the moon risen, hills far off and nearer with
/// pines on them, and the snow reaching up to them. All of it is laid out from `middle` (across) and `ground` (up), as far as `REACH` either
/// side, whatever part of it shows in `area`: only the plain fills below the hills reach to
/// `area`'s edges. So zooming, which shows more or less of it, only makes it bigger or smaller and
/// never moves one hill against another.
fn hillside(story: &str, area: Rect, middle: f32, ground: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let mut out = Vec::new();
    let horizon = ground - 230.0 * u;
    let (from, to) = (middle - REACH * u, middle + REACH * u);
    for i in 0..160 {
        let mut s = Seeds::of(story, 96_000 + i);
        let at = pos2(from + (to - from) * s.next(0.0, 1.0), horizon - s.next(40.0, 760.0) * u);
        let a = 0.5 + 0.5 * (t * s.next(0.6, 1.8) + s.next(0.0, 6.0)).sin();
        out.push(Shape::circle_filled(at, s.next(0.5, 1.4) * u, glow(STAR, 0.3 + 0.5 * a)));
    }
    let mut s = Seeds::of(story, 97_000);
    let moon = moon(story, middle, ground, u);
    s.next(0.0, 1.0);
    out.push(radial(moon, 110.0 * u, 110.0 * u, MOON, 0.22));
    out.push(Shape::circle_filled(moon, 22.0 * u, MOON));
    for (layer, (color, y, high)) in [(HILL_FAR, horizon, 90.0), (HILL, horizon + 60.0 * u, 60.0)].into_iter().enumerate() {
        // A hill at `x`, `w` wide, with pines on the far ones, snow on their boughs.
        let hill = |out: &mut Vec<Shape>, s: &mut Seeds, x: f32, w: f32| {
            let h = s.next(0.5, 1.0) * high * u;
            if color == HILL_FAR {
                for _ in 0..(s.next(2.0, 6.0) as usize) {
                    let base = pos2(x + s.next(-0.6, 0.6) * w, y - h * s.next(0.2, 0.7));
                    let tall = s.next(40.0, 85.0) * u;
                    for k in 0..3 {
                        let (top, wide) = (base.y - tall * (0.35 + 0.3 * k as f32), tall * (0.42 - 0.1 * k as f32));
                        let bottom = top + tall * 0.42;
                        out.push(solid(vec![pos2(base.x, top), pos2(base.x + wide, bottom), pos2(base.x - wide, bottom)], PINE));
                        out.push(solid(vec![pos2(base.x, top), pos2(base.x + wide * 0.4, top + tall * 0.16), pos2(base.x - wide * 0.4, top + tall * 0.16)], HILL));
                    }
                }
            }
            out.push(oval(pos2(x, y), vec2(1.0, 0.0), w * 0.6, h, color));
        };
        let mut x = from;
        while x < to {
            let w = s.next(180.0, 360.0) * u;
            hill(&mut out, &mut s, x, w);
            x += w * s.next(0.6, 0.9);
        }
        // Beyond `REACH`, if more shows (the page pushed to one side), more hills, from seeds of their
        // own, so those within it are the same whatever shows.
        let mut more = Seeds::of(story, 97_100 + layer);
        while x < area.right() + 200.0 * u {
            let w = more.next(180.0, 360.0) * u;
            hill(&mut out, &mut more, x, w);
            x += w * more.next(0.6, 0.9);
        }
        let mut more = Seeds::of(story, 97_200 + layer);
        let mut x = from;
        while x > area.left() - 200.0 * u {
            let w = more.next(180.0, 360.0) * u;
            x -= w * more.next(0.6, 0.9);
            hill(&mut out, &mut more, x, w);
        }
        out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), area.bottom())), 0.0, color));
    }
    // The snow, bluer up the hill.
    let mut mesh = egui::epaint::Mesh::default();
    let top = horizon + 80.0 * u;
    mesh.colored_vertex(pos2(area.left(), top), HILL);
    mesh.colored_vertex(pos2(area.right(), top), HILL);
    mesh.colored_vertex(pos2(area.right(), ground), SNOW);
    mesh.colored_vertex(pos2(area.left(), ground), SNOW);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    out.push(Shape::mesh(mesh));
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), ground), area.max), 0.0, SNOW));
    out
}

/// The moon, off to one side of `middle`, where the page does not hide it.
fn moon(story: &str, middle: f32, ground: f32, u: f32) -> Pos2 {
    let mut s = Seeds::of(story, 97_000);
    pos2(middle + if s.next(0.0, 1.0) < 0.5 { -490.0 } else { 490.0 } * u, ground - 620.0 * u)
}

/// Snow falling softly over everything, each flake drifting from side to side. It falls in the
/// window, not on the hillside: it is never zoomed, and how far a flake has fallen is a share of the
/// window's height, so a window made taller only stretches its way down. (Its time multiplied by a
/// size that changed made every flake leap.)
fn snowfall(story: &str, area: Rect, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    (0..140).map(|i| {
        let mut s = Seeds::of(story, 98_000 + i);
        let near = s.next(0.0, 1.0);
        let y = area.top() + area.height() * (s.next(0.0, 1.0) + t * (12.0 + 26.0 * near) / 900.0).rem_euclid(1.0);
        let x = area.left() + area.width() * s.next(0.0, 1.0) + (t * s.next(0.3, 0.8) + i as f32).sin() * 14.0 * u;
        Shape::circle_filled(pos2(x, y), (0.8 + 1.8 * near) * u, glow(SNOW, 0.5 + 0.4 * near))
    }).collect()
}

impl App {
    /// The hillside on the desk, behind the pages lying at `page`, in the Magic Snow only: for each
    /// chapter an igloo with a brick for each page it has had (`Doc::garden`), each cut out of the
    /// lake, the igloo before the newest finished and lit when its chapter begins.
    pub fn draw_snow(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.snow = None;
        if self.themes.theme != Theme::Snow {
            self.eggs.ice.forget();
            return;
        }
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let garden = &self.doc.garden;
        let n = garden.len().max(1);
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        // How far the writer has zoomed in from the page fitting the view: the hillside comes
        // nearer with the page, as through a camera, rather than the ice alone moving.
        let z = (self.zoom / self.fitting_zoom(area)).clamp(0.5, 4.0);
        let e = &mut self.eggs;
        if e.ice.tend(&self.story_id, garden, BRICKS, HANG, FINISH, now) {
            ctx.request_repaint();
        } else {
            // The snow falls and the lights flicker.
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        let unzoomed = e.unit(&ctx, area, now);
        let u = unzoomed * z;
        let story = &self.story_id;
        let painter = ui.painter();
        let ground = area.bottom() - hidden - 14.0 * unzoomed;
        // The hillside is laid out as at the fitting zoom over as much of the desk as shows once
        // zoomed, then zoomed about the foot of the desk under the page's middle, where the
        // places are zoomed about too (their units and the page's edges).
        let anchor = pos2(page.center().x, ground);
        let seen_area = Rect::from_min_max(anchor + (area.min - anchor) / z, anchor + (area.max - anchor) / z);
        let zoom = egui::emath::TSTransform::new(anchor.to_vec2() * (1.0 - z), z);
        // The lake fits the window at the fitting zoom, so the whole of it shows; zoomed in, it
        // comes nearer with the rest.
        let lake = Lake::of(area, ground, unzoomed);
        let laid = [hillside(story, seen_area, anchor.x, ground, unzoomed, now), lake.draw(story, moon(story, anchor.x, ground, unzoomed).x, now)];
        painter.extend(laid.into_iter().flatten().map(|mut s| {
            s.transform(zoom);
            s
        }));
        let places = places(story, page, ground, u);
        let finishing = e.ice.finishing(now, FINISH);
        let mut seen = Seen { igloos: vec![(Rect::NOTHING, 0, 0.0); n], finishing: finishing.is_some(), ..Seen::default() };
        // Those further up the hill first, so the nearer stand in front.
        let mut order: Vec<(usize, Place)> = (0..n).map(|k| (k, places[e.ice.place(k, places.len(), |p| places[p].in_view(area, page))])).collect();
        order.sort_by(|a, b| a.1.foot.y.total_cmp(&b.1.foot.y).then(a.0.cmp(&b.0)));
        let mut coming = (Vec::new(), Vec::new());
        for (k, place) in order {
            let face = if place.foot.x < page.center().x { 1.0 } else { -1.0 };
            let bricks = e.ice.grown(garden, k, BRICKS);
            let igloo = Igloo::of(story, k, &place, face);
            let finished = match finishing {
                Some((m, q)) if m == k => q,
                _ if k == n - 1 => 0.0,
                _ => 1.0,
            };
            painter.extend(igloo.draw(bricks, finished, now));
            if k == n - 1 && bricks.fract() > 0.0 {
                // The block on its way, cut out of the lake below the place, as laid out unzoomed.
                let below = anchor.x + (place.foot.x - anchor.x) / z;
                let from = zoom * lake.cut_at(story, k, bricks as usize, below);
                let (lake, block, at) = igloo.coming(bricks as usize, bricks.fract(), from, u, now);
                coming.0.extend(lake);
                coming.1.extend(block);
                seen.coming = Some(at);
            }
            let shown = if finished > 0.0 { igloo.bounds() } else { igloo.built_bounds(bricks as usize) };
            seen.igloos[k] = (shown, bricks as usize, finished);
            seen.lit += usize::from(finished >= 1.0);
        }
        painter.extend(coming.0);
        painter.extend(coming.1);
        painter.extend(snowfall(story, area, unzoomed, now));
        self.eggs.snow = Some(seen);
    }
}

/// The hillside drawn last frame, for tests: each chapter's igloo, around what of it stands (the
/// bricks laid, or all of it once its chapter is done), with how many bricks and how far it is
/// finished; where the block on its way is, if one is; whether an igloo is being finished, and
/// how many are lit.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub igloos: Vec<(Rect, usize, f32)>,
    pub coming: Option<Pos2>,
    pub finishing: bool,
    pub lit: usize,
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

    fn igloo(k: usize) -> Igloo {
        Igloo::of("a story", k, &Place { foot: pos2(300.0, 820.0), size: 1.0, haze: 0.0, w: 100.0 }, 1.0)
    }

    fn around(shapes: &[Shape]) -> Rect {
        shapes.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect())
    }

    #[test]
    fn the_hillside_is_there_from_the_start_the_same_for_the_same_story() {
        let shapes = |story: &str| [hillside(story, desk(), 700.0, 880.0, 1.0, 0.0), snowfall(story, desk(), 1.0, 0.0)].concat();
        assert!(shapes("a story").len() > 200, "stars, hills, pines and snow: {}", shapes("a story").len());
        let at = |story: &str| shapes(story).iter().map(|s| s.visual_bounding_rect().center()).collect::<Vec<_>>();
        assert_eq!(at("a story"), at("a story"));
    }

    #[test]
    fn the_hills_go_on_to_the_edges_of_whatever_shows_however_far_from_the_page() {
        for far in [-4000.0, 4000.0] {
            let area = desk().translate(vec2(far, 0.0));
            let ovals = hillside("a story", area, 700.0, 880.0, 1.0, 0.0).into_iter().filter(|s| matches!(s, Shape::Path(_))).map(|s| s.visual_bounding_rect()).filter(|r| r.x_range().intersects(area.x_range())).fold(Rect::NOTHING, |a, r| a | r);
            assert!(ovals.left() <= area.left() && ovals.right() >= area.right(), "{far}: {ovals:?}");
        }
    }

    #[test]
    fn the_hillside_is_laid_out_the_same_whatever_part_of_it_shows() {
        // As when zoomed out (more of it shows) or in (less of it): the hills, pines, stars and moon
        // are where they were, only the plain fills reaching further.
        let laid = |area: Rect| hillside("a story", area, 700.0, 880.0, 1.0, 3.0).into_iter().filter(|s| !matches!(s, Shape::Rect(_) | Shape::Mesh(_))).map(|s| s.visual_bounding_rect()).collect::<Vec<_>>();
        assert_eq!(laid(desk()), laid(desk().expand2(vec2(500.0, 300.0))));
        assert_eq!(laid(desk()), laid(desk().shrink(200.0)));
    }

    #[test]
    fn the_whole_lake_shows_along_the_foot_of_the_window_either_side_of_the_page() {
        let ground = 880.0;
        let lake = Lake::of(desk(), ground, 1.0);
        let ice = lake.draw("a story", 1190.0, 0.0);
        let all = around(&ice);
        let water = ice.iter().find_map(|s| match s {
            Shape::Mesh(m) if m.vertices.iter().any(|v| v.color == LAKE_FAR) => Some(m.calc_bounds()),
            _ => None,
        }).expect("the lake");
        assert!(desk().contains_rect(all), "all of it in the window, shores and all: {all:?}");
        assert!(water.left() < page().left() - 300.0 && water.right() > page().right() + 300.0, "either side of the page: {water:?}");
        assert!(water.bottom() < ground && water.top() > ground - 80.0 && water.height() > 40.0, "along the foot, wide enough to be a lake: {water:?}");
        for x in [lake.left, lake.right] {
            assert!((lake.far(x) - lake.near(x)).abs() < 1.0, "its shores meet at its ends");
        }
        for x in [-200.0, 100.0, 700.0, 1300.0, 1600.0] {
            let at = lake.cut_at("a story", 0, 3, x);
            assert!(at.y > lake.far(at.x) + 5.0 && at.y < lake.near(at.x) - 5.0, "cut out of the ice: {at:?}");
        }
        let places = places("a story", page(), ground, 1.0);
        assert!(places.iter().all(|p| p.foot.y < lake.far(p.foot.x)), "the ice stands beyond the lake");
    }

    #[test]
    fn the_snow_falls_gently_whatever_the_size_of_things() {
        // The size of things changing a little (zooming, a window resized) moves no flake far.
        let at = |u: f32, t: f64| snowfall("a story", desk(), u, t).iter().map(|s| s.visual_bounding_rect().center()).collect::<Vec<_>>();
        for (a, b) in at(1.0, 5000.0).iter().zip(at(1.01, 5000.0)) {
            assert!(a.distance(b) < 3.0, "{a:?} {b:?}");
        }
        for (a, b) in at(1.0, 5000.0).iter().zip(at(1.0, 5000.0 + 1.0 / 60.0)) {
            assert!(a.distance(b) < 2.0 || (a.y - b.y).abs() > 800.0, "a frame later: {a:?} {b:?}");
        }
    }

    #[test]
    fn a_block_is_cut_out_of_the_lake_rises_and_floats_onto_the_igloo_as_its_next_brick() {
        let f = igloo(0);
        let from = pos2(420.0, 870.0);
        let (lake, block, at) = f.coming(3, 0.1, from, 1.0, 0.0);
        assert!(block.is_empty() && at == from && !lake.is_empty(), "first a glint traces round it in the ice");
        let (lake, _, at) = f.coming(3, 0.45, from, 1.0, 0.0);
        assert!(at.y < from.y - 5.0 && (at.x - from.x).abs() < 1.0, "risen out of the water: {at:?}");
        assert!(lake.iter().any(|s| matches!(s, Shape::Mesh(m) if m.vertices[0].color.b() > 40 && m.vertices[0].color.r() < 40)), "the hole dark");
        let (_, _, half) = f.coming(3, 0.7, from, 1.0, 0.0);
        assert!(half.y < f.slot(3).y.min(from.y) - 10.0, "floating over: {half:?}");
        let (lake, block, at) = f.coming(3, 1.0, from, 1.0, 0.0);
        assert!(at.distance(f.slot(3)) < 0.5, "at its brick: {at:?} {:?}", f.slot(3));
        assert!(block.iter().all(|s| !matches!(s, Shape::Mesh(m) if m.vertices.iter().any(|v| v.color.a() > 0))), "become the brick");
        assert!(lake.iter().all(|s| !matches!(s, Shape::Mesh(m) if m.vertices.iter().any(|v| v.color.a() > 0))), "frozen over");
        assert!(f.draw(3.95, 0.0, 0.0).len() > f.draw(3.0, 0.0, 0.0).len(), "the brick there as it lands");
        for k in 0..200 {
            let (a, b) = (f.coming(3, 0.3 + 0.7 * k as f32 / 200.0, from, 1.0, 0.0).2, f.coming(3, 0.3 + 0.7 * (k + 1) as f32 / 200.0, from, 1.0, 0.0).2);
            assert!(a.distance(b) < 6.0, "smoothly at {k}: {a:?} {b:?}");
        }
    }

    #[test]
    fn an_igloo_gets_a_brick_a_page_course_by_course_from_the_bottom_up() {
        let f = igloo(0);
        let dome = f.bounds();
        for j in 0..BRICKS {
            assert!(dome.expand(0.5).contains(f.slot(j)), "{j}: in the dome: {:?}", f.slot(j));
            if j > 0 {
                assert!(f.slot(j).y <= f.slot(j - 1).y + 0.01, "{j}: never below the one before");
            }
            assert!(f.draw(j as f32 + 1.0, 0.0, 0.0).len() > f.draw(j as f32, 0.0, 0.0).len(), "{j}: one more");
        }
        assert!(f.slot(BRICKS - 1).y < f.slot(0).y - 25.0, "the cap at the top");
        // The bricks fill the dome without overlapping: their middles apart, the dome's area theirs.
        let area = |p: &[(f32, f32)]| (0..p.len()).map(|i| p[i].0 * p[(i + 1) % p.len()].1 - p[(i + 1) % p.len()].0 * p[i].1).sum::<f32>().abs() / 2.0;
        let all: f32 = (0..BRICKS).map(|j| area(&Igloo::brick(j))).sum();
        let half_oval = PI * DOME.0 * DOME.1 / 2.0;
        assert!((all / half_oval - 1.0).abs() < 0.08, "the dome, brick by brick: {all} {half_oval}");
        assert!(f.built_bounds(BRICKS).height() > f.built_bounds(5).height() * 3.0, "courses on courses");
    }

    #[test]
    fn an_igloo_is_finished_when_its_chapter_is_and_a_warm_light_comes_on_inside() {
        let f = igloo(1);
        let warm = |shapes: &[Shape]| shapes.iter().filter(|s| match s {
            Shape::Mesh(m) => m.vertices.iter().any(|v| v.color.r() as i32 > v.color.b() as i32 + 60 && v.color.a() > 60),
            Shape::Path(p) => p.fill.r() as i32 > p.fill.b() as i32 + 60 && p.fill.a() > 60,
            _ => false,
        }).count();
        let bricks = |shapes: &[Shape]| shapes.iter().filter(|s| matches!(s, Shape::Path(p) if p.closed && p.fill.a() == 255)).count();
        let building = f.draw(5.0, 0.0, 0.0);
        assert!(warm(&building) == 0 && bricks(&building) == 5, "five bricks laid, dark");
        let half = f.draw(5.0, 0.3, 0.0);
        assert!(bricks(&half) > 5 && bricks(&half) < BRICKS, "the bricks it lacks dropping in: {}", bricks(&half));
        let done = f.draw(5.0, 1.0, 0.0);
        assert_eq!(bricks(&done), BRICKS, "all of them");
        assert!(warm(&done) >= 3, "lit inside, through the doorway and on the snow: {}", warm(&done));
        assert!(f.draw(5.0, 1.0, 0.0) != f.draw(5.0, 1.0, 0.7), "the light flickers");
        for q in 0..100 {
            assert!(around(&f.draw(5.0, q as f32 / 100.0, 0.0)).bottom() < f.foot.y + 16.0, "on the snow at {q}");
        }
        assert!(f.bounds().width() > f.built_bounds(BRICKS).width(), "a tunnel before the doorway");
    }

    #[test]
    fn the_places_are_spread_about_beside_the_page_near_and_far_up_the_hill() {
        let all = places("a story", page(), 880.0, 1.0);
        let mut taken = Tended::default();
        let got: Vec<Place> = (0..24).map(|k| all[taken.place(k, all.len(), |p| all[p].in_view(desk(), page()))]).collect();
        assert!(got.iter().any(|p| p.foot.x < page().left()) && got.iter().any(|p| p.foot.x > page().right()), "on both sides");
        let left = |p: &Place| p.foot.x < page().center().x;
        assert!(got[..6].windows(2).all(|w| left(&w[0]) != left(&w[1])), "either side in turn: {got:?}");
        let depths = |n: usize| { let mut d: Vec<i32> = got[..n].iter().map(|p| (p.size * 10.0).round() as i32).collect(); d.sort(); d.dedup(); d.len() };
        assert!(depths(8) >= 3, "the first few at several depths, not in a row: {got:?}");
        for (k, p) in got.iter().enumerate() {
            assert!(p.in_view(desk(), page()), "{k}: on the desk beside the page: {p:?}");
            assert!(got[..k].iter().all(|q| q.foot.distance(p.foot) > 20.0 * p.size), "{k}: a place of its own: {p:?}");
        }
        let near = |p: &Place| (p.foot.x - page().left()).abs().min((p.foot.x - page().right()).abs());
        assert!(near(&got[0]) < near(&got[23]), "nearer the page first");
        assert_ne!(places("a story", page(), 880.0, 1.0)[0].foot, places("another story", page(), 880.0, 1.0)[0].foot, "each story its own");
    }
}
