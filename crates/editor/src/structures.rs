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

/// Size of a sprite in pixels (from its sheet's frame size).
pub fn sprite_px(db: &Database, s: &SpriteRef) -> egui::Vec2 {
    let (cw, ch) = s.cells();
    db.sheet(&s.sheet).map_or(vec2(TILE_SIZE as f32, TILE_SIZE as f32), |d| vec2((d.frame_w * cw) as f32, (d.frame_h * ch) as f32))
}

/// Paints a structure scaled into `rect` (tiles of all layers, its entities above the detail
/// layer). `tileset` gives the texture and pixel size of a tileset image; `sprite` paints a
/// sprite into a rect (bottom-centered, aspect kept).
pub fn paint_structure(
    ui: &egui::Ui,
    rect: Rect,
    d: &StructureDef,
    db: &Database,
    tileset: &dyn Fn(&str) -> Option<(TextureId, egui::Vec2)>,
    sprite: &dyn Fn(&SpriteRef, Rect) -> bool,
) {
    let ts = TILE_SIZE as f32;
    let (w, h) = (d.w.max(1) as f32 * ts, d.h.max(1) as f32 * ts);
    let scale = (rect.width() / w).min(rect.height() / h);
    let origin = rect.center() - vec2(w, h) * scale / 2.0;
    let cell = ts * scale;
    let painter = ui.painter_at(rect);
    let tiles = |layer: &Vec<u32>| {
        for (i, &v) in layer.iter().enumerate() {
            let Some((ts_id, index)) = d.tile_source(v) else { continue };
            let Some((tex, size)) = tileset(ts_id) else { continue };
            let cols = (size.x / ts).floor().max(1.0) as u32;
            let (sx, sy) = ((index % cols) as f32 * ts, (index / cols) as f32 * ts);
            let uv = Rect::from_min_size(pos2(sx / size.x, sy / size.y), vec2(ts / size.x, ts / size.y));
            let (x, y) = ((i as u32 % d.w) as f32, (i as u32 / d.w) as f32);
            painter.image(tex, Rect::from_min_size(origin + vec2(x, y) * cell, vec2(cell, cell)), uv, Color32::WHITE);
        }
    };
    for layer in d.layers.iter().take(2) {
        tiles(layer);
    }
    let mut ents: Vec<&EntityInstance> = d.entities.iter().collect();
    ents.sort_by(|a, b| a.y.total_cmp(&b.y));
    for e in ents {
        let Some(s) = entity_sprite(db, e) else { continue };
        let size = sprite_px(db, &s) * scale;
        let feet = origin + vec2(e.x, e.y + 1.0) * scale;
        sprite(&s, Rect::from_min_size(feet - vec2(size.x / 2.0, size.y), size));
    }
    for layer in d.layers.iter().skip(2) {
        tiles(layer);
    }
}

/// Paints a project structure (tilesets and sprites from the editor's textures).
pub fn paint_project_structure(ui: &egui::Ui, rect: Rect, d: &StructureDef, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    let tileset = |id: &str| {
        let t = tex.tilesets.get(id)?;
        let a = assets.tilesets.get(id)?;
        Some((t.id(), vec2(a.image.width as f32, a.image.height as f32)))
    };
    let sprite = |s: &SpriteRef, r: Rect| crate::widgets::paint_sprite(ui, tex, assets, s, r);
    paint_structure(ui, rect, d, db, &tileset, &sprite);
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
        self.set_status(format!("New structure '{id}': pick tiles from a tileset on the left and paint them on the grid"));
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
        let missing: Vec<&String> = def.tilesets.iter().filter(|t| self.project.db.tileset(t).is_none()).collect();
        let added = doc.place_structure(&def, tile.0, tile.1, &self.assets);
        if let Some(&uid) = added.first() {
            self.sel = crate::app::Selection::Entity(uid);
        }
        let mut msg = format!("Placed {} at {},{}", def.name, tile.0, tile.1);
        if !missing.is_empty() {
            msg += &format!(" (missing tileset {} — add it from the library)", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
        if def.layers.len() > doc.map.layers.len() {
            msg += &format!(" (the map has only {} layers; the rest was skipped)", doc.map.layers.len());
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
            if ui.button(ui::t(ph::PLUS, "New…")).on_hover_text("An empty structure, painted from tileset stamps in Database › Structures").clicked() {
                self.new_structure();
            }
            let label = ui::t(ph::SELECTION_PLUS, if self.capture.dragging { "Drag an area on the map…" } else { "New from map area…" });
            if ui.selectable_label(self.capture.dragging, label).on_hover_text("Drag a rectangle on the map: its tiles (all layers), collision and entities become a new structure").clicked() {
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
            ui.weak(format!("{}×{} tiles · {} entities · tilesets: {}", d.w, d.h, d.entities.len(), d.tilesets.join(", ")));
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
            ui.checkbox(&mut self.capture.ground, "Include the ground layer").on_hover_text("Off for things that should stand on whatever ground the map has (trees, walls, furniture)");
            ui.weak("Tiles of all layers, collision overrides and the entities standing in the area are copied. Inside the structure, empty upper layers clear what the map has there when placed.");
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
        use macroquad::prelude::{draw_rectangle_lines, draw_texture_ex, vec2 as mvec2, Color, DrawTextureParams, YELLOW};
        let ts = self.ts();
        let (ox, oy) = origin;
        let tint = Color::new(1.0, 1.0, 1.0, 0.6);
        let draw_layer = |layer: &Vec<u32>| {
            for (i, &v) in layer.iter().enumerate() {
                let Some((id, index)) = d.tile_source(v) else { continue };
                let Some(t) = self.assets.tilesets.get(id) else { continue };
                let (x, y) = ((ox + (i as u32 % d.w) as i32) as f32 * ts, (oy + (i as u32 / d.w) as i32) as f32 * ts);
                draw_texture_ex(&t.texture, x, y, tint, DrawTextureParams { source: Some(t.src_rect(index, ts)), ..Default::default() });
            }
        };
        for l in d.layers.iter().take(2) {
            draw_layer(l);
        }
        for e in &d.entities {
            let feet = mvec2(ox as f32 * ts + e.x, oy as f32 * ts + e.y);
            if e.kind == EntityKind::Character {
                self.draw_char_layers(&self.char_layers(&e.def, Some(e), e.facing, 0), feet, tint);
            } else if let Some(s) = entity_sprite(&self.project.db, e) {
                self.assets.draw_sprite(&s, 0, feet, tint);
            }
        }
        for l in d.layers.iter().skip(2) {
            draw_layer(l);
        }
        let px = 1.0 / self.cam.zoom;
        draw_rectangle_lines(ox as f32 * ts, oy as f32 * ts, d.w as f32 * ts, d.h as f32 * ts, 2.0 * px, YELLOW);
    }
}

// ---------------------------------------------------------------- structure editor

/// What a click in the structure editor does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StTool {
    /// Paint the selected tileset stamp on the current layer (right-click picks a tile).
    Stamp,
    /// Empty cell: placing keeps whatever the map has there.
    Keep,
    /// Placing empties the map's tile on this layer (e.g. trees inside a room).
    Clear,
    /// Collision override (1 solid, 2 walkable, `StructureDef::COLLISION_AUTO`, 0 keep).
    Collision(u8),
    /// Place the chosen prop (right-click removes the entity on a cell).
    Prop,
}

/// State of the structure editor (Database → Structures, also in the Asset Library window).
pub struct StructEdit {
    pub layer: usize,
    pub tool: StTool,
    pub tileset: String,
    /// Selected stamp in the tileset: (x, y, w, h) in tiles.
    pub stamp: (u32, u32, u32, u32),
    pub prop: String,
    /// Size fields (applied with "Resize"), for the structure `size_of`.
    pub size: (u32, u32),
    size_of: String,
    size_was: (u32, u32),
    pal_start: Option<(u32, u32)>,
    last_cell: Option<(u32, u32)>,
    /// Undo / redo snapshots of the structure being edited (`hist_of`).
    undo: Vec<StructureDef>,
    redo: Vec<StructureDef>,
    hist_of: String,
    /// Layers hidden in the editor grid (by index: ground, detail, overhead).
    pub hidden: [bool; 3],
    /// Draw the layers you are not painting on faded.
    pub dim: bool,
    /// Grid cell size in screen points (0 = fit the available width).
    pub zoom: f32,
    pub prop_filter: String,
}

impl Default for StructEdit {
    fn default() -> Self {
        StructEdit {
            layer: 1,
            tool: StTool::Stamp,
            tileset: String::new(),
            stamp: (0, 0, 1, 1),
            prop: String::new(),
            size: (0, 0),
            size_of: String::new(),
            size_was: (0, 0),
            pal_start: None,
            last_cell: None,
            undo: Vec::new(),
            redo: Vec::new(),
            hist_of: String::new(),
            hidden: [false; 3],
            dim: true,
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

    /// Applies the current tool at a cell (one cell of a stroke).
    pub fn apply(&mut self, d: &mut StructureDef, cell: (u32, u32), db: &Database, assets: &rustpg2d_engine::render::Assets) {
        let (cx, cy) = cell;
        match self.tool {
            StTool::Stamp => {
                let Some(t) = assets.tilesets.get(&self.tileset) else { return };
                let slot = d.tileset_slot(&self.tileset);
                let (sx, sy, sw, sh) = self.stamp;
                for dy in 0..sh {
                    for dx in 0..sw {
                        let index = (sy + dy) * t.columns.max(1) + sx + dx;
                        d.set_tile(self.layer, cx + dx, cy + dy, rustpg2d_engine::map::Tile::new(slot, index).0);
                    }
                }
            }
            StTool::Keep => d.set_tile(self.layer, cx, cy, 0),
            StTool::Clear => d.set_tile(self.layer, cx, cy, StructureDef::CLEAR),
            StTool::Collision(c) => d.set_collision(cx, cy, c),
            StTool::Prop => {
                if self.prop.is_empty() {
                    return;
                }
                let ts = TILE_SIZE as f32;
                // Like placing on the map: even-width props stand on a tile edge.
                let tiles_wide = db.prop(&self.prop).map_or(1, |p| (sprite_px(db, &p.sprite).x / ts).round().max(1.0) as u32);
                let p = rustpg2d_engine::grid::snap_feet(macroquad::math::vec2(cx as f32 * ts, (cy as f32 + 0.5) * ts), tiles_wide, ts);
                d.entities.push(EntityInstance::new(0, EntityKind::Prop, &self.prop, p.x, p.y));
            }
        }
        d.prune();
    }
}

/// The map layers a structure paints on, top to bottom as they are drawn in the game.
const LAYERS: [(usize, &str, &str); 3] = [
    (2, "Overhead", "Drawn above characters: roofs, treetops, arches"),
    (1, "Detail", "Walls, doors, furniture, trees, fences"),
    (0, "Ground", "Floors, grass, paths, water"),
];

fn layer_name(i: usize) -> &'static str {
    LAYERS.iter().find(|l| l.0 == i).map_or("layer", |l| l.1)
}

const ACCENT: Color32 = Color32::from_rgb(70, 110, 170);

/// Small caps section title in the editor's side columns.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).small().strong().color(Color32::from_gray(150)));
    ui.add_space(2.0);
}

impl StTool {
    fn uses_layers(self) -> bool {
        matches!(self, StTool::Stamp | StTool::Keep | StTool::Clear)
    }
    fn icon(self) -> &'static str {
        match self {
            StTool::Stamp => ph::PAINT_BRUSH,
            StTool::Keep => ph::ERASER,
            StTool::Clear => ph::X_CIRCLE,
            StTool::Collision(1) => ph::WALL,
            StTool::Collision(2) => ph::FOOTPRINTS,
            StTool::Collision(0) => ph::MINUS,
            StTool::Collision(_) => ph::GRID_FOUR,
            StTool::Prop => ph::TREASURE_CHEST,
        }
    }
    fn label(self) -> &'static str {
        match self {
            StTool::Stamp => "Paint tiles",
            StTool::Keep => "Erase",
            StTool::Clear => "Clear map tile",
            StTool::Collision(1) => "Solid",
            StTool::Collision(2) => "Walkable",
            StTool::Collision(0) => "Keep map collision",
            StTool::Collision(_) => "Tileset default",
            StTool::Prop => "Place prop",
        }
    }
}

/// The structure editor: tools and layers on the left, the grid in the middle, the tileset (or
/// the props) to pick from on the right. Edits `d` in place.
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
    if st.tileset.is_empty() || !assets.tilesets.contains_key(&st.tileset) {
        st.tileset = d.tilesets.iter().find(|t| assets.tilesets.contains_key(*t)).cloned().or_else(|| db.tilesets.first().map(|t| t.id.clone())).unwrap_or_default();
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
                StTool::Prop => prop_picker(ui, st, db, tex, assets),
                StTool::Collision(_) => {
                    section(ui, "COLLISION");
                    ui.label("Click or drag over cells to set how they block movement once the structure is placed:");
                    for (c, col, text) in [
                        (1, Color32::from_rgb(220, 60, 60), "Solid: always blocks"),
                        (2, Color32::from_rgb(60, 200, 90), "Walkable: never blocks (doors, gaps)"),
                        (StructureDef::COLLISION_AUTO, Color32::from_gray(160), "Tileset default: blocks if the tile is solid"),
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
                _ => tileset_palette(ui, st, db, tex, assets),
            }
        });
    });
}

/// Left column: tools, layer stack, size and history.
fn sidebar(ui: &mut egui::Ui, d: &mut StructureDef, st: &mut StructEdit) {
    let w = ui.available_width();
    let tool = |ui: &mut egui::Ui, st: &mut StructEdit, t: StTool, tip: &str| {
        let r = ui.add_sized([w, 24.0], egui::SelectableLabel::new(st.tool == t, ui::t(t.icon(), t.label()))).on_hover_text(tip);
        if r.clicked() {
            st.tool = t;
        }
    };
    section(ui, "TILES");
    tool(ui, st, StTool::Stamp, "Paint the stamp selected in the tileset on the right (click or drag on the grid; right-click a cell picks its tile)");
    tool(ui, st, StTool::Keep, "Empty cell: placing the structure leaves the map's tile there");
    tool(ui, st, StTool::Clear, "Placing the structure removes the map's tile on this layer (e.g. trees inside a room) — red cross");
    section(ui, "COLLISION");
    tool(ui, st, StTool::Collision(1), "Always blocks");
    tool(ui, st, StTool::Collision(2), "Never blocks (doors)");
    tool(ui, st, StTool::Collision(StructureDef::COLLISION_AUTO), "Blocks if the tile is solid in its tileset");
    tool(ui, st, StTool::Collision(0), "Leave the map's collision alone");
    section(ui, "OBJECTS");
    tool(ui, st, StTool::Prop, "Put props (doors, chests, signs…) into the structure; right-click removes one");

    section(ui, "LAYERS");
    let uses = st.tool.uses_layers();
    ui.add_enabled_ui(uses, |ui| {
        for (i, name, desc) in LAYERS {
            let on = st.layer == i;
            let (tiles, clears) = d.layers.get(i).map_or((0, 0), |l| {
                (l.iter().filter(|&&v| v != 0 && v != StructureDef::CLEAR).count(), l.iter().filter(|&&v| v == StructureDef::CLEAR).count())
            });
            let n = |t: usize| if t == 1 { "1 tile".to_string() } else { format!("{t} tiles") };
            let count = match (tiles, clears) {
                (0, 0) => "empty".to_string(),
                (t, 0) => n(t),
                (0, c) => format!("clears {c}"),
                (t, c) => format!("{}, clears {c}", n(t)),
            };
            let frame = egui::Frame::new()
                .fill(if on && uses { ACCENT } else { Color32::from_gray(38) })
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(6, 4));
            let r = frame
                .show(ui, |ui| {
                    ui.set_width(w - 12.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(name).strong().color(Color32::WHITE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mut shown = !st.hidden[i];
                            if ui.checkbox(&mut shown, "").on_hover_text("Show this layer in the grid").changed() {
                                st.hidden[i] = !shown;
                            }
                            ui.label(egui::RichText::new(&count).small().color(Color32::from_gray(190)))
                                .on_hover_text("Tiles this layer sets, and cells where placing removes the map's tile");
                        });
                    });
                    ui.label(egui::RichText::new(desc).small().color(Color32::from_gray(if on { 225 } else { 150 })));
                })
                .response
                .interact(Sense::click());
            if r.on_hover_text(format!("Paint on the {name} layer")).clicked() {
                st.layer = i;
                st.hidden[i] = false;
            }
            ui.add_space(2.0);
        }
    });
    if !uses {
        ui.weak("Layers only matter for tile tools.");
    }
    ui.checkbox(&mut st.dim, "Fade the other layers");

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
        let chip = |ui: &mut egui::Ui, text: String, fill: Color32| {
            egui::Frame::new().fill(fill).corner_radius(10.0).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                ui.label(egui::RichText::new(text).color(Color32::WHITE));
            });
        };
        chip(ui, ui::t(st.tool.icon(), st.tool.label()), Color32::from_gray(70));
        if st.tool.uses_layers() {
            ui.label("on");
            chip(ui, format!("{} layer", layer_name(st.layer)), ACCENT);
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
        legend(ui, Color32::from_rgb(230, 70, 70), "× clears the map tile");
        if matches!(st.tool, StTool::Collision(_)) {
            legend(ui, Color32::from_rgb(220, 60, 60), "solid");
            legend(ui, Color32::from_rgb(60, 200, 90), "walkable");
            legend(ui, Color32::from_gray(160), "tileset default");
        }
        ui.label(egui::RichText::new("· right-click picks a tile").small().color(Color32::from_gray(160)));
    });
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

/// Right column (Place prop): searchable gallery of the props.
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
    ui.weak("Click a cell to place it; right-click a cell removes the prop standing there.");
}

/// Right column (tile tools): tileset combo and image; click a tile or drag over several to
/// select the stamp; a large preview of the stamp below.
fn tileset_palette(ui: &mut egui::Ui, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    section(ui, "TILESET");
    egui::ComboBox::from_id_salt("st_ts").selected_text(st.tileset.as_str()).width(ui.available_width() - 10.0).show_ui(ui, |ui| {
        for t in &db.tilesets {
            if ui.selectable_label(st.tileset == t.id, &t.id).clicked() {
                st.tileset = t.id.clone();
                st.stamp = (0, 0, 1, 1);
            }
        }
    });
    let (Some(t), Some(a)) = (tex.tilesets.get(&st.tileset), assets.tilesets.get(&st.tileset)) else {
        ui.weak("No tileset");
        return;
    };
    let ts = TILE_SIZE as f32;
    let (iw, ih) = (a.image.width as f32, a.image.height as f32);
    let scale = ((ui.available_width() - 4.0) / iw).min(1.5);
    let (rect, resp) = ui.allocate_exact_size(vec2(iw, ih) * scale, Sense::click_and_drag());
    ui.painter().rect_filled(rect, 0.0, Color32::from_gray(25));
    ui.painter().image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    let cell = ts * scale;
    let (cols, rows) = ((iw / ts) as u32, (ih / ts) as u32);
    let at = |p: egui::Pos2| (((p.x - rect.min.x) / cell).clamp(0.0, cols as f32 - 1.0) as u32, ((p.y - rect.min.y) / cell).clamp(0.0, rows as f32 - 1.0) as u32);
    if let Some(p) = resp.interact_pointer_pos() {
        let c = at(p);
        if st.pal_start.is_none() {
            // Start where the button went down (a drag is only reported after some movement).
            let origin = ui.input(|i| i.pointer.press_origin()).filter(|o| rect.contains(*o)).unwrap_or(p);
            st.pal_start = Some(at(origin));
        }
        if let Some(s) = st.pal_start {
            st.stamp = (s.0.min(c.0), s.1.min(c.1), s.0.abs_diff(c.0) + 1, s.1.abs_diff(c.1) + 1);
            if !st.tool.uses_layers() || st.tool != StTool::Stamp {
                st.tool = StTool::Stamp;
            }
        }
    }
    if !resp.is_pointer_button_down_on() {
        st.pal_start = None;
    }
    if let Some(h) = resp.hover_pos() {
        let c = at(h);
        let r = Rect::from_min_size(rect.min + vec2(c.0 as f32, c.1 as f32) * cell, vec2(cell, cell));
        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_white_alpha(120)), StrokeKind::Inside);
    }
    let (x, y, w, h) = st.stamp;
    let r = Rect::from_min_size(rect.min + vec2(x as f32, y as f32) * cell, vec2(w as f32, h as f32) * cell);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
    ui.weak("Click a tile, or drag over several for a bigger stamp.");
    section(ui, "STAMP");
    let size = vec2(w as f32, h as f32) * ts;
    let k = (120.0 / size.x.max(size.y)).min(2.0);
    let (pr, _) = ui.allocate_exact_size(size * k, Sense::hover());
    let uv = Rect::from_min_size(pos2(x as f32 * ts / iw, y as f32 * ts / ih), vec2(w as f32 * ts / iw, h as f32 * ts / ih));
    ui.painter().rect_filled(pr, 0.0, Color32::from_gray(30));
    ui.painter().image(t.id(), pr, uv, Color32::WHITE);
    ui.label(format!("{w} × {h} tiles"));
}

/// The structure's grid: visible layers (the others faded), clear marks, collision, props;
/// paints with the current tool.
fn structure_canvas(ui: &mut egui::Ui, d: &mut StructureDef, st: &mut StructEdit, db: &Database, tex: &crate::widgets::Textures, assets: &rustpg2d_engine::render::Assets) {
    let ts = TILE_SIZE as f32;
    let fit = ((ui.available_width() - 8.0) / d.w.max(1) as f32).clamp(12.0, 48.0).floor();
    let cell = if st.zoom > 0.0 { st.zoom } else { fit };
    let size = vec2(d.w as f32, d.h as f32) * cell;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(rect.expand(2.0));
    painter.rect_filled(rect.expand(6.0), 6.0, Color32::from_gray(22));
    // Checkerboard: empty cells.
    for y in 0..d.h {
        for x in 0..d.w {
            let c = if (x + y) % 2 == 0 { 46 } else { 38 };
            painter.rect_filled(Rect::from_min_size(rect.min + vec2(x as f32, y as f32) * cell, vec2(cell, cell)), 0.0, Color32::from_gray(c));
        }
    }
    let tile_rect = |i: usize| Rect::from_min_size(rect.min + vec2((i as u32 % d.w) as f32, (i as u32 / d.w) as f32) * cell, vec2(cell, cell));
    let faded = st.dim && st.tool.uses_layers();
    let draw_layer = |li: usize, l: &Vec<u32>| {
        if st.hidden.get(li).copied().unwrap_or(false) {
            return;
        }
        let tint = if !faded || li == st.layer { Color32::WHITE } else { Color32::from_white_alpha(70) };
        for (i, &v) in l.iter().enumerate() {
            let Some((id, index)) = d.tile_source(v) else { continue };
            let (Some(t), Some(a)) = (tex.tilesets.get(id), assets.tilesets.get(id)) else { continue };
            let (iw, ih) = (a.image.width as f32, a.image.height as f32);
            let cols = (iw / ts).floor().max(1.0) as u32;
            let uv = Rect::from_min_size(pos2((index % cols) as f32 * ts / iw, (index / cols) as f32 * ts / ih), vec2(ts / iw, ts / ih));
            painter.image(t.id(), tile_rect(i), uv, tint);
        }
    };
    for (li, l) in d.layers.iter().enumerate().take(2) {
        draw_layer(li, l);
    }
    for e in &d.entities {
        if let Some(s) = entity_sprite(db, e) {
            let sz = sprite_px(db, &s) * (cell / ts);
            let feet = rect.min + vec2(e.x, e.y + 1.0) * (cell / ts);
            crate::widgets::paint_sprite(ui, tex, assets, &s, Rect::from_min_size(feet - vec2(sz.x / 2.0, sz.y), sz));
        }
    }
    for (li, l) in d.layers.iter().enumerate().skip(2) {
        draw_layer(li, l);
    }
    // "Clear map tile" marks of the current layer.
    if let Some(l) = d.layers.get(st.layer).filter(|_| st.tool.uses_layers()) {
        for (i, &v) in l.iter().enumerate() {
            if v == StructureDef::CLEAR {
                let r = tile_rect(i).shrink(cell * 0.25);
                let s = Stroke::new(2.0_f32, Color32::from_rgb(230, 70, 70));
                painter.line_segment([r.left_top(), r.right_bottom()], s);
                painter.line_segment([r.right_top(), r.left_bottom()], s);
            }
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

    // Hover: ghost of what a click would do.
    let cell_at = |p: egui::Pos2| -> Option<(u32, u32)> {
        let v = (p - rect.min) / cell;
        (v.x >= 0.0 && v.y >= 0.0 && v.x < d.w as f32 && v.y < d.h as f32).then_some((v.x as u32, v.y as u32))
    };
    if let Some(c) = resp.hover_pos().and_then(cell_at) {
        let (w, h) = if st.tool == StTool::Stamp { (st.stamp.2, st.stamp.3) } else { (1, 1) };
        if st.tool == StTool::Stamp {
            if let (Some(t), Some(a)) = (tex.tilesets.get(&st.tileset), assets.tilesets.get(&st.tileset)) {
                let (iw, ih) = (a.image.width as f32, a.image.height as f32);
                let (sx, sy, sw, sh) = st.stamp;
                let uv = Rect::from_min_size(pos2(sx as f32 * ts / iw, sy as f32 * ts / ih), vec2(sw as f32 * ts / iw, sh as f32 * ts / ih));
                let r = Rect::from_min_size(rect.min + vec2(c.0 as f32, c.1 as f32) * cell, vec2(sw as f32, sh as f32) * cell);
                painter.image(t.id(), r, uv, Color32::from_white_alpha(160));
            }
        }
        let r = Rect::from_min_size(rect.min + vec2(c.0 as f32, c.1 as f32) * cell, vec2(w as f32, h as f32) * cell);
        painter.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
        let what = if st.tool.uses_layers() { format!(" · {} layer", layer_name(st.layer)) } else { String::new() };
        resp.clone().on_hover_text(format!("cell {}, {}{what}", c.0, c.1));
    }

    // Painting: one undo snapshot per stroke; a stroke paints each cell once.
    let primary = ui.input(|i| i.pointer.primary_down());
    if let Some(c) = resp.interact_pointer_pos().and_then(cell_at) {
        if resp.secondary_clicked() {
            match st.tool {
                StTool::Prop => {
                    let at = |e: &EntityInstance| ((e.x / ts).floor() as u32, ((e.y - 1.0) / ts).floor() as u32);
                    if let Some(i) = d.entities.iter().position(|e| at(e) == c) {
                        st.snapshot(d);
                        d.entities.remove(i);
                    }
                }
                _ => {
                    // Eyedropper: pick the tile of this cell on the current layer.
                    if let Some((id, index)) = d.tile_source(d.tile(st.layer, c.0, c.1)) {
                        if let Some(a) = assets.tilesets.get(id) {
                            st.tileset = id.to_string();
                            st.stamp = (index % a.columns.max(1), index / a.columns.max(1), 1, 1);
                            st.tool = StTool::Stamp;
                        }
                    }
                }
            }
        } else if resp.clicked() && st.last_cell.is_none() {
            // A click so quick that the button was never seen held down on the grid.
            st.snapshot(d);
            st.apply(d, c, db, assets);
        } else if primary && (resp.drag_started() || resp.is_pointer_button_down_on()) && st.last_cell != Some(c) {
            if st.last_cell.is_none() {
                st.snapshot(d);
            }
            if st.tool != StTool::Prop || st.last_cell.is_none() {
                st.apply(d, c, db, assets);
            }
            st.last_cell = Some(c);
        }
    }
    if !primary {
        st.last_cell = None;
    }
}
