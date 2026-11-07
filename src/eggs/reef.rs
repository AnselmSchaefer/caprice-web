//! The Magic Reef. A sunlit shallow sea lies on the desk: light falls in slanting rays from the
//! surface and sways with it, seagrass waves along a sandy floor with rocks and the reef fading
//! into the blue behind, and bubbles rise. Each chapter has a coral on the floor beside the page,
//! taking turns left and right, then in rows further back, smaller and paler in the water, once
//! the row in front is full; each of its own kind (a branching coral, a sea fan, tube sponges)
//! and colour; each page of the chapter being written grows it a branch, out of a little glimmer.
//! When the next chapter begins, the coral of the one before spawns: a cloud of eggs rises from its
//! tips to the surface, its polyps open, and a little shoal of fish swims in to live about it.
//! A new chapter title is brought by an octopus, which swims down from above with the title
//! hanging below it in two of its arms, sets it in its place, lets go and jets back up.
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
pub const GRASS: [Color32; 2] = [Color32::from_rgb(64, 150, 100), Color32::from_rgb(92, 170, 96)];
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
pub const EYE: Color32 = Color32::from_rgb(20, 24, 32);
/// The water that pales what lies further back.
const HAZE: Color32 = Color32::from_rgb(64, 150, 182);
/// The octopus: its skin, darker and lighter, its suckers and the whites of its eyes.
pub const OCTOPUS: Color32 = Color32::from_rgb(226, 104, 92);
const OCTOPUS_DARK: Color32 = Color32::from_rgb(190, 76, 70);
const OCTOPUS_LIGHT: Color32 = Color32::from_rgb(246, 150, 128);
const SUCKER: Color32 = Color32::from_rgb(255, 214, 200);
const WHITE: Color32 = Color32::from_rgb(255, 250, 240);

/// The rows of the reef, from the front back: how far up the sand each stands, in units, and how
/// much the water pales it; and how far up the sand reaches, and how far from the page the places
/// reach whatever the window.
const ROWS: [(f32, f32); 5] = [(0.0, 0.0), (34.0, 0.25), (58.0, 0.42), (74.0, 0.55), (86.0, 0.65)];
const FLOOR: f32 = 98.0;
const REACH: f32 = 1800.0;

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

    /// The coral further back, paled `haze` of the way into the water.
    pub fn hazed(self, haze: f32) -> Coral {
        Coral { body: self.body.lerp_to_gamma(HAZE, haze), tip: self.tip.lerp_to_gamma(HAZE, haze), ..self }
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

/// A place on the reef a coral can stand: its foot, its size, how much room it has across, and
/// its row from the front.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub foot: Pos2,
    pub size: f32,
    pub w: f32,
    pub row: usize,
}

impl Place {
    /// Whether the coral there is wholly on the desk.
    pub fn in_view(&self, area: Rect) -> bool {
        self.foot.x - self.w / 2.0 >= area.left() && self.foot.x + self.w / 2.0 <= area.right()
    }
}

/// The places on the reef, in the order corals take them: row by row from the front back, each
/// further up the sand, smaller; in each, taking turns left and right of the page from its edge
/// outwards, every other row shifted by half a place so it does not line up with the one in front. They reach as far from the page
/// whatever the window, so a place never moves when it changes; those beyond the desk are out of
/// sight, like the Magic Sea's places in the bay.
pub fn places(page: Rect, ground: f32, u: f32) -> Vec<Place> {
    let mut out = Vec::new();
    for (row, &(up, _)) in ROWS.iter().enumerate() {
        let s = 0.75f32.powi(row as i32);
        let (w, y) = (150.0 * u * s, ground - up * u);
        let shift = if row % 2 == 1 { w / 2.0 } else { 0.0 };
        let mut j = 0.0;
        while 30.0 * u + shift + w * (j + 1.0) <= REACH * u {
            let d = 30.0 * u + shift + w * (j + 0.5);
            for x in [page.left() - d, page.right() + d] {
                out.push(Place { foot: pos2(x, y), size: 1.4 * u * s, w, row });
            }
            j += 1.0;
        }
    }
    out
}

/// The place for a new coral when `taken` are: the first free one in sight, else the first free.
pub fn bed(taken: &[Option<usize>], places: &[Place], area: Rect) -> usize {
    let free = |p: &usize| !taken.contains(&Some(*p));
    (0..places.len()).filter(free).find(|&p| places[p].in_view(area)).or_else(|| (0..places.len()).find(free)).unwrap_or(0)
}

/// The sea behind the reef: rays of sunlight swaying down from the surface, the reef fading into the
/// blue far behind, and the sand reaching back into it.
fn water(story: &str, area: Rect, ground: f32, u: f32, time: f64) -> Vec<Shape> {
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
    // The reef far behind, in the blue, beyond the back of the sand.
    let back = ground - FLOOR * u;
    let mut x = area.left() - 60.0 * u;
    let mut s = Seeds::of(story, 61_000);
    while x < area.right() + 60.0 * u {
        let w = s.next(50.0, 110.0) * u;
        out.push(oval(pos2(x, back), vec2(1.0, 0.0), w, s.next(18.0, 42.0) * u, glow(DISTANT, 0.45)));
        x += w * s.next(0.9, 1.3);
    }
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), back), pos2(area.right(), area.bottom())), 0.0, glow(DISTANT, 0.45)));
    // The sand, reaching back into the blue, rolling a little, with pebbles and rocks on it.
    let mut s = Seeds::of(story, 63_000);
    let far = SAND.lerp_to_gamma(HAZE, 0.7);
    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(pos2(area.left(), back), far);
    mesh.colored_vertex(pos2(area.right(), back), far);
    mesh.colored_vertex(pos2(area.right(), ground), SAND);
    mesh.colored_vertex(pos2(area.left(), ground), SAND);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    out.push(Shape::mesh(mesh));
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), ground), pos2(area.right(), area.bottom())), 0.0, SAND));
    let mut x = area.left() - 40.0 * u;
    while x < area.right() + 40.0 * u {
        let w = s.next(70.0, 140.0) * u;
        out.push(oval(pos2(x, ground + 2.0 * u), vec2(1.0, 0.0), w, s.next(5.0, 10.0) * u, SAND));
        out.push(oval(pos2(x + w * 0.4, ground + s.next(8.0, 30.0) * u), vec2(1.0, 0.0), s.next(2.0, 4.0) * u, 1.5 * u, SAND_DARK));
        x += w;
    }
    out
}

/// What lies in front of every coral, nearest the writer: rocks on the sand and seagrass, in clumps
/// beside the page, swaying with the water.
fn front(story: &str, page: Rect, ground: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let mut out = Vec::new();
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
fn fish(at: Pos2, face: f32, u: f32, (color, stripe): (Color32, Color32), haze: f32, k: usize, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let (color, stripe) = (color.lerp_to_gamma(HAZE, haze), stripe.lerp_to_gamma(HAZE, haze));
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

/// The octopus bringing a title lying at `place` on a page lying at `page`: its head, its size,
/// where the two arms holding the title have their tips and how much they hold it, and how far the
/// title is from its place.
#[derive(Clone, Copy, Debug)]
pub struct Octopus {
    pub body: Pos2,
    pub size: f32,
    pub grips: [Pos2; 2],
    pub holding: f32,
    pub offset: Vec2,
    t: f32,
}

/// Where the octopus is at `p`, drifting a little in from the right or the left. It swims down
/// from beyond the top of the `desk` with the title hanging below it, held at its top by two arms,
/// over the title's place, lowers it in, lets go and jets back up out of sight.
pub fn octopus_at(right: bool, p: Pose, place: Rect, page: Rect, desk: Rect) -> Octopus {
    let side = if right { 1.0 } else { -1.0 };
    // A title is often high on its page: the octopus, about four sizes tall over what it holds,
    // holds it up only as high as there is room for below the top of the page, and is smaller there.
    let room = place.top() - (page.top() - page.height() * 0.03);
    let s = (room / 4.2).clamp(page.width() * 0.045, page.width() * 0.08);
    let lift = (room - 4.2 * s).clamp(0.0, 1.4 * s);
    let top = if desk.is_positive() { desk.top() } else { page.top() };
    let away = 1.0 - p.enter;
    // How far the head is above the top of what it holds, and the arms' hold on it.
    let (arms, hold) = (2.0 * s, (place.width() * 0.38).clamp(s * 0.4, s * 1.8));
    // Gone, the title and all of the octopus above it are above the desk.
    let gone = top - place.height() / 2.0 - s * 0.5 - place.center().y;
    let (body, offset) = if p.hooked {
        let bob = (p.t * 2.4).sin() * s * 0.12 * (1.0 - p.lower);
        let offset = vec2(side * page.width() * 0.12 * (1.0 - p.run), -lift * (1.0 - p.lower) + bob + gone * away);
        (place.center_top() + offset - vec2(0.0, arms), offset)
    } else {
        // Let go, it rises clear of the title, then jets away up.
        let up = place.center_top() - vec2(0.0, arms + s * 1.2 * p.hook);
        let out = top - s * 2.6 - place.height();
        (pos2(up.x, up.y + (out - up.y) * away), Vec2::ZERO)
    };
    let holding = if p.hooked { 1.0 } else { 1.0 - ease(p.hook) };
    let on = place.center_top() + offset + vec2(0.0, 1.0);
    let grips = [-1.0f32, 1.0].map(|k| {
        let held = on + vec2(k * hold, 0.0);
        let rest = body + vec2(k * s * 0.7, s * 1.7);
        rest + (held - rest) * holding
    });
    Octopus { body, size: s, grips, holding, offset, t: p.t }
}

impl Octopus {
    /// The octopus over the page: six arms waving below it, its mantle swelling as it swims, its
    /// head and eyes looking down at what it holds; then the two arms holding the title, their tips
    /// curled over its top.
    pub fn draw(&self) -> Vec<Shape> {
        let (s, b, t) = (self.size, self.body, self.t);
        let mut out = Vec::new();
        for a in 0..6 {
            let spread = (a as f32 - 2.5) / 2.5;
            let root = b + vec2(spread * s * 0.45, s * 0.45);
            let spine: Vec<Pos2> = (0..=8).map(|j| {
                let f = j as f32 / 8.0;
                let wave = (t * 2.0 + a as f32 * 1.3 + f * 3.0).sin() * s * 0.3 * f;
                root + vec2(spread * s * 1.1 * f + wave, s * 1.7 * f - s * 0.45 * f * f * f)
            }).collect();
            let widths: Vec<f32> = (0..=8).map(|j| s * 0.3 * (1.0 - j as f32 / 8.0 * 0.85)).collect();
            out.extend(ribbon(&spine, &widths, if a % 2 == 0 { OCTOPUS_DARK } else { OCTOPUS }));
        }
        // It swims by pulses, its mantle filling and pushing out.
        let pulse = 1.0 + (t * 2.4).sin() * 0.06;
        let mantle = b + vec2(0.0, -s * 0.8);
        out.push(oval(mantle, vec2(0.0, -1.0), s * 1.15 * pulse, s * 0.85 / pulse, OCTOPUS));
        for (dx, dy, r) in [(-0.3, -1.1, 0.13), (0.25, -1.35, 0.1), (0.4, -0.85, 0.08), (-0.1, -1.55, 0.07)] {
            out.push(Shape::circle_filled(mantle + vec2(dx * s, (dy + 0.8) * s), r * s, OCTOPUS_LIGHT));
        }
        out.push(oval(b, vec2(1.0, 0.0), s * 0.85, s * 0.62, OCTOPUS));
        let title = (self.grips[0] + self.grips[1].to_vec2()) / 2.0;
        for dx in [-0.36, 0.36] {
            let eye = b + vec2(dx * s, -s * 0.12);
            let look = (title - eye).normalized() * s * 0.08;
            out.push(Shape::circle_filled(eye, s * 0.24, WHITE));
            out.push(Shape::circle_filled(eye + look, s * 0.12, EYE));
            out.push(Shape::circle_filled(eye + look + vec2(-s * 0.04, -s * 0.04), s * 0.035, WHITE));
        }
        let mouth = b + vec2(0.0, s * 0.3);
        out.push(Shape::line_segment([mouth + vec2(-s * 0.12, -s * 0.03), mouth], Stroke::new(s * 0.05, OCTOPUS_DARK)));
        out.push(Shape::line_segment([mouth, mouth + vec2(s * 0.12, -s * 0.03)], Stroke::new(s * 0.05, OCTOPUS_DARK)));
        for (k, &tip) in self.grips.iter().enumerate() {
            let sign = if k == 0 { -1.0 } else { 1.0 };
            let root = b + vec2(sign * s * 0.3, s * 0.45);
            let bow = root + vec2(sign * s * 0.5, (tip.y - root.y) * 0.5);
            let n = 10;
            let mut spine: Vec<Pos2> = (0..=n).map(|j| bend(root, bow, tip, j as f32 / n as f32)).collect();
            // Curled over the title's top edge while it holds it.
            let c = s * 0.16 * self.holding;
            spine.extend([tip + vec2(-sign * c, c), tip + vec2(-sign * c * 1.6, c * 0.3)]);
            let last = spine.len() - 1;
            let widths: Vec<f32> = (0..=last).map(|j| s * (0.3 - 0.22 * j as f32 / last as f32)).collect();
            out.extend(ribbon(&spine, &widths, OCTOPUS));
            for j in (2..n - 1).step_by(2) {
                let along = (spine[j + 1] - spine[j - 1]).normalized();
                let inner = along.rot90() * sign;
                out.push(Shape::circle_filled(spine[j] + inner * widths[j] * 0.28, widths[j] * 0.2, SUCKER));
            }
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
            e.beds.clear();
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
        painter.extend(water(story, area, ground, u, now));
        let spawning = e.spawn.map(|(at, k)| (k, (now - at) as f32 / SPAWN));
        let places = places(page, ground, u);
        e.beds.resize(e.beds.len().max(n), None);
        for k in 0..n {
            if e.beds[k].is_none() {
                e.beds[k] = Some(bed(&e.beds, &places, area));
            }
        }
        let mut seen = Seen { corals: vec![(Rect::NOTHING, 0, false, 0); n], ..Seen::default() };
        // The rows further back first, each coral with its fish, so the nearer ones come in front.
        let mut order: Vec<(usize, Place)> = (0..n).map(|k| (k, places[e.beds[k].unwrap_or(0).min(places.len() - 1)])).collect();
        order.sort_by_key(|(k, p)| (std::cmp::Reverse(p.row), *k));
        let mut clouds = Vec::new();
        for (k, place) in order {
            let haze = ROWS[place.row].1;
            let coral = Coral::of(story, k, place.foot, place.size).hazed(haze);
            let grown = if k == newest { e.branched } else { garden.get(k).map_or(0, |&p| (p as usize).min(BRANCHES)) as f32 };
            let open = match spawning {
                Some((m, q)) if m == k => ease(q / 0.4),
                _ if k == newest => 0.0,
                _ => 1.0,
            };
            painter.extend(coral.draw(grown, open, now));
            let home = coral.bounds(grown);
            seen.corals[k] = (home, grown.floor() as usize, open > 0.0, place.row);
            if k == newest || grown <= 0.0 {
                continue;
            }
            let size = place.size;
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
                painter.extend(fish(at, face, size * 0.95, kind, haze, i, now));
                seen.fish.push(at);
            }
            if let Some((_, q)) = spawning.filter(|&(m, _)| m == k) {
                clouds.push((k, coral.tips(grown), size, q));
            }
        }
        // The rocks and seagrass nearest the writer, in front of every coral and fish.
        painter.extend(front(story, page, ground, u, now));
        painter.extend(bubbles(story, area, page, ground, u, now));
        for (m, tips, size, q) in clouds {
            // The eggs rise from its tips to the surface, drifting and fading, in front of the reef.
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
        self.eggs.reef = Some(seen);
    }
}

/// The reef drawn last frame, for tests: each chapter's coral, around what is shown of it, with how
/// many whole branches, whether its polyps are open, and its row from the front; each fish; and
/// how many eggs are rising.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub corals: Vec<(Rect, usize, bool, usize)>,
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
        let shapes = |story: &str| [water(story, desk(), 880.0, 1.0, 0.0), front(story, page(), 880.0, 1.0, 0.0)].concat();
        assert!(shapes("a story").len() > 100, "rays, sand, rocks, seagrass: {}", shapes("a story").len());
        assert_eq!(shapes("a story").len(), shapes("a story").len());
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
    fn the_corals_fill_the_front_row_beside_the_page_then_rows_further_back_smaller() {
        let all = places(page(), 880.0, 1.0);
        let mut taken = Vec::new();
        for _ in 0..20 {
            taken.push(Some(bed(&taken, &all, desk())));
        }
        let got: Vec<Place> = taken.iter().flatten().map(|&p| all[p]).collect();
        assert!(got[0].row == 0 && got[0].foot.x < page().left() && got[1].foot.x > page().right(), "taking turns: {got:?}");
        assert!(got[2].foot.x < got[0].foot.x && got[3].foot.x > got[1].foot.x, "from the page outwards: {got:?}");
        let front = got.iter().take_while(|p| p.row == 0).count();
        assert!(front >= 4 && got[front].row == 1, "the front row first: {front}");
        for (k, p) in got.iter().enumerate() {
            assert!(p.in_view(desk()) && !page().x_range().contains(p.foot.x), "{k}: on the desk beside the page: {p:?}");
            assert!(k == 0 || p.row >= got[k - 1].row, "{k}: row by row from the front back");
            assert!(p.row == 0 || (p.foot.y < 880.0 - 20.0 && p.size < got[0].size), "{k}: further back, further up and smaller: {p:?}");
        }
    }

    #[test]
    fn the_octopus_swims_down_from_above_sets_the_title_in_its_place_and_jets_back_up() {
        let place = Rect::from_min_size(pos2(620.0, 400.0), vec2(160.0, 24.0));
        for right in [true, false] {
            let side = if right { 1.0 } else { -1.0 };
            let at = |t: f32| octopus_at(right, pose(t, None), place, page(), desk());
            let out_of_sight = |o: &Octopus| o.body.y + o.size * 2.0 < desk().top() && place.translate(o.offset).bottom() < desk().top();
            assert!(out_of_sight(&at(0.0)), "{right}: above the desk with the title: {:?}", at(0.0));
            let come = at(ENTER);
            assert!(come.offset.x * side > 20.0 && desk().contains(come.body), "{right}: down, a little to its side: {come:?}");
            let over = at(ENTER + RUN);
            assert!(over.offset.x.abs() < 1.0 && over.offset.y < -20.0, "{right}: over its place, holding it up: {over:?}");
            assert!(over.body.y < place.top() + over.offset.y, "{right}: above what it holds: {over:?}");
            let high = Rect::from_min_size(pos2(620.0, page().top() + 30.0), vec2(160.0, 24.0));
            let small = octopus_at(right, pose(ENTER + RUN, None), high, page(), desk());
            assert!(small.size < over.size && small.offset.y > over.offset.y, "{right}: high on the page, smaller and lower: {small:?}");
            let set = at(ENTER + RUN + LOWER + SETTLE * 0.99);
            assert!(set.offset.length() < 3.0, "{right}: in its place: {set:?}");
            assert!(set.grips.iter().all(|g| (g.y - place.top()).abs() < 3.0 && place.x_range().contains(g.x)), "{right}: holding its top: {set:?}");
            let gone = at(COURSE + 0.1);
            assert!(out_of_sight(&Octopus { offset: vec2(0.0, -1000.0), ..gone }) && gone.offset == Vec2::ZERO, "{right}: back up out of sight: {gone:?}");
            for k in 0..300 {
                let (a, b) = (at(k as f32 / 100.0), at((k + 1) as f32 / 100.0));
                assert!((a.offset - b.offset).length() < 25.0 && a.body.distance(b.body) < 25.0, "{right}: smoothly at {k}: {a:?} {b:?}");
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
