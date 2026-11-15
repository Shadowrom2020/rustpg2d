//! The Ground tool: paints a map's ground with materials ([`rustpg2d_engine::ground`]): a round
//! brush, a line (a road or a river of a width) and a fill; Shift erases, right-click picks the
//! material under the cursor. One drag is one undo step.

use egui_macroquad::egui;
use macroquad::prelude::*;

use crate::app::Editor;
use crate::ui::{self, ph};
use rustpg2d_engine::ground::{Ground, CELL};

/// How the Ground tool paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundMode {
    /// Click or drag a round brush.
    #[default]
    Brush,
    /// Drag from one end to the other: a stroke of the brush's width.
    Line,
    /// Click: the connected area of the same material.
    Fill,
}

impl GroundMode {
    pub const ALL: [GroundMode; 3] = [GroundMode::Brush, GroundMode::Line, GroundMode::Fill];
    pub fn label(self) -> &'static str {
        match self {
            GroundMode::Brush => "Brush",
            GroundMode::Line => "Line",
            GroundMode::Fill => "Fill",
        }
    }
    pub fn parse(s: &str) -> Option<GroundMode> {
        GroundMode::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(s))
    }
}

/// A ground stroke in progress (the mouse is down).
pub enum GroundDrag {
    /// Brush: the last point painted and the cells changed so far (x, y, old value).
    Brush { last: Vec2, value: u8, cells: Vec<(i32, i32, u8)> },
    /// Line: where it started.
    Line { start: Vec2 },
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

    /// Mouse pressed on the map with the Ground tool.
    pub(crate) fn ground_down(&mut self, world: Vec2, erase: bool) {
        let Some(v) = self.ground_value(erase) else { return };
        let r = self.ground_size;
        match self.ground_mode {
            GroundMode::Brush => {
                let Some(doc) = &mut self.doc else { return };
                let cells = doc.map.ground.paint_circle((world.x, world.y), r, v);
                self.ground_drag = Some(GroundDrag::Brush { last: world, value: v, cells });
            }
            GroundMode::Line => self.ground_drag = Some(GroundDrag::Line { start: world }),
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
        let r = self.ground_size;
        if let (Some(GroundDrag::Brush { last, value, cells }), Some(doc)) = (&mut self.ground_drag, &mut self.doc) {
            if world.distance(*last) >= 1.0 {
                cells.extend(doc.map.ground.paint_line((last.x, last.y), (world.x, world.y), r * 2.0, *value));
                *last = world;
            }
        }
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
                let r = self.ground_size;
                if let Some(doc) = &mut self.doc {
                    let cells = doc.map.ground.paint_line((start.x, start.y), (world.x, world.y), r * 2.0, v);
                    doc.record_ground(cells, v, &self.assets);
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

    /// The brush outline at the cursor, or the line being dragged (world coordinates).
    pub(crate) fn draw_ground_hover(&self, world: Vec2, px: f32, erase: bool) {
        let col = if erase { RED } else { Color::new(0.4, 1.0, 0.5, 1.0) };
        let r = self.ground_size;
        match (&self.ground_drag, self.ground_mode) {
            (Some(GroundDrag::Line { start }), _) => {
                draw_line(start.x, start.y, world.x, world.y, (r * 2.0).max(px), Color::new(col.r, col.g, col.b, 0.35));
                draw_circle_lines(start.x, start.y, r, 1.5 * px, col);
                draw_circle_lines(world.x, world.y, r, 1.5 * px, col);
            }
            (_, GroundMode::Fill) => {
                let c = CELL as f32;
                draw_rectangle_lines((world.x / c).floor() * c, (world.y / c).floor() * c, c, c, 2.0 * px, col);
            }
            _ => draw_circle_lines(world.x, world.y, r, 1.5 * px, col),
        }
    }

    /// For the editor's `status`: material:mode:diameter in tiles / the open map's materials.
    pub(crate) fn ground_status(&self) -> String {
        format!(
            "{}:{}:{}/{}",
            if self.ground_material.is_empty() { "-" } else { &self.ground_material },
            self.ground_mode.label().to_lowercase(),
            self.ground_size * 2.0 / rustpg2d_engine::project::TILE_SIZE as f32,
            self.doc.as_ref().map_or(String::new(), |d| d.map.materials.join(","))
        )
    }

    /// The Ground tool's panel: mode, size and the materials.
    pub(crate) fn ground_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Ground");
        ui.label("Paint the map's ground with materials: natural edges where they meet. Shift erases, right-click picks the material under the cursor.");
        ui.horizontal(|ui| {
            for m in GroundMode::ALL {
                let help = match m {
                    GroundMode::Brush => "Click or drag a round brush",
                    GroundMode::Line => "Drag from one end to the other: a road or a stream as wide as the brush",
                    GroundMode::Fill => "Click: the connected area of the same material (or of no ground)",
                };
                if ui.selectable_label(self.ground_mode == m, m.label()).on_hover_text(help).clicked() {
                    self.ground_mode = m;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Size");
            let mut tiles = self.ground_size * 2.0 / rustpg2d_engine::project::TILE_SIZE as f32;
            if ui.add(egui::Slider::new(&mut tiles, 0.25..=8.0).step_by(0.25).suffix(" tiles")).on_hover_text("The brush's diameter (a line's width); [ and ] change it").changed() {
                self.ground_size = tiles * rustpg2d_engine::project::TILE_SIZE as f32 / 2.0;
            }
        });
        ui.separator();
        let used: Vec<String> = self.doc.as_ref().map_or(Vec::new(), |d| d.map.materials.clone());
        if self.project.db.materials.is_empty() {
            ui.weak("No materials yet: add some in Database › Materials or bring them from the asset library.");
        }
        let mut chosen: Option<String> = None;
        egui::ScrollArea::vertical().id_salt("ground_materials").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for d in &self.project.db.materials {
                    let on = d.id == self.ground_material;
                    let size = egui::vec2(52.0, 52.0);
                    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                    match self.tex.materials.get(&d.id) {
                        Some(t) => {
                            // The texture repeated twice across (how it looks on the map).
                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(52.0 / t.size()[0] as f32, 52.0 / t.size()[1] as f32));
                            ui.painter().image(t.id(), rect, uv, egui::Color32::WHITE);
                        }
                        None => {
                            ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(120, 0, 120));
                        }
                    }
                    let stroke = if on { egui::Stroke::new(3.0_f32, egui::Color32::YELLOW) } else { egui::Stroke::new(1.0_f32, egui::Color32::from_gray(90)) };
                    ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                    let tip = format!("{} ({}){}", d.name, d.id, if used.contains(&d.id) { " · on this map" } else { "" });
                    if resp.on_hover_text(tip).clicked() {
                        chosen = Some(d.id.clone());
                    }
                }
            });
        });
        if let Some(c) = chosen {
            self.ground_material = c;
        }
        if let Some(d) = self.project.db.material(&self.ground_material) {
            ui.label(ui::t(ph::PAINT_BRUSH_BROAD, &format!("{} ({})", d.name, d.id)));
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
