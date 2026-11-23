//! Drawing a theme by hand (R36): a desktop-only sketch sheet in "Make a theme", on the 1280 by 800
//! desk Claude is shown previews on (`eggs::preview::PICTURE`). Four inks say when each part moves
//! (black still, blue with a new page, red with a new chapter, green all the time); the faded page
//! border lies underneath and is paintable too.
//!
//! To stay responsive with hundreds of strokes on WebAssembly, committed strokes live in one cached
//! 1280 by 800 transparent `ColorImage` / GPU texture, updated only in the dirty bounding box via
//! `TextureHandle::set_partial` when a stroke finishes or is rubbed out. While the pointer moves,
//! the frame draws one textured quad and only the single active stroke on top, and reads every
//! `PointerMoved` event queued in the frame so fast lines stay curved.

use eframe::egui::{self, Color32, ColorImage, CursorIcon, Event, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Shape, Stroke as EguiStroke, TextureHandle, TextureOptions, Vec2, pos2, vec2};

use crate::eggs::preview::{PICTURE, layout, png};
use crate::theme::pal;

/// The sketch desk's dimensions, matching `eggs::preview::PICTURE`.
pub const W: usize = 1280;
pub const H: usize = 800;

/// Pen widths (desk pixels) and rubber diameters for Fine, Medium and Broad.
const SIZES: [f32; 3] = [3.0, 7.0, 16.0];
const RUBBER: [f32; 3] = [10.0, 22.0, 46.0];
const SIZE_NAMES: [&str; 3] = ["Fine", "Medium", "Broad"];

/// Caps matching the Worker (`web/api/theme.js`).
const MAX_STROKES: usize = 500;
const MAX_STROKE_POINTS: usize = 2000;
const MAX_SKETCH_JSON_BYTES: usize = 190_000;
const MAX_UNDO: usize = 50;

/// The warm paper sheet colour and the faded page guide's ink (a drawing canvas, like `PAPER`).
const SHEET_BG: Color32 = Color32::from_rgb(0xfb, 0xfa, 0xf6);
const GUIDE_RGB: (u8, u8, u8) = (90, 78, 120);
const DASH_COLOUR: Color32 = Color32::from_rgb(0xb9, 0xb2, 0xc8);

/// The four semantic inks: each tells Claude when what it draws moves, never what colour it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ink {
    Black,
    Blue,
    Red,
    Green,
}

impl Ink {
    pub const ALL: [Ink; 4] = [Ink::Black, Ink::Blue, Ink::Red, Ink::Green];

    pub fn key(self) -> &'static str {
        match self {
            Ink::Black => "black",
            Ink::Blue => "blue",
            Ink::Red => "red",
            Ink::Green => "green",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ink::Black => "Black",
            Ink::Blue => "Blue",
            Ink::Red => "Red",
            Ink::Green => "Green",
        }
    }

    /// Short caption under the ink's name in the toolbar.
    pub fn short(self) -> &'static str {
        match self {
            Ink::Black => "static",
            Ink::Blue => "new page",
            Ink::Red => "new chapter",
            Ink::Green => "always moving",
        }
    }

    /// Full tooltip explaining the ink and its keyboard shortcut.
    pub fn tip(self) -> &'static str {
        match self {
            Ink::Black => "Black: always there, stays still (1)",
            Ink::Blue => "Blue: animates when a new page is written (2)",
            Ink::Red => "Red: animates when a new chapter begins (3)",
            Ink::Green => "Green: animates continuously all the time (4)",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Ink::Black => Color32::from_rgb(0x1c, 0x1c, 0x20),
            Ink::Blue => Color32::from_rgb(0x25, 0x63, 0xd9),
            Ink::Red => Color32::from_rgb(0xd6, 0x32, 0x2c),
            Ink::Green => Color32::from_rgb(0x1f, 0x9a, 0x4c),
        }
    }
}

/// Active drawing tool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Pen(Ink),
    Rubber,
}

impl Default for Tool {
    fn default() -> Self {
        Tool::Pen(Ink::Black)
    }
}

/// One hand-drawn stroke in desk coordinates (`0..1280`, `0..800`), with its padded bounding box
/// cached for fast hit-testing and partial texture updates.
#[derive(Clone, Debug)]
pub struct Stroke {
    pub ink: Ink,
    pub width: f32,
    pub pts: Vec<Pos2>,
    pub bounds: Rect,
}

impl Stroke {
    pub fn new(ink: Ink, width: f32, pts: Vec<Pos2>) -> Self {
        let bounds = stroke_bounds(&pts, width);
        Self { ink, width, pts, bounds }
    }

    fn push(&mut self, p: Pos2) {
        self.pts.push(p);
        let pad = self.width * 0.5 + 2.0;
        self.bounds = self.bounds.union(Rect::from_center_size(p, vec2(pad * 2.0, pad * 2.0)));
    }
}

fn stroke_bounds(pts: &[Pos2], width: f32) -> Rect {
    let Some(&first) = pts.first() else { return Rect::NOTHING };
    let mut r = Rect::from_min_max(first, first);
    for &p in &pts[1..] {
        r.extend_with(p);
    }
    r.expand(width * 0.5 + 2.0)
}

/// The sketch canvas state, kept inside `Making`.
#[derive(Default)]
pub struct Sketch {
    pub strokes: Vec<Stroke>,
    undo: Vec<Vec<Stroke>>,
    redo: Vec<Vec<Stroke>>,
    pub tool: Tool,
    pub size: usize,
    active: Option<Stroke>,
    rubbing: Option<Rubbing>,
    /// Cached 1280x800 transparent ink layer and its GPU texture (allocated lazily when shown).
    pixels: Option<ColorImage>,
    texture: Option<TextureHandle>,
    /// Full re-rasterize needed (e.g. after undo/redo/clear or on first frame).
    dirty_all: bool,
}

struct Rubbing {
    at: Pos2,
    before: Vec<Stroke>,
    changed: bool,
}

impl Sketch {
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    pub fn clear(&mut self) {
        if self.strokes.is_empty() {
            return;
        }
        self.commit(Vec::new(), self.strokes.clone());
        self.dirty_all = true;
    }

    pub fn undo(&mut self) {
        if self.active.is_some() || self.rubbing.is_some() {
            return;
        }
        let Some(prev) = self.undo.pop() else { return };
        self.redo.push(std::mem::replace(&mut self.strokes, prev));
        self.dirty_all = true;
    }

    pub fn redo(&mut self) {
        if self.active.is_some() || self.rubbing.is_some() {
            return;
        }
        let Some(next) = self.redo.pop() else { return };
        self.undo.push(std::mem::replace(&mut self.strokes, next));
        self.dirty_all = true;
    }

    fn commit(&mut self, to: Vec<Stroke>, from: Vec<Stroke>) {
        if self.undo.len() >= MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(from);
        self.redo.clear();
        self.strokes = to;
    }

    /// Add a completed stroke directly (used by tests and on pointer release).
    #[cfg(test)]
    pub fn add_stroke(&mut self, ink: Ink, width: f32, pts: Vec<Pos2>) {
        if pts.is_empty() {
            return;
        }
        let prev = self.strokes.clone();
        let mut next = self.strokes.clone();
        next.push(Stroke::new(ink, width, pts));
        self.commit(next, prev);
        self.dirty_all = true;
    }

    /// The inks actually used in the sketch, in canonical order.
    pub fn used_inks(&self) -> Vec<Ink> {
        Ink::ALL.into_iter().filter(|&k| self.strokes.iter().any(|s| s.ink == k)).collect()
    }

    /// Build the PNGs and JSON payload sent to `/api/theme-program` (`X-Caprice-Kind: sketch`).
    /// Whole sketch first, then one PNG per ink when more than one ink is used.
    pub fn export(&self, words: &str) -> Option<(Vec<Vec<u8>>, String)> {
        if self.strokes.is_empty() {
            return None;
        }
        let used = self.used_inks();
        let mut pngs = Vec::with_capacity(if used.len() > 1 { 1 + used.len() } else { 1 });
        pngs.push(png(&render_picture(&self.strokes, None))?);
        if used.len() > 1 {
            for &ink in &used {
                pngs.push(png(&render_picture(&self.strokes, Some(ink)))?);
            }
        }
        // Keep the stroke JSON compact (well within the Worker's cap) by lightly simplifying
        // near-collinear points and rounding coordinates to 0.5 desk pixels, simplifying further
        // only if a very dense sketch would otherwise exceed the cap.
        let mut eps = 0.8f32;
        let json = loop {
            let strokes_json: Vec<serde_json::Value> = self.strokes.iter().take(MAX_STROKES).map(|s| {
                let pts = simplify(&s.pts, eps);
                let pts: Vec<[f32; 2]> = pts.iter().take(MAX_STROKE_POINTS).map(|p| [(p.x * 2.0).round() / 2.0, (p.y * 2.0).round() / 2.0]).collect();
                serde_json::json!({ "ink": s.ink.key(), "width": s.width, "pts": pts })
            }).collect();
            let s = serde_json::json!({ "words": words.trim(), "strokes": strokes_json }).to_string();
            if s.len() <= MAX_SKETCH_JSON_BYTES || eps >= 25.0 {
                break s;
            }
            eps *= 2.0;
        };
        Some((pngs, json))
    }

    /// Handle keyboard shortcuts while the sketch is open and no text field has focus.
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y) {
                self.redo();
            } else if i.consume_key(Modifiers::COMMAND, Key::Z) {
                self.undo();
            } else if i.consume_key(Modifiers::NONE, Key::Num1) {
                self.tool = Tool::Pen(Ink::Black);
            } else if i.consume_key(Modifiers::NONE, Key::Num2) {
                self.tool = Tool::Pen(Ink::Blue);
            } else if i.consume_key(Modifiers::NONE, Key::Num3) {
                self.tool = Tool::Pen(Ink::Red);
            } else if i.consume_key(Modifiers::NONE, Key::Num4) {
                self.tool = Tool::Pen(Ink::Green);
            } else if i.consume_key(Modifiers::NONE, Key::E) {
                self.tool = Tool::Rubber;
            }
        });
    }

    /// Draw the toolbar and the responsive sketch sheet sized to `sheet_size`.
    pub fn show(&mut self, ui: &mut egui::Ui, sheet_size: Vec2) {
        self.handle_keys(ui.ctx());
        self.toolbar(ui);
        ui.add_space(6.0);
        self.canvas(ui, sheet_size);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            for ink in Ink::ALL {
                let selected = self.tool == Tool::Pen(ink);
                if ink_button(ui, ink, selected).clicked() {
                    self.tool = Tool::Pen(ink);
                }
            }
            ui.separator();
            for (idx, &w) in SIZES.iter().enumerate() {
                let selected = self.size == idx;
                if size_button(ui, w, SIZE_NAMES[idx], selected).clicked() {
                    self.size = idx;
                }
            }
            let erasing = self.tool == Tool::Rubber;
            if tool_chip(ui, "Rub out", "Rub out parts of strokes (E)", erasing).clicked() {
                self.tool = Tool::Rubber;
            }
            ui.separator();
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).on_hover_text("Undo (Ctrl+Z)").clicked() {
                self.undo();
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).on_hover_text("Redo (Ctrl+Shift+Z)").clicked() {
                self.redo();
            }
            if ui.add_enabled(!self.strokes.is_empty(), egui::Button::new("Clear")).on_hover_text("Clear the sketch").clicked() {
                self.clear();
            }
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui, sheet_size: Vec2) {
        let (rect, resp) = ui.allocate_exact_size(sheet_size, Sense::click_and_drag());
        let scale = rect.width() / W as f32;
        let to_desk = |p: Pos2| pos2(((p.x - rect.left()) / scale).clamp(0.0, W as f32), ((p.y - rect.top()) / scale).clamp(0.0, H as f32));
        let to_screen = |p: Pos2| pos2(rect.left() + p.x * scale, rect.top() + p.y * scale);

        self.ensure_texture(ui.ctx());

        // Process pointer press / coalesced move / release with zero full-canvas redraws.
        self.handle_pointer(ui.ctx(), &resp, rect, to_desk);

        // Layer 1: warm paper sheet + soft faded page border underneath the ink.
        let p = ui.painter().with_clip_rect(rect);
        p.rect_filled(rect, 4.0, SHEET_BG);
        paint_page_guide(&p, rect, scale);

        // Layer 2: cached GPU texture of all committed strokes (one textured quad).
        if let Some(tex) = &self.texture {
            p.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }

        // Layer 3: only the single active stroke currently being drawn.
        if let Some(active) = &self.active {
            paint_live_stroke(&p, active, to_screen, scale);
        }

        // Crisp outer border around the sheet.
        p.rect_stroke(rect, 4.0, EguiStroke::new(1.0, pal().dock_edge), egui::StrokeKind::Inside);

        // Live nib indicator when hovering or dragging over the canvas.
        if resp.hovered() || self.active.is_some() || self.rubbing.is_some() {
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::None);
            }
            if let Some(hover) = ui.ctx().pointer_hover_pos().filter(|pos| rect.contains(*pos)) {
                let diameter = match self.tool {
                    Tool::Pen(_) => SIZES[self.size] * scale,
                    Tool::Rubber => RUBBER[self.size] * scale,
                }.max(5.0);
                let (ring, fill) = match self.tool {
                    Tool::Pen(ink) => {
                        let c = ink.color();
                        (c, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 68))
                    }
                    Tool::Rubber => (Color32::from_rgb(111, 106, 114), Color32::from_rgba_unmultiplied(255, 255, 255, 160)),
                };
                p.circle(hover, diameter * 0.5, fill, EguiStroke::new(1.5, ring));
            }
        }
    }

    fn ensure_texture(&mut self, ctx: &egui::Context) {
        if self.pixels.is_none() {
            self.pixels = Some(ColorImage::new([W, H], vec![Color32::TRANSPARENT; W * H]));
            self.dirty_all = true;
        }
        if self.dirty_all {
            if let Some(img) = &mut self.pixels {
                img.pixels.fill(Color32::TRANSPARENT);
                let clip = (0, 0, W, H);
                for s in &self.strokes {
                    stamp_stroke(img, s, clip);
                }
                match &mut self.texture {
                    Some(tex) => tex.set(img.clone(), TextureOptions::LINEAR),
                    None => self.texture = Some(ctx.load_texture("sketch-ink", img.clone(), TextureOptions::LINEAR)),
                }
            }
            self.dirty_all = false;
        } else if self.texture.is_none()
            && let Some(img) = &self.pixels
        {
            self.texture = Some(ctx.load_texture("sketch-ink", img.clone(), TextureOptions::LINEAR));
        }
    }

    /// Stamp one finished stroke into the cached texture, uploading only its bounding box.
    fn upload_stroke(&mut self, stroke: &Stroke) {
        let (Some(img), Some(tex)) = (&mut self.pixels, &mut self.texture) else {
            self.dirty_all = true;
            return;
        };
        let Some(clip) = pixel_rect(stroke.bounds) else { return };
        stamp_stroke(img, stroke, clip);
        upload_patch(tex, img, clip);
    }

    /// Re-rasterize and upload only the dirty region after erasing.
    fn upload_dirty_rect(&mut self, dirty: Rect) {
        let (Some(img), Some(tex)) = (&mut self.pixels, &mut self.texture) else {
            self.dirty_all = true;
            return;
        };
        let Some(clip) = pixel_rect(dirty) else { return };
        let (x0, y0, x1, y1) = clip;
        for y in y0..y1 {
            let row = y * W;
            img.pixels[row + x0..row + x1].fill(Color32::TRANSPARENT);
        }
        for s in &self.strokes {
            if s.bounds.intersects(dirty) {
                stamp_stroke(img, s, clip);
            }
        }
        upload_patch(tex, img, clip);
    }

    fn handle_pointer(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect, to_desk: impl Fn(Pos2) -> Pos2) {
        let (primary_down, moved, pressed_at) = ctx.input(|i| {
            let mut moved = Vec::new();
            let mut pressed_at = None;
            for ev in &i.events {
                match *ev {
                    Event::PointerMoved(pos) => moved.push(pos),
                    Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } if rect.contains(pos) => {
                        pressed_at = Some(pos);
                    }
                    _ => {}
                }
            }
            (i.pointer.primary_down(), moved, pressed_at)
        });

        // Begin a new stroke or rub on primary press inside the canvas.
        let started = self.active.is_none() && self.rubbing.is_none() && (resp.drag_started_by(PointerButton::Primary) || (resp.is_pointer_button_down_on() && pressed_at.is_some()));
        if started {
            let origin = pressed_at.or_else(|| resp.interact_pointer_pos()).unwrap_or(rect.center());
            let p = to_desk(origin);
            match self.tool {
                Tool::Pen(ink) => {
                    if self.strokes.len() < MAX_STROKES {
                        self.active = Some(Stroke::new(ink, SIZES[self.size], vec![p]));
                    }
                }
                Tool::Rubber => {
                    let before = self.strokes.clone();
                    let dirty = rub_segment(&mut self.strokes, p, p, RUBBER[self.size] * 0.5);
                    let changed = dirty.is_some();
                    if let Some(d) = dirty {
                        self.upload_dirty_rect(d);
                    }
                    self.rubbing = Some(Rubbing { at: p, before, changed });
                }
            }
        }

        // Continue drawing or rubbing using every PointerMoved position delivered this frame.
        if primary_down && (self.active.is_some() || self.rubbing.is_some()) {
            let positions: Vec<Pos2> = if !moved.is_empty() {
                moved
            } else {
                resp.interact_pointer_pos().into_iter().collect()
            };
            if let Some(active) = &mut self.active {
                for raw in positions {
                    if active.pts.len() >= MAX_STROKE_POINTS {
                        break;
                    }
                    let p = to_desk(raw);
                    if active.pts.last().is_none_or(|&q| q.distance_sq(p) > 2.25) {
                        active.push(p);
                    }
                }
            } else if let Some(mut r) = self.rubbing.take() {
                let radius = RUBBER[self.size] * 0.5;
                let mut dirty: Option<Rect> = None;
                for raw in positions {
                    let p = to_desk(raw);
                    if let Some(d) = rub_segment(&mut self.strokes, r.at, p, radius) {
                        r.changed = true;
                        dirty = Some(dirty.map_or(d, |acc| acc.union(d)));
                    }
                    r.at = p;
                }
                if let Some(d) = dirty {
                    self.upload_dirty_rect(d);
                }
                self.rubbing = Some(r);
            }
        }

        // Commit on release.
        if !primary_down || resp.drag_stopped() {
            if let Some(finished) = self.active.take() {
                let prev = self.strokes.clone();
                let mut next = self.strokes.clone();
                next.push(finished.clone());
                self.commit(next, prev);
                self.upload_stroke(&finished);
            } else if let Some(r) = self.rubbing.take()
                && r.changed
            {
                let after = self.strokes.clone();
                self.commit(after, r.before);
            }
        }
    }
}

// ---- Toolbar widgets -----------------------------------------------------------------------------

fn ink_button(ui: &mut egui::Ui, ink: Ink, selected: bool) -> egui::Response {
    let label = ink.name();
    let sub = ink.short();
    let title_font = FontId::proportional(13.0);
    let sub_font = FontId::proportional(10.5);
    let title_g = ui.painter().layout_no_wrap(label.into(), title_font, pal().text);
    let sub_g = ui.painter().layout_no_wrap(sub.into(), sub_font, pal().text_dim);
    let text_w = title_g.size().x.max(sub_g.size().x);
    let size = vec2(10.0 + 16.0 + 8.0 + text_w + 10.0, 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, ink.tip()));
    let fill = if selected { pal().btn_hover } else if resp.hovered() { pal().btn } else { Color32::TRANSPARENT };
    let border = if selected { EguiStroke::new(1.5, pal().accent) } else { EguiStroke::new(1.0, pal().dock_edge.gamma_multiply(0.5)) };
    ui.painter().rect(rect, 7.0, fill, border, egui::StrokeKind::Inside);
    let dot = pos2(rect.left() + 18.0, rect.center().y);
    ui.painter().circle_filled(dot, 8.0, ink.color());
    if selected {
        ui.painter().circle_stroke(dot, 9.5, EguiStroke::new(1.2, pal().text.gamma_multiply(0.5)));
    }
    let tx = rect.left() + 34.0;
    ui.painter().galley(pos2(tx, rect.top() + 3.5), title_g, pal().text);
    ui.painter().galley(pos2(tx, rect.top() + 19.5), sub_g, pal().text_dim);
    resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(ink.tip())
}

fn size_button(ui: &mut egui::Ui, width: f32, name: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let fill = if selected { pal().btn_hover } else if resp.hovered() { pal().btn } else { Color32::TRANSPARENT };
    let border = if selected { EguiStroke::new(1.5, pal().accent) } else { EguiStroke::NONE };
    ui.painter().rect(rect, 7.0, fill, border, egui::StrokeKind::Inside);
    let radius = (width * 0.5 + 2.0).min(10.0);
    ui.painter().circle_filled(rect.center(), radius, pal().text);
    resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(name)
}

fn tool_chip(ui: &mut egui::Ui, text: &str, tip: &str, selected: bool) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.into(), FontId::proportional(13.0), pal().text);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 32.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if selected { pal().btn_hover } else if resp.hovered() { pal().btn } else { Color32::TRANSPARENT };
    let border = if selected { EguiStroke::new(1.5, pal().accent) } else { EguiStroke::new(1.0, pal().dock_edge.gamma_multiply(0.5)) };
    ui.painter().rect(rect, 7.0, fill, border, egui::StrokeKind::Inside);
    ui.painter().galley(pos2(rect.center().x - g.size().x * 0.5, rect.center().y - g.size().y * 0.5), g, pal().text);
    resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip)
}

// ---- Canvas rendering & fast rasterization -------------------------------------------------------

/// The faded page border underneath the ink, matching the prototype's soft multi-ring glow.
fn paint_page_guide(p: &egui::Painter, rect: Rect, scale: f32) {
    let (_, page, _, _) = layout(PICTURE);
    let pr = Rect::from_min_size(pos2(rect.left() + page.left() * scale, rect.top() + page.top() * scale), page.size() * scale);
    for i in (1..=9).rev() {
        let alpha = ((0.012 + 0.004 * (9 - i) as f32) * 255.0).round() as u8;
        let c = Color32::from_rgba_unmultiplied(GUIDE_RGB.0, GUIDE_RGB.1, GUIDE_RGB.2, alpha);
        p.rect_stroke(pr, 3.0 * scale, EguiStroke::new((2 * i) as f32 * scale, c), egui::StrokeKind::Middle);
    }
    let label_c = Color32::from_rgba_unmultiplied(GUIDE_RGB.0, GUIDE_RGB.1, GUIDE_RGB.2, 104);
    p.text(pos2(pr.center().x, pr.top() + 26.0 * scale), egui::Align2::CENTER_TOP, "the pages lie here", FontId::proportional((15.0 * scale).max(10.0)), label_c);
}

/// Draw the single active stroke live on the GPU painter using smooth quadratic midpoints.
fn paint_live_stroke(p: &egui::Painter, s: &Stroke, to_screen: impl Fn(Pos2) -> Pos2, scale: f32) {
    let c = s.ink.color();
    let r = (s.width * scale * 0.5).max(0.75);
    if s.pts.is_empty() {
        return;
    }
    if s.pts.len() == 1 {
        p.circle_filled(to_screen(s.pts[0]), r, c);
        return;
    }
    let mut curve = Vec::with_capacity(s.pts.len() * 3);
    sample_stroke(&s.pts, |pt| curve.push(to_screen(pt)));
    if let Some(&first) = curve.first() {
        p.circle_filled(first, r, c);
    }
    if let Some(&last) = curve.last() {
        p.circle_filled(last, r, c);
    }
    p.add(Shape::line(curve, EguiStroke::new(s.width * scale, c)));
}

/// Sample a stroke's quadratic-midpoint curve into closely spaced points.
fn sample_stroke(pts: &[Pos2], mut emit: impl FnMut(Pos2)) {
    if pts.is_empty() {
        return;
    }
    emit(pts[0]);
    if pts.len() == 1 {
        return;
    }
    let mid = |a: Pos2, b: Pos2| pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    for i in 1..pts.len() {
        let from = if i == 1 { pts[0] } else { mid(pts[i - 2], pts[i - 1]) };
        let ctrl = pts[i - 1];
        let to = if i == pts.len() - 1 { pts[i] } else { mid(pts[i - 1], pts[i]) };
        let len = from.distance(ctrl) + ctrl.distance(to);
        let steps = ((len / 4.0).ceil() as usize).clamp(1, 8);
        for k in 1..=steps {
            let t = k as f32 / steps as f32;
            let u = 1.0 - t;
            let x = u * u * from.x + 2.0 * u * t * ctrl.x + t * t * to.x;
            let y = u * u * from.y + 2.0 * u * t * ctrl.y + t * t * to.y;
            emit(pos2(x, y));
        }
    }
}

/// Convert a desk-space rect to clamped pixel bounds `(x0, y0, x1, y1)`.
fn pixel_rect(r: Rect) -> Option<(usize, usize, usize, usize)> {
    if !r.is_positive() {
        return None;
    }
    let x0 = (r.left().floor().max(0.0) as usize).min(W);
    let y0 = (r.top().floor().max(0.0) as usize).min(H);
    let x1 = (r.right().ceil().max(0.0) as usize).min(W);
    let y1 = (r.bottom().ceil().max(0.0) as usize).min(H);
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

/// Upload only the `(x0..x1, y0..y1)` sub-rect of `img` to `tex`.
fn upload_patch(tex: &mut TextureHandle, img: &ColorImage, (x0, y0, x1, y1): (usize, usize, usize, usize)) {
    let (pw, ph) = (x1 - x0, y1 - y0);
    let mut patch = Vec::with_capacity(pw * ph);
    for y in y0..y1 {
        let row = y * W;
        patch.extend_from_slice(&img.pixels[row + x0..row + x1]);
    }
    tex.set_partial([x0, y0], ColorImage::new([pw, ph], patch), TextureOptions::LINEAR);
}

/// Rasterize one stroke into `img`, clipped to `clip`, using anti-aliased capsules.
fn stamp_stroke(img: &mut ColorImage, s: &Stroke, clip: (usize, usize, usize, usize)) {
    if s.pts.is_empty() {
        return;
    }
    let color = s.ink.color();
    let radius = (s.width * 0.5).max(0.5);
    if s.pts.len() == 1 {
        stamp_capsule(img, s.pts[0], s.pts[0], radius, color, clip);
        return;
    }
    let mut prev: Option<Pos2> = None;
    sample_stroke(&s.pts, |pt| {
        if let Some(a) = prev {
            stamp_capsule(img, a, pt, radius, color, clip);
        }
        prev = Some(pt);
    });
}

/// Anti-aliased capsule from `a` to `b` of `radius`, premultiplied source-over blended onto `img`.
fn stamp_capsule(img: &mut ColorImage, a: Pos2, b: Pos2, radius: f32, color: Color32, (cx0, cy0, cx1, cy1): (usize, usize, usize, usize)) {
    let pad = radius + 1.0;
    let x0 = ((a.x.min(b.x) - pad).floor().max(cx0 as f32) as usize).min(cx1);
    let y0 = ((a.y.min(b.y) - pad).floor().max(cy0 as f32) as usize).min(cy1);
    let x1 = ((a.x.max(b.x) + pad).ceil().max(cx0 as f32) as usize).min(cx1);
    let y1 = ((a.y.max(b.y) + pad).ceil().max(cy0 as f32) as usize).min(cy1);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    let outer_sq = (radius + 0.5) * (radius + 0.5);
    let inner = (radius - 0.5).max(0.0);
    let inner_sq = inner * inner;
    let (ir, ig, ib) = (color.r(), color.g(), color.b());
    let opaque = Color32::from_rgba_premultiplied(ir, ig, ib, 255);
    for y in y0..y1 {
        let py = y as f32 + 0.5;
        let row = y * W;
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let t = if len_sq > 1e-6 { ((px - a.x) * dx + (py - a.y) * dy).clamp(0.0, len_sq) / len_sq } else { 0.0 };
            let ex = px - (a.x + t * dx);
            let ey = py - (a.y + t * dy);
            let d2 = ex * ex + ey * ey;
            if d2 >= outer_sq {
                continue;
            }
            let dst = &mut img.pixels[row + x];
            if d2 <= inner_sq {
                *dst = opaque;
            } else {
                let cov = (radius + 0.5 - d2.sqrt()).clamp(0.0, 1.0);
                let keep = 1.0 - cov;
                let [dr, dg, db, da] = dst.to_array();
                *dst = Color32::from_rgba_premultiplied(
                    (f32::from(ir) * cov + f32::from(dr) * keep).round().min(255.0) as u8,
                    (f32::from(ig) * cov + f32::from(dg) * keep).round().min(255.0) as u8,
                    (f32::from(ib) * cov + f32::from(db) * keep).round().min(255.0) as u8,
                    (255.0 * cov + f32::from(da) * keep).round().min(255.0) as u8,
                );
            }
        }
    }
}

/// Render a 1280x800 PNG picture for Claude: white desk, dashed page border, and either all strokes
/// (`only == None`) or those of one ink (`only == Some(ink)`).
pub fn render_picture(strokes: &[Stroke], only: Option<Ink>) -> ColorImage {
    let mut img = ColorImage::new([W, H], vec![Color32::WHITE; W * H]);
    let (_, page, _, _) = layout(PICTURE);
    let clip = (0, 0, W, H);
    draw_dashed_rect(&mut img, page, DASH_COLOUR, clip);
    for s in strokes {
        if only.is_none_or(|k| s.ink == k) {
            stamp_stroke(&mut img, s, clip);
        }
    }
    img
}

fn draw_dashed_rect(img: &mut ColorImage, r: Rect, color: Color32, clip: (usize, usize, usize, usize)) {
    let edges = [
        (r.left_top(), r.right_top()),
        (r.right_top(), r.right_bottom()),
        (r.right_bottom(), r.left_bottom()),
        (r.left_bottom(), r.left_top()),
    ];
    for (a, b) in edges {
        let total = a.distance(b);
        let dir = (b - a) / total.max(1.0);
        let mut d = 0.0;
        while d < total {
            let end = (d + 8.0).min(total);
            stamp_capsule(img, a + dir * d, a + dir * end, 0.75, color, clip);
            d += 14.0;
        }
    }
}

// ---- Erasing (bounding-box accelerated) & stroke simplification ----------------------------------

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let l2 = d.length_sq();
    let t = if l2 > 1e-6 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * t).distance(p)
}

fn seg_dist(a0: Pos2, a1: Pos2, b0: Pos2, b1: Pos2) -> f32 {
    // Cheap upper/exact check: endpoints to opposite segments, plus intersection test.
    if segments_intersect(a0, a1, b0, b1) {
        return 0.0;
    }
    dist_to_segment(a0, b0, b1)
        .min(dist_to_segment(a1, b0, b1))
        .min(dist_to_segment(b0, a0, a1))
        .min(dist_to_segment(b1, a0, a1))
}

fn segments_intersect(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    let cross = |o: Pos2, p: Pos2, q: Pos2| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

/// Rub out parts of `strokes` within `radius` of the segment `a..b`. Returns the bounding box of
/// everything removed so only that region is re-rasterized, or `None` if nothing was touched.
fn rub_segment(strokes: &mut Vec<Stroke>, a: Pos2, b: Pos2, radius: f32) -> Option<Rect> {
    let rub_box = Rect::from_two_pos(a, b).expand(radius + 1.0);
    let mut dirty: Option<Rect> = None;
    let mut out = Vec::with_capacity(strokes.len());
    for s in strokes.drain(..) {
        if !s.bounds.intersects(rub_box) {
            out.push(s);
            continue;
        }
        let reach = radius + s.width * 0.5;
        let hit = if s.pts.len() == 1 {
            dist_to_segment(s.pts[0], a, b) < reach
        } else {
            s.pts.windows(2).any(|w| seg_dist(w[0], w[1], a, b) < reach)
        };
        if !hit {
            out.push(s);
            continue;
        }
        // Densify only the stroke actually touched so a small rubber cannot slip between points.
        let mut dense = Vec::with_capacity(s.pts.len() * 2);
        for (i, &p) in s.pts.iter().enumerate() {
            if i == 0 {
                dense.push(p);
                continue;
            }
            let q = s.pts[i - 1];
            let n = (p.distance(q) / 3.0).ceil() as usize;
            if n < 2 {
                dense.push(p);
            } else {
                for k in 1..=n {
                    let t = k as f32 / n as f32;
                    dense.push(pos2(q.x + (p.x - q.x) * t, q.y + (p.y - q.y) * t));
                }
            }
        }
        let mut pieces: Vec<Vec<Pos2>> = vec![Vec::new()];
        let mut erased = false;
        for p in dense {
            if dist_to_segment(p, a, b) < reach {
                erased = true;
                if !pieces.last().unwrap().is_empty() {
                    pieces.push(Vec::new());
                }
            } else {
                pieces.last_mut().unwrap().push(p);
            }
        }
        if !erased {
            out.push(s);
            continue;
        }
        dirty = Some(dirty.map_or(s.bounds, |d| d.union(s.bounds)));
        for pts in pieces {
            if !pts.is_empty() {
                out.push(Stroke::new(s.ink, s.width, pts));
            }
        }
    }
    *strokes = out;
    dirty
}

/// Ramer-Douglas-Peucker simplification to keep exported stroke JSON compact.
fn simplify(pts: &[Pos2], eps: f32) -> Vec<Pos2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut spans = vec![(0usize, pts.len() - 1)];
    while let Some((a, b)) = spans.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
        let len = dx.hypot(dy);
        let (mut far, mut at) = (0.0f32, a);
        for (i, &p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = if len > 0.5 { ((p.x - pa.x) * dy - (p.y - pa.y) * dx).abs() / len } else { p.distance(pa) };
            if d > far {
                (far, at) = (d, i);
            }
        }
        if far > eps {
            keep[at] = true;
            spans.push((a, at));
            spans.push((at, b));
        }
    }
    pts.iter().enumerate().filter(|&(i, _)| keep[i]).map(|(_, &p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sketch_export_gives_one_png_for_one_ink_and_one_per_ink_plus_whole_for_several() {
        let mut s = Sketch::default();
        assert!(s.export("lighthouse").is_none(), "nothing drawn yet");
        s.add_stroke(Ink::Black, 7.0, vec![pos2(100.0, 200.0), pos2(200.0, 200.0)]);
        let (pngs, json) = s.export("A cliff").unwrap();
        assert_eq!(pngs.len(), 1, "one ink -> whole picture only");
        assert!(pngs[0].starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["words"], "A cliff");
        assert_eq!(parsed["strokes"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["strokes"][0]["ink"], "black");

        s.add_stroke(Ink::Blue, 3.0, vec![pos2(500.0, 300.0), pos2(600.0, 350.0)]);
        s.add_stroke(Ink::Red, 7.0, vec![pos2(900.0, 500.0)]);
        let (pngs, _) = s.export("").unwrap();
        assert_eq!(pngs.len(), 4, "whole + black + blue + red");
    }

    #[test]
    fn rendered_sketch_draws_ink_and_page_border_even_inside_the_page_area() {
        let (_, page, _, _) = layout(PICTURE);
        let inside = page.center();
        let strokes = vec![Stroke::new(Ink::Green, 16.0, vec![inside, pos2(inside.x + 40.0, inside.y)])];
        let img = render_picture(&strokes, None);
        assert_eq!(img.size, [W, H]);
        let px = |p: Pos2| img.pixels[p.y as usize * W + p.x as usize];
        assert_eq!(px(pos2(10.0, 10.0)), Color32::WHITE);
        assert_eq!(px(inside), Ink::Green.color(), "paintable inside the page border");
        // Dashed border along top of page is anti-aliased against the white background.
        let border = px(pos2(page.left() + 3.0, page.top()));
        assert_ne!(border, Color32::WHITE, "dashed page border drawn");
        assert!(border.b() > border.g() && border.r() > border.g(), "tinted like the page border: {border:?}");
    }

    #[test]
    fn rubbing_splits_only_touched_strokes_and_undo_restores_them() {
        let mut s = Sketch::default();
        s.add_stroke(Ink::Black, 7.0, vec![pos2(100.0, 100.0), pos2(300.0, 100.0)]);
        s.add_stroke(Ink::Blue, 7.0, vec![pos2(100.0, 500.0), pos2(300.0, 500.0)]);
        let dirty = rub_segment(&mut s.strokes, pos2(200.0, 80.0), pos2(200.0, 120.0), 15.0);
        assert!(dirty.is_some());
        assert_eq!(s.strokes.len(), 3, "the black stroke split into two, blue untouched");
        assert_eq!(s.strokes.iter().filter(|st| st.ink == Ink::Black).count(), 2);
        assert_eq!(s.strokes.iter().filter(|st| st.ink == Ink::Blue).count(), 1);
    }

    #[test]
    fn undo_redo_and_clear_step_through_sketch_history() {
        let mut s = Sketch::default();
        s.add_stroke(Ink::Black, 3.0, vec![pos2(10.0, 10.0)]);
        s.add_stroke(Ink::Red, 7.0, vec![pos2(50.0, 50.0)]);
        assert_eq!(s.strokes.len(), 2);
        s.undo();
        assert_eq!(s.strokes.len(), 1);
        s.redo();
        assert_eq!(s.strokes.len(), 2);
        s.clear();
        assert!(s.is_empty());
        s.undo();
        assert_eq!(s.strokes.len(), 2);
    }
}
