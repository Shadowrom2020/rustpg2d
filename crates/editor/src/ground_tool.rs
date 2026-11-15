//! The Ground tool: paints a map's ground with materials ([`rustpg2d_engine::ground`]): a round or
//! square brush, a line (a road or a river of a width), a rectangle (a floor, a carpet, a field)
//! and a fill; *Snap to tiles* puts squares, rectangles and line ends on the tile grid. Shift
//! erases, right-click picks the material under the cursor. One drag is one undo step.

use egui_macroquad::egui;
use macroquad::prelude::*;

use crate::app::Editor;
use crate::ui::{self, ph};
use rustpg2d_engine::ground::{Ground, CELL};
use rustpg2d_engine::project::TILE_SIZE;

/// How the Ground tool paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundMode {
    /// Click or drag a brush (round or square).
    #[default]
    Brush,
    /// Drag from one end to the other: a stroke of the brush's width.
    Line,
    /// Drag from one corner to the other: a filled rectangle.
    Rect,
    /// Click: the connected area of the same material.
    Fill,
}

impl GroundMode {
    pub const ALL: [GroundMode; 4] = [GroundMode::Brush, GroundMode::Line, GroundMode::Rect, GroundMode::Fill];
    pub fn label(self) -> &'static str {
        match self {
            GroundMode::Brush => "Brush",
            GroundMode::Line => "Line",
            GroundMode::Rect => "Rectangle",
            GroundMode::Fill => "Fill",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            GroundMode::Brush => ph::PAINT_BRUSH_BROAD,
            GroundMode::Line => ph::LINE_SEGMENT,
            GroundMode::Rect => ph::RECTANGLE,
            GroundMode::Fill => ph::PAINT_BUCKET,
        }
    }
    fn help(self) -> &'static str {
        match self {
            GroundMode::Brush => "Click or drag a brush: round, or square for straight edges",
            GroundMode::Line => "Drag from one end to the other: a road or a stream as wide as the brush",
            GroundMode::Rect => "Drag from one corner to the other: a filled rectangle (a floor, a carpet, a field)",
            GroundMode::Fill => "Click: the connected area of the same material (or of no ground)",
        }
    }
    pub fn parse(s: &str) -> Option<GroundMode> {
        if s.eq_ignore_ascii_case("rect") {
            return Some(GroundMode::Rect);
        }
        GroundMode::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(s))
    }
}

/// A ground stroke in progress (the mouse is down).
pub enum GroundDrag {
    /// Brush: the last point painted and the cells changed so far (x, y, old value).
    Brush { last: Vec2, value: u8, cells: Vec<(i32, i32, u8)> },
    /// Line: where it started.
    Line { start: Vec2 },
    /// Rectangle: the corner where it started.
    Rect { start: Vec2 },
}

impl Editor {
    /// The cell value to paint (0 = erase), registering the material in the map's list; `None`
    /// when no material is chosen or the map is full.
    fn ground_value(&mut self, erase: bool) -> Option<u8> {
        if erase {
            return Some(0);
        }
        if self.ground_material.is_empty() {
            self.set_status("Pick a material in the left panel first (Shift erases)");
            return None;
        }
        let m = self.ground_material.clone();
        let v = self.doc.as_mut()?.map.material_value(&m);
        if v.is_none() {
            self.set_status(format!("A map can use at most {} materials", rustpg2d_engine::ground::MAX_MATERIALS));
        }
        v
    }

    /// The grid squares, rectangles and line ends snap to: whole tiles, else cells.
    fn ground_grid(&self) -> f32 {
        if self.ground_snap {
            TILE_SIZE as f32
        } else {
            CELL as f32
        }
    }

    /// A rectangle corner: the nearest grid corner.
    fn ground_corner(&self, p: Vec2) -> Vec2 {
        let g = self.ground_grid();
        (p / g).round() * g
    }

    /// A line end: the middle of the grid square under it (snapped), else the point itself.
    fn ground_line_end(&self, p: Vec2) -> Vec2 {
        if self.ground_snap {
            let g = self.ground_grid();
            ((p / g).floor() + 0.5) * g
        } else {
            p
        }
    }

    /// The square brush at `p`: a block of grid squares about the brush's size, around the
    /// square under the cursor; (top-left, bottom-right) in world pixels.
    pub(crate) fn ground_square_at(&self, p: Vec2) -> (Vec2, Vec2) {
        let g = self.ground_grid();
        let n = ((self.ground_size * 2.0 / g).round() as i32).max(1);
        let start = (p / g).floor() - vec2(((n - 1) / 2) as f32, ((n - 1) / 2) as f32);
        (start * g, (start + vec2(n as f32, n as f32)) * g)
    }

    /// One dab of the brush at `p`.
    fn ground_dab(&self, g: &mut Ground, p: Vec2, v: u8) -> Vec<(i32, i32, u8)> {
        if self.ground_square {
            let (a, b) = self.ground_square_at(p);
            g.paint_rect((a.x, a.y), (b.x, b.y), v)
        } else {
            g.paint_circle((p.x, p.y), self.ground_size, v)
        }
    }

    /// Mouse pressed on the map with the Ground tool.
    pub(crate) fn ground_down(&mut self, world: Vec2, erase: bool) {
        let Some(v) = self.ground_value(erase) else { return };
        match self.ground_mode {
            GroundMode::Brush => {
                let Some(mut doc) = self.doc.take() else { return };
                let cells = self.ground_dab(&mut doc.map.ground, world, v);
                self.doc = Some(doc);
                self.ground_drag = Some(GroundDrag::Brush { last: world, value: v, cells });
            }
            GroundMode::Line => self.ground_drag = Some(GroundDrag::Line { start: self.ground_line_end(world) }),
            GroundMode::Rect => self.ground_drag = Some(GroundDrag::Rect { start: self.ground_corner(world) }),
            GroundMode::Fill => {
                let Some(doc) = &mut self.doc else { return };
                let c = CELL as f32;
                let cells = doc.map.ground.flood_fill((world.x / c).floor() as i32, (world.y / c).floor() as i32, v);
                let n = cells.len();
                doc.record_ground(cells, v, &self.assets);
                self.set_status(format!("Filled {n} cells"));
            }
        }
    }

    /// The mouse moved with the button down.
    pub(crate) fn ground_drag_to(&mut self, world: Vec2) {
        if !matches!(self.ground_drag, Some(GroundDrag::Brush { .. })) {
            return;
        }
        // (Lines and rectangles only paint when the button goes up.)
        let Some(GroundDrag::Brush { last, value, mut cells }) = self.ground_drag.take() else {
            return;
        };
        if world.distance (last) < 1.0 {
            self.ground_drag = Some(GroundDrag::Brush { last, value, cells });
            return;
        }
        if let Some(mut doc) = self.doc.take() {
            if self.ground_square {
                // Dabs every few pixels along the way.
                let steps = (world.distance(last) / (CELL as f32 * 0.5)).ceil().max(1.0) as i32;
                for k in 1..=steps {
                    let p = last.lerp(world, k as f32 / steps as f32);
                    cells.extend(self.ground_dab(&mut doc.map.ground, p, value));
                }
            } else {
                cells.extend(doc.map.ground.paint_line((last.x, last.y), (world.x, world.y), self.ground_size * 2.0,value));
            }
            self.doc = Some(doc);
            }
        self.ground_drag = Some(GroundDrag::Brush { last: world, value, cells
        });
    }

    /// The mouse button went up: the stroke becomes one undo step.
    pub(crate) fn ground_up(&mut self, world: Vec2, erase: bool) {
        match self.ground_drag.take() {
            Some(GroundDrag::Brush { value, cells, .. }) => {
                if let Some(doc) = &mut self.doc {
                    doc.record_ground(cells, value, &self.assets);
                }
            }
            Some(GroundDrag::Line { start }) => {
                let Some(v) = self.ground_value(erase) else { return };
                let (end, r) = (self.ground_line_end(world), self.ground_size);
                if let Some(doc) = &mut self.doc {
                    let cells = doc.map.ground.paint_line((start.x, start.y), (end.x, end.y), r * 2.0, v);
                    doc.record_ground(cells, v, &self.assets);
                }
            }
            Some(GroundDrag::Rect { start }) => {
                let Some(v) = self.ground_value(erase) else {
                    return;
                };
                let end = self.ground_corner(world);
                if let Some(doc) = &mut self.doc {
                    let cells = doc.map.ground.paint_rect((start.x, start.y), (end.x, end.y), v);
                    let n = cells.len();
                    doc.record_ground(cells, v, &self.assets);
                    self.set_status(format!("Painted a rectangle of {n} cells"));
                }
            }
            None => {}
        }
    }

    /// Right-click: the material under the cursor becomes the brush's.
    pub(crate) fn pick_ground(&mut self, world: Vec2) {
        let Some(doc) = &self.doc else { return };
        let c = CELL as f32;
        let v = doc.map.ground.get((world.x / c).floor() as i32, (world.y / c).floor() as i32);
        match doc.map.material_of(v) {
            Some(id) => {
                self.ground_material = id.to_string();
                self.set_status(format!("Material: {id}"));
            }
            None => self.set_status("No painted ground here"),
        }
    }

    /// The brush outline at the cursor, or the line / rectangle being dragged (world coordinates).
    pub(crate) fn draw_ground_hover(&self, world: Vec2, px: f32, erase: bool) {
        let col = if erase { RED } else { Color::new(0.4, 1.0, 0.5, 1.0) };
        let fill = Color::new(col.r, col.g, col.b, 0.25);
        let r = self.ground_size;
        let square = |a: Vec2, b: Vec2| {
            let (lo, hi) = (a.min(b), a.max(b));
            draw_rectangle(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y, fill);
            draw_rectangle_lines(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y, 1.5 * px, col);
        };
        match (&self.ground_drag, self.ground_mode) {
            (Some(GroundDrag::Line { start }), _) => {
                let end = self.ground_line_end(world);
                draw_line(start.x, start.y, end.x, end.y, (r * 2.0).max(px), Color::new(col.r, col.g, col.b, 0.35));
                draw_circle_lines(start.x, start.y, r, 1.5 * px, col);
                draw_circle_lines(end.x, end.y, r, 1.5 * px, col);
            }
            (Some(GroundDrag::Rect { start }), _) => square(*start, self.ground_corner(world)),
            (_, GroundMode::Rect) => {
                let p = self.ground_corner(world);
                draw_circle(p.x, p.y, 3.0 * px, col);
            }
            (_, GroundMode::Fill) => {
                let c = CELL as f32;
                draw_rectangle_lines((world.x / c).floor() * c, (world.y / c).floor() * c, c, c, 2.0 * px, col);
            }
            _ if self.ground_square => {
                let (a, b) = self.ground_square_at(world);
                square(a, b);
            }
            _ => draw_circle_lines(world.x, world.y, r, 1.5 * px, col),
        }
    }

    /// For the editor's `status`: material:mode:diameter in tiles[:square][:snap] / the open map's
    /// materials.
    pub(crate) fn ground_status(&self) -> String {
        format!(
            "{}:{}:{}{}{}/{}",
            if self.ground_material.is_empty() { "-" } else { &self.ground_material },
            self.ground_mode.label().to_lowercase(),
            self.ground_size * 2.0 /TILE_SIZE as f32,
            if self.ground_square { ":square" } else { "" },
            if self.ground_snap { ":snap" } else { "" },
            self.doc.as_ref().map_or(String::new(), |d| d.map.materials.join(","))
        )
    }

    /// The Ground tool's panel: mode, brush, snapping and the materials.
    pub(crate) fn ground_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Ground");
        ui.label("Paint the map's ground with materials. Shift erases, right-click picks the material under the cursor.");
        ui.horizontal_wrapped(|ui| {
            for m in GroundMode::ALL {
                if ui.selectable_label(self.ground_mode == m, ui::t(m.icon(), m.label())).on_hover_text(m.help()).clicked() {
                    self.ground_mode = m;
                }
            }
        });
        if matches!(self.ground_mode, GroundMode::Brush | GroundMode::Line) {
        ui.horizontal(|ui| {
            ui.label("Size");
            let mut tiles = self.ground_size * 2.0 /TILE_SIZE as f32;
            if ui.add(egui::Slider::new(&mut tiles, 0.25..=8.0).step_by(0.25).suffix(" tiles")).on_hover_text("The brush's diameter (a line's width); [ and ] change it").changed() {
                self.ground_size = tiles *TILE_SIZE as f32 / 2.0;
            }
        });
        }
        if self.ground_mode == GroundMode::Brush {
            ui.horizontal(|ui| {
                ui.label("Brush");
                ui.selectable_value(&mut self.ground_square, false, ui::t(ph::CIRCLE, "Round"));
                ui.selectable_value(&mut self.ground_square, true, ui::t(ph::SQUARE, "Square"))
                    .on_hover_text("Squares of cells (or whole tiles when snapping): straight edges");
            });
        }
        if self.ground_mode != GroundMode::Fill {
            ui.checkbox(&mut self.ground_snap, "Snap to tiles")
                .on_hover_text("Rectangles, the square brush and line ends follow the 32 px tile grid; off: the 8 px cells");
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.add(egui::TextEdit::singleline(&mut self.ground_filter).hint_text("search").desired_width(150.0));
        });
        let used: Vec<String> = self.doc.as_ref().map_or(Vec::new(), |d| d.map.materials.clone());
        if self.project.db.materials.is_empty() {
            ui.weak("No materials yet: add some in Database › Materials or bring them from the asset library.");
        }
        let q = self.ground_filter.trim().to_lowercase();
        // The map's own materials first, then the others, each by name.
        let mut list: Vec<&rustpg2d_engine::ground::MaterialDef> =
            self.project.db.materials.iter().filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q) || d.id.contains(&q)).collect();
        list.sort_by_key(|d| (!used.contains(&d.id), d.name.to_lowercase()));
        let mut chosen: Option<String> = None;
        egui::ScrollArea::vertical().id_salt("ground_materials").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for d in list {
                    let on = d.id == self.ground_material;
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(72.0, 92.0), egui::Sense::click());
                    let icon = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 2.0), egui::vec2(64.0, 64.0));
                    if on || resp.hovered() {
                        ui.painter().rect_filled(rect, 4.0, if on { egui::Color32::from_rgb(70, 90, 140) } else { egui::Color32::from_gray(50) });
                    }
                    match self.tex.material_icons.get(&d.id) {
                        // 64 screen points of texture at its own size: whole repeats, no odd scaling.
                        Some(t) => {
                            ui.painter().image(t.id(), icon, crate::widgets::icon_uv(t, 64.0), egui::Color32::WHITE);
                        }
                        None => {
                            ui.painter().rect_filled(icon, 2.0, egui::Color32::from_rgb(120, 0, 120));
                        }
                    }
                    let stroke = if on { egui::Stroke::new(2.0_f32, egui::Color32::YELLOW) } else { egui::Stroke::new(1.0_f32, egui::Color32::from_gray(90)) };
                    ui.painter().rect_stroke(icon, 2.0, stroke, egui::StrokeKind::Outside);
                    let name = egui::text::LayoutJob::simple(
                        d.name.clone(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_gray(if on { 255 } else { 200 }),
                        70.0,
                    );
                    let galley = ui.painter().layout_job(name);
                    ui.painter().galley(egui::pos2(rect.center().x - galley.size().x / 2.0, icon.max.y + 3.0), galley, egui::Color32::WHITE);
                    let tip = format!("{} ({}) · {:?} edge, {}{}", d.name, d.id, d.edge, d.walk.label(), if used.contains(&d.id) { " · on this map" } else { "" });
                    if resp.on_hover_text(tip).clicked() {
                        chosen = Some(d.id.clone());
                    }
                }
            });
        });
        if let Some(c) = chosen {
            self.ground_material = c;
        }
        ui.weak(format!("This map: {} of {} materials.", used.len(), rustpg2d_engine::ground::MAX_MATERIALS));
    }
}

/// For `Ground` users outside this module (the agent tools): a stroke through several points.
pub fn paint_polyline(g: &mut Ground, points: &[(f32, f32)], width: f32, v: u8) -> Vec<(i32, i32, u8)> {
    let mut cells = Vec::new();
    match points {
        [] => {}
        [p] => cells.extend(g.paint_circle(*p, width / 2.0, v)),
        _ => {
            for w in points.windows(2) {
                cells.extend(g.paint_line(w[0], w[1], width, v));
            }
        }
    }
    cells
}
