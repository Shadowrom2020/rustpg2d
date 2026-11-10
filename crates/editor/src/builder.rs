//! The character builder: assembles a layered character visually - a walking preview beside
//! rows of clickable thumbnails for the body, skin, hair, beard, clothing, armor and held items,
//! plus the carried items. Used inline by the Characters form (project and library), as a
//! window for new characters and for a placed NPC's own look (`Builder`), and by the Project
//! tab's creator settings.

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};
use rustpg2d_engine::defs::{Appearance, CharacterDef, Database, EquipSlot, SheetKind};
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::inventory::ItemStack;
use rustpg2d_engine::render::character::{self, Layer};
use rustpg2d_engine::render::Assets;
use rustpg2d_engine::state::Equipment;

use crate::app::Editor;
use crate::ui::ph;
use crate::widgets::{self, Textures, HAIR_PRESETS, SKIN_PRESETS};

/// What the builder edits.
pub struct Parts<'a> {
    pub appearance: &'a mut Appearance,
    pub equipment: &'a mut Equipment,
    pub inventory: &'a mut Vec<ItemStack>,
}

/// What the builder paints with.
pub struct Cx<'a> {
    pub db: &'a Database,
    pub tex: &'a Textures,
    pub assets: &'a Assets,
}

thread_local! {
    /// A body sheet's playable flag changed in a builder (applied by `apply_builder_requests`;
    /// the builder only sees a read-only copy of the definitions).
    static PLAYABLE: std::cell::RefCell<Option<(String, bool)>> = const { std::cell::RefCell::new(None) };
}

/// Asks the editor to set body sheet `sheet` playable (or not).
pub fn request_playable(sheet: &str, on: bool) {
    PLAYABLE.with(|r| *r.borrow_mut() = Some((sheet.to_string(), on)));
}

/// Cards of the body sheets `ids` ("" = `none`), each shown on `base`; playable bodies are
/// marked with a controller icon. Returns the clicked id.
pub fn body_cards(ui: &mut Ui, cx: &Cx, ids: &[String], none: Option<&str>, base: &Appearance, current: &str) -> Option<String> {
    let mut entries: Vec<(String, String)> = none.map(|n| (String::new(), n.to_string())).into_iter().collect();
    for id in ids {
        let label = cx.db.sheet(id).map_or(id.clone(), |s| if s.playable { format!("{} {}", ph::GAME_CONTROLLER, s.label()) } else { s.label() });
        entries.push((id.clone(), label));
    }
    let eq = Equipment::default();
    cards(ui, THUMB, &entries, Some(&current.to_string()), &|ui, id, r| {
        let a = Appearance { body: id.clone(), ..base.clone() };
        !id.is_empty() && paint(ui, cx, &character::layers(cx.db, &a, &eq, Facing::Down, 0, false), r)
    })
}

/// What happened in the builder this frame.
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

/// Color swatches (presets, framed when current) plus a color button for any other color.
pub fn swatches(ui: &mut Ui, c: &mut [u8; 3], presets: &[[u8; 3]]) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for p in presets {
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            ui.painter().rect_filled(r, 3.0, Color32::from_rgb(p[0], p[1], p[2]));
            if p == c {
                ui.painter().rect_stroke(r, 3.0, Stroke::new(2.0_f32, Color32::WHITE), StrokeKind::Inside);
            } else if resp.hovered() {
                ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
            }
            if resp.on_hover_text(format!("#{:02x}{:02x}{:02x}", p[0], p[1], p[2])).clicked() {
                *c = *p;
                changed = true;
            }
        }
        changed |= ui.color_edit_button_srgb(c).on_hover_text("Any other color").changed();
    });
    changed
}

/// Sheets of a kind as card entries: (id, label).
fn sheets_of(db: &Database, kind: SheetKind, none: Option<&str>) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = none.map(|n| (String::new(), n.to_string())).into_iter().collect();
    v.extend(db.sheets.iter().filter(|s| s.kind == kind).map(|s| (s.id.clone(), s.label())));
    v
}

/// Items that fit `slot` as card entries: (id, name).
fn items_of(db: &Database, slot: EquipSlot) -> Vec<(String, String)> {
    let mut v = vec![(String::new(), "None".to_string())];
    v.extend(db.items.iter().filter(|d| d.equip_slot() == Some(slot)).map(|d| (d.id.clone(), d.name.clone())));
    v
}

/// Paints a character (its layers) into `rect`.
fn paint(ui: &Ui, cx: &Cx, layers: &[Layer], rect: Rect) -> bool {
    widgets::paint_character(ui, cx.tex, cx.assets, layers, rect)
}

/// Thumbnail cards of sheets of `kind` (plus `none`), each shown on `base` with
/// `apply(appearance, id)` applied; `current` is framed.
pub fn sheet_cards(ui: &mut Ui, cx: &Cx, kind: SheetKind, none: Option<&str>, base: (&Appearance, &Equipment), current: &str, apply: &dyn Fn(&mut Appearance, &str)) -> Option<String> {
    let entries = sheets_of(cx.db, kind, none);
    cards(ui, THUMB, &entries, Some(&current.to_string()), &|ui, id, r| {
        let mut a = base.0.clone();
        apply(&mut a, id);
        paint(ui, cx, &character::layers(cx.db, &a, base.1, Facing::Down, 0, false), r)
    })
}

/// Thumbnail cards of the items for `slot`: clothing is shown worn, other items by their icon.
fn item_cards(ui: &mut Ui, cx: &Cx, slot: EquipSlot, a: &Appearance, eq: &Equipment) -> Option<String> {
    let entries = items_of(cx.db, slot);
    let current = eq.item(slot).unwrap_or("").to_string();
    let worn = slot == EquipSlot::Clothing;
    cards(ui, if worn { THUMB } else { ICON }, &entries, Some(&current), &|ui, id, r| {
        if worn {
            let mut e = eq.clone();
            *e.slot_mut(slot) = (!id.is_empty()).then(|| ItemStack::new(id, 1));
            paint(ui, cx, &character::layers(cx.db, a, &e, Facing::Down, 0, false), r)
        } else if id.is_empty() {
            false
        } else {
            cx.db.item(id).is_some_and(|d| widgets::paint_sprite(ui, cx.tex, cx.assets, &d.icon, r))
        }
    })
}

/// The animated preview: walking in place, facing `facing` (or turning), the four idle
/// directions below.
fn preview(ui: &mut Ui, id: egui::Id, cx: &Cx, a: &Appearance, eq: &Equipment) {
    let mut st = ui.ctx().data_mut(|d| *d.get_temp_mut_or(id, Preview { facing: Facing::Down, turning: true }));
    let t = macroquad::time::get_time();
    let order = [Facing::Down, Facing::Left, Facing::Up, Facing::Right];
    let facing = if st.turning { order[(t / 1.5) as usize % 4] } else { st.facing };
    let col = (t * 7.0) as u32 % character::LAYOUT_FRAMES;
    // (World size: a high-resolution body shows as big as a 1x one.)
    let (fw, fh) = cx.assets.sheets.get(&a.body).map_or((32.0, 48.0), |s| (s.world_size().x, s.world_size().y));
    let scale = (PREVIEW_H / fh).floor().max(1.0);
    ui.vertical(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2((fw * scale).max(120.0), PREVIEW_H + 6.0), Sense::hover());
        ui.painter().rect_filled(r, 4.0, Color32::from_gray(34));
        let dest = Rect::from_min_size(pos2(r.center().x - fw * scale / 2.0, r.max.y - 3.0 - fh * scale), vec2(fw * scale, fh * scale));
        if !paint(ui, cx, &character::layers(cx.db, a, eq, facing, col, false), dest) {
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "no body", egui::FontId::default(), Color32::LIGHT_RED);
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
            for f in order {
                let (r, _) = ui.allocate_exact_size(vec2(fw * 1.0 + 2.0, fh * 1.0 + 2.0), Sense::hover());
                paint(ui, cx, &character::layers(cx.db, a, eq, f, 0, false), r.shrink(1.0));
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, st));
    ui.ctx().request_repaint();
}

/// The builder: preview on the left, the choices on the right. `id` keeps its state apart.
pub fn builder_ui(ui: &mut Ui, id: &str, p: Parts, cx: &Cx) -> Out {
    let mut out = Out::default();
    let Parts { appearance: a, equipment: eq, inventory } = p;
    let female = cx.db.female_body(&a.body);
    ui.horizontal_top(|ui| {
        preview(ui, egui::Id::new(("builder_preview", id)), cx, a, eq);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.set_min_width(ui.available_width().min(560.0));
            ui.horizontal(|ui| {
                if ui.button(crate::ui::t(ph::DICE_FIVE, "Randomize")).on_hover_text("Random hair, colors, beard and clothing for this body").clicked() {
                    randomize(a, eq, cx.db);
                    out.changed = true;
                }
                if ui.button(crate::ui::t(ph::USERS, "Start from…")).on_hover_text("Copy the look and equipment of a character of this project or the asset library").clicked() {
                    out.template = true;
                }
            });
            let section = |ui: &mut Ui, title: &str, open: bool, body: &mut dyn FnMut(&mut Ui)| {
                egui::CollapsingHeader::new(title).id_salt((id, title)).default_open(open).show(ui, |ui| body(ui));
            };
            section(ui, "Body", true, &mut |ui| {
                let ids: Vec<String> = cx.db.sheets.iter().filter(|s| s.kind == SheetKind::Body).map(|s| s.id.clone()).collect();
                if let Some(b) = body_cards(ui, cx, &ids, None, a, &a.body) {
                    a.body = b;
                    out.changed = true;
                }
                if let Some(s) = cx.db.sheet(&a.body).filter(|s| s.kind == SheetKind::Body) {
                    let mut on = s.playable;
                    let label = crate::ui::t(ph::GAME_CONTROLLER, &format!("Playable: players may choose the {} body in the character creator", s.label().to_lowercase()));
                    if ui.checkbox(&mut on, label).on_hover_text("For the whole project (a slime or a ghost usually isn't). The creator offers the playable bodies of the chosen sex.").changed() {
                        request_playable(&s.id, on);
                    }
                }
                if cx.db.sheets.iter().all(|s| s.kind != SheetKind::Body) {
                    ui.colored_label(Color32::YELLOW, "No body sheets in this project: add some from the asset library (sprite sheets of kind \"Character body\").");
                }
                ui.label("Skin");
                out.changed |= swatches(ui, &mut a.skin, &SKIN_PRESETS);
            });
            section(ui, "Hair", true, &mut |ui| {
                let eq_bare = Equipment::default();
                let base = Appearance { beard: String::new(), ..a.clone() };
                if let Some(h) = sheet_cards(ui, cx, SheetKind::Hair, Some("No hair"), (&base, &eq_bare), &a.hair, &|a, id| a.hair = id.to_string()) {
                    a.hair = h;
                    out.changed = true;
                }
                ui.label("Hair color");
                out.changed |= swatches(ui, &mut a.hair_color, &HAIR_PRESETS);
            });
            if cx.db.sheets.iter().any(|s| s.kind == SheetKind::Beard) {
                section(ui, "Beard", !female, &mut |ui| {
                    let eq_bare = Equipment::default();
                    if let Some(b) = sheet_cards(ui, cx, SheetKind::Beard, Some("No beard"), (a, &eq_bare), &a.beard, &|a, id| a.beard = id.to_string()) {
                        a.beard = b;
                        out.changed = true;
                    }
                });
            }
            section(ui, "Clothing", true, &mut |ui| {
                ui.weak(if female { "Outfits are shown in their female cut on this body." } else { "Armor is worn over the clothing." });
                if let Some(c) = item_cards(ui, cx, EquipSlot::Clothing, a, eq) {
                    *eq.slot_mut(EquipSlot::Clothing) = (!c.is_empty()).then(|| ItemStack::new(&c, 1));
                    out.changed = true;
                }
            });
            section(ui, "Armor & hands", !eq.is_empty(), &mut |ui| {
                for slot in [EquipSlot::Head, EquipSlot::Body, EquipSlot::Weapon, EquipSlot::Offhand] {
                    ui.label(match slot {
                        EquipSlot::Body => "Body armor",
                        EquipSlot::Weapon => "Weapon (right hand)",
                        s => s.label(),
                    });
                    if let Some(c) = item_cards(ui, cx, slot, a, eq) {
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

/// Random hair, colors, beard and clothing that fit the body (the body itself stays).
pub fn randomize(a: &mut Appearance, eq: &mut Equipment, db: &Database) {
    use macroquad::rand::{gen_range, srand};
    srand((macroquad::time::get_time() * 1000.0) as u64 ^ a.body.len() as u64);
    let pick = |ids: &[String]| ids[gen_range(0, ids.len())].clone();
    let goblin = a.body.contains("goblin");
    let female = db.female_body(&a.body);
    let skins: Vec<[u8; 3]> = if goblin { vec![[0x7c, 0xb3, 0x42], [0x8b, 0xc3, 0x4a], [0x5a, 0x8a, 0x30]] } else { SKIN_PRESETS[..7].to_vec() };
    a.skin = skins[gen_range(0, skins.len())];
    a.hair_color = HAIR_PRESETS[gen_range(0, HAIR_PRESETS.len())];
    let hairs: Vec<String> = db.sheets.iter().filter(|s| s.kind == SheetKind::Hair).map(|s| s.id.clone()).collect();
    if !hairs.is_empty() {
        a.hair = if gen_range(0, 10) == 0 { String::new() } else { pick(&hairs) };
    }
    let beards: Vec<String> = db.sheets.iter().filter(|s| s.kind == SheetKind::Beard).map(|s| s.id.clone()).collect();
    a.beard = if female || beards.is_empty() || gen_range(0, 10) < 6 { String::new() } else { pick(&beards) };
    let clothes: Vec<String> = db.items.iter().filter(|d| d.equip_slot() == Some(EquipSlot::Clothing)).map(|d| d.id.clone()).collect();
    if !clothes.is_empty() {
        *eq.slot_mut(EquipSlot::Clothing) = Some(ItemStack::new(&pick(&clothes), 1));
    }
}

/// A fresh character: the first person body (a `body_…` sheet, else any body), hair and
/// clothing of the project.
pub fn default_character(db: &Database, id: &str, name: &str) -> CharacterDef {
    let first = |k: SheetKind| {
        let of_kind = || db.sheets.iter().filter(|s| s.kind == k);
        of_kind().find(|s| s.id.starts_with("body_")).or_else(|| of_kind().next()).map(|s| s.id.clone()).unwrap_or_default()
    };
    let mut equipment = Equipment::default();
    if let Some(outfit) = db.items.iter().find(|d| d.equip_slot() == Some(EquipSlot::Clothing)) {
        equipment.clothing = Some(ItemStack::new(&outfit.id, 1));
    }
    let u = rustpg2d_engine::project::TILE_SIZE as f32 / 16.0;
    CharacterDef {
        name: name.into(),
        speed: 50.0 * u,
        hitbox: [10.0 * u, 6.0 * u],
        appearance: Appearance { body: first(SheetKind::Body), hair: first(SheetKind::Hair), ..Default::default() },
        equipment,
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
}

impl Editor {
    /// Applies a playable flag changed in a builder (see [`request_playable`]).
    pub fn apply_builder_requests(&mut self) {
        let Some((sheet, on)) = PLAYABLE.with(|r| r.borrow_mut().take()) else { return };
        if let Some(s) = self.project.db.sheets.iter_mut().find(|s| s.id == sheet) {
            s.playable = on;
            self.defs_dirty = true;
            let what = if on { "can now be chosen" } else { "can no longer be chosen" };
            self.set_status(format!("Body '{sheet}' {what} in the character creator. Save the definitions to keep it."));
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
        self.builder = Some(Builder { target, id, name, appearance: d.appearance, equipment: d.equipment, inventory: d.inventory, error: None });
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
                if b.appearance.body.is_empty() {
                    self.builder = Some(b);
                    anyhow::bail!("Choose a body");
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
        egui::Window::new(title).id(egui::Id::new("builder")).open(&mut open).collapsible(false).default_size(vec2(820.0, 600.0)).show(ctx, |ui| {
            if b.target == Target::New {
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
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                let cx = Cx { db: &self.project.db, tex: &self.tex, assets: &self.assets };
                let out = builder_ui(ui, "builder_window", Parts { appearance: &mut b.appearance, equipment: &mut b.equipment, inventory: &mut b.inventory }, &cx);
                template |= out.template;
            });
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
        let rgb = |v: &str| u32::from_str_radix(v, 16).ok().map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
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
                _ => {}
            }
        }
        let db = self.project.db.clone();
        let Some(p) = self.builder_parts() else { return Err("no builder open and no character selected".into()) };
        match key {
            "body" => p.appearance.body = value.to_string(),
            "hair" => p.appearance.hair = value.to_string(),
            "beard" => p.appearance.beard = value.to_string(),
            "skin" => p.appearance.skin = rgb(value).ok_or("skin: RRGGBB expected")?,
            "haircolor" => p.appearance.hair_color = rgb(value).ok_or("haircolor: RRGGBB expected")?,
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
            "random" => {
                let (a, eq) = (p.appearance, p.equipment);
                randomize(a, eq, &db);
            }
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
                format!("{:?} id={} name={} body={} skin={:?} hair={} {:?} beard={} worn={} carried={} err={:?}", b.target, b.id, b.name, b.appearance.body, b.appearance.skin, b.appearance.hair, b.appearance.hair_color, b.appearance.beard, worn.join(","), b.inventory.len(), b.error)
            }
        }
    }
}
