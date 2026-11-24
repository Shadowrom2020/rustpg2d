//! Structures in the editor: the Structures map tool (palette, ghost preview, one-click
//! placement as a single undo step), "New from map area" (capture a rectangle of the map as a
//! new structure) and the thumbnail painter shared with the Database and the asset library.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureId};
use rustpg2d_engine::defs::{Database, SpriteRef, StructureDef};
use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::project::TILE_SIZE;

use crate::app::{Editor, Tool};
use crate::ui::{self, ph};

/// Sprite an entity of a structure shows (its own model, else the definition's).
pub fn entity_sprite(db: &Database, e: &EntityInstance) -> Option<SpriteRef> {
    match e.kind {
        EntityKind::Prop => db.prop(&e.def).map(|d| d.sprite.clone()),
        EntityKind::Item => db.item(&e.def).map(|d| d.icon.clone()),
        EntityKind::Character => db.character(&e.def).map(crate::widgets::character_sprite),
    }
}

/// World size of a sprite in pixels (its sheet's frame size divided by the sheet's scale).
pub fn sprite_px(db: &Database, s: &SpriteRef) -> egui::Vec2 {
    let (cw, ch) = s.cells();
    db.sheet(&s.sheet).map_or(vec2(TILE_SIZE as f32, TILE_SIZE as f32), |d| {
        let k = d.scale.max(1) as f32;
        vec2((d.frame_w * cw) as f32 / k, (d.frame_h * ch) as f32 / k)
    })
}

/// What a structure shows, back to front: its decorations (flat ones, then sorted ones with
/// its entities by their feet, then the ones over everyone). `(y, layer, picture)`.
fn structure_pictures(d: &StructureDef, db: &Database) -> Vec<(i32, SpriteRef, egui::Vec2, bool)> {
    use rustpg2d_engine::decor::DecorLayer;
    let mut out: Vec<(u8, f32, SpriteRef, egui::Vec2, bool)> = Vec::new();
    for dec in &d.decor {
        let Some(p) = d.decor_defs.get(dec.def as usize).and_then(|id| db.prop(id)) else { continue };
        let order = match dec.layer {
            DecorLayer::Flat => 0,
            DecorLayer::Sorted => 1,
            DecorLayer::Above => 2,
        };
        out.push((order, dec.y as f32, p.state_look(dec.state_index()).0.clone(), vec2(dec.x as f32, dec.y as f32), dec.flip));
    }
    for e in &d.entities {
        if let Some(s) = entity_sprite(db, e) {
            out.push((1, e.y + 1.0, s, vec2(e.x, e.y + 1.0), false));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.into_iter().map(|(_, y, s, feet, flip)| (y as i32, s, feet, flip)).collect()
}

/// Paints a structure scaled into `rect`: its ground (`material` gives a material texture and
/// its pixel size), its decorations and entities (`sprite` paints a sprite into a rect,
/// bottom-centered, aspect kept).
pub fn paint_structure(
    ui: &egui::Ui,
    rect: Rect,
    d: &StructureDef,
    db: &Database,
    material: &dyn Fn(&str) -> Option<(TextureId, egui::Vec2)>,
    sprite: &dyn Fn(&SpriteRef, Rect) -> bool,
) {
    let ts = TILE_SIZE as f32;
    let (w, h) = (d.w.max(1) as f32 * ts, d.h.max(1) as f32 * ts);
    let scale = (rect.width() / w).min(rect.height() / h);
    let origin = rect.center() - vec2(w, h) * scale / 2.0;
    paint_ground(ui, origin, scale, d, material, |_| Color32::WHITE);
    for (_, s, feet, _) in structure_pictures(d, db) {
        let size = sprite_px(db, &s) * scale;
        let feet = origin + feet * scale;
        sprite(&s, Rect::from_min_size(feet - vec2(size.x / 2.0, size.y), size));
    }
}

/// Paints a structure's ground cells (a tile whose cells are all one material in one piece)
/// from `origin`, `scale` screen points per world pixel.
fn paint_ground(ui: &egui::Ui, origin: egui::Pos2, scale: f32, d: &StructureDef, material: &dyn Fn(&str) -> Option<(TextureId, egui::Vec2)>, tint: impl Fn(u8) -> Color32) {
    use rustpg2d_engine::ground::{CELL, CELLS};
    if d.ground.is_empty() {
        return;
    }
    let painter = ui.painter();
    let c = CELL as f32;
    let (gw, gh) = d.cells_size();
    let piece = |x: u32, y: u32, n: u32, v: u8| {
        let Some((tex, size)) = d.material_of(v).and_then(material) else { return };
        let (wx, wy) = ((x * CELL) as f32, (y * CELL) as f32);
        let uv = Rect::from_min_size(pos2((wx % size.x) / size.x, (wy % size.y) / size.y), vec2(n as f32 * c / size.x, n as f32 * c / size.y));
        painter.image(tex, Rect::from_min_size(origin + vec2(wx, wy) * scale, vec2(n as f32 * c, n as f32 * c) * scale), uv, tint(v));
    };
    for ty in 0..gh / CELLS {
        for tx in 0..gw / CELLS {
            let first = d.ground_at(tx * CELLS, ty * CELLS);
            let whole = (0..CELLS).all(|cy| (0..CELLS).all(|cx| d.ground_at(tx * CELLS + cx, ty * CELLS + cy) == first));
            if whole {
                piece(tx * CELLS, ty * CELLS, CELLS, first);
            } else {
                for cy in 0..CELLS {
                    for cx in 0..CELLS {
                        piece(tx * CELLS + cx, ty * CELLS + cy, 1, d.ground_at(tx * CELLS + cx, ty * CELLS + cy));
                    }
                }
            }
        }
    }
}

/// A material's editor texture and pixel size.
fn material_tex(tex: &crate::widgets::Textures, id: &str) -> Option<(TextureId, egui::Vec2)> {
    tex.materials.get(id).map(|t| (t.id(), vec2(t.size()[0] as f32, t.size()[1] as f32)))
}

/// Paints a project structure (materials and sprites from the editor's textures).
pub fn paint_project_structure(ui: &egui::Ui, rect: Rect, d: &StructureDef, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    let material = |id: &str| material_tex(tex, id);
    let sprite = |s: &SpriteRef, r: Rect| crate::widgets::paint_sprite(ui, tex, assets, s, r);
    paint_structure(ui, rect, d, db, &material, &sprite);
}

/// "New from map area": drag a rectangle on the map, then name the new structure.
#[derive(Default)]
pub struct CaptureState {
    /// Waiting for the user to drag a rectangle on the map.
    pub dragging: bool,
    /// The rectangle (tiles) once dragged: the naming window is open.
    pub rect: Option<(u32, u32, u32, u32)>,
    pub id: String,
    pub name: String,
    pub category: String,
    /// Include the ground layer (off: the structure keeps whatever ground the map has).
    pub ground: bool,
}

impl Editor {
    /// A project structure's thumbnail as a clickable card.
    pub fn structure_card(&self, ui: &mut egui::Ui, d: &StructureDef, size: egui::Vec2, selected: bool) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let bg = if selected { Color32::from_rgb(70, 90, 140) } else { Color32::from_gray(40) };
        ui.painter().rect_filled(rect, 3.0, bg);
        paint_project_structure(ui, rect.shrink(3.0), d, &self.project.db, &self.tex, &self.assets);
        if resp.hovered() || selected {
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { Color32::YELLOW } else { Color32::WHITE }), StrokeKind::Inside);
        }
        resp
    }

    /// Creates an empty structure (6×5) and opens it in Database → Structures (in the Asset
    /// Library window: its Edit view) to be painted.
    pub fn new_structure(&mut self) {
        let id = (1..).map(|i| format!("structure_{i}")).find(|c| self.project.db.structure(c).is_none()).unwrap();
        self.project.db.structures.push(StructureDef { id: id.clone(), name: "New structure".into(), w: 6, h: 5, ..Default::default() });
        self.defs_dirty = true;
        self.db.tab = crate::database::Tab::Structures;
        self.db.sel = self.project.db.structures.len() - 1;
        self.mode = crate::app::Mode::Database;
        self.set_status(format!("New structure '{id}': paint its ground and place decorations on the grid"));
    }

    /// Arms a structure for placing (Structures tool).
    pub fn arm_structure(&mut self, id: &str) {
        self.tool = Tool::Structure;
        self.place_structure = id.to_string();
        self.capture = CaptureState::default();
    }

    /// Places the armed structure with its top-left corner at `tile`.
    pub fn place_structure_at(&mut self, tile: (i32, i32)) {
        let Some(def) = self.project.db.structure(&self.place_structure).cloned() else { return };
        let Some(doc) = &mut self.doc else { return };
        let missing: Vec<&String> = def.materials.iter().filter(|m| self.project.db.material(m).is_none()).chain(def.decor_defs.iter().filter(|p| self.project.db.prop(p).is_none())).collect();
        let added = doc.place_structure(&def, tile.0, tile.1, &self.assets);
        if let Some(&uid) = added.first() {
            self.sel = crate::app::Selection::Entity(uid);
        }
        let mut msg = format!("Placed {} at {},{}", def.name, tile.0, tile.1);
        if !missing.is_empty() {
            msg += &format!(" (missing {} — add them from the library)", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
        self.set_status(msg);
    }

    /// Tile under the cursor where a placed structure's top-left corner goes (centered on the
    /// cursor, so the ghost follows the mouse in its middle).
    pub fn structure_origin(&self, d: &StructureDef, hover: (i32, i32)) -> (i32, i32) {
        (hover.0 - (d.w as i32 - 1) / 2, hover.1 - (d.h as i32 - 1) / 2)
    }

    /// Captures a map rectangle as a new project structure and arms it.
    pub fn create_structure(&mut self, rect: (u32, u32, u32, u32), id: &str, name: &str, category: &str, ground: bool) -> anyhow::Result<String> {
        let id = id.trim();
        anyhow::ensure!(!id.is_empty(), "the structure needs an id");
        anyhow::ensure!(self.project.db.structure(id).is_none(), "a structure '{id}' exists already");
        let doc = self.doc.as_ref().ok_or_else(|| anyhow::anyhow!("no map open"))?;
        let mut d = StructureDef::capture(&doc.map, id, if name.trim().is_empty() { id } else { name.trim() }, rect, ground);
        d.category = category.trim().to_string();
        let msg = format!("New structure '{}' ({}×{} tiles, {} entities). Save the definitions to keep it.", d.name, d.w, d.h, d.entities.len());
        self.project.db.structures.push(d);
        self.defs_dirty = true;
        self.arm_structure(id);
        Ok(msg)
    }

    /// Left panel of the Structures tool: categories with thumbnails, search, capture.
    pub fn structure_palette(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Structures");
            self.add_from_library_button(ui, Some(rustpg2d_engine::refactor::RefKind::Structure));
        });
        ui.weak("Pick one, then click the map to place it (one undo step) · right-click / Esc: stop placing.");
        ui.horizontal(|ui| {
            if ui.button(ui::t(ph::PLUS, "New…")).on_hover_text("An empty structure, painted with ground and decorations in Database › Structures").clicked() {
                self.new_structure();
            }
            let label = ui::t(ph::SELECTION_PLUS, if self.capture.dragging { "Drag an area on the map…" } else { "New from map area…" });
            if ui.selectable_label(self.capture.dragging, label).on_hover_text("Drag a rectangle on the map: its ground, decorations, collision and entities become a new structure").clicked() {
                self.capture.dragging = !self.capture.dragging;
                self.place_structure.clear();
            }
        });
        ui.horizontal(|ui| {
            ui.label(ph::MAGNIFYING_GLASS);
            ui.add(egui::TextEdit::singleline(&mut self.structure_filter).hint_text("search").desired_width(150.0));
        });
        let q = self.structure_filter.trim().to_lowercase();
        let mut cats: Vec<String> = self.project.db.structures.iter().map(|d| d.category.clone()).collect();
        cats.sort();
        cats.dedup();
        if self.project.db.structures.is_empty() {
            ui.weak("No structures in this project yet: add some from the library, or capture an area of the map.");
        }
        let mut clicked: Option<String> = None;
        for cat in cats {
            let list: Vec<&StructureDef> = self
                .project
                .db
                .structures
                .iter()
                .filter(|d| d.category == cat && (q.is_empty() || d.id.to_lowercase().contains(&q) || d.name.to_lowercase().contains(&q)))
                .collect();
            if list.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(if cat.is_empty() { "Other".to_string() } else { cat.clone() }).default_open(true).id_salt(("st_cat", &cat)).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for d in list {
                        let on = self.place_structure == d.id;
                        let r = self.structure_card(ui, d, vec2(76.0, 64.0), on).on_hover_text(format!("{}\n{}×{} tiles · id: {}", d.name, d.w, d.h, d.id));
                        if r.clicked() {
                            clicked = Some(d.id.clone());
                        }
                    }
                });
            });
        }
        if let Some(id) = clicked {
            if self.place_structure == id {
                self.place_structure.clear();
            } else {
                self.arm_structure(&id);
            }
        }
        if let Some(d) = self.project.db.structure(&self.place_structure) {
            ui.separator();
            ui.label(egui::RichText::new(&d.name).strong());
            ui.weak(format!("{}×{} tiles · {} decorations · {} entities · materials: {}", d.w, d.h, d.decor.len(), d.entities.len(), d.materials.join(", ")));
        }
    }

    /// The naming window after a capture rectangle was dragged.
    pub fn capture_window(&mut self, ctx: &egui::Context) {
        let Some(rect) = self.capture.rect else { return };
        let mut create = false;
        let mut cancel = false;
        egui::Window::new("New structure").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(format!("Area {}×{} tiles at {},{}", rect.2, rect.3, rect.0, rect.1));
            egui::Grid::new("st_new").num_columns(2).show(ui, |ui| {
                ui.label("Id");
                ui.text_edit_singleline(&mut self.capture.id);
                ui.end_row();
                ui.label("Name");
                ui.text_edit_singleline(&mut self.capture.name);
                ui.end_row();
                ui.label("Category");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.capture.category).desired_width(120.0));
                    let mut cats: Vec<String> = self.project.db.structures.iter().map(|d| d.category.clone()).filter(|c| !c.is_empty()).collect();
                    cats.sort();
                    cats.dedup();
                    egui::ComboBox::from_id_salt("st_cat_pick").selected_text("").width(24.0).show_ui(ui, |ui| {
                        for c in cats {
                            if ui.selectable_label(self.capture.category == c, &c).clicked() {
                                self.capture.category = c;
                            }
                        }
                    });
                });
                ui.end_row();
            });
            ui.checkbox(&mut self.capture.ground, "Include the ground").on_hover_text("Off for things that should stand on whatever ground the map has (trees, walls, furniture)");
            ui.weak("Decorations, collision overrides and the entities standing in the area are copied. With the ground, placing it also clears the decorations of the area first.");
            ui.horizontal(|ui| {
                create = ui.add_enabled(!self.capture.id.trim().is_empty(), egui::Button::new(ui::t(ph::CHECK, "Create"))).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if create {
            let (id, name, cat, ground) = (self.capture.id.clone(), self.capture.name.clone(), self.capture.category.clone(), self.capture.ground);
            let r = self.create_structure(rect, &id, &name, &cat, ground);
            self.show_result(r);
        } else if cancel {
            self.capture = CaptureState::default();
        }
    }

    /// Called when a capture rectangle was dragged on the map.
    pub fn capture_dragged(&mut self, rect: (u32, u32, u32, u32)) {
        let n = (1..).map(|i| format!("structure_{i}")).find(|c| self.project.db.structure(c).is_none()).unwrap();
        self.capture = CaptureState { dragging: false, rect: Some(rect), id: n.clone(), name: String::new(), category: String::new(), ground: true };
    }

    /// Canvas: semi-transparent ghost of the armed structure at its would-be position.
    pub fn draw_structure_ghost(&self, hover: (i32, i32)) {
        let Some(d) = self.project.db.structure(&self.place_structure) else { return };
        self.draw_structure_def_ghost(d, self.structure_origin(d, hover));
    }

    /// Canvas: semi-transparent ghost of a structure with its top-left corner at tile `origin`
    /// (also the clipboard while pasting).
    pub fn draw_structure_def_ghost(&self, d: &StructureDef, origin: (i32, i32)) {
        use macroquad::prelude::{draw_rectangle, draw_rectangle_lines, vec2 as mvec2, Color, YELLOW};
        use rustpg2d_engine::ground::{CELL, CELLS};
        let ts = self.ts();
        let (ox, oy) = origin;
        let tint = Color::new(1.0, 1.0, 1.0, 0.6);
        // The ground as the materials' colours (cells in runs along each row).
        let (gw, gh) = d.cells_size();
        let c = CELL as f32;
        for cy in 0..gh {
            let mut cx = 0;
            while cx < gw {
                let v = d.ground_at(cx, cy);
                let mut n = 1;
                while cx + n < gw && d.ground_at(cx + n, cy) == v {
                    n += 1;
                }
                let col = match v {
                    0 => None,
                    StructureDef::CLEAR_GROUND => Some(Color::new(0.0, 0.0, 0.0, 0.35)),
                    v => d.material_of(v).and_then(|m| self.assets.material_colors.get(m)).map(|k| Color::new(k.r, k.g, k.b, 0.6)),
                };
                if let Some(col) = col {
                    draw_rectangle(ox as f32 * ts + cx as f32 * c, oy as f32 * ts + cy as f32 * c, n as f32 * c, c, col);
                }
                cx += n;
            }
        }
        let _ = CELLS;
        for (layer, s, feet, flip) in structure_pictures(d, &self.project.db) {
            let feet = mvec2(ox as f32 * ts + feet.x, oy as f32 * ts + feet.y);
            if layer == 0 {
                self.assets.draw_sprite_flipped(&s, feet, tint, flip);
            } else {
                self.standing(feet, || self.assets.draw_sprite_flipped(&s, feet, tint, flip));
            }
        }
        let px = 1.0 / self.cam.zoom;
        draw_rectangle_lines(ox as f32 * ts, oy as f32 * ts, d.w as f32 * ts, d.h as f32 * ts, 2.0 * px, YELLOW);
    }
}

// ---------------------------------------------------------------- structure editor

/// What a click in the structure editor does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StTool {
    /// Paint the chosen material on the tile (right-click picks the material of a tile).
    Ground,
    /// Empty ground: placing keeps the map's ground there.
    Keep,
    /// Placing removes the map's ground there (a pit, the edge of a cliff).
    NoGround,
    /// Place the chosen prop as a decoration (right-click removes the decorations on a tile).
    Decor,
    /// Placing removes the map's decorations standing on the tile (trees inside a room).
    ClearDecor,
    /// Collision override (1 solid, 2 walkable, `StructureDef::COLLISION_AUTO`, 0 keep).
    Collision(u8),
    /// Place the chosen prop as an entity (right-click removes the entity on a tile).
    Prop,
}

/// State of the structure editor (Database → Structures, also in the Asset Library window).
pub struct StructEdit {
    pub tool: StTool,
    pub material: String,
    pub prop: String,
    /// The layer new decorations go on.
    pub decor_layer: rustpg2d_engine::decor::DecorLayer,
    /// Size fields (applied with "Resize"), for the structure `size_of`.
    pub size: (u32, u32),
    size_of: String,
    size_was: (u32, u32),
    last_cell: Option<(u32, u32)>,
    /// Undo / redo snapshots of the structure being edited (`hist_of`).
    undo: Vec<StructureDef>,
    redo: Vec<StructureDef>,
    hist_of: String,
    /// Grid cell size in screen points (0 = fit the available width).
    pub zoom: f32,
    pub prop_filter: String,
}

impl Default for StructEdit {
    fn default() -> Self {
        StructEdit {
            tool: StTool::Ground,
            material: String::new(),
            prop: String::new(),
            decor_layer: rustpg2d_engine::decor::DecorLayer::Sorted,
            size: (0, 0),
            size_of: String::new(),
            size_was: (0, 0),
            last_cell: None,
            undo: Vec::new(),
            redo: Vec::new(),
            hist_of: String::new(),
            zoom: 0.0,
            prop_filter: String::new(),
        }
    }
}

impl StructEdit {
    fn snapshot(&mut self, d: &StructureDef) {
        self.undo.push(d.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo_step(&mut self, d: &mut StructureDef) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(d, prev));
        }
    }

    pub fn redo_step(&mut self, d: &mut StructureDef) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(d, next));
        }
    }

    /// Applies the current tool at a tile (one tile of a stroke).
    pub fn apply(&mut self, d: &mut StructureDef, cell: (u32, u32), db: &Database) {
        use rustpg2d_engine::ground::CELLS;
        let (cx, cy) = cell;
        let ground = |d: &mut StructureDef, v: u8| {
            for y in 0..CELLS {
                for x in 0..CELLS {
                    d.set_ground(cx * CELLS + x, cy * CELLS + y, v);
                }
            }
        };
        let ts = TILE_SIZE as f32;
        match self.tool {
            StTool::Ground => {
                if self.material.is_empty() {
                    return;
                }
                let v = d.material_value(&self.material);
                ground(d, v);
            }
            StTool::Keep => ground(d, 0),
            StTool::NoGround => ground(d, StructureDef::CLEAR_GROUND),
            StTool::Decor => {
                let Some(p) = db.prop(&self.prop) else { return };
                // Wall pieces snap to their grid; anything else stands in the tile's middle.
                let snap = p.snap.max(1) as i32;
                let (x, y) = ((cx as i32 * TILE_SIZE as i32 + TILE_SIZE as i32 / 2) / snap * snap, (cy as i32 + 1) * TILE_SIZE as i32);
                let x = if p.snap > 0 { x + snap / 2 } else { x };
                let def = d.decor_slot(&p.id);
                d.decor.push(rustpg2d_engine::decor::Decor { def, x, y, flip: false, state: 0, layer: self.decor_layer });
            }
            StTool::ClearDecor => {
                d.clear.resize((d.w * d.h) as usize, 0);
                let i = (cy * d.w + cx) as usize;
                d.clear[i] = 1 - d.clear[i].min(1);
            }
            StTool::Collision(c) => d.set_collision(cx, cy, c),
            StTool::Prop => {
                if self.prop.is_empty() {
                    return;
                }
                // Like placing on the map: even-width props stand on a tile edge.
                let tiles_wide = db.prop(&self.prop).map_or(1, |p| (sprite_px(db, &p.sprite).x / ts).round().max(1.0) as u32);
                let p = rustpg2d_engine::grid::snap_feet(macroquad::math::vec2(cx as f32 * ts, (cy as f32 + 0.5) * ts), tiles_wide, ts);
                d.entities.push(EntityInstance::new(0, EntityKind::Prop, &self.prop, p.x, p.y));
            }
        }
        d.prune();
    }
}

const ACCENT: Color32 = Color32::from_rgb(70, 110, 170);

/// Small caps section title in the editor's side columns.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).small().strong().color(Color32::from_gray(150)));
    ui.add_space(2.0);
}

impl StTool {
    fn icon(self) -> &'static str {
        match self {
            StTool::Ground => ph::PAINT_BRUSH_BROAD,
            StTool::Keep => ph::ERASER,
            StTool::NoGround => ph::X_CIRCLE,
            StTool::Decor => ph::TREE,
            StTool::ClearDecor => ph::BROOM,
            StTool::Collision(1) => ph::WALL,
            StTool::Collision(2) => ph::FOOTPRINTS,
            StTool::Collision(0) => ph::MINUS,
            StTool::Collision(_) => ph::GRID_FOUR,
            StTool::Prop => ph::TREASURE_CHEST,
        }
    }
    fn label(self) -> &'static str {
        match self {
            StTool::Ground => "Paint ground",
            StTool::Keep => "Erase ground",
            StTool::NoGround => "Remove map ground",
            StTool::Decor => "Place decoration",
            StTool::ClearDecor => "Clear map decorations",
            StTool::Collision(1) => "Solid",
            StTool::Collision(2) => "Walkable",
            StTool::Collision(0) => "Keep map collision",
            StTool::Collision(_) => "Automatic",
            StTool::Prop => "Place prop",
        }
    }
    /// Picks a prop (decorations and props) rather than a material.
    fn picks_props(self) -> bool {
        matches!(self, StTool::Decor | StTool::Prop)
    }
}

/// The structure editor: tools on the left, the grid in the middle, the materials (or the
/// props) to pick from on the right. Edits `d` in place.
pub fn structure_editor(ui: &mut egui::Ui, d: &mut StructureDef, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    if st.hist_of != d.id {
        st.undo.clear();
        st.redo.clear();
        st.hist_of = d.id.clone();
    }
    // Size fields follow the structure (switching, undo, resizing elsewhere).
    if st.size_of != d.id || st.size_was != (d.w, d.h) {
        st.size = (d.w, d.h);
        st.size_was = (d.w, d.h);
        st.size_of = d.id.clone();
    }
    if st.material.is_empty() || db.material(&st.material).is_none() {
        st.material = d.materials.iter().find(|m| db.material(m).is_some()).cloned().or_else(|| db.materials.first().map(|m| m.id.clone())).unwrap_or_default();
    }
    if st.prop.is_empty() {
        st.prop = db.props.first().map(|p| p.id.clone()).unwrap_or_default();
    }
    // Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z) while no text field has the focus.
    // (The focus check must stay outside `ui.input`: both take the egui context lock.)
    let typing = ui.ctx().wants_keyboard_input();
    let (undo, redo) = ui.input(|i| {
        if typing || !i.modifiers.command || i.modifiers.alt {
            return (false, false);
        }
        let z = i.key_pressed(egui::Key::Z);
        (z && !i.modifiers.shift, i.key_pressed(egui::Key::Y) || (z && i.modifiers.shift))
    });
    if undo {
        st.undo_step(d);
    } else if redo {
        st.redo_step(d);
    }
    let total = ui.available_width();
    let (side, right) = (190.0, 290.0);
    let middle = (total - side - right - 40.0).max(260.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(side);
            sidebar(ui, d, st);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_width(middle);
            canvas_column(ui, d, st, db, tex, assets);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_width(right);
            match st.tool {
                t if t.picks_props() => prop_picker(ui, st, db, tex, assets),
                StTool::Collision(_) => {
                    section(ui, "COLLISION");
                    ui.label("Click or drag over tiles to set how they block movement once the structure is placed:");
                    for (c, col, text) in [
                        (1, Color32::from_rgb(220, 60, 60), "Solid: always blocks"),
                        (2, Color32::from_rgb(60, 200, 90), "Walkable: never blocks (doors, gaps)"),
                        (StructureDef::COLLISION_AUTO, Color32::from_gray(160), "Automatic: the ground and decorations decide"),
                        (0, Color32::from_gray(60), "Keep: the map's own setting stays"),
                    ] {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(r, 2.0, col);
                            if ui.selectable_label(st.tool == StTool::Collision(c), text).clicked() {
                                st.tool = StTool::Collision(c);
                            }
                        });
                    }
                }
                StTool::ClearDecor => {
                    section(ui, "CLEAR DECORATIONS");
                    ui.label("Click tiles: when the structure is placed, the map's decorations standing there are removed first (trees inside a room). Click again to undo the mark.");
                }
                _ => material_picker(ui, st, db, tex),
            }
        });
    });
}

/// Left column: tools, size and history.
fn sidebar(ui: &mut egui::Ui, d: &mut StructureDef, st: &mut StructEdit) {
    let w = ui.available_width();
    let tool = |ui: &mut egui::Ui, st: &mut StructEdit, t: StTool, tip: &str| {
        let r = ui.add_sized([w, 24.0], egui::SelectableLabel::new(st.tool == t, ui::t(t.icon(), t.label()))).on_hover_text(tip);
        if r.clicked() {
            st.tool = t;
        }
    };
    section(ui, "GROUND");
    tool(ui, st, StTool::Ground, "Paint the material chosen on the right on whole tiles (right-click a tile picks its material)");
    tool(ui, st, StTool::Keep, "No ground of its own: placing the structure leaves the map's ground there");
    tool(ui, st, StTool::NoGround, "Placing the structure removes the map's ground there (black cross)");
    section(ui, "DECORATIONS");
    tool(ui, st, StTool::Decor, "Place the prop chosen on the right as a decoration (a picture: trees, walls, furniture); right-click removes the decorations on a tile");
    tool(ui, st, StTool::ClearDecor, "Placing the structure removes the map's decorations on these tiles (red cross)");
    section(ui, "COLLISION");
    tool(ui, st, StTool::Collision(1), "Always blocks");
    tool(ui, st, StTool::Collision(2), "Never blocks (doors)");
    tool(ui, st, StTool::Collision(StructureDef::COLLISION_AUTO), "Blocks where the ground or a decoration is solid");
    tool(ui, st, StTool::Collision(0), "Leave the map's collision alone");
    section(ui, "OBJECTS");
    tool(ui, st, StTool::Prop, "Put props (doors, chests, signs…) into the structure as entities; right-click removes one");

    section(ui, "SIZE");
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut st.size.0).range(1..=64));
        ui.label("×");
        ui.add(egui::DragValue::new(&mut st.size.1).range(1..=64));
        ui.label("tiles");
    });
    if ui.add_enabled(st.size != (d.w, d.h), egui::Button::new("Resize")).on_hover_text("Adds or removes columns / rows at the right / bottom").clicked() {
        st.snapshot(d);
        d.resize(st.size.0, st.size.1);
        d.prune();
    }
    section(ui, "HISTORY");
    ui.horizontal(|ui| {
        if ui::icon_button(ui, !st.undo.is_empty(), ph::ARROW_ARC_LEFT, "Undo", "Ctrl+Z", "").clicked() {
            st.undo_step(d);
        }
        if ui::icon_button(ui, !st.redo.is_empty(), ph::ARROW_ARC_RIGHT, "Redo", "Ctrl+Y", "").clicked() {
            st.redo_step(d);
        }
    });
}

/// Middle column: what a click does, zoom, the grid, the entities.
fn canvas_column(ui: &mut egui::Ui, d: &mut StructureDef, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    ui.horizontal(|ui| {
        egui::Frame::new().fill(Color32::from_gray(70)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
            ui.label(egui::RichText::new(ui::t(st.tool.icon(), st.tool.label())).color(Color32::WHITE));
        });
        if st.tool == StTool::Decor {
            use rustpg2d_engine::decor::DecorLayer;
            egui::ComboBox::from_id_salt("st_decor_layer")
                .selected_text(match st.decor_layer {
                    DecorLayer::Flat => "flat",
                    DecorLayer::Sorted => "sorted",
                    DecorLayer::Above => "above",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.decor_layer, DecorLayer::Flat, "Flat: under everyone");
                    ui.selectable_value(&mut st.decor_layer, DecorLayer::Sorted, "Sorted: in front of or behind characters");
                    ui.selectable_value(&mut st.decor_layer, DecorLayer::Above, "Above: over everyone");
                })
                .response
                .on_hover_text("Where new decorations are drawn");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.selectable_label(st.zoom == 0.0, "Fit").clicked() {
                st.zoom = 0.0;
            }
            let mut z = if st.zoom == 0.0 { 32.0 } else { st.zoom };
            if ui.add(egui::Slider::new(&mut z, 12.0..=64.0).show_value(false)).on_hover_text("Zoom").changed() {
                st.zoom = z;
            }
        });
    });
    ui.add_space(4.0);
    egui::ScrollArea::both().id_salt("st_grid").max_height(560.0).auto_shrink([false, true]).show(ui, |ui| {
        structure_canvas(ui, d, st, db, tex, assets);
    });
    ui.horizontal_wrapped(|ui| {
        let legend = |ui: &mut egui::Ui, col: Color32, text: &str| {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, col);
            ui.label(egui::RichText::new(text).small().color(Color32::from_gray(160)));
        };
        legend(ui, Color32::BLACK, "× removes the map's ground");
        legend(ui, Color32::from_rgb(230, 70, 70), "× clears the map's decorations");
        if matches!(st.tool, StTool::Collision(_)) {
            legend(ui, Color32::from_rgb(220, 60, 60), "solid");
            legend(ui, Color32::from_rgb(60, 200, 90), "walkable");
            legend(ui, Color32::from_gray(160), "automatic");
        }
    });
    ui.weak(format!("{} decorations · {} props", d.decor.len(), d.entities.len()));
    if !d.entities.is_empty() {
        section(ui, "PROPS IN THIS STRUCTURE");
        let ts = TILE_SIZE as f32;
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            for (i, e) in d.entities.iter().enumerate() {
                egui::Frame::new().fill(Color32::from_gray(40)).corner_radius(4.0).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(s) = entity_sprite(db, e) {
                            let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                            crate::widgets::paint_sprite(ui, tex, assets, &s, r);
                        }
                        ui.label(format!("{} ({}, {})", e.def, (e.x / ts).floor(), ((e.y - 1.0) / ts).floor()));
                        if ui.small_button(ph::X).on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
        });
        if let Some(i) = remove {
            st.snapshot(d);
            d.entities.remove(i);
        }
    }
}

/// Right column (decorations, props): searchable gallery of the props.
fn prop_picker(ui: &mut egui::Ui, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    section(ui, "PROPS");
    ui.horizontal(|ui| {
        ui.label(ph::MAGNIFYING_GLASS);
        ui.add(egui::TextEdit::singleline(&mut st.prop_filter).hint_text("search").desired_width(200.0));
    });
    let q = st.prop_filter.trim().to_lowercase();
    egui::ScrollArea::vertical().id_salt("st_props").max_height(460.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for p in db.props.iter().filter(|p| q.is_empty() || p.id.contains(&q) || p.name.to_lowercase().contains(&q)) {
                let on = st.prop == p.id;
                let (r, resp) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::click());
                ui.painter().rect_filled(r, 4.0, if on { ACCENT } else if resp.hovered() { Color32::from_gray(55) } else { Color32::from_gray(40) });
                crate::widgets::paint_sprite(ui, tex, assets, &p.sprite, r.shrink(6.0));
                if resp.on_hover_text(format!("{} ({})", p.name, p.id)).clicked() {
                    st.prop = p.id.clone();
                }
            }
        });
    });
    if let Some(p) = db.prop(&st.prop) {
        ui.label(egui::RichText::new(format!("{}  ({})", p.name, p.id)).strong());
    }
    ui.weak("Click a tile to place it; right-click a tile removes what stands there.");
}

/// Right column (ground tools): the materials.
fn material_picker(ui: &mut egui::Ui, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures) {
    section(ui, "MATERIALS");
    if db.materials.is_empty() {
        ui.weak("No materials yet: add some in Database › Materials or from the asset library.");
    }
    egui::ScrollArea::vertical().id_salt("st_materials").max_height(460.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for m in &db.materials {
                let on = st.material == m.id;
                let (r, resp) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::click());
                if let Some(t) = tex.material_icons.get(&m.id) {
                    ui.painter().image(t.id(), r, crate::widgets::icon_uv(t, 64.0), Color32::WHITE);
                }
                let stroke = if on { Stroke::new(3.0_f32, Color32::YELLOW) } else { Stroke::new(1.0_f32, Color32::from_gray(90)) };
                ui.painter().rect_stroke(r, 2.0, stroke, StrokeKind::Inside);
                if resp.on_hover_text(format!("{} ({})", m.name, m.id)).clicked() {
                    st.material = m.id.clone();
                    if st.tool != StTool::Ground {
                        st.tool = StTool::Ground;
                    }
                }
            }
        });
    });
    if let Some(m) = db.material(&st.material) {
        ui.label(egui::RichText::new(format!("{}  ({})", m.name, m.id)).strong());
    }
    ui.weak("Click or drag over tiles to paint them; finer shapes: paint the ground on a map and capture it (Structures tool › New from map area).");
}

/// The structure's grid: ground, decorations and props, the clear marks, collision; paints
/// with the current tool.
fn structure_canvas(ui: &mut egui::Ui, d: &mut StructureDef, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    use rustpg2d_engine::ground::CELLS;
    let ts = TILE_SIZE as f32;
    let fit = ((ui.available_width() - 8.0) / d.w.max(1) as f32).clamp(12.0, 48.0).floor();
    let cell = if st.zoom > 0.0 { st.zoom } else { fit };
    let size = vec2(d.w as f32, d.h as f32) * cell;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect.expand(2.0));
    painter.rect_filled(rect.expand(6.0), 6.0, Color32::from_gray(22));
    // Checkerboard: tiles without ground of their own.
    for y in 0..d.h {
        for x in 0..d.w {
            let c = if (x + y) % 2 == 0 { 46 } else { 38 };
            painter.rect_filled(Rect::from_min_size(rect.min + vec2(x as f32, y as f32) * cell, vec2(cell, cell)), 0.0, Color32::from_gray(c));
        }
    }
    let scale = cell / ts;
    let fade = matches!(st.tool, StTool::Collision(_) | StTool::ClearDecor);
    paint_ground(ui, rect.min, scale, d, &|id| material_tex(tex, id), |_| if fade { Color32::from_white_alpha(140) } else { Color32::WHITE });
    let tile_rect = |i: usize| Rect::from_min_size(rect.min + vec2((i as u32 % d.w) as f32, (i as u32 / d.w) as f32) * cell, vec2(cell, cell));
    let cross = |r: Rect, col: Color32| {
        let r = r.shrink(cell * 0.25);
        let s = Stroke::new(2.0_f32, col);
        painter.line_segment([r.left_top(), r.right_bottom()], s);
        painter.line_segment([r.right_top(), r.left_bottom()], s);
    };
    // Tiles whose ground the structure removes.
    for i in 0..(d.w * d.h) as usize {
        let (tx, ty) = (i as u32 % d.w, i as u32 / d.w);
        if d.ground_at(tx * CELLS, ty * CELLS) == StructureDef::CLEAR_GROUND {
            cross(tile_rect(i), Color32::BLACK);
        }
    }
    for (_, s, feet, _) in structure_pictures(d, db) {
        let sz = sprite_px(db, &s) * scale;
        let feet = rect.min + feet * scale;
        crate::widgets::paint_sprite(ui, tex, assets, &s, Rect::from_min_size(feet - vec2(sz.x / 2.0, sz.y), sz));
    }
    // (Faint unless the tool that sets them is on: they cover the whole picture of a building.)
    let clear_col = if st.tool == StTool::ClearDecor { Color32::from_rgb(230, 70, 70) } else { Color32::from_rgba_unmultiplied(230, 70, 70, 50) };
    for (i, &c) in d.clear.iter().enumerate() {
        if c == 1 {
            cross(tile_rect(i), clear_col);
        }
    }
    if matches!(st.tool, StTool::Collision(_)) {
        for (i, &c) in d.collision.iter().enumerate() {
            let col = match c {
                1 => Color32::from_rgba_unmultiplied(220, 50, 50, 110),
                2 => Color32::from_rgba_unmultiplied(50, 200, 80, 110),
                0 => continue,
                _ => Color32::from_rgba_unmultiplied(160, 160, 160, 60),
            };
            painter.rect_filled(tile_rect(i), 0.0, col);
        }
    }
    for x in 0..=d.w {
        painter.vline(rect.min.x + x as f32 * cell, rect.y_range(), Stroke::new(1.0_f32, Color32::from_white_alpha(22)));
    }
    for y in 0..=d.h {
        painter.hline(rect.x_range(), rect.min.y + y as f32 * cell, Stroke::new(1.0_f32, Color32::from_white_alpha(22)));
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::from_gray(110)), StrokeKind::Outside);

    let cell_at = |p: egui::Pos2| -> Option<(u32, u32)> {
        let v = (p - rect.min) / cell;
        (v.x >= 0.0 && v.y >= 0.0 && v.x < d.w as f32 && v.y < d.h as f32).then_some((v.x as u32, v.y as u32))
    };
    if let Some(c) = resp.hover_pos().and_then(cell_at) {
        let r = Rect::from_min_size(rect.min + vec2(c.0 as f32, c.1 as f32) * cell, vec2(cell, cell));
        painter.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
        let ground = d.material_of(d.ground_at(c.0 * CELLS, c.1 * CELLS)).map_or(String::new(), |m| format!(" · {m}"));
        resp.clone().on_hover_text(format!("tile {}, {}{ground}", c.0, c.1));
    }

    // Painting: one undo snapshot per stroke; a stroke paints each tile once.
    let primary = ui.input(|i| i.pointer.primary_down());
    if let Some(c) = resp.interact_pointer_pos().and_then(cell_at) {
        // (A tile's decorations and entities: their feet lie in it.)
        let on_tile = |x: f32, y: f32| ((x / ts).floor() as u32, ((y - 1.0) / ts).floor() as u32) == c;
        if resp.secondary_clicked() {
            match st.tool {
                StTool::Prop => {
                    if let Some(i) = d.entities.iter().position(|e| on_tile(e.x, e.y)) {
                        st.snapshot(d);
                        d.entities.remove(i);
                    }
                }
                StTool::Decor => {
                    if d.decor.iter().any(|e| on_tile(e.x as f32, e.y as f32)) {
                        st.snapshot(d);
                        d.decor.retain(|e| !on_tile(e.x as f32, e.y as f32));
                        d.prune();
                    }
                }
                _ => {
                    // Eyedropper: the material of this tile.
                    if let Some(m) = d.material_of(d.ground_at(c.0 * CELLS + CELLS / 2, c.1 * CELLS + CELLS / 2)) {
                        st.material = m.to_string();
                        st.tool = StTool::Ground;
                    }
                }
            }
        } else if resp.clicked() && st.last_cell.is_none() {
            // A click so quick that the button was never seen held down on the grid.
            st.snapshot(d);
            st.apply(d, c, db);
        } else if primary && (resp.drag_started() || resp.is_pointer_button_down_on()) && st.last_cell != Some(c) {
            if st.last_cell.is_none() {
                st.snapshot(d);
            }
            // (Objects and clear marks: one per click, not along a drag.)
            if !matches!(st.tool, StTool::Prop | StTool::Decor | StTool::ClearDecor) || st.last_cell.is_none() {
                st.apply(d, c, db);
            }
            st.last_cell = Some(c);
        }
    }
    if !primary {
        st.last_cell = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::defs::SheetDef;

    /// A 2x sheet's 64 x 96 frames take the place of a 1x sheet's 32 x 48 ones.
    #[test]
    fn sprite_size_is_in_world_pixels() {
        let db = Database { sheets: vec![SheetDef::new("lo", "sprites/lo.png", 32, 48), SheetDef { scale: 2, ..SheetDef::new("hi", "sprites/hi.png", 64, 96) }], ..Default::default() };
        assert_eq!(sprite_px(&db, &SpriteRef::new("lo", 0)), sprite_px(&db, &SpriteRef::new("hi", 0)));
        assert_eq!(sprite_px(&db, &SpriteRef::new("hi", 0)), vec2(32.0, 48.0));
    }
}
