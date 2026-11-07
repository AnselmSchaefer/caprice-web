//! The Magic Snow. A snowy hillside at twilight lies on the desk: a few stars and the moon come out
//! over hills with pines on them, and snow falls softly in front of everything. Each chapter has a
//! Christmas tree on the hillside beside the page, the trees spread about it, some near and big,
//! some further up the hill, smaller and bluer in the dusk. Each page of the chapter being written
//! hangs an ornament on its tree, popping out of a glint: baubles, three golden garlands, and a
//! star on top last. When the next chapter begins, three penguins in scarves drag its tree in
//! from beyond the side of the desk, lying down on ropes, haul it upright, turn to wave to the
//! writer and waddle off the way they came; and the tree before it is lit, its fairy lights
//! twinkling, its star shining and its glow warm on the snow. Nothing brings a title in this theme.
//!
//! Like everything in `eggs.rs` it lies on the desk behind the pages and is drawn from shapes as a
//! function of the story and of time; nothing is printed or saved, and nothing new is kept: the
//! trees and their ornaments are counted from `Doc::garden`.

use super::*;

pub const SNOW: Color32 = Color32::from_rgb(250, 252, 255);
const SNOW_SHADE: Color32 = Color32::from_rgb(192, 204, 232);
const HILL: Color32 = Color32::from_rgb(214, 222, 244);
const HILL_FAR: Color32 = Color32::from_rgb(160, 172, 214);
const PINE: Color32 = Color32::from_rgb(70, 92, 128);
const STAR: Color32 = Color32::from_rgb(255, 250, 230);
const MOON: Color32 = Color32::from_rgb(250, 246, 226);
/// The dusk that pales and blues what lies further up the hill.
const DUSK: Color32 = Color32::from_rgb(120, 130, 182);
/// The trees: their needles in the light, in shade and deep within, their trunks, lit and shaded,
/// and what hangs on them: gold, lit and shaded, glass baubles and warm white fairy lights.
const FIR_LIT: Color32 = Color32::from_rgb(92, 152, 106);
const FIR_SHADE: Color32 = Color32::from_rgb(26, 74, 54);
const FIR_DEEP: Color32 = Color32::from_rgb(10, 36, 30);
const BARK_LIT: Color32 = Color32::from_rgb(132, 88, 58);
const BARK_DARK: Color32 = Color32::from_rgb(66, 42, 30);
pub const GOLD: Color32 = Color32::from_rgb(246, 206, 112);
const GOLD_DARK: Color32 = Color32::from_rgb(176, 128, 44);
const BAUBLES: [Color32; 5] = [
    Color32::from_rgb(160, 28, 44),
    Color32::from_rgb(214, 170, 80),
    Color32::from_rgb(200, 206, 218),
    Color32::from_rgb(40, 66, 136),
    Color32::from_rgb(232, 206, 160),
];
pub const LIGHTS: [Color32; 5] = [
    Color32::from_rgb(255, 236, 180),
    Color32::from_rgb(255, 222, 150),
    Color32::from_rgb(255, 246, 214),
    Color32::from_rgb(255, 210, 140),
    Color32::from_rgb(250, 240, 200),
];
const GLOW: Color32 = Color32::from_rgb(255, 214, 140);
const GLINT: Color32 = Color32::from_rgb(255, 248, 220);
/// The penguins, and their ropes.
pub const PENGUIN: Color32 = Color32::from_rgb(30, 34, 44);
const BELLY: Color32 = Color32::from_rgb(250, 250, 250);
const BEAK: Color32 = Color32::from_rgb(250, 170, 40);
const SCARVES: [Color32; 3] = [Color32::from_rgb(220, 60, 60), Color32::from_rgb(60, 150, 90), Color32::from_rgb(70, 110, 200)];
pub const ROPE: Color32 = Color32::from_rgb(150, 110, 70);

/// How many ornaments make a whole tree, one for each page of its chapter; the last is its star.
pub const ORNAMENTS: usize = 12;
/// Seconds an ornament takes to come, and the penguins to bring a new tree in, raise it and go
/// (the tree before is lit as the new one stands).
pub const HANG: f32 = 1.2;
pub const ARRIVE: f32 = 9.0;
/// How far the places reach from the page whatever the window, and how far up the hill they go,
/// in units; how many depths there are.
const REACH: f32 = 1800.0;
const RISE: f32 = 170.0;
const DEPTHS: usize = 5;
/// The tiers of boughs of a tree from the bottom up: where each begins, in units above its foot,
/// how wide it is either side, and how high; each overlaps the one below.
const TIERS: [(f32, f32, f32); 6] = [(-16.0, 46.0, 40.0), (-34.0, 40.0, 38.0), (-52.0, 34.0, 36.0), (-70.0, 27.0, 34.0), (-87.0, 20.0, 31.0), (-102.0, 13.0, 26.0)];
/// Which ornaments are garlands, round which tier.
const GARLANDS: [(usize, usize); 3] = [(2, 0), (6, 2), (9, 4)];
const FAIRY: usize = 34;

/// A point within tier `t`, `f` of the way up it from its bottom and `along` of the way out to its
/// edge (from -1 to 1), and how wide the tier is there.
fn within(t: usize, f: f32, along: f32) -> ((f32, f32), f32) {
    let (base, w, h) = TIERS[t];
    let room = w * (1.0 - f);
    ((along * room, base - h * f), room)
}

/// How much light falls on a tree at `x` of the way across it (from -1 to 1): the light comes from
/// the left.
fn light(x: f32) -> f32 {
    (0.5 - 0.55 * x).clamp(0.0, 1.0)
}

/// The needles' colour where `light` falls, `deep` within the tree.
fn needles(light: f32, deep: f32) -> Color32 {
    FIR_SHADE.lerp_to_gamma(FIR_LIT, light).lerp_to_gamma(FIR_DEEP, deep)
}

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

/// What hangs on a tree.
#[derive(Clone, Copy, Debug)]
enum Ornament {
    Bauble((f32, f32), Color32),
    Garland(usize),
    Star,
}

/// A Christmas tree: where it stands, its size, how much the dusk blues it, the side away from the
/// page (where it is brought in from), its ornaments in the order they come, its fairy lights, and
/// a number of its own that makes its boughs unlike another's.
#[derive(Clone, Debug)]
pub struct Tree {
    pub foot: Pos2,
    u: f32,
    haze: f32,
    out: f32,
    ornaments: Vec<Ornament>,
    fairy: Vec<((f32, f32), f32)>,
    phase: f32,
}

impl Tree {
    /// Chapter `k`'s tree, at `place`, its back to `out`.
    pub fn of(story: &str, k: usize, place: &Place, out: f32) -> Tree {
        let mut s = Seeds::of(story, 99_500 + k);
        let ornaments = (0..ORNAMENTS).map(|j| {
            if j == ORNAMENTS - 1 {
                return Ornament::Star;
            }
            if let Some(&(_, t)) = GARLANDS.iter().find(|g| g.0 == j) {
                return Ornament::Garland(t);
            }
            let t = (s.next(0.0, TIERS.len() as f32 - 1.0) as usize).min(TIERS.len() - 2);
            let (at, _) = within(t, s.next(0.12, 0.5), s.next(-0.8, 0.8));
            Ornament::Bauble(at, BAUBLES[(j + k) % BAUBLES.len()])
        }).collect();
        let fairy = (0..FAIRY).map(|_| {
            let t = (s.next(0.0, TIERS.len() as f32) as usize).min(TIERS.len() - 1);
            (within(t, s.next(0.08, 0.75), s.next(-0.9, 0.9)).0, s.next(0.0, 6.0))
        }).collect();
        Tree { foot: place.foot, u: place.size, haze: place.haze, out, ornaments, fairy, phase: s.next(0.0, 50.0) }
    }

    fn hazed(&self, c: Color32) -> Color32 {
        c.lerp_to_gamma(DUSK, self.haze)
    }

    /// Around the tree standing in its place.
    pub fn bounds(&self) -> Rect {
        Rect::from_min_max(self.foot + vec2(-46.0, -140.0) * self.u, self.foot + vec2(46.0, 0.0) * self.u)
    }

    /// The point `(x, y)` of the tree, in its units from its foot, with its foot at `foot` and
    /// raised `up` (0 lying on its side, its top away from the page, 1 upright).
    pub fn point(&self, foot: Pos2, up: f32, (x, y): (f32, f32)) -> Pos2 {
        let a = self.out * PI / 2.0 * (1.0 - up);
        let (c, s) = (a.cos(), a.sin());
        foot + vec2(0.0, -42.0 * self.u * s.abs()) + vec2(x * c - y * s, x * s + y * c) * self.u
    }

    /// The bottom edge of tier `t`, `b` from its foot and `w` wide: the tips of its boughs, the
    /// outer ones drooping a little, and the notches between them, each with whether it is a tip.
    fn edge(&self, t: usize, b: f32, w: f32) -> Vec<((f32, f32), bool)> {
        let n = (5.0 + w / 7.0) as usize;
        let mut out = Vec::new();
        for j in 0..n {
            let x = -w + 2.0 * w * j as f32 / (n - 1) as f32;
            let jitter = ((j as f32 * 12.99 + t as f32 * 78.23 + self.phase).sin() * 437.5).fract() * 2.4 - 1.2;
            out.push(((x, b + 1.5 * (x / w).powi(2) + jitter), true));
            if j + 1 < n {
                let mid = x + w / (n - 1) as f32;
                out.push(((mid, b - 4.5 - 1.5 * (1.0 - (mid / w).powi(2))), false));
            }
        }
        out
    }

    /// The tree with its foot at `foot`, raised `up`, `ornaments` hung on it (the last perhaps
    /// coming out of a glint), and lit from 0 to 1. Its boughs come in tiers, each one smooth mesh
    /// shaded from the light on the left into shade on the right and deeper between its tips, each
    /// casting a soft shadow on the tier below, with snow resting on some of its tips.
    pub fn draw(&self, foot: Pos2, up: f32, ornaments: f32, lit: f32, time: f64) -> Vec<Shape> {
        let (u, t) = (self.u, time as f32);
        let p = |x: f32, y: f32| self.point(foot, up, (x, y));
        let mut out = Vec::new();
        if up >= 1.0 {
            // Its shadow on the snow, away from the light.
            out.push(radial(foot + vec2(14.0, -2.0) * u, 58.0 * u, 8.0 * u, self.hazed(SNOW_SHADE), 0.8));
            if lit > 0.0 {
                out.push(radial(p(0.0, -55.0), 120.0 * u, 95.0 * u, GLOW, 0.26 * lit));
            }
        }
        // The trunk, round: lit on one side, in shade on the other.
        let bark = [BARK_LIT, BARK_DARK, BARK_DARK, BARK_LIT].map(|c| self.hazed(c));
        out.push(painted(&[p(-5.5, -22.0), p(5.5, -22.0), p(5.5, 0.0), p(-5.5, 0.0)], &bark));
        let edges: Vec<_> = TIERS.iter().enumerate().map(|(i, &(b, w, _))| self.edge(i, b, w)).collect();
        for (i, &(b, w, h)) in TIERS.iter().enumerate() {
            let apex = (0.0, b - h);
            let edge = &edges[i];
            let mut mesh = egui::epaint::Mesh::default();
            mesh.colored_vertex(p(apex.0, apex.1), self.hazed(needles(0.55, 0.3)));
            for &((x, y), tip) in edge {
                mesh.colored_vertex(p(x, y), self.hazed(needles(light(x / w), if tip { 0.0 } else { 0.5 })));
            }
            for j in 1..edge.len() as u32 {
                mesh.add_triangle(0, j, j + 1);
            }
            out.push(Shape::mesh(mesh));
            // Its edge, drawn finely so it is smooth.
            let rim = |x: f32| glow(self.hazed(needles(light(x / w), 0.35)), 0.8);
            for pair in edge.windows(2) {
                out.push(Shape::line_segment([p(pair[0].0.0, pair[0].0.1), p(pair[1].0.0, pair[1].0.1)], Stroke::new(0.8 * u, rim(pair[0].0.0))));
            }
            for &((x, y), _) in [edge[0], edge[edge.len() - 1]].iter() {
                out.push(Shape::line_segment([p(apex.0, apex.1), p(x, y)], Stroke::new(0.8 * u, rim(x))));
            }
            let top = p(apex.0, apex.1);
            for (j, &((x, y), _)) in edge.iter().filter(|e| e.1).enumerate() {
                let tip = p(x, y);
                // A bough: lighter along it where the light falls, darker in shade.
                let l = light(x / w);
                let ridge = if l > 0.5 { glow(self.hazed(FIR_LIT.lerp_to_gamma(Color32::WHITE, 0.12)), 0.3 * l) } else { glow(self.hazed(FIR_DEEP), 0.25 * (1.0 - l)) };
                out.push(Shape::line_segment([top + (tip - top) * 0.3, top + (tip - top) * 0.96], Stroke::new(0.9 * u, ridge)));
                // Snow resting here and there on a tip, on its upper side, more where the light falls.
                if (j * 7 + i * 3 + self.phase as usize).is_multiple_of(3) && l > 0.3 {
                    let along = (tip - top).normalized();
                    let mut n = along.rot90();
                    if n.y > 0.0 {
                        n = -n;
                    }
                    out.push(oval(top + (tip - top) * 0.8 + n * 1.0 * u, along, (tip - top).length() * 0.11, 1.2 * u, glow(self.hazed(SNOW), 0.85)));
                }
            }
            // The shadow the tier above casts on this one, soft, just below its edge.
            if let Some(above) = edges.get(i + 1) {
                let mut mesh = egui::epaint::Mesh::default();
                for &((x, y), _) in above {
                    mesh.colored_vertex(p(x, y - 1.0), glow(self.hazed(FIR_DEEP), 0.45));
                    mesh.colored_vertex(p(x, y + 7.0), glow(self.hazed(FIR_DEEP), 0.0));
                }
                for j in 0..above.len() as u32 - 1 {
                    let (a, b) = (2 * j, 2 * j + 2);
                    mesh.add_triangle(a, a + 1, b);
                    mesh.add_triangle(b, a + 1, b + 1);
                }
                out.push(Shape::mesh(mesh));
            }
        }
        let (_, _, h) = TIERS[TIERS.len() - 1];
        let crown = p(0.0, TIERS[TIERS.len() - 1].0 - h + 3.0);
        out.push(oval(crown, vec2(1.0, 0.0), 2.2 * u, 1.2 * u, self.hazed(SNOW)));
        if lit > 0.0 {
            for (j, &((x, y), phase)) in self.fairy.iter().enumerate() {
                let a = lit * (0.55 + 0.45 * (t * 2.2 + phase).sin());
                let c = p(x, y);
                let color = LIGHTS[j % LIGHTS.len()];
                out.push(Shape::circle_filled(c, 3.4 * u, glow(color, 0.2 * a)));
                out.push(Shape::circle_filled(c, 1.2 * u, glow(color, a)));
            }
        }
        let shown = (ornaments.ceil() as usize).min(ORNAMENTS);
        for (j, o) in self.ornaments.iter().enumerate().take(shown) {
            let g = (ornaments - j as f32).clamp(0.0, 1.0);
            let k = ease(g);
            let at = match *o {
                Ornament::Bauble((x, y), color) => {
                    // Glass, shaded round, with a highlight where the light falls, hung by a thread.
                    let (c, r) = (p(x, y), 3.4 * u * k);
                    out.push(Shape::line_segment([p(x, y - 3.4 * k - 2.5), p(x, y - 3.4 * k)], Stroke::new(0.5 * u, self.hazed(GOLD_DARK))));
                    out.push(Shape::circle_filled(c, r, self.hazed(color.lerp_to_gamma(Color32::BLACK, 0.45))));
                    out.push(Shape::circle_filled(c + vec2(-0.16, -0.16) * r, r * 0.8, self.hazed(color)));
                    out.push(Shape::circle_filled(c + vec2(-0.36, -0.36) * r, r * 0.36, self.hazed(color.lerp_to_gamma(Color32::WHITE, 0.45))));
                    out.push(Shape::circle_filled(c + vec2(-0.42, -0.46) * r, r * 0.13, glow(Color32::WHITE, 0.9)));
                    out.push(Shape::circle_filled(p(x, y - 3.3 * k), 0.9 * u * k, self.hazed(GOLD)));
                    (x, y)
                }
                Ornament::Garland(tier) => {
                    // Fine gold beads looped round the tier, sagging, lit on one side.
                    let ((_, y0), room) = within(tier, 0.3, 0.0);
                    let n = (2.0 * room / 2.4) as usize;
                    let reach = (k * n as f32).ceil() as usize;
                    for i in 0..reach.min(n + 1) {
                        let x = room * (2.0 * i as f32 / n as f32 - 1.0) * 0.96;
                        let y = y0 + 4.0 * (1.0 - (x / room).powi(2));
                        out.push(Shape::circle_filled(p(x, y), 0.8 * u, self.hazed(GOLD_DARK.lerp_to_gamma(GOLD, light(x / room)))));
                    }
                    (0.0, y0)
                }
                Ornament::Star => {
                    // A star of five points, each in two facets, one in the light and one in shade.
                    let c = p(0.0, -128.0);
                    if lit > 0.0 {
                        out.push(radial(c, 28.0 * u, 28.0 * u, GOLD, 0.55 * lit * (0.85 + 0.15 * (t * 3.0).sin())));
                    }
                    let r = 9.0 * u * k;
                    let a0 = -PI / 2.0 + self.out * PI / 2.0 * (1.0 - up);
                    let at = |i: f32, rr: f32| c + Vec2::angled(a0 + i * 2.0 * PI / 5.0) * rr;
                    for i in 0..5 {
                        let i = i as f32;
                        out.push(solid(vec![c, at(i - 0.5, r * 0.4), at(i, r)], self.hazed(GOLD)));
                        out.push(solid(vec![c, at(i, r), at(i + 0.5, r * 0.4)], self.hazed(GOLD_DARK)));
                    }
                    (0.0, -128.0)
                }
            };
            if g < 1.0 {
                out.push(radial(p(at.0, at.1), 16.0 * u, 16.0 * u, GLINT, 0.8 * (1.0 - k).max(0.3)));
            }
        }
        out
    }

    /// The tree being brought in, `q` of the way through: where its foot is, how far it is raised,
    /// and the penguins at work. They drag it in from `edge` (the desk's edge away from the page)
    /// lying on its side, its top trailing, haul it upright on their ropes, turn to wave to the
    /// writer and waddle off the way they came, beyond the edge.
    pub fn arriving(&self, edge: f32, q: f32, t: f32) -> (Pos2, f32, Crew) {
        let (u, out) = (self.u, self.out);
        let start = edge + out * 120.0 * u;
        let foot = pos2(start + (self.foot.x - start) * ease(q / 0.4), self.foot.y);
        let up = ease((q - 0.42) / 0.18);
        let leaving = ((q - 0.68) / 0.32).clamp(0.0, 1.0);
        let moving = q < 0.4 || leaving > 0.0;
        let face = if q < 0.6 {
            -out
        } else if q < 0.68 {
            -out * (1.0 - ease((q - 0.6) / 0.03))
        } else {
            out * ease((q - 0.68) / 0.03)
        };
        let mut crew = Crew { at: [Pos2::ZERO; 3], face, step: [0.0; 3], wave: ease((q - 0.6) / 0.02) * (1.0 - ease((q - 0.67) / 0.02)), rope: None, s: 18.0 * u, haze: self.haze };
        for i in 0..3 {
            // Ahead of the tree, on the page's side of it, a little apart.
            let base = foot.x - out * (26.0 + 22.0 * i as f32) * u;
            let x = base + out * ((edge - base).abs() + 60.0 * u) * leaving * leaving;
            crew.at[i] = pos2(x, foot.y - (i % 2) as f32 * 3.0 * u);
            crew.step[i] = if moving { (t * 9.0 + i as f32 * 1.7).sin() } else { 0.0 };
        }
        crew.rope = if q < 0.42 {
            Some(self.point(foot, up, (0.0, -8.0)))
        } else if q < 0.6 {
            Some(self.point(foot, up, (0.0, -110.0)))
        } else {
            None
        };
        (foot, up, crew)
    }
}

/// The penguins bringing a tree: where each stands, which way they face (1 to the right, -1 to the
/// left, 0 the writer), how each waddles, how far they wave, and what their ropes are tied to.
#[derive(Clone, Copy, Debug)]
pub struct Crew {
    pub at: [Pos2; 3],
    pub face: f32,
    step: [f32; 3],
    wave: f32,
    pub rope: Option<Pos2>,
    s: f32,
    haze: f32,
}

impl Crew {
    /// The penguins, each with its rope to the tree while they pull.
    pub fn draw(&self, time: f64) -> Vec<Shape> {
        let mut out = Vec::new();
        let hazed = |c: Color32| c.lerp_to_gamma(DUSK, self.haze);
        for i in (0..3).rev() {
            let (shapes, hand) = penguin(self.at[i], self.s, self.face, self.step[i], self.wave, hazed(SCARVES[i]), time as f32, &hazed);
            if let Some(to) = self.rope {
                out.push(Shape::line_segment([hand, to], Stroke::new(0.12 * self.s, hazed(ROPE))));
            }
            out.extend(shapes);
        }
        out
    }
}

/// A penguin standing at `foot`, `s` its size, facing `f` (1 to the right, -1 to the left, 0 the
/// writer), waddling `step` (from -1 to 1), waving `wave`, in a scarf of `scarf`; and where its
/// front flipper ends, which a rope is held in.
#[allow(clippy::too_many_arguments)]
fn penguin(foot: Pos2, s: f32, f: f32, step: f32, wave: f32, scarf: Color32, t: f32, hazed: &dyn Fn(Color32) -> Color32) -> (Vec<Shape>, Pos2) {
    let mut out = Vec::new();
    let px = foot.x + step * 0.08 * s;
    let y = foot.y;
    for (k, dx) in [-0.25f32, 0.25].into_iter().enumerate() {
        let lift = (if k == 0 { step } else { -step }).max(0.0) * 0.15 * s;
        out.push(oval(pos2(px + dx * s + f * 0.1 * s, y - 0.05 * s - lift), vec2(1.0, 0.0), 0.22 * s, 0.08 * s, hazed(BEAK)));
    }
    let back = if f.abs() > 0.1 { -f.signum() } else { 1.0 };
    out.push(oval(pos2(px - f * 0.45 * s, y - 1.05 * s), vec2(-f * 0.3, 1.0).normalized(), 0.55 * s, 0.14 * s, hazed(PENGUIN)));
    out.push(oval(pos2(px, y - 1.1 * s), vec2(step * 0.12, 1.0).normalized(), 1.1 * s, 0.7 * s, hazed(PENGUIN)));
    out.push(oval(pos2(px + f * 0.2 * s, y - 0.95 * s), vec2(0.0, 1.0), 0.8 * s, 0.48 * s * (1.0 - f.abs() * 0.25), hazed(BELLY)));
    let eye = y - 1.75 * s;
    if f.abs() > 0.5 {
        out.push(Shape::circle_filled(pos2(px + f * 0.3 * s, eye), 0.08 * s, hazed(BELLY)));
        out.push(Shape::circle_filled(pos2(px + f * 0.33 * s, eye), 0.045 * s, hazed(PENGUIN)));
        out.push(solid(vec![pos2(px + f * 0.55 * s, eye + 0.02 * s), pos2(px + f * 0.95 * s, eye + 0.15 * s), pos2(px + f * 0.55 * s, eye + 0.24 * s)], hazed(BEAK)));
    } else {
        for dx in [-0.22, 0.22] {
            out.push(Shape::circle_filled(pos2(px + dx * s, eye), 0.09 * s, hazed(BELLY)));
            out.push(Shape::circle_filled(pos2(px + dx * s, eye + 0.02 * s), 0.05 * s, hazed(PENGUIN)));
        }
        out.push(solid(vec![pos2(px - 0.12 * s, eye + 0.12 * s), pos2(px + 0.12 * s, eye + 0.12 * s), pos2(px, eye + 0.32 * s)], hazed(BEAK)));
    }
    let neck = y - 1.4 * s;
    out.push(oval(pos2(px, neck), vec2(1.0, 0.0), 0.62 * s, 0.13 * s, scarf));
    let flap = (t * 8.0).sin() * 0.08 * s;
    out.push(oval(pos2(px + back * 0.7 * s, neck + 0.15 * s + flap), vec2(back * 0.3, 1.0).normalized(), 0.4 * s, 0.1 * s, scarf));
    let shoulder = pos2(px + f * 0.35 * s + (1.0 - f.abs()) * 0.55 * s, y - 1.35 * s);
    let up = wave * (0.6 + 0.4 * (t * 10.0).sin());
    let side = if f.abs() > 0.1 { f.signum() } else { 1.0 };
    let dir = vec2(side * (0.35 + 0.3 * up), 1.0 - 1.8 * up).normalized();
    out.push(oval(shoulder + dir * 0.45 * s, dir, 0.5 * s, 0.13 * s, hazed(PENGUIN)));
    (out, shoulder + dir * 0.9 * s)
}

/// A place on the hillside a tree can stand: its foot, its size, how much the dusk blues it, and
/// how much room it has across.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub foot: Pos2,
    pub size: f32,
    pub haze: f32,
    pub w: f32,
}

impl Place {
    /// Whether the tree there is wholly on the desk, beside the page.
    pub fn in_view(&self, area: Rect, page: Rect) -> bool {
        let r = Rect::from_min_max(self.foot + vec2(-self.w / 2.0, -140.0 * self.size), self.foot + vec2(self.w / 2.0, 0.0));
        area.contains_rect(r) && !r.x_range().intersects(page.x_range())
    }
}

/// The places on the hillside, in the order trees take them: spread about rather than in rows,
/// nearer the page first but at every depth, near and big or further up the hill and smaller, each
/// a little off a grid so they do not line up. Far enough from the page that the penguins pulling a
/// tree in stand beside the page too. They come from `story` and reach as far from the page
/// whatever the window, so a place never moves when it changes; those beyond the desk are out of
/// sight.
pub fn places(story: &str, page: Rect, ground: f32, u: f32) -> Vec<Place> {
    let mut s = Seeds::of(story, 99_000);
    let mut out = Vec::new();
    for level in 0..DEPTHS {
        let depth = level as f32 / (DEPTHS - 1) as f32;
        let k = 1.0 - 0.5 * depth;
        let step = 135.0 * u * k;
        let y = ground - depth * RISE * u;
        let mut j = 0.0;
        while 95.0 * u * k + step * (j + 1.5) <= REACH * u {
            for x in [-1.0f32, 1.0] {
                let d = 95.0 * u * k + step * (j + 0.5 * (level % 2) as f32 + s.next(0.0, 0.3));
                let foot = pos2(if x < 0.0 { page.left() - d } else { page.right() + d }, y + s.next(-10.0, 10.0) * u * k);
                let key = d + s.next(0.0, 380.0) * u + level as f32 * 25.0 * u;
                out.push((key, Place { foot, size: u * k, haze: 0.5 * depth, w: 92.0 * u * k }));
            }
            j += 1.0;
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, p)| p).collect()
}

/// The hillside behind the village: stars coming out and the moon rising, hills far off and nearer
/// with pines on them, and the snow reaching up to them.
fn hillside(story: &str, area: Rect, ground: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let mut out = Vec::new();
    let horizon = ground - 230.0 * u;
    for i in 0..60 {
        let mut s = Seeds::of(story, 96_000 + i);
        let at = pos2(area.left() + area.width() * s.next(0.0, 1.0), area.top() + (horizon - area.top()) * s.next(0.0, 0.6));
        let a = 0.5 + 0.5 * (t * s.next(0.6, 1.8) + s.next(0.0, 6.0)).sin();
        out.push(Shape::circle_filled(at, s.next(0.5, 1.4) * u, glow(STAR, 0.3 + 0.5 * a)));
    }
    let mut s = Seeds::of(story, 97_000);
    // The moon off to one side, where the page does not hide it.
    let moon = pos2(area.left() + area.width() * if s.next(0.0, 1.0) < 0.5 { 0.12 } else { 0.88 }, area.top() + area.height() * 0.22);
    out.push(radial(moon, 110.0 * u, 110.0 * u, MOON, 0.22));
    out.push(Shape::circle_filled(moon, 22.0 * u, MOON));
    for (color, y, high) in [(HILL_FAR, horizon, 90.0), (HILL, horizon + 60.0 * u, 60.0)] {
        let mut x = area.left() - 100.0 * u;
        while x < area.right() + 100.0 * u {
            let w = s.next(180.0, 360.0) * u;
            let h = s.next(0.5, 1.0) * high * u;
            // Pines on the far hills, snow on their boughs.
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
            x += w * s.next(0.6, 0.9);
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

/// Snow falling softly over everything, each flake drifting from side to side.
fn snowfall(story: &str, area: Rect, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    (0..140).map(|i| {
        let mut s = Seeds::of(story, 98_000 + i);
        let near = s.next(0.0, 1.0);
        let y = area.top() + (area.height() * s.next(0.0, 1.0) + t * (12.0 + 26.0 * near) * u).rem_euclid(area.height());
        let x = area.left() + area.width() * s.next(0.0, 1.0) + (t * s.next(0.3, 0.8) + i as f32).sin() * 14.0 * u;
        Shape::circle_filled(pos2(x, y), (0.8 + 1.8 * near) * u, glow(SNOW, 0.5 + 0.4 * near))
    }).collect()
}

impl App {
    /// The hillside on the desk, behind the pages lying at `page`, in the Magic Snow only: a
    /// Christmas tree for each chapter, an ornament for each page it has had (`Doc::garden`), the
    /// newest pulled in by penguins when its chapter begins and the one before then lit.
    pub fn draw_snow(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.snow = None;
        if self.themes.theme != Theme::Snow {
            self.eggs.trees.forget();
            return;
        }
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let garden = &self.doc.garden;
        let n = garden.len().max(1);
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        let e = &mut self.eggs;
        if e.trees.tend(&self.story_id, garden, ORNAMENTS, HANG, ARRIVE, now) {
            ctx.request_repaint();
        } else {
            // The snow falls and the lights twinkle.
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        let u = e.unit(&ctx, area, now);
        let story = &self.story_id;
        let painter = ui.painter();
        let ground = area.bottom() - hidden - 14.0 * u;
        painter.extend(hillside(story, area, ground, u, now));
        let places = places(story, page, ground, u);
        let arriving = e.trees.finishing(now, ARRIVE);
        let mut seen = Seen { trees: vec![(Rect::NOTHING, 0, 0.0, 0.0); n], ..Seen::default() };
        // Those further up the hill first, so the nearer stand in front.
        let mut order: Vec<(usize, Place)> = (0..n).map(|k| (k, places[e.trees.place(k, places.len(), |p| places[p].in_view(area, page))])).collect();
        order.sort_by(|a, b| a.1.foot.y.total_cmp(&b.1.foot.y).then(a.0.cmp(&b.0)));
        for (k, place) in order {
            let out = if place.foot.x < page.center().x { -1.0 } else { 1.0 };
            let tree = Tree::of(story, k, &place, out);
            let ornaments = e.trees.grown(garden, k, ORNAMENTS);
            let lit = match arriving {
                Some((m, q)) if m == k => ease((q - 0.6) / 0.2),
                _ if k == n - 1 => 0.0,
                _ => 1.0,
            };
            let up = match arriving.filter(|_| k == n - 1) {
                Some((_, q)) => {
                    let edge = if out < 0.0 { area.left() } else { area.right() };
                    let (foot, up, crew) = tree.arriving(edge, q, now as f32);
                    painter.extend(tree.draw(foot, up, ornaments, lit, now));
                    painter.extend(crew.draw(now));
                    seen.penguins.extend(crew.at);
                    seen.pulling = crew.rope.is_some();
                    up
                }
                None => {
                    painter.extend(tree.draw(tree.foot, 1.0, ornaments, lit, now));
                    1.0
                }
            };
            seen.trees[k] = (tree.bounds(), ornaments.floor() as usize, lit, up);
            seen.lit += usize::from(lit >= 1.0);
        }
        painter.extend(snowfall(story, area, u, now));
        self.eggs.snow = Some(seen);
    }
}

/// The hillside drawn last frame, for tests: each chapter's tree, around it standing in its place,
/// with how many ornaments, how far it is lit and how far it is raised; where each penguin is and
/// whether they are pulling on their ropes; and how many trees are lit.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub trees: Vec<(Rect, usize, f32, f32)>,
    pub penguins: Vec<Pos2>,
    pub pulling: bool,
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

    fn tree(k: usize) -> Tree {
        Tree::of("a story", k, &Place { foot: pos2(300.0, 880.0), size: 1.0, haze: 0.0, w: 92.0 }, -1.0)
    }

    fn around(shapes: &[Shape]) -> Rect {
        shapes.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect())
    }

    #[test]
    fn the_hillside_is_there_from_the_start_the_same_for_the_same_story() {
        let shapes = |story: &str| [hillside(story, desk(), 880.0, 1.0, 0.0), snowfall(story, desk(), 1.0, 0.0)].concat();
        assert!(shapes("a story").len() > 200, "stars, hills, pines and snow: {}", shapes("a story").len());
        let at = |story: &str| shapes(story).iter().map(|s| s.visual_bounding_rect().center()).collect::<Vec<_>>();
        assert_eq!(at("a story"), at("a story"));
    }

    #[test]
    fn a_tree_gets_an_ornament_a_page_its_star_last() {
        let t = tree(0);
        let foot = t.foot;
        for n in 1..=ORNAMENTS {
            assert!(t.draw(foot, 1.0, n as f32, 0.0, 0.0).len() > t.draw(foot, 1.0, n as f32 - 1.0, 0.0, 0.0).len(), "{n}: one more");
            assert!(t.bounds().expand(1.0).contains_rect(around(&t.draw(foot, 1.0, n as f32, 0.0, 0.0)[1..])), "{n}: on the tree");
        }
        let star = |n: f32| t.draw(foot, 1.0, n, 0.0, 0.0).iter().any(|s| matches!(s, Shape::Path(p) if p.fill == GOLD && p.visual_bounding_rect().top() < foot.y - 132.0));
        assert!(!star(ORNAMENTS as f32 - 1.0) && star(ORNAMENTS as f32), "the star on top last");
        assert!(matches!(t.ornaments.iter().filter(|o| matches!(o, Ornament::Garland(_))).count(), 3), "three garlands");
    }

    #[test]
    fn a_tree_lit_twinkles_with_fairy_lights_and_glows() {
        let t = tree(1);
        let lights = |lit: f32, time: f64| t.draw(t.foot, 1.0, 5.0, lit, time).iter().filter(|s| matches!(s, Shape::Circle(c) if LIGHTS.iter().any(|l| c.fill.a() > 100 && c.fill.to_srgba_unmultiplied()[..3] == l.to_array()[..3]))).count();
        assert_eq!(lights(0.0, 0.0), 0, "dark until lit");
        assert!(lights(1.0, 0.0) >= FAIRY / 3, "lit: {}", lights(1.0, 0.0));
        let a = t.draw(t.foot, 1.0, 5.0, 1.0, 0.0);
        let b = t.draw(t.foot, 1.0, 5.0, 1.0, 0.7);
        assert!(a.iter().zip(&b).any(|(x, y)| x != y), "twinkling");
    }

    #[test]
    fn the_trees_are_spread_about_beside_the_page_near_and_far_up_the_hill() {
        let all = places("a story", page(), 880.0, 1.0);
        let mut taken = Tended::default();
        let got: Vec<Place> = (0..24).map(|k| all[taken.place(k, all.len(), |p| all[p].in_view(desk(), page()))]).collect();
        assert!(got.iter().any(|p| p.foot.x < page().left()) && got.iter().any(|p| p.foot.x > page().right()), "on both sides");
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

    #[test]
    fn the_penguins_drag_the_tree_in_lying_down_raise_it_wave_and_go() {
        let t = tree(2);
        let edge = desk().left();
        let at = |q: f32| t.arriving(edge, q, 0.0);
        let seen = |q: f32| {
            let (foot, up, crew) = at(q);
            around(&[t.draw(foot, up, 3.0, 0.0, 0.0), crew.draw(0.0)].concat())
        };
        assert!(!seen(0.0).intersects(desk()), "from beyond the desk: {:?}", seen(0.0));
        let (foot, up, crew) = at(0.2);
        assert!(up == 0.0 && t.point(foot, up, (0.0, -120.0)).x < foot.x - 100.0, "lying, its top trailing: {foot:?}");
        assert!(crew.rope.is_some() && crew.at.iter().all(|p| p.x > foot.x && crew.face > 0.9), "pulling ahead of it: {crew:?}");
        let (foot, up, _) = at(0.41);
        assert!(foot.distance(t.foot) < 1.0 && up == 0.0, "in its place, still lying");
        let (_, up, crew) = at(0.5);
        assert!(up > 0.2 && up < 0.9 && crew.rope.is_some(), "hauled up");
        let (_, up, crew) = at(0.64);
        assert!(up == 1.0 && crew.rope.is_none() && crew.face.abs() < 0.1 && crew.wave > 0.9, "standing; the penguins wave to the writer");
        let (_, _, crew) = at(1.0);
        assert!(crew.at.iter().all(|p| p.x < edge - 30.0), "gone the way they came: {crew:?}");
        for k in 0..200 {
            let (a, b) = (at(k as f32 / 200.0).0, at((k + 1) as f32 / 200.0).0);
            assert!(a.distance(b) < 15.0, "smoothly at {k}");
        }
    }
}
