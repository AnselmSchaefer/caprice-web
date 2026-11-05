//! The Magic Forest. An old forest at night stands on the desk on either side of the page: great
//! trunks with their roots and branches, rows of thinner ones fading into the mist behind, leaves
//! overhead, and the moon. Each page of the chapter being written lights a lantern, which rises
//! from the forest floor and joins the others in a swarm, drifting over the desk and now and then
//! across behind the page. When a new chapter begins, the swarm of the one before flies to a
//! branch of one of the trees nearest the page and becomes one big lantern hanging there, its warm
//! light falling on the trunks about it; over a book the forest fills with them. An owl lives
//! there, sitting beside the newest big lantern; when the writer makes a new chapter title it flies
//! out with the title in its talons, sets it in its place on the page, and flies back to sit beside
//! the new lantern.
//!
//! Like everything in `eggs.rs` it lies on the desk behind the pages, the owl in flight over the
//! page on the screen only; it is drawn from shapes as a function of the story and of time;
//! nothing is printed or saved, and nothing new is kept: the lanterns are counted from
//! `Doc::garden`.

use super::*;

const TRUNKS: [Color32; 3] = [Color32::from_rgb(22, 28, 32), Color32::from_rgb(34, 54, 60), Color32::from_rgb(50, 78, 88)];
const CANOPY: Color32 = Color32::from_rgb(12, 28, 30);
/// Moonlight on the trunks, the moon, and the mist.
const MOON: Color32 = Color32::from_rgb(214, 236, 230);
const MIST: Color32 = Color32::from_rgb(170, 204, 210);
const FLOOR: Color32 = Color32::from_rgb(8, 24, 24);
/// Where a lantern's light falls on the bark.
const LIT_BARK: Color32 = Color32::from_rgb(112, 78, 48);
/// A lantern's paper, its flame, and the warm light it throws.
pub const PAPER_GLOW: Color32 = Color32::from_rgb(255, 200, 110);
const FLAME: Color32 = Color32::from_rgb(255, 244, 200);
const WARM: Color32 = Color32::from_rgb(255, 176, 80);
/// The owl, seen by lantern light.
const FEATHER: Color32 = Color32::from_rgb(150, 108, 70);
const FEATHER_LIGHT: Color32 = Color32::from_rgb(214, 176, 128);
const EYE: Color32 = Color32::from_rgb(255, 214, 90);
const DARK: Color32 = Color32::from_rgb(20, 14, 12);
const BEAK: Color32 = Color32::from_rgb(230, 186, 90);

/// How far up the desk each row of trunks behind stands, in units, and how far out from the page
/// the forest reaches.
const DEPTH: f32 = 30.0;
const REACH: f32 = 1500.0;
/// Seconds a new lantern takes to rise into the swarm, and a chapter's swarm to fly into the tree,
/// become its lantern, and light what is there.
pub const RISE: f32 = 2.0;
pub const MERGE: f32 = 3.5;
/// The most lanterns drawn in a swarm.
const LANTERNS: usize = 90;

/// One of the trees nearest the front: where it stands, how wide it is at its foot, how much it
/// leans from the floor up past the top of the window, and its branches, one above the other,
/// each from the trunk out to one side. The `i`th from the page on the left side or the right (`k`), at a distance
/// from the page and a size that come from the story, whatever the window.
#[derive(Clone, Copy, Debug)]
struct Trunk {
    x: f32,
    w: f32,
    lean: f32,
    branches: [(Pos2, Pos2); BRANCHES],
}

/// How many branches each of the trees nearest the front has, for big lanterns to hang from.
const BRANCHES: usize = 5;

fn near(story: &str, i: usize, k: usize, area: Rect, page: Rect, ground: f32, u: f32) -> Trunk {
    let mut seeds = Seeds::of(story, 3000 + i * 2 + k);
    let top = area.top() - 20.0;
    let d = (70.0 + (i as f32 + seeds.next(-0.3, 0.3)) * 210.0) * u;
    let x = if k == 0 { page.left() - d } else { page.right() + d };
    let w = seeds.next(34.0, 52.0) * u;
    let lean = seeds.next(-0.04, 0.04) * (ground - top);
    let first = if seeds.next(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
    let branches = std::array::from_fn(|b| {
        let y = ground - (0.32 + 0.11 * b as f32 + seeds.next(0.0, 0.05)) * (ground - area.top());
        let from = pos2(x + lean * (ground - y) / (ground - top), y);
        let out = if b % 2 == 0 { first } else { -first };
        (from, from + vec2(out * seeds.next(55.0, 105.0) * u, -seeds.next(15.0, 45.0) * u))
    });
    Trunk { x, w, lean, branches }
}

/// A branch a big lantern hangs from: where it leaves its trunk and where it ends, and how wide
/// the trunk is.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    from: Pos2,
    to: Pos2,
    w: f32,
}

impl Spot {
    /// A point along the branch, `along` of the way out.
    fn on(&self, along: f32) -> Pos2 {
        self.from + (self.to - self.from) * along
    }

    /// Where its lantern hangs from, near its end.
    pub fn hook(&self) -> Pos2 {
        self.on(0.78)
    }

    /// Which way the branch reaches.
    fn dir(&self) -> f32 {
        if self.to.x < self.from.x { -1.0 } else { 1.0 }
    }
}

/// The branches the chapters' big lanterns hang from, in turn: those nearest the page first,
/// taking turns left and right; never one whose lantern would hang behind the page, under the
/// window's top or low among the trunks, nor one too near a lantern before it.
pub fn spots(story: &str, area: Rect, page: Rect, ground: f32, u: f32) -> Vec<Spot> {
    let mut sides: [Vec<(f32, Spot)>; 2] = [Vec::new(), Vec::new()];
    for i in 0..(REACH / (210.0 * u)) as usize {
        for (k, side) in sides.iter_mut().enumerate() {
            let t = near(story, i, k, area, page, ground, u);
            for (from, to) in t.branches {
                let spot = Spot { from, to, w: t.w };
                let h = spot.hook();
                let away = if k == 0 { page.left() - 30.0 * u - h.x } else { h.x - page.right() - 30.0 * u };
                if away > 0.0 && h.y > area.top() + 90.0 * u && h.y < ground - 160.0 * u {
                    side.push((away, spot));
                }
            }
        }
    }
    for side in &mut sides {
        side.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let mut out: Vec<Spot> = Vec::new();
    let (mut l, mut r) = (sides[0].iter(), sides[1].iter());
    loop {
        let (a, b) = (l.next(), r.next());
        if a.is_none() && b.is_none() {
            return out;
        }
        for (_, spot) in a.into_iter().chain(b) {
            if out.iter().all(|o| o.hook().distance(spot.hook()) > 46.0 * u) {
                out.push(*spot);
            }
        }
    }
}

/// The forest at night: the moon over the side of the page with more room, leaves overhead, the trunks row by
/// row from the back with mist between, their roots and branches, and the floor. Each trunk's
/// place and size come from the story, at a distance from the page whatever the window.
fn woods(story: &str, area: Rect, page: Rect, ground: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let mut out = Vec::new();
    let left = page.left() - area.left() >= area.right() - page.right();
    let (room, edge, side) = if left { (page.left() - area.left(), page.left(), -1.0) } else { (area.right() - page.right(), page.right(), 1.0) };
    if room > 120.0 * u {
        let moon = pos2(edge + side * (room * 0.5).min(260.0 * u), area.top() + 130.0 * u);
        for (r, a) in [(90.0, 0.03), (60.0, 0.05), (42.0, 0.08)] {
            out.push(Shape::circle_filled(moon, r * u, glow(MOON, a)));
        }
        out.push(Shape::circle_filled(moon, 30.0 * u, MOON));
        out.push(Shape::circle_filled(moon + vec2(-8.0, -5.0) * u, 7.0 * u, glow(MIST, 0.5)));
        out.push(Shape::circle_filled(moon + vec2(9.0, 8.0) * u, 4.5 * u, glow(MIST, 0.45)));
    }
    let top = area.top() - 20.0;
    for layer in (0..3).rev() {
        let (spacing, widths, offset) = [(210.0, (34.0, 52.0), 70.0), (95.0, (16.0, 26.0), 40.0), (40.0, (6.0, 12.0), 15.0)][layer];
        let foot_y = ground - layer as f32 * DEPTH * u;
        for i in 0..(REACH / (spacing * u)) as usize {
            for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                let color = TRUNKS[layer];
                if layer == 0 {
                    // The trees nearest the front, whose branches the big lanterns hang from.
                    let n = near(story, i, k, area, page, ground, u);
                    let (x, w, lean) = (n.x, n.w, n.lean);
                    out.push(solid(vec![pos2(x - w / 2.0, foot_y), pos2(x + w / 2.0, foot_y), pos2(x + lean + w * 0.36, top), pos2(x + lean - w * 0.36, top)], color));
                    // Moonlight down one side, roots spilling over the ground, and a branch or two.
                    out.push(solid(vec![pos2(x - w / 2.0, foot_y), pos2(x - w * 0.36, foot_y), pos2(x + lean - w * 0.26, top), pos2(x + lean - w * 0.36, top)], glow(MOON, 0.1)));
                    for (dx, reach) in [(-0.5, -1.0), (0.5, 1.0), (-0.2, -0.55), (0.25, 0.6)] {
                        let at = pos2(x + dx * w * 0.8, foot_y - w * 0.45);
                        out.push(solid(vec![at, pos2(x + reach * w, foot_y + 2.0 * u), pos2(x + dx * w * 0.2, foot_y + 2.0 * u)], color));
                    }
                    for (from, to) in n.branches {
                        out.push(Shape::line_segment([from, to], Stroke::new(w * 0.14, color)));
                    }
                    continue;
                }
                let mut seeds = Seeds::of(story, 3000 + layer * 1000 + i * 2 + k);
                let d = (offset + (i as f32 + seeds.next(-0.3, 0.3)) * spacing) * u;
                let x = if side < 0.0 { page.left() - d } else { page.right() + d };
                let w = seeds.next(widths.0, widths.1) * u;
                let lean = seeds.next(-0.04, 0.04) * (foot_y - top);
                out.push(solid(vec![pos2(x - w / 2.0, foot_y), pos2(x + w / 2.0, foot_y), pos2(x + lean + w * 0.36, top), pos2(x + lean - w * 0.36, top)], color));
            }
        }
        // Mist drifting between the rows.
        for m in 0..6 {
            let mut seeds = Seeds::of(story, 2000 + layer * 10 + m);
            let span = area.width() + 400.0 * u;
            let x = area.left() - 200.0 * u + (seeds.next(0.0, span) + t * seeds.next(4.0, 10.0) * u).rem_euclid(span);
            let y = foot_y - seeds.next(10.0, 70.0) * u;
            out.push(oval(pos2(x, y), vec2(1.0, 0.0), seeds.next(120.0, 220.0) * u, seeds.next(10.0, 18.0) * u, glow(MIST, 0.05)));
        }
    }
    // Leaves overhead, along the top of the window on both sides.
    for i in 0..(REACH / (60.0 * u)) as usize {
        for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let mut seeds = Seeds::of(story, 1000 + i * 2 + k);
            let d = (i as f32 * 60.0 + seeds.next(-20.0, 20.0)) * u;
            let x = if side < 0.0 { page.left() - d } else { page.right() + d };
            out.push(Shape::circle_filled(pos2(x, area.top() + seeds.next(-30.0, 40.0) * u), seeds.next(40.0, 80.0) * u, CANOPY));
        }
    }
    let floor = ground - 3.0 * DEPTH * u - 20.0 * u;
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), floor), pos2(area.right(), area.bottom())), 0.0, glow(FLOOR, 0.45)));
    out
}


/// Where the swarm is at `time`: over the desk beside the page, most of the time on one side or the
/// other, crossing behind the page now and then, and wandering a little where it is.
fn swarm(area: Rect, page: Rect, u: f32, time: f32) -> Pos2 {
    let a = 2.0 * PI / 90.0;
    let room = ((page.left() - area.left()).min(area.right() - page.right()) * 0.5).clamp(80.0 * u, 260.0 * u);
    let x = page.center().x + (3.0 * (a * time).sin()).tanh() / 3f32.tanh() * (page.width() / 2.0 + room);
    let y = area.top() + area.height() * 0.34 + (1.9 * a * time + 1.0).sin() * 50.0 * u;
    pos2(x + (2.3 * a * time).sin() * 30.0 * u, y)
}

/// Where lantern `i` of chapter `chapter`'s swarm of `n` is at `time`: about the swarm's middle,
/// each going round it its own way and a little behind it, so the swarm trails as it moves; the
/// more lanterns, the wider it is.
#[allow(clippy::too_many_arguments)]
pub fn lantern_at(story: &str, chapter: usize, i: usize, n: usize, area: Rect, page: Rect, u: f32, time: f64) -> Pos2 {
    let mut seeds = Seeds::of(story, 20_000 + chapter * 1000 + i);
    let t = time as f32;
    let middle = swarm(area, page, u, t - seeds.next(0.0, 3.0));
    let reach = (40.0 + 12.0 * (n as f32).sqrt()) * u;
    let r = seeds.next(0.15, 1.0).sqrt() * reach;
    let (start, turn) = (seeds.next(0.0, 2.0 * PI), seeds.next(0.08, 0.22) * if seeds.next(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 });
    let ang = start + turn * t;
    middle + vec2(ang.cos() * r, ang.sin() * r * 0.55 + (t * seeds.next(0.6, 1.2) + start).sin() * 4.0 * u)
}

/// Where a new lantern rises from: the forest floor beside the page, below the swarm's side.
fn lantern_from(story: &str, chapter: usize, i: usize, page: Rect, ground: f32, u: f32, near: Pos2) -> Pos2 {
    let mut seeds = Seeds::of(story, 30_000 + chapter * 1000 + i);
    let right = near.x > page.center().x;
    let d = seeds.next(30.0, 260.0) * u;
    pos2(if right { page.right() + d } else { page.left() - d }, ground - seeds.next(0.0, 40.0) * u)
}

/// A paper lantern at `at`, as bright as `light`, `u` its size, swaying a little: its glow, its
/// paper, its flame.
fn lantern(at: Pos2, light: f32, u: f32, k: usize, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let flicker = light * (0.88 + 0.12 * (t * 7.0 + k as f32 * 1.7).sin() * (t * 3.1 + k as f32).sin());
    let lean = (t * 0.9 + k as f32).sin() * 0.12;
    let up = vec2(lean, -1.0).normalized();
    let side = -up.rot90();
    let (w, h) = (5.0 * u, 8.0 * u);
    vec![
        radial(at, 18.0 * u, 18.0 * u, PAPER_GLOW, 0.22 * flicker),
        solid(vec![at - up * h * 0.5 - side * w * 0.7, at - up * h * 0.5 + side * w * 0.7, at + up * h * 0.5 + side * w, at + up * h * 0.5 - side * w], glow(PAPER_GLOW, 0.9 * flicker)),
        Shape::circle_filled(at - up * h * 0.38, 1.3 * u, glow(FLAME, flicker)),
    ]
}


/// Light about `at`, `r` across and `h` high, `alpha` at its middle and fading smoothly to
/// nothing at its edge: one mesh, so it shows no rings.
fn radial(at: Pos2, r: f32, h: f32, color: Color32, alpha: f32) -> Shape {
    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(at, glow(color, alpha));
    let rings = [(0.35, 0.55), (0.7, 0.18), (1.0, 0.0)];
    let n = 32;
    for &(f, a) in &rings {
        for i in 0..n {
            let ang = i as f32 / n as f32 * 2.0 * PI;
            mesh.colored_vertex(at + vec2(ang.cos() * r * f, ang.sin() * h * f), glow(color, alpha * a));
        }
    }
    for i in 0..n as u32 {
        let j = (i + 1) % n as u32;
        mesh.add_triangle(0, 1 + i, 1 + j);
        for ring in 0..rings.len() as u32 - 1 {
            let (a, b) = (1 + ring * n as u32, 1 + (ring + 1) * n as u32);
            mesh.add_triangle(a + i, b + i, a + j);
            mesh.add_triangle(a + j, b + i, b + j);
        }
    }
    Shape::mesh(mesh)
}

/// The warm light of a big lantern at `at` hanging from `spot`, `shown` of the way lit: thrown
/// wide over the trunks about it, and falling on its own tree's bark.
fn warm_light(spot: &Spot, at: Pos2, shown: f32, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    let flicker = 0.92 + 0.08 * (t * 5.0 + at.x * 0.1).sin() * (t * 2.3).sin();
    let a = ease(shown) * flicker;
    let bark = pos2(spot.from.x + spot.dir() * spot.w * 0.2, spot.from.y + 20.0 * u);
    vec![radial(bark, spot.w * 0.5, 110.0 * u, LIT_BARK, 0.45 * a), radial(at, 170.0 * u, 150.0 * u, WARM, 0.28 * a)]
}

/// Where the owl sits by the big lantern on `spot`: on its branch, between the trunk and it.
pub fn perch(spot: &Spot) -> Pos2 {
    spot.on(0.42) - vec2(0.0, spot.w * 0.07)
}

/// How much bigger the owl is than at a unit.
const OWL: f32 = 1.3;

/// The owl at a moment of bringing a title: where its feet are, whether it holds the title by its
/// top in them, and whether it beats its wings (it glides as it lowers the title).
#[derive(Clone, Copy, Debug)]
pub struct Flight {
    pub feet: Pos2,
    pub carrying: bool,
    pub beating: bool,
}

/// Where the owl is at `p`, bringing the title lying at `place` from its perch at `from` to its
/// new one at `to`: it flies up and over to above the title's place with it, lowers it into
/// place, lets go, rises a little and flies in an arc to its new perch, arriving as its course
/// ends. Its course in time is the dragon's and the balloon's (`pose`), along a way of its own.
pub fn flight(p: Pose, place: Rect, from: Pos2, to: Pos2, u: f32) -> Flight {
    // Close above the title, so that over a title at the top of a page the owl stays below the
    // toolbar.
    let hold = place.center_top();
    let over = hold - vec2(0.0, 22.0 * u);
    let arc = |a: Pos2, b: Pos2, s: f32| bend(a, pos2((a.x + b.x) / 2.0, a.y.min(b.y) - 90.0 * u), b, ease(s));
    if p.hooked && p.lower <= 0.0 {
        let s = (p.enter * ENTER + p.run * RUN) / (ENTER + RUN);
        return Flight { feet: arc(from, over, s), carrying: true, beating: true };
    }
    if p.hooked {
        return Flight { feet: over + (hold - over) * p.lower, carrying: true, beating: false };
    }
    let up = hold - vec2(0.0, 40.0 * u * p.hook);
    Flight { feet: arc(up, to, 1.0 - p.enter), carrying: false, beating: true }
}

/// How far the title lying at `place` is from its place at `p`, in the owl's feet.
pub fn owl_offset(p: Pose, place: Rect, from: Pos2, to: Pos2, u: f32) -> Vec2 {
    let f = flight(p, place, from, to, u);
    if f.carrying { f.feet - place.center_top() } else { Vec2::ZERO }
}

/// The owl with its feet at `feet`: sitting with its wings folded, blinking now and then and
/// turning its head; or flying, seen from the front, its wings spread and beating (or held out
/// gliding), its talons closed on the title's top when it carries one.
pub fn owl(feet: Pos2, flying: Option<Flight>, u: f32, time: f64) -> Vec<Shape> {
    let t = time as f32;
    // A little smaller in flight, holding the title close.
    let z = if flying.is_some() { 1.05 * u } else { OWL * u };
    let mut out = Vec::new();
    let body = feet - vec2(0.0, if flying.is_some() { 12.0 } else { 16.0 } * z);
    if let Some(f) = flying {
        let beat = if f.beating { (t * 9.0).sin() } else { 0.3 };
        for s in [-1.0f32, 1.0] {
            let shoulder = body + vec2(s * 7.0, -4.0) * z;
            let tip = shoulder + vec2(s * 30.0, -6.0 - 12.0 * beat) * z;
            let trail = shoulder + vec2(s * 21.0, 9.0 - 4.0 * beat) * z;
            out.push(solid(vec![shoulder, tip, trail, shoulder + vec2(0.0, 8.0 * z)], FEATHER));
            out.push(solid(vec![shoulder + (tip - shoulder) * 0.45, tip, shoulder + (trail - shoulder) * 0.7], FEATHER_LIGHT));
        }
        if f.carrying {
            for s in [-1.0f32, 1.0] {
                out.push(Shape::line_segment([body + vec2(s * 3.0, 9.0) * z, feet + vec2(s * 2.0 * z, 0.0)], Stroke::new(1.4 * z, BEAK)));
            }
        }
    }
    out.push(oval(body, vec2(0.0, 1.0), 15.0 * z, 11.0 * z, FEATHER));
    out.push(oval(body + vec2(0.0, 4.0 * z), vec2(0.0, 1.0), 9.0 * z, 7.0 * z, FEATHER_LIGHT));
    let turn = if flying.is_some() { 0.0 } else { (t * 0.35).sin().clamp(-0.6, 0.6) * 4.0 * z };
    let head = body - vec2(-turn * 0.3, 17.0 * z);
    out.push(Shape::circle_filled(head, 10.0 * z, FEATHER));
    for s in [-1.0f32, 1.0] {
        out.push(solid(vec![head + vec2(s * 4.0, -7.0) * z, head + vec2(s * 9.0, -6.0) * z, head + vec2(s * 8.0, -14.0) * z], FEATHER));
        let eye = head + vec2(s * 4.2 * z + turn, 0.0);
        out.push(Shape::circle_filled(eye, 3.4 * z, FEATHER_LIGHT));
        if flying.is_some() || (t % 5.0) > 0.15 {
            out.push(Shape::circle_filled(eye, 2.3 * z, EYE));
            out.push(Shape::circle_filled(eye, 1.0 * z, DARK));
        }
    }
    let beak = head + vec2(turn, 0.0);
    out.push(solid(vec![beak + vec2(-1.5, 3.0) * z, beak + vec2(1.5, 3.0) * z, beak + vec2(0.0, 6.5) * z], BEAK));
    out
}

/// Chapter `k`'s big lantern, hanging by a string from its branch at `hook`, swaying a little, as
/// lit as `shown`: it swells out of the swarm that becomes it. Where it hangs, and its shapes.
fn big_lantern(hook: Pos2, shown: f32, u: f32, k: usize, time: f64) -> (Pos2, Vec<Shape>) {
    let t = time as f32;
    let s = ease(shown / 0.4);
    let swing = (t * 0.7 + k as f32).sin() * 0.06;
    let down = vec2(swing, 1.0).normalized();
    let at = hook + down * 30.0 * u;
    let mut out = vec![Shape::line_segment([hook, at - down * 13.0 * u * s], Stroke::new(1.0 * u, glow(FEATHER_LIGHT, s)))];
    out.extend(lantern(at, s, 2.6 * u * s.max(0.05), k, time));
    (at, out)
}

impl App {
    /// The forest on the desk, behind the pages lying at `page`, in the Magic Forest only: a lantern
    /// for each page the chapter being written has had, a big lantern in the trees for each
    /// chapter before it (`Doc::garden`, so none goes out), and the owl beside the newest.
    pub fn draw_forest(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.forest = None;
        if self.themes.theme != Theme::Forest {
            self.eggs.forest_for = None;
            return;
        }
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let garden = &self.doc.garden;
        // The chapter being written is the newest, not the last of `Doc::chapters`: a title taken
        // away must not bring a lantern back down out of the trees.
        let chapter = garden.len().max(1) - 1;
        let lit = garden.last().map_or(0, |&n| n as usize);
        let hidden = if self.bar.side == crate::ui::BarSide::Bottom { area.bottom() - self.bar_rect(area, self.bar.side).top() } else { 0.0 };
        let e = &mut self.eggs;
        if e.forest_for.as_deref() != Some(&self.story_id) {
            // A story just opened, or the theme just picked, comes with its forest as it stands.
            (e.forest_for, e.flowed, e.merge) = (Some(self.story_id.clone()), chapter, None);
            e.born = vec![f64::NEG_INFINITY; lit];
        }
        if chapter > e.flowed {
            // A new chapter: the swarm of the one before flies into the trees, and a new one begins.
            e.merge = Some((now, e.flowed, e.born.len()));
            (e.flowed, e.born) = (chapter, Vec::new());
        }
        if e.merge.is_some_and(|(at, _, _)| now - at >= MERGE as f64) {
            e.merge = None;
        }
        // A lantern for each new page begins to rise.
        while e.born.len() < lit {
            e.born.push(now);
        }
        if e.merge.is_some() || e.born.iter().any(|&b| now - b < RISE as f64) {
            ctx.request_repaint();
        } else {
            // The lanterns are always on the move, and flicker.
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        let unit = e.unit(&ctx, area, now);
        let ground = area.bottom() - hidden - 6.0 * unit;
        let story = &self.story_id;
        let painter = ui.painter();
        painter.extend(woods(story, area, page, ground, unit, now));
        let places = spots(story, area, page, ground, unit);
        let mut seen = Seen::default();

        // A big lantern for every chapter before the one being written; the newest while its swarm
        // flies in becomes it.
        let merging = e.merge.map(|(at, k, n)| ((now - at) as f32 / MERGE, k, n));
        let shown = |k: usize| match merging {
            Some((q, m, _)) if m == k => ((q - 0.5) / 0.5).clamp(0.0, 1.0),
            _ => 1.0,
        };
        // Chapters past the trees' branches hang in the last of them.
        let spot = |k: usize| places[k.min(places.len().saturating_sub(1))];
        let hung: Vec<usize> = (0..chapter).filter(|&k| shown(k) > 0.0 && !places.is_empty()).collect();
        for &k in &hung {
            painter.extend(warm_light(&spot(k), spot(k).hook() + vec2(0.0, 30.0 * unit), shown(k), unit, now));
        }
        for &k in &hung {
            painter.extend(big_lantern(spot(k).hook(), shown(k), unit, k, now).1);
            seen.hung.push((spot(k).hook(), shown(k)));
        }
        // The owl sits by the newest big lantern, or where the first will hang; while it brings a
        // title it is drawn in flight over the page (`draw_carrier`), on its way to sit there.
        e.perch = (!places.is_empty()).then(|| perch(&spot(chapter.saturating_sub(1))));
        if let Some(at) = e.perch.filter(|_| e.lift.is_none_or(|l| l.by != Carrier::Owl)) {
            painter.extend(owl(at, None, unit, now));
            seen.owl = Some(at);
        }
        // The swarm flying into the trees, each lantern to the big one's place, and into it.
        if let Some((q, k, n)) = merging.filter(|_| !places.is_empty()) {
            let to = spot(k).hook() + vec2(0.0, 30.0 * unit);
            for i in 0..n.min(LANTERNS) {
                let from = lantern_at(story, k, i, n, area, page, unit, now);
                let s = ease((q - Seeds::of(story, 70_000 + i).next(0.0, 0.15)) / 0.5);
                let at = bend(from, pos2((from.x + to.x) / 2.0, from.y.min(to.y) - 60.0 * unit), to, s);
                painter.extend(lantern(at, 1.0 - ease((s - 0.85) / 0.15), unit, i, now));
                seen.flowing.push(at);
            }
        }
        // The swarm of the chapter being written, each new lantern rising into it from the floor.
        let n = e.born.len();
        for (i, &born) in e.born.iter().enumerate().take(LANTERNS) {
            let place = lantern_at(story, chapter, i, n, area, page, unit, now);
            let s = ease(((now - born) as f32 / RISE).min(1.0));
            let from = lantern_from(story, chapter, i, page, ground, unit, place);
            let at = if s < 1.0 { bend(from, pos2(from.x, place.y), place, s) } else { place };
            painter.extend(lantern(at, 0.4 + 0.6 * s, unit, i, now));
            seen.lanterns.push(at);
        }
        e.forest = Some(seen);
    }
}

/// The forest drawn last frame, for tests: each lantern of the swarm, each flying into the trees,
/// each big lantern in them, where it hangs from and how lit it is, and where the owl sits.
#[derive(Clone, Default, Debug)]
pub struct Seen {
    pub lanterns: Vec<Pos2>,
    pub flowing: Vec<Pos2>,
    pub hung: Vec<(Pos2, f32)>,
    pub owl: Option<Pos2>,
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
    fn the_forest_is_there_from_the_start_the_same_for_the_same_story() {
        let shapes = woods("a story", desk(), page(), 880.0, 1.0, 0.0);
        assert!(shapes.len() > 100, "trunks, leaves, the moon: {}", shapes.len());
        assert_eq!(shapes.len(), woods("a story", desk(), page(), 880.0, 1.0, 0.0).len());
    }

    #[test]
    fn the_lanterns_fly_together_in_a_swarm_which_drifts_from_one_side_of_the_page_to_the_other() {
        let at = |i: usize, t: f64| lantern_at("a story", 0, i, 12, desk(), page(), 1.0, t);
        let (mut lefts, mut rights) = (0, 0);
        for f in 0..(180 * 10) {
            let t = f as f64 / 10.0;
            let all: Vec<Pos2> = (0..12).map(|i| at(i, t)).collect();
            let middle = all.iter().fold(Vec2::ZERO, |m, p| m + p.to_vec2()) / 12.0;
            assert!(all.iter().all(|p| p.distance(middle.to_pos2()) < 260.0), "together: {all:?}");
            assert!(all.iter().all(|p| desk().contains(*p)), "over the desk");
            (lefts, rights) = (lefts + usize::from(middle.x < page().left()), rights + usize::from(middle.x > page().right()));
            for i in 0..12 {
                assert!(at(i, t).distance(at(i, t + 1.0 / 60.0)) < 2.0, "gently");
            }
        }
        assert!(lefts > 400 && rights > 400, "beside the page on both sides: {lefts} {rights}");
        assert!(lantern_at("a story", 0, 0, 40, desk(), page(), 1.0, 0.0) != lantern_at("a story", 1, 0, 40, desk(), page(), 1.0, 0.0), "each chapter its own");
    }

    #[test]
    fn the_trees_nearest_the_page_have_branches_enough_for_many_big_lanterns_close_to_it() {
        let places = spots("a story", desk(), page(), 880.0, 1.0);
        assert!(places.len() >= 20, "branches enough: {}", places.len());
        let hooks: Vec<Pos2> = places.iter().map(|s| s.hook()).collect();
        for (k, h) in hooks.iter().enumerate() {
            assert!(h.x < page().left() - 20.0 || h.x > page().right() + 20.0, "{k}: beside the page: {h:?}");
            assert!(h.y < 880.0 - 150.0 && h.y > desk().top() + 80.0, "{k}: up in the trees: {h:?}");
            assert!(hooks[..k].iter().all(|o| o.distance(*h) > 45.0), "{k}: room of its own");
        }
        let near = |h: &Pos2| if h.x < page().left() { page().left() - h.x } else { h.x - page().right() };
        assert!(hooks[..8].iter().all(|h| near(h) < 330.0), "the first eight close to the page: {:?}", &hooks[..8]);
        let lefts = hooks[..8].iter().filter(|h| h.x < page().left()).count();
        assert!((3..=5).contains(&lefts), "on both sides: {lefts}");
        assert!(places.iter().all(|s| perch(s).distance(s.hook()) < 70.0), "the owl can sit beside each");
    }

    #[test]
    fn the_owl_flies_from_its_perch_with_the_title_sets_it_in_place_and_sits_by_the_new_lantern() {
        let (from, to) = (pos2(300.0, 500.0), pos2(1100.0, 300.0));
        let place = Rect::from_min_size(pos2(600.0, 140.0), vec2(200.0, 30.0));
        let pace = Carrier::Owl.pace();
        let at = |t: f32| pose_of(Carrier::Owl, t * pace, None);
        let feet = |t: f32| flight(at(t), place, from, to, 1.0).feet;
        let title = |t: f32| place.center_top() + owl_offset(at(t), place, from, to, 1.0);
        assert!(feet(0.0).distance(from) < 0.5 && title(0.0).distance(from) < 0.5, "from its perch, the title in its feet");
        let over = feet(OVER);
        assert!((over.x - place.center().x).abs() < 1.0 && over.y < place.top(), "over the title's place: {over:?}");
        assert!(title(ENTER + RUN + LOWER).distance(place.center_top()) < 0.5, "lowered into place");
        assert_eq!(title(COURSE - LEAVE), place.center_top(), "and let go");
        assert!(feet(COURSE).distance(to) < 0.5, "sitting at its new perch");
        assert!(pose_of(Carrier::Owl, (COURSE + 0.01) * pace, None).done);
        let mut was = feet(0.0);
        for f in 1..(COURSE * 60.0) as usize {
            let now = feet(f as f32 / 60.0);
            assert!(now.distance(was) < 40.0, "flying, not jumping, at {f}");
            was = now;
        }
    }
}
