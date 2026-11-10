//! Scenes that move a little. Claude paints a scene still, but puts each part that could move in
//! a group of its own, named by how it moves (`move-wing-1-310-240`: a wing beating about the
//! shoulder at 310,240). The picture is rendered once without those parts, and each part on its
//! own; Caprice then moves the parts every frame, as textures, which costs next to nothing. Smoke
//! is Caprice's own: Claude only marks the chimney tops. The world moves, never the people: a part
//! that does not fit one of the few kinds of movement stays in the still picture, as does one
//! marked wrongly (a wing whose shoulder is nowhere near it).

use std::f64::consts::TAU;

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2, epaint::Mesh, pos2, vec2};

/// What Claude is told about the parts that move, in the scene's prompt.
pub const PROMPT: &str = "- The picture will move a little. Put each part that moves in its own top-level <g>, after \
      everything it stands in front of, with an id saying how it moves; everything else stays still. \
      Do not use SVG animation yourself. Mark only things that belong in the scene: add nothing just \
      to have something move, and leave people and animals on the ground still.\n  \
      - id=\"move-drift-N\": one cloud or mist bank, drifting slowly sideways.\n  \
      - id=\"move-wing-N-X-Y\": one wing of a flying creature, beating by turning about the point X,Y \
        (in viewBox units) where it joins the body, which must lie on the shoulder. The wing only, \
        never the body, drawn in its middle position.\n  \
      - id=\"move-sway-N-X-Y\": a tree crown, reeds, a flag or a branch, swaying gently about the point \
        X,Y at its foot.\n  \
      - id=\"move-smoke-N-X-Y\": an empty group marking a chimney top at X,Y where smoke rises; draw no \
        smoke yourself.\n  \
      - id=\"move-flicker-N\": a lit window, lantern, fire or star that flickers.\n  \
      - id=\"move-shimmer-N\": the glints and ripple lines on water, apart from the water itself.\n  \
      Ids use whole numbers, e.g. move-wing-1-310-240.\n";

/// How far from its part a wing's shoulder or a sway's foot may be, as a share of the picture's width.
const NEAR_PART: f32 = 0.04;
/// How close two wings' shoulders are for them to be one creature's, beating together.
const ONE_CREATURE: f32 = 0.06;

/// How a part of a picture moves. Points are shares of the picture's width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Drift,
    /// `side` is 1 for a wing reaching out to the left of its shoulder, -1 to the right; `phase`
    /// is shared by the wings of one creature.
    Wing { pivot: Pos2, side: f32, phase: f32 },
    Sway { foot: Pos2 },
    Smoke,
    Flicker,
    Shimmer,
}

/// A part that moves: how, a number to set it apart from the others, where it is in the picture
/// (shares of its size; for smoke, the chimney top), and the part rendered (none for smoke).
pub struct Layer {
    pub motion: Motion,
    pub seed: f32,
    pub at: Rect,
    pub image: Option<ColorImage>,
}

/// A picture rendered: what stays still, and the parts that move.
pub struct Layered {
    pub still: ColorImage,
    pub layers: Vec<Layer>,
}

impl Layered {
    pub fn still(image: ColorImage) -> Self {
        Self { still: image, layers: Vec::new() }
    }
}

/// A point in viewBox units, as Claude writes it in an id.
type Point = (f32, f32);

/// The motion an id names, and its number, with its point in viewBox units.
fn parse_id(id: &str) -> Option<(&str, f32, Option<Point>)> {
    let mut bits = id.strip_prefix("move-")?.split('-');
    let (kind, n) = (bits.next()?, bits.next()?.parse().ok()?);
    let point = match (bits.next(), bits.next()) {
        (Some(x), Some(y)) => Some((x.parse().ok()?, y.parse().ok()?)),
        _ => None,
    };
    let needs_point = matches!(kind, "wing" | "sway" | "smoke");
    (bits.next().is_none() && needs_point == point.is_some()).then_some((kind, n, point))
}

/// Render `svg` `width` pixels wide, its moving parts apart from the rest. A part that cannot be
/// found, rendered or trusted stays in the still picture.
pub fn layered(svg: &str, width: f32) -> Result<Layered, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("the picture could not be read: {e}"))?;
    let Ok(xml) = roxmltree::Document::parse(svg) else { return crate::backdrop::rasterize(svg, width).map(Layered::still) };
    let size = tree.size();
    let scale = width / size.width();
    let (w, h) = (width.round() as u32, (size.height() * scale).round().max(1.0) as u32);
    let root = tree.root().transform();
    // A point in viewBox units, as a share of the picture.
    let share = |x: f32, y: f32| {
        let mut p = tiny_skia::Point::from_xy(x, y);
        root.map_point(&mut p);
        pos2(p.x / size.width(), p.y / size.height())
    };
    let (mut cut, mut layers) = (Vec::new(), Vec::new());
    let mut shoulders: Vec<(Pos2, f32)> = Vec::new();
    let moving = |n: &roxmltree::Node| n.is_element() && n.attribute("id").is_some_and(|id| id.starts_with("move-"));
    for node in xml.descendants().filter(moving) {
        // A part inside another moves with it; one in <defs> is not drawn where it stands.
        if node.ancestors().skip(1).any(|a| moving(&a) || a.has_tag_name("defs")) {
            continue;
        }
        let id = node.attribute("id").unwrap_or_default();
        let Some((kind, seed, point)) = parse_id(id) else { continue };
        let point = point.map(|(x, y)| share(x, y));
        if kind == "smoke" {
            if let Some(p) = point.filter(|p| (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y)) {
                layers.push(Layer { motion: Motion::Smoke, seed, at: Rect::from_min_max(p, p), image: None });
                cut.push(node.range());
            }
            continue;
        }
        let Some(part) = tree.node_by_id(id) else { continue };
        let Some(bb) = part.abs_layer_bounding_box() else { continue };
        let at = Rect::from_min_max(pos2(bb.x() / size.width(), bb.y() / size.height()), pos2(bb.right() / size.width(), bb.bottom() / size.height()));
        // A shoulder or foot is on its part's edge, or near it.
        let margin = vec2(NEAR_PART, NEAR_PART * size.width() / size.height());
        let near = |p: Pos2| at.expand2(margin).contains(p);
        let motion = match (kind, point) {
            ("drift", _) => Motion::Drift,
            ("flicker", _) => Motion::Flicker,
            ("shimmer", _) => Motion::Shimmer,
            ("sway", Some(foot)) if near(foot) => Motion::Sway { foot },
            ("wing", Some(pivot)) if near(pivot) => {
                let side = if at.center().x < pivot.x { 1.0 } else { -1.0 };
                let close = |&&(s, _): &&(Pos2, f32)| (s.x - pivot.x).hypot((s.y - pivot.y) * size.height() / size.width()) < ONE_CREATURE;
                let phase = shoulders.iter().find(close).map_or(shoulders.len() as f32 * 0.37 % 1.0, |&(_, ph)| ph);
                shoulders.push((pivot, phase));
                Motion::Wing { pivot, side, phase }
            }
            _ => continue,
        };
        // The part rendered alone, cropped to where it is, in its place under its groups.
        let (x0, y0) = ((bb.x() * scale).floor().max(0.0), (bb.y() * scale).floor().max(0.0));
        let (x1, y1) = ((bb.right() * scale).ceil().min(w as f32), (bb.bottom() * scale).ceil().min(h as f32));
        let Some(mut pixmap) = tiny_skia::Pixmap::new((x1 - x0) as u32, (y1 - y0) as u32) else { continue };
        let parent = match part {
            usvg::Node::Group(g) => g.transform().invert().map(|own| g.abs_transform().pre_concat(own)),
            _ => Some(part.abs_transform()),
        };
        let Some(parent) = parent else { continue };
        let place = tiny_skia::Transform::from_translate(-x0, -y0).pre_scale(scale, scale).pre_concat(parent).pre_translate(bb.x(), bb.y());
        if resvg::render_node(part, place, &mut pixmap.as_mut()).is_none() {
            continue;
        }
        let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
        let at = Rect::from_min_max(pos2(x0 / w as f32, y0 / h as f32), pos2(x1 / w as f32, y1 / h as f32));
        layers.push(Layer { motion, seed, at, image: Some(image) });
        cut.push(node.range());
    }
    // The still picture is the drawing without the parts that move.
    let mut still = svg.to_owned();
    cut.sort_by_key(|r| std::cmp::Reverse(r.start));
    for r in cut {
        still.replace_range(r, "");
    }
    match crate::backdrop::rasterize(&still, width) {
        Ok(image) => Ok(Layered { still: image, layers }),
        Err(_) => crate::backdrop::rasterize(svg, width).map(Layered::still),
    }
}

/// A part of a picture rendered, ready to move.
struct Part {
    motion: Motion,
    seed: f32,
    at: Rect,
    texture: Option<TextureHandle>,
}

/// A picture as textures: the still one, its parts, and a puff of smoke if it has chimneys.
pub struct Drawn {
    pub still: TextureHandle,
    parts: Vec<Part>,
    puff: Option<TextureHandle>,
}

/// Where a part is drawn at a moment: turned by `angle` about `pivot`, squashed upright by
/// `squash` there first, shifted by `shift`; and how strongly it shows.
struct Pose {
    pivot: Pos2,
    angle: f32,
    squash: f32,
    shift: Vec2,
    alpha: f32,
}

impl Pose {
    const STILL: Pose = Pose { pivot: Pos2::ZERO, angle: 0.0, squash: 1.0, shift: Vec2::ZERO, alpha: 1.0 };

    fn apply(&self, p: Pos2) -> Pos2 {
        let v = p - self.pivot;
        let v = vec2(v.x, v.y * self.squash);
        let (s, c) = self.angle.sin_cos();
        self.pivot + vec2(v.x * c - v.y * s, v.x * s + v.y * c) + self.shift
    }
}

/// A wing's beat at `t` beats: 1 up, -1 down, quickly down and slowly up again.
fn wingbeat(t: f64) -> f32 {
    let ph = t.rem_euclid(1.0) as f32;
    let a = if ph < 0.4 { 1.0 - 2.0 * ph / 0.4 } else { -1.0 + 2.0 * (ph - 0.4) / 0.6 };
    a.signum() * a.abs().powf(0.8)
}

/// Where `motion` has a part at `t` seconds on a page `size` large, its points in page units.
fn pose(motion: Motion, seed: f32, t: f64, page: Rect) -> Pose {
    let (seed, w) = (seed as f64, page.width());
    let wave = |period: f64, offset: f64| (TAU * t / period + offset).sin() as f32;
    let at = |p: Pos2| page.min + p.to_vec2() * page.size();
    match motion {
        // Clouds take over a minute to go and come back, a few points either way.
        Motion::Drift => Pose { shift: vec2(0.03 * w * wave(70.0 + 11.0 * (seed % 4.0), seed), 0.0), ..Pose::STILL },
        Motion::Wing { pivot, side, phase } => {
            let a = wingbeat(t + phase as f64);
            Pose { pivot: at(pivot), angle: side * 22f32.to_radians() * a, squash: 1.0 - 0.15 * (1.0 - a.abs()), ..Pose::STILL }
        }
        Motion::Sway { foot } => Pose { pivot: at(foot), angle: 1.2f32.to_radians() * wave(5.0 + seed % 3.0, seed), ..Pose::STILL },
        Motion::Flicker => Pose { alpha: 0.75 + 0.25 * ((t * 7.0 + seed * 1.7).sin() * (t * 3.1 + seed).sin()) as f32, ..Pose::STILL },
        Motion::Shimmer => Pose { shift: vec2(0.004 * w * wave(3.2, seed), 0.0), alpha: 0.7 + 0.3 * wave(2.3, seed * 1.3), ..Pose::STILL },
        Motion::Smoke => Pose::STILL,
    }
}

/// `texture` stretched over the area `at(u, v)` gives for `u` and `v` from 0 to 1, in `strips`
/// across, so it bends with a turning page.
pub fn strip_mesh(texture: egui::TextureId, strips: usize, tint: Color32, at: &dyn Fn(f32, f32) -> Pos2) -> Mesh {
    let mut mesh = Mesh::with_texture(texture);
    for k in 0..=strips {
        let f = k as f32 / strips as f32;
        mesh.vertices.push(egui::epaint::Vertex { pos: at(f, 0.0), uv: pos2(f, 0.0), color: tint });
        mesh.vertices.push(egui::epaint::Vertex { pos: at(f, 1.0), uv: pos2(f, 1.0), color: tint });
        if k > 0 {
            let b = (k * 2) as u32;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b + 1, b);
        }
    }
    mesh
}

/// A soft round puff of smoke, white at its heart and fading out to its edge.
fn puff_image() -> ColorImage {
    const N: usize = 32;
    let mut rgba = Vec::with_capacity(N * N * 4);
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 + 0.5 - N as f32 / 2.0).hypot(y as f32 + 0.5 - N as f32 / 2.0) / (N as f32 / 2.0)).min(1.0);
            let a = (1.0 - d * d).powi(2);
            rgba.extend([210, 214, 222, (255.0 * a) as u8]);
        }
    }
    ColorImage::from_rgba_unmultiplied([N, N], &rgba)
}

impl Drawn {
    pub fn load(ctx: &egui::Context, picture: Layered) -> Self {
        let still = ctx.load_texture("scene", picture.still, TextureOptions::LINEAR);
        let puff = picture.layers.iter().any(|l| l.motion == Motion::Smoke).then(|| ctx.load_texture("puff", puff_image(), TextureOptions::LINEAR));
        let parts = picture.layers.into_iter()
            .map(|l| Part { motion: l.motion, seed: l.seed, at: l.at, texture: l.image.map(|i| ctx.load_texture("scene part", i, TextureOptions::LINEAR)) })
            .collect();
        Self { still, parts, puff }
    }

    /// Something in the picture moves.
    pub fn moves(&self) -> bool {
        !self.parts.is_empty()
    }

    /// The textures of the parts that move, for the tests to find them among the shapes painted.
    #[cfg(test)]
    pub fn part_textures(&self) -> Vec<(Motion, egui::TextureId)> {
        self.parts.iter().filter_map(|p| Some((p.motion, p.texture.as_ref()?.id()))).collect()
    }

    /// The picture over `rect` through `map`, tinted `tint`, as it is at `time` seconds, or with
    /// nothing moving (and no smoke) without a time.
    pub fn shapes(&self, rect: Rect, tint: Color32, map: &dyn Fn(Pos2) -> Pos2, time: Option<f64>) -> Vec<Shape> {
        let page = |f: Pos2| rect.min + f.to_vec2() * rect.size();
        let mut shapes = vec![Shape::mesh(strip_mesh(self.still.id(), 12, tint, &|u, v| map(page(pos2(u, v)))))];
        for part in &self.parts {
            match (&part.texture, time) {
                (Some(texture), _) => {
                    let pose = time.map_or(Pose::STILL, |t| pose(part.motion, part.seed, t, rect));
                    let at = |u: f32, v: f32| map(pose.apply(page(part.at.lerp_inside(vec2(u, v)))));
                    shapes.push(Shape::mesh(strip_mesh(texture.id(), 4, tint.gamma_multiply(pose.alpha), &at)));
                }
                (None, Some(t)) if part.motion == Motion::Smoke => {
                    if let Some(puff) = &self.puff {
                        shapes.push(Shape::mesh(smoke(puff.id(), page(part.at.min), rect.width(), part.seed, t, tint, map)));
                    }
                }
                _ => {}
            }
        }
        shapes
    }
}

/// Smoke rising from a chimney at `top` on a page `w` wide at `t` seconds: puffs that rise, lean
/// with the wind, grow and fade, one after the other.
fn smoke(puff: egui::TextureId, top: Pos2, w: f32, seed: f32, t: f64, tint: Color32, map: &dyn Fn(Pos2) -> Pos2) -> Mesh {
    const PUFFS: usize = 8;
    let mut mesh = Mesh::with_texture(puff);
    for k in 0..PUFFS {
        let life = ((t / 4.0 + k as f64 / PUFFS as f64 + seed as f64 * 0.37).rem_euclid(1.0)) as f32;
        let centre = top + vec2(w * (0.023 * life + 0.003 * (life * 9.0 + seed).sin()), -w * 0.067 * life);
        let r = w * (0.0025 + 0.0117 * life);
        let alpha = 0.45 * (1.0 - life) * (life * 6.0).min(1.0);
        let at = |u: f32, v: f32| map(centre + vec2((u * 2.0 - 1.0) * r, (v * 2.0 - 1.0) * r));
        let one = strip_mesh(puff, 1, tint.gamma_multiply(alpha), &at);
        mesh.append(one);
    }
    mesh
}

/// A night with a cloud, a bird whose wings beat, a chimney and a window, and a wing marked
/// with its shoulder far away.
#[cfg(test)]
pub const NIGHT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 800">
    <rect width="600" height="800" fill="#123"/>
    <g id="move-drift-1"><ellipse cx="150" cy="100" rx="80" ry="20" fill="#ccc"/></g>
    <g id="move-wing-1-300-200"><path d="M300,200 L200,150 L230,210Z" fill="#f00"/></g>
    <g id="move-wing-2-310-200"><path d="M310,200 L410,150 L380,210Z" fill="#f00"/></g>
    <g id="move-wing-3-50-700"><path d="M500,100 L560,80 L540,120Z" fill="#0f0"/></g>
    <g id="move-smoke-1-420-600"/>
    <rect id="move-flicker-1" x="400" y="650" width="10" height="10" fill="#fc0"/>
    <g id="move-walk-1"><rect x="10" y="700" width="5" height="20" fill="#fff"/></g>
</svg>"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_that_move_are_taken_out_of_the_still_picture() {
        let picture = layered(NIGHT, 600.0).unwrap();
        let motions: Vec<Motion> = picture.layers.iter().map(|l| l.motion).collect();
        assert!(matches!(motions.as_slice(), [Motion::Drift, Motion::Wing { .. }, Motion::Wing { .. }, Motion::Smoke, Motion::Flicker]), "{motions:?}");
        let px = |x: usize, y: usize| picture.still.pixels[y * 600 + x];
        assert_eq!(px(150, 100), px(20, 20), "the cloud is not in the still picture");
        assert_eq!(px(240, 180), px(20, 20), "nor the wings");
        assert_ne!(px(540, 100), px(20, 20), "a wing marked far from its shoulder stays in it, still");
        assert_ne!(px(12, 710), px(20, 20), "and so does what moves in no way Caprice knows");
        // The cloud on its own, where it was.
        let cloud = &picture.layers[0];
        assert!((cloud.at.center().x - 0.25).abs() < 0.01 && (cloud.at.center().y - 0.125).abs() < 0.01, "{:?}", cloud.at);
        let image = cloud.image.as_ref().unwrap();
        assert!(image.pixels[image.size[1] / 2 * image.size[0] + image.size[0] / 2].a() > 200);
    }

    #[test]
    fn a_creatures_wings_beat_together_each_upward_on_its_own_side() {
        let picture = layered(NIGHT, 600.0).unwrap();
        let wings: Vec<(f32, f32)> = picture.layers.iter().filter_map(|l| match l.motion {
            Motion::Wing { side, phase, .. } => Some((side, phase)),
            _ => None,
        }).collect();
        assert_eq!(wings.len(), 2);
        assert_eq!((wings[0].0, wings[1].0), (1.0, -1.0), "the left wing turns up clockwise, the right one the other way");
        assert_eq!(wings[0].1, wings[1].1, "the same beat");
        // Up, the left wing's tip is higher than drawn.
        let page = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 800.0));
        let Motion::Wing { pivot, side, phase } = picture.layers[1].motion else { unreachable!() };
        let up = (0..100).map(|k| k as f64 / 100.0).find(|&t| wingbeat(t + phase as f64) > 0.99).unwrap();
        let tip = pose(Motion::Wing { pivot, side, phase }, 1.0, up, page).apply(pos2(200.0, 150.0));
        assert!(tip.y < 150.0 - 30.0, "{tip:?}");
    }

    #[test]
    fn a_picture_with_nothing_marked_is_all_still() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 85"><circle cx="30" cy="40" r="20" fill="#36c"/></svg>"##;
        let picture = layered(svg, 600.0).unwrap();
        assert!(picture.layers.is_empty());
        assert_eq!(picture.still.pixels, crate::backdrop::rasterize(svg, 600.0).unwrap().pixels);
    }

    #[test]
    fn ids_name_their_motion_number_and_point() {
        assert_eq!(parse_id("move-wing-1-310-240"), Some(("wing", 1.0, Some((310.0, 240.0)))));
        assert_eq!(parse_id("move-drift-3"), Some(("drift", 3.0, None)));
        assert_eq!(parse_id("move-wing-1"), None, "a wing needs its shoulder");
        assert_eq!(parse_id("move-drift-1-2-3-4"), None);
        assert_eq!(parse_id("cloud-1"), None);
    }
}
