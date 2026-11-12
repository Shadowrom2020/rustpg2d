//! Pixel editor: draw and edit the project's images (sprite sheets, tilesets, icons).

use egui_macroquad::egui::{self, pos2, vec2, Color32, PointerButton, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions};
use rustpg2d_engine::imageio::Rgba;

use crate::app::Editor;
use crate::ui::{self, ph};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PxTool {
    /// Default: a click only selects the frame under it (for copy / paste etc.), never draws.
    Select,
    Pencil,
    Eraser,
    Fill,
    Line,
    Rect,
    Picker,
}

impl PxTool {
    /// (tool, icon, name, key, help)
    const ALL: [(PxTool, &'static str, &'static str, &'static str, &'static str); 7] = [
        (PxTool::Select, ph::SELECTION, "Select frame", "S", "Click a frame to select it (for copy / cut / paste); nothing is drawn"),
        (PxTool::Pencil, ph::PENCIL_SIMPLE, "Pencil", "B", "Click or drag to draw single pixels with the left / right color"),
        (PxTool::Eraser, ph::ERASER, "Eraser", "E", "Click or drag to make pixels transparent"),
        (PxTool::Fill, ph::PAINT_BUCKET, "Fill", "G", "Click to fill the connected area of the same color"),
        (PxTool::Line, ph::LINE_SEGMENT, "Line", "L", "Drag to draw a straight line"),
        (PxTool::Rect, ph::RECTANGLE, "Rectangle", "R", "Drag to draw a rectangle outline"),
        (PxTool::Picker, ph::EYEDROPPER, "Pick color", "I", "Click a pixel to make its color the left / right color"),
    ];
    pub fn parse(s: &str) -> Option<PxTool> {
        Some(match s {
            "select" => PxTool::Select,
            "pencil" => PxTool::Pencil,
            "eraser" => PxTool::Eraser,
            "fill" => PxTool::Fill,
            "line" => PxTool::Line,
            "rect" => PxTool::Rect,
            "picker" => PxTool::Picker,
            _ => return None,
        })
    }
}

/// Labels the rows of directional sheets (the four-facing layout: one row per facing).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    Plain,
    Character,
}

/// An undo step: the image.
type Snap = Rgba;

pub struct PixelDoc {
    /// Project-relative path of the image.
    pub rel: String,
    pub img: Rgba,
    /// Frame (or tile) size of the grid.
    pub frame: (u32, u32),
    pub layout: Layout,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    /// The image as on disk; the doc is dirty only while `img` differs from it.
    saved: Rgba,
    dirty: bool,
    tex: Option<TextureHandle>,
    tex_dirty: bool,
}

impl PixelDoc {
    fn cols(&self) -> u32 {
        (self.img.w / self.frame.0.max(1)).max(1)
    }
    fn rows(&self) -> u32 {
        (self.img.h / self.frame.1.max(1)).max(1)
    }
    fn frame_origin(&self, f: u32) -> (i32, i32) {
        ((f % self.cols() * self.frame.0) as i32, (f / self.cols() * self.frame.1) as i32)
    }
    fn frame_at(&self, x: i32, y: i32) -> u32 {
        (y.max(0) as u32 / self.frame.1.max(1)) * self.cols() + (x.max(0) as u32 / self.frame.0.max(1)).min(self.cols() - 1)
    }
    /// Remembers the current image for undo (call before changing it).
    pub fn snapshot(&mut self) {
        self.undo.push(self.img.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn changed(&mut self) {
        self.dirty = self.img != self.saved;
        self.tex_dirty = true;
    }
    /// Whether the image differs from the file (undoing back to it makes it clean again).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn mark_saved(&mut self) {
        self.saved = self.img.clone();
        self.dirty = false;
    }
    pub fn undo(&mut self) {
        if let Some(img) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.img, img));
            self.changed();
        }
    }
    pub fn redo(&mut self) {
        if let Some(img) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.img, img));
            self.changed();
        }
    }
    /// Frame operations on the image.
    pub fn frames(&mut self) -> crate::frame_ops::Frames<'_> {
        crate::frame_ops::Frames { img: &mut self.img, frame: self.frame }
    }
    fn texture(&mut self, ctx: &egui::Context) -> TextureHandle {
        let ci = crate::widgets::color_image(ctx, self.img.w as usize, self.img.h as usize, &self.img.px);
        match &mut self.tex {
            Some(t) if !self.tex_dirty => t.clone(),
            Some(t) => {
                t.set(ci, TextureOptions::NEAREST);
                self.tex_dirty = false;
                t.clone()
            }
            None => {
                let t = ctx.load_texture(format!("px:{}", self.rel), ci, TextureOptions::NEAREST);
                self.tex = Some(t.clone());
                self.tex_dirty = false;
                t
            }
        }
    }

    // ---------------------------------------------------------------- drawing operations

    pub fn line(&mut self, a: (i32, i32), b: (i32, i32), c: [u8; 4]) {
        for (x, y) in line_points(a, b) {
            self.img.set(x, y, c);
        }
    }
    pub fn rect(&mut self, a: (i32, i32), b: (i32, i32), c: [u8; 4]) {
        let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
        for x in x0..=x1 {
            self.img.set(x, y0, c);
            self.img.set(x, y1, c);
        }
        for y in y0..=y1 {
            self.img.set(x0, y, c);
            self.img.set(x1, y, c);
        }
    }
    /// Flood fill of the contiguous area with the color at (x, y).
    pub fn fill(&mut self, x: i32, y: i32, c: [u8; 4]) {
        let target = self.img.get(x, y);
        if !self.img.in_bounds(x, y) || target == c {
            return;
        }
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            if !self.img.in_bounds(x, y) || self.img.get(x, y) != target {
                continue;
            }
            self.img.set(x, y, c);
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
    }
    fn frame_pixels(&self, f: u32) -> Rgba {
        let (ox, oy) = self.frame_origin(f);
        let mut out = Rgba::new(self.frame.0, self.frame.1);
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                out.set(x, y, self.img.get(ox + x, oy + y));
            }
        }
        out
    }
    fn put_frame(&mut self, f: u32, src: &Rgba, flip: bool) {
        let (ox, oy) = self.frame_origin(f);
        for y in 0..self.frame.1.min(src.h) as i32 {
            for x in 0..self.frame.0.min(src.w) as i32 {
                let sx = if flip { src.w as i32 - 1 - x } else { x };
                self.img.set(ox + x, oy + y, src.get(sx, y));
            }
        }
    }
}

/// Pixels on the line between two points (Bresenham).
fn line_points(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    let mut out = Vec::new();
    loop {
        out.push((x, y));
        if (x, y) == b {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// A pleasant default palette (32 colors).
const PALETTE: [u32; 32] = [
    0x000000, 0x1a1622, 0x3a3a48, 0x5f5f6e, 0x8f8f9a, 0xbcbcc8, 0xe8e8f0, 0xffffff, 0x3e2410, 0x6b4423, 0x8a5a2e, 0xb07e4a, 0xe2cf92, 0xf2c49b, 0xc68a5e, 0x7a1010,
    0xb71c1c, 0xe53935, 0xff8a80, 0xe8c05a, 0xfff2a8, 0xff9f43, 0x1d5124, 0x2e7d32, 0x43a047, 0x7cc56f, 0x1f7a63, 0x4fc3a1, 0x3a72c8, 0x8ec9ef, 0x7b4fa0, 0xf06292,
];

fn c32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}
fn rgba(c: Color32) -> [u8; 4] {
    c.to_srgba_unmultiplied()
}

/// Mouse button, start pixel and last pixel of a stroke.
type Stroke3 = (usize, (i32, i32), (i32, i32));

pub struct PixelState {
    pub doc: Option<PixelDoc>,
    pub tool: PxTool,
    /// Left and right mouse button colors.
    pub colors: [[u8; 4]; 2],
    zoom: f32,
    pan: egui::Vec2,
    /// Wheel / trackpad scrolling not yet turned into a zoom step (in notches).
    wheel: f32,
    /// Active stroke: (button, start pixel, last pixel).
    stroke: Option<Stroke3>,
    pub sel_frame: u32,
    fps: f32,
    /// Copied / cut frame, and the frame it came from (shown in the panel).
    clipboard: Option<Rgba>,
    clip_from: u32,
    grid: bool,
    resize: (u32, u32),
    checker: Option<TextureHandle>,
    /// Figures: the animation (0 walk, 1 idle, 2 attack, 3 hurt) and facing (0 down, 1 left,
    /// 2 right, 3 up) the animation panel shows, and whether it plays.
    pub anim: u32,
    pub dir: u32,
    pub playing: bool,
    /// Onion skin: the previous (and the next) frame of the animation shown faintly over the
    /// selected one.
    pub onion: bool,
    pub onion_next: bool,
    /// A copied row (Copy row).
    row_clip: Option<Vec<Rgba>>,
}

/// A frame operation of the sprite editor (buttons and editor script `px…` commands).
#[derive(Clone, Debug, PartialEq)]
pub enum FrameOp {
    /// The selected frame into the next column.
    Duplicate,
    /// Shift the selected frame's contents.
    Nudge(i32, i32),
    RowCopy,
    RowPaste,
    RowClear,
    /// Right rows = the mirrored Left rows of the selected animation (true: of all animations).
    Mirror(bool),
    /// Empty idle and hurt frames get the standing frame.
    FillStanding,
    /// Replace a colour in the selected frame / its row / the whole sheet.
    Replace { from: [u8; 3], to: [u8; 4], scope: Scope },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Frame,
    Row,
    Sheet,
}

impl Default for PixelState {
    fn default() -> Self {
        PixelState {
            doc: None,
            tool: PxTool::Select,
            colors: [[0x1a, 0x16, 0x22, 255], [0, 0, 0, 0]],
            zoom: 8.0,
            wheel: 0.0,
            pan: egui::Vec2::ZERO,
            stroke: None,
            sel_frame: 0,
            fps: 6.0,
            clipboard: None,
            clip_from: 0,
            grid: true,
            resize: (1, 1),
            checker: None,
            anim: 0,
            dir: 0,
            playing: true,
            onion: false,
            onion_next: false,
            row_clip: None,
        }
    }
}

impl PixelState {
    pub fn is_dirty(&self) -> bool {
        self.doc.as_ref().is_some_and(PixelDoc::is_dirty)
    }

    /// The selected frame of the open image (clamped to the frames it has).
    fn frame(&self) -> Option<u32> {
        let d = self.doc.as_ref()?;
        Some(self.sel_frame.min(d.cols() * d.rows() - 1))
    }

    /// Copies the selected frame (Ctrl+C). Returns the frame copied.
    pub fn copy_frame(&mut self) -> Option<u32> {
        let f = self.frame()?;
        self.clipboard = Some(self.doc.as_ref()?.frame_pixels(f));
        self.clip_from = f;
        Some(f)
    }

    /// Copies the selected frame and clears it (Ctrl+X), one undo step.
    pub fn cut_frame(&mut self) -> Option<u32> {
        let f = self.copy_frame()?;
        let d = self.doc.as_mut()?;
        d.snapshot();
        d.put_frame(f, &Rgba::new(d.frame.0, d.frame.1), false);
        d.changed();
        Some(f)
    }

    /// Replaces the selected frame with the clipboard (Ctrl+V), optionally mirrored; one undo step.
    pub fn paste_frame(&mut self, mirrored: bool) -> Option<u32> {
        let f = self.frame()?;
        let clip = self.clipboard.as_ref()?;
        let d = self.doc.as_mut()?;
        d.snapshot();
        // Transparent pixels of the frame are cleared too: the frame becomes exactly the copy.
        d.put_frame(f, &Rgba::new(d.frame.0, d.frame.1), false);
        d.put_frame(f, clip, mirrored);
        d.changed();
        Some(f)
    }

    /// Frames of the animation panel's animation: (first frame, count).
    pub fn anim_frames(&self) -> Option<(u32, u32)> {
        let d = self.doc.as_ref()?;
        let start = crate::frame_ops::anim_frame(self.anim, self.dir, d.cols());
        let count = crate::frame_ops::ANIMS[self.anim as usize % 4].1.min(d.cols());
        (start < d.cols() * d.rows()).then_some((start, count))
    }

    /// Shows animation `anim` facing `dir` (selects its first frame).
    pub fn show_anim(&mut self, anim: u32, dir: u32) {
        self.anim = anim.min(3);
        self.dir = dir.min(3);
        if let Some((start, _)) = self.anim_frames() {
            self.sel_frame = start;
        }
    }

    /// Runs a frame operation as one undo step. Returns a status message (None: nothing open, or
    /// nothing to do).
    pub fn frame_op(&mut self, op: FrameOp) -> Option<String> {
        let sel = self.frame()?;
        let anim = self.anim;
        let d = self.doc.as_mut()?;
        let before = d.img.clone();
        let mut fr = d.frames();
        let cols = fr.cols();
        let row = sel / cols;
        let msg = match &op {
            FrameOp::Duplicate => fr.duplicate(sel).then(|| format!("Frame {sel} copied into frame {}", sel + 1)),
            FrameOp::Nudge(dx, dy) => {
                fr.nudge(sel, *dx, *dy);
                Some(format!("Frame {sel} moved by {dx},{dy}"))
            }
            FrameOp::RowCopy => {
                self.row_clip = Some(fr.row(row));
                return Some(format!("Copied row {row}"));
            }
            FrameOp::RowPaste => {
                let clip = self.row_clip.clone()?;
                fr.put_row(row, &clip);
                Some(format!("Pasted into row {row}"))
            }
            FrameOp::RowClear => {
                fr.clear_row(row);
                Some(format!("Cleared row {row}"))
            }
            FrameOp::Mirror(all) => {
                let rows = fr.mirror_left_to_right(if *all { None } else { Some(anim) });
                (!rows.is_empty()).then(|| format!("Right rows {rows:?} = the mirrored Left rows"))
            }
            FrameOp::FillStanding => {
                let n = fr.fill_standing();
                Some(format!("{n} empty idle / hurt frame(s) got the standing frame"))
            }
            FrameOp::Replace { from, to, scope } => {
                let frames: Option<Vec<u32>> = match scope {
                    Scope::Frame => Some(vec![sel]),
                    Scope::Row => Some((row * cols..(row + 1) * cols).collect()),
                    Scope::Sheet => None,
                };
                let n = fr.replace_color(*from, *to, frames.as_deref());
                Some(format!("Replaced {n} pixel(s)"))
            }
        };
        if d.img != before {
            d.undo.push(before);
            if d.undo.len() > 100 {
                d.undo.remove(0);
            }
            d.redo.clear();
            d.changed();
        }
        msg
    }

    /// `pixel=` for `status`.
    pub fn status(&self) -> String {
        match &self.doc {
            None => "closed".into(),
            Some(d) => format!(
                "{}:frame{}:{}_{}:{}{}{}:undo{}",
                d.rel,
                self.sel_frame,
                crate::frame_ops::ANIMS[self.anim as usize % 4].0,
                crate::frame_ops::DIRS[self.dir as usize % 4],
                if self.playing { "playing" } else { "paused" },
                if self.onion { ":onion" } else { "" },
                if d.is_dirty() { ":dirty" } else { "" },
                d.undo.len()
            ),
        }
    }

    /// Applies the current tool at a pixel as a complete action (used by scripts / clicks).
    pub fn apply_at(&mut self, button: usize, p: (i32, i32)) {
        let c = if self.tool == PxTool::Eraser { [0; 4] } else { self.colors[button] };
        let Some(d) = &mut self.doc else { return };
        match self.tool {
            PxTool::Select => {
                if d.img.in_bounds(p.0, p.1) {
                    self.sel_frame = d.frame_at(p.0, p.1);
                }
            }
            PxTool::Picker => self.colors[button] = d.img.get(p.0, p.1),
            PxTool::Fill => {
                d.snapshot();
                d.fill(p.0, p.1, c);
                d.changed();
            }
            _ => {
                d.snapshot();
                d.img.set(p.0, p.1, c);
                d.changed();
            }
        }
    }
}

impl Editor {
    /// Opens an image of the project in the pixel editor (`frame` = grid size).
    pub fn open_pixel(&mut self, rel: &str, frame: (u32, u32), layout: Layout) {
        if self.px.is_dirty() {
            self.set_status("Save or revert the image in the pixel editor first");
            self.mode = crate::app::Mode::Pixel;
            return;
        }
        if self.load_pixel(rel, frame, layout) {
            self.mode = crate::app::Mode::Pixel;
        }
    }

    /// Loads an image into the pixel editor (without switching to its tab). False on errors.
    pub fn load_pixel(&mut self, rel: &str, frame: (u32, u32), layout: Layout) -> bool {
        let path = self.project.root.join(rel);
        let img = match std::fs::read(&path).map_err(anyhow::Error::from).and_then(|b| Rgba::decode(&b)) {
            Ok(i) => i,
            Err(e) => {
                self.set_status(format!("Cannot open {rel} in the pixel editor: {e:#}"));
                return false;
            }
        };
        let frame = (frame.0.clamp(1, img.w), frame.1.clamp(1, img.h));
        self.px.resize = ((img.w / frame.0).max(1), (img.h / frame.1).max(1));
        // Fit the image into roughly 900x700 screen pixels.
        self.px.zoom = (900.0 / img.w as f32).min(700.0 / img.h as f32).floor().clamp(1.0, 24.0);
        self.px.pan = egui::Vec2::ZERO;
        self.px.sel_frame = 0;
        self.px.doc = Some(PixelDoc { rel: rel.to_string(), saved: img.clone(), img, frame, layout, undo: vec![], redo: vec![], dirty: false, tex: None, tex_dirty: true });
        true
    }

    /// The pixel editor inside another page (Database → Sprite sheets): tools on the left, the
    /// canvas on the right, `height` pixels tall. Loads `rel` if it isn't open yet.
    pub fn pixel_embedded(&mut self, ui: &mut egui::Ui, rel: &str, frame: (u32, u32), layout: Layout, height: f32) {
        let open = self.px.doc.as_ref().map(|d| d.rel.clone());
        if open.as_deref() != Some(rel) {
            if self.px.is_dirty() {
                let other = open.unwrap_or_default();
                ui.colored_label(Color32::YELLOW, format!("The pixel editor has unsaved changes in {other}."));
                ui.horizontal(|ui| {
                    if ui.button("Save them").clicked() {
                        self.save_pixel();
                    }
                    if ui.button("Discard them").clicked() {
                        self.px.doc = None;
                    }
                    if ui.button("Show them").clicked() {
                        self.mode = crate::app::Mode::Pixel;
                    }
                });
                return;
            }
            let path = self.project.root.join(rel);
            if !path.exists() {
                ui.label(format!("The image {rel} doesn't exist yet."));
                if ui.button(format!("Create a blank image ({}×{} px frames, 4×4 frames)", frame.0.max(1), frame.1.max(1))).clicked() {
                    let img = Rgba::new(frame.0.max(1) * 4, frame.1.max(1) * 4);
                    let res = img.encode().and_then(|b| {
                        if let Some(dir) = path.parent() {
                            std::fs::create_dir_all(dir)?;
                        }
                        Ok(std::fs::write(&path, b)?)
                    });
                    match res {
                        Ok(()) => self.reload_assets(),
                        Err(e) => self.set_status(format!("Could not create {rel}: {e:#}")),
                    }
                }
                return;
            }
            if !self.load_pixel(rel, frame, layout) {
                ui.colored_label(Color32::LIGHT_RED, format!("{rel} can't be opened (only PNG images can be edited)."));
                return;
            }
            // Fit the embedded canvas.
            if let Some(d) = &self.px.doc {
                let w = (ui.available_width() - 280.0).max(200.0);
                self.px.zoom = (w / d.img.w as f32).min((height - 40.0) / d.img.h as f32).floor().clamp(1.0, 16.0);
            }
        }
        if let Some(d) = &mut self.px.doc {
            // The grid follows the sheet's frame size as it is edited.
            if d.frame != frame && frame.0 > 0 && frame.1 > 0 {
                d.frame = frame;
            }
        }
        let down = egui::Layout::top_down(egui::Align::LEFT);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(262.0, height), down, |ui| {
                egui::ScrollArea::vertical().id_salt("px_embedded_side").max_height(height).show(ui, |ui| {
                    ui.set_width(250.0);
                    self.pixel_side(ui, true);
                });
            });
            ui.separator();
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), down, |ui| {
                ui.set_min_size(egui::vec2(ui.available_width(), height));
                self.pixel_canvas(ui);
            });
        });
        let ctx = ui.ctx().clone();
        self.pixel_keys(&ctx);
    }

    pub fn save_pixel(&mut self) {
        let Some(d) = &mut self.px.doc else { return };
        let res = d.img.encode().and_then(|b| Ok(std::fs::write(self.project.root.join(&d.rel), b)?));
        match res {
            Ok(()) => {
                d.mark_saved();
                let rel = d.rel.clone();
                self.reload_assets();
                self.set_status(format!("Saved {rel}"));
            }
            Err(e) => self.set_status(format!("Save failed: {e:#}")),
        }
    }

    /// Images of the project the pixel editor can open: (label, path, frame size, layout).
    fn pixel_targets(&self) -> Vec<(String, String, (u32, u32), Layout)> {
        let ts = rustpg2d_engine::project::TILE_SIZE;
        let db = &self.project.db;
        let mut out: Vec<_> = db
            .sheets
            .iter()
            .map(|s| (format!("Sheet: {}", s.id), s.image.clone(), (s.frame_w, s.frame_h), Layout::Plain))
            .collect();
        out.extend(db.tilesets.iter().map(|t| (format!("Tileset: {}", t.id), t.image.clone(), (ts, ts), Layout::Plain)));
        out
    }

    pub fn pixel_ui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("px_left").resizable(true).default_width(270.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("px_left_scroll").auto_shrink(false).show(ui, |ui| self.pixel_side(ui, false));
        });
        egui::CentralPanel::default().show(ctx, |ui| self.pixel_canvas(ui));
        self.pixel_keys(ctx);
    }

    /// Frame clipboard: "copy", "cut", "paste" or "mirror" (paste mirrored), with a status message.
    pub fn frame_clipboard(&mut self, action: &str) {
        let done = match action {
            "copy" => self.px.copy_frame().map(|f| format!("Copied frame {f}")),
            "cut" => self.px.cut_frame().map(|f| format!("Cut frame {f}: select another frame and press Ctrl+V to move it there")),
            "paste" | "mirror" => self.px.paste_frame(action == "mirror").map(|f| format!("Pasted frame {} into frame {f}", self.px.clip_from)),
            _ => None,
        };
        if let Some(msg) = done {
            self.set_status(msg);
        }
    }

    fn pixel_keys(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        // egui-miniquad turns Ctrl+C / X / V into text clipboard events (V only when the system
        // clipboard holds text), so these come from macroquad's keyboard state.
        {
            use macroquad::input::{is_key_pressed, KeyCode};
            if crate::ui::command_down() {
                for (k, action) in [(KeyCode::C, "copy"), (KeyCode::X, "cut"), (KeyCode::V, "paste")] {
                    if is_key_pressed(k) {
                        self.frame_clipboard(action);
                    }
                }
            }
        }
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        let pressed = |k: egui::Key| ctx.input(|i| i.key_pressed(k));
        if ctrl {
            if pressed(egui::Key::Z) && !shift {
                if let Some(d) = &mut self.px.doc {
                    d.undo();
                }
            }
            if pressed(egui::Key::Y) || (pressed(egui::Key::Z) && shift) {
                if let Some(d) = &mut self.px.doc {
                    d.redo();
                }
            }
            if pressed(egui::Key::S) {
                self.save_pixel();
            }
            return;
        }
        for (k, t) in [(egui::Key::S, PxTool::Select), (egui::Key::B, PxTool::Pencil), (egui::Key::E, PxTool::Eraser), (egui::Key::G, PxTool::Fill), (egui::Key::L, PxTool::Line), (egui::Key::R, PxTool::Rect), (egui::Key::I, PxTool::Picker)] {
            if pressed(k) {
                self.px.tool = t;
            }
        }
        if pressed(egui::Key::X) {
            self.px.colors.swap(0, 1);
        }
        // Shift+arrows move the selected frame's pixels (lining up feet between frames).
        if shift {
            for (k, dx, dy) in [(egui::Key::ArrowLeft, -1, 0), (egui::Key::ArrowRight, 1, 0), (egui::Key::ArrowUp, 0, -1), (egui::Key::ArrowDown, 0, 1)] {
                if pressed(k) {
                    self.px.frame_op(FrameOp::Nudge(dx, dy));
                }
            }
        }
    }

    fn pixel_side(&mut self, ui: &mut egui::Ui, embedded: bool) {
        let targets = self.pixel_targets();
        let current = self.px.doc.as_ref().map_or("(nothing open)".to_string(), |d| d.rel.clone());
        let dirty = self.px.is_dirty();
        let mut open = None;
        if !embedded {
            ui.heading("Pixel editor");
        }
        ui.add_enabled_ui(!dirty && !embedded, |ui| {
            if embedded {
                return;
            }
            egui::ComboBox::from_id_salt("px_open").selected_text(&current).width(240.0).show_ui(ui, |ui| {
                for (label, rel, frame, layout) in &targets {
                    if ui.selectable_label(*rel == current, label).on_hover_text(rel).clicked() {
                        open = Some((rel.clone(), *frame, *layout));
                    }
                }
            });
        });
        if let Some((rel, frame, layout)) = open {
            self.open_pixel(&rel, frame, layout);
        }
        if self.px.doc.is_none() {
            ui.weak("Open an image above, or create one with Assets \u{E13A} New asset…");
            return;
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(dirty, egui::Button::new(ui::t(ph::FLOPPY_DISK, "Save   Ctrl+S"))).on_hover_text("Writes the PNG and reloads it everywhere").clicked() {
                self.save_pixel();
            }
            if ui.add_enabled(dirty, egui::Button::new(ui::t(ph::ARROW_COUNTER_CLOCKWISE, "Revert"))).on_hover_text("Back to the saved image").clicked() {
                let d = self.px.doc.take().unwrap();
                self.load_pixel(&d.rel, d.frame, d.layout);
            }
            if !embedded && ui.add_enabled(!dirty, egui::Button::new(ui::t(ph::X, "Close"))).clicked() {
                self.px.doc = None;
            }
        });
        if dirty {
            ui.colored_label(Color32::YELLOW, "Unsaved changes");
        }
        let Some(d) = &mut self.px.doc else { return };
        ui.weak(format!("{}×{} px · frames {}×{} px · {}×{} frames", d.img.w, d.img.h, d.frame.0, d.frame.1, d.cols(), d.rows()));
        ui.horizontal(|ui| {
            let (u, r) = (!d.undo.is_empty(), !d.redo.is_empty());
            if ui::icon_button(ui, u, ph::ARROW_ARC_LEFT, "Undo", "Ctrl+Z", "").clicked() {
                d.undo();
            }
            if ui::icon_button(ui, r, ph::ARROW_ARC_RIGHT, "Redo", "Ctrl+Y", "").clicked() {
                d.redo();
            }
        });

        ui.separator();
        if let Some(d) = &mut self.px.doc {
            let mut four = d.layout == Layout::Character;
            if ui.checkbox(&mut four, "Four-facing rows").on_hover_text("The sheet's rows are facings (down, left, right, up) in blocks of walk, idle, attack and hurt: labelled rows and the animation panel").changed() {
                d.layout = if four { Layout::Character } else { Layout::Plain };
            }
        }
        ui.horizontal(|ui| {
            for (t, icon, name, key, help) in PxTool::ALL {
                if ui::tool_button(ui, self.px.tool == t, icon, name, key, help).clicked() {
                    self.px.tool = t;
                }
            }
        });
        if let Some((_, icon, name, _, help)) = PxTool::ALL.iter().find(|t| t.0 == self.px.tool) {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{icon} {name}")).strong());
                ui.weak(*help);
            });
        }

        ui.separator();
        ui.horizontal(|ui| {
            for (b, label) in [(0, "Left"), (1, "Right")] {
                ui.label(label);
                let mut c = c32(self.px.colors[b]);
                if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::OnlyBlend).changed() {
                    self.px.colors[b] = rgba(c);
                }
            }
            if ui.small_button("Swap").on_hover_text("Swap (X)").clicked() {
                self.px.colors.swap(0, 1);
            }
        });
        ui.weak("Left / right click draws with the left / right color. Right starts as transparent.");
        let swatches = |ui: &mut egui::Ui, colors: &[[u8; 4]], px: &mut PixelState| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                for &c in colors {
                    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    ui.painter().rect_filled(rect, 2.0, c32(c));
                    if c[3] == 0 {
                        ui.painter().line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(1.5_f32, Color32::RED));
                    }
                    let selected = px.colors[0] == c;
                    ui.painter().rect_stroke(rect, 2.0, Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { Color32::YELLOW } else { Color32::from_gray(60) }), StrokeKind::Inside);
                    if resp.clicked() {
                        px.colors[0] = c;
                    }
                    if resp.secondary_clicked() {
                        px.colors[1] = c;
                    }
                }
            });
        };
        let mut pal: Vec<[u8; 4]> = PALETTE.iter().map(|&v| [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]).collect();
        pal.push([0, 0, 0, 0]);
        ui.label("Palette");
        swatches(ui, &pal, &mut self.px);
        let d = self.px.doc.as_ref().unwrap();
        let mut used: Vec<[u8; 4]> = Vec::new();
        for &c in d.img.px.as_chunks::<4>().0 {
            if c[3] > 0 && !used.contains(&c) {
                used.push(c);
                if used.len() >= 48 {
                    break;
                }
            }
        }
        if !used.is_empty() {
            ui.label("Colors in this image");
            swatches(ui, &used, &mut self.px);
        }

        ui.separator();
        ui.checkbox(&mut self.px.grid, "Pixel grid (when zoomed in)");
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.px.zoom, 1.0..=32.0).step_by(1.0));
        });

        // Frames: copy / paste / flip, canvas size.
        let Some(d) = &mut self.px.doc else { return };
        ui.separator();
        ui.label(format!("Frame {} (click on the canvas selects it)", self.px.sel_frame));
        let f = self.px.sel_frame.min(d.cols() * d.rows() - 1);
        let mut clip: Option<&str> = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button(ui::t(ph::COPY, "Copy")).on_hover_text("Ctrl+C").clicked() {
                clip = Some("copy");
            }
            if ui.button(ui::t(ph::SCISSORS, "Cut")).on_hover_text("Ctrl+X: copy and clear the frame (paste it elsewhere to move it)").clicked() {
                clip = Some("cut");
            }
            if ui.add_enabled(self.px.clipboard.is_some(), egui::Button::new(ui::t(ph::CLIPBOARD_TEXT, "Paste"))).on_hover_text("Ctrl+V").clicked() {
                clip = Some("paste");
            }
            if ui.add_enabled(self.px.clipboard.is_some(), egui::Button::new(ui::t(ph::FLIP_HORIZONTAL, "Paste mirrored"))).on_hover_text("e.g. turn a left-facing frame into a right-facing one").clicked() {
                clip = Some("mirror");
            }
            if ui.button(ui::t(ph::ARROWS_LEFT_RIGHT, "Flip")).on_hover_text("Mirror the frame horizontally").clicked() {
                let cur = d.frame_pixels(f);
                d.snapshot();
                d.put_frame(f, &cur, true);
                d.changed();
            }
            if ui.button(ui::t(ph::ERASER, "Clear")).on_hover_text("Empty the frame").clicked() {
                d.snapshot();
                d.put_frame(f, &Rgba::new(d.frame.0, d.frame.1), false);
                d.changed();
            }
        });
        if self.px.clipboard.is_some() {
            ui.weak(format!("Clipboard: frame {} · click another frame, then Ctrl+V", self.px.clip_from));
        }
        if let Some(action) = clip {
            self.frame_clipboard(action);
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Canvas");
            ui.add(egui::DragValue::new(&mut self.px.resize.0).range(1..=64));
            ui.label("×");
            ui.add(egui::DragValue::new(&mut self.px.resize.1).range(1..=64));
            ui.label("frames");
            let (w, h) = (self.px.resize.0 * d.frame.0, self.px.resize.1 * d.frame.1);
            if ui.add_enabled((w, h) != (d.img.w, d.img.h), egui::Button::new("Resize")).on_hover_text("Adds or removes frames at the right / bottom").clicked() {
                d.snapshot();
                d.img = d.img.with_canvas(w, h);
                d.changed();
            }
        });

        if d.layout == Layout::Character {
            self.animation_panel(ui);
            return;
        }
        // Preview: selected frame large + animation of its row.
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Animation");
            ui.add(egui::Slider::new(&mut self.px.fps, 1.0..=16.0).suffix(" fps"));
        });
        let tex = d.texture(ui.ctx());
        let row = f / d.cols();
        let n = d.cols();
        let t = ui.input(|i| i.time);
        let anim = row * n + ((t * self.px.fps as f64) as u32 % n);
        ui.horizontal(|ui| {
            for frame in [f, anim] {
                frame_image(ui, d, &tex, frame, 110.0);
            }
        });
        ui.weak("Left: selected frame. Right: its row played as an animation.");
        ui.ctx().request_repaint();
    }

    /// Figures: the animation panel (animation, facing, play / pause / step, frame strip, onion
    /// skin) and the frame and row operations.
    fn animation_panel(&mut self, ui: &mut egui::Ui) {
        use crate::frame_ops::{ANIMS, DIRS};
        ui.separator();
        ui.label(egui::RichText::new("Animation").strong());
        let (mut anim, mut dir) = (self.px.anim, self.px.dir);
        ui.horizontal_wrapped(|ui| {
            for (k, (name, n)) in ANIMS.iter().enumerate() {
                let label = format!("{}{name} ({n})", name[..1].to_uppercase(), name = &name[1..]);
                if ui.selectable_label(anim == k as u32, label).clicked() {
                    anim = k as u32;
                }
            }
        });
        ui.horizontal(|ui| {
            for (k, (icon, name)) in [(ph::ARROW_DOWN, DIRS[0]), (ph::ARROW_LEFT, DIRS[1]), (ph::ARROW_RIGHT, DIRS[2]), (ph::ARROW_UP, DIRS[3])].iter().enumerate() {
                if ui.selectable_label(dir == k as u32, *icon).on_hover_text(*name).clicked() {
                    dir = k as u32;
                }
            }
            ui.separator();
            let icon = if self.px.playing { ph::PAUSE } else { ph::PLAY };
            if ui.button(icon).on_hover_text(if self.px.playing { "Pause" } else { "Play" }).clicked() {
                self.px.playing = !self.px.playing;
            }
            if ui.button(ph::SKIP_FORWARD).on_hover_text("Next frame of the animation (pauses)").clicked() {
                self.px.playing = false;
                if let (Some((start, count)), Some(f)) = (self.px.anim_frames(), self.px.frame()) {
                    let k = f.saturating_sub(start).min(count - 1);
                    self.px.sel_frame = start + (k + 1) % count;
                }
            }
        });
        if (anim, dir) != (self.px.anim, self.px.dir) {
            self.px.show_anim(anim, dir);
        }
        ui.horizontal(|ui| {
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut self.px.fps, 1.0..=16.0).suffix(" fps"));
        });
        let Some((start, count)) = self.px.anim_frames() else {
            ui.weak("This sheet has no rows for that animation yet: Canvas … Resize to 4 × 16 frames.");
            return;
        };
        let sel = self.px.frame().unwrap_or(start);
        let Some(d) = &mut self.px.doc else { return };
        let tex = d.texture(ui.ctx());
        let t = ui.input(|i| i.time);
        let shown = if self.px.playing { start + (t * self.px.fps as f64) as u32 % count } else { sel };
        ui.horizontal(|ui| {
            frame_image(ui, d, &tex, shown, 120.0);
            ui.vertical(|ui| {
                ui.weak(if self.px.playing { "Playing" } else { "Paused: the selected frame" });
                // The frame strip: click one to select it.
                ui.horizontal_wrapped(|ui| {
                    for k in 0..count {
                        let f = start + k;
                        let r = frame_image(ui, d, &tex, f, 44.0);
                        if f == sel {
                            ui.painter().rect_stroke(r.rect, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Outside);
                        }
                        if r.interact(Sense::click()).on_hover_text(format!("frame {f}")).clicked() {
                            self.px.sel_frame = f;
                            self.px.playing = false;
                        }
                    }
                });
            });
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.px.onion, "Onion skin").on_hover_text("The previous frame of the animation shown faintly (red) over the selected one, to line up the next pose");
            ui.checkbox(&mut self.px.onion_next, "and the next").on_hover_text("The next frame too (green)");
        });
        ui.ctx().request_repaint();

        ui.separator();
        ui.label(egui::RichText::new("Frames and rows").strong());
        let mut op: Option<FrameOp> = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button(ui::t(ph::COPY_SIMPLE, &format!("Duplicate {}", ph::CARET_RIGHT))).on_hover_text("The selected frame into the next column").clicked() {
                op = Some(FrameOp::Duplicate);
            }
            for (icon, dx, dy) in [(ph::ARROW_LEFT, -1, 0), (ph::ARROW_RIGHT, 1, 0), (ph::ARROW_UP, 0, -1), (ph::ARROW_DOWN, 0, 1)] {
                if ui.small_button(icon).on_hover_text("Move the frame's pixels by one (Shift+arrow keys), e.g. to line up the feet").clicked() {
                    op = Some(FrameOp::Nudge(dx, dy));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Copy row").clicked() {
                op = Some(FrameOp::RowCopy);
            }
            if ui.add_enabled(self.px.row_clip.is_some(), egui::Button::new("Paste row")).on_hover_text("Into the selected frame's row").clicked() {
                op = Some(FrameOp::RowPaste);
            }
            if ui.button("Clear row").clicked() {
                op = Some(FrameOp::RowClear);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(ui::t(ph::FLIP_HORIZONTAL, &format!("Mirror Left {} Right", ph::CARET_RIGHT))).on_hover_text("The Right row of this animation becomes the mirrored Left row").clicked() {
                op = Some(FrameOp::Mirror(false));
            }
            if ui.button("(all animations)").on_hover_text("Every animation's Right row from its Left row").clicked() {
                op = Some(FrameOp::Mirror(true));
            }
            if ui.button("Fill idle & hurt").on_hover_text("Empty idle and hurt frames get the standing walk frame of their facing").clicked() {
                op = Some(FrameOp::FillStanding);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Replace colour");
            let from = self.px.colors[1];
            let to = self.px.colors[0];
            ui.weak("right colour to left colour in the");
            for (label, scope) in [("frame", Scope::Frame), ("row", Scope::Row), ("sheet", Scope::Sheet)] {
                if ui.small_button(label).on_hover_text("Pick the colour to replace with the picker's right click; the left colour replaces it (e.g. a hair shade one step off the boots' shade, to split a colour feature)").clicked() && from[3] > 0 {
                    op = Some(FrameOp::Replace { from: [from[0], from[1], from[2]], to, scope });
                }
            }
        });
        if let Some(op) = op {
            if let Some(m) = self.px.frame_op(op) {
                self.set_status(m);
            }
        }
    }

    fn pixel_canvas(&mut self, ui: &mut egui::Ui) {
        let Some(d) = &mut self.px.doc else {
            ui.centered_and_justified(|ui| ui.weak("Nothing open"));
            return;
        };
        let avail = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(avail, Sense::click_and_drag());
        let zoom = self.px.zoom;
        let size = vec2(d.img.w as f32 * zoom, d.img.h as f32 * zoom);
        let origin = avail.center() + self.px.pan - size / 2.0;
        let img_rect = Rect::from_min_size(origin, size);
        let painter = ui.painter_at(avail);
        painter.rect_filled(avail, 0.0, Color32::from_gray(30));
        // Checkerboard behind transparent pixels: one square per 4x4 image pixels, baked into a
        // texture of matching size (rebuilt when the image size changes).
        let cw = d.img.w.div_ceil(4) as usize;
        let ch = d.img.h.div_ceil(4) as usize;
        if self.px.checker.as_ref().is_none_or(|t| t.size() != [cw, ch]) {
            let mut ci = egui::ColorImage::new([cw, ch], Color32::from_gray(90));
            for y in 0..ch {
                for x in 0..cw {
                    if (x + y) % 2 == 1 {
                        ci.pixels[y * cw + x] = Color32::from_gray(118);
                    }
                }
            }
            self.px.checker = Some(ui.ctx().load_texture("px_checker", ci, TextureOptions::NEAREST));
        }
        let checker = self.px.checker.as_ref().unwrap();
        let full = Rect::from_min_size(origin, vec2(cw as f32 * 4.0, ch as f32 * 4.0) * zoom);
        painter.with_clip_rect(img_rect.intersect(avail)).image(checker.id(), full, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let tex = d.texture(ui.ctx());
        painter.image(tex.id(), img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // Onion skin: the animation's previous (red) and next (green) frame over the selected one.
        if d.layout == Layout::Character && (self.px.onion || self.px.onion_next) {
            let sel = self.px.sel_frame.min(d.cols() * d.rows() - 1);
            let cols = d.cols();
            let row = sel / cols;
            let count = if row < 16 { crate::frame_ops::ANIMS[(row / 4) as usize].1.min(cols) } else { cols };
            let col = (sel % cols).min(count - 1);
            let (prev, next) = (row * cols + (col + count - 1) % count, row * cols + (col + 1) % count);
            let (sx, sy) = d.frame_origin(sel);
            let dest = Rect::from_min_size(origin + vec2(sx as f32, sy as f32) * zoom, vec2(d.frame.0 as f32, d.frame.1 as f32) * zoom);
            for (f, on, tint) in [(prev, self.px.onion, Color32::from_rgba_unmultiplied(255, 110, 110, 110)), (next, self.px.onion_next, Color32::from_rgba_unmultiplied(110, 255, 140, 110))] {
                if on && f != sel {
                    let (ox, oy) = d.frame_origin(f);
                    let uv = Rect::from_min_size(pos2(ox as f32 / d.img.w as f32, oy as f32 / d.img.h as f32), vec2(d.frame.0 as f32 / d.img.w as f32, d.frame.1 as f32 / d.img.h as f32));
                    painter.with_clip_rect(avail).image(tex.id(), dest, uv, tint);
                }
            }
        }

        // Grids.
        if self.px.grid && zoom >= 8.0 {
            let c = Color32::from_rgba_unmultiplied(255, 255, 255, 22);
            for x in 0..=d.img.w {
                let sx = origin.x + x as f32 * zoom;
                painter.line_segment([pos2(sx, origin.y), pos2(sx, origin.y + size.y)], Stroke::new(1.0_f32, c));
            }
            for y in 0..=d.img.h {
                let sy = origin.y + y as f32 * zoom;
                painter.line_segment([pos2(origin.x, sy), pos2(origin.x + size.x, sy)], Stroke::new(1.0_f32, c));
            }
        }
        let fc = Color32::from_rgba_unmultiplied(120, 200, 255, 140);
        for x in (0..=d.img.w).step_by(d.frame.0.max(1) as usize) {
            let sx = origin.x + x as f32 * zoom;
            painter.line_segment([pos2(sx, origin.y), pos2(sx, origin.y + size.y)], Stroke::new(1.0_f32, fc));
        }
        for y in (0..=d.img.h).step_by(d.frame.1.max(1) as usize) {
            let sy = origin.y + y as f32 * zoom;
            painter.line_segment([pos2(origin.x, sy), pos2(origin.x + size.x, sy)], Stroke::new(1.0_f32, fc));
        }
        let (fx, fy) = d.frame_origin(self.px.sel_frame.min(d.cols() * d.rows() - 1));
        let fr = Rect::from_min_size(origin + vec2(fx as f32, fy as f32) * zoom, vec2(d.frame.0 as f32, d.frame.1 as f32) * zoom);
        painter.rect_stroke(fr, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Outside);
        if d.layout == Layout::Character {
            // Walk rows, then the animation rows (render::character::ANIM_ROWS).
            const LABELS: [&str; 16] = ["Down", "Left", "Right", "Up", "Idle Down", "Idle Left", "Idle Right", "Idle Up", "Attack Down", "Attack Left", "Attack Right", "Attack Up", "Hurt Down", "Hurt Left", "Hurt Right", "Hurt Up"];
            for (r, label) in LABELS.iter().enumerate().take(d.rows() as usize) {
                let p = pos2(origin.x - 6.0, origin.y + (r as f32 + 0.5) * d.frame.1 as f32 * zoom);
                painter.text(p, egui::Align2::RIGHT_CENTER, *label, egui::FontId::proportional(14.0), Color32::LIGHT_GRAY);
            }
        }

        // Input.
        let hover = resp.hover_pos().map(|p| {
            let q = (p - origin) / zoom;
            (q.x.floor() as i32, q.y.floor() as i32)
        });
        if resp.hovered() {
            // Whole zoom steps: a wheel notch is one, trackpad deltas add up to one.
            self.px.wheel += ui.input(|i| i.raw_scroll_delta.y) / crate::EGUI_NOTCH;
            let scroll = if self.px.wheel.abs() >= 1.0 { std::mem::take(&mut self.px.wheel) } else { 0.0 };
            if scroll != 0.0 {
                let new = (zoom * if scroll > 0.0 { 1.25 } else { 0.8 }).clamp(1.0, 32.0).round();
                if let Some(p) = resp.hover_pos() {
                    // Keep the pixel under the cursor in place.
                    let rel = (p - origin) / zoom;
                    let new_origin = p - rel * new;
                    self.px.pan += new_origin - (avail.center() + self.px.pan - vec2(d.img.w as f32, d.img.h as f32) * new / 2.0);
                }
                self.px.zoom = new;
            }
        }
        if resp.dragged_by(PointerButton::Middle) {
            self.px.pan += resp.drag_delta();
        }
        let buttons = [PointerButton::Primary, PointerButton::Secondary];
        let (pressed, down) = ui.input(|i| {
            (buttons.map(|b| i.pointer.button_pressed(b)), buttons.map(|b| i.pointer.button_down(b)))
        });
        let tool = self.px.tool;
        if let Some(p) = hover.filter(|_| resp.hovered()) {
            if d.img.in_bounds(p.0, p.1) {
                // Select outlines the frame a click would select, the drawing tools the pixel.
                let r = if tool == PxTool::Select {
                    let (fx, fy) = d.frame_origin(d.frame_at(p.0, p.1));
                    Rect::from_min_size(origin + vec2(fx as f32, fy as f32) * zoom, vec2(d.frame.0 as f32, d.frame.1 as f32) * zoom)
                } else {
                    Rect::from_min_size(origin + vec2(p.0 as f32, p.1 as f32) * zoom, vec2(zoom, zoom))
                };
                painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Outside);
            }
            if let Some(b) = (0..2).find(|&b| pressed[b]) {
                if d.img.in_bounds(p.0, p.1) {
                    self.px.sel_frame = d.frame_at(p.0, p.1);
                }
                let c = if tool == PxTool::Eraser { [0; 4] } else { self.px.colors[b] };
                match tool {
                    PxTool::Select => {}
                    PxTool::Picker => self.px.colors[b] = d.img.get(p.0, p.1),
                    PxTool::Fill => {
                        d.snapshot();
                        d.fill(p.0, p.1, c);
                        d.changed();
                    }
                    PxTool::Pencil | PxTool::Eraser => {
                        d.snapshot();
                        d.img.set(p.0, p.1, c);
                        d.changed();
                        self.px.stroke = Some((b, p, p));
                    }
                    PxTool::Line | PxTool::Rect => self.px.stroke = Some((b, p, p)),
                }
            }
        }
        if let Some((b, start, last)) = self.px.stroke {
            let c = if tool == PxTool::Eraser { [0; 4] } else { self.px.colors[b] };
            let cur = hover.unwrap_or(last);
            if down[b] {
                match tool {
                    PxTool::Pencil | PxTool::Eraser if cur != last => {
                        d.line(last, cur, c);
                        d.changed();
                    }
                    PxTool::Line | PxTool::Rect => {
                        // Preview.
                        let pts = if tool == PxTool::Line { line_points(start, cur) } else { rect_points(start, cur) };
                        for (x, y) in pts {
                            let r = Rect::from_min_size(origin + vec2(x as f32, y as f32) * zoom, vec2(zoom, zoom));
                            painter.rect_filled(r, 0.0, c32(c).gamma_multiply(0.8));
                        }
                    }
                    _ => {}
                }
                self.px.stroke = Some((b, start, cur));
            } else {
                if matches!(tool, PxTool::Line | PxTool::Rect) {
                    d.snapshot();
                    if tool == PxTool::Line {
                        d.line(start, cur, c);
                    } else {
                        d.rect(start, cur, c);
                    }
                    d.changed();
                }
                self.px.stroke = None;
            }
        }
        let info = match hover {
            Some(p) if d.img.in_bounds(p.0, p.1) => format!("pixel {},{} · frame {}", p.0, p.1, d.frame_at(p.0, p.1)),
            _ => String::new(),
        };
        painter.text(avail.left_bottom() + vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("{info}   wheel: zoom · middle-drag: pan"), egui::FontId::monospace(12.0), Color32::GRAY);
    }
}


/// Paints frame `f` of the open image `size` pixels on its longer side.
fn frame_image(ui: &mut egui::Ui, d: &PixelDoc, tex: &TextureHandle, f: u32, size: f32) -> egui::Response {
    let (ox, oy) = d.frame_origin(f);
    let scale = (size / d.frame.0.max(d.frame.1) as f32).max(0.25);
    let dims = vec2(d.frame.0 as f32 * scale, d.frame.1 as f32 * scale);
    let (rect, resp) = ui.allocate_exact_size(dims, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, Color32::from_gray(48));
    let uv = Rect::from_min_size(pos2(ox as f32 / d.img.w as f32, oy as f32 / d.img.h as f32), vec2(d.frame.0 as f32 / d.img.w as f32, d.frame.1 as f32 / d.img.h as f32));
    ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
    resp
}

fn rect_points(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    let mut out = Vec::new();
    for x in x0..=x1 {
        out.push((x, y0));
        out.push((x, y1));
    }
    for y in y0..=y1 {
        out.push((x0, y));
        out.push((x1, y));
    }
    out
}
