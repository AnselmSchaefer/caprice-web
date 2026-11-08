//! The country the Magic Vineyard (`vineyard`) lies in: a golden autumn afternoon, the sun low to
//! one side, soft clouds drifting slowly over rolling hills in the haze, mist in their valleys,
//! cypresses and a farmhouse on them. What lies in it draws its own lines along the nearer hills,
//! what flies over them and its own slope in front, so another theme of the countryside can share
//! it. Like everything in `eggs.rs` it is a function of the story and of time, laid out
//! from the land's middle and ground whatever part of it shows.

use super::*;

/// The sky, the sun and the clouds, lit warm from below the sun.
pub const SKY_TOP: Color32 = Color32::from_rgb(118, 164, 210);
const SKY_LOW: Color32 = Color32::from_rgb(244, 226, 194);
pub const SUN: Color32 = Color32::from_rgb(255, 246, 222);
const SUN_GLOW: Color32 = Color32::from_rgb(255, 214, 140);
pub const CLOUD_LIT: Color32 = Color32::from_rgb(255, 250, 240);
const CLOUD_SHADE: Color32 = Color32::from_rgb(206, 194, 200);
/// The air that pales what lies further off.
pub const HAZE: Color32 = Color32::from_rgb(222, 212, 194);
/// The hills far off, nearer and nearest; the light; the cypresses and the farmhouse; and shadow.
const HILL_FAR: Color32 = Color32::from_rgb(118, 138, 148);
const HILL_MID: Color32 = Color32::from_rgb(178, 164, 104);
const HILL_NEAR: Color32 = Color32::from_rgb(196, 170, 96);
pub const SUNLIT: Color32 = Color32::from_rgb(255, 232, 170);
const CYPRESS: Color32 = Color32::from_rgb(40, 58, 34);
const STONE: Color32 = Color32::from_rgb(230, 210, 172);
const ROOF: Color32 = Color32::from_rgb(176, 88, 58);
pub const SHADOW: Color32 = Color32::from_rgb(70, 52, 30);

/// How far up the slope in front goes, in units. (The country reaches as far as the bay, `REACH`.)
pub const RISE: f32 = 260.0;

/// The land a magic theme of the countryside lies on: laid out from `middle` (across) and `ground` (the foot of the desk).
#[derive(Clone, Copy, Debug)]
pub struct Land {
    pub middle: f32,
    pub ground: f32,
    pub u: f32,
}

/// How far `s` has gone from `a` to `b`, eased.
pub fn between(s: f32, a: f32, b: f32) -> f32 {
    ease((s - a) / (b - a))
}

/// A soft puff of cloud about `at`, `r` across and `h` high, `alpha` at its middle and fading to
/// nothing at its edge: like `radial`, with fewer points, as a sky holds many.
pub fn puff(at: Pos2, r: f32, h: f32, color: Color32, alpha: f32) -> Shape {
    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(at, glow(color, alpha));
    let n = 14;
    for (f, a) in [(0.55, 0.5), (1.0, 0.0)] {
        for i in 0..n {
            let ang = i as f32 / n as f32 * 2.0 * PI;
            mesh.colored_vertex(at + vec2(ang.cos() * r * f, ang.sin() * h * f), glow(color, alpha * a));
        }
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.add_triangle(0, 1 + i, 1 + j);
        mesh.add_triangle(1 + i, 1 + n + i, 1 + j);
        mesh.add_triangle(1 + j, 1 + n + i, 1 + n + j);
    }
    Shape::mesh(mesh)
}

/// A hill's height at `x`: rolling, from three waves of its own.
pub fn hill(waves: &[(f32, f32, f32)], base: f32, x: f32, middle: f32, u: f32) -> f32 {
    base - waves.iter().map(|&(a, wl, ph)| a * u * (0.5 + 0.5 * ((x - middle) / (wl * u) + ph).sin())).sum::<f32>()
}

/// The hills behind the slope, from the furthest: each one's colour, the height of its foot, how high
/// it rolls, how much the haze pales it, how far apart the stripes of the fields along it are (none
/// on the far ones), and its waves.
#[allow(clippy::type_complexity)]
fn hills(story: &str, land: &Land) -> Vec<(Color32, f32, f32, f32, f32, Vec<(f32, f32, f32)>)> {
    let u = land.u;
    let top = land.ground - RISE * u;
    let horizon = top - 150.0 * u;
    let mut s = Seeds::of(story, 134_000);
    [
        (HILL_FAR, horizon - 10.0 * u, 120.0, 0.72, 0.0),
        (HILL_FAR.lerp_to_gamma(HILL_MID, 0.4), horizon + 40.0 * u, 100.0, 0.55, 0.0),
        (HILL_MID, top - 60.0 * u, 80.0, 0.32, 6.0),
        (HILL_NEAR, top - 20.0 * u, 50.0, 0.14, 8.0),
    ].into_iter().map(|(color, base, high, haze, rows)| {
        let waves = (0..3).map(|w| (high * s.next(0.25, 0.5), s.next(70.0, 160.0) * (1.0 + 1.5 * w as f32), s.next(0.0, 6.3))).collect();
        (color, base, high, haze, rows, waves)
    }).collect()
}

/// Where the farmhouse stands on the nearer hills.
pub fn farm(story: &str, land: &Land) -> Pos2 {
    let mut s = Seeds::of(story, 137_000);
    let side = if s.next(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
    let x = land.middle + side * s.next(480.0, 640.0) * land.u;
    let (_, base, _, _, _, waves) = &hills(story, land)[2];
    pos2(x, hill(waves, *base, x, land.middle, land.u) + 6.0 * land.u)
}

/// The country behind the slope: the sky, warmer low down, the low sun to one side and its glow,
/// soft clouds drifting slowly, and hills far off and nearer, paler in the haze, the nearer with lines along their slopes (`lines`: each one's colour and width,
/// from its number and how much the haze pales it, if it is drawn), cypresses and a farmhouse; the
/// nearest hill's foot `field`'s colour, the slope's; then what else flies over them (`company`). All of it is laid out from the land's middle
/// and its ground, as far as `REACH` either side, whatever part of it shows in `area`: only the
/// plain fills reach to `area`'s edges. So zooming only makes it bigger or smaller and moves nothing.
pub fn country(story: &str, area: Rect, land: &Land, time: f64, field: Color32, lines: &dyn Fn(usize, f32) -> Option<(Color32, f32)>, company: &dyn Fn(&mut Vec<Shape>)) -> Vec<Shape> {
    let (u, middle, ground) = (land.u, land.middle, land.ground);
    let t = time as f32;
    let mut out = Vec::new();
    let top_of_slope = ground - RISE * u;
    let horizon = top_of_slope - 150.0 * u;
    let mut sky = egui::epaint::Mesh::default();
    sky.colored_vertex(area.left_top(), SKY_TOP);
    sky.colored_vertex(area.right_top(), SKY_TOP);
    sky.colored_vertex(pos2(area.right(), horizon), SKY_LOW);
    sky.colored_vertex(pos2(area.left(), horizon), SKY_LOW);
    sky.add_triangle(0, 1, 2);
    sky.add_triangle(0, 2, 3);
    out.push(Shape::mesh(sky));
    out.push(Shape::rect_filled(Rect::from_min_max(pos2(area.left(), horizon), area.max), 0.0, SKY_LOW));
    let mut s = Seeds::of(story, 136_000);
    // The sun low on the side the light comes from.
    let sun = pos2(middle - 700.0 * u, horizon - 230.0 * u);
    out.push(radial(sun, 620.0 * u, 420.0 * u, SUN_GLOW, 0.5));
    out.push(radial(sun, 130.0 * u, 130.0 * u, SUN, 0.75));
    out.push(Shape::circle_filled(sun, 22.0 * u, SUN));
    // Clouds, soft, of puffs lit above and shaded below, drifting slowly the same way; how far is a
    // share of the way across, never the time multiplied by a size.
    for i in 0..10 {
        let mut c = Seeds::of(story, 135_000 + i);
        let share = (c.next(0.0, 1.0) + t * c.next(2.0, 4.0) / (2.0 * REACH)).rem_euclid(1.0);
        let at = pos2(middle - REACH * u + 2.0 * REACH * u * share, horizon - c.next(60.0, 380.0) * u);
        let w = c.next(150.0, 320.0) * u;
        let puffs = c.next(6.0, 11.0) as usize;
        let mut lit = Vec::new();
        for _ in 0..puffs {
            let f = c.next(-1.0, 1.0);
            let r = w * c.next(0.12, 0.22) * (1.0 - 0.4 * f.abs());
            let p = at + vec2(f * w * 0.5, -(1.0 - f * f) * w * c.next(0.05, 0.16));
            out.push(puff(p + vec2(0.0, r * 0.35), r * 1.25, r * 0.7, CLOUD_SHADE, 0.55));
            lit.push((p, r));
        }
        for (p, r) in lit {
            out.push(puff(p + vec2(-r * 0.1, -r * 0.12), r * 1.1, r * 0.85, CLOUD_LIT, 0.8));
        }
    }
    // The hills, from the furthest: each a smooth fill from its crest down, lit where it faces the
    // sun, mist in the valleys of the far ones, the nearer with fields mown in stripes.
    let from = middle - REACH * u;
    let grid = 16.0 * u;
    // As far as `REACH` either side, and further if more shows (the page pushed to one side), on the
    // same points, so the hills within it are the same whatever shows.
    let first = ((area.left() - from) / grid).floor().min(0.0) as i64 - 1;
    let last = ((area.right() - from) / grid).ceil().max(2.0 * REACH * u / grid) as i64 + 1;
    let xs: Vec<f32> = (first..=last).map(|i| from + i as f32 * grid).collect();
    let n = xs.len() - 1;
    let all = hills(story, land);
    for (j, (color, base, high, haze, rows, waves)) in all.iter().enumerate() {
        let (color, base, high, haze, rows) = (*color, *base, *high, *haze, *rows);
        let y = |x: f32| hill(waves, base, x, middle, u);
        let color = color.lerp_to_gamma(HAZE, haze);
        let low = if j == all.len() - 1 { field } else { color.lerp_to_gamma(HAZE, 0.5) };
        let mut mesh = egui::epaint::Mesh::default();
        for &x in &xs {
            let slope = (y(x + grid) - y(x - grid)) / (2.0 * grid);
            let lit = (0.5 - slope * 4.0).clamp(0.0, 1.0);
            mesh.colored_vertex(pos2(x, y(x)), color.lerp_to_gamma(SUNLIT.lerp_to_gamma(HAZE, haze), 0.35 * lit));
            mesh.colored_vertex(pos2(x, y(x) + high * u * 1.2), low);
            mesh.colored_vertex(pos2(x, ground), low);
        }
        for i in 0..n as u32 {
            let (a, b) = (3 * i, 3 * i + 3);
            mesh.add_triangle(a, a + 1, b);
            mesh.add_triangle(b, a + 1, b + 1);
            mesh.add_triangle(a + 1, a + 2, b + 1);
            mesh.add_triangle(b + 1, a + 2, b + 2);
        }
        out.push(Shape::mesh(mesh));
        if rows > 0.0 {
            for r in 1..9 {
                let Some((ink, wide)) = lines(r, haze) else { continue };
                let down = rows_apart(rows, r, u);
                let line: Vec<Pos2> = xs.iter().map(|&x| pos2(x, y(x) + down)).collect();
                out.push(Shape::line(line, Stroke::new(wide * u, ink)));
            }
        }
        if j == 2 {
            // A farmhouse of pale stone with a red roof, cypresses by it and along the crests.
            let at = farm(story, land);
            let (x, foot) = (at.x, at.y);
            let hz = |c: Color32| c.lerp_to_gamma(HAZE, 0.35);
            let (w, tall) = (40.0 * u, 20.0 * u);
            out.push(Shape::rect_filled(Rect::from_min_max(pos2(x - w / 2.0, foot - tall), pos2(x + w / 2.0, foot)), 0.0, hz(STONE)));
            out.push(solid(vec![pos2(x + w / 2.0, foot - tall), pos2(x + w / 2.0 + 14.0 * u, foot - tall - 4.0 * u), pos2(x + w / 2.0 + 14.0 * u, foot - 3.0 * u), pos2(x + w / 2.0, foot)], hz(STONE.lerp_to_gamma(SHADOW, 0.25))));
            out.push(solid(vec![pos2(x - w / 2.0 - 3.0 * u, foot - tall), pos2(x + w / 2.0 + 3.0 * u, foot - tall), pos2(x + w / 2.0 - 4.0 * u, foot - tall - 9.0 * u), pos2(x - w / 2.0 + 4.0 * u, foot - tall - 9.0 * u)], hz(ROOF)));
            out.push(solid(vec![pos2(x + w / 2.0 + 3.0 * u, foot - tall), pos2(x + w / 2.0 + 15.0 * u, foot - tall - 4.0 * u), pos2(x + w / 2.0 - 4.0 * u, foot - tall - 9.0 * u)], hz(ROOF.lerp_to_gamma(SHADOW, 0.3))));
            for k in 0..3 {
                let wx = x - w / 2.0 + (8.0 + 12.0 * k as f32) * u;
                out.push(Shape::rect_filled(Rect::from_min_size(pos2(wx, foot - tall + 6.0 * u), vec2(3.5 * u, 5.0 * u)), 0.0, hz(SHADOW.lerp_to_gamma(STONE, 0.2))));
            }
            for i in 0..14 {
                let near = i < 2;
                let cx = if near { x + (if i == 0 { -w * 0.75 } else { w * 0.85 }) } else { middle + s.next(-1.0, 1.0) * REACH * u };
                let tall = s.next(28.0, 50.0) * u * if near { 1.2 } else { 1.0 };
                let base = pos2(cx, y(cx) + 3.0 * u);
                let dark = CYPRESS.lerp_to_gamma(HAZE, 0.38);
                out.push(flame(base, vec2(0.0, -1.0), tall, tall * 0.12, dark));
                out.push(flame(base + vec2(-tall * 0.03, -tall * 0.05), vec2(0.0, -1.0), tall * 0.85, tall * 0.05, dark.lerp_to_gamma(SUNLIT, 0.25)));
            }
        }
    }
    company(&mut out);
    out
}

/// How far below its hill's crest line `r` of the lines along it lies, the lines `rows` apart at the
/// top and further apart lower down, as nearer.
fn rows_apart(rows: f32, r: usize, u: f32) -> f32 {
    rows * u * r as f32 * (1.0 + 0.15 * r as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hills_go_on_to_the_edges_of_whatever_shows_however_far_from_the_page() {
        for far in [-4000.0, 4000.0] {
            let area = Rect::from_min_size(pos2(far, 0.0), vec2(1400.0, 900.0));
            let land = Land { middle: 700.0, ground: 880.0, u: 1.0 };
            let lines = |_: usize, _: f32| Some((Color32::BLACK, 1.0));
            let hills = country("a story", area, &land, 0.0, SKY_TOP, &lines, &|_| {}).into_iter().filter(|s| matches!(s, Shape::Mesh(m) if m.vertices.len() > 100) || matches!(s, Shape::Path(_)));
            let across = hills.map(|s| s.visual_bounding_rect()).fold(Rect::NOTHING, |a, r| a | r);
            assert!(across.left() <= area.left() && across.right() >= area.right(), "{far}: {across:?}");
        }
    }

    #[test]
    fn the_clouds_drift_slowly_whatever_the_size_of_things() {
        let desk = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0));
        let clouds = |u: f32, t: f64| country("a story", desk, &Land { middle: 700.0, ground: 880.0, u }, t, SKY_TOP, &|_, _| None, &|_| {}).iter().filter_map(|s| match s {
            Shape::Mesh(m) if m.vertices.first().is_some_and(|v| v.color.a() > 0 && v.color.to_srgba_unmultiplied()[..3] == CLOUD_LIT.to_array()[..3]) => Some(m.vertices[0].pos),
            _ => None,
        }).collect::<Vec<_>>();
        let (a, b, c) = (clouds(1.0, 5000.0), clouds(1.0, 5001.0), clouds(1.01, 5000.0));
        assert!(!a.is_empty());
        for ((a, b), c) in a.iter().zip(&b).zip(&c) {
            assert!(a.distance(*b) < 5.0 || (a.x - b.x).abs() > 3000.0, "a second later: {a:?} {b:?}");
            assert!(a.distance(*c) < 40.0, "a little bigger: {a:?} {c:?}");
        }
    }
}
