//! Themes of the writer's own. A theme is one SVG drawing, made in any drawing program (or by
//! hand, or by an AI chat told the format), imported from a file. Three groups in it, found by
//! their id or by their layer's name:
//!
//! - `desk`, the picture the pages lie on: it covers the whole desk whatever the window's shape,
//!   cut to fit, held at its foot (`Anchor`). It must move: parts of it are marked as in a scene
//!   (`move-drift-1`, `move-sway-2-310-240`, `move-snow-1`, … see `motion.rs`), and a drawing with
//!   fewer than two is refused.
//! - `piece` (or several, `piece-1`, `piece-2`, …), optional: what grows beside the pages, one for
//!   each chapter, a stage a page (its parts in the order drawn, or groups `stage-1`, `stage-2`, …),
//!   and its `done` group shown once the next chapter begins.
//! - `carrier`, optional: what flies a new chapter title in, holding it at its `hook` (a small mark
//!   inside it), or at its foot.
//!
//! Settings go as JSON in `<metadata id="caprice">` (`Meta`): a name, a few words, colours to
//! use rather than the ones taken from the picture, how the pieces stand. The rest of the theme's
//! palette comes from the picture itself (`palette_of`), and is made readable whatever it is.
//!
//! A drawing is checked when it is imported (`check`), with the reasons in plain words when it is
//! refused, and kept on the shelf of themes (`Shelf`) in the data folder: the drawing as it was
//! made readable (layer names turned into ids, links out of the file taken away), a little picture
//! for the gallery, and what the gallery shows of it. Drawing it on the desk is `eggs::own`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::platform::fs;
use crate::theme::{Palette, Theme, pal};

/// The largest drawing taken, and the most elements in it: more could not be drawn smoothly.
pub const MOST_BYTES: usize = 8 * 1024 * 1024;
const MOST_ELEMENTS: usize = 60_000;
/// The most stages a piece grows in; a piece of more parts groups them into as many.
pub const MOST_STAGES: usize = 24;
/// The fewest parts of the desk that must move.
pub const LEAST_MOVING: usize = 2;
/// How wide the gallery's little picture of a theme is, in pixels.
const THUMB_WIDTH: f32 = 320.0;

/// A template to start from: a theme with everything in it, explained inside.
pub const TEMPLATE: &str = include_str!("../assets/theme-template.svg");

/// What the theme's maker says about it. Everything may be left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub name: String,
    pub about: String,
    pub author: String,
    /// Colours to use rather than those taken from the picture, by the palette's names
    /// (`"accent": "#e0604a"`, see `NAMES`); and `"light": "yes"` or `"no"` for egui's widgets.
    pub palette: BTreeMap<String, String>,
    pub anchor: Anchor,
    pub growth: Growth,
    pub carrier: CarrierLook,
}

/// Where the picture is held when it is cut to fit the desk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    /// Its foot on the foot of the desk: the sky is cut before the ground.
    #[default]
    Bottom,
    Middle,
    Top,
}

/// How the pieces stand on the desk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Growth {
    pub layout: Layout,
    /// How tall a piece is, as a share of the picture's height.
    pub size: f32,
    /// Where on the picture the pieces stand, in its own units (the y of its ground); by default a
    /// little above the foot of the desk.
    pub ground: Option<f32>,
    /// The pieces on the right of the page mirrored, to face it.
    pub mirror: bool,
    /// How a stage comes.
    pub arrive: Arrive,
    /// Drawn in place: one piece for the whole story, or one growing for each chapter.
    pub each: Each,
    /// Which piece a chapter has: each in turn, or one at random.
    pub order: Order,
    /// What happens to a chapter's piece when the next begins.
    pub finish: Finish,
}

impl Default for Growth {
    fn default() -> Self {
        Self { layout: Layout::Row, size: 0.26, ground: None, mirror: false, arrive: Arrive::Light, each: Each::Story, order: Order::Turn, finish: Finish::Done }
    }
}

/// With `Layout::Drawn`, what a piece grows for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Each {
    /// The whole story: every page adds a stage.
    #[default]
    Story,
    /// Its chapter: the chapter's piece (a tree) grows a stage a page, where it is drawn.
    Chapter,
}

/// Which piece each chapter has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    /// piece-1, piece-2, … in turn.
    #[default]
    Turn,
    /// Any, at random, never the one the chapter before had.
    Random,
}

/// What a chapter's piece does once the next chapter begins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    /// The stages it had no pages for come in, and its `done` shows.
    #[default]
    Done,
    /// Its stages fall to the ground, one after another, and lie there (coconuts off a palm).
    Fall,
}

/// How a stage of a piece comes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arrive {
    /// Out of a little light, as the ship's parts.
    #[default]
    Light,
    /// Dropping from above and bouncing where it lands: a coconut, an apple, a snowball.
    Fall,
    /// Rising up out of the ground from its foot: a flower, a house.
    Grow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Side by side out from the page, taking turns left and right, further rows behind.
    #[default]
    Row,
    /// About the lower part of the desk beside the page, the higher ones further away.
    Scatter,
    /// Where it is drawn on the desk, one for the whole story, a stage for every page of it, its
    /// `done` once every stage has come.
    Drawn,
}

/// How the carrier is drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CarrierLook {
    /// The way it faces as drawn; it is mirrored to face the way it flies.
    pub faces: Faces,
    /// Larger or smaller than the balloon's size.
    pub size: f32,
}

impl Default for CarrierLook {
    fn default() -> Self {
        Self { faces: Faces::Left, size: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Faces {
    #[default]
    Left,
    Right,
}

/// A part of the drawing on its own: a standalone SVG, and its size in its own units.
#[derive(Clone, Debug)]
pub struct Art {
    pub svg: String,
    pub size: Vec2,
}

/// What grows: its stages, each on its own in the frame of the whole piece, and its finish; and
/// where it is drawn on the desk, as shares of the desk's size.
#[derive(Clone, Debug)]
pub struct Piece {
    pub stages: Vec<Art>,
    pub done: Option<Art>,
    pub size: Vec2,
    pub at: Rect,
    /// It had more parts than `MOST_STAGES`, shared out among as many stages.
    pub shared: bool,
}

/// What flies a title in, and where in it the title hangs (shares of its size).
#[derive(Clone, Debug)]
pub struct Carrier {
    pub art: Art,
    pub hook: Pos2,
}

/// A theme read: its drawing, its settings, and its parts on their own.
#[derive(Clone, Debug)]
pub struct Pack {
    pub svg: String,
    pub meta: Meta,
    pub desk: Art,
    pub pieces: Vec<Piece>,
    pub carrier: Option<Carrier>,
    /// Where the pieces stand, as a share of the desk picture's height.
    pub ground: Option<f32>,
}

/// A drawing that passed: the theme, what was found in it, and what was noticed about it.
pub struct Checked {
    pub pack: Pack,
    pub found: Vec<String>,
    pub notes: Vec<String>,
}

/// The palette's names, as the settings and the shelf write them.
pub const NAMES: [&str; 14] =
    ["desk", "desk_top", "dock", "dock_edge", "button", "button_hover", "text", "text_hot", "text_dim", "accent", "on_accent", "field", "caret", "thumb"];

fn slots(p: &mut Palette) -> [&mut Color32; 14] {
    [
        &mut p.desk, &mut p.desk_top, &mut p.dock, &mut p.dock_edge, &mut p.btn, &mut p.btn_hover, &mut p.text, &mut p.text_hot,
        &mut p.text_dim, &mut p.accent, &mut p.on_accent, &mut p.field, &mut p.caret, &mut p.thumb,
    ]
}

/// `#rrggbb` (or `#rgb`) as a colour.
pub fn hex(s: &str) -> Option<Color32> {
    let s = s.trim().strip_prefix('#')?;
    let digits: Vec<u8> = match s.len() {
        3 => s.chars().map(|c| c.to_digit(16).map(|d| d as u8 * 17)).collect::<Option<_>>()?,
        6 => (0..3).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect::<Option<_>>()?,
        _ => return None,
    };
    Some(Color32::from_rgb(digits[0], digits[1], digits[2]))
}

fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// The palette as names and colours, for the shelf.
pub fn colours(p: &Palette) -> BTreeMap<String, String> {
    let mut p = *p;
    let light = p.light;
    let mut map: BTreeMap<String, String> = NAMES.iter().zip(slots(&mut p)).map(|(n, c)| (n.to_string(), to_hex(*c))).collect();
    map.insert("light".into(), if light { "yes" } else { "no" }.into());
    map
}

/// `base` with the colours `map` names put in, and the names it could not read.
pub fn with_colours(base: Palette, map: &BTreeMap<String, String>) -> (Palette, Vec<String>) {
    let mut p = base;
    let mut unread = Vec::new();
    for (name, value) in map {
        if name == "light" {
            match value.as_str() {
                "yes" | "true" => p.light = true,
                "no" | "false" => p.light = false,
                _ => unread.push(name.clone()),
            }
            continue;
        }
        match (NAMES.iter().position(|n| n == name), hex(value)) {
            (Some(i), Some(c)) => *slots(&mut p)[i] = c,
            _ => unread.push(name.clone()),
        }
    }
    (p, unread)
}

// ---- Colours ---------------------------------------------------------------------------------

fn linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// How light a colour looks, from 0 (black) to 1 (white), as WCAG measures it.
pub fn luminance(c: Color32) -> f32 {
    0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
}

/// How far apart two colours are to read one on the other, from 1 to 21.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// `c` moved towards black or white, whichever reads better on `on`, until it reads at `least`.
fn readable(c: Color32, on: &[Color32], least: f32) -> Color32 {
    let worst = |c: Color32| on.iter().map(|&o| contrast(c, o)).fold(f32::MAX, f32::min);
    if worst(c) >= least {
        return c;
    }
    let to = if worst(Color32::BLACK) > worst(Color32::WHITE) { Color32::BLACK } else { Color32::WHITE };
    (1..=20).map(|k| mix(c, to, k as f32 / 20.0)).find(|&m| worst(m) >= least).unwrap_or(to)
}

/// Hue (0 to 1), saturation and value of a colour.
fn hsv(c: Color32) -> (f32, f32, f32) {
    let (r, g, b) = (c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

fn average(pixels: impl Iterator<Item = Color32>) -> Option<Color32> {
    let (mut sum, mut n) = ([0.0f32; 3], 0.0);
    for c in pixels.filter(|c| c.a() > 0) {
        let c = Color32::from_rgb(c.r(), c.g(), c.b());
        for (s, v) in sum.iter_mut().zip([c.r(), c.g(), c.b()]) {
            *s += v as f32;
        }
        n += 1.0;
    }
    (n > 0.0).then(|| Color32::from_rgb((sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8))
}

/// A palette for the desk `picture`: the desk's own colours at its foot and top, docks and windows
/// of its light or its dark, the accent its most vivid colour; then `overrides`, and then all of it
/// made readable (R29): text on the docks and buttons at 4.5:1, dim text at 3:1.
pub fn palette_of(picture: &ColorImage, overrides: &BTreeMap<String, String>) -> Palette {
    let [w, h] = picture.size;
    let px = |x: usize, y: usize| picture.pixels[y * w + x];
    let band = |from: f32, to: f32| average((((h as f32 * from) as usize)..((h as f32 * to) as usize).max(1)).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| px(x, y)));
    // What the maker chose is what the rest is made from: a dock of their own takes buttons to match.
    let chosen = |name: &str| overrides.get(name).and_then(|v| hex(v));
    let all = average(picture.pixels.iter().copied()).unwrap_or(Color32::from_gray(40));
    let desk = chosen("desk").or(band(0.8, 1.0)).unwrap_or(all);
    let desk_top = chosen("desk_top").or(band(0.0, 0.2)).unwrap_or(all);
    let light = match overrides.get("light").map(String::as_str) {
        Some("yes" | "true") => true,
        Some("no" | "false") => false,
        _ => chosen("dock").map_or(luminance(all) > 0.36, |d| luminance(d) > 0.3),
    };
    // The most vivid hue, by how much of it there is and how vivid it is.
    let mut bins = [(0.0f32, [0.0f32; 3], 0.0f32); 12];
    for c in picture.pixels.iter().filter(|c| c.a() > 0) {
        let (hue, s, v) = hsv(*c);
        let weight = s * s * v;
        if weight > 0.12 {
            let b = &mut bins[((hue * 12.0) as usize).min(11)];
            b.0 += weight;
            for (sum, x) in b.1.iter_mut().zip([c.r(), c.g(), c.b()]) {
                *sum += x as f32 * weight;
            }
            b.2 += weight;
        }
    }
    let base = if light { Theme::Typewriter.palette() } else { Theme::Midnight.palette() };
    let vivid = bins.iter().max_by(|a, b| a.0.total_cmp(&b.0)).filter(|b| b.0 > (w * h) as f32 * 0.002);
    let accent = chosen("accent").unwrap_or(vivid.map_or(base.accent, |b| Color32::from_rgb((b.1[0] / b.2) as u8, (b.1[1] / b.2) as u8, (b.1[2] / b.2) as u8)));
    let mut p = if light {
        let dock = chosen("dock").unwrap_or(mix(desk_top, Color32::WHITE, 0.82));
        Palette {
            desk,
            desk_top,
            dock,
            dock_edge: mix(dock, desk, 0.45),
            btn: mix(dock, desk, 0.2),
            btn_hover: mix(dock, desk, 0.34),
            text: mix(desk, Color32::BLACK, 0.82),
            text_hot: mix(desk, Color32::BLACK, 0.93),
            text_dim: mix(desk, Color32::BLACK, 0.55),
            accent,
            on_accent: Color32::WHITE,
            field: mix(dock, Color32::WHITE, 0.6),
            caret: mix(accent, Color32::BLACK, 0.2),
            thumb: mix(desk, accent, 0.45),
            light: true,
        }
    } else {
        let dock = chosen("dock").unwrap_or(mix(mix(desk, desk_top, 0.5), Color32::BLACK, 0.35));
        Palette {
            desk,
            desk_top,
            dock,
            dock_edge: mix(dock, Color32::WHITE, 0.16),
            btn: mix(dock, Color32::WHITE, 0.09),
            btn_hover: mix(dock, Color32::WHITE, 0.17),
            text: mix(desk_top, Color32::WHITE, 0.88),
            text_hot: Color32::WHITE,
            text_dim: mix(desk_top, Color32::WHITE, 0.58),
            accent,
            on_accent: mix(desk, Color32::BLACK, 0.7),
            field: mix(dock, Color32::BLACK, 0.25),
            caret: accent,
            thumb: mix(desk, accent, 0.55),
            light: false,
        }
    };
    p = with_colours(p, overrides).0;
    let (dock, btn) = (p.dock, p.btn);
    p.text = readable(p.text, &[dock, btn, p.btn_hover], 4.5);
    p.text_hot = readable(p.text_hot, &[dock, btn, p.btn_hover], 4.5);
    p.text_dim = readable(p.text_dim, &[dock, btn], 3.0);
    p.accent = readable(p.accent, &[dock], 3.0);
    p.on_accent = readable(p.on_accent, &[p.accent], 4.5);
    p.caret = readable(p.caret, &[crate::theme::PAPER], 3.0);
    p
}

// ---- Reading a drawing -----------------------------------------------------------------------

const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
const XLINK: &str = "http://www.w3.org/1999/xlink";

fn parse(svg: &str) -> Result<roxmltree::Document<'_>, roxmltree::Error> {
    // Illustrator writes a DTD of entities; usvg reads it, and so must we.
    roxmltree::Document::parse_with_options(svg, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
}

/// A name as a drawing program writes it, as Caprice reads it: Illustrator's escapes (`_x2D_` for a
/// hyphen) and the `_2_` it adds to a name used twice taken out, spaces as hyphens, small letters.
pub fn plain_name(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw.trim();
    while let Some(at) = rest.find("_x") {
        let code = rest.get(at + 2..at + 4).and_then(|h| u8::from_str_radix(h, 16).ok());
        match code.filter(|_| rest.get(at + 4..at + 5) == Some("_")) {
            Some(c) => {
                out.push_str(&rest[..at]);
                out.push(c as char);
                rest = &rest[at + 5..];
            }
            None => {
                out.push_str(&rest[..at + 2]);
                rest = &rest[at + 2..];
            }
        }
    }
    out.push_str(rest);
    // Illustrator's `_2_` for a name used twice.
    if let Some(stem) = out.strip_suffix('_').and_then(|s| s.rfind('_').filter(|&i| s[i + 1..].chars().all(|c| c.is_ascii_digit()) && i + 1 < s.len()).map(|i| s[..i].to_owned())) {
        out = stem;
    }
    out.split_whitespace().collect::<Vec<_>>().join("-").replace('_', "-").to_lowercase()
}

/// What a name means to Caprice, as it should be written, if anything.
fn role(name: &str) -> Option<String> {
    let n = plain_name(name);
    let numbered = |stem: &str| n.strip_prefix(stem).and_then(|r| r.strip_prefix('-')).is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
    match n.as_str() {
        "desk" | "backdrop" | "background" => Some("desk".into()),
        "piece" | "carrier" | "done" | "hook" => Some(n),
        _ if numbered("piece") || numbered("stage") => Some(n),
        _ if n.starts_with("move-") => Some(n),
        _ => None,
    }
}

/// Changes to a text, each a range and what goes there.
#[derive(Default)]
struct Edits(Vec<(usize, usize, String)>);

impl Edits {
    fn put(&mut self, at: std::ops::Range<usize>, with: impl Into<String>) {
        self.0.push((at.start, at.end, with.into()));
    }

    fn apply(mut self, text: &str) -> String {
        self.0.sort_by_key(|e| (std::cmp::Reverse(e.0), std::cmp::Reverse(e.1)));
        let mut out = text.to_owned();
        let mut below = usize::MAX;
        for (a, b, with) in self.0 {
            // One inside another (a part taken out, and a change inside it): the outer one is enough.
            if b > below {
                continue;
            }
            out.replace_range(a..b, &with);
            below = a;
        }
        out
    }
}

/// Where an element's start tag's name ends, to put an attribute after it.
fn after_name(svg: &str, node: roxmltree::Node) -> usize {
    let start = node.range().start + 1;
    start + svg[start..].find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(0)
}

/// The drawing made readable: the names of layers and groups (Inkscape's, Illustrator's, Figma's)
/// as ids, a moving part named twice made two, links out of the file taken away. And what was
/// noticed doing it.
fn normalize(svg: &str) -> Result<(String, Vec<String>), String> {
    let doc = parse(svg).map_err(|e| e.to_string())?;
    if !doc.root_element().has_tag_name("svg") {
        return Err("it is not an SVG drawing".into());
    }
    let mut edits = Edits::default();
    let mut notes = Vec::new();
    let (mut outside, mut texts, mut animated, mut count) = (0, 0, 0, 0);
    let mut moving: HashMap<String, usize> = HashMap::new();
    let mut fresh = 9000;
    for node in doc.descendants().filter(|n| n.is_element()) {
        count += 1;
        match node.tag_name().name() {
            "text" => texts += 1,
            "animate" | "animateTransform" | "animateMotion" | "set" => animated += 1,
            _ => {}
        }
        let id = node.attribute("id");
        let named = [id, node.attribute("data-name"), node.attribute((INKSCAPE, "label"))].into_iter().flatten().find_map(role);
        if let Some(mut name) = named {
            if let Some(rest) = name.strip_prefix("move-").map(str::to_owned) {
                let seen = moving.entry(name.clone()).or_insert(0);
                *seen += 1;
                if *seen > 1 {
                    // Two parts of one name would be one part moving twice: the second gets a number of its own.
                    let mut bits: Vec<String> = rest.split('-').map(str::to_owned).collect();
                    if bits.len() >= 2 {
                        fresh += 1;
                        bits[1] = fresh.to_string();
                        name = format!("move-{}", bits.join("-"));
                    }
                }
            }
            if id != Some(name.as_str()) {
                match node.attributes().find(|a| a.name() == "id" && a.namespace().is_none()) {
                    Some(a) => edits.put(a.range_value(), name),
                    None => {
                        let at = after_name(svg, node);
                        edits.put(at..at, format!(" id=\"{name}\""));
                    }
                }
            }
        }
        for a in node.attributes().filter(|a| a.name() == "href" && (a.namespace().is_none() || a.namespace() == Some(XLINK))) {
            let v = a.value().trim();
            if !v.starts_with('#') && !v.starts_with("data:") {
                edits.put(a.range(), "");
                outside += 1;
            }
        }
    }
    if count > MOST_ELEMENTS {
        return Err(format!("it has {count} parts, more than {MOST_ELEMENTS}: too many to draw smoothly. Simplify it, or turn detail into fewer paths"));
    }
    if outside > 0 {
        notes.push(format!("{outside} picture{} linked from outside the file {} left out: put pictures in the file itself (embedded).", s(outside), are(outside)));
    }
    if texts > 0 {
        notes.push(format!("{texts} text{} {} not shown, as there are no fonts for drawings: turn text into paths (in Inkscape, Path › Object to Path).", s(texts), are(texts)));
    }
    if animated > 0 {
        notes.push("SVG animation is not played: mark the parts that move instead (move-drift-1 and the like).".into());
    }
    Ok((edits.apply(svg), notes))
}

fn s(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn are(n: usize) -> &'static str {
    if n == 1 { "is" } else { "are" }
}

/// Tags that hold what other parts refer to, kept wherever they are.
const KEPT: [&str; 13] = ["defs", "style", "linearGradient", "radialGradient", "pattern", "filter", "clipPath", "mask", "marker", "symbol", "metadata", "title", "desc"];

/// The edit that hides `node` from a drawing while keeping what others refer to in it: a group
/// becomes `<defs>` (never drawn, its gradients still there), a shape is taken out.
fn hide(svg: &str, node: roxmltree::Node, edits: &mut Edits) {
    let tag = node.tag_name().name();
    if KEPT.contains(&tag) {
        return;
    }
    let r = node.range();
    let element = &svg[r.clone()];
    let named_plainly = element[1..].starts_with(tag);
    if named_plainly && matches!(tag, "g" | "a" | "switch" | "svg") && element.ends_with(&format!("</{tag}>")) {
        edits.put(r.start + 1..r.start + 1 + tag.len(), "defs");
        edits.put(r.end - tag.len() - 1..r.end - 1, "defs");
    } else {
        edits.put(r, "");
    }
}

/// `svg` with only `target` drawn (and what it refers to), less the parts of it in `without`; framed
/// on `view` (its own units) when given.
fn cut(svg: &str, doc: &roxmltree::Document, target: roxmltree::Node, without: &[roxmltree::Node], view: Option<Rect>) -> String {
    let mut edits = Edits::default();
    let chain: Vec<roxmltree::Node> = target.ancestors().collect();
    for a in chain.iter().skip(1) {
        for c in a.children().filter(|c| c.is_element() && !chain.contains(c)) {
            hide(svg, c, &mut edits);
        }
    }
    for n in without {
        hide(svg, *n, &mut edits);
    }
    if let Some(v) = view {
        let root = doc.root_element();
        for a in root.attributes().filter(|a| a.namespace().is_none() && matches!(a.name(), "viewBox" | "width" | "height" | "preserveAspectRatio")) {
            edits.put(a.range(), "");
        }
        let at = after_name(svg, root);
        edits.put(at..at, format!(" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\"", v.left(), v.top(), v.width(), v.height(), v.width(), v.height()));
    }
    edits.apply(svg)
}

/// The elements with a role, not inside another with one.
fn roles<'a, 'i>(doc: &'a roxmltree::Document<'i>, within: roxmltree::Node<'a, 'i>, wanted: impl Fn(&str) -> bool) -> Vec<roxmltree::Node<'a, 'i>> {
    let _ = doc;
    within
        .descendants()
        .skip(1)
        .filter(|n| n.is_element() && n.attribute("id").is_some_and(&wanted))
        // Inside a moving part, or in the desk itself (a coconut on its palm), it is still its own.
        .filter(|n| !n.ancestors().skip(1).take_while(|a| *a != within).any(|a| a.attribute("id").and_then(role).is_some_and(|r| !r.starts_with("move-") && r != "desk")))
        .collect()
}

/// A number at the end of an id (`stage-12`), to put stages in order.
fn number(id: &str) -> usize {
    id.rsplit('-').next().and_then(|d| d.parse().ok()).unwrap_or(0)
}

/// Is it something drawn, rather than what others refer to?
fn drawn(n: &roxmltree::Node) -> bool {
    n.is_element() && !KEPT.contains(&n.tag_name().name()) && !matches!(n.attribute("id"), Some("done" | "hook"))
}

impl Pack {
    /// A drawing already made readable, read into its parts.
    pub fn read(svg: String) -> Result<Pack, String> {
        use resvg::{tiny_skia, usvg};
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|e| format!("the drawing could not be read: {e}"))?;
        let doc = parse(&svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let size = tree.size();
        let into_canvas = tree.root().transform();
        let from_canvas = into_canvas.invert().unwrap_or_default();
        let meta = doc
            .descendants()
            .find(|n| n.has_tag_name("metadata") && n.attribute("id") == Some("caprice"))
            .and_then(|n| n.text())
            .map(|t| serde_json::from_str::<Meta>(t.trim()).map_err(|e| format!("its settings (the metadata) could not be read: {e}")))
            .transpose()?
            .unwrap_or_default();
        // A part's place as shares of the desk's size.
        let share = |id: &str| -> Option<Rect> {
            let bb = tree.node_by_id(id)?.abs_layer_bounding_box()?;
            Some(Rect::from_min_max(pos2(bb.x() / size.width(), bb.y() / size.height()), pos2(bb.right() / size.width(), bb.bottom() / size.height())))
        };
        // A part's place in the drawing's own units, from where it is drawn.
        let frame = |id: &str| -> Option<Rect> {
            let bb = tree.node_by_id(id)?.abs_layer_bounding_box()?;
            let mut corners = [tiny_skia::Point::from_xy(bb.x(), bb.y()), tiny_skia::Point::from_xy(bb.right(), bb.bottom())];
            from_canvas.map_points(&mut corners);
            let r = Rect::from_two_pos(pos2(corners[0].x, corners[0].y), pos2(corners[1].x, corners[1].y));
            Some(r.expand(r.size().max_elem() * 0.01))
        };
        let tops = roles(&doc, root, |id| matches!(role(id).as_deref(), Some(r) if r == "desk" || r == "carrier" || r == "piece" || r.starts_with("piece-")));
        let desk = tops.iter().find(|n| n.attribute("id") == Some("desk")).copied().ok_or(
            "it has no desk: name the layer or group holding the picture behind the pages \"desk\" (its id, or its name in the layers panel)",
        )?;
        let others: Vec<roxmltree::Node> = tops.iter().filter(|n| **n != desk).copied().collect();
        let desk = Art { svg: cut(&svg, &doc, root, &others, None), size: vec2(size.width(), size.height()) };

        let mut pieces: Vec<(usize, Piece)> = Vec::new();
        for p in tops.iter().filter(|n| n.attribute("id").is_some_and(|id| id == "piece" || id.starts_with("piece-"))) {
            let id = p.attribute("id").unwrap_or_default();
            let Some(view) = frame(id) else { continue };
            let done: Vec<roxmltree::Node> = p.descendants().filter(|n| n.attribute("id") == Some("done")).collect();
            let mut parts: Vec<roxmltree::Node> = roles(&doc, *p, |id| plain_name(id).starts_with("stage-"));
            parts.sort_by_key(|n| number(n.attribute("id").unwrap_or_default()));
            if parts.is_empty() {
                // Its parts in the order drawn; inside a lone group (a layer within the layer), that group's.
                let mut within = *p;
                loop {
                    let children: Vec<roxmltree::Node> = within.children().filter(drawn).collect();
                    match children.as_slice() {
                        [only] if only.has_tag_name("g") && only.attribute("id").and_then(role).is_none() => within = *only,
                        _ => {
                            parts = children;
                            break;
                        }
                    }
                }
            }
            if parts.is_empty() {
                continue;
            }
            // More parts than stages: as many stages, the parts shared out among them in order.
            let n = parts.len().min(MOST_STAGES);
            let chunk = |k: usize| (k * parts.len() / n)..((k + 1) * parts.len() / n);
            let stages = (0..n)
                .map(|k| {
                    let r = chunk(k);
                    let without: Vec<roxmltree::Node> = parts.iter().enumerate().filter(|(i, _)| !r.contains(i)).map(|(_, n)| *n).chain(done.iter().copied()).collect();
                    Art { svg: cut(&svg, &doc, *p, &without, Some(view)), size: view.size() }
                })
                .collect();
            let done = done.first().map(|_| Art { svg: cut(&svg, &doc, *p, &parts, Some(view)), size: view.size() });
            let at = share(id).map_or(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), |r| r.expand2(r.size() * 0.01));
            pieces.push((if id == "piece" { 0 } else { number(id) }, Piece { stages, done, size: view.size(), at, shared: parts.len() > MOST_STAGES }));
        }
        // piece-1, piece-2, … in their order, whatever order they were drawn in.
        pieces.sort_by_key(|p| p.0);
        let pieces = pieces.into_iter().map(|p| p.1).collect();

        let carrier = tops.iter().find(|n| n.attribute("id") == Some("carrier")).and_then(|c| {
            let view = frame("carrier")?;
            let hook: Vec<roxmltree::Node> = c.descendants().filter(|n| n.attribute("id") == Some("hook")).collect();
            let at = frame("hook").map_or(pos2(0.5, 1.0), |h| {
                let m = h.center();
                pos2((m.x - view.left()) / view.width(), (m.y - view.top()) / view.height())
            });
            Some(Carrier { art: Art { svg: cut(&svg, &doc, *c, &hook, Some(view)), size: view.size() }, hook: at })
        });

        let ground = meta.growth.ground.map(|y| {
            let mut p = [tiny_skia::Point::from_xy(0.0, y)];
            into_canvas.map_points(&mut p);
            (p[0].y / size.height()).clamp(0.0, 1.0)
        });
        Ok(Pack { svg, meta, desk, pieces, carrier, ground })
    }
}

/// Check a drawing to be a theme: what makes it one, and what would spoil it. `name` is its file's,
/// for a theme that names itself nothing. Refused, the reasons, in plain words.
pub fn check(svg: &str, name: &str) -> Result<Checked, Vec<String>> {
    if svg.len() > MOST_BYTES {
        return Err(vec![format!("The drawing is {} MB, more than {} MB: make its pictures smaller, or leave some out.", svg.len() / (1024 * 1024), MOST_BYTES / (1024 * 1024))]);
    }
    let (normal, mut notes) = normalize(svg).map_err(|e| vec![format!("This is not a drawing Caprice can read: {e}.")])?;
    let mut pack = Pack::read(normal).map_err(|e| vec![format!("{}{}.", e[..1].to_uppercase(), &e[1..])])?;
    if pack.meta.name.trim().is_empty() {
        pack.meta.name = name.trim().to_owned();
    }
    pack.meta.name = pack.meta.name.chars().take(40).collect();
    pack.meta.about = pack.meta.about.chars().take(120).collect();
    let mut problems = Vec::new();
    let mut found = Vec::new();

    let desk = crate::motion::layered(&pack.desk.svg, 480.0).map_err(|e| vec![format!("The desk could not be drawn: {e}.")])?;
    let marked: Vec<String> = {
        let doc = parse(&pack.desk.svg).map_err(|e| vec![e.to_string()])?;
        doc.descendants()
            .filter(|n| n.is_element() && !n.ancestors().any(|a| a.has_tag_name("defs")))
            .filter_map(|n| n.attribute("id").filter(|id| id.starts_with("move-")).map(str::to_owned))
            .collect()
    };
    let moving = desk.layers.len();
    if moving < LEAST_MOVING {
        problems.push(format!(
            "The desk must move, and {} of it does: mark at least {LEAST_MOVING} parts with how they move, by their id or layer name \
             (move-drift-1 for a cloud, move-twinkle-1 for a star, move-snow-1 on a rectangle where snow falls; the template shows them all).",
            if moving == 0 { "nothing".to_owned() } else { format!("only {moving} part") },
        ));
    }
    let left_still: Vec<&String> = marked.iter().filter(|id| !desk.layers.iter().any(|l| &l.id == *id)).collect();
    if !left_still.is_empty() {
        let names: Vec<&str> = left_still.iter().take(4).map(|s| s.as_str()).collect();
        notes.push(format!(
            "{} marked to move {} still: {}{}. A wing or a sway needs the point it turns about, near the part (move-sway-1-310-240); a spin, a point inside it.",
            left_still.len(),
            if left_still.len() == 1 { "part stays" } else { "parts stay" },
            names.join(", "),
            if left_still.len() > 4 { ", …" } else { "" }
        ));
    }
    if moving > 0 && desk.layers.iter().all(|l| (0.3..0.7).contains(&l.at.center().x)) {
        notes.push("Everything that moves is in the middle of the desk, where the page hides it: move some of it to the sides.".into());
    }
    let aspect = pack.desk.size.x / pack.desk.size.y;
    if aspect < 1.0 {
        notes.push("The desk is taller than it is wide: on most screens much of it is cut off. A wide drawing (16:9 or wider) fits best.".into());
    }
    found.push(format!("A desk with {moving} moving part{}.", s(moving)));
    // The pieces' and the carrier's parts marked to move, and whether they do.
    let mut still_elsewhere = Vec::new();
    let parts = pack.pieces.iter().flat_map(|p| p.stages.iter().chain(&p.done)).chain(pack.carrier.as_ref().map(|c| &c.art));
    for art in parts {
        let Ok(doc) = parse(&art.svg) else { continue };
        let marked: Vec<String> = doc
            .descendants()
            .filter(|n| n.is_element() && !n.ancestors().any(|a| a.has_tag_name("defs")))
            .filter_map(|n| n.attribute("id").filter(|id| id.starts_with("move-")).map(str::to_owned))
            .collect();
        if marked.is_empty() {
            continue;
        }
        let layered = crate::motion::layered(&art.svg, 240.0).map(|l| l.layers.into_iter().map(|l| l.id).collect::<Vec<_>>()).unwrap_or_default();
        for id in marked {
            if !layered.contains(&id) && !still_elsewhere.contains(&id) {
                still_elsewhere.push(id);
            }
        }
    }
    if !still_elsewhere.is_empty() {
        notes.push(format!(
            "In the piece or the carrier, {} marked to move {} still: {}. Its point (X,Y) must be on the part: in the drawing's units, or in those of the group it is drawn in.",
            still_elsewhere.len(),
            if still_elsewhere.len() == 1 { "part stays" } else { "parts stay" },
            still_elsewhere.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    for (k, p) in pack.pieces.iter().enumerate() {
        let name = if pack.pieces.len() == 1 { "The piece".to_owned() } else { format!("Piece {}", k + 1) };
        let aspect = p.size.x / p.size.y;
        if pack.meta.growth.layout != Layout::Drawn && !(0.15..=4.0).contains(&aspect) {
            notes.push(format!(
                "{name} is {:.0} wide and {:.0} tall, with its finish: Caprice stands one beside the page for each chapter, so draw it as one thing, \
                 close together. To have it stay where it is drawn on the desk instead, set \"layout\": \"drawn\".",
                p.size.x, p.size.y
            ));
        }
        if let Some(k) = repeated_stage(p) {
            notes.push(format!(
                "{name}'s stage {} repeats what the stage before it shows: each stage should hold only what it adds, as the stages before it are shown too.",
                k + 1
            ));
        }
    }
    for (k, p) in pack.pieces.iter().enumerate() {
        found.push(format!(
            "{} growing in {} stage{}{}.",
            if pack.pieces.len() == 1 { "A piece".to_owned() } else { format!("Piece {}", k + 1) },
            p.stages.len(),
            s(p.stages.len()),
            if p.done.is_some() { ", and a finish" } else { "" }
        ));
        if p.shared {
            notes.push(format!("A piece grows in {MOST_STAGES} stages at most: its parts are shared out among them."));
        }
    }
    if let Some(c) = &pack.carrier {
        let wings = crate::motion::layered(&c.art.svg, 240.0).map(|l| l.layers.iter().filter(|l| matches!(l.motion, crate::motion::Motion::Wing { .. })).count()).unwrap_or(0);
        found.push(if wings > 0 { format!("A carrier with {wings} wing{} that beat{}.", s(wings), if wings == 1 { "s" } else { "" }) } else { "A carrier, gliding (no wings marked to beat).".into() });
    }
    let (_, unread) = with_colours(Theme::Midnight.palette(), &pack.meta.palette);
    if !unread.is_empty() {
        notes.push(format!("Colours not understood, and taken from the picture instead: {}. Write them as \"accent\": \"#e0604a\".", unread.join(", ")));
    }
    if problems.is_empty() { Ok(Checked { pack, found, notes }) } else { Err(problems) }
}

/// The first stage of a piece that shows again most of what the stage before it does (as if each
/// stage were the whole piece so far), if one does.
fn repeated_stage(piece: &Piece) -> Option<usize> {
    let mask = |art: &Art| crate::backdrop::rasterize(&art.svg, 96.0).ok().map(|i| i.pixels.iter().map(|c| c.a() > 40).collect::<Vec<bool>>());
    let masks: Vec<Vec<bool>> = piece.stages.iter().take(6).map(mask).collect::<Option<_>>()?;
    (1..masks.len()).find(|&k| {
        let before = masks[k - 1].iter().filter(|&&b| b).count();
        let again = masks[k - 1].iter().zip(&masks[k]).filter(|(a, b)| **a && **b).count();
        before > 0 && again * 10 >= before * 8
    })
}

/// A drawing checked and made ready to keep: the theme, its palette and the gallery's picture of it.
pub struct Ready {
    pub checked: Checked,
    pub palette: Palette,
    pub thumb: ColorImage,
}

/// `check`, then the palette and the gallery's picture: all of it slow, so off the frame.
pub fn prepare(svg: &str, name: &str) -> Result<Ready, Vec<String>> {
    let checked = check(svg, name)?;
    let thumb = crate::backdrop::rasterize(&checked.pack.desk.svg, THUMB_WIDTH).map_err(|e| vec![format!("The desk could not be drawn: {e}.")])?;
    let small = crate::backdrop::rasterize(&checked.pack.desk.svg, 96.0).map_err(|e| vec![e])?;
    let palette = palette_of(&small, &checked.pack.meta.palette);
    Ok(Ready { checked, palette, thumb })
}

// ---- The shelf of themes ---------------------------------------------------------------------

/// What the gallery shows of a theme on the shelf, kept beside the drawings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub author: String,
    pub colours: BTreeMap<String, String>,
    #[serde(default)]
    pub grows: bool,
    #[serde(default)]
    pub carries: bool,
    #[serde(default)]
    pub added: u64,
}

impl Entry {
    pub fn palette(&self) -> Palette {
        with_colours(Theme::Midnight.palette(), &self.colours).0
    }
}

/// The writer's themes: in the data folder's `themes` (in the browser, IndexedDB), or in memory
/// only, for tests.
#[derive(Default)]
pub struct Shelf {
    pub dir: Option<PathBuf>,
    pub entries: Vec<Entry>,
    /// Without a folder: the drawings, and the pictures of them.
    held: HashMap<String, (String, ColorImage)>,
    /// Changed since the gallery last loaded its pictures.
    pub version: u64,
}

impl Shelf {
    pub fn kept_in(dir: Option<PathBuf>) -> Self {
        let entries = dir.as_ref().and_then(|d| fs::read_to_string(d.join("index.json")).ok()).and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
        Self { dir, entries, held: HashMap::new(), version: 0 }
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The drawing of a theme on the shelf.
    pub fn svg(&self, id: &str) -> Option<String> {
        match &self.dir {
            Some(d) => fs::read_to_string(d.join(format!("{id}.svg"))).ok(),
            None => self.held.get(id).map(|h| h.0.clone()),
        }
    }

    /// The gallery's picture of a theme.
    pub fn thumb(&self, id: &str) -> Option<ColorImage> {
        match &self.dir {
            Some(d) => {
                let bytes = fs::read(d.join(format!("{id}.png"))).ok()?;
                let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
                Some(ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw()))
            }
            None => self.held.get(id).map(|h| h.1.clone()),
        }
    }

    /// Put a theme on the shelf; its id.
    pub fn add(&mut self, ready: &Ready) -> Result<String, String> {
        let id = crate::versions::new_id();
        let pack = &ready.checked.pack;
        let entry = Entry {
            id: id.clone(),
            name: pack.meta.name.clone(),
            about: pack.meta.about.clone(),
            author: pack.meta.author.clone(),
            colours: colours(&ready.palette),
            grows: !pack.pieces.is_empty(),
            carries: pack.carrier.is_some(),
            added: crate::platform::since_epoch().as_secs(),
        };
        match &self.dir {
            Some(d) => {
                fs::create_dir_all(d).map_err(|e| e.to_string())?;
                fs::write(d.join(format!("{id}.svg")), pack.svg.as_bytes()).map_err(|e| e.to_string())?;
                let t = &ready.thumb;
                let rgba: Vec<u8> = t.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                if let Some(img) = image::RgbaImage::from_raw(t.size[0] as u32, t.size[1] as u32, rgba) {
                    let mut png = std::io::Cursor::new(Vec::new());
                    if img.write_to(&mut png, image::ImageFormat::Png).is_ok() {
                        let _ = fs::write(d.join(format!("{id}.png")), png.into_inner());
                    }
                }
            }
            None => {
                self.held.insert(id.clone(), (pack.svg.clone(), ready.thumb.clone()));
            }
        }
        self.entries.push(entry);
        self.save()?;
        Ok(id)
    }

    /// Take a theme off the shelf, and its files.
    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        self.held.remove(id);
        if let Some(d) = &self.dir {
            for ext in ["svg", "png"] {
                let _ = fs::remove_file(d.join(format!("{id}.{ext}")));
            }
        }
        let _ = self.save();
    }

    fn save(&mut self) -> Result<(), String> {
        self.version += 1;
        let Some(d) = &self.dir else { return Ok(()) };
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&self.entries).map_err(|e| e.to_string())?;
        fs::write(d.join("index.json"), json).map_err(|e| e.to_string())
    }
}

// ---- In the app ------------------------------------------------------------------------------

/// What a drawing imported came to, for its window.
pub struct Report {
    pub name: String,
    pub added: bool,
    pub found: Vec<String>,
    pub notes: Vec<String>,
    pub problems: Vec<String>,
}

/// A drawing being checked off the frame: its name, and what it comes to.
type Importing = (String, Receiver<Result<Ready, Vec<String>>>);

/// Making and keeping themes of one's own: the import on its way, its report, the guide, the
/// gallery's pictures.
#[derive(Default)]
pub struct Workshop {
    importing: Option<Importing>,
    pub report: Option<Report>,
    pub guide: bool,
    pub deleting: Option<String>,
    pub thumbs: HashMap<String, TextureHandle>,
    thumbs_for: Option<u64>,
}

impl Workshop {
    pub fn importing(&self) -> bool {
        self.importing.is_some()
    }
}

/// Something asked for in the gallery's "Yours".
pub enum Ask {
    Use(String),
    Import,
    Template,
    Guide,
    Export(String),
    Delete(String),
}

impl App {
    /// Ask for a drawing to import as a theme.
    pub fn choose_theme_file(&mut self, ctx: &egui::Context) {
        let dialog = crate::platform::Dialog::new().add_filter("Theme drawing (SVG)", &["svg"]);
        self.ask_path(ctx, crate::fileio::DialogFor::ImportTheme, dialog);
    }

    /// Import the drawing at `path` as a theme.
    pub fn import_theme(&mut self, ctx: &egui::Context, path: &Path) {
        let name = path.file_stem().map_or("My theme".to_owned(), |s| s.to_string_lossy().replace(['_', '-'], " "));
        match fs::read(path) {
            Ok(bytes) => self.import_theme_svg(ctx, String::from_utf8_lossy(&bytes).into_owned(), name),
            Err(e) => self.workshop.report = Some(Report { name, added: false, found: Vec::new(), notes: Vec::new(), problems: vec![format!("The file could not be read: {e}.")] }),
        }
    }

    /// Import a drawing as a theme, checked off the frame; it is chosen once it is on the shelf.
    pub fn import_theme_svg(&mut self, ctx: &egui::Context, svg: String, name: String) {
        let (tx, rx) = channel();
        let wake = crate::platform::Waker::new(ctx);
        let named = name.clone();
        crate::platform::spawn(move || {
            let _ = tx.send(prepare(&svg, &named));
            wake.wake();
        });
        self.workshop.importing = Some((name, rx));
        self.status = "- reading the theme…".into();
    }

    /// Once a frame: a theme imported goes on the shelf and is chosen; the gallery's pictures load.
    pub fn tend_workshop(&mut self, ctx: &egui::Context) {
        if let Some((name, rx)) = &self.workshop.importing {
            let got = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(vec!["The drawing could not be read.".into()])),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            };
            if let Some(got) = got {
                let name = name.clone();
                self.workshop.importing = None;
                self.workshop.report = Some(match got {
                    Ok(ready) => match self.themes.own.add(&ready) {
                        Ok(id) => {
                            let name = ready.checked.pack.meta.name.clone();
                            self.themes.choose_own(ctx, &id);
                            self.status = format!("- theme \u{201c}{name}\u{201d} added");
                            Report { name, added: true, found: ready.checked.found, notes: ready.checked.notes, problems: Vec::new() }
                        }
                        Err(e) => Report { name, added: false, found: Vec::new(), notes: Vec::new(), problems: vec![format!("The theme could not be kept: {e}.")] },
                    },
                    Err(problems) => {
                        self.status = "- the theme was not added".into();
                        Report { name, added: false, found: Vec::new(), notes: Vec::new(), problems }
                    }
                });
            }
        }
        if self.workshop.thumbs_for != Some(self.themes.own.version) {
            self.workshop.thumbs_for = Some(self.themes.own.version);
            let ids: Vec<String> = self.themes.own.entries.iter().map(|e| e.id.clone()).collect();
            self.workshop.thumbs.retain(|id, _| ids.contains(id));
            for id in ids {
                if !self.workshop.thumbs.contains_key(&id)
                    && let Some(image) = self.themes.own.thumb(&id)
                {
                    self.workshop.thumbs.insert(id.clone(), ctx.load_texture(format!("theme-{id}"), image, TextureOptions::LINEAR));
                }
            }
        }
    }

    /// Do what was asked for in the gallery.
    pub fn theme_asked(&mut self, ctx: &egui::Context, ask: Ask) {
        match ask {
            Ask::Use(id) => self.themes.choose_own(ctx, &id),
            Ask::Import => self.choose_theme_file(ctx),
            Ask::Template => {
                let dialog = crate::platform::Dialog::new().add_filter("Theme drawing (SVG)", &["svg"]).set_file_name("Caprice theme template.svg");
                self.ask_path(ctx, crate::fileio::DialogFor::ThemeTemplate, dialog);
            }
            Ask::Guide => self.workshop.guide = true,
            Ask::Export(id) => {
                let name = self.themes.own.get(&id).map_or("Theme".to_owned(), |e| e.name.clone());
                let dialog = crate::platform::Dialog::new().add_filter("Theme drawing (SVG)", &["svg"]).set_file_name(format!("{name}.svg"));
                self.ask_path(ctx, crate::fileio::DialogFor::ExportTheme(id), dialog);
            }
            Ask::Delete(id) => self.workshop.deleting = Some(id),
        }
    }

    /// Write the theme `id`'s drawing to `path`, to keep, change or give to someone.
    pub fn export_theme_to(&mut self, id: &str, path: &Path) {
        self.status = match self.themes.own.svg(id).map(|svg| fs::write(path, svg.as_bytes())) {
            Some(Ok(())) => format!("- theme saved as {}", path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())),
            Some(Err(e)) => format!("- theme not saved: {e}"),
            None => "- the theme is no longer there".into(),
        };
    }

    pub fn save_theme_template_to(&mut self, path: &Path) {
        self.status = match fs::write(path, TEMPLATE.as_bytes()) {
            Ok(()) => "- template saved: change it in any drawing program, then import it".into(),
            Err(e) => format!("- template not saved: {e}"),
        };
    }

    /// Take theme `id` off the shelf; if it was in use, the usual theme takes its place.
    pub fn delete_theme(&mut self, ctx: &egui::Context, id: &str) {
        if self.themes.theme == Theme::Own && self.themes.own_id.as_deref() == Some(id) {
            self.themes.choose(ctx, Theme::default());
        }
        self.themes.own.remove(id);
    }

    /// The import's report, the guide to making a theme, and the question before deleting one.
    pub fn workshop_windows(&mut self, ctx: &egui::Context) {
        let mut ask = None;
        if let Some(report) = &self.workshop.report {
            let mut open = true;
            let mut close = false;
            let title = if report.added { "Theme added" } else { "Theme not added" };
            egui::Window::new(title).id(egui::Id::new("theme_report")).open(&mut open).collapsible(false).resizable(false).default_width(420.0).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                ui.set_max_width(440.0);
                ui.label(egui::RichText::new(&report.name).strong().size(16.0));
                if report.added {
                    ui.label("It is the theme in use now, and in the Page menu's Theme gallery under Yours.");
                }
                for (head, lines, colour) in [("Found", &report.found, pal().text), ("Worth knowing", &report.notes, pal().text), ("Why not", &report.problems, pal().accent)] {
                    if lines.is_empty() {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(head).color(pal().text_dim).size(12.0));
                    for line in lines {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("•").color(colour));
                            ui.label(line);
                        });
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if !report.added {
                        if ui.button("Try another…").clicked() {
                            ask = Some(Ask::Import);
                            close = true;
                        }
                        if ui.button("How to make one").clicked() {
                            ask = Some(Ask::Guide);
                            close = true;
                        }
                    }
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                });
            });
            if !open || close {
                self.workshop.report = None;
            }
        }
        if self.workshop.guide {
            let mut open = true;
            egui::Window::new("Make a theme of your own").id(egui::Id::new("theme_guide")).open(&mut open).collapsible(false).default_width(480.0).show(ctx, |ui| {
                ui.set_max_width(500.0);
                egui::ScrollArea::vertical().max_height(460.0).show(ui, guide);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Get the template…").clicked() {
                        ask = Some(Ask::Template);
                    }
                    if ui.button("Import a theme…").clicked() {
                        ask = Some(Ask::Import);
                    }
                });
            });
            self.workshop.guide &= open;
        }
        if let Some(id) = self.workshop.deleting.clone() {
            let name = self.themes.own.get(&id).map_or(String::new(), |e| e.name.clone());
            let mut answer = None;
            egui::Window::new("Delete theme?").id(egui::Id::new("theme_delete")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                ui.label(format!("\u{201c}{name}\u{201d} will be gone from this device. Export it first to keep a copy."));
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Keep it").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(yes) = answer {
                if yes {
                    self.delete_theme(ctx, &id);
                    self.status = format!("- theme \u{201c}{name}\u{201d} deleted");
                }
                self.workshop.deleting = None;
            }
        }
        if let Some(ask) = ask {
            self.theme_asked(ctx, ask);
        }
    }
}

/// The guide to making a theme, short enough to read in the app and to give to an AI chat.
fn guide(ui: &mut egui::Ui) {
    let head = |ui: &mut egui::Ui, s: &str| {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(s).strong());
    };
    let line = |ui: &mut egui::Ui, s: &str| {
        ui.horizontal_wrapped(|ui| {
            ui.label("•");
            ui.label(s);
        });
    };
    ui.label("A theme is one SVG drawing. Draw it in Inkscape, Illustrator, Affinity or Figma, write it by hand, or ask an AI chat for one, giving it this guide. Start from the template: everything is in it, explained.");
    head(ui, "The desk (required, and it must move)");
    line(ui, "A layer or group named \u{201c}desk\u{201d}: the picture behind the pages. It covers the whole desk, cut to fit, its foot kept. Draw it wide (1920 × 1080 or wider).");
    line(ui, "The page hides the middle: put what matters, and what moves, to the left and the right.");
    line(ui, "Name the parts that move (their id, or their name in the layers panel). At least two:");
    for m in MOTIONS {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new(m.0).monospace().size(12.0));
            ui.label(egui::RichText::new(m.1).color(pal().text_dim).size(12.0));
        });
    }
    line(ui, "X,Y are in the drawing's units (its viewBox), or in those of the group the part is drawn in. N is any number, to tell parts apart. For particles, draw a rectangle where they fall or rise, filled with their colour.");
    head(ui, "What grows (optional)");
    line(ui, "A group named \u{201c}piece\u{201d}: Caprice stands one beside the pages for each chapter, wherever it is drawn. Each page shows one more of its parts, in the order drawn (or groups stage-1, stage-2, …, up to 24). A group \u{201c}done\u{201d} in it shows once the next chapter begins.");
    line(ui, "Each stage holds only what it adds: the stages before it stay shown.");
    line(ui, "Or, with \"layout\": \"drawn\", it stays where it is drawn on the desk, one for the whole story, a stage for every page; its done shows once every stage has come.");
    line(ui, "With \"each\": \"chapter\" as well, each chapter has its own piece where it is drawn (piece-1 on one tree, piece-2 on another, or at random with \"order\": \"random\"); when the next chapter begins its done shows, or with \"finish\": \"fall\" its stages fall to the ground. What is drawn on a moving part of the desk moves with it.");
    line(ui, "Stages come out of a little light, or (\"arrive\": \"fall\") drop from above, or (\"grow\") rise from their foot.");
    line(ui, "Several pieces (piece-1, piece-2, …) take turns, chapter by chapter.");
    head(ui, "What brings a new chapter title (optional)");
    line(ui, "A group named \u{201c}carrier\u{201d}: it flies in holding the title at a small mark named \u{201c}hook\u{201d} (or at its foot). Wings marked move-wing-N-X-Y beat.");
    head(ui, "Settings (optional)");
    line(ui, "JSON in <metadata id=\"caprice\">: name, about, author; palette (colours by name, e.g. \"accent\": \"#e0604a\"; the rest come from the picture); growth (layout \"row\", \"scatter\" or \"drawn\"; arrive \"light\", \"fall\" or \"grow\"; each \"story\" or \"chapter\"; order \"turn\" or \"random\"; finish \"done\" or \"fall\"; size, ground, mirror); carrier (faces: the way its head points as drawn, \"left\" or \"right\"; size); anchor (\"bottom\", \"middle\" or \"top\").");
    head(ui, "Good to know");
    line(ui, "Text is not shown: turn it into paths. Pictures must be in the file (embedded). Nothing of a theme is printed or saved with a story.");
}

/// The kinds of motion, as the guide lists them.
pub const MOTIONS: [(&str, &str); 17] = [
    ("move-drift-N", "a cloud or mist, drifting sideways"),
    ("move-sway-N-X-Y", "a tree, reeds or a flag, swaying about its foot X,Y"),
    ("move-wing-N-X-Y", "a wing, beating about its shoulder X,Y"),
    ("move-spin-N-X-Y", "sails or a wheel, turning round X,Y"),
    ("move-float-N", "a boat or a lantern, bobbing"),
    ("move-wander-N", "a fish or a butterfly, about a little loop"),
    ("move-twinkle-N", "a star"),
    ("move-flicker-N", "a lit window, a flame"),
    ("move-pulse-N", "a glow, brighter and dimmer"),
    ("move-shimmer-N", "glints on water"),
    ("move-smoke-N-X-Y", "an empty group: smoke rises from X,Y"),
    ("move-snow-N", "a rectangle: snow falls in it"),
    ("move-rain-N", "a rectangle: rain"),
    ("move-fireflies-N", "a rectangle: fireflies"),
    ("move-leaves-N", "a rectangle: falling leaves"),
    ("move-bubbles-N", "a rectangle: rising bubbles"),
    ("move-embers-N", "a rectangle: rising embers"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A desk of a sky, hills, a cloud and a star, a piece of three parts and a done, and a bird.
    pub const LITTLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" viewBox="0 0 1600 900">
        <metadata id="caprice">{"name": "Little", "palette": {"accent": "#ff8800"}, "growth": {"ground": 860}}</metadata>
        <g inkscape:label="Desk" inkscape:groupmode="layer">
            <rect width="1600" height="900" fill="#203050"/>
            <rect y="700" width="1600" height="200" fill="#305030"/>
            <g inkscape:label="move-drift-1"><ellipse cx="200" cy="150" rx="90" ry="25" fill="#ddd"/></g>
            <circle id="move_x2D_twinkle_x2D_2" cx="1400" cy="100" r="6" fill="#ffe"/>
        </g>
        <g id="piece">
            <rect x="0" y="600" width="100" height="100" fill="#a33"/>
            <rect x="0" y="500" width="100" height="100" fill="#3a3"/>
            <rect x="0" y="400" width="100" height="100" fill="#33a"/>
            <g id="done"><circle cx="50" cy="380" r="20" fill="#ff0"/></g>
        </g>
        <g id="carrier">
            <ellipse cx="300" cy="300" rx="40" ry="20" fill="#eee"/>
            <circle id="hook" cx="300" cy="330" r="3" fill="#000"/>
        </g>
    </svg>"##;

    #[test]
    fn names_from_inkscape_illustrator_and_figma_are_read_alike() {
        assert_eq!(plain_name("move_x2D_drift_x2D_1"), "move-drift-1");
        assert_eq!(plain_name("Desk_2_"), "desk");
        assert_eq!(plain_name("  Stage 3 "), "stage-3");
        assert_eq!(role("Background").as_deref(), Some("desk"));
        assert_eq!(role("piece_x2D_2").as_deref(), Some("piece-2"));
        assert_eq!(role("trees"), None);
    }

    #[test]
    fn a_drawing_with_a_desk_that_moves_a_piece_and_a_carrier_is_read_into_its_parts() {
        let checked = check(LITTLE, "file").unwrap();
        let pack = &checked.pack;
        assert_eq!(pack.meta.name, "Little");
        assert_eq!(pack.pieces.len(), 1);
        assert_eq!(pack.pieces[0].stages.len(), 3, "a stage a part, in the order drawn");
        assert!(pack.pieces[0].done.is_some());
        let c = pack.carrier.as_ref().unwrap();
        assert!((c.hook.x - 0.5).abs() < 0.05 && c.hook.y > 0.9, "held at its hook: {:?}", c.hook);
        assert!((pack.ground.unwrap() - 860.0 / 900.0).abs() < 0.01);
        // The desk alone: the piece and the carrier are not in it.
        let desk = crate::backdrop::rasterize(&pack.desk.svg, 1600.0).unwrap();
        let at = |x: usize, y: usize| desk.pixels[y * 1600 + x];
        assert_eq!(at(50, 650), at(800, 650), "no piece on the desk");
        assert_eq!(at(300, 300), at(800, 300), "no carrier on the desk");
        // Each stage alone, in the frame of the whole piece.
        let first = crate::backdrop::rasterize(&pack.pieces[0].stages[0].svg, 100.0).unwrap();
        let [w, h] = first.size;
        assert!(first.pixels[(h - 10) * w + w / 2].a() > 200, "the first part at its foot");
        assert!(first.pixels[h / 2 * w + w / 2].a() == 0, "and nothing of the others");
        let found = checked.found.join(" ");
        assert!(found.contains("2 moving parts") && found.contains("3 stages") && found.contains("finish"), "{found}");
    }

    #[test]
    fn a_drawing_whose_desk_does_not_move_is_refused_saying_why() {
        let still = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 900"><g id="desk"><rect width="1600" height="900" fill="#234"/></g></svg>"##;
        let why = check(still, "still").err().unwrap().join(" ");
        assert!(why.contains("must move") && why.contains("move-drift-1"), "{why}");
        let none = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"##;
        assert!(check(none, "x").err().unwrap()[0].contains("no desk"));
        assert!(check("not a drawing", "x").err().unwrap()[0].contains("not a drawing Caprice can read"));
    }

    #[test]
    fn links_out_of_the_file_are_taken_away_and_text_is_noted() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 1600 900"><g id="desk">
            <image xlink:href="/etc/secret.png" width="10" height="10"/><text x="5" y="5">Hi</text>
            <g id="move-drift-1"><rect width="5" height="5"/></g><g id="move-drift-1"><rect x="900" width="5" height="5"/></g></g></svg>"##;
        let checked = check(svg, "x").unwrap();
        assert!(!checked.pack.svg.contains("secret"));
        let notes = checked.notes.join(" ");
        assert!(notes.contains("linked from outside") && notes.contains("text"), "{notes}");
        assert!(checked.found[0].contains("2 moving parts"), "a part named twice is two parts: {:?}", checked.found);
    }

    #[test]
    fn a_piece_drawn_spread_out_or_with_stages_repeating_the_ones_before_is_noted() {
        let svg = |layout: &str| format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 900">
            <metadata id="caprice">{{"growth": {{"layout": "{layout}"}}}}</metadata>
            <g id="desk"><rect width="1600" height="900"/><g id="move-drift-1"><rect width="9" height="9" fill="#fff"/></g><g id="move-drift-2"><rect x="1500" width="9" height="9" fill="#fff"/></g></g>
            <g id="piece">
                <g id="stage-1"><circle cx="100" cy="850" r="10" fill="#963"/></g>
                <g id="stage-2"><circle cx="100" cy="850" r="10" fill="#963"/><circle cx="1400" cy="850" r="10" fill="#963"/></g>
            </g></svg>"##);
        let notes = check(&svg("row"), "x").unwrap().notes.join(" ");
        assert!(notes.contains("\"layout\": \"drawn\"") && notes.contains("stage 2 repeats"), "{notes}");
        let notes = check(&svg("drawn"), "x").unwrap().notes.join(" ");
        assert!(!notes.contains("wide") && notes.contains("stage 2 repeats"), "where it is drawn, any shape will do: {notes}");
    }

    #[test]
    fn the_palette_comes_from_the_picture_and_stays_readable_whatever_it_is() {
        let pictures = [
            ColorImage::filled([40, 30], Color32::from_rgb(250, 250, 250)),
            ColorImage::filled([40, 30], Color32::BLACK),
            ColorImage::filled([40, 30], Color32::from_rgb(255, 0, 255)),
            ColorImage::filled([40, 30], Color32::from_rgb(128, 128, 128)),
        ];
        for picture in &pictures {
            for overrides in [BTreeMap::new(), BTreeMap::from([("text".to_owned(), "#808080".to_owned()), ("dock".to_owned(), "#7f7f7f".to_owned())])] {
                let p = palette_of(picture, &overrides);
                assert!(contrast(p.text, p.dock) >= 4.5 && contrast(p.text, p.btn) >= 4.5, "{:?} {p:?}", picture.pixels[0]);
                assert!(contrast(p.text_dim, p.dock) >= 3.0, "{:?}", picture.pixels[0]);
                assert!(contrast(p.on_accent, p.accent) >= 4.5, "{:?}", picture.pixels[0]);
            }
        }
        let sunset = palette_of(&ColorImage::filled([40, 30], Color32::from_rgb(230, 120, 40)), &BTreeMap::new());
        let (h, s, _) = hsv(sunset.accent);
        assert!(s > 0.4 && h < 0.15, "the accent is the picture's own vivid colour: {:?}", sunset.accent);
        let chosen = palette_of(&pictures[1], &BTreeMap::from([("accent".to_owned(), "#ff8800".to_owned())]));
        assert_eq!(chosen.accent, Color32::from_rgb(255, 136, 0), "a colour the maker chose is kept");
    }

    #[test]
    fn the_template_is_a_theme_with_everything_in_it() {
        let checked = check(TEMPLATE, "template").unwrap_or_else(|e| panic!("{e:?}"));
        assert!(checked.notes.is_empty(), "{:?}", checked.notes);
        let pack = &checked.pack;
        assert!(!pack.pieces.is_empty() && pack.pieces.iter().all(|p| p.done.is_some()) && pack.carrier.is_some());
        let desk = crate::motion::layered(&pack.desk.svg, 480.0).unwrap();
        let kinds: std::collections::HashSet<String> = desk.layers.iter().map(|l| l.id.split('-').nth(1).unwrap_or_default().to_owned()).collect();
        assert!(kinds.len() >= 6, "it shows many kinds of motion: {kinds:?}");
    }

    #[test]
    fn a_theme_put_on_the_shelf_is_there_the_next_time() {
        let dir = std::env::temp_dir().join(format!("caprice-own-shelf-{}", crate::versions::new_id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ready = prepare(LITTLE, "file").unwrap();
        let mut shelf = Shelf::kept_in(Some(dir.clone()));
        let id = shelf.add(&ready).unwrap();
        let again = Shelf::kept_in(Some(dir.clone()));
        assert_eq!(again.entries, shelf.entries);
        assert_eq!(again.get(&id).unwrap().palette(), ready.palette);
        assert!(again.svg(&id).unwrap().contains("id=\"desk\""));
        assert_eq!(again.thumb(&id).unwrap().size, ready.thumb.size);
        shelf.remove(&id);
        assert!(Shelf::kept_in(Some(dir.clone())).entries.is_empty() && shelf.svg(&id).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
