//! Scenes that move a little. Claude paints a scene still, but puts each part that could move in
//! a group of its own, named by how it moves (`move-wing-1-310-240`: a wing beating about the
//! shoulder at 310,240). The picture is rendered once without those parts, and each part on its
//! own; Caprice then moves the parts every frame, as textures, which costs next to nothing. Smoke
//! is Caprice's own: Claude only marks the chimney tops. The world moves, never the people: a part
//! that does not fit one of the few kinds of movement stays in the still picture, as does one
//! marked wrongly (a wing whose shoulder is nowhere near it).
//!
//! A theme of the writer's own (`own.rs`) moves its desk the same way, with a few more kinds a
//! picture that stays on the screen for hours wants: things that float, twinkle, glow, wander or
//! turn round, and areas Caprice fills with what falls or rises there (snow, rain, fireflies,
//! leaves, bubbles, embers), as it makes the smoke.

use std::f64::consts::TAU;

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Shape, Stroke, TextureHandle, TextureOptions, Vec2, epaint::Mesh, pos2, vec2};

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
/// How far a wing turns up or down at most in a beat, and how far up it is held gliding, as a
/// share of that.
const WING_TURN: f32 = 12.0;
const GLIDE: f64 = 0.25;
/// Seconds a stroke takes for the smallest wings and the largest; in between, by their span.
const QUICKEST_BEAT: f64 = 0.8;
const SLOWEST_BEAT: f64 = 3.0;
/// Most strokes in a burst of beating, and the least seconds of gliding between bursts.
const MOST_STROKES: f64 = 3.0;
const LEAST_GLIDE: f64 = 15.0;

/// How a part of a picture moves. Points are shares of the picture's width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Drift,
    /// `side` is 1 for a wing reaching out to the left of its shoulder, -1 to the right; `phase`
    /// and `beat` (seconds a stroke) are shared by the wings of one creature.
    Wing { pivot: Pos2, side: f32, phase: f32, beat: f32 },
    Sway { foot: Pos2 },
    Smoke,
    Flicker,
    Shimmer,
    /// Bobbing gently up and down: a boat, a lantern, a balloon.
    Float,
    /// A star coming and going, slowly and unevenly.
    Twinkle,
    /// A glow growing brighter and dimmer.
    Pulse,
    /// Going round a small loop of its own: a fish, a butterfly.
    Wander,
    /// Turning round and round about `pivot`: a windmill's sails, a water wheel.
    Spin { pivot: Pos2 },
    /// Caprice's own, like smoke: what falls or rises in the area marked, in the marker's colour.
    Particles { kind: Particle, color: Color32 },
}

/// What fills an area marked for particles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Particle {
    Snow,
    Rain,
    Fireflies,
    Leaves,
    Bubbles,
    Embers,
}

impl Particle {
    fn named(kind: &str) -> Option<Particle> {
        Some(match kind {
            "snow" => Particle::Snow,
            "rain" => Particle::Rain,
            "fireflies" => Particle::Fireflies,
            "leaves" => Particle::Leaves,
            "bubbles" => Particle::Bubbles,
            "embers" => Particle::Embers,
            _ => return None,
        })
    }

    /// Their colour when the marker has none to give.
    fn colour(self) -> Color32 {
        match self {
            Particle::Snow => Color32::from_rgb(248, 250, 255),
            Particle::Rain => Color32::from_rgb(200, 214, 232),
            Particle::Fireflies => Color32::from_rgb(222, 255, 140),
            Particle::Leaves => Color32::from_rgb(214, 128, 52),
            Particle::Bubbles => Color32::from_rgb(220, 244, 255),
            Particle::Embers => Color32::from_rgb(255, 170, 70),
        }
    }
}

/// A part that moves: how, a number to set it apart from the others, where it is in the picture
/// (shares of its size; for smoke, the chimney top), the part rendered (none for smoke and
/// particles), and its id, for a theme's checks to name it.
pub struct Layer {
    pub motion: Motion,
    pub seed: f32,
    pub at: Rect,
    pub image: Option<ColorImage>,
    pub id: String,
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
    let needs_point = matches!(kind, "wing" | "sway" | "smoke" | "spin");
    (bits.next().is_none() && needs_point == point.is_some()).then_some((kind, n, point))
}

/// Render `svg` `width` pixels wide, its moving parts apart from the rest. A part that cannot be
/// found, rendered or trusted stays in the still picture.
pub fn layered(svg: &str, width: f32) -> Result<Layered, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("the picture could not be read: {e}"))?;
    // Illustrator writes a DTD of entities, which usvg reads.
    let Ok(xml) = roxmltree::Document::parse_with_options(svg, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }) else { return crate::backdrop::rasterize(svg, width).map(Layered::still) };
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
    // The shoulders found, by creature; and each wing's layer, creature and span.
    let mut shoulders: Vec<(Pos2, usize)> = Vec::new();
    let mut wings: Vec<(usize, usize, f32)> = Vec::new();
    let moving = |n: &roxmltree::Node| n.is_element() && n.attribute("id").is_some_and(|id| id.starts_with("move-"));
    for node in xml.descendants().filter(moving) {
        // A part inside another moves with it; one in <defs> is not drawn where it stands.
        if node.ancestors().skip(1).any(|a| moving(&a) || a.has_tag_name("defs")) {
            continue;
        }
        let id = node.attribute("id").unwrap_or_default();
        let Some((kind, seed, given)) = parse_id(id) else { continue };
        let point = given.map(|(x, y)| share(x, y));
        if kind == "smoke" {
            if let Some(p) = point.filter(|p| (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y)) {
                layers.push(Layer { motion: Motion::Smoke, seed, at: Rect::from_min_max(p, p), image: None, id: id.to_owned() });
                cut.push(node.range());
            }
            continue;
        }
        let Some(part) = tree.node_by_id(id) else { continue };
        let Some(bb) = part.abs_layer_bounding_box() else { continue };
        let at = Rect::from_min_max(pos2(bb.x() / size.width(), bb.y() / size.height()), pos2(bb.right() / size.width(), bb.bottom() / size.height()));
        // An area for particles is only a marker: Caprice draws what fills it, in its colour.
        if let Some(particle) = Particle::named(kind) {
            let color = fill_of(part).unwrap_or(particle.colour());
            layers.push(Layer { motion: Motion::Particles { kind: particle, color }, seed, at, image: None, id: id.to_owned() });
            cut.push(node.range());
            continue;
        }
        // A shoulder or foot is on its part's edge, or near it.
        let margin = vec2(NEAR_PART, NEAR_PART * size.width() / size.height());
        let near = |p: Pos2| at.expand2(margin).contains(p);
        let parent = match part {
            usvg::Node::Group(g) => g.transform().invert().map(|own| g.abs_transform().pre_concat(own)),
            _ => Some(part.abs_transform()),
        };
        let Some(parent) = parent else { continue };
        // The point in the drawing's units, as Claude writes it; or, if that is nowhere near the part,
        // in the units of the group it is drawn in, or its own (a hand-made theme's wing written
        // as 0,0 inside its bird's `translate`): whichever lies on the part.
        let point = given.and_then(|(x, y)| {
            let on = |t: tiny_skia::Transform| {
                let mut p = tiny_skia::Point::from_xy(x, y);
                t.map_point(&mut p);
                pos2(p.x / size.width(), p.y / size.height())
            };
            let fits = |p: &Pos2| if kind == "spin" { at.contains(*p) } else { near(*p) };
            let candidates = [point, Some(on(parent)), Some(on(part.abs_transform()))];
            candidates.into_iter().flatten().find(fits).or(point)
        });
        let mut wing = None;
        let motion = match (kind, point) {
            ("drift", _) => Motion::Drift,
            ("flicker", _) => Motion::Flicker,
            ("shimmer", _) => Motion::Shimmer,
            ("float", _) => Motion::Float,
            ("twinkle", _) => Motion::Twinkle,
            ("pulse", _) => Motion::Pulse,
            ("wander", _) => Motion::Wander,
            ("sway", Some(foot)) if near(foot) => Motion::Sway { foot },
            // A wheel turns about a point inside it.
            ("spin", Some(pivot)) if at.contains(pivot) => Motion::Spin { pivot },
            ("wing", Some(pivot)) if near(pivot) => {
                let side = if at.center().x < pivot.x { 1.0 } else { -1.0 };
                let close = |&&(s, _): &&(Pos2, usize)| (s.x - pivot.x).hypot((s.y - pivot.y) * size.height() / size.width()) < ONE_CREATURE;
                let creature = shoulders.iter().find(close).map_or(shoulders.len(), |&(_, c)| c);
                shoulders.push((pivot, creature));
                wing = Some((creature, at.width().hypot(at.height() * size.height() / size.width())));
                Motion::Wing { pivot, side, phase: creature as f32 * 0.37 % 1.0, beat: 0.0 }
            }
            _ => continue,
        };
        // The part rendered alone, cropped to where it is, in its place under its groups.
        let (x0, y0) = ((bb.x() * scale).floor().max(0.0), (bb.y() * scale).floor().max(0.0));
        let (x1, y1) = ((bb.right() * scale).ceil().min(w as f32), (bb.bottom() * scale).ceil().min(h as f32));
        let Some(mut pixmap) = tiny_skia::Pixmap::new((x1 - x0) as u32, (y1 - y0) as u32) else { continue };
        let place = tiny_skia::Transform::from_translate(-x0, -y0).pre_scale(scale, scale).pre_concat(parent).pre_translate(bb.x(), bb.y());
        if resvg::render_node(part, place, &mut pixmap.as_mut()).is_none() {
            continue;
        }
        let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
        let at = Rect::from_min_max(pos2(x0 / w as f32, y0 / h as f32), pos2(x1 / w as f32, y1 / h as f32));
        layers.push(Layer { motion, seed, at, image: Some(image), id: id.to_owned() });
        if let Some((creature, span)) = wing {
            wings.push((layers.len() - 1, creature, span));
        }
        cut.push(node.range());
    }
    // A creature beats as slowly as its largest wing: a dragon's stroke takes seconds, a sparrow's
    // under one.
    for &(l, creature, _) in &wings {
        let span = wings.iter().filter(|w| w.1 == creature).map(|w| w.2).fold(0.0, f32::max) as f64;
        if let Motion::Wing { beat, .. } = &mut layers[l].motion {
            *beat = (QUICKEST_BEAT + 8.0 * span).min(SLOWEST_BEAT) as f32;
        }
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

/// The colour a marker is filled with: its own, or its first shape's.
fn fill_of(node: &resvg::usvg::Node) -> Option<Color32> {
    use resvg::usvg::{Node, Paint};
    match node {
        Node::Path(p) => match p.fill()?.paint() {
            Paint::Color(c) => Some(Color32::from_rgb(c.red, c.green, c.blue)),
            _ => None,
        },
        Node::Group(g) => g.children().iter().find_map(fill_of),
        _ => None,
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

/// How far up a wing is at `t` seconds (1 a full stroke up, -1 down), for a creature with `phase`
/// taking `beat` seconds a stroke; and how much it is beating rather than gliding, from 0 to 1.
/// Like a large bird it mostly glides, wings held a little up and shifting slowly, and now and
/// then beats two or three strokes, easing in and out of them: steady beating, and turning
/// sharply at the top and bottom, would draw the eye away from the writing. The bursts come at
/// uneven times, each creature on its own.
fn wing_lift(t: f64, beat: f64, phase: f64) -> (f64, f64) {
    let burst = MOST_STROKES * beat;
    let cycle = burst + LEAST_GLIDE + 10.0 * phase;
    let t_cycle = t + phase * cycle;
    let k = (t_cycle / cycle).floor();
    let random = |x: f64| ((x * 12.9898 + phase * 78.233).sin() * 43758.5453).rem_euclid(1.0);
    let strokes = if random(k) < 0.5 { 2.0 } else { 3.0 };
    let u = (t_cycle - k * cycle - random(k + 0.5) * (cycle - burst)) / beat;
    let ease = |x: f64| { let x = x.clamp(0.0, 1.0); x * x * (3.0 - 2.0 * x) };
    let beating = if (0.0..strokes).contains(&u) { ease(u.min(strokes - u) / 0.5) } else { 0.0 };
    let soaring = 0.1 * (TAU * t / 7.0 + TAU * phase).sin();
    // Down first, from the glide.
    (GLIDE + soaring - beating * (TAU * u).sin(), beating)
}

/// Where `motion` has a part at `t` seconds on a page `size` large, its points in page units.
fn pose(motion: Motion, seed: f32, t: f64, page: Rect) -> Pose {
    let (seed, w) = (seed as f64, page.width());
    let wave = |period: f64, offset: f64| (TAU * t / period + offset).sin() as f32;
    let at = |p: Pos2| page.min + p.to_vec2() * page.size();
    match motion {
        // Clouds take over a minute to go and come back, a few points either way.
        Motion::Drift => Pose { shift: vec2(0.03 * w * wave(70.0 + 11.0 * (seed % 4.0), seed), 0.0), ..Pose::STILL },
        Motion::Wing { pivot, side, phase, beat } => {
            let (lift, beating) = wing_lift(t, beat as f64, phase as f64);
            // Seen edge on in the middle of a stroke, the wing is a little shorter.
            let squash = 1.0 - 0.1 * beating * (1.0 - (lift - GLIDE).abs().min(1.0));
            Pose { pivot: at(pivot), angle: side * WING_TURN.to_radians() * lift as f32, squash: squash as f32, ..Pose::STILL }
        }
        Motion::Sway { foot } => Pose { pivot: at(foot), angle: 1.2f32.to_radians() * wave(5.0 + seed % 3.0, seed), ..Pose::STILL },
        Motion::Flicker => Pose { alpha: 0.75 + 0.25 * ((t * 7.0 + seed * 1.7).sin() * (t * 3.1 + seed).sin()) as f32, ..Pose::STILL },
        Motion::Shimmer => Pose { shift: vec2(0.004 * w * wave(3.2, seed), 0.0), alpha: 0.7 + 0.3 * wave(2.3, seed * 1.3), ..Pose::STILL },
        Motion::Float => Pose { shift: vec2(0.0015 * w * wave(9.0 + seed % 3.0, seed * 0.7), 0.005 * w * wave(5.0 + seed % 2.0, seed)), ..Pose::STILL },
        // Two slow waves at odd periods, so no two stars keep time.
        Motion::Twinkle => {
            let a = 0.5 + 0.5 * wave(2.9 + seed % 1.7, seed) * wave(4.3 + seed % 2.3, seed * 2.1);
            Pose { alpha: 0.35 + 0.65 * a, ..Pose::STILL }
        }
        Motion::Pulse => Pose { alpha: 0.72 + 0.28 * wave(4.5 + seed % 2.0, seed), ..Pose::STILL },
        Motion::Wander => Pose { shift: vec2(0.012 * w * wave(13.0 + seed % 5.0, seed), 0.006 * w * wave(8.0 + seed % 3.0, seed * 1.7)), ..Pose::STILL },
        // Once round in half a minute or so, the same way for every wheel.
        Motion::Spin { pivot } => Pose { pivot: at(pivot), angle: (TAU * t / (24.0 + 6.0 * (seed % 3.0))) as f32, ..Pose::STILL },
        Motion::Smoke | Motion::Particles { .. } => Pose::STILL,
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

    /// How many of its parts move.
    pub fn parts(&self) -> usize {
        self.parts.len()
    }

    /// The part moving as a whole whose place holds `at` (shares of the picture), the smallest if
    /// several do: what is drawn on it (a coconut on a palm) should move with it.
    pub fn holding(&self, at: Pos2) -> Option<usize> {
        let size = |r: Rect| r.width() * r.height();
        self.parts.iter().enumerate().filter(|(_, p)| p.texture.is_some() && p.at.contains(at)).min_by(|a, b| size(a.1.at).total_cmp(&size(b.1.at))).map(|(i, _)| i)
    }

    /// Where part `i` of the picture lying over `rect` takes a point at `time`.
    pub fn moved(&self, i: usize, rect: Rect, time: f64) -> impl Fn(Pos2) -> Pos2 + use<> {
        let pose = self.parts.get(i).map_or(Pose::STILL, |p| pose(p.motion, p.seed, time, rect));
        move |q| pose.apply(q)
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
                (None, Some(t)) => {
                    if let Motion::Particles { kind, color } = part.motion {
                        let area = Rect::from_min_max(page(part.at.min), page(part.at.max));
                        shapes.extend(particles(kind, color * tint, area, rect.width(), part.seed, t, map));
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

/// What fills an `area` marked for particles at `t` seconds, in a picture `w` wide. Each is
/// placed by a share of the way across and down the area, never the time multiplied by a size,
/// so a window made larger moves none of them at a stroke.
fn particles(kind: Particle, color: Color32, area: Rect, w: f32, seed: f32, t: f64, map: &dyn Fn(Pos2) -> Pos2) -> Vec<Shape> {
    let u = (w / 1600.0).max(0.4);
    // As many as fit the area without crowding it, and never so many a frame grows slow.
    let room = (area.width() * area.height()) / (u * u);
    let (spacing, most) = match kind {
        Particle::Snow => (60.0, 160.0),
        Particle::Rain => (50.0, 160.0),
        Particle::Fireflies => (140.0, 40.0),
        Particle::Leaves => (150.0, 28.0),
        Particle::Bubbles => (110.0, 40.0),
        Particle::Embers => (100.0, 50.0),
    };
    let n = (room.sqrt() / spacing * room.sqrt() / spacing).clamp(4.0, most) as usize;
    let t = t as f32;
    let rand = |i: usize, k: u32| ((i as f32 * 12.9898 + k as f32 * 78.233 + seed * 37.719).sin() * 43_758.547).rem_euclid(1.0);
    let at = |x: f32, y: f32| map(pos2(area.left() + area.width() * x.rem_euclid(1.0), area.top() + area.height() * y.rem_euclid(1.0)));
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let (r0, r1, r2, near) = (rand(i, 0), rand(i, 1), rand(i, 2), rand(i, 3));
        match kind {
            Particle::Snow => {
                let p = at(r0 + 0.012 * (t * (0.3 + 0.5 * r2) + i as f32).sin(), r1 + t * (0.03 + 0.05 * near));
                out.push(Shape::circle_filled(p, (0.9 + 1.9 * near) * u, color.gamma_multiply(0.5 + 0.4 * near)));
            }
            Particle::Rain => {
                let p = at(r0 - t * 0.04, r1 + t * (0.7 + 0.5 * near));
                let len = (10.0 + 16.0 * near) * u;
                out.push(Shape::line_segment([p, p + vec2(-0.18 * len, len)], Stroke::new((0.7 + 0.6 * near) * u, color.gamma_multiply(0.25 + 0.3 * near))));
            }
            Particle::Fireflies => {
                let p = at(r0 + 0.05 * (t * (0.15 + 0.2 * r2) + i as f32).sin(), r1 + 0.06 * (t * (0.11 + 0.15 * near) + i as f32 * 1.3).cos());
                let blink = (t * (0.4 + 0.5 * r2) + i as f32 * 2.3).sin().max(0.0).powi(2);
                out.push(Shape::circle_filled(p, 10.0 * u, color.gamma_multiply(0.12 * blink)));
                out.push(Shape::circle_filled(p, 5.0 * u, color.gamma_multiply(0.22 * blink)));
                out.push(Shape::circle_filled(p, 2.2 * u, color.gamma_multiply(0.3 + 0.7 * blink)));
            }
            Particle::Leaves => {
                let p = at(r0 + 0.05 * (t * (0.4 + 0.3 * r2) + i as f32).sin(), r1 + t * (0.035 + 0.035 * near));
                let (len, turn) = ((5.0 + 4.0 * near) * u, t * (0.8 + 1.2 * r2) + i as f32);
                let dir = Vec2::angled(turn);
                let side = vec2(-dir.y, dir.x) * len * 0.45 * (turn * 0.7).sin().abs().max(0.2);
                let points = (0..10).map(|k| {
                    let a = k as f32 / 10.0 * TAU as f32;
                    p + dir * len * a.cos() + side * a.sin()
                }).collect();
                out.push(Shape::convex_polygon(points, color.lerp_to_gamma(Color32::from_rgb(150, 60, 30), r2 * 0.5), Stroke::NONE));
            }
            Particle::Bubbles => {
                let life = (r1 - t * (0.05 + 0.07 * near)).rem_euclid(1.0);
                let p = at(r0 + 0.01 * (t * 1.3 + i as f32).sin(), life);
                let fade = (life * 5.0).min(1.0);
                out.push(Shape::circle_stroke(p, (1.5 + 3.5 * near) * u, Stroke::new(0.9 * u, color.gamma_multiply(0.6 * fade))));
            }
            Particle::Embers => {
                let life = (r1 - t * (0.06 + 0.08 * near)).rem_euclid(1.0);
                let p = at(r0 + 0.03 * (t * (0.6 + r2) + i as f32).sin(), life);
                let glow = life * (0.6 + 0.4 * (t * 9.0 + i as f32).sin().abs());
                out.push(Shape::circle_filled(p, (0.8 + 1.4 * near) * u, color.gamma_multiply(glow)));
            }
        }
    }
    out
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
        let Motion::Wing { beat, phase, .. } = picture.layers[1].motion else { unreachable!() };
        let up = (0..6000).map(|k| k as f64 / 100.0).find(|&t| wing_lift(t, beat as f64, phase as f64).0 > 1.0).unwrap();
        let tip = pose(picture.layers[1].motion, 1.0, up, page).apply(pos2(200.0, 150.0));
        assert!(tip.y < 150.0 - 12.0, "{tip:?}");
    }

    #[test]
    fn large_wings_beat_slowly_now_and_then_and_mostly_glide() {
        // A sparrow and a dragon.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 800">
            <g id="move-wing-1-100-100"><path d="M100,100 L85,92 L90,104Z"/></g>
            <g id="move-wing-2-300-400"><path d="M300,400 L60,250 L120,430Z"/></g>
        </svg>"##;
        let picture = layered(svg, 600.0).unwrap();
        let page = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 800.0));
        let beats: Vec<f32> = picture.layers.iter().map(|l| match l.motion { Motion::Wing { beat, .. } => beat, _ => 0.0 }).collect();
        assert!(beats[0] < 1.2 && beats[1] >= 2.5, "{beats:?}");
        for layer in &picture.layers {
            let Motion::Wing { beat, phase, .. } = layer.motion else { unreachable!() };
            let angle = |t: f64| pose(layer.motion, 1.0, t, page).angle.to_degrees();
            let frames: Vec<f64> = (0..120 * 60).map(|k| k as f64 / 60.0).collect();
            let beating = frames.iter().filter(|&&t| wing_lift(t, beat as f64, phase as f64).1 > 0.0).count();
            assert!(beating > 0, "it beats now and then");
            assert!(beating * 10 < frames.len() * 4, "but glides most of the time: {beating} of {}", frames.len());
            let fastest = frames.windows(2).map(|w| (angle(w[1]) - angle(w[0])).abs() * 60.0).fold(0.0, f32::max);
            let most = if beat >= 2.5 { 35.0 } else { 120.0 };
            assert!(fastest < most, "a wing of {beat} s a stroke turns gently, at most {fastest}° a second");
        }
    }

    #[test]
    fn a_picture_with_nothing_marked_is_all_still() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 85"><circle cx="30" cy="40" r="20" fill="#36c"/></svg>"##;
        let picture = layered(svg, 600.0).unwrap();
        assert!(picture.layers.is_empty());
        assert_eq!(picture.still.pixels, crate::backdrop::rasterize(svg, 600.0).unwrap().pixels);
    }

    #[test]
    fn a_theme_s_kinds_of_motion_are_read_and_its_particles_take_their_marker_s_colour() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 800">
            <rect width="600" height="800" fill="#123"/>
            <g id="move-spin-1-100-100"><rect x="60" y="95" width="80" height="10" fill="#ddd"/></g>
            <g id="move-spin-2-500-500"><rect x="60" y="195" width="80" height="10" fill="#ddd"/></g>
            <circle id="move-float-1" cx="300" cy="600" r="20" fill="#963"/>
            <circle id="move-twinkle-1" cx="50" cy="50" r="2" fill="#fff"/>
            <rect id="move-snow-1" x="0" y="0" width="300" height="400" fill="#ff0000"/>
            <rect id="move-rain-1" x="300" y="0" width="300" height="400" fill="none" stroke="none"/>
        </svg>"##;
        let picture = layered(svg, 600.0).unwrap();
        let ids: Vec<&str> = picture.layers.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["move-spin-1-100-100", "move-float-1", "move-twinkle-1", "move-snow-1", "move-rain-1"], "a wheel turns about a point inside it");
        assert!(matches!(picture.layers[3].motion, Motion::Particles { kind: Particle::Snow, color } if color == Color32::RED), "in the marker's colour");
        assert!(matches!(picture.layers[4].motion, Motion::Particles { kind: Particle::Rain, color } if color == Particle::Rain.colour()), "or their own");
        let px = |x: usize, y: usize| picture.still.pixels[y * 600 + x];
        assert_eq!(px(200, 300), Color32::from_rgb(0x11, 0x22, 0x33), "the marker is not drawn");
        // Turning round: a quarter of the way after a quarter of its time (half a minute for this one).
        let page = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 800.0));
        let spin = pose(picture.layers[0].motion, 1.0, 7.5, page);
        assert!((spin.angle - std::f32::consts::FRAC_PI_2).abs() < 0.01 && spin.pivot == pos2(100.0, 100.0), "{}", spin.angle);
        let drawn = particles(Particle::Snow, Color32::WHITE, Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0)), 600.0, 1.0, 3.0, &|p| p);
        let later = particles(Particle::Snow, Color32::WHITE, Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0)), 600.0, 1.0, 4.0, &|p| p);
        assert!(drawn.len() >= 4 && drawn.iter().all(|s| Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0)).expand(10.0).contains(s.visual_bounding_rect().center())), "in their area");
        assert!(drawn.iter().zip(&later).any(|(a, b)| a.visual_bounding_rect().center().y != b.visual_bounding_rect().center().y), "falling");
    }

    #[test]
    fn a_point_may_be_written_in_the_units_of_the_group_its_part_is_drawn_in() {
        // A bird moved into place as a whole, its wing's shoulder written as where it is in the bird.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 800">
            <g transform="translate(300 200)"><ellipse rx="40" ry="15" fill="#f80"/>
                <path id="move-wing-1-0-0" d="M-10,5 Q-40,-40 0,-50 Q0,-20 8,4Z" fill="#088"/></g>
            <g id="move-sway-1-0-0" transform="translate(100 700)"><rect x="-5" y="-80" width="10" height="80" fill="#630"/></g>
        </svg>"##;
        let picture = layered(svg, 600.0).unwrap();
        let motions: Vec<Motion> = picture.layers.iter().map(|l| l.motion).collect();
        assert!(matches!(motions[0], Motion::Wing { pivot, .. } if (pivot.x - 0.5).abs() < 0.01 && (pivot.y - 0.25).abs() < 0.01), "{motions:?}");
        assert!(matches!(motions[1], Motion::Sway { foot } if (foot.x - 100.0 / 600.0).abs() < 0.01 && (foot.y - 700.0 / 800.0).abs() < 0.01), "in its own units: {motions:?}");
    }

    #[test]
    fn what_lies_on_a_moving_part_finds_it_and_moves_with_it() {
        let picture = layered(NIGHT, 600.0).unwrap();
        let ctx = egui::Context::default();
        let drawn = Drawn::load(&ctx, picture);
        let cloud = drawn.holding(pos2(0.25, 0.125)).expect("on the cloud");
        assert_eq!(drawn.holding(pos2(0.5, 0.9)), None, "nothing moves there");
        let page = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 800.0));
        let (a, b) = (drawn.moved(cloud, page, 0.0)(pos2(150.0, 100.0)), drawn.moved(cloud, page, 20.0)(pos2(150.0, 100.0)));
        assert!((a.x - b.x).abs() > 1.0 && a.y == b.y, "drifting with the cloud: {a:?} {b:?}");
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
