//! The Magic Reef. A sunlit shallow sea lies on the desk: light falls in slanting rays from the
//! surface and sways with it, seagrass waves along a sandy floor with rocks and the reef fading
//! into the blue behind, and bubbles rise. Each chapter has a coral on the floor beside the page,
//! taking turns left and right, each of its own kind (a branching coral, a sea fan, tube sponges)
//! and colour; each page of the chapter being written grows it a branch, out of a little glimmer.
//! When the next chapter begins, the coral of the one before spawns: a cloud of eggs rises from its
//! tips to the surface, its polyps open, and a little shoal of fish swims in to live about it.
//! A new chapter title is brought by an octopus, which comes out from behind the side of the page,
//! reaches over it with an arm holding the title, sets it in its place and slips back behind.
//!
//! Like everything in `eggs.rs` it lies on the desk behind the pages and is drawn from shapes as a
//! function of the story and of time; nothing is printed or saved, and nothing new is kept: the
//! corals and their branches are counted from `Doc::garden`.

use super::*;

const SUN: Color32 = Color32::from_rgb(255, 252, 226);
const SAND: Color32 = Color32::from_rgb(234, 214, 168);
const SAND_DARK: Color32 = Color32::from_rgb(206, 184, 138);
const ROCK: Color32 = Color32::from_rgb(98, 120, 128);
const ROCK_DARK: Color32 = Color32::from_rgb(72, 92, 102);
/// The reef far behind, fading into the blue, and the seagrass.
const DISTANT: Color32 = Color32::from_rgb(30, 110, 140);
const GRASS: [Color32; 2] = [Color32::from_rgb(64, 150, 100), Color32::from_rgb(92, 170, 96)];
const BUBBLE: Color32 = Color32::from_rgb(236, 252, 255);
/// The light a new branch grows out of, the eggs a coral spawns, and its polyps once open.
const GLIMMER: Color32 = Color32::from_rgb(255, 250, 200);
const EGGS: [Color32; 3] = [Color32::from_rgb(255, 196, 206), Color32::from_rgb(255, 228, 170), Color32::from_rgb(255, 255, 240)];
const POLYP: Color32 = Color32::from_rgb(255, 246, 214);
/// Each kind of coral's colours, its body and its tips: one pair a coral.
const BRANCHING: [(Color32, Color32); 3] = [
    (Color32::from_rgb(242, 120, 112), Color32::from_rgb(255, 186, 168)),
    (Color32::from_rgb(178, 112, 210), Color32::from_rgb(222, 178, 244)),
    (Color32::from_rgb(244, 168, 64), Color32::from_rgb(255, 214, 134)),
];
const FANS: [(Color32, Color32); 3] = [
    (Color32::from_rgb(214, 70, 98), Color32::from_rgb(250, 140, 150)),
    (Color32::from_rgb(140, 84, 196), Color32::from_rgb(200, 160, 240)),
    (Color32::from_rgb(232, 112, 60), Color32::from_rgb(255, 172, 116)),
];
const TUBES: [(Color32, Color32); 3] = [
    (Color32::from_rgb(236, 150, 60), Color32::from_rgb(255, 204, 120)),
    (Color32::from_rgb(110, 172, 196), Color32::from_rgb(178, 222, 236)),
    (Color32::from_rgb(222, 96, 140), Color32::from_rgb(252, 166, 196)),
];
/// The fish: each coral's shoal one kind, its body and its stripe.
const FISH: [(Color32, Color32); 4] = [
    (Color32::from_rgb(255, 140, 40), Color32::from_rgb(255, 255, 255)),
    (Color32::from_rgb(56, 110, 220), Color32::from_rgb(255, 222, 60)),
    (Color32::from_rgb(255, 214, 40), Color32::from_rgb(40, 40, 60)),
    (Color32::from_rgb(120, 220, 210), Color32::from_rgb(40, 90, 150)),
];
const EYE: Color32 = Color32::from_rgb(20, 24, 32);
/// The octopus: its skin, darker and lighter, its suckers and the whites of its eyes.
pub const OCTOPUS: Color32 = Color32::from_rgb(226, 104, 92);
const OCTOPUS_DARK: Color32 = Color32::from_rgb(190, 76, 70);
const OCTOPUS_LIGHT: Color32 = Color32::from_rgb(246, 150, 128);
const SUCKER: Color32 = Color32::from_rgb(255, 214, 200);
const WHITE: Color32 = Color32::from_rgb(255, 250, 240);

/// How many branches make a whole coral, one for each page of its chapter.
pub const BRANCHES: usize = 12;
/// Seconds a branch takes to grow, and a coral to spawn once its chapter is done, the fish coming
/// in over the end of it.
pub const GROW: f32 = 1.4;
pub const SPAWN: f32 = 4.0;
/// The most fish in a coral's shoal, and how many eggs it spawns.
const SHOAL: usize = 6;
const CLOUD: usize = 40;

/// The kinds of coral, taking turns along the reef.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Branching,
    Fan,
    Tubes,
}

/// A branch of a coral, or one of its tubes: where it starts and where it ends once grown, and how
/// thick it is there.
#[derive(Clone, Copy, Debug)]
struct Branch {
    from: Pos2,
    to: Pos2,
    w: f32,
}

/// A coral: all its branches, in the order they grow, whatever how many are shown, so growing only
/// adds at the tips.
#[derive(Clone, Debug)]
pub struct Coral {
    pub kind: Kind,
    foot: Pos2,
    branches: Vec<Branch>,
    body: Color32,
    tip: Color32,
    u: f32,
}

impl Coral {
    /// Chapter `k`'s coral, standing at `foot`, `u` its size.
    pub fn of(story: &str, k: usize, foot: Pos2, u: f32) -> Coral {
        let turn = Seeds::of(story, 39_999).next(0.0, 3.0) as usize;
        let kind = [Kind::Branching, Kind::Fan, Kind::Tubes][(k + turn) % 3];
        let mut s = Seeds::of(story, 40_000 + k);
        let pick = (s.next(0.0, 3.0) as usize).min(2);
        let (body, tip) = match kind {
            Kind::Branching => BRANCHING[pick],
            Kind::Fan => FANS[pick],
            Kind::Tubes => TUBES[pick],
        };
        let up = -PI / 2.0;
        let dir = |a: f32| vec2(a.cos(), a.sin());
        let mut branches: Vec<Branch> = Vec::with_capacity(BRANCHES);
        match kind {
            Kind::Branching => {
                // Each branch forks in two near its tip, one each way, further out and thinner,
                // reaching up: the coral fills out level by level.
                let mut angles = vec![up + s.next(-0.15, 0.15)];
                branches.push(Branch { from: foot, to: foot + dir(angles[0]) * 40.0 * u, w: 13.0 * u });
                for i in 1..BRANCHES {
                    let p = (i - 1) / 2;
                    let parent = branches[p];
                    let from = parent.from + (parent.to - parent.from) * s.next(0.85, 1.0);
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let a = (angles[p] + side * s.next(0.35, 0.6)).clamp(up - 1.3, up + 1.3);
                    let len = (parent.from.distance(parent.to) * s.next(0.72, 0.95)).max(16.0 * u);
                    angles.push(a);
                    branches.push(Branch { from, to: from + dir(a) * len, w: (parent.w * 0.8).max(4.5 * u) });
                }
            }
            Kind::Fan => {
                // A short stem, then the fan's ribs spreading from it, from the middle outwards.
                let top = foot + vec2(0.0, -16.0 * u);
                branches.push(Branch { from: foot, to: top, w: 6.0 * u });
                for i in 1..BRANCHES {
                    let j = i - 1;
                    let side = if j % 2 == 0 { 1.0 } else { -1.0 };
                    let a = up + side * (0.05 + 0.19 * (j / 2) as f32 + s.next(-0.04, 0.04));
                    let len = s.next(62.0, 92.0) * u * (1.0 - 0.04 * (j / 2) as f32);
                    branches.push(Branch { from: top, to: top + dir(a) * len, w: 2.6 * u });
                }
            }
            Kind::Tubes => {
                // Tubes side by side on the floor, the tallest in the middle.
                for i in 0..BRANCHES {
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let n = i.div_ceil(2) as f32;
                    let x = side * n * 11.0 * u + s.next(-2.0, 2.0) * u;
                    let h = s.next(48.0, 80.0) * u * (1.0 - 0.07 * n);
                    let from = foot + vec2(x, 0.0);
                    branches.push(Branch { from, to: from + vec2(s.next(-5.0, 5.0) * u, -h), w: s.next(10.0, 14.0) * u });
                }
            }
        }
        Coral { kind, foot, branches, body, tip, u }
    }

    /// How far branch `i` is grown when `grown` branches are: none, in part, or whole.
    fn part(grown: f32, i: usize) -> f32 {
        (grown - i as f32).clamp(0.0, 1.0)
    }

    /// Where branch `i` ends when `grown` branches are.
    fn end(&self, grown: f32, i: usize) -> Pos2 {
        let b = &self.branches[i];
        b.from + (b.to - b.from) * Coral::part(grown, i)
    }

    /// Around what is shown of it when `grown` branches are.
    pub fn bounds(&self, grown: f32) -> Rect {
        let mut r = Rect::from_center_size(self.foot, Vec2::splat(2.0 * self.u));
        for (i, b) in self.branches.iter().enumerate().take(grown.ceil() as usize) {
            r = r.union(Rect::from_center_size(self.end(grown, i), Vec2::splat(b.w))).union(Rect::from_center_size(b.from, Vec2::splat(b.w)));
        }
        r
    }

    /// Where its tips are when `grown` branches are, which the eggs rise from.
    fn tips(&self, grown: f32) -> Vec<Pos2> {
        (0..(grown.ceil() as usize).min(BRANCHES)).map(|i| self.end(grown, i)).collect()
    }

    /// The coral with `grown` branches, the last perhaps in part, growing out of a glimmer; its
    /// polyps `open` from 0 to 1.
    pub fn draw(&self, grown: f32, open: f32, time: f64) -> Vec<Shape> {
        let (u, t) = (self.u, time as f32);
        let mut out = Vec::new();
        let shown = (grown.ceil() as usize).min(BRANCHES);
        if self.kind == Kind::Fan && shown > 1 {
            // The fan's lattice between its ribs.
            let mut web: Vec<Pos2> = (1..shown).map(|i| self.end(grown, i)).collect();
            web.push(self.branches[0].to);
            out.push(hull(web, glow(self.body, 0.35)));
        }
        for i in 0..shown {
            let b = &self.branches[i];
            let end = self.end(grown, i);
            if end.distance(b.from) < 0.5 {
                continue;
            }
            match self.kind {
                Kind::Branching => {
                    out.extend(ribbon(&[b.from, end], &[b.w, b.w * 0.82], self.body));
                    out.push(Shape::circle_filled(end, b.w * 0.36, self.tip));
                }
                Kind::Fan => {
                    out.push(Shape::line_segment([b.from, end], Stroke::new(b.w, self.body)));
                    out.push(Shape::circle_filled(end, b.w * 0.9, self.tip));
                }
                Kind::Tubes => {
                    let n = vec2(b.w / 2.0, 0.0);
                    out.push(solid(vec![b.from - n, b.from + n, end + n, end - n], self.body));
                    out.push(solid(vec![b.from - n, b.from - n * 0.4, end - n * 0.4, end - n], glow(self.tip, 0.35)));
                    out.push(oval(end, vec2(1.0, 0.0), b.w / 2.0, b.w * 0.2, self.tip));
                    out.push(oval(end, vec2(1.0, 0.0), b.w * 0.36, b.w * 0.12, glow(EYE, 0.55)));
                }
            }
            let g = Coral::part(grown, i);
            if g < 1.0 {
                // A new branch grows out of a little light at its tip.
                out.push(radial(end, 16.0 * u, 16.0 * u, GLIMMER, 0.75 * (1.0 - ease(g)).max(0.3)));
                out.push(Shape::circle_filled(end, 1.8 * u, glow(GLIMMER, 1.0 - g * 0.5)));
            } else if open > 0.0 {
                // Its polyps, open and swaying a little.
                for p in 0..3 {
                    let a = t * 1.3 + (i * 3 + p) as f32 * 2.1;
                    let at = end + vec2(a.cos(), a.sin() * 0.7) * (b.w * 0.45 + 2.0 * u);
                    out.push(Shape::circle_filled(at, 1.4 * u, glow(POLYP, 0.85 * open)));
                }
                out.push(radial(end, 9.0 * u, 9.0 * u, POLYP, 0.25 * open));
            }
        }
        out
    }
}

/// Where chapter `k`'s coral of `n` stands: taking turns on either side of the page, from its edge
/// outwards, closer together when there are many; and how big it is.
pub fn foot(k: usize, n: usize, area: Rect, page: Rect, ground: f32, u: f32) -> (Pos2, f32) {
    let left = k.is_multiple_of(2);
    let (j, on_side) = (k / 2, if left { n.div_ceil(2) } else { n / 2 });
    let gutter = if left { page.left() - area.left() } else { area.right() - page.right() };
    let gap = (170.0 * u).min(gutter / (on_side as f32 + 0.3)).max(40.0 * u);
    let d = gap * (j as f32 + 0.6);
    let x = if left { page.left() - d } else { page.right() + d };
    (pos2(x, ground), 1.4 * u * (gap / (170.0 * u)).clamp(0.55, 1.0))
}

/// The sea about the reef: rays of sunlight swaying down from the surface, the reef fading into the
/// blue behind, rocks and seagrass on the sand. The seagrass and the rays move with the water.
fn water(story: &str, area: Rect, page: Rect, ground: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let mut out = Vec::new();
    for i in 0..7 {
        let mut s = Seeds::of(story, 60_000 + i);
        let x = area.left() + area.width() * (i as f32 + s.next(0.1, 0.9)) / 7.0 + (t * s.next(0.15, 0.3) + i as f32).sin() * 24.0 * u;
        let (top, bottom) = (s.next(30.0, 70.0) * u, s.next(90.0, 170.0) * u);
        let slant = area.height() * 0.28;
        let foot_y = area.top() + area.height() * s.next(0.7, 0.95);
        let a = s.next(0.1, 0.18) * (0.8 + 0.2 * (t * 0.5 + i as f32 * 1.3).sin());
        let mut mesh = egui::epaint::Mesh::default();
        mesh.colored_vertex(pos2(x - top / 2.0, area.top()), glow(SUN, a));
        mesh.colored_vertex(pos2(x + top / 2.0, area.top()), glow(SUN, a));
        mesh.colored_vertex(pos2(x + slant + bottom / 2.0, foot_y), glow(SUN, 0.0));
        mesh.colored_vertex(pos2(x + slant - bottom / 2.0, foot_y), glow(SUN, 0.0));
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        out.push(Shape::mesh(mesh));
    }
    // The reef far behind, in the blue.
    let back = ground - 34.0 * u;
    let mut x = area.left() - 60.0 * u;
    let mut s = Seeds::of(story, 61_000);
    while x < area.right() + 60.0 * u {
        let w = s.next(50.0, 110.0) * u;
        out.push(oval(pos2(x, back), vec2(1.0, 0.0), w, s.next(18.0, 42.0) * u, glow(DISTANT, 0.45)));
        x += w * s.next(0.9, 1.3);
    }
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), back), pos2(area.right(), area.bottom())), 0.0, glow(DISTANT, 0.45)));
    // Seagrass, in clumps beside the page, swaying.
    for i in 0..(1800.0 / (120.0 * u)) as usize {
        for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let mut s = Seeds::of(story, 62_000 + i * 2 + k);
            let d = (40.0 + (i as f32 + s.next(-0.3, 0.3)) * 120.0) * u;
            let x = if side < 0.0 { page.left() - d } else { page.right() + d };
            for blade in 0..5 {
                let h = s.next(40.0, 110.0) * u;
                let root = pos2(x + (blade as f32 - 2.0) * 4.0 * u, ground + 4.0 * u);
                let phase = s.next(0.0, 6.0);
                let color = GRASS[blade % 2];
                let mut at = root;
                for j in 1..=5 {
                    let f = j as f32 / 5.0;
                    let sway = (t * 0.9 + phase + f * 1.5).sin() * h * 0.22 * f * f;
                    let next = pos2(root.x + sway + (blade as f32 - 2.0) * 3.0 * u * f, root.y - h * f);
                    out.push(Shape::line_segment([at, next], Stroke::new(3.2 * u * (1.0 - f * 0.6), color)));
                    at = next;
                }
            }
        }
    }
    // The sand, rolling a little, with pebbles and rocks on it.
    let mut s = Seeds::of(story, 63_000);
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), ground), pos2(area.right(), area.bottom())), 0.0, SAND));
    let mut x = area.left() - 40.0 * u;
    while x < area.right() + 40.0 * u {
        let w = s.next(70.0, 140.0) * u;
        out.push(oval(pos2(x, ground + 2.0 * u), vec2(1.0, 0.0), w, s.next(5.0, 10.0) * u, SAND));
        out.push(oval(pos2(x + w * 0.4, ground + s.next(8.0, 30.0) * u), vec2(1.0, 0.0), s.next(2.0, 4.0) * u, 1.5 * u, SAND_DARK));
        x += w;
    }
    for i in 0..(1800.0 / (260.0 * u)) as usize {
        for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let mut s = Seeds::of(story, 64_000 + i * 2 + k);
            if s.next(0.0, 1.0) < 0.4 {
                continue;
            }
            let d = (130.0 + (i as f32 + s.next(-0.3, 0.3)) * 260.0) * u;
            let x = if side < 0.0 { page.left() - d } else { page.right() + d };
            let (w, h) = (s.next(22.0, 40.0) * u, s.next(12.0, 22.0) * u);
            out.push(oval(pos2(x, ground), vec2(1.0, 0.0), w, h, ROCK_DARK));
            out.push(oval(pos2(x - w * 0.15, ground - h * 0.2), vec2(1.0, 0.0), w * 0.75, h * 0.75, ROCK));
        }
    }
    out
}

/// Bubbles rising from the sand beside the page, wobbling, in streams.
fn bubbles(story: &str, area: Rect, page: Rect, ground: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let mut out = Vec::new();
    let rise = ground - area.top();
    for i in 0..6 {
        let mut s = Seeds::of(story, 65_000 + i);
        let d = s.next(60.0, 520.0) * u;
        let x = if i % 2 == 0 { page.left() - d } else { page.right() + d };
        let speed = s.next(40.0, 70.0) * u;
        for j in 0..5 {
            let y = ground - (t * speed + j as f32 * rise / 5.0 + s.next(0.0, rise)).rem_euclid(rise);
            let r = s.next(2.0, 4.5) * u * (1.0 + 0.4 * (ground - y) / rise);
            let at = pos2(x + (t * 2.0 + j as f32).sin() * 4.0 * u, y);
            out.push(Shape::circle_stroke(at, r, Stroke::new(1.0 * u, glow(BUBBLE, 0.7))));
            out.push(Shape::circle_filled(at + vec2(-r * 0.35, -r * 0.35), r * 0.25, glow(BUBBLE, 0.8)));
        }
    }
    out
}

/// Where fish `i` of chapter `k`'s shoal is at `time`, swimming round its coral `home` its own way;
/// and which way it faces, from 1 (to the right) to -1. At each end of its way round it turns
/// through facing the writer over a second or two, as a fish turns, rather than at a stroke.
pub fn fish_at(story: &str, k: usize, i: usize, home: Rect, u: f32, time: f64) -> (Pos2, f32) {
    let mut s = Seeds::of(story, 50_000 + k * 100 + i);
    let t = time as f32;
    let c = home.center() + vec2(0.0, -home.height() * 0.15);
    let (rx, ry) = (home.width() / 2.0 + s.next(18.0, 46.0) * u, home.height() * 0.3 + s.next(8.0, 24.0) * u);
    let speed = s.next(0.22, 0.42) * if k.is_multiple_of(2) { 1.0 } else { -1.0 };
    let ang = s.next(0.0, 2.0 * PI) + speed * t;
    let at = c + vec2(ang.cos() * rx, ang.sin() * ry + (t * 1.7 + i as f32).sin() * 3.0 * u);
    // Facing the way it swims across, turning while it swims mostly up or down.
    let across = (-ang.sin() * speed.signum() / 0.35).clamp(-1.0, 1.0);
    (at, (across * PI / 2.0).sin())
}

/// A little fish at `at` facing `face` (1 to the right, -1 to the left, between while it turns,
/// seen narrower towards the middle), `color` and `stripe`, its tail beating.
fn fish(at: Pos2, face: f32, u: f32, (color, stripe): (Color32, Color32), k: usize, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let f = vec2(face, 0.0);
    let wag = (t * 9.0 + k as f32 * 1.3).sin() * 1.6 * u;
    let tail = at - f * 7.0 * u;
    vec![
        solid(vec![tail, tail - f * 5.0 * u + vec2(0.0, -4.5 * u + wag), tail - f * 5.0 * u + vec2(0.0, 4.5 * u + wag)], color),
        solid(vec![at + vec2(0.0, -3.0 * u), at - f * 4.0 * u + vec2(0.0, -6.5 * u), at - f * 5.0 * u + vec2(0.0, -3.0 * u)], color),
        oval(at, vec2(1.0, 0.0), (2.6 + 5.4 * face.abs()) * u, 4.5 * u, color),
        oval(at + f * 2.5 * u, vec2(0.0, 1.0), 4.0 * u, (0.4 + 0.8 * face.abs()) * u, stripe),
        Shape::circle_filled(at + f * 5.0 * u + vec2(0.0, -u), 1.1 * u, EYE),
    ]
}

/// The octopus bringing a title lying at `place` on a page lying at `page`: its head, behind the
/// page; where its arm comes over the page's edge and where its tip is; how far the title is from
/// its place, and how much of it shows.
#[derive(Clone, Copy, Debug)]
pub struct Octopus {
    pub body: Pos2,
    pub size: f32,
    side: f32,
    pub base: Pos2,
    pub tip: Pos2,
    pub offset: Vec2,
    pub shown: f32,
}

/// Where the octopus is at `p`, coming out on the right of the page or the left. Hidden behind the
/// page, it slides out past its edge a little below the title's height, holding the title up by
/// its end in the tip of an arm that comes over the edge, the title fading in as it shows; carries
/// it over its place, lowers it in, lets go and draws its arm back, and slips behind the page again.
pub fn octopus_at(right: bool, p: Pose, place: Rect, page: Rect) -> Octopus {
    let side = if right { 1.0 } else { -1.0 };
    let edge = if right { page.right() } else { page.left() };
    let size = page.width() * 0.075;
    let y = (place.center().y + size * 1.2).clamp(page.top() + size * 2.5, page.bottom() - size * 2.5);
    let (hidden, out) = (edge - side * size * 1.8, edge + side * size * 0.7);
    let body = pos2(hidden + (out - hidden) * p.enter, y);
    let base = pos2(edge, y - size * 0.1);
    // The title is held by its end nearer the octopus, lying towards the page.
    let end = |grip: Pos2| vec2(grip.x - side * place.width() / 2.0 - place.center().x, grip.y - place.center().y);
    let lift = size * 1.6;
    let set = pos2(place.center().x + side * place.width() / 2.0, place.center().y);
    let held = pos2(edge + side * size * 0.3, place.center().y - lift);
    let (tip, offset, shown) = if p.hooked {
        let over = set - vec2(0.0, lift);
        let carried = if p.run > 0.0 { bend(held, pos2((held.x + over.x) / 2.0, held.y - size * 0.8), over, p.run) } else { base + (held - base) * p.enter };
        let grip = carried + vec2(0.0, lift * p.lower);
        let shown = if p.run > 0.0 { 1.0 } else { ease((p.enter - 0.3) / 0.7) };
        (grip, end(grip), shown)
    } else {
        (set + (base - set) * p.hook, Vec2::ZERO, 1.0)
    };
    Octopus { body, size, side, base, tip, offset, shown }
}

impl Octopus {
    /// Its head and the arms it does not carry with, waving below it: behind the page, so the
    /// page's edge hides what is still behind it. Its eyes follow the tip of its arm.
    pub fn behind(&self, time: f64) -> Vec<Shape> {
        let (s, b, side, t) = (self.size, self.body, self.side, time as f32);
        let mut out = Vec::new();
        for a in 0..4 {
            let spread = a as f32 - 1.5;
            let root = b + vec2(side * spread * s * 0.32, s * 0.45);
            let spine: Vec<Pos2> = (0..=8).map(|j| {
                let f = j as f32 / 8.0;
                let wave = (t * 1.6 + a as f32 * 1.1 + f * 3.0).sin() * s * 0.35 * f;
                root + vec2(side * spread * s * 0.3 * f + wave + side * s * 0.5 * f * f * f, f * s * 1.9)
            }).collect();
            let widths: Vec<f32> = (0..=8).map(|j| s * 0.34 * (1.0 - j as f32 / 8.0 * 0.85)).collect();
            out.extend(ribbon(&spine, &widths, if a % 2 == 0 { OCTOPUS_DARK } else { OCTOPUS }));
        }
        let breathe = 1.0 + (t * 1.8).sin() * 0.03;
        let mantle = b + vec2(side * s * 0.15, -s * 0.8);
        out.push(oval(mantle, vec2(side * 0.25, -1.0).normalized(), s * 1.15 * breathe, s * 0.85 * breathe, OCTOPUS));
        for (dx, dy, r) in [(-0.3, -1.1, 0.13), (0.25, -1.35, 0.1), (0.4, -0.85, 0.08), (-0.1, -1.55, 0.07)] {
            out.push(Shape::circle_filled(mantle + vec2(side * dx * s, (dy + 0.8) * s), r * s, OCTOPUS_LIGHT));
        }
        out.push(oval(b, vec2(1.0, 0.0), s * 0.85, s * 0.62, OCTOPUS));
        for dx in [-0.36, 0.36] {
            let eye = b + vec2(dx * s, -s * 0.12);
            let look = (self.tip - eye).normalized() * s * 0.08;
            out.push(Shape::circle_filled(eye, s * 0.24, WHITE));
            out.push(Shape::circle_filled(eye + look, s * 0.12, EYE));
            out.push(Shape::circle_filled(eye + look + vec2(-s * 0.04, -s * 0.04), s * 0.035, WHITE));
        }
        let mouth = b + vec2(0.0, s * 0.3);
        out.push(Shape::line_segment([mouth + vec2(-s * 0.12, -s * 0.03), mouth], Stroke::new(s * 0.05, OCTOPUS_DARK)));
        out.push(Shape::line_segment([mouth, mouth + vec2(s * 0.12, -s * 0.03)], Stroke::new(s * 0.05, OCTOPUS_DARK)));
        out
    }

    /// The arm that carries the title, from the page's edge over the page to its tip, curling
    /// under the title's end, with suckers along its underside: in front of the page.
    pub fn arm(&self, time: f64) -> Vec<Shape> {
        let (s, side, t) = (self.size, self.side, time as f32);
        let reach = self.base.distance(self.tip);
        if reach < s * 0.2 {
            return Vec::new();
        }
        let mid = self.base + (self.tip - self.base) * 0.5;
        let bow = mid + vec2((t * 2.1).sin() * s * 0.12, -reach * 0.3 - s * 0.3);
        let n = 14;
        let mut spine: Vec<Pos2> = (0..=n).map(|j| bend(self.base, bow, self.tip, j as f32 / n as f32)).collect();
        spine.extend([self.tip + vec2(side * s * 0.14, s * 0.1), self.tip + vec2(side * s * 0.04, s * 0.24), self.tip + vec2(-side * s * 0.08, s * 0.18)]);
        let last = spine.len() - 1;
        let widths: Vec<f32> = (0..=last).map(|j| s * (0.34 - 0.26 * j as f32 / last as f32)).collect();
        let mut out = ribbon(&spine, &widths, OCTOPUS);
        for j in (2..n - 1).step_by(2) {
            let along = (spine[j + 1] - spine[j - 1]).normalized();
            let under = if along.rot90().y > 0.0 { along.rot90() } else { -along.rot90() };
            out.push(Shape::circle_filled(spine[j] + under * widths[j] * 0.28, widths[j] * 0.2, SUCKER));
        }
        out
    }
}

impl App {
    /// The reef on the desk, behind the pages lying at `page`, in the Magic Reef only: a coral for
    /// each chapter, a branch for each page it has had (`Doc::garden`), spawning when the next
    /// chapter begins, and fish about each coral whose chapter is done.
    pub fn draw_reef(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.reef = None;
        if self.themes.theme != Theme::Reef {
            (self.eggs.reef_for, self.eggs.spawn) = (None, None);
            return;
        }
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        // The newest coral grows, not the last chapter's: a title taken away must not make a coral
        // that has spawned grow again.
        let garden = &self.doc.garden;
        let n = garden.len().max(1);
        let newest = n - 1;
        let target = garden.get(newest).map_or(0, |&p| (p as usize).min(BRANCHES)) as f32;
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        let e = &mut self.eggs;
        if e.reef_for.as_ref().is_none_or(|(id, _)| *id != self.story_id) {
            // A story just opened, or the theme just picked, comes with its reef grown.
            (e.reef_for, e.branched, e.spawn) = (Some((self.story_id.clone(), newest)), target, None);
        } else if e.reef_for.as_ref().is_some_and(|(_, was)| *was < newest) {
            // A new chapter: the coral of the one before spawns, and the next is begun.
            let was = e.reef_for.as_ref().map_or(0, |f| f.1);
            (e.reef_for, e.branched, e.spawn) = (Some((self.story_id.clone(), newest)), 0.0, Some((now, was)));
        }
        if e.spawn.is_some_and(|(at, _)| now - at >= SPAWN as f64) {
            e.spawn = None;
        }
        let dt = (now - e.branched_at).clamp(0.0, 0.1) as f32;
        e.branched_at = now;
        if e.branched < target {
            // A branch at a time, quicker when many are waiting, like the city's floors.
            e.branched = (e.branched + (target - e.branched).max(3.0) / 3.0 / GROW * dt).min(target);
        }
        if e.spawn.is_some() || e.branched < target {
            ctx.request_repaint();
        } else {
            // The water moves, and the fish swim.
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }

        let u = e.unit(&ctx, area, now);
        let ground = area.bottom() - hidden - 10.0 * u;
        let story = &self.story_id;
        let painter = ui.painter();
        painter.extend(water(story, area, page, ground, u, now));
        let spawning = e.spawn.map(|(at, k)| (k, (now - at) as f32 / SPAWN));
        let mut seen = Seen::default();
        let mut shoals = Vec::new();
        for k in 0..n {
            let (at, size) = foot(k, n, area, page, ground, u);
            let coral = Coral::of(story, k, at, size);
            let grown = if k == newest { e.branched } else { garden.get(k).map_or(0, |&p| (p as usize).min(BRANCHES)) as f32 };
            let open = match spawning {
                Some((m, q)) if m == k => ease(q / 0.4),
                _ if k == newest => 0.0,
                _ => 1.0,
            };
            painter.extend(coral.draw(grown, open, now));
            let home = coral.bounds(grown);
            seen.corals.push((home, grown.floor() as usize, open > 0.0));
            if k != newest && grown > 0.0 {
                shoals.push((k, home, coral, grown, size));
            }
        }
        painter.extend(bubbles(story, area, page, ground, u, now));
        for (k, home, coral, grown, size) in shoals {
            let pages = garden.get(k).map_or(1, |&p| p as usize);
            let kind = FISH[Seeds::of(story, 51_000 + k).next(0.0, FISH.len() as f32) as usize % FISH.len()];
            let arriving = spawning.filter(|&(m, _)| m == k).map(|(_, q)| ease((q - 0.35) / 0.65));
            for i in 0..SHOAL.min(1 + pages / 2) {
                let (mut at, mut face) = fish_at(story, k, i, home, size, now);
                if let Some(a) = arriving {
                    if a <= 0.0 {
                        continue;
                    }
                    // Swimming in from beyond the desk on the coral's side, a little behind each other.
                    let from = pos2(if home.center().x < page.center().x { area.left() - 40.0 * u } else { area.right() + 40.0 * u }, at.y - 50.0 * u);
                    let a = ease((a - i as f32 * 0.06) / (1.0 - SHOAL as f32 * 0.06));
                    // Arrived, it turns into its way round the coral as it slows.
                    let coming = (at.x - from.x).signum();
                    face = coming + (face - coming) * ease((a - 0.75) / 0.25);
                    at = bend(from, pos2((from.x + at.x) / 2.0, at.y - 20.0 * u), at, a);
                }
                painter.extend(fish(at, face, size * 0.95, kind, i, now));
                seen.fish.push(at);
            }
            if let Some((m, q)) = spawning.filter(|&(m, _)| m == k) {
                // The eggs rise from its tips to the surface, drifting and fading.
                let tips = coral.tips(grown);
                for j in 0..CLOUD {
                    let mut s = Seeds::of(story, 52_000 + m * 100 + j);
                    let from = tips[j % tips.len()];
                    let r = ((q - s.next(0.0, 0.25)) / 0.75).clamp(0.0, 1.0);
                    if r <= 0.0 || r >= 1.0 {
                        continue;
                    }
                    let y = from.y - ease(r) * (from.y - area.top() + 20.0 * u) * s.next(0.6, 1.0);
                    let x = from.x + (r * 6.0 + s.next(0.0, 6.0)).sin() * 14.0 * u * r;
                    let (color, fade) = (EGGS[j % EGGS.len()], 1.0 - r * r);
                    painter.add(radial(pos2(x, y), 7.0 * size, 7.0 * size, color, 0.45 * fade));
                    painter.add(Shape::circle_filled(pos2(x, y), s.next(1.8, 2.8) * size, glow(color, 0.95 * fade)));
                    seen.spawn += 1;
                }
            }
        }
        self.eggs.reef = Some(seen);
    }
}

/// The reef drawn last frame, for tests: each coral, around what is shown of it, with how many
/// whole branches and whether its polyps are open; each fish; and how many eggs are rising.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub corals: Vec<(Rect, usize, bool)>,
    pub fish: Vec<Pos2>,
    pub spawn: usize,
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

    #[test]
    fn the_sea_is_there_from_the_start_the_same_for_the_same_story() {
        let shapes = water("a story", desk(), page(), 880.0, 1.0, 0.0);
        assert!(shapes.len() > 100, "rays, seagrass, sand, rocks: {}", shapes.len());
        assert_eq!(shapes.len(), water("a story", desk(), page(), 880.0, 1.0, 0.0).len());
    }

    #[test]
    fn a_coral_grows_a_branch_a_page_only_at_its_tips() {
        for k in 0..3 {
            let coral = Coral::of("a story", k, pos2(300.0, 880.0), 1.0);
            let mut was = coral.bounds(0.0);
            for grown in 1..=BRANCHES {
                let now = coral.bounds(grown as f32);
                assert!(now.contains_rect(was), "{:?} {grown}: what was there stays", coral.kind);
                assert!(now.bottom() <= 880.0 + 14.0 && now.top() > 880.0 - 260.0, "{:?} {grown}: on the floor: {now:?}", coral.kind);
                was = now;
            }
            assert!(was.height() > 50.0, "{:?}: whole, it stands up from the floor: {was:?}", coral.kind);
            let a = Coral::of("a story", k, pos2(300.0, 880.0), 1.0).draw(5.0, 0.0, 0.0).len();
            assert_eq!(a, coral.draw(5.0, 0.0, 0.0).len(), "the same for the same story");
        }
        let kinds: Vec<Kind> = (0..3).map(|k| Coral::of("a story", k, Pos2::ZERO, 1.0).kind).collect();
        assert!(kinds.contains(&Kind::Branching) && kinds.contains(&Kind::Fan) && kinds.contains(&Kind::Tubes), "each of its own kind");
    }

    #[test]
    fn the_corals_take_turns_on_either_side_of_the_page_from_its_edge_outwards() {
        let feet: Vec<f32> = (0..6).map(|k| foot(k, 6, desk(), page(), 880.0, 1.0).0.x).collect();
        for (k, x) in feet.iter().enumerate() {
            assert_eq!(*x < page().left(), k % 2 == 0, "{k}: {x}");
            assert!(desk().x_range().contains(*x) && !page().x_range().contains(*x), "{k}: on the desk beside the page: {x}");
        }
        assert!(feet[2] < feet[0] && feet[3] > feet[1], "outwards: {feet:?}");
    }

    #[test]
    fn the_octopus_comes_out_from_behind_the_page_sets_the_title_in_its_place_and_goes_back() {
        let place = Rect::from_min_size(pos2(620.0, 200.0), vec2(160.0, 24.0));
        for right in [true, false] {
            let at = |t: f32| octopus_at(right, pose(t, None), place, page());
            let behind = |o: &Octopus| page().x_range().contains(o.body.x - o.size * 0.9) && page().x_range().contains(o.body.x + o.size * 0.9);
            assert!(behind(&at(0.0)) && at(0.0).shown == 0.0, "{right}: hidden behind the page, the title not yet shown");
            let out = at(ENTER);
            assert!(!page().x_range().contains(out.body.x) && out.shown == 1.0, "{right}: out past the edge, the title shown: {out:?}");
            assert!(out.offset.y < -20.0, "{right}: the title held up: {out:?}");
            let set = at(ENTER + RUN + LOWER + SETTLE * 0.99);
            assert!(set.offset.length() < 3.0, "{right}: in its place: {set:?}");
            assert!((set.tip.x - place.center().x).abs() > place.width() * 0.45, "{right}: held by its end: {set:?}");
            let gone = at(COURSE + 0.1);
            assert!(behind(&gone) && gone.offset == Vec2::ZERO && gone.tip == gone.base, "{right}: back behind the page, its arm drawn in: {gone:?}");
            for k in 0..300 {
                let (a, b) = (at(k as f32 / 100.0), at((k + 1) as f32 / 100.0));
                assert!((a.offset - b.offset).length() < 12.0 && a.tip.distance(b.tip) < 12.0, "{right}: smoothly at {k}: {a:?} {b:?}");
            }
        }
    }

    #[test]
    fn the_fish_swim_round_their_coral_gently() {
        let home = Rect::from_min_max(pos2(200.0, 700.0), pos2(320.0, 880.0));
        let (mut sides, mut faced) = ((0, 0), (0, 0));
        for f in 0..(60 * 30) {
            let t = f as f64 / 60.0;
            let (at, face) = fish_at("a story", 0, 0, home, 1.0, t);
            let (next, then) = fish_at("a story", 0, 0, home, 1.0, t + 1.0 / 60.0);
            assert!(home.expand(80.0).contains(at), "about its coral: {at:?}");
            assert!(at.distance(next) < 2.0, "gently");
            assert!((face - then).abs() < 0.05, "turning gently, never at a stroke: {face} then {then}");
            faced = (faced.0 + usize::from(face > 0.99), faced.1 + usize::from(face < -0.99));
            sides = (sides.0 + usize::from(at.x < home.center().x), sides.1 + usize::from(at.x > home.center().x));
        }
        assert!(sides.0 > 300 && sides.1 > 300, "round it: {sides:?}");
        assert!(faced.0 > 300 && faced.1 > 300, "facing either way most of the time: {faced:?}");
    }
}
