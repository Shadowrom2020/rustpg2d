//! The pixel editor, in a sprite sheet's tab: the frame picked above painted pixel by pixel
//! (pencil, eraser, fill, line, rectangle, colour picker; two colours on the two buttons), the
//! frame copied, mirrored and moved, with its own undo and *Save picture*.
//!
//! What painting does is `editor_core::pixels`; here are the canvas and its buttons.

use eframe::egui::{self, pos2, vec2, Color32, Sense, Stroke};
use editor_core::pixels::{line_points, rect_points, PixelDoc, PxTool};
use editor_core::Workspace;

/// Colours to start from when the picture has none yet.
const PALETTE: [u32; 16] = [0x000000, 0x3a3a48, 0x8f8f9a, 0xffffff, 0x6b4423, 0xb07e4a, 0xf2c49b, 0xb71c1c, 0xe53935, 0xe8c05a, 0x2e7d32, 0x7cc56f, 0x1f7a63, 0x3a72c8, 0x8ec9ef, 0x7b4fa0];

fn c32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn hex(c: [u8; 4]) -> String {
    if c[3] == 255 {
        format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    } else {
        format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
    }
}

fn parse_hex(s: &str) -> Option<[u8; 4]> {
    let s = s.trim_start_matches('#');
    let byte = |k: usize| u8::from_str_radix(s.get(k * 2..k * 2 + 2)?, 16).ok();
    match s.len() {
        6 => Some([byte(0)?, byte(1)?, byte(2)?, 255]),
        8 => Some([byte(0)?, byte(1)?, byte(2)?, byte(3)?]),
        _ => None,
    }
}

/// A stroke under way: the button (0 the first colour, 1 the second), where it began and
/// where the pointer was last.
type Stroke3 = (usize, (i32, i32), (i32, i32));

pub struct PixelEdit {
    /// The canvas is shown.
    pub open: bool,
    pub doc: Option<PixelDoc>,
    error: Option<String>,
    pub tool: PxTool,
    /// The first colour (left button) and the second (right button).
    pub colors: [[u8; 4]; 2],
    stroke: Option<Stroke3>,
    /// The frame as egui shows it: for which version of the picture and which frame.
    tex: Option<(egui::TextureHandle, u64, u32)>,
    /// The picture's colours, for which version.
    used: Option<(u64, Vec<[u8; 4]>)>,
    /// Where the canvas is in the window, and how many points a pixel is (for scripts).
    place: Option<(egui::Rect, f32)>,
}

impl Default for PixelEdit {
    fn default() -> PixelEdit {
        PixelEdit { open: false, doc: None, error: None, tool: PxTool::Pencil, colors: [[0, 0, 0, 255], [255, 255, 255, 255]], stroke: None, tex: None, used: None, place: None }
    }
}

fn hovered_id() -> egui::Id {
    egui::Id::unique("pixel canvas under the pointer")
}

/// Whether the pointer was over a pixel canvas in the last frame: Ctrl+Z and Ctrl+Y are the
/// picture's then, not the project's.
pub fn has_pointer(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<u64>(hovered_id())).is_some_and(|f| now.saturating_sub(f) <= 1)
}

impl PixelEdit {
    pub fn dirty(&self) -> bool {
        self.doc.as_ref().is_some_and(|d| d.is_dirty())
    }

    /// Reads the sheet's picture (again when the sheet names another).
    fn load(&mut self, ws: &Workspace, id: &str) {
        let Some(def) = ws.project.db.sheets.iter().find(|s| s.id == id) else { return };
        let frame = (def.frame_w, def.frame_h);
        match &mut self.doc {
            Some(d) if d.rel == def.image => d.set_frame(frame),
            _ => match PixelDoc::open(&ws.project.root, &def.image, frame) {
                Ok(d) => (self.doc, self.error, self.tex, self.used) = (Some(d), None, None, None),
                Err(e) => (self.doc, self.error) = (None, Some(e)),
            },
        }
    }

    /// Writes the picture and shows it everywhere at once.
    pub fn save(&mut self, ws: &mut Workspace, id: &str) -> Result<(), String> {
        let Some(doc) = &mut self.doc else { return Ok(()) };
        if !doc.is_dirty() {
            return Ok(());
        }
        doc.save(&ws.project.root)?;
        if ws.gpu != editor_core::Gpu::No {
            if let Some(def) = ws.project.db.sheets.iter().find(|s| s.id == id).cloned() {
                ws.assets.load_sheet(&ws.project, &def);
            }
        }
        ws.stale = true;
        ws.set_status(format!("Saved {}", doc.rel));
        Ok(())
    }

    /// The script's line: `ID closed` or `ID frame=K WxH tool= colors=a/b saved|changed undo=N [canvas@X,Y zoom=Z]`.
    pub fn status(&self, id: &str) -> String {
        let Some(d) = self.doc.as_ref().filter(|_| self.open) else { return format!("{id} closed{}", self.error.as_ref().map(|e| format!(": {e}")).unwrap_or_default()) };
        let place = self.place.map(|(r, k)| format!(" canvas@{:.0},{:.0} zoom={k}", r.min.x, r.min.y)).unwrap_or_default();
        format!("{id} frame={} {}x{} tool={} colors={}/{} {} used={}{place}", d.cur, d.frame.0, d.frame.1, self.tool.label().to_lowercase().replace(' ', "_"), hex(self.colors[0]), hex(self.colors[1]), if d.is_dirty() { "changed" } else { "saved" }, d.colors(64).len())
    }

    /// The script's words: `open`, `close`, `frame K`, `tool NAME`, `color RRGGBB[AA] [2]`,
    /// `dot X Y [2]`, `line|rect X1 Y1 X2 Y2 [2]`, `fill X Y [2]`, `undo`, `redo`, `copy`,
    /// `paste`, `flip`, `clear`, `nudge DX DY`, `replace FROM TO [all]`, `save`.
    pub fn word(&mut self, ws: &mut Workspace, id: &str, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        let num = |i: usize| arg(i).parse::<i32>().map_err(|_| format!("pixels {}: a whole number is missing", arg(0)));
        match arg(0) {
            "" => return Ok(()),
            "open" => {
                self.open = true;
                self.load(ws, id);
                return self.error.clone().map_or(Ok(()), Err);
            }
            "close" => {
                self.open = false;
                return Ok(());
            }
            "save" => return self.save(ws, id),
            "tool" => {
                self.tool = PxTool::named(arg(1)).ok_or("tool pencil|eraser|fill|line|rectangle|pick")?;
                return Ok(());
            }
            "color" => {
                self.colors[(arg(2) == "2") as usize] = parse_hex(arg(1)).ok_or("color RRGGBB[AA] [2]")?;
                return Ok(());
            }
            _ => {}
        }
        let colors = self.colors;
        let Some(d) = self.doc.as_mut().filter(|_| self.open) else { return Err("the pixel editor isn't open: pixels open".into()) };
        match arg(0) {
            "frame" => d.cur = (num(1)?.max(0) as u32).min(d.count() - 1),
            "dot" => d.dot(self.tool, (num(1)?, num(2)?), colors[(arg(3) == "2") as usize]),
            "line" | "rect" => d.shape(if arg(0) == "line" { PxTool::Line } else { PxTool::Rect }, (num(1)?, num(2)?), (num(3)?, num(4)?), colors[(arg(5) == "2") as usize]),
            "fill" => d.shape(PxTool::Fill, (num(1)?, num(2)?), (0, 0), colors[(arg(3) == "2") as usize]),
            "undo" => {
                d.undo();
            }
            "redo" => {
                d.redo();
            }
            "copy" => d.copy(),
            "paste" => {
                if !d.paste() {
                    return Err("no frame was copied".into());
                }
            }
            "flip" => d.flip(),
            "clear" => d.clear(),
            "nudge" => d.nudge((num(1)?, num(2)?)),
            "replace" => {
                let (from, to) = (parse_hex(arg(1)).ok_or("replace FROM TO [all]")?, parse_hex(arg(2)).ok_or("replace FROM TO [all]")?);
                println!("[pixels] {} pixels", d.replace(from, to, arg(3) == "all"));
            }
            other => return Err(format!("pixels {other}: open, close, frame, tool, color, dot, line, rect, fill, undo, redo, copy, paste, flip, clear, nudge, replace, save")),
        }
        Ok(())
    }

    /// The section under a sheet's picture. `frame`: the frame picked above.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, id: &str, frame: Option<u32>) {
        ui.horizontal(|ui| {
            if ui.toggle_value(&mut self.open, "Paint").on_hover_text("Paint the frame picked above, pixel by pixel").clicked() && self.open {
                self.load(ws, id);
            }
            if !self.open {
                if self.dirty() {
                    ui.colored_label(ui.visuals().warn_fg_color, "the picture has changes that aren't saved");
                } else {
                    ui.weak("The frame picked above, pixel by pixel.");
                }
            }
        });
        if !self.open {
            return;
        }
        self.load(ws, id);
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
            return;
        }
        let mut save = false;
        {
            let Some(doc) = self.doc.as_mut() else { return };
            if let Some(f) = frame {
                doc.cur = f.min(doc.count() - 1);
            }
            // The tools and the two colours.
            ui.horizontal_wrapped(|ui| {
                for (tool, name, key, help) in PxTool::ALL {
                    if ui.selectable_label(self.tool == tool, name).on_hover_text(format!("{help} ({key})")).clicked() {
                        self.tool = tool;
                    }
                }
                ui.separator();
                for (k, name) in ["First colour (left button)", "Second colour (right button)"].into_iter().enumerate() {
                    let mut c = self.colors[k];
                    let r = ui.color_edit_button_srgba_unmultiplied(&mut c).on_hover_text(name);
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColorWell, true, name));
                    self.colors[k] = c;
                }
                if ui.small_button("Swap").on_hover_text("The two colours change places (X)").clicked() {
                    self.colors.swap(0, 1);
                }
            });
            // The picture's own colours, then a few to start from: left click the first, right the second.
            if !self.used.as_ref().is_some_and(|(v, _)| *v == doc.version) {
                self.used = Some((doc.version, doc.colors(24)));
            }
            let used = self.used.as_ref().map(|(_, c)| c.clone()).unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                let start: Vec<[u8; 4]> = PALETTE.iter().map(|c| [(c >> 16) as u8, (c >> 8) as u8, *c as u8, 255]).filter(|c| !used.contains(c)).collect();
                for (k, c) in used.iter().chain(start.iter()).enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Colour #{}", hex(*c))));
                    ui.painter().rect_filled(r, 2.0, c32(*c));
                    if self.colors[0] == *c {
                        ui.painter().rect_stroke(r, 2.0, Stroke::new(2.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
                    }
                    if resp.clicked() {
                        self.colors[0] = *c;
                    }
                    if resp.secondary_clicked() {
                        self.colors[1] = *c;
                    }
                    resp.on_hover_text(format!("#{}{}", hex(*c), if k < used.len() { " (a colour of the picture)" } else { "" }));
                }
            });
            // The frame as a whole, undo, save.
            ui.horizontal_wrapped(|ui| {
                if ui.button("Copy frame").clicked() {
                    doc.copy();
                }
                if ui.add_enabled(doc.clip.is_some(), egui::Button::new("Paste")).on_hover_text("The frame copied replaces this one").clicked() {
                    doc.paste();
                }
                if ui.button("Mirror").on_hover_text("Left to right: a figure facing the other way").clicked() {
                    doc.flip();
                }
                if ui.button("Clear").on_hover_text("Empties the frame").clicked() {
                    doc.clear();
                }
                for (label, d, tip) in [("Left", (-1, 0), "left"), ("Up", (0, -1), "up"), ("Down", (0, 1), "down"), ("Right", (1, 0), "right")] {
                    if ui.small_button(label).on_hover_text(format!("Moves what is painted a pixel {tip}")).clicked() {
                        doc.nudge(d);
                    }
                }
                ui.separator();
                if ui.add_enabled(doc.can_undo(), egui::Button::new("Undo")).on_hover_text("The picture's last change (Ctrl+Z over the picture)").clicked() {
                    doc.undo();
                }
                if ui.add_enabled(doc.can_redo(), egui::Button::new("Redo")).clicked() {
                    doc.redo();
                }
                if ui.add_enabled(doc.is_dirty(), egui::Button::new("Save picture")).on_hover_text(format!("Writes {} (Save all does too)", doc.rel)).clicked() {
                    save = true;
                }
                if doc.is_dirty() {
                    ui.colored_label(ui.visuals().warn_fg_color, "changed");
                }
            });
        }
        self.canvas(ui);
        if save {
            if let Err(e) = self.save(ws, id) {
                ws.set_status(e);
            }
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.doc.as_mut() else { return };
        let (fw, fh) = (doc.frame.0 as f32, doc.frame.1 as f32);
        // A whole number of points a pixel, as large as fits.
        let k = (420.0 / fw).min(420.0 / fh).floor().clamp(1.0, 32.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(fw, fh) * k, Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, "The frame to paint"));
        self.place = Some((rect, k));
        let painter = ui.painter_at(rect);
        // See-through shows as a chequered ground.
        painter.rect_filled(rect, 0.0, Color32::from_rgb(70, 72, 80));
        let step = (k * 4.0).max(8.0);
        let (nx, ny) = ((rect.width() / step).ceil() as i32, (rect.height() / step).ceil() as i32);
        for y in 0..ny {
            for x in (y % 2..nx).step_by(2) {
                painter.rect_filled(egui::Rect::from_min_size(rect.min + vec2(x as f32 * step, y as f32 * step), vec2(step, step)).intersect(rect), 0.0, Color32::from_rgb(92, 94, 104));
            }
        }
        // The frame's picture.
        if !self.tex.as_ref().is_some_and(|(_, v, f)| (*v, *f) == (doc.version, doc.cur)) {
            let px = doc.frame_pixels();
            let image = egui::ColorImage::from_rgba_unmultiplied([px.w as usize, px.h as usize], &px.px);
            match &mut self.tex {
                Some((t, v, f)) => {
                    t.set(image, egui::TextureOptions::NEAREST);
                    (*v, *f) = (doc.version, doc.cur);
                }
                None => self.tex = Some((ui.ctx().load_texture(format!("pixels:{}", doc.rel), image, egui::TextureOptions::NEAREST), doc.version, doc.cur)),
            }
        }
        if let Some((t, ..)) = &self.tex {
            painter.image(t.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        if k >= 8.0 {
            let line = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 40));
            for x in 1..doc.frame.0 {
                painter.line_segment([rect.min + vec2(x as f32 * k, 0.0), rect.min + vec2(x as f32 * k, rect.height())], line);
            }
            for y in 1..doc.frame.1 {
                painter.line_segment([rect.min + vec2(0.0, y as f32 * k), rect.min + vec2(rect.width(), y as f32 * k)], line);
            }
        }
        let cell = |p: (i32, i32)| egui::Rect::from_min_size(rect.min + vec2(p.0 as f32 * k, p.1 as f32 * k), vec2(k, k));
        // The pointer: which pixel, which buttons.
        let (pos, down, pressed) = ui.input(|i| (i.pointer.latest_pos(), [i.pointer.primary_down(), i.pointer.secondary_down()], [i.pointer.primary_pressed(), i.pointer.secondary_pressed()]));
        let at = pos.map(|p| (((p.x - rect.min.x) / k).floor() as i32, ((p.y - rect.min.y) / k).floor() as i32));
        let over = resp.hovered() || resp.is_pointer_button_down_on();
        if over {
            let now = ui.ctx().cumulative_frame_nr();
            ui.ctx().data_mut(|d| d.insert_temp(hovered_id(), now));
            // Keys while the pointer is over the picture: the tools, the colours swapped, undo.
            if !ui.ctx().text_edit_focused() {
                let (undo, redo, swap, tool) = ui.input(|i| {
                    let c = i.modifiers.command;
                    let tool = PxTool::ALL.iter().find(|t| !c && egui::Key::from_name(t.2).is_some_and(|key| i.key_pressed(key))).map(|t| t.0);
                    (c && !i.modifiers.shift && i.key_pressed(egui::Key::Z), c && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))), !c && i.key_pressed(egui::Key::X), tool)
                });
                if undo {
                    doc.undo();
                } else if redo {
                    doc.redo();
                }
                if swap {
                    self.colors.swap(0, 1);
                }
                if let Some(t) = tool {
                    self.tool = t;
                }
            }
        }
        // A stroke begins on the picture, goes on wherever the pointer is, ends when the button is let go.
        if self.stroke.is_none() && resp.hovered() {
            if let (Some(p), Some(button)) = (at, (0..2).find(|b| pressed[*b])) {
                match self.tool {
                    PxTool::Picker => self.colors[button] = doc.get(p),
                    PxTool::Fill => doc.shape(PxTool::Fill, p, p, self.colors[button]),
                    PxTool::Pencil | PxTool::Eraser => {
                        doc.snapshot();
                        doc.drag(self.tool, p, p, self.colors[button]);
                        doc.version += 1;
                        self.stroke = Some((button, p, p));
                    }
                    PxTool::Line | PxTool::Rect => self.stroke = Some((button, p, p)),
                }
            }
        } else if let Some((button, from, last)) = self.stroke {
            let now = at.unwrap_or(last);
            if down[button] {
                if matches!(self.tool, PxTool::Pencil | PxTool::Eraser) && now != last {
                    doc.drag(self.tool, last, now, self.colors[button]);
                    doc.version += 1;
                }
                self.stroke = Some((button, from, now));
            } else {
                match self.tool {
                    PxTool::Line | PxTool::Rect => doc.shape(self.tool, from, now, self.colors[button]),
                    _ => doc.changed(),
                }
                self.stroke = None;
            }
        }
        // A line or rectangle being dragged, shown before it is drawn.
        if let Some((button, from, now)) = self.stroke.filter(|_| matches!(self.tool, PxTool::Line | PxTool::Rect)) {
            let points = if self.tool == PxTool::Line { line_points(from, now) } else { rect_points(from, now) };
            for p in points.into_iter().filter(|p| doc.inside(*p)) {
                painter.rect_filled(cell(p), 0.0, c32(self.colors[button]));
            }
        }
        if let Some(p) = at.filter(|p| over && doc.inside(*p)) {
            painter.rect_stroke(cell(p), 0.0, Stroke::new(1.0_f32, Color32::WHITE), egui::StrokeKind::Outside);
            let c = doc.get(p);
            ui.weak(format!("Frame {} · pixel {}, {} · {}", doc.cur, p.0, p.1, if c[3] == 0 { "see-through".to_string() } else { format!("#{}", hex(c)) }));
        } else {
            ui.weak(format!("Frame {} · {} x {} pixels", doc.cur, doc.frame.0, doc.frame.1));
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use rustpg2d_engine::imageio::Rgba;

    struct State {
        ws: Workspace,
        edit: PixelEdit,
        frame: Option<u32>,
    }

    /// A new project with a sheet of 2 x 1 frames of 16 x 16 pixels, empty.
    fn harness(tag: &str) -> (Harness<'static, State>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("rustpg2d-pixeledit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        std::fs::create_dir_all(dir.join("sprites")).unwrap();
        std::fs::write(dir.join("sprites/icons.png"), Rgba::new(32, 16).encode().unwrap()).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.project.db.sheets.push(rustpg2d_engine::defs::SheetDef { image: "sprites/icons.png".into(), frame_w: 16, frame_h: 16, ..rustpg2d_engine::defs::SheetDef::fresh("icons") });
        ws.project.db.reindex();
        ws.history_frame(None);
        let h = Harness::builder().with_size(egui::vec2(900.0, 700.0)).build_ui_state(|ui, s: &mut State| s.edit.show(ui, &mut s.ws, "icons", s.frame), State { ws, edit: PixelEdit::default(), frame: None });
        (h, dir)
    }

    /// The middle of pixel `p` on the canvas.
    fn at(h: &Harness<'static, State>, p: (i32, i32)) -> egui::Pos2 {
        let (rect, k) = h.state().edit.place.expect("the canvas is shown");
        rect.min + vec2((p.0 as f32 + 0.5) * k, (p.1 as f32 + 0.5) * k)
    }

    #[test]
    fn a_frame_is_painted_with_the_pointer_and_saved() {
        let (mut h, dir) = harness("paint");
        h.run();
        h.get_by_label("Paint").click();
        h.run();
        h.run();
        assert_eq!(h.state().edit.status("icons").split(" canvas@").next(), Some("icons frame=0 16x16 tool=pencil colors=000000/ffffff saved used=0"));
        // A colour of the palette, then a stroke of the pencil along a row: one step.
        h.get_by_label("Colour #b71c1c").click();
        h.run();
        let (a, b) = (at(&h, (2, 5)), at(&h, (9, 5)));
        h.drag_at(a);
        h.run();
        h.hover_at(b);
        h.run();
        h.drop_at(b);
        h.run();
        h.run();
        let red = [0xb7, 0x1c, 0x1c, 255];
        let d = h.state().edit.doc.as_ref().unwrap();
        assert!((2..=9).all(|x| d.get((x, 5)) == red) && d.get((1, 5)) == [0; 4] && d.get((10, 5)) == [0; 4], "the row is drawn from 2 to 9");
        assert!(d.is_dirty() && d.can_undo());
        h.get_by_label("changed");
        // The second frame is picked above: the same strokes land there, the first keeps its own.
        h.state_mut().frame = Some(1);
        h.run();
        h.get_by_label("Fill").click();
        h.run();
        let p = at(&h, (0, 0));
        h.hover_at(p);
        h.run();
        h.drag_at(p);
        h.run();
        h.drop_at(p);
        h.run();
        h.run();
        let d = h.state().edit.doc.as_ref().unwrap();
        assert_eq!((d.cur, d.get((7, 7)), d.img.get(16 + 7, 7), d.img.get(7, 7)), (1, red, red, [0; 4]));
        // Undo takes the fill back, and the stroke after it; redo brings the stroke again.
        h.get_by_label("Undo").click();
        h.run();
        assert_eq!(h.state().edit.doc.as_ref().unwrap().img.get(16 + 7, 7), [0; 4]);
        h.get_by_label("Undo").click();
        h.run();
        assert!(!h.state().edit.dirty());
        h.get_by_label("Redo").click();
        h.run();
        // Saved: the file holds it.
        h.get_by_label("Save picture").click();
        h.run();
        assert!(!h.state().edit.dirty());
        let file = Rgba::decode(&std::fs::read(dir.join("sprites/icons.png")).unwrap()).unwrap();
        assert!(file.get(2, 5) == red && file.get(9, 5) == red && file.get(16 + 7, 7) == [0; 4]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
