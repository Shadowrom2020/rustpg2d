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

thread_local! {
    /// Structures' thumbnails as the game shows them (by id: a hash of the definition and the
    /// texture), the ids waiting to be rendered, and a structure asked to be edited on the map.
    static THUMBS: std::cell::RefCell<std::collections::HashMap<String, (u64, macroquad::texture::Texture2D)>> = Default::default();
    static THUMB_QUEUE: std::cell::RefCell<Vec<String>> = Default::default();
    static EDIT_REQUEST: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Asks for structure `id` to be opened on the map canvas (handled after the UI).
pub fn request_edit(id: &str) {
    EDIT_REQUEST.with(|r| *r.borrow_mut() = Some(id.to_string()));
}

pub fn take_edit_request() -> Option<String> {
    EDIT_REQUEST.with(|r| r.borrow_mut().take())
}

/// Paints structure `d` as the game shows it into `rect` (its rendered thumbnail; false, and
/// asked for, while there is none for this version of it yet).
pub fn paint_structure_thumb(ui: &egui::Ui, rect: Rect, d: &StructureDef) -> bool {
    if crate::ui::headless() {
        return false;
    }
    let key = structure_key(d);
    let Some(tex) = THUMBS.with(|t| t.borrow().get(&d.id).filter(|(k, _)| *k == key).map(|(_, t)| t.clone())) else {
        THUMB_QUEUE.with(|q| {
            let mut q = q.borrow_mut();
            if !q.contains(&d.id) {
                q.push(d.id.clone());
            }
        });
        return false;
    };
    let raw = unsafe { macroquad::window::get_internal_gl().quad_context.texture_raw_id(tex.raw_miniquad_id()) };
    #[allow(irrefutable_let_patterns)]
    let macroquad::miniquad::RawId::OpenGl(gl) = raw else { return false };
    let side = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(side, side));
    ui.painter().image(egui::TextureId::User(gl as u64), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    true
}

/// A hash of a structure's definition: its thumbnail and ghost are made again when it changes.
pub fn structure_key(d: &StructureDef) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ron::to_string(d).unwrap_or_default().hash(&mut h);
    h.finish()
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
        if !paint_structure_thumb(ui, rect.shrink(3.0), d) {
            paint_project_structure(ui, rect.shrink(3.0), d, &self.project.db, &self.tex, &self.assets);
        }
        if resp.hovered() || selected {
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { Color32::YELLOW } else { Color32::WHITE }), StrokeKind::Inside);
        }
        resp
    }

    /// Renders the next waiting structure thumbnail (after the canvas, before the UI that covers
    /// the window's corner it is drawn in) and reads it back into a texture.
    pub fn render_structure_thumb(&mut self) {
        use macroquad::prelude::*;
        const N: f32 = 128.0;
        let Some(id) = THUMB_QUEUE.with(|q| q.borrow_mut().pop()) else { return };
        let Some(d) = self.project.db.structure(&id).cloned() else { return };
        let ts = self.ts();
        let (w, h) = (d.w.max(1) as f32 * ts, d.h.max(1) as f32 * ts);
        let tall = 3.5 * ts;
        let s = (N * 0.92 / (w + h)).min(N * 0.92 / ((w + h) / 2.0 + tall));
        let corner = vec2(8.0, 120.0);
        let q = corner + vec2(N / 2.0, N / 2.0 + tall * s / 2.0);
        let screen = vec2(screen_width(), screen_height());
        let center = vec2(w / 2.0, h / 2.0);
        let target = center - rustpg2d_engine::render::iso::unproject((q - screen / 2.0) / s);
        self.draw_zoom.set(Some(s * rustpg2d_engine::render::iso::UPRIGHT));
        self.draw_shift.set(self.cam.center - target);
        set_default_camera();
        draw_rectangle(corner.x, corner.y, N, N, Color::from_rgba(40, 40, 40, 255));
        self.with_structure_map(&d, |editor, map| {
            let cam = editor.camera();
            set_camera(&cam);
            let view = rustpg2d_engine::render::camera_view(&cam);
            editor.ghost_ground.borrow_mut().draw(map, &editor.project.db, &editor.assets, view, WHITE);
            editor.draw_objects(map, view, false, WHITE);
        });
        self.draw_zoom.set(None);
        self.draw_shift.set(vec2(0.0, 0.0));
        set_default_camera();
        let shot = get_screen_data();
        let k = shot.width as f32 / screen.x;
        let (x0, y0, n) = ((corner.x * k) as u32, (corner.y * k) as u32, (N * k) as u32);
        let mut img = Image::gen_image_color(n as u16, n as u16, BLANK);
        for y in 0..n {
            for x in 0..n {
                // (Screen data comes bottom row first.)
                let sy = shot.height as u32 - 1 - (y0 + y);
                if x0 + x < shot.width as u32 && sy < shot.height as u32 {
                    img.set_pixel(x, y, shot.get_pixel(x0 + x, sy));
                }
            }
        }
        let tex = Texture2D::from_image(&img);
        THUMBS.with(|t| t.borrow_mut().insert(id, (structure_key(&d), tex)));
    }

    /// Creates an empty structure (6×5) and opens it on the map canvas to be built.
    pub fn new_structure(&mut self) {
        let id = (1..).map(|i| format!("structure_{i}")).find(|c| self.project.db.structure(c).is_none()).unwrap();
        self.project.db.structures.push(StructureDef { id: id.clone(), name: "New structure".into(), w: 6, h: 5, ..Default::default() });
        self.defs_dirty = true;
        self.edit_structure_on_canvas(&id);
    }

    /// Opens structure `id` on the map canvas as a small map of its own, with every map tool;
    /// saving (Ctrl+S, *Done*) captures it back into the structure. The map open now is saved
    /// first and comes back afterwards.
    pub fn edit_structure_on_canvas(&mut self, id: &str) {
        let Some(d) = self.project.db.structure(id).cloned() else { return };
        if self.doc.as_ref().is_some_and(|doc| doc.is_dirty()) {
            self.save_map();
        }
        let back = match &self.editing_structure {
            Some((_, b)) => b.clone(),
            None => self.doc.as_ref().map(|doc| doc.map.name.clone()),
        };
        let mut map = self.with_structure_map(&d, |_, m| m.clone());
        map.name = format!("@structure:{id}");
        if let Some(doc) = &self.doc {
            map.background = doc.map.background;
        }
        self.editing_structure = Some((id.to_string(), back));
        self.load_doc(map);
        self.mode = crate::app::Mode::Map;
        self.set_status(format!("Editing structure '{}' on the map: Ctrl+S or Done saves it", d.name));
    }

    /// Saves the structure being edited on the canvas from its map.
    pub fn save_structure_doc(&mut self) {
        let Some((id, _)) = self.editing_structure.clone() else { return };
        let Some(old) = self.project.db.structure(&id).cloned() else { return };
        let Some(doc) = &mut self.doc else { return };
        doc.end_stroke();
        let (w, h) = (doc.map.width, doc.map.height);
        let ground = !old.ground.is_empty() || !doc.map.materials.is_empty();
        let mut new = StructureDef::capture(&doc.map, &id, &old.name, (0, 0, w, h), ground);
        new.category = old.category.clone();
        doc.mark_saved();
        if let Some(slot) = self.project.db.structures.iter_mut().find(|s| s.id == id) {
            *slot = new;
        }
        self.defs_dirty = true;
        self.save_defs();
        self.set_status(format!("Saved structure '{}'", old.name));
    }

    /// Leaves the structure on the canvas (saving it when `save`) and opens the map it came from.
    pub fn finish_structure_edit(&mut self, save: bool) {
        let Some((_, back)) = self.editing_structure.clone() else { return };
        if save {
            self.save_structure_doc();
        }
        self.editing_structure = None;
        match back {
            Some(m) => self.open_map(&m),
            None => self.doc = None,
        }
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
    /// (also the clipboard while pasting), drawn as the game shows it: its painted ground and its
    /// decorations and entities (models, buildings, blocks, turns) in depth order.
    pub fn draw_structure_def_ghost(&self, d: &StructureDef, origin: (i32, i32)) {
        use macroquad::prelude::{draw_rectangle_lines, set_camera, vec2 as mvec2, Color, YELLOW};
        let ts = self.ts();
        self.with_structure_map(d, |editor, map| {
            let shift = mvec2(origin.0 as f32, origin.1 as f32) * ts;
            editor.draw_shift.set(shift);
            let cam = editor.camera();
            set_camera(&cam);
            let view = rustpg2d_engine::render::camera_view(&cam);
            let tint = Color::new(1.0, 1.0, 1.0, 0.7);
            editor.ghost_ground.borrow_mut().draw(map, &editor.project.db, &editor.assets, view, tint);
            editor.draw_objects(map, view, false, tint);
            editor.draw_shift.set(mvec2(0.0, 0.0));
            set_camera(&editor.camera());
        });
        let px = 1.0 / self.cam.zoom;
        let (ox, oy) = origin;
        draw_rectangle_lines(ox as f32 * ts, oy as f32 * ts, d.w as f32 * ts, d.h as f32 * ts, 2.0 * px, YELLOW);
    }

    /// Runs `f` with structure `d` placed on a map of its own (its size, its top-left corner at 0, 0;
    /// kept while the definition doesn't change).
    pub fn with_structure_map<R>(&self, d: &StructureDef, f: impl FnOnce(&Self, &rustpg2d_engine::map::Map) -> R) -> R {
        let key = structure_key(d);
        let fresh = self.ghost.borrow().as_ref().is_none_or(|(k, _)| *k != key);
        if fresh {
            let mut map = rustpg2d_engine::map::Map::new(&format!("@structure:{key:x}"), d.w.max(1), d.h.max(1));
            d.place(&mut map, 0, 0);
            *self.ghost.borrow_mut() = Some((key, map));
        }
        let cache = self.ghost.borrow();
        f(self, &cache.as_ref().unwrap().1)
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
