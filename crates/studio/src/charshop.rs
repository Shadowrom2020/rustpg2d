//! The character workshop, at the top of a character's tab: the figure large as the game
//! draws it (with what it wears and holds), turning or seen from a side, walking, standing,
//! striking; beside it its look as things to pick: the model from cards, the pieces that
//! model can wear, the colour of each of its features from swatches.
//!
//! What a look can be is worked out in `editor_core::looks`.

use eframe::egui::{self, Color32, Sense, Stroke};
use eframe::egui_wgpu::RenderState;
use editor_core::looks;
use editor_core::Workspace;
use rustpg2d_engine::defs::{Appearance, ColorFeature, EquipSlot, NO_PART};
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::state::Equipment;
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::render::character::{Anim, LAYOUT_FRAMES};
use rustpg2d_engine::render::model::{anim_req, Gear, PoseReq};

/// The preview's height.
const PREVIEW: egui::Vec2 = egui::vec2(220.0, 250.0);
const ANIMS: [(&str, &str); 4] = [("Walk", "The walk cycle"), ("Idle", "Standing still"), ("Attack", "Wind-up, swing, follow-through, recover"), ("Hurt", "Just hit")];
const SIDES: [(Facing, &str); 4] = [(Facing::Down, "Front"), (Facing::Left, "Left"), (Facing::Up, "Back"), (Facing::Right, "Right")];

/// The slots whose items show on the figure, and the others (worn magic items, a tool, ammunition).
const SHOWN: [EquipSlot; 6] = [EquipSlot::Weapon, EquipSlot::Offhand, EquipSlot::Head, EquipSlot::Body, EquipSlot::Clothing, EquipSlot::Back];
const MORE: [EquipSlot; 9] = [EquipSlot::Tool, EquipSlot::Ammo, EquipSlot::Neck, EquipSlot::Cloak, EquipSlot::Belt, EquipSlot::Feet, EquipSlot::Hands, EquipSlot::Ring, EquipSlot::Ring2];

pub struct CharStage {
    pub facing: Facing,
    /// Turning around, a side every second and a half.
    pub turning: bool,
    /// 0 walk, 1 idle, 2 attack, 3 hurt.
    pub anim: u8,
    /// The model cards are shown (else the current model's name with *Change…*).
    pub choosing: bool,
    search: String,
    /// The slot whose items are offered under the slots.
    pub slot: Option<EquipSlot>,
    item_search: String,
    /// The slots of worn magic items, tool and ammunition are shown too.
    pub more: bool,
}

impl Default for CharStage {
    fn default() -> CharStage {
        CharStage { facing: Facing::Down, turning: true, anim: 1, choosing: false, search: String::new(), slot: None, item_search: String::new(), more: false }
    }
}

/// The animation frame of preview mode `anim` at time `t` (an attack plays its four frames, then rests).
fn preview_anim(anim: u8, t: f64) -> Anim {
    match anim {
        1 => Anim::Idle(0),
        2 => {
            let k = (t * 6.0) as u32 % 7;
            if k < 4 {
                Anim::Attack(k)
            } else {
                Anim::Idle(0)
            }
        }
        3 => Anim::Hurt,
        _ => Anim::Walk((t * 7.0) as u32 % LAYOUT_FRAMES),
    }
}

fn swatch(ui: &mut egui::Ui, c: [u8; 3], on: bool, name: String) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.as_str()));
    ui.painter().rect_filled(r, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
    if on {
        ui.painter().rect_stroke(r, 3.0, Stroke::new(2.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
    }
    resp.on_hover_text(format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
}

/// The colour of one feature: as drawn, one of the feature's offered colours, any other.
fn feature_colors(ui: &mut egui::Ui, f: &ColorFeature, a: &mut Appearance) -> bool {
    let mut changed = false;
    let current = a.colors.get(&f.name).copied();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        ui.label(f.label());
        let drawn = ui.selectable_label(current.is_none(), "As drawn").on_hover_text(if f.default.is_some() { "The model's own colour" } else { "The colours it was painted in" });
        drawn.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current.is_none(), format!("{}: as drawn", f.label())));
        if drawn.clicked() && current.is_some() {
            a.colors.remove(&f.name);
            changed = true;
        }
        for (k, c) in f.colors.iter().enumerate() {
            if swatch(ui, *c, current == Some(*c), format!("{}: colour {}", f.label(), k + 1)).clicked() {
                a.colors.insert(f.name.clone(), *c);
                changed = true;
            }
        }
        let mut any = current.unwrap_or_else(|| f.shades.get(f.shades.len() / 2).copied().unwrap_or([128, 128, 128]));
        if ui.color_edit_button_srgb(&mut any).on_hover_text("Any other colour (for this character only: the character creator offers the feature's colours)").changed() {
            a.colors.insert(f.name.clone(), any);
            changed = true;
        }
    });
    changed
}

impl CharStage {
    /// The script's line: `ID model= facing=|turning anim= parts=slot:piece… colors=feature:rrggbb…`.
    pub fn status(&self, ws: &Workspace, id: &str) -> String {
        let Some(c) = ws.project.db.character(id) else { return "-".into() };
        let a = &c.appearance;
        let parts: Vec<String> = a.parts.iter().map(|(s, p)| format!("{s}:{p}")).collect();
        let colors: Vec<String> = a.colors.iter().map(|(f, c)| format!("{f}:{:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect();
        let side = if self.turning { "turning".to_string() } else { format!("{:?}", self.facing) };
        let wears: Vec<String> = EquipSlot::ALL.iter().filter_map(|slot| c.equipment.item(*slot).map(|i| format!("{}:{i}", slot.label().to_lowercase().replace(' ', "_")))).collect();
        format!("{id} model={} facing={side} anim={} parts=[{}] colors=[{}] wears=[{}] slots={} features={}", if a.model.is_empty() { "-" } else { &a.model }, ANIMS[self.anim.min(3) as usize].0.to_lowercase(), parts.join(" "), colors.join(" "), wears.join(" "), looks::piece_slots(&ws.project.db, &a.model).len(), looks::look_features(&ws.project.db, a).len())
    }

    /// The script's words: `model ID`, `part SLOT ID|-|own`, `color FEATURE RRGGBB|-`, `random`,
    /// `wear SLOT ITEM|-`, `face front|left|back|right|turn`, `anim walk|idle|attack|hurt`.
    /// Returns why not, if not.
    pub fn word(&mut self, ws: &mut Workspace, id: &str, args: &[String]) -> Result<(), String> {
        let arg = |k: usize| args.get(k).map(String::as_str).unwrap_or("");
        let Some(pos) = ws.project.db.characters.iter().position(|c| c.id == id) else { return Err(format!("there is no character {id}")) };
        let mut a = ws.project.db.characters[pos].appearance.clone();
        let mut eq = ws.project.db.characters[pos].equipment.clone();
        match arg(0) {
            "" => return Ok(()),
            "wear" => {
                let slot = EquipSlot::parse(arg(1)).ok_or_else(|| format!("{} is no slot (weapon, offhand, head, body, clothing, back…)", arg(1)))?;
                match arg(2) {
                    "-" => looks::wear(&mut eq, slot, None),
                    item if looks::items_for(&ws.project.db, slot).iter().any(|(i, _)| i == item) => looks::wear(&mut eq, slot, Some(item)),
                    item => return Err(format!("{item} doesn't go there (these do: {})", looks::items_for(&ws.project.db, slot).iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(", "))),
                }
            }
            "face" => {
                match SIDES.iter().find(|(_, l)| l.eq_ignore_ascii_case(arg(1))) {
                    Some((f, _)) => (self.facing, self.turning) = (*f, false),
                    None => self.turning = true,
                }
                return Ok(());
            }
            "anim" => {
                self.anim = ANIMS.iter().position(|(l, _)| l.eq_ignore_ascii_case(arg(1))).ok_or("anim walk|idle|attack|hurt")? as u8;
                return Ok(());
            }
            "model" => {
                if !ws.project.db.has(rustpg2d_engine::refactor::RefKind::Model, arg(1)) {
                    return Err(format!("there is no model {}", arg(1)));
                }
                looks::set_model(&mut a, arg(1));
            }
            "part" => {
                let slots = looks::piece_slots(&ws.project.db, &a.model);
                let slot = slots.iter().find(|s| s.slot == arg(1)).ok_or_else(|| format!("this model has no slot {} (it has: {})", arg(1), slots.iter().map(|s| s.slot.as_str()).collect::<Vec<_>>().join(", ")))?;
                match arg(2) {
                    "own" => looks::set_piece(&mut a, &slot.slot, None),
                    "-" => looks::set_piece(&mut a, &slot.slot, Some(NO_PART)),
                    p if slot.pieces.iter().any(|x| x.0 == p) => looks::set_piece(&mut a, &slot.slot, Some(p)),
                    p => return Err(format!("no piece {p} fits (these do: {})", slot.pieces.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(", "))),
                }
            }
            "color" => {
                if arg(2) == "-" {
                    a.colors.remove(arg(1));
                } else {
                    let v = u32::from_str_radix(arg(2), 16).map_err(|_| "color FEATURE RRGGBB|-")?;
                    a.colors.insert(arg(1).to_string(), [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
                }
            }
            "random" => looks::random_colors(&mut a, &ws.project.db, (editor_core::now() * 1000.0) as u64),
            other => return Err(format!("char {other}: model, part, color, random, wear, face, anim")),
        }
        CharStage::put(ws, pos, a, eq);
        Ok(())
    }

    fn put(ws: &mut Workspace, pos: usize, a: Appearance, eq: Equipment) -> bool {
        let c = &ws.project.db.characters[pos];
        let (look, gear) = (c.appearance != a, c.equipment != eq);
        if !look && !gear {
            return false;
        }
        let name = if c.name.is_empty() { c.id.clone() } else { c.name.clone() };
        ws.begin(&if look { format!("Change the look of {name}") } else { format!("Change what {name} wears") });
        let c = &mut ws.project.db.characters[pos];
        (c.appearance, c.equipment) = (a, eq);
        ws.stale = true;
        true
    }

    /// The workshop. True when it changed the character.
    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str) -> bool {
        let Some(pos) = ws.project.db.characters.iter().position(|c| c.id == id) else { return false };
        let mut a = ws.project.db.characters[pos].appearance.clone();
        let mut eq = ws.project.db.characters[pos].equipment.clone();
        ui.horizontal_top(|ui| {
            self.preview(ui, ws, rs, thumbs, pos);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                self.look(ui, ws, rs, thumbs, &mut a);
                self.gear(ui, ws, rs, thumbs, &mut eq);
            });
        });
        CharStage::put(ws, pos, a, eq)
    }

    /// The figure, and how it is looked at.
    fn preview(&mut self, ui: &mut egui::Ui, ws: &Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, pos: usize) {
        let t = ui.input(|i| i.time);
        let facing = if self.turning { SIDES[(t / 1.5) as usize % 4].0 } else { self.facing };
        ui.vertical(|ui| {
            let (rect, _) = ui.allocate_exact_size(PREVIEW, Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(32, 34, 40));
            if let Some(rs) = rs {
                let c = &ws.project.db.characters[pos];
                let look = ws.assets.models.look_of(&c.appearance);
                let gear = Gear::of(&c.equipment, "", None);
                let (anim, time, once) = anim_req(preview_anim(self.anim, t), t as f32);
                let req = PoseReq { look: &look, gear: &gear, anim, time, once, facing, turn: 0, bust: false };
                // As large as the figure standing still is tall (so the size holds from pose to
                // pose), on its feet; what reaches further (a spear, a swung arm) as far as the box does.
                // (A wolf or an owlbear is longer than tall: its side has to fit too.)
                let bare = Gear::default();
                let still = |facing| PoseReq { look: &look, gear: &bare, anim: "idle", time: 0.0, once: true, facing, turn: 0, bust: false };
                let (front, side) = (thumbs.pose(ws, rs, &still(Facing::Down)), thumbs.pose(ws, rs, &still(Facing::Left)));
                // (It stands on its feet in the middle: a head or a tail reaching far to one
                // side needs that much room on both.)
                let size = front.zip(side).map(|(f, s)| (f.height.max(s.height), f.reach.max(s.reach)));
                match (thumbs.pose(ws, rs, &req), size) {
                    (Some(pose), Some((tall, reach))) => {
                        let k = ((rect.height() - 36.0) / tall).min((rect.width() / 2.0 - 12.0) / reach).min(6.0);
                        let stands = egui::pos2(rect.center().x, rect.max.y - 16.0);
                        let whole = egui::Rect::from_min_size(stands - pose.feet.to_vec2() * k, pose.thumb.size * k);
                        ui.painter_at(rect).image(pose.thumb.texture, whole, pose.thumb.uv, Color32::WHITE);
                    }
                    _ => {
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "drawing…", egui::FontId::default(), Color32::GRAY);
                    }
                }
                // (It moves: the next frame is wanted while it is in view.)
                if ui.is_rect_visible(rect) {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                }
            }
            ui.horizontal(|ui| {
                for (f, label) in SIDES {
                    if ui.selectable_label(!self.turning && self.facing == f, label).on_hover_text("Seen from this side").clicked() {
                        (self.facing, self.turning) = (f, false);
                    }
                }
                ui.toggle_value(&mut self.turning, "Turning").on_hover_text("Turns around, a side at a time");
            });
            ui.horizontal(|ui| {
                for (k, (label, tip)) in ANIMS.iter().enumerate() {
                    if ui.selectable_label(self.anim == k as u8, *label).on_hover_text(*tip).clicked() {
                        self.anim = k as u8;
                    }
                }
            });
        });
    }

    /// What it wears and holds: a box per slot with the item's picture; the one clicked offers
    /// the items that go there.
    fn gear(&mut self, ui: &mut egui::Ui, ws: &Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, eq: &mut Equipment) {
        let db = &ws.project.db;
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.strong("Wears and holds");
            ui.weak("Counts in play (damage, armor, what it drops). A weapon and what is in the left hand show in the hands, a backpack on the back; armor, headgear and clothing show when the item names a style this model has a piece of.");
        });
        ui.horizontal_wrapped(|ui| {
            let slots: Vec<EquipSlot> = SHOWN.iter().copied().chain(MORE.iter().copied().filter(|s| self.more || eq.item(*s).is_some())).collect();
            for slot in slots {
                let item = eq.item(slot).map(str::to_string);
                let name = item.as_deref().map(|i| db.item(i).map_or(i.to_string(), |d| if d.name.is_empty() { i.to_string() } else { d.name.clone() }));
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(72.0, 70.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{}: {}", slot.label(), name.as_deref().unwrap_or("nothing"))));
                let open = self.slot == Some(slot);
                let fill = if open { ui.visuals().selection.bg_fill.gamma_multiply(0.45) } else if resp.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().faint_bg_color };
                ui.painter().rect_filled(rect, 4.0, fill);
                let picture = egui::Rect::from_min_size(egui::pos2(rect.center().x - 20.0, rect.min.y + 4.0), egui::vec2(40.0, 40.0));
                match (&item, rs) {
                    (Some(i), Some(rs)) => {
                        if let Some(t) = thumbs.of(ws, rs, RefKind::Item, i) {
                            t.paint(ui, picture);
                        }
                    }
                    (None, _) => {
                        ui.painter().rect_stroke(picture.shrink(6.0), 3.0, Stroke::new(1.0_f32, Color32::from_gray(80)), egui::StrokeKind::Inside);
                    }
                    _ => {}
                }
                ui.painter().text(egui::pos2(rect.center().x, rect.max.y - 4.0), egui::Align2::CENTER_BOTTOM, slot.label(), egui::FontId::proportional(11.0), Color32::from_gray(if item.is_some() { 220 } else { 140 }));
                if resp.on_hover_text(match &name {
                    Some(n) => format!("{}: {n}. Click to change it.", slot.label()),
                    None => format!("{}: nothing. Click to choose.", slot.label()),
                })
                .clicked()
                {
                    self.slot = if open { None } else { Some(slot) };
                    self.item_search.clear();
                }
            }
            if ui.selectable_label(self.more, "More slots").on_hover_text("A tool, ammunition and what is worn besides: neck, cloak, belt, feet, hands, rings. They count in play and aren't drawn on the figure.").clicked() {
                self.more = !self.more;
            }
        });
        // The items that go into the slot picked.
        let Some(slot) = self.slot else { return };
        let items = looks::items_for(db, slot);
        let current = eq.item(slot).map(str::to_string);
        ui.horizontal(|ui| {
            ui.label(format!("{}:", slot.label()));
            ui.add(egui::TextEdit::singleline(&mut self.item_search).hint_text("Search the items").desired_width(200.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the items"));
            if ui.add_enabled(current.is_some(), egui::Button::new("Nothing")).on_hover_text("Takes it off").clicked() {
                looks::wear(eq, slot, None);
                self.slot = None;
            }
        });
        if items.is_empty() {
            ui.weak("This project has no item that goes there: the asset library has weapons, armor and clothing to add (New… › Item › as one of the asset library).");
        }
        let f = self.item_search.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("items for the slot").max_height(240.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (id, name) in items.iter().filter(|(id, name)| f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)) {
                    let r = crate::thumbs::card(ui, name, id, current.as_deref() == Some(id.as_str()), None, || rs.and_then(|rs| thumbs.of(ws, rs, RefKind::Item, id)));
                    if r.clicked() {
                        looks::wear(eq, slot, Some(id));
                        self.slot = None;
                    }
                }
            });
        });
    }

    /// The look: model, pieces, colours.
    fn look(&mut self, ui: &mut egui::Ui, ws: &Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, a: &mut Appearance) {
        let db = &ws.project.db;
        let figures = looks::figures(db);
        // The model: its name, and cards to choose another from.
        ui.horizontal_wrapped(|ui| {
            ui.strong("Model");
            let name = figures.iter().find(|(id, _)| *id == a.model).map_or_else(|| if a.model.is_empty() { "none (the mannequin)".to_string() } else { format!("{} (missing)", a.model) }, |(_, n)| n.clone());
            ui.label(name);
            if ui.selectable_label(self.choosing, "Change…").on_hover_text("Shows the project's figures to choose from. Another model has other colours and wears other pieces: those chosen for this one go.").clicked() {
                self.choosing = !self.choosing;
            }
            let features = looks::look_features(db, a);
            if ui.add_enabled(features.iter().any(|f| !f.colors.is_empty()), egui::Button::new("Random colours")).on_hover_text("One of the offered colours for every feature that offers some").clicked() {
                looks::random_colors(a, db, (ui.input(|i| i.time) * 1000.0) as u64);
            }
        });
        if figures.is_empty() {
            ui.colored_label(ui.visuals().warn_fg_color, "This project has no 3D models yet: add figures from the asset library. Until then characters are drawn as the mannequin.");
        }
        if self.choosing {
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search the figures").desired_width(220.0)).widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Search the figures"));
            let f = self.search.trim().to_lowercase();
            egui::ScrollArea::vertical().id_salt("figures").max_height(250.0).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (id, name) in figures.iter().filter(|(id, name)| f.is_empty() || id.to_lowercase().contains(&f) || name.to_lowercase().contains(&f)) {
                        let r = crate::thumbs::card(ui, name, id, *id == a.model, None, || rs.and_then(|rs| thumbs.figure(ws, rs, &Appearance { model: id.clone(), ..Default::default() })));
                        if r.clicked() {
                            looks::set_model(a, id);
                            self.choosing = false;
                        }
                    }
                });
            });
        }
        // The pieces this model can wear.
        let slots = looks::piece_slots(db, &a.model);
        if !slots.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.strong("Pieces");
                ui.weak("What this model can wear. Equipment that shows on the figure (an armour, a helmet, a cloak) takes its slot while it is worn.");
            });
            egui::Grid::new("pieces").num_columns(2).show(ui, |ui| {
                for s in &slots {
                    let l = ui.label(s.label());
                    let chosen = a.parts.get(&s.slot).cloned();
                    let mut pick = chosen.clone();
                    egui::ComboBox::from_id_salt(("piece", &s.slot)).selected_text(s.chosen_label(chosen.as_deref())).width(220.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut pick, None, s.own_label());
                        ui.selectable_value(&mut pick, Some(NO_PART.to_string()), "None");
                        for (pid, label) in &s.pieces {
                            ui.selectable_value(&mut pick, Some(pid.clone()), label);
                        }
                    }).response.labelled_by(l.id);
                    if pick != chosen {
                        looks::set_piece(a, &s.slot, pick.as_deref());
                    }
                    ui.end_row();
                }
            });
        }
        // The colour of each feature.
        let features = looks::look_features(db, a);
        ui.add_space(4.0);
        ui.strong("Colours");
        if features.is_empty() {
            ui.weak("This model has no colour features: its skin, hair, clothing… are marked on the model (its tab, Colour features) to recolour it here.");
        }
        for f in &features {
            feature_colors(ui, f, a);
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        stage: CharStage,
        thumbs: crate::thumbs::Thumbs,
        dir: std::path::PathBuf,
    }

    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                if e.file_name() != "saves" {
                    copy(&src, &dst);
                }
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    fn look(s: &State) -> &Appearance {
        &s.ws.project.db.character("guard").unwrap().appearance
    }

    #[test]
    fn a_characters_pieces_colours_and_model_are_picked() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-charshop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        let before = ws.project.db.character("guard").expect("the example has a guard").appearance.clone();
        let clothing = looks::look_features(&ws.project.db, &before).into_iter().find(|f| f.name == "clothing").expect("his clothing can be coloured");
        assert!(looks::piece_slots(&ws.project.db, &before.model).iter().any(|s| s.slot == "headwear"));
        let other = looks::figures(&ws.project.db).into_iter().find(|(id, _)| *id != before.model).expect("another figure");
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                s.ws.history_frame(ui.ctx().memory(|m| m.focused()).map(|id| id.value()));
                s.stage.show(ui, &mut s.ws, None, &mut s.thumbs, "guard");
            },
            State { ws, stage: CharStage::default(), thumbs: Default::default(), dir },
        );
        h.run();
        // No headwear, from the slot's list.
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Headwear").click();
        h.run();
        h.get_by_label("None").click();
        h.run();
        h.run();
        assert_eq!(look(h.state()).parts.get("headwear").map(String::as_str), Some(NO_PART));
        assert_eq!(h.state().ws.timeline.done.last().unwrap().label, "Change the look of Town Guard");
        // His clothing in the second colour it offers, then as drawn again.
        h.get_by_label("Clothing: colour 2").click();
        h.run();
        h.run();
        assert_eq!(look(h.state()).colors.get("clothing"), Some(&clothing.colors[1]));
        h.get_by_label("Clothing: as drawn").click();
        h.run();
        h.run();
        assert!(!look(h.state()).colors.contains_key("clothing"));
        // Seen from the left, striking.
        h.get_by_label("Left").click();
        h.get_by_label("Attack").click();
        h.run();
        assert!(h.state().stage.facing == Facing::Left && !h.state().stage.turning && h.state().stage.anim == 2);
        // What he holds: the slot says so; clicked, it offers the weapons, searched; one picked
        // is in his hand, as a step called what it is.
        let had = h.state().ws.project.db.character("guard").unwrap().equipment.item(EquipSlot::Weapon).map(str::to_string).expect("the guard holds a weapon");
        let other_weapon = looks::items_for(&h.state().ws.project.db, EquipSlot::Weapon).into_iter().find(|(id, _)| *id != had).expect("another weapon");
        let held = h.state().ws.project.db.item(&had).unwrap().name.clone();
        h.get_by_label(&format!("Weapon: {held}")).click();
        h.run();
        assert_eq!(h.state().stage.slot, Some(EquipSlot::Weapon));
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the items");
        search.focus();
        search.type_text(&other_weapon.0);
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::Button, other_weapon.1.as_str()).click();
        h.run();
        h.run();
        let s = h.state();
        assert_eq!(s.ws.project.db.character("guard").unwrap().equipment.item(EquipSlot::Weapon), Some(other_weapon.0.as_str()));
        assert_eq!(s.ws.timeline.done.last().unwrap().label, "Change what Town Guard wears");
        assert!(s.stage.slot.is_none());
        // Nothing in the hand; and undone twice he holds what he held.
        h.get_by_label(&format!("Weapon: {}", other_weapon.1)).click();
        h.run();
        h.get_by_label("Nothing").click();
        h.run();
        h.run();
        assert!(h.state().ws.project.db.character("guard").unwrap().equipment.item(EquipSlot::Weapon).is_none());
        h.get_by_label("Weapon: nothing");
        h.state_mut().ws.undo().unwrap();
        h.state_mut().ws.undo().unwrap();
        h.run();
        assert_eq!(h.state().ws.project.db.character("guard").unwrap().equipment.item(EquipSlot::Weapon), Some(had.as_str()));
        // Another model from the cards: what was chosen for the old one goes.
        assert!(h.query_all_by_label(other.1.as_str()).next().is_none(), "the cards are shown on request");
        h.get_by_label("Change…").click();
        h.run();
        let search = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search the figures");
        search.focus();
        search.type_text(&other.0);
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::Button, other.1.as_str()).click();
        h.run();
        h.run();
        let s = h.state();
        assert_eq!(look(s).model, other.0);
        assert!(look(s).parts.is_empty() && look(s).colors.is_empty() && !s.stage.choosing);
        // Every step undone, he looks as he did.
        while h.state().ws.timeline.done.last().is_some_and(|c| c.label == "Change the look of Town Guard") {
            h.state_mut().ws.undo().unwrap();
        }
        assert_eq!(look(h.state()), &before);
        let dir = h.state().dir.clone();
        let _ = std::fs::remove_dir_all(dir);
    }
}
