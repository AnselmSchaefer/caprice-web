//! The Magic Snow. A snowy hillside at twilight lies on the desk: a few stars and the moon come out
//! over hills with pines on them, a frozen lake lies along the foot of the desk, under the page and
//! either side of it, and snow falls softly in front of everything. Each chapter has a place on
//! the hillside beside the page, some near and big, some further up the hill, smaller and bluer in
//! the dusk. Each page of the chapter being written cuts a block of ice out of the lake: a glint
//! traces round it, it rises out of the water, floats up to the chapter's place and settles on its
//! pile, and the hole freezes over. When the next chapter begins, the pile shimmers and turns into
//! an ice figure, rising from its foot: a swan, a stag, a bear, an owl, a fox or a leaping fish,
//! bigger the more pages its chapter had, a gleam passing over it as it is done. Nothing brings a
//! title in this theme.
//!
//! Like everything in `eggs.rs` it lies on the desk behind the pages and is drawn from shapes as a
//! function of the story and of time; nothing is printed or saved, and nothing new is kept: the
//! blocks and the figures are counted from `Doc::garden`.

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
const LAKE_FAR: Color32 = Color32::from_rgb(132, 150, 204);
const LAKE_NEAR: Color32 = Color32::from_rgb(196, 214, 242);
const WATER: Color32 = Color32::from_rgb(30, 44, 92);
/// Ice cut and carved: in the light, clear, in shade and deep within.
pub const ICE_LIT: Color32 = Color32::from_rgb(240, 250, 255);
pub const ICE: Color32 = Color32::from_rgb(168, 212, 244);
const ICE_SHADE: Color32 = Color32::from_rgb(92, 142, 206);
const ICE_DEEP: Color32 = Color32::from_rgb(46, 86, 164);
const GLINT: Color32 = Color32::from_rgb(250, 254, 255);

/// How many blocks make a whole pile, one for each page of its chapter; a pile is rows of these
/// many from the bottom up.
pub const BLOCKS: usize = 12;
const ROWS: [usize; 4] = [4, 3, 3, 2];
/// A block's width, the height of its front and the depth of its top, in units.
const BLOCK: (f32, f32, f32) = (17.0, 10.0, 4.0);
/// Seconds a block takes to be cut, rise and float to its pile, and a pile to become its figure.
pub const HANG: f32 = 2.6;
pub const FORM: f32 = 7.0;
/// How far the places reach from the page whatever the window, and how far up the hill they go,
/// in units; how many depths there are.
const REACH: f32 = 1800.0;
const RISE: f32 = 110.0;
const DEPTHS: usize = 5;
/// How far the lake reaches either side of the middle of the desk, in units.
const LAKE: f32 = 1150.0;
/// How big the ice is beside the hillside: a figure stands as tall as a tree did.
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

/// The far shore of the lake at `x`, laid out from `middle` and `ground`: highest under the
/// page, curving down to the foot of the desk at its ends, a little uneven.
fn shore(middle: f32, ground: f32, u: f32, x: f32) -> f32 {
    let d = (x - middle) / u;
    let f = (1.0 - (d / LAKE).powi(2)).max(0.0).powf(0.3);
    ground + 30.0 * u - 82.0 * u * f + (2.0 * (d / 37.0).sin() + 1.5 * (d / 91.0 + 1.0).sin()) * u * f
}

/// Where block `j` of chapter `k` is cut out of the lake, below `x` (kept within the lake).
fn cut_at(story: &str, k: usize, j: usize, middle: f32, ground: f32, u: f32, x: f32) -> Pos2 {
    let mut s = Seeds::of(story, 99_700 + k * BLOCKS + j);
    let reach = (LAKE - 90.0) * u;
    let x = (x + s.next(-40.0, 40.0) * u).clamp(middle - reach, middle + reach);
    pos2(x, shore(middle, ground, u, x) + s.next(14.0, 30.0) * u)
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

/// An oval about `(cx, cy)`, `rx` and `ry` across, turned `tilt`, in a figure's units.
fn ell(cx: f32, cy: f32, rx: f32, ry: f32, tilt: f32) -> Vec<(f32, f32)> {
    let (c, s) = (tilt.cos(), tilt.sin());
    (0..18).map(|k| {
        let a = k as f32 / 18.0 * 2.0 * PI;
        let (x, y) = (rx * a.cos(), ry * a.sin());
        (cx + x * c - y * s, cy + x * s + y * c)
    }).collect()
}

/// A limb from `a` to `b`, `wa` thick at one end and `wb` at the other.
fn limb(a: (f32, f32), b: (f32, f32), wa: f32, wb: f32) -> Vec<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).sqrt().max(0.01);
    let (nx, ny) = (-dy / l, dx / l);
    vec![(a.0 + nx * wa / 2.0, a.1 + ny * wa / 2.0), (b.0 + nx * wb / 2.0, b.1 + ny * wb / 2.0), (b.0 - nx * wb / 2.0, b.1 - ny * wb / 2.0), (a.0 - nx * wa / 2.0, a.1 - ny * wa / 2.0)]
}

/// The figures a pile can become, in their units from the foot of their plinth, facing right: each
/// a list of convex parts, the further back first. A swan, a stag, a bear, an owl, a fox sitting
/// with its tail round its feet, and a fish leaping out of a wave.
pub const FIGURES: usize = 6;

fn figure_parts(which: usize) -> Vec<Vec<(f32, f32)>> {
    let mut parts = vec![vec![(-24.0, 0.0), (24.0, 0.0), (24.0, -10.0), (-24.0, -10.0)], vec![(-24.0, -10.0), (24.0, -10.0), (22.0, -14.0), (-22.0, -14.0)]];
    let tri = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| vec![a, b, c];
    match which % FIGURES {
        0 => {
            // A swan, its wing raised and its neck curved.
            parts.push(tri((-22.0, -28.0), (-38.0, -42.0), (-20.0, -20.0)));
            parts.push(ell(-2.0, -28.0, 27.0, 12.0, 0.06));
            parts.push(ell(-9.0, -44.0, 22.0, 8.0, 0.6));
            let spine = [(16.0, -32.0), (22.0, -46.0), (19.0, -60.0), (21.0, -72.0)];
            let widths = [9.0, 6.5, 5.5, 5.0];
            for i in 0..3 {
                parts.push(limb(spine[i], spine[i + 1], widths[i], widths[i + 1]));
                parts.push(ell(spine[i + 1].0, spine[i + 1].1, widths[i + 1] / 2.0, widths[i + 1] / 2.0, 0.0));
            }
            parts.push(ell(25.0, -75.0, 7.0, 4.5, 0.15));
            parts.push(tri((30.0, -77.5), (40.0, -73.0), (30.0, -72.0)));
        }
        1 => {
            // A stag, its antlers branching.
            for (x, lean) in [(-14.0, -1.0), (-8.0, 1.5), (10.0, -1.0), (16.0, 1.5)] {
                parts.push(limb((x + lean, -14.0), (x, -40.0), 3.4, 4.6));
            }
            parts.push(ell(-22.0, -48.0, 3.0, 4.0, 0.3));
            parts.push(ell(0.0, -45.0, 23.0, 11.0, 0.0));
            parts.push(limb((14.0, -46.0), (22.0, -66.0), 12.0, 7.0));
            parts.push(ell(19.0, -73.0, 4.0, 2.0, -0.6));
            for (a, b) in [((24.0, -72.0), (27.0, -92.0)), ((27.0, -92.0), (31.0, -104.0)), ((26.0, -86.0), (35.0, -93.0))] {
                parts.push(limb(a, b, 2.4, 1.8));
            }
            parts.push(ell(26.0, -68.0, 8.5, 5.0, 0.35));
            for (a, b) in [((21.0, -72.0), (16.0, -90.0)), ((16.0, -90.0), (10.0, -104.0)), ((17.0, -86.0), (24.0, -97.0)), ((13.0, -97.0), (4.0, -104.0))] {
                parts.push(limb(a, b, 2.6, 2.0));
            }
        }
        2 => {
            // A bear, walking.
            for (x, lean) in [(-16.0, 1.0), (-6.0, -1.0), (9.0, 1.0), (18.0, -1.0)] {
                parts.push(limb((x + lean, -14.0), (x, -34.0), 8.0, 9.0));
            }
            parts.push(ell(0.0, -38.0, 26.0, 15.0, 0.0));
            parts.push(ell(-4.0, -47.0, 15.0, 8.0, -0.05));
            parts.push(ell(21.0, -50.0, 3.4, 3.4, 0.0));
            parts.push(ell(28.0, -51.0, 3.2, 3.2, 0.0));
            parts.push(ell(26.0, -42.0, 10.0, 8.0, 0.1));
            parts.push(ell(35.0, -39.0, 5.5, 3.8, 0.1));
        }
        3 => {
            // An owl, its ear tufts up and its eyes wide.
            parts.push(ell(-5.0, -15.0, 4.0, 2.0, 0.0));
            parts.push(ell(5.0, -15.0, 4.0, 2.0, 0.0));
            parts.push(ell(0.0, -38.0, 15.0, 23.0, 0.0));
            parts.push(ell(-13.0, -36.0, 6.0, 17.0, 0.12));
            parts.push(ell(13.0, -36.0, 6.0, 17.0, -0.12));
            parts.push(tri((-12.0, -70.0), (-15.0, -84.0), (-4.0, -73.0)));
            parts.push(tri((12.0, -70.0), (15.0, -84.0), (4.0, -73.0)));
            parts.push(ell(0.0, -65.0, 14.0, 11.0, 0.0));
            parts.push(ell(-5.5, -66.0, 4.5, 4.5, 0.0));
            parts.push(ell(5.5, -66.0, 4.5, 4.5, 0.0));
            parts.push(tri((-2.0, -62.0), (2.0, -62.0), (0.0, -56.0)));
        }
        4 => {
            // A fox sitting, its tail round its feet.
            parts.push(ell(-2.0, -33.0, 12.0, 19.0, -0.15));
            parts.push(tri((1.0, -59.0), (2.0, -72.0), (7.0, -62.0)));
            parts.push(ell(5.0, -38.0, 7.0, 12.0, -0.2));
            parts.push(limb((7.0, -14.0), (7.0, -36.0), 4.0, 4.5));
            parts.push(ell(7.0, -56.0, 9.0, 7.0, 0.05));
            parts.push(tri((8.0, -61.0), (13.0, -72.0), (14.0, -59.0)));
            parts.push(tri((13.0, -59.0), (24.0, -55.0), (13.0, -51.0)));
            parts.push(ell(10.0, -19.0, 20.0, 6.0, -0.2));
        }
        _ => {
            // A fish leaping out of a wave.
            parts.push(ell(0.0, -18.0, 24.0, 5.0, 0.0));
            parts.push(tri((-14.0, -20.0), (-22.0, -30.0), (-8.0, -22.0)));
            parts.push(tri((14.0, -20.0), (24.0, -29.0), (8.0, -22.0)));
            parts.push(tri((-18.0, -38.0), (-32.0, -34.0), (-25.0, -24.0)));
            parts.push(tri((-1.0, -57.0), (5.0, -68.0), (11.0, -60.0)));
            parts.push(ell(2.0, -48.0, 26.0, 10.0, -0.55));
            parts.push(ell(19.0, -60.0, 7.0, 6.5, -0.55));
            parts.push(tri((6.0, -43.0), (12.0, -36.0), (1.0, -39.0)));
        }
    }
    parts
}

/// A chapter's ice: where it stands, its size, how much the dusk blues it, which way its figure
/// faces (towards the page), which figure it becomes and the parts of it, how far they reach, the
/// size it is carved at (bigger for more pages), its pile's blocks a little out of line, and
/// where it sparkles.
#[derive(Clone, Debug)]
pub struct Figure {
    pub foot: Pos2,
    u: f32,
    haze: f32,
    face: f32,
    pub which: usize,
    parts: Vec<Vec<(f32, f32)>>,
    extent: Rect,
    scale: f32,
    jitter: Vec<f32>,
    sparkles: Vec<((f32, f32), f32)>,
}

impl Figure {
    /// Chapter `k`'s ice at `place`, facing `face`, of `pages` pages.
    pub fn of(story: &str, k: usize, place: &Place, face: f32, pages: usize) -> Figure {
        let mut s = Seeds::of(story, 99_500 + k);
        // Each chapter its own figure, never the one before it again.
        let which = (k + Seeds::of(story, 99_400).next(0.0, FIGURES as f32) as usize) % FIGURES;
        let parts = figure_parts(which);
        let extent = parts.iter().flatten().fold(Rect::NOTHING, |r, &(x, y)| r.union(Rect::from_pos(pos2(x, y))));
        let jitter = (0..BLOCKS).map(|_| s.next(-1.5, 1.5)).collect();
        let sparkles = (0..6).map(|_| {
            let p = &parts[2 + s.next(0.0, (parts.len() - 2) as f32) as usize % (parts.len() - 2)];
            (p[s.next(0.0, p.len() as f32) as usize % p.len()], s.next(0.0, 40.0))
        }).collect();
        let scale = place.size * (0.6 + 0.4 * pages.min(BLOCKS) as f32 / BLOCKS as f32);
        Figure { foot: place.foot, u: place.size, haze: place.haze, face, which, parts, extent, scale, jitter, sparkles }
    }

    fn hazed(&self, c: Color32) -> Color32 {
        c.lerp_to_gamma(DUSK, self.haze)
    }

    /// The point `(x, y)` of the figure, in its units from its foot, turned the way it faces.
    fn point(&self, (x, y): (f32, f32)) -> Pos2 {
        self.foot + vec2(x * self.face, y) * self.scale
    }

    /// Around the figure carved.
    pub fn bounds(&self) -> Rect {
        Rect::from_two_pos(self.point((self.extent.left(), self.extent.top())), self.point((self.extent.right(), self.extent.bottom())))
    }

    /// Where block `j` of the pile stands: the middle of its foot.
    pub fn slot(&self, j: usize) -> Pos2 {
        let (mut row, mut i) = (0, j);
        while row + 1 < ROWS.len() && i >= ROWS[row] {
            i -= ROWS[row];
            row += 1;
        }
        let across = i as f32 - (ROWS[row] as f32 - 1.0) / 2.0;
        self.foot + vec2((across * (BLOCK.0 + 1.0) + self.jitter[j % BLOCKS]) * self.face, -(row as f32) * (BLOCK.1 + 1.0)) * self.u
    }

    /// Around the pile of `blocks` blocks (the first, if none).
    pub fn pile_bounds(&self, blocks: usize) -> Rect {
        let (w, h) = (BLOCK.0 * self.u / 2.0, (BLOCK.1 + BLOCK.2) * self.u);
        (0..blocks.clamp(1, BLOCKS)).map(|j| self.slot(j)).fold(Rect::NOTHING, |r, p| r | Rect::from_min_max(p - vec2(w, h), p + vec2(w, 0.0)))
    }

    /// One part of the figure, shown as far up as `level` (in its units, from the foot): a mesh
    /// clear at its edge and deeper blue within, lit from the left, brighter where the `gleam`
    /// passes (across, in its units), with a fine bright edge.
    fn part(&self, poly: &[(f32, f32)], level: f32, gleam: Option<f32>, alpha: f32) -> Vec<Shape> {
        let poly = if level > self.extent.top() { cut(poly, level, true) } else { poly.to_vec() };
        if poly.len() < 3 {
            return Vec::new();
        }
        let e = self.extent;
        let lit = |(x, y): (f32, f32)| {
            let (dx, dy) = ((x * self.face - e.center().x * self.face) / e.width(), (y - e.center().y) / e.height());
            let shine = gleam.map_or(0.0, |g| (1.0 - (x - g).abs() / 9.0).max(0.0));
            ((0.5 - 0.7 * dx - 0.5 * dy).clamp(0.0, 1.0), shine)
        };
        let mid = poly.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
        let mid = (mid.0 / poly.len() as f32, mid.1 / poly.len() as f32);
        let mut mesh = egui::epaint::Mesh::default();
        let (l, g) = lit(mid);
        mesh.colored_vertex(self.point(mid), glow(self.hazed(ICE_DEEP.lerp_to_gamma(ICE, 0.25 + 0.45 * l).lerp_to_gamma(ICE_LIT, g)), 0.82 * alpha));
        for &p in &poly {
            let (l, g) = lit(p);
            mesh.colored_vertex(self.point(p), glow(self.hazed(ICE_SHADE.lerp_to_gamma(ICE_LIT, l).lerp_to_gamma(Color32::WHITE, g)), 0.92 * alpha));
        }
        let n = poly.len() as u32;
        for i in 1..=n {
            mesh.add_triangle(0, i, i % n + 1);
        }
        let points = poly.iter().map(|&p| self.point(p)).collect();
        vec![Shape::mesh(mesh), Shape::closed_line(points, Stroke::new(0.6 * self.scale, glow(self.hazed(ICE_LIT), 0.7 * alpha)))]
    }

    /// The chapter's ice with `blocks` on its pile (whole ones; the one on its way is `coming`),
    /// `formed` from 0 (a pile) to 1 (its figure). As it forms the pile shimmers and fades, light
    /// swirling up about it, and the figure rises out of it from its foot, a bright edge where it
    /// is reaching; at the last a gleam passes over it. Once carved, it sparkles now and then.
    pub fn draw(&self, blocks: f32, formed: f32, time: f64) -> Vec<Shape> {
        let (t, u, q) = (time as f32, self.u, formed);
        let mut out = Vec::new();
        let shimmer = if q > 0.0 && q < 1.0 { (PI * ((q - 0.05) / 0.7).clamp(0.0, 1.0)).sin() } else { 0.0 };
        let pile = self.pile_bounds(blocks as usize);
        if q > 0.0 {
            // Its shadow on the snow, away from the light.
            let b = self.bounds();
            out.push(radial(self.foot + vec2(10.0, -2.0) * u, b.width() * 0.75, 7.0 * u, self.hazed(SNOW_SHADE), 0.8 * ease(q / 0.4)));
            out.push(radial(b.center(), b.width() * 0.9, b.height() * 0.7, self.hazed(ICE_LIT), 0.1 + 0.5 * shimmer));
        }
        if q < 0.5 {
            let fade = 1.0 - ease((q - 0.2) / 0.3);
            for j in 0..(blocks as usize).min(BLOCKS) {
                let flicker = 0.5 + 0.5 * (t * 7.0 + j as f32 * 1.9).sin();
                out.extend(block(self.slot(j), u, fade, shimmer * (0.4 + 0.5 * flicker), self.haze));
            }
        }
        if q > 0.0 && q < 0.85 {
            // Light swirling up about the pile.
            for i in 0..14 {
                let f = i as f32 / 14.0;
                let a = t * 2.5 + f * 2.0 * PI;
                let up = (q * 1.4 + f * 0.6).fract();
                let at = pile.center_bottom() + vec2(a.cos() * pile.width() * 0.6 * (1.0 - 0.4 * up), -up * self.bounds().height() * 1.1 - 6.0 * u);
                out.push(Shape::circle_filled(at, 1.4 * u, glow(GLINT, shimmer * (1.0 - up) * (0.6 + 0.4 * a.sin()))));
            }
        }
        let reveal = ease((q - 0.25) / 0.5);
        if reveal > 0.0 {
            let level = if reveal >= 1.0 { f32::MIN } else { self.extent.bottom() + (self.extent.top() - self.extent.bottom()) * reveal };
            let gleam = (q > 0.75 && q < 1.0).then(|| self.extent.left() - 10.0 + (self.extent.width() + 20.0) * ease((q - 0.75) / 0.22));
            for p in &self.parts {
                out.extend(self.part(p, level, gleam, 1.0));
            }
            if reveal > 0.0 && reveal < 1.0 {
                out.push(radial(self.point((self.extent.center().x, level)), self.extent.width() * 0.55 * self.scale, 3.0 * self.scale, ICE_LIT, 0.9));
            }
        }
        if q >= 1.0 {
            for &((x, y), phase) in &self.sparkles {
                let a = (t * 0.7 + phase).sin().max(0.0).powi(16);
                if a > 0.02 {
                    let (c, r) = (self.point((x, y)), 4.5 * self.scale * a);
                    out.push(Shape::line_segment([c - vec2(r, 0.0), c + vec2(r, 0.0)], Stroke::new(0.6 * self.scale, glow(GLINT, a))));
                    out.push(Shape::line_segment([c - vec2(0.0, r), c + vec2(0.0, r)], Stroke::new(0.6 * self.scale, glow(GLINT, a))));
                    out.push(Shape::circle_filled(c, 1.2 * self.scale, glow(GLINT, a)));
                }
            }
        }
        out
    }

    /// Block `j` on its way, `g` of the way (from 0 to 1), cut out of the lake at `from` where its
    /// size is `lake_u`: a glint traces round it in the ice, it rises out of the dark water, floats
    /// up to its place on the pile, glints as it settles, and the hole freezes over. The lake's
    /// part of it, the block's, and where the block is.
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
            (bend(lifted, over, to, s) + vec2(0.0, bob), lake_u + (self.u - lake_u) * s, self.haze * s)
        };
        out.extend(block(at, size, 1.0, 0.15, haze));
        if g > 0.9 {
            out.push(radial(at - vec2(0.0, 6.0 * size), 16.0 * size, 12.0 * size, GLINT, 0.9 * (1.0 - (g - 0.9) / 0.1)));
        }
        (lake, out, at)
    }
}

/// A place on the hillside a chapter's ice can stand: its foot, its size, how much the dusk blues
/// it, and how much room it has across.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub foot: Pos2,
    pub size: f32,
    pub haze: f32,
    pub w: f32,
}

impl Place {
    /// Whether the figure there is wholly on the desk, beside the page.
    pub fn in_view(&self, area: Rect, page: Rect) -> bool {
        let r = Rect::from_min_max(self.foot + vec2(-self.w / 2.0, -112.0 * self.size), self.foot + vec2(self.w / 2.0, 0.0));
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
        let y = ground - 66.0 * u - depth * RISE * u;
        let near = 70.0 * u * k;
        let mut j = 0.0;
        while near + step * (j + 1.5) <= REACH * u {
            for x in [-1.0f32, 1.0] {
                let d = near + step * (j + 0.5 * (level % 2) as f32 + s.next(0.0, 0.3));
                let foot = pos2(if x < 0.0 { page.left() - d } else { page.right() + d }, y + s.next(-6.0, 6.0) * u);
                let key = d + s.next(0.0, 380.0) * u + level as f32 * 25.0 * u;
                out.push((key, Place { foot, size: u * k, haze: 0.5 * depth, w: 84.0 * u * k }));
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

/// The hillside behind the ice: stars coming out and the moon risen, hills far off and nearer with
/// pines on them, the snow reaching up to them, and the frozen lake at its foot with the moon in
/// it. All of it is laid out from `middle` (across) and `ground` (up), as far as `REACH` either
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
    // The moon off to one side, where the page does not hide it.
    let moon = pos2(middle + if s.next(0.0, 1.0) < 0.5 { -490.0 } else { 490.0 } * u, ground - 620.0 * u);
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
    out.extend(lake(story, middle, ground, u, moon.x, time));
    out
}

/// The frozen lake along the foot of the desk: its ice mirroring the dusk far off and paler near,
/// the moon's light lying on it, faint scratches in it, and snow heaped along its far shore.
fn lake(story: &str, middle: f32, ground: f32, u: f32, moon: f32, time: f64) -> Vec<Shape> {
    let mut out = Vec::new();
    let bottom = ground + 30.0 * u;
    let n = 96;
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..=n {
        let x = middle + LAKE * u * (2.0 * i as f32 / n as f32 - 1.0);
        mesh.colored_vertex(pos2(x, shore(middle, ground, u, x)), LAKE_FAR);
        mesh.colored_vertex(pos2(x, bottom), LAKE_NEAR);
    }
    for i in 0..n as u32 {
        let (a, b) = (2 * i, 2 * i + 2);
        mesh.add_triangle(a, a + 1, b);
        mesh.add_triangle(b, a + 1, b + 1);
    }
    out.push(Shape::mesh(mesh));
    // The moon's light on the ice, shimmering a little.
    let shine = 0.3 + 0.04 * (time as f32 * 0.8).sin();
    out.push(radial(pos2(moon, shore(middle, ground, u, moon) + 26.0 * u), 46.0 * u, 22.0 * u, MOON, shine));
    let mut s = Seeds::of(story, 97_500);
    for _ in 0..40 {
        let x = middle + s.next(-1.0, 1.0) * (LAKE - 80.0) * u;
        let y = shore(middle, ground, u, x) + s.next(10.0, 40.0) * u;
        let (len, tilt) = (s.next(10.0, 40.0) * u, s.next(-0.08, 0.08));
        out.push(Shape::line_segment([pos2(x, y), pos2(x + len, y + len * tilt)], Stroke::new(0.6 * u, glow(ICE_LIT, s.next(0.15, 0.35)))));
    }
    // Snow heaped along the far shore, in shade underneath.
    let mut x = middle - LAKE * u;
    while x < middle + LAKE * u {
        let w = s.next(14.0, 30.0) * u;
        let y = shore(middle, ground, u, x);
        out.push(oval(pos2(x, y + 1.6 * u), vec2(1.0, 0.0), w, s.next(2.5, 4.0) * u, SNOW_SHADE));
        out.push(oval(pos2(x, y), vec2(1.0, 0.0), w, s.next(2.5, 4.5) * u, SNOW));
        x += w * s.next(1.0, 1.6);
    }
    out
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
    /// chapter a pile with a block of ice for each page it has had (`Doc::garden`), cut out of the
    /// lake, the pile before the newest carved into a figure when its chapter begins.
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
        if e.ice.tend(&self.story_id, garden, BLOCKS, HANG, FORM, now) {
            ctx.request_repaint();
        } else {
            // The snow falls and the ice sparkles.
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
        painter.extend(hillside(story, seen_area, anchor.x, ground, unzoomed, now).into_iter().map(|mut s| {
            s.transform(zoom);
            s
        }));
        let places = places(story, page, ground, u);
        let forming = e.ice.finishing(now, FORM);
        let mut seen = Seen { figures: vec![(Rect::NOTHING, 0, 0.0); n], forming: forming.is_some(), ..Seen::default() };
        // Those further up the hill first, so the nearer stand in front.
        let mut order: Vec<(usize, Place)> = (0..n).map(|k| (k, places[e.ice.place(k, places.len(), |p| places[p].in_view(area, page))])).collect();
        order.sort_by(|a, b| a.1.foot.y.total_cmp(&b.1.foot.y).then(a.0.cmp(&b.0)));
        let mut coming = (Vec::new(), Vec::new());
        for (k, place) in order {
            let face = if place.foot.x < page.center().x { 1.0 } else { -1.0 };
            let blocks = e.ice.grown(garden, k, BLOCKS);
            let figure = Figure::of(story, k, &place, face, blocks.ceil() as usize);
            let formed = match forming {
                Some((m, q)) if m == k => q,
                _ if k == n - 1 => 0.0,
                _ => 1.0,
            };
            painter.extend(figure.draw(blocks, formed, now));
            if k == n - 1 && blocks.fract() > 0.0 {
                // The block on its way, cut out of the lake below the place, as laid out unzoomed.
                let below = anchor.x + (place.foot.x - anchor.x) / z;
                let from = zoom * cut_at(story, k, blocks as usize, anchor.x, ground, unzoomed, below);
                let (lake, block, at) = figure.coming(blocks as usize, blocks.fract(), from, u, now);
                coming.0.extend(lake);
                coming.1.extend(block);
                seen.coming = Some(at);
            }
            let shown = if formed > 0.5 { figure.bounds() } else { figure.pile_bounds(blocks as usize) };
            seen.figures[k] = (shown, blocks as usize, formed);
            seen.formed += usize::from(formed >= 1.0);
            seen.which.push(figure.which);
        }
        painter.extend(coming.0);
        painter.extend(coming.1);
        painter.extend(snowfall(story, area, unzoomed, now));
        self.eggs.snow = Some(seen);
    }
}

/// The hillside drawn last frame, for tests: each chapter's ice, around what stands in its place
/// (its pile, or its figure once mostly formed), with how many blocks and how far it is formed;
/// where the block on its way is, if one is; whether a pile is forming, how many figures are
/// carved, and which figures were drawn.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub figures: Vec<(Rect, usize, f32)>,
    pub coming: Option<Pos2>,
    pub forming: bool,
    pub formed: usize,
    pub which: Vec<usize>,
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

    fn figure(k: usize, pages: usize) -> Figure {
        Figure::of("a story", k, &Place { foot: pos2(300.0, 820.0), size: 1.0, haze: 0.0, w: 84.0 }, 1.0, pages)
    }

    fn around(shapes: &[Shape]) -> Rect {
        shapes.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect())
    }

    #[test]
    fn the_hillside_is_there_from_the_start_the_same_for_the_same_story() {
        let shapes = |story: &str| [hillside(story, desk(), 700.0, 880.0, 1.0, 0.0), snowfall(story, desk(), 1.0, 0.0)].concat();
        assert!(shapes("a story").len() > 200, "stars, hills, pines, the lake and snow: {}", shapes("a story").len());
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
        // As when zoomed out (more of it shows) or in (less of it): the hills, pines, stars, moon
        // and lake are where they were, only the plain fills reaching further.
        let laid = |area: Rect| hillside("a story", area, 700.0, 880.0, 1.0, 3.0).into_iter().filter(|s| !matches!(s, Shape::Rect(_) | Shape::Mesh(_))).map(|s| s.visual_bounding_rect()).collect::<Vec<_>>();
        assert_eq!(laid(desk()), laid(desk().expand2(vec2(500.0, 300.0))));
        assert_eq!(laid(desk()), laid(desk().shrink(200.0)));
    }

    #[test]
    fn the_lake_lies_along_the_foot_of_the_desk_under_the_page_and_either_side() {
        let (middle, ground) = (700.0, 880.0);
        let ice = hillside("a story", desk(), middle, ground, 1.0, 0.0).into_iter().find_map(|s| match s {
            Shape::Mesh(m) if m.vertices.iter().any(|v| v.color == LAKE_FAR) => Some(m.calc_bounds()),
            _ => None,
        }).expect("the lake");
        assert!(ice.left() < 0.0 && ice.right() > desk().right(), "either side of the page: {ice:?}");
        assert!(ice.top() > ground - 90.0 && ice.top() < ground - 40.0 && ice.bottom() > desk().bottom(), "along the foot: {ice:?}");
        for x in [100.0, 700.0, 1300.0] {
            let at = cut_at("a story", 0, 3, middle, ground, 1.0, x);
            assert!(at.y > shore(middle, ground, 1.0, at.x) + 10.0 && at.y < desk().bottom(), "cut out of the ice: {at:?}");
        }
        let places = places("a story", page(), ground, 1.0);
        assert!(places.iter().all(|p| p.foot.y < shore(middle, ground, 1.0, p.foot.x)), "the ice stands beyond the lake");
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
    fn a_block_is_cut_out_of_the_lake_rises_and_floats_onto_the_pile() {
        let f = figure(0, 5);
        let from = pos2(420.0, 870.0);
        let (lake, block, at) = f.coming(3, 0.1, from, 1.0, 0.0);
        assert!(block.is_empty() && at == from && !lake.is_empty(), "first a glint traces round it in the ice");
        let (lake, _, at) = f.coming(3, 0.45, from, 1.0, 0.0);
        assert!(at.y < from.y - 5.0 && (at.x - from.x).abs() < 1.0, "risen out of the water: {at:?}");
        assert!(lake.iter().any(|s| matches!(s, Shape::Mesh(m) if m.vertices[0].color.b() > 40 && m.vertices[0].color.r() < 40)), "the hole dark");
        let (_, _, half) = f.coming(3, 0.7, from, 1.0, 0.0);
        assert!(half.y < f.slot(3).y.min(from.y) - 10.0, "floating over: {half:?}");
        let (lake, _, at) = f.coming(3, 1.0, from, 1.0, 0.0);
        assert!(at.distance(f.slot(3)) < 0.5, "settled on the pile: {at:?} {:?}", f.slot(3));
        assert!(lake.iter().all(|s| !matches!(s, Shape::Mesh(m) if m.vertices.iter().any(|v| v.color.a() > 0))), "frozen over");
        for k in 0..200 {
            let (a, b) = (f.coming(3, 0.3 + 0.7 * k as f32 / 200.0, from, 1.0, 0.0).2, f.coming(3, 0.3 + 0.7 * (k + 1) as f32 / 200.0, from, 1.0, 0.0).2);
            assert!(a.distance(b) < 6.0, "smoothly at {k}: {a:?} {b:?}");
        }
    }

    #[test]
    fn the_pile_stacks_a_block_a_page_in_rows_without_overlapping() {
        let f = figure(0, BLOCKS);
        for j in 0..BLOCKS {
            let a = f.slot(j);
            assert!(a.y <= f.foot.y && (a.x - f.foot.x).abs() < 40.0, "{j}: on the place: {a:?}");
            for i in 0..j {
                let b = f.slot(i);
                assert!((a.x - b.x).abs() >= BLOCK.0 - 3.5 || (a.y - b.y).abs() >= BLOCK.1, "{j} and {i} apart: {a:?} {b:?}");
            }
        }
        let drawn = |n: f32| f.draw(n, 0.0, 0.0).len();
        assert!(drawn(5.0) > drawn(4.0) && drawn(BLOCKS as f32) > drawn(BLOCKS as f32 - 1.0), "a block more for a page more");
        assert!(f.pile_bounds(BLOCKS).height() > f.pile_bounds(4).height(), "rows on rows");
    }

    #[test]
    fn a_pile_becomes_an_ice_figure_rising_from_its_foot_bigger_for_more_pages() {
        for which in 0..FIGURES {
            let f = (0..FIGURES).map(|k| figure(k, 8)).find(|f| f.which == which).expect("every figure for some chapter");
            let pile = f.pile_bounds(8);
            let top = |q: f32| around(&f.draw(8.0, q, 0.0).into_iter().filter(|s| matches!(s, Shape::Mesh(m) if m.vertices.len() > 4 && m.vertices.len() < 40)).collect::<Vec<_>>()).top();
            assert!(f.draw(8.0, 0.0, 0.0).iter().all(|s| !matches!(s, Shape::Path(p) if p.closed)), "{which}: a pile, no figure yet");
            assert!(top(0.4) > top(0.6) && top(0.6) > top(1.0), "{which}: rising from its foot");
            assert!(f.bounds().height() > pile.height() * 1.2, "{which}: the figure taller than its pile: {:?} {pile:?}", f.bounds());
            assert!(f.bounds().center().distance(f.foot) < f.bounds().height(), "{which}: in its place");
            let blocks = |q: f32| f.draw(8.0, q, 0.0).iter().filter(|s| matches!(s, Shape::Mesh(m) if m.vertices.len() == 4 && m.vertices[0].color.a() > 0)).count();
            assert!(blocks(0.1) >= 16 && f.draw(8.0, 1.0, 0.0).len() == f.draw(0.0, 1.0, 0.0).len(), "{which}: the blocks gone into it");
        }
        assert!(figure(0, BLOCKS).bounds().height() > figure(0, 2).bounds().height() * 1.2, "a long chapter's figure bigger");
        let whiches: Vec<usize> = (0..8).map(|k| figure(k, 6).which).collect();
        assert!(whiches.windows(2).all(|w| w[0] != w[1]), "never the same twice running: {whiches:?}");
    }

    #[test]
    fn a_figure_carved_sparkles_now_and_then_and_is_ice() {
        let f = figure(1, 6);
        let sparks = |t: f64| f.draw(6.0, 1.0, t).iter().filter(|s| matches!(s, Shape::LineSegment { stroke, .. } if stroke.color.a() > 0)).count();
        assert!((0..400).any(|i| sparks(i as f64 * 0.1) > 0) && (0..400).any(|i| sparks(i as f64 * 0.1) == 0), "now and then");
        let icy = f.draw(6.0, 1.0, 0.0).iter().any(|s| matches!(s, Shape::Mesh(m) if m.vertices.iter().any(|v| v.color.b() > v.color.r() + 20)));
        assert!(icy, "clear blue ice");
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
