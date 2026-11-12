//! The character editor: a character's 3D model with a walking preview, the colours of the
//! model's colour features, its equipment (counted in play; weapons, left-hand items and tools
//! show in its hands) and carried items. Used inline by the Characters
//! form (project and library), as a window for a placed NPC's own look (`Builder`), and by the
//! New character window.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};
use rustpg2d_engine::defs::{Appearance, CharacterDef, ColorFeature, Database, EquipSlot};
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::inventory::ItemStack;
use rustpg2d_engine::render::character;
use rustpg2d_engine::render::Assets;
use rustpg2d_engine::state::Equipment;

use crate::app::Editor;
use crate::ui::ph;
use crate::widgets::{self, Textures};

/// What the character editor edits.
pub struct Parts<'a> {
    pub appearance: &'a mut Appearance,
    pub equipment: &'a mut Equipment,
    pub inventory: &'a mut Vec<ItemStack>,
}

/// What the character editor paints with.
pub struct Cx<'a> {
    pub db: &'a Database,
    pub tex: &'a Textures,
    pub assets: &'a Assets,
}

thread_local! {
    /// A model's playable flag changed in a character editor (applied by
    /// `apply_builder_requests`; the editor only sees a read-only copy of the definitions).
    static PLAYABLE: std::cell::RefCell<Option<(String, bool)>> = const { std::cell::RefCell::new(None) };
}

/// Asks the editor to set model `model` playable (or not).
pub fn request_playable(model: &str, on: bool) {
    PLAYABLE.with(|r| *r.borrow_mut() = Some((model.to_string(), on)));
}

/// Cards of the project's 3D models (standing, facing the front); playable ones marked. Returns the
/// clicked id.
pub fn model_cards(ui: &mut Ui, cx: &Cx, base: &Appearance, current: &str) -> Option<String> {
    let ids: Vec<String> = cx.db.models.iter().map(|m| m.id.clone()).collect();
    model_cards_of(ui, cx, &ids, None, base, current)
}

/// Cards of the models `ids` ("" = `none`, a card without a picture). Returns the clicked id.
pub fn model_cards_of(ui: &mut Ui, cx: &Cx, ids: &[String], none: Option<&str>, base: &Appearance, current: &str) -> Option<String> {
    let mut entries: Vec<(String, String)> = none.map(|n| (String::new(), n.to_string())).into_iter().collect();
    entries.extend(ids.iter().filter_map(|id| cx.db.model(id)).map(|m| (m.id.clone(), if m.playable { format!("{} {}", ph::GAME_CONTROLLER, m.label()) } else { m.label().to_string() })));
    let gear = rustpg2d_engine::render::model::Gear::default();
    cards(ui, THUMB, &entries, Some(&current.to_string()), &|ui, id, r| {
        if id.is_empty() {
            return false;
        }
        let a = Appearance { model: id.clone(), colors: if *id == base.model { base.colors.clone() } else { Default::default() }, ..base.clone() };
        let req = rustpg2d_engine::render::model::PoseReq { look: &a, gear: &gear, anim: "idle", time: 0.0, once: false, facing: Facing::Down, bust: false };
        widgets::paint_model(ui, cx.assets, cx.db, &req, r)
    })
}

/// What happened in the character editor this frame.
#[derive(Default)]
pub struct Out {
    pub changed: bool,
    /// "Start from…" was clicked: open the template chooser.
    pub template: bool,
}

/// Preview state kept between frames (egui temp data).
#[derive(Clone, Copy)]
struct Preview {
    facing: Facing,
    turning: bool,
    /// What the big preview plays: 0 walk, 1 idle, 2 attack, 3 hurt.
    anim: u8,
}

thread_local! {
    /// Editor script `charanim walk|idle|attack|hurt`: the animation every preview plays.
    static PREVIEW_ANIM: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
}

/// Sets the previews' animation (editor script).
pub fn set_preview_anim(name: &str) {
    let k = match name {
        "idle" => 1,
        "attack" => 2,
        "hurt" => 3,
        _ => 0,
    };
    PREVIEW_ANIM.with(|a| a.set(Some(k)));
}

/// The animation frame of preview mode `anim` at time `t` (an attack plays its 4 frames, then rests).
pub fn preview_anim(anim: u8, t: f64) -> character::Anim {
    use character::Anim;
    match anim {
        1 => Anim::Idle((t * 1.25) as u32 % 2),
        2 => {
            let k = (t * 6.0) as u32 % 7;
            if k < 4 {
                Anim::Attack(k)
            } else {
                Anim::Idle(0)
            }
        }
        3 => Anim::Hurt,
        _ => Anim::Walk((t * 7.0) as u32 % character::LAYOUT_FRAMES),
    }
}

const THUMB: egui::Vec2 = vec2(52.0, 72.0);
const ICON: egui::Vec2 = vec2(44.0, 44.0);
const PREVIEW_H: f32 = 150.0;

/// A wrapping grid of clickable cards (a painted thumbnail with a label). The card equal to
/// `current` is framed. Returns the clicked value.
pub fn cards<T: PartialEq + Clone>(ui: &mut Ui, size: egui::Vec2, entries: &[(T, String)], current: Option<&T>, paint: &dyn Fn(&Ui, &T, Rect) -> bool) -> Option<T> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (value, label) in entries {
            let (r, resp) = ui.allocate_exact_size(size + vec2(8.0, 20.0), Sense::click());
            let on = current == Some(value);
            let bg = if resp.hovered() { Color32::from_gray(72) } else if on { Color32::from_rgb(58, 70, 100) } else { Color32::from_gray(46) };
            ui.painter().rect_filled(r, 4.0, bg);
            let img = Rect::from_min_size(r.min + vec2(4.0, 3.0), size);
            if !paint(ui, value, img) {
                ui.painter().text(img.center(), egui::Align2::CENTER_CENTER, "none", egui::FontId::proportional(12.0), Color32::GRAY);
            }
            let short: String = label.chars().take(10).collect();
            ui.painter().text(pos2(r.center().x, r.max.y - 9.0), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(11.0), Color32::LIGHT_GRAY);
            if on {
                ui.painter().rect_stroke(r, 4.0, Stroke::new(2.0_f32, Color32::from_rgb(255, 210, 90)), StrokeKind::Inside);
            }
            if resp.on_hover_text(label).clicked() {
                picked = Some(value.clone());
            }
        }
    });
    picked
}

/// One clickable colour square (framed when `on`).
pub fn swatch(ui: &mut Ui, c: [u8; 3], on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    ui.painter().rect_filled(r, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
    if on {
        ui.painter().rect_stroke(r, 3.0, Stroke::new(2.0_f32, Color32::WHITE), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
    }
    resp.on_hover_text(format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
}

/// The colour of one feature: *As drawn*, the feature's offered colours, any other colour.
/// Returns true when it changed.
pub fn feature_colors_ui(ui: &mut Ui, f: &ColorFeature, a: &mut Appearance) -> bool {
    let mut changed = false;
    let current = a.colors.get(&f.name).copied();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        ui.label(f.label());
        if ui.selectable_label(current.is_none(), "As drawn").on_hover_text("The art's own colours").clicked() && current.is_some() {
            a.colors.remove(&f.name);
            changed = true;
        }
        for c in &f.colors {
            if swatch(ui, *c, current == Some(*c)).clicked() {
                a.colors.insert(f.name.clone(), *c);
                changed = true;
            }
        }
        let mut any = current.unwrap_or_else(|| f.shades.get(f.shades.len() / 2).copied().unwrap_or([128, 128, 128]));
        if ui.color_edit_button_srgb(&mut any).on_hover_text("Any other colour (only for this character: the creator offers the feature's colours)").changed() {
            a.colors.insert(f.name.clone(), any);
            changed = true;
        }
    });
    changed
}

/// Items that fit `slot` as card entries: (id, name).
fn items_of(db: &Database, slot: EquipSlot) -> Vec<(String, String)> {
    let mut v = vec![(String::new(), "None".to_string())];
    v.extend(db.items.iter().filter(|d| d.equip_slot() == Some(slot)).map(|d| (d.id.clone(), d.name.clone())));
    v
}

/// Icon cards of the items for `slot`.
fn item_cards(ui: &mut Ui, cx: &Cx, slot: EquipSlot, eq: &Equipment) -> Option<String> {
    let entries = items_of(cx.db, slot);
    let current = eq.item(slot).unwrap_or("").to_string();
    cards(ui, ICON, &entries, Some(&current), &|ui, id, r| !id.is_empty() && cx.db.item(id).is_some_and(|d| widgets::paint_sprite(ui, cx.tex, cx.assets, &d.icon, r)))
}

/// A character with what it holds (`eq`: weapon, left hand) in `dest`, its model rendered with
/// its gear (the mannequin without one).
fn paint_holding(ui: &Ui, cx: &Cx, a: &Appearance, eq: &Equipment, (facing, anim): (Facing, character::Anim), dest: Rect) -> bool {
    let a = cx.assets.models.look_of(a);
    let gear = rustpg2d_engine::render::model::Gear::of(eq, "", None);
    let (name, time, once) = rustpg2d_engine::render::model::anim_req(anim, macroquad::time::get_time() as f32);
    let req = rustpg2d_engine::render::model::PoseReq { look: &a, gear: &gear, anim: name, time, once, facing, bust: false };
    widgets::paint_model(ui, cx.assets, cx.db, &req, dest)
}

/// The colour features of a look's model.
pub fn look_features(db: &Database, a: &Appearance) -> Vec<ColorFeature> {
    db.model(&a.model).map(|m| m.features.clone()).unwrap_or_default()
}

/// World size of a look's frame.
fn look_size(cx: &Cx, a: &Appearance) -> (f32, f32) {
    cx.assets.models.frame_size(&cx.assets.models.look_of(a).model).unwrap_or((32.0, 48.0))
}

/// The animated preview: facing `facing` (or turning) playing the chosen animation, the four
/// idle directions below; with what it holds.
fn preview(ui: &mut Ui, id: egui::Id, cx: &Cx, a: &Appearance, eq: &Equipment) {
    let mut st = ui.ctx().data_mut(|d| *d.get_temp_mut_or(id, Preview { facing: Facing::Down, turning: true, anim: 0 }));
    if let Some(k) = PREVIEW_ANIM.with(|a| a.take()) {
        st.anim = k;
    }
    let t = macroquad::time::get_time();
    let order = [Facing::Down, Facing::Left, Facing::Up, Facing::Right];
    let facing = if st.turning { order[(t / 1.5) as usize % 4] } else { st.facing };
    let anim = preview_anim(st.anim, t);
    let (fw, fh) = look_size(cx, a);
    let scale = (PREVIEW_H / fh).floor().max(1.0);
    ui.vertical(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2((fw * scale).max(120.0), PREVIEW_H + 6.0), Sense::hover());
        ui.painter().rect_filled(r, 4.0, Color32::from_gray(34));
        let dest = Rect::from_min_size(pos2(r.center().x - fw * scale / 2.0, r.max.y - 3.0 - fh * scale), vec2(fw * scale, fh * scale));
        if !paint_holding(ui, cx, a, eq, (facing, anim), dest) {
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "rendering…", egui::FontId::default(), Color32::GRAY);
        }
        ui.horizontal(|ui| {
            for (f, label) in [(Facing::Down, ph::ARROW_DOWN), (Facing::Left, ph::ARROW_LEFT), (Facing::Up, ph::ARROW_UP), (Facing::Right, ph::ARROW_RIGHT)] {
                if ui.selectable_label(!st.turning && st.facing == f, label).on_hover_text(f.label()).clicked() {
                    st.facing = f;
                    st.turning = false;
                }
            }
            ui.toggle_value(&mut st.turning, ph::ARROWS_CLOCKWISE).on_hover_text("Turn around");
        });
        ui.horizontal(|ui| {
            for (k, label, tip) in [(0u8, "Walk", "The walk cycle"), (1, "Idle", "Standing still: breathing"), (2, "Attack", "Wind-up, swing, follow-through, recover"), (3, "Hurt", "Just hit")] {
                if ui.selectable_label(st.anim == k, label).on_hover_text(tip).clicked() {
                    st.anim = k;
                }
            }
        });
        ui.horizontal(|ui| {
            for f in order {
                let (r, _) = ui.allocate_exact_size(vec2(fw + 2.0, fh + 2.0), Sense::hover());
                paint_holding(ui, cx, a, eq, (f, character::Anim::Idle(0)), r.shrink(1.0));
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, st));
    ui.ctx().request_repaint();
}

/// The character editor: preview on the left, the choices on the right. `id` keeps its state
/// apart.
pub fn builder_ui(ui: &mut Ui, id: &str, p: Parts, cx: &Cx) -> Out {
    let mut out = Out::default();
    let Parts { appearance: a, equipment: eq, inventory } = p;
    ui.horizontal_top(|ui| {
        preview(ui, egui::Id::new(("builder_preview", id)), cx, a, eq);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.set_min_width(ui.available_width().min(560.0));
            let features: Vec<ColorFeature> = look_features(cx.db, a);
            ui.horizontal(|ui| {
                if ui.add_enabled(features.iter().any(|f| !f.colors.is_empty()), egui::Button::new(crate::ui::t(ph::DICE_FIVE, "Random colours"))).on_hover_text("A random colour of each colour feature that offers colours").clicked() {
                    random_colors(a, cx.db);
                    out.changed = true;
                }
                if ui.button(crate::ui::t(ph::USERS, "Start from…")).on_hover_text("Copy the model and colours of a character of this project or the asset library").clicked() {
                    out.template = true;
                }
            });
            let section = |ui: &mut Ui, title: &str, open: bool, body: &mut dyn FnMut(&mut Ui)| {
                egui::CollapsingHeader::new(title).id_salt((id, title)).default_open(open).show(ui, |ui| body(ui));
            };
            section(ui, "Model", true, &mut |ui| {
                if let Some(m) = model_cards(ui, cx, a, &a.model) {
                    if m != a.model {
                        // Another model has other features.
                        a.colors.clear();
                    }
                    a.model = m;
                    a.body.clear();
                    out.changed = true;
                }
                if let Some(m) = cx.db.model(&a.model) {
                    let mut on = m.playable;
                    let label = crate::ui::t(ph::GAME_CONTROLLER, &format!("Playable: players may choose {} in the character creator", m.label()));
                    if ui.checkbox(&mut on, label).on_hover_text("For the whole project (a slime or a ghost usually isn't). The creator offers the playable models of the chosen sex, with their colour features.").changed() {
                        request_playable(&m.id, on);
                    }
                }
                if cx.db.models.is_empty() {
                    ui.colored_label(Color32::YELLOW, "No 3D models in this project: import one in Database › 3D models, generate one (New character… › AI) or add some from the asset library. Until then characters show as the mannequin.");
                }
            });
            section(ui, "Colours", true, &mut |ui| {
                if features.is_empty() {
                    ui.weak("This model has no colour features. Pick its skin, hair, clothing… in Database › 3D models › Colour features to recolour it.");
                }
                for f in &features {
                    out.changed |= feature_colors_ui(ui, f, a);
                }
            });
            section(ui, "Equipment", !eq.is_empty(), &mut |ui| {
                ui.weak("Counts in play (damage, defense, what it drops). Weapons and left-hand items show in the hands (as their 3D model or their picture); worn armour shows only in the model's art.");
                for slot in [EquipSlot::Weapon, EquipSlot::Offhand, EquipSlot::Head, EquipSlot::Body, EquipSlot::Clothing] {
                    ui.label(match slot {
                        EquipSlot::Body => "Body armor",
                        EquipSlot::Weapon => "Weapon",
                        s => s.label(),
                    });
                    if let Some(c) = item_cards(ui, cx, slot, eq) {
                        *eq.slot_mut(slot) = (!c.is_empty()).then(|| ItemStack::new(&c, 1));
                        out.changed = true;
                    }
                }
            });
            section(ui, "Carried items", !inventory.is_empty(), &mut |ui| {
                ui.weak("Dropped with the equipment when the character is defeated (the player character: its start inventory).");
                let items: Vec<String> = cx.db.items.iter().map(|d| d.id.clone()).collect();
                let before = inventory.clone();
                widgets::carried_ui(ui, &format!("{id}/inv"), inventory, &items);
                out.changed |= *inventory != before;
            });
        });
    });
    out
}

/// A random offered colour for each colour feature of the model that offers colours.
pub fn random_colors(a: &mut Appearance, db: &Database) {
    use macroquad::rand::{gen_range, srand};
    srand((macroquad::time::get_time() * 1000.0) as u64 ^ a.model.len() as u64);
    for f in look_features(db, a).iter().filter(|f| !f.colors.is_empty()) {
        a.colors.insert(f.name.clone(), f.colors[gen_range(0, f.colors.len())]);
    }
}

/// A fresh character: the first playable model of the project (else none: the mannequin), with
/// walking speed and hitbox of a person.
pub fn default_character(db: &Database, id: &str, name: &str) -> CharacterDef {
    let model = db.models.iter().find(|m| m.playable).map(|m| m.id.clone()).unwrap_or_default();
    let u = rustpg2d_engine::project::TILE_SIZE as f32 / 16.0;
    CharacterDef {
        name: name.into(),
        speed: 50.0 * u,
        hitbox: [10.0 * u, 6.0 * u],
        appearance: Appearance { model, ..Default::default() },
        ..CharacterDef::fresh(id)
    }
}

/// Lowercase letters, digits and underscores.
pub fn clean_id(s: &str) -> String {
    s.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

// ------------------------------------------------------------------ the builder window

/// What the builder window creates or changes.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A new character definition.
    New,
    /// The own look / items of a placed character (entity uid).
    Instance(u32),
}

pub struct Builder {
    pub target: Target,
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
    pub equipment: Equipment,
    pub inventory: Vec<ItemStack>,
    pub error: Option<String>,
    /// New characters: where the model comes from.
    pub source: Source,
    /// *Generate with AI*: a woman.
    pub female: bool,
}

/// Where a new character's model comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A model of the project (or one added from the asset library).
    Existing,
    /// A rigged `.glb` (Meshy, Blender, Mixamo…).
    Import,
    /// Generated with Meshy (Generate art: a 3D character).
    Ai,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Existing, Source::Import, Source::Ai];
    pub fn label(self) -> &'static str {
        match self {
            Source::Existing => "Existing model",
            Source::Import => "Import a model",
            Source::Ai => "Generate with AI",
        }
    }
    pub fn parse(s: &str) -> Option<Source> {
        Some(match s {
            "existing" => Source::Existing,
            "import" => Source::Import,
            "ai" => Source::Ai,
            _ => return None,
        })
    }
}

impl Editor {
    /// Opens Database › Sprite sheets on `sheet` (its form, the sprite editor below).
    pub fn edit_sheet(&mut self, sheet: &str) {
        if let Some(k) = self.project.db.sheets.iter().position(|s| s.id == sheet) {
            self.mode = crate::app::Mode::Database;
            self.db.tab = crate::database::Tab::Sheets;
            self.db.sel = k;
        }
    }

    /// Applies a playable flag changed in a builder (see [`request_playable`]).
    pub fn apply_builder_requests(&mut self) {
        let Some((model, on)) = PLAYABLE.with(|r| r.borrow_mut().take()) else { return };
        if let Some(m) = self.project.db.models.iter_mut().find(|m| m.id == model) {
            m.playable = on;
            self.defs_dirty = true;
            let what = if on { "can now be chosen" } else { "can no longer be chosen" };
            self.set_status(format!("Model '{model}' {what} in the character creator. Save the definitions to keep it."));
        }
    }

    /// Opens the builder window for `target`.
    pub fn open_builder(&mut self, target: Target) {
        let db = &self.project.db;
        let (id, name, d) = match &target {
            Target::New => {
                let ids: Vec<String> = db.characters.iter().map(|d| d.id.clone()).collect();
                let mut n = 1;
                while ids.contains(&format!("character_{n}")) {
                    n += 1;
                }
                (format!("character_{n}"), String::new(), default_character(db, "", ""))
            }
            Target::Instance(uid) => {
                let e = self.doc.as_ref().and_then(|d| d.map.entities.iter().find(|e| e.uid == *uid));
                let Some(def) = e.and_then(|e| db.character(&e.def)) else {
                    self.set_status("Select a placed character first");
                    return;
                };
                let mut d = def.clone();
                d.appearance = character::appearance_of(def, e).clone();
                d.equipment = character::equipment_of(def, e).clone();
                d.inventory = e.and_then(|e| e.inventory.clone()).unwrap_or_else(|| def.inventory.clone());
                (def.id.clone(), def.name.clone(), d)
            }
        };
        self.builder = Some(Builder { target, id, name, appearance: d.appearance, equipment: d.equipment, inventory: d.inventory, error: None, source: Source::Existing, female: false });
    }

    /// Creates the character / writes the NPC's own look. Returns a status message.
    pub fn apply_builder(&mut self) -> anyhow::Result<String> {
        let Some(b) = self.builder.take() else { anyhow::bail!("the builder is not open") };
        let msg = match &b.target {
            Target::New => {
                let id = clean_id(&b.id);
                if id.is_empty() {
                    self.builder = Some(b);
                    anyhow::bail!("Please enter an id");
                }
                if self.project.db.character(&id).is_some() {
                    self.builder = Some(b);
                    anyhow::bail!("The id '{id}' is already used");
                }
                if b.source != Source::Existing {
                    self.builder = Some(b);
                    anyhow::bail!("Import or generate the model first (the buttons above), then choose it under Existing model");
                }
                let name = if b.name.trim().is_empty() { id.replace('_', " ") } else { b.name.trim().to_string() };
                let mut d = default_character(&self.project.db, &id, &name);
                d.appearance = b.appearance;
                d.equipment = b.equipment;
                d.inventory = b.inventory;
                self.project.db.characters.push(d);
                self.defs_dirty = true;
                self.db.tab = crate::database::Tab::Characters;
                self.db.sel = self.project.db.characters.len() - 1;
                self.mode = crate::app::Mode::Database;
                format!("Created character '{id}'. Set its behavior and combat values in the form, then save the definitions.")
            }
            Target::Instance(uid) => {
                let doc = self.doc.as_mut().ok_or_else(|| anyhow::anyhow!("no map open"))?;
                let before = crate::doc::Objects::of(&doc.map);
                let e = doc.map.entities.iter_mut().find(|e| e.uid == *uid).ok_or_else(|| anyhow::anyhow!("no entity #{uid}"))?;
                let def = self.project.db.character(&e.def);
                // Same as the definition's = no override.
                e.appearance = def.filter(|d| d.appearance == b.appearance).map_or(Some(b.appearance), |_| None);
                e.equipment = def.filter(|d| d.equipment == b.equipment).map_or(Some(b.equipment), |_| None);
                e.inventory = def.filter(|d| d.inventory == b.inventory).map_or(Some(b.inventory), |_| None);
                doc.record_objects(before, "look");
                format!("#{uid} has its own look now.")
            }
        };
        self.reload_assets();
        Ok(msg)
    }

    /// Shows the builder window.
    pub fn builder_window(&mut self, ctx: &egui::Context) {
        let Some(mut b) = self.builder.take() else { return };
        let mut open = true;
        let mut done: Option<bool> = None;
        let mut template = false;
        let title = match &b.target {
            Target::New => "New character".to_string(),
            Target::Instance(uid) => format!("Look of #{uid} (only this one)"),
        };
        let mut import = false;
        let mut ai = false;
        egui::Window::new(title).id(egui::Id::new("builder")).open(&mut open).collapsible(false).default_size(vec2(820.0, 600.0)).show(ctx, |ui| {
            if b.target == Target::New {
                ui.horizontal(|ui| {
                    for s in Source::ALL {
                        ui.selectable_value(&mut b.source, s, s.label());
                    }
                });
                ui.separator();
                egui::Grid::new("builder_head").num_columns(2).show(ui, |ui| {
                    ui.label("Id");
                    ui.add(egui::TextEdit::singleline(&mut b.id).hint_text("e.g. innkeeper").desired_width(200.0));
                    ui.end_row();
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut b.name).hint_text("shown in game").desired_width(200.0));
                    ui.end_row();
                });
            } else {
                ui.weak(format!("{} ({}) - this placed character's own look, equipment and carried items.", b.name, b.id));
            }
            ui.separator();
            match b.source {
                Source::Import if b.target == Target::New => {
                    ui.label("A rigged 3D model (.glb from Meshy, Blender, Mixamo…): it is copied into models/ and its clips are matched to the animations. Then choose it under Existing model. Animals and monsters without a skeleton get a rig template in Database › 3D models.");
                    if ui.button(crate::ui::t(ph::DOWNLOAD_SIMPLE, "Import a model…")).clicked() {
                        import = true;
                    }
                }
                Source::Ai if b.target == Target::New => {
                    ui.label("Meshy makes a 3D character (Generate art, with your Meshy account): a turnaround picture, a model, a skeleton and animations. Add to project creates the model and the character with this id.");
                    ui.checkbox(&mut b.female, "a woman");
                    if ui.button(crate::ui::t(ph::PAINT_BRUSH, "Open Generate art…")).clicked() {
                        ai = true;
                    }
                }
                _ => {
                    egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        let cx = Cx { db: &self.project.db, tex: &self.tex, assets: &self.assets };
                        let out = builder_ui(ui, "builder_window", Parts { appearance: &mut b.appearance, equipment: &mut b.equipment, inventory: &mut b.inventory }, &cx);
                        template |= out.template;
                    });
                }
            }
            ui.separator();
            if let Some(e) = &b.error {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
            ui.horizontal(|ui| {
                let label = if b.target == Target::New { "Create" } else { "Apply" };
                if ui.button(label).clicked() {
                    done = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    done = Some(false);
                }
                if let Target::Instance(uid) = b.target {
                    if ui.button("Use the definition's look").on_hover_text("Drop this NPC's own look, equipment and carried items").clicked() {
                        self.reset_instance_look(uid);
                        done = Some(false);
                    }
                }
            });
        });
        if template {
            crate::models::request_model(crate::models::ModelTarget::Builder);
        }
        if import {
            self.open_file_dialog(crate::files::Purpose::ModelFile { id: String::new(), clips: false });
            b.source = Source::Existing;
        }
        if ai {
            let w = &mut self.art.window;
            w.open = true;
            w.kind = crate::art::ArtKind::Model3d;
            w.female = b.female;
            w.id = clean_id(&b.id);
            if !b.name.trim().is_empty() && w.description.trim().is_empty() {
                w.description = b.name.trim().to_string();
            }
            self.builder = None;
            return;
        }
        match done {
            Some(true) => {
                self.builder = Some(b);
                match self.apply_builder() {
                    Ok(m) => self.set_status(m),
                    Err(e) => {
                        if let Some(b) = &mut self.builder {
                            b.error = Some(format!("{e:#}"));
                        }
                    }
                }
            }
            Some(false) => {}
            None if open => self.builder = Some(b),
            None => {}
        }
    }

    /// Clears a placed character's own look, equipment and carried items.
    pub fn reset_instance_look(&mut self, uid: u32) {
        if let Some(doc) = &mut self.doc {
            let before = crate::doc::Objects::of(&doc.map);
            if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                e.appearance = None;
                e.equipment = None;
                e.inventory = None;
            }
            doc.record_objects(before, "look");
        }
    }

    /// The parts a script command edits: the open builder window, else the character
    /// selected in Database › Characters.
    pub fn builder_parts(&mut self) -> Option<Parts<'_>> {
        if let Some(b) = &mut self.builder {
            return Some(Parts { appearance: &mut b.appearance, equipment: &mut b.equipment, inventory: &mut b.inventory });
        }
        if self.mode == crate::app::Mode::Database && self.db.tab == crate::database::Tab::Characters {
            let d = self.project.db.characters.get_mut(self.db.sel)?;
            self.defs_dirty = true;
            return Some(Parts { appearance: &mut d.appearance, equipment: &mut d.equipment, inventory: &mut d.inventory });
        }
        None
    }

    /// Script command `charset KEY VALUE`.
    pub fn builder_set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let rgb = |v: &str| u32::from_str_radix(v.trim_start_matches('#'), 16).ok().map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        if let Some(b) = &mut self.builder {
            match key {
                "id" => {
                    b.id = value.to_string();
                    return Ok(());
                }
                "name" => {
                    b.name = value.to_string();
                    return Ok(());
                }
                "source" => {
                    b.source = Source::parse(value).ok_or("source: existing, import or ai")?;
                    return Ok(());
                }
                "female" => {
                    b.female = value == "on" || value == "true";
                    return Ok(());
                }
                _ => {}
            }
        }
        let db = self.project.db.clone();
        let Some(p) = self.builder_parts() else { return Err("no builder open and no character selected".into()) };
        match key {
            // `model ID` (or `-`: none, the mannequin).
            "model" => {
                let id = if value == "-" { "" } else { value };
                if !id.is_empty() && db.model(id).is_none() {
                    return Err(format!("unknown model '{id}'"));
                }
                if p.appearance.model != id {
                    p.appearance.colors.clear();
                }
                p.appearance.model = id.to_string();
                p.appearance.body.clear();
            }
            // `color FEATURE RRGGBB` (or `-`: as drawn).
            "color" => {
                let (feature, c) = value.split_once(' ').ok_or("color: FEATURE RRGGBB|- expected")?;
                if c.trim() == "-" {
                    p.appearance.colors.remove(feature);
                } else {
                    p.appearance.colors.insert(feature.to_string(), rgb(c.trim()).ok_or("color: RRGGBB expected")?);
                }
            }
            "clothing" | "head" | "armor" | "body_armor" | "weapon" | "offhand" => {
                let slot = EquipSlot::parse(if key == "armor" || key == "body_armor" { "body" } else { key }).unwrap();
                if !value.is_empty() && db.item(value).is_none() {
                    return Err(format!("unknown item '{value}'"));
                }
                *p.equipment.slot_mut(slot) = (!value.is_empty()).then(|| ItemStack::new(value, 1));
            }
            "carry" => {
                let (item, n) = value.split_once('x').unwrap_or((value, "1"));
                p.inventory.push(ItemStack::new(item, n.parse().unwrap_or(1)));
            }
            "random" => random_colors(p.appearance, &db),
            other => return Err(format!("unknown key '{other}'")),
        }
        Ok(())
    }

    /// Builder state for the `status` command.
    pub fn builder_status(&self) -> String {
        match &self.builder {
            None => "closed".into(),
            Some(b) => {
                let worn: Vec<&str> = b.equipment.stacks().map(|s| s.item.as_str()).collect();
                let colors: Vec<String> = b.appearance.colors.iter().map(|(k, v)| format!("{k}:{:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect();
                format!("{:?} source={:?} id={} name={} model={} colors={{{}}} worn={} carried={} err={:?}", b.target, b.source, b.id, b.name, b.appearance.model, colors.join(" "), worn.join(","), b.inventory.len(), b.error)
            }
        }
    }
}
