//! Reusable egui widgets: textures, sprite pickers and the trigger editor.

use std::collections::HashMap;

use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui};
use rustpg2d_engine::defs::{ContainerSpec, LockSpec, LootEntry, OnUnlock, PropDef, ShopEntry, ShopSpec, SpriteRef};
use rustpg2d_engine::effects::{EffectKind, EffectSpec};
use rustpg2d_engine::inventory::ItemStack;
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::events::{Action, CmpOp, Cond, Condition, Trigger, TriggerOn, TriggerOwner, VarOp};
use rustpg2d_engine::render::Assets;

/// egui copies of tileset and sprite sheet images.
#[derive(Default)]
pub struct Textures {
    pub tilesets: HashMap<String, TextureHandle>,
    pub sheets: HashMap<String, TextureHandle>,
    /// The looks of the character definitions as drawn (their models; painted for `@char:`
    /// sprites).
    pub characters: HashMap<String, rustpg2d_engine::defs::Appearance>,
}

/// An egui image of RGBA pixels, shrunk by a whole factor (nearest neighbour) when a side is
/// larger than the GPU allows (egui panics on such textures). Callers compute UVs from the
/// original image size, so a shrunk texture still maps right, just less sharp.
/// An activity (fishing, hammering...) chooser with its tool: "Nothing" = none.
pub fn activity_ui(ui: &mut Ui, id: &str, act: &mut Option<rustpg2d_engine::entity::Activity>, items: &[String]) {
    use rustpg2d_engine::entity::{Activity, ActivityKind};
    let shown = act.as_ref().map_or("nothing", |a| a.kind.label());
    egui::ComboBox::from_id_salt((id, "kind")).width(96.0).selected_text(shown).show_ui(ui, |ui| {
        if ui.selectable_label(act.is_none(), "nothing").clicked() {
            *act = None;
        }
        for k in ActivityKind::ALL {
            if ui.selectable_label(act.as_ref().is_some_and(|a| a.kind == k), k.label()).clicked() {
                let tool = act.as_ref().map(|a| a.tool.clone()).unwrap_or_default();
                *act = Some(Activity { kind: k, tool });
            }
        }
    })
    .response
    .on_hover_text("Seen doing it there: it stays put facing its way and works in place (fishing line, sparks, soil...). Only the look: nothing is produced.");
    if let Some(a) = act {
        // The tool it works with, e.g. fishing_rod; empty = none.
        suggest(ui, &format!("{id}/tool"), &mut a.tool, items, 76.0);
    }
}

pub fn color_image(ctx: &egui::Context, w: usize, h: usize, px: &[u8]) -> egui::ColorImage {
    let max = ctx.input(|i| i.max_texture_side).max(256);
    let k = w.max(h).div_ceil(max).max(1);
    if k == 1 {
        return egui::ColorImage::from_rgba_unmultiplied([w, h], px);
    }
    let (nw, nh) = (w.div_ceil(k), h.div_ceil(k));
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = ((x * k).min(w - 1), (y * k).min(h - 1));
            let (s, d) = ((sy * w + sx) * 4, (y * nw + x) * 4);
            out[d..d + 4].copy_from_slice(&px[s..s + 4]);
        }
    }
    eprintln!("[editor] an image of {w}x{h} pixels is larger than the graphics card allows; shown at 1/{k} size");
    egui::ColorImage::from_rgba_unmultiplied([nw, nh], &out)
}

impl Textures {
    pub fn build(ctx: &egui::Context, assets: &Assets) -> Self {
        let load = |name: &str, img: &macroquad::texture::Image| {
            let ci = color_image(ctx, img.width as usize, img.height as usize, &img.bytes);
            ctx.load_texture(name, ci, egui::TextureOptions::NEAREST)
        };
        Textures {
            tilesets: assets.tilesets.iter().map(|(id, t)| (id.clone(), load(&format!("ts:{id}"), &t.image))).collect(),
            sheets: assets.sheets.iter().map(|(id, s)| (id.clone(), load(&format!("sh:{id}"), &s.image))).collect(),
            characters: HashMap::new(),
        }
    }

    /// Refreshes the looks of the character definitions (cheap; every frame, so previews follow
    /// edits at once).
    pub fn set_characters(&mut self, db: &rustpg2d_engine::defs::Database, assets: &Assets) {
        self.characters.clear();
        for d in &db.characters {
            self.characters.insert(d.id.clone(), assets.models.look_of(&d.appearance));
        }
    }
}

/// Paints a character (`a`, resolved with `Models::look_of`) standing facing `facing` into
/// `rect`, bottom-centred. False when it isn't rendered (yet).
pub fn paint_character(ui: &Ui, assets: &Assets, a: &rustpg2d_engine::defs::Appearance, facing: Facing, rect: Rect) -> bool {
    let gear = rustpg2d_engine::render::model::Gear::default();
    let req = rustpg2d_engine::render::model::PoseReq { look: a, gear: &gear, anim: "idle", time: 0.0, once: true, facing, turn: 0, bust: false };
    paint_pose(ui, &assets.models, empty_db(), &assets.sheets, &req, rect)
}

/// An empty database (poses without gear need none).
pub fn empty_db() -> &'static rustpg2d_engine::defs::Database {
    static DB: std::sync::OnceLock<rustpg2d_engine::defs::Database> = std::sync::OnceLock::new();
    DB.get_or_init(Default::default)
}

/// Sheet name prefix of a character's thumbnail sprite (see [`character_sprite`]).
pub const CHAR_PREFIX: &str = "@char:";

/// The thumbnail sprite of a character definition: a `@char:` marker that [`paint_sprite`]
/// paints from its layers.
pub fn character_sprite(d: &rustpg2d_engine::defs::CharacterDef) -> SpriteRef {
    SpriteRef::new(&format!("{CHAR_PREFIX}{}", d.id), 0)
}

/// Paints one frame of a sheet into `rect`.
pub fn paint_frame(ui: &Ui, tex: &Textures, assets: &Assets, sheet: &str, frame: u32, rect: Rect) -> bool {
    paint_sprite(ui, tex, assets, &SpriteRef::new(sheet, frame), rect)
}

/// Paints a (possibly multi-cell) sprite into `rect`, keeping its aspect ratio, bottom-centered.
/// Paints a pose of a 3D model character into `rect` (bottom-centred, its aspect kept): the
/// engine renders it into its atlas, egui shows that texture directly. False without a model.
pub fn paint_model(ui: &Ui, assets: &Assets, db: &rustpg2d_engine::defs::Database, req: &rustpg2d_engine::render::model::PoseReq, rect: Rect) -> bool {
    paint_pose(ui, &assets.models, db, &assets.sheets, req, rect)
}

/// [`paint_model`] with all of the pose in the box, held items too (the item form's preview).
pub fn paint_model_whole(ui: &Ui, assets: &Assets, db: &rustpg2d_engine::defs::Database, req: &rustpg2d_engine::render::model::PoseReq, rect: Rect) -> bool {
    paint_pose_fit(ui, &assets.models, db, &assets.sheets, req, rect, true)
}

/// [`paint_model`] with any set of models (the asset library's own).
pub fn paint_pose(ui: &Ui, models: &rustpg2d_engine::render::model::Models, db: &rustpg2d_engine::defs::Database, sheets: &HashMap<String, rustpg2d_engine::render::SheetTex>, req: &rustpg2d_engine::render::model::PoseReq, rect: Rect) -> bool {
    paint_pose_fit(ui, models, db, sheets, req, rect, false)
}

/// Paints a pose into `rect`: its standard frame fits the box (`whole`: all of it, held items
/// sticking out included).
fn paint_pose_fit(ui: &Ui, models: &rustpg2d_engine::render::model::Models, db: &rustpg2d_engine::defs::Database, sheets: &HashMap<String, rustpg2d_engine::render::SheetTex>, req: &rustpg2d_engine::render::model::PoseReq, rect: Rect, whole: bool) -> bool {
    let Some(cell) = models.pose(db, sheets, req) else { return false };
    let Some((tex, uv, base)) = models.cell_texture(&cell) else { return false };
    let base = if whole { macroquad::prelude::Rect::new(0.0, 0.0, 1.0, 1.0) } else { base };
    // The atlas page's OpenGL texture: egui-miniquad draws `TextureId::User` ids as raw GL textures.
    let raw = unsafe { macroquad::window::get_internal_gl().quad_context.texture_raw_id(tex.raw_miniquad_id()) };
    #[allow(irrefutable_let_patterns)]
    let macroquad::miniquad::RawId::OpenGl(gl) = raw else { return false };
    // The standard frame fits the box (bottom-centred); held items sticking out of it reach past
    // the box, clipped to the box and its surroundings by the panel.
    let (w, h) = (uv.w * base.w, uv.h * base.h);
    let k = (rect.width() / w).min(rect.height() / h);
    let size = vec2(w * k, h * k);
    let frame = Rect::from_min_size(pos2(rect.center().x - size.x / 2.0, rect.max.y - size.y), size);
    let whole = vec2(uv.w * k, uv.h * k);
    let dest = Rect::from_min_size(pos2(frame.min.x - base.x * whole.x, frame.min.y - base.y * whole.y), whole);
    let uv = Rect::from_min_max(pos2(uv.x, uv.y), pos2(uv.x + uv.w, uv.y + uv.h));
    ui.painter().with_clip_rect(rect.expand(rect.width() * 0.25).intersect(ui.clip_rect())).image(egui::TextureId::User(gl as u64), dest, uv, Color32::WHITE);
    true
}

pub fn paint_sprite(ui: &Ui, tex: &Textures, assets: &Assets, sprite: &SpriteRef, rect: Rect) -> bool {
    if let Some(id) = sprite.sheet.strip_prefix(CHAR_PREFIX) {
        return tex.characters.get(id).is_some_and(|a| paint_character(ui, assets, a, Facing::Down, rect));
    }
    let (Some(t), Some(s)) = (tex.sheets.get(&sprite.sheet), assets.sheets.get(&sprite.sheet)) else { return false };
    let (cw, ch) = sprite.cells();
    let src = s.region(sprite.frame, cw, ch);
    let (w, h) = (s.image.width as f32, s.image.height as f32);
    let uv = Rect::from_min_max(pos2(src.x / w, src.y / h), pos2((src.x + src.w) / w, (src.y + src.h) / h));
    // Keep aspect ratio, bottom-centered.
    let scale = (rect.width() / src.w).min(rect.height() / src.h);
    let size = vec2(src.w * scale, src.h * scale);
    let r = Rect::from_min_size(pos2(rect.center().x - size.x / 2.0, rect.max.y - size.y), size);
    ui.painter().image(t.id(), r, uv, Color32::WHITE);
    true
}

/// A sprite thumbnail as a widget (multi-cell sprites are scaled to fit).
pub fn sprite_thumb(ui: &mut Ui, tex: &Textures, assets: &Assets, sprite: &SpriteRef, size: f32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let bg = if selected { Color32::from_rgb(70, 90, 140) } else { Color32::from_gray(40) };
    ui.painter().rect_filled(rect, 2.0, bg);
    if !paint_sprite(ui, tex, assets, sprite, rect.shrink(2.0)) {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::default(), Color32::RED);
    }
    if resp.hovered() {
        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
    }
    let (w, h) = sprite.cells();
    resp.on_hover_text(format!("frame {} ({w}×{h} cells)", sprite.frame))
}

/// Sheet combo + frame grid.
/// Editor for a key lock: key item, consumption, effect and messages.
/// `prop`: the prop definition (its states are offered as "switch to" targets and previewed).
#[allow(clippy::too_many_arguments)]
pub fn lock_ui(ui: &mut Ui, id: &str, lock: &mut LockSpec, refs: &Refs, tex: &Textures, assets: &Assets, db: &rustpg2d_engine::defs::Database, prop: Option<&PropDef>) {
    let sprite = prop.map(|p| &p.sprite);
    ui.horizontal(|ui| {
        ui.label("Key item");
        let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        if let Some(d) = db.item(&lock.key) {
            paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
        }
        suggest(ui, &format!("{id}/key"), &mut lock.key, &refs.items, 120.0);
        if lock.key.is_empty() {
            ui.weak("(no key: opens when used, e.g. doors)");
        } else if db.item(&lock.key).is_none() {
            ui.colored_label(Color32::LIGHT_RED, "unknown item");
        }
    });
    ui.add_enabled_ui(!lock.key.is_empty(), |ui| {
        ui.horizontal(|ui| {
            ui.label("On unlocking the key");
            ui.radio_value(&mut lock.consume_key, true, "is used up");
            ui.radio_value(&mut lock.consume_key, false, "stays in the inventory");
        });
    });
    ui.horizontal(|ui| {
        ui.label("When unlocked");
        let mut k = match lock.on_unlock {
            OnUnlock::Nothing => 0,
            OnUnlock::Disappear => 1,
            OnUnlock::Open { .. } => 3,
            OnUnlock::ToState { .. } => 2,
        };
        let old = k;
        ui.radio_value(&mut k, 0, "stays (chests open)");
        ui.radio_value(&mut k, 1, "disappears");
        ui.radio_value(&mut k, 2, "switches state (doors, gates)");
        if old == 3 {
            ui.radio_value(&mut k, 3, "old frame offset");
        }
        if k != old {
            lock.on_unlock = match k {
                0 => OnUnlock::Nothing,
                1 => OnUnlock::Disappear,
                3 => OnUnlock::Open { frame: 1 },
                _ => {
                    // The first non-default state, ideally one called "open".
                    let names = prop.map(|p| p.state_names()).unwrap_or_default();
                    let pick = names.iter().skip(1).find(|n| n.contains("open")).or(names.get(1)).cloned().unwrap_or_default();
                    OnUnlock::ToState { state: pick }
                }
            };
        }
    });
    if let OnUnlock::ToState { state } = &mut lock.on_unlock {
        let names = prop.map(|p| p.state_names()).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Switch to state");
            egui::ComboBox::from_id_salt(format!("{id}/to_state")).selected_text(if state.is_empty() { "(choose)" } else { state.as_str() }).show_ui(ui, |ui| {
                for n in &names {
                    ui.selectable_value(state, n.clone(), n);
                }
            });
            if names.len() < 2 {
                ui.colored_label(Color32::YELLOW, "The prop has only one state: add an 'open' state in Database \u{E13A} Props.");
            }
        });
        if let Some(p) = prop {
            let target = p.state_index(state).map(|i| p.state_look(i));
            ui.horizontal(|ui| {
                let looks = [(p.default_state.as_str(), Some((&p.sprite, p.solid))), (state.as_str(), target)];
                for (label, look) in looks {
                    ui.vertical(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                        if !look.is_some_and(|(sp, _)| paint_sprite(ui, tex, assets, sp, r.shrink(3.0))) {
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(20.0), Color32::GRAY);
                        }
                        ui.weak(label);
                    });
                }
                if target.is_some_and(|(_, solid)| solid) {
                    ui.colored_label(Color32::YELLOW, "This state blocks movement: the opened prop can't be walked through.");
                }
            });
        }
    }
    if let OnUnlock::Open { frame } = &mut lock.on_unlock {
        ui.horizontal(|ui| {
            ui.label("Open frame offset");
            ui.add(egui::DragValue::new(frame).range(0..=255));
            ui.weak("(also stops blocking movement)");
        });
        // Closed / open preview, so a wrong offset (another prop's picture, an empty cell) shows.
        if let Some(s) = sprite {
            let open = SpriteRef { frame: s.frame + *frame, ..s.clone() };
            ui.horizontal(|ui| {
                for (label, sp) in [("closed", s), ("open", &open)] {
                    ui.vertical(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                        if !paint_sprite(ui, tex, assets, sp, r.shrink(3.0)) {
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(20.0), Color32::GRAY);
                        }
                        ui.weak(label);
                    });
                }
                let empty = assets.sheets.get(&s.sheet).is_some_and(|sh| {
                    let r = sh.region(open.frame, open.cells().0, open.cells().1);
                    let (x0, y0) = (r.x as u32, r.y as u32);
                    let (w, h) = (sh.image.width as u32, sh.image.height as u32);
                    x0 >= w || y0 >= h || (0..r.h as u32).all(|y| (0..r.w as u32).all(|x| x0 + x >= w || y0 + y >= h || sh.image.get_pixel(x0 + x, y0 + y).a == 0.0))
                });
                if empty {
                    ui.colored_label(Color32::YELLOW, "The open frame is empty: draw it in the pixel editor or pick another offset.");
                }
            });
        }
    }
    let disappears = lock.on_unlock == OnUnlock::Disappear;
    ui.add_enabled_ui(!disappears, |ui| {
        let label = if lock.key.is_empty() { "Can be closed again" } else { "Can be locked again with the key" };
        ui.horizontal(|ui| {
            ui.label("Lockpicking");
            ui.add(egui::DragValue::new(&mut lock.pick_difficulty).range(0..=99))
                .on_hover_text("A lockpicking skill of this level opens the lock without the key (0 = can't be picked). Without a key item the lock can only be picked.");
            if lock.pick_difficulty > 0 {
                ui.weak(format!("needs lockpicking {}", lock.pick_difficulty));
            }
        });
        ui.checkbox(&mut lock.relock, label).on_hover_text(
            "Interacting with the opened prop (or using the key on it) locks it again if the player carries the key. \
             Containers are only relocked by using the key on them. 'Locked again' triggers run afterwards.",
        );
        if disappears {
            ui.weak("(not possible for props that disappear)");
        } else if lock.relock && lock.consume_key && !lock.key.is_empty() {
            ui.colored_label(Color32::YELLOW, "The key is used up on unlocking: relocking needs another key.");
        }
    });
    ui.horizontal(|ui| {
        ui.label("Locked message");
        ui.add(egui::TextEdit::singleline(&mut lock.locked_text).hint_text("It's locked. You need the <key>.").desired_width(240.0));
    });
    ui.horizontal(|ui| {
        ui.label("Unlock message");
        ui.add(egui::TextEdit::singleline(&mut lock.unlock_text).hint_text("Unlocked with the <key>.").desired_width(240.0));
    });
    if lock.can_relock() {
        ui.horizontal(|ui| {
            ui.label("Relock message");
            let hint = if lock.key.is_empty() { "Closed." } else { "Locked with the <key>." };
            ui.add(egui::TextEdit::singleline(&mut lock.lock_text).hint_text(hint).desired_width(240.0));
        });
    }
    ui.weak("Interacting (or using the key on it) checks the inventory for the key. 'Unlocked' triggers run when it opens, 'Locked again' triggers when it is relocked.");
}

/// Editor for a container: size, starting contents, lock conditions and open frame.
/// `states`: the prop's state names (for the "when opened" state).
#[allow(clippy::too_many_arguments)]
pub fn container_ui(ui: &mut Ui, id: &str, spec: &mut ContainerSpec, refs: &Refs, tex: &Textures, assets: &Assets, db: &rustpg2d_engine::defs::Database, states: &[String]) {
    ui.horizontal(|ui| {
        ui.label("Slots");
        ui.add(egui::DragValue::new(&mut spec.size).range(1..=64));
        ui.label("when opened, switch to");
        let label = if spec.open_state.is_empty() { "(no change)".to_string() } else { spec.open_state.clone() };
        egui::ComboBox::from_id_salt(format!("{id}/open_state")).selected_text(label).show_ui(ui, |ui| {
            ui.selectable_value(&mut spec.open_state, String::new(), "(no change)");
            for s in states {
                ui.selectable_value(&mut spec.open_state, s.clone(), s);
            }
        });
        if !spec.open_state.is_empty() {
            spec.open_frame = 0;
        } else if spec.open_frame > 0 {
            ui.weak(format!("(old frame offset {})", spec.open_frame));
        }
    });
    ui.label("Contents (filled into the slots in order; big counts split by the item's stack size):");
    let mut remove = None;
    for (k, s) in spec.items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            if let Some(d) = db.item(&s.item) {
                paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
            }
            ui.add(egui::DragValue::new(&mut s.count).range(1..=9999));
            ui.label("x");
            suggest(ui, &format!("{id}/c{k}"), &mut s.item, &refs.items, 110.0);
            if !s.item.is_empty() && db.item(&s.item).is_none() {
                ui.colored_label(Color32::LIGHT_RED, "unknown item");
            }
            if ui.small_button("\u{E4F6}").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        spec.items.remove(k);
    }
    ui.horizontal(|ui| {
        if ui.button("+ Item").clicked() {
            spec.items.push(ItemStack::new(refs.items.first().map_or("", |s| s.as_str()), 1));
        }
        // Slots needed vs. available.
        let needed: u32 = spec.items.iter().map(|s| s.count.div_ceil(db.item(&s.item).map_or(99, |d| d.max_stack.max(1)))).sum();
        if needed > spec.size {
            ui.colored_label(Color32::LIGHT_RED, format!("needs {needed} slots, only {} — extra items are dropped", spec.size));
        } else {
            ui.weak(format!("{needed}/{} slots used", spec.size));
        }
    });
    ui.label("Opens only if all of:");
    ui.indent(format!("{id}/cond"), |ui| conditions_ui(ui, &format!("{id}/cond"), &mut spec.conditions, refs));
    ui.horizontal(|ui| {
        ui.label("Locked message");
        ui.add(egui::TextEdit::singleline(&mut spec.locked_text).hint_text("It's locked.").desired_width(220.0));
    });
}

/// Loot table editor: item, drop chance in percent and amount (min-max) per line.
pub fn loot_ui(ui: &mut Ui, id: &str, table: &mut Vec<LootEntry>, refs: &Refs, tex: &Textures, assets: &Assets, db: &rustpg2d_engine::defs::Database) {
    let mut remove = None;
    if !table.is_empty() {
        egui::Grid::new(format!("{id}/loot")).num_columns(5).striped(true).show(ui, |ui| {
            ui.weak("");
            ui.weak("Item");
            ui.weak("Chance");
            ui.weak("Amount");
            ui.weak("");
            ui.end_row();
            for (k, e) in table.iter_mut().enumerate() {
                let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                let def = db.item(&e.item);
                if let Some(d) = def {
                    paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
                }
                ui.horizontal(|ui| {
                    suggest(ui, &format!("{id}/i{k}"), &mut e.item, &refs.items, 110.0);
                    if !e.item.is_empty() && def.is_none() {
                        ui.colored_label(Color32::LIGHT_RED, "unknown");
                    }
                });
                ui.add(egui::DragValue::new(&mut e.chance).range(0.0..=100.0).speed(0.5).suffix(" %"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut e.min).range(1..=999));
                    ui.label("–");
                    ui.add(egui::DragValue::new(&mut e.max).range(1..=999));
                });
                if e.max < e.min {
                    e.max = e.min;
                }
                if ui.small_button("\u{E4F6}").on_hover_text("Remove").clicked() {
                    remove = Some(k);
                }
                ui.end_row();
            }
        });
    }
    if let Some(k) = remove {
        table.remove(k);
    }
    ui.horizontal(|ui| {
        if ui.button("+ Item").clicked() {
            let first = refs.items.iter().find(|i| i.contains("coin")).or(refs.items.first()).cloned().unwrap_or_default();
            table.push(LootEntry::new(&first, 50.0, 1, 1));
        }
        if table.is_empty() {
            ui.weak("Drops nothing.");
        } else {
            ui.weak("Each line rolls on its own; drops land where the character fell.");
        }
    });
}

/// Merchant editor: currency, sell rate, and the stock with quantities and prices.
pub fn shop_ui(ui: &mut Ui, id: &str, spec: &mut ShopSpec, refs: &Refs, tex: &Textures, assets: &Assets, db: &rustpg2d_engine::defs::Database) {
    ui.horizontal(|ui| {
        ui.label("Shop title");
        ui.add(egui::TextEdit::singleline(&mut spec.title).hint_text("(character name)").desired_width(180.0));
    });
    ui.horizontal(|ui| {
        ui.label("Currency");
        let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        if let Some(d) = db.item(&spec.currency) {
            paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
        }
        suggest(ui, &format!("{id}/cur"), &mut spec.currency, &refs.items, 110.0);
        if db.item(&spec.currency).is_none() {
            ui.colored_label(Color32::LIGHT_RED, "unknown item");
        }
    });
    ui.horizontal(|ui| {
        ui.label("Buys the player's items for");
        ui.add(egui::DragValue::new(&mut spec.sell_rate).range(0..=200).suffix(" %"))
            .on_hover_text("Percent of the price (or the item's value) paid to the player; 0 = this merchant doesn't buy anything");
        if spec.sell_rate == 0 {
            ui.weak("(doesn't buy)");
        }
    });
    ui.checkbox(&mut spec.open_on_talk, "Talking opens the shop")
        .on_hover_text("Off: only an 'Open shop' action (e.g. after a dialog choice) or open_shop(\"name\") opens it");
    ui.label("Stock:");
    let mut remove = None;
    let mut swap = None;
    let n = spec.items.len();
    egui::Grid::new(format!("{id}/stock")).num_columns(6).striped(true).show(ui, |ui| {
        ui.weak("");
        ui.weak("Item");
        ui.weak("Quantity");
        ui.weak("Price");
        ui.weak("");
        ui.weak("");
        ui.end_row();
        for (k, e) in spec.items.iter_mut().enumerate() {
            let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            let def = db.item(&e.item);
            if let Some(d) = def {
                paint_frame(ui, tex, assets, &d.icon.sheet, d.icon.frame, rect);
            }
            ui.horizontal(|ui| {
                suggest(ui, &format!("{id}/i{k}"), &mut e.item, &refs.items, 120.0);
                if !e.item.is_empty() && def.is_none() {
                    ui.colored_label(Color32::LIGHT_RED, "unknown");
                }
            });
            ui.horizontal(|ui| {
                ui.add_enabled(!e.unlimited, egui::DragValue::new(&mut e.count).range(0..=9999));
                ui.checkbox(&mut e.unlimited, "unlimited");
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut e.price).range(0..=999_999));
                if e.price == 0 {
                    let v = def.map_or(0, |d| d.value);
                    ui.weak(if v > 0 { format!("= value {v}") } else { "free!".to_string() })
                        .on_hover_text("0 uses the item's Value (set in Database > Items)");
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(k > 0, egui::Button::new("^").small()).on_hover_text("Move up").clicked() {
                    swap = Some(k - 1);
                }
                if ui.add_enabled(k + 1 < n, egui::Button::new("v").small()).on_hover_text("Move down").clicked() {
                    swap = Some(k);
                }
            });
            if ui.small_button("\u{E4F6}").on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
            ui.end_row();
        }
    });
    if let Some(k) = swap {
        spec.items.swap(k, k + 1);
    }
    if let Some(k) = remove {
        spec.items.remove(k);
    }
    ui.horizontal(|ui| {
        if ui.button("+ Item").clicked() {
            let first = refs.items.iter().find(|i| **i != spec.currency).cloned().unwrap_or_default();
            spec.items.push(ShopEntry::new(&first, 1, 0));
        }
        let dup = spec.items.iter().enumerate().any(|(k, e)| spec.items[..k].iter().any(|o| o.item == e.item));
        if dup {
            ui.colored_label(Color32::LIGHT_RED, "an item is listed twice (only the first price counts)");
        }
    });
}

pub fn sprite_picker(ui: &mut Ui, id: &str, sprite: &mut SpriteRef, tex: &Textures, assets: &Assets) {
    let mut sheets: Vec<&String> = assets.sheets.keys().collect();
    sheets.sort();
    ui.horizontal(|ui| {
        ui.label("Sheet");
        egui::ComboBox::from_id_salt(format!("{id}/sheet"))
            .selected_text(sprite.sheet.as_str())
            .show_ui(ui, |ui| {
                for s in sheets {
                    ui.selectable_value(&mut sprite.sheet, s.clone(), s.as_str());
                }
            });
        ui.label("Frame");
        ui.add(egui::DragValue::new(&mut sprite.frame).range(0..=4096));
        ui.label("Size (cells)");
        if sprite.w == 0 {
            sprite.w = 1;
        }
        if sprite.h == 0 {
            sprite.h = 1;
        }
        ui.add(egui::DragValue::new(&mut sprite.w).range(1..=16));
        ui.label("×");
        ui.add(egui::DragValue::new(&mut sprite.h).range(1..=16));
    });
    region_picker(ui, id, sprite, tex, assets);
}

/// The whole sheet with a grid: click a cell, or drag over several cells for a multi-cell sprite.
pub fn region_picker(ui: &mut Ui, id: &str, sprite: &mut SpriteRef, tex: &Textures, assets: &Assets) {
    let (Some(t), Some(s)) = (tex.sheets.get(&sprite.sheet), assets.sheets.get(&sprite.sheet)) else {
        ui.colored_label(Color32::LIGHT_RED, "unknown sheet");
        return;
    };
    ui.weak("Click a cell, or drag over several cells for a bigger sprite (e.g. 2×2 gate, 3×4 tree).");
    let rows = s.count.div_ceil(s.columns.max(1));
    let scale = ((ui.available_width() - 8.0) / s.image.width as f32).clamp(1.0, 3.0).floor();
    let (cw, ch) = (s.frame_w * scale, s.frame_h * scale);
    let size = vec2(s.image.width as f32 * scale, s.image.height as f32 * scale);
    egui::ScrollArea::both().id_salt(format!("{id}/region")).max_height(320.0).show(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, Color32::from_gray(30));
        p.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        for c in 1..s.columns {
            let x = rect.min.x + c as f32 * cw;
            p.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], Stroke::new(1.0_f32, Color32::from_white_alpha(25)));
        }
        for r in 1..rows {
            let y = rect.min.y + r as f32 * ch;
            p.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], Stroke::new(1.0_f32, Color32::from_white_alpha(25)));
        }
        let cell_at = |pos: egui::Pos2| {
            let rel = pos - rect.min;
            (((rel.x / cw).floor().max(0.0) as u32).min(s.columns - 1), ((rel.y / ch).floor().max(0.0) as u32).min(rows - 1))
        };
        let drag_id = egui::Id::new(format!("{id}/region_start"));
        if resp.drag_started() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                ui.memory_mut(|m| m.data.insert_temp(drag_id, cell_at(pos)));
            }
        }
        if resp.dragged() || resp.clicked() {
            if let (Some(pos), Some(start)) = (resp.interact_pointer_pos(), ui.memory(|m| m.data.get_temp::<(u32, u32)>(drag_id))) {
                let end = cell_at(pos);
                let (x0, y0) = (start.0.min(end.0), start.1.min(end.1));
                sprite.frame = y0 * s.columns + x0;
                sprite.w = start.0.abs_diff(end.0) + 1;
                sprite.h = start.1.abs_diff(end.1) + 1;
            }
        }
        let (c, r) = (sprite.frame % s.columns.max(1), sprite.frame / s.columns.max(1));
        let (w, h) = sprite.cells();
        let sel = Rect::from_min_size(rect.min + vec2(c as f32 * cw, r as f32 * ch), vec2(w as f32 * cw, h as f32 * ch));
        p.rect_stroke(sel, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
    });
}

/// Values offered by dropdowns in the trigger editor.
#[derive(Default, Clone)]
pub struct Refs {
    pub items: Vec<String>,
    /// Prop definition ids.
    pub props: Vec<String>,
    pub maps: Vec<String>,
    /// "self", "player" and entity names on the current map.
    pub targets: Vec<String>,
    pub functions: Vec<String>,
    /// map -> spawn names
    pub spawns: HashMap<String, Vec<String>>,
    /// map -> its transition zones: (zone, target map, target zone)
    pub transitions: HashMap<String, Vec<(String, String, String)>>,
    /// Name of the open map.
    pub current_map: String,
    pub flags: Vec<String>,
    pub sounds: Vec<String>,
    pub characters: Vec<String>,
    /// All prop state names (for "Set prop state" / "Prop state").
    pub states: Vec<String>,
    /// Named entities on all maps (script targets).
    pub entity_names: Vec<String>,
    /// Dialog speakers for the open map: (value, explanation).
    pub speakers: Vec<(String, String)>,
    /// Flags and variables used by triggers anywhere in the project.
    pub all_flags: Vec<String>,
    pub vars: Vec<String>,
    /// Quest ids with their number of stages.
    pub quests: Vec<String>,
    pub quest_stages: HashMap<String, Vec<String>>,
    pub skills: Vec<String>,
    pub spells: Vec<String>,
    /// Race ids (Database › Races).
    pub races: Vec<String>,
    /// Faction ids (Database › Project › Factions).
    pub factions: Vec<String>,
    /// Crafting stations named by recipes.
    pub stations: Vec<String>,
    /// Achievement ids.
    pub achievements: Vec<String>,
    /// Sprite sheet ids (effects).
    pub sheets: Vec<String>,
    /// Named animations of sprite sheets (Aseprite tags).
    pub sheet_tags: HashMap<String, Vec<rustpg2d_engine::defs::SheetTag>>,
    /// Emotion names (the tags of Emotions sprite sheets).
    pub emotions: Vec<String>,
}

/// Placeholders understood in dialog / choice texts (and quest texts).
pub const TEXT_HELP: &str = "Placeholders: {player} = the player's name, {sir|madam} = first word for a male player, second for a female one, {name} = value of variable 'name'. In the speaker field {self} is the name of the trigger's owner.";

/// Stage number with the stage's objective text as a dropdown.
fn quest_stage_field(ui: &mut Ui, id: &str, quest: &str, stage: &mut u32, refs: &Refs) {
    let stages = refs.quest_stages.get(quest).cloned().unwrap_or_default();
    let text = |k: u32| stages.get(k as usize).map_or(format!("stage {k}"), |s| format!("{k}: {}", s.chars().take(40).collect::<String>()));
    egui::ComboBox::from_id_salt(id).width(200.0).selected_text(text(*stage)).show_ui(ui, |ui| {
        for k in 0..stages.len().max(*stage as usize + 1) as u32 {
            ui.selectable_value(stage, k, text(k));
        }
    });
}

/// Single-line text field with a dropdown of suggestions.
pub fn suggest(ui: &mut Ui, id: &str, value: &mut String, options: &[String], width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.add(egui::TextEdit::singleline(value).desired_width(width));
        if !options.is_empty() {
            ui.push_id(id, |ui| {
                ui.menu_button("\u{E136}", |ui| {
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for o in options {
                            if ui.selectable_label(o == value, o).clicked() {
                                *value = o.clone();
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        }
    });
}

/// Like [`suggest`], with an explanation next to each option in the dropdown.
pub fn suggest_described(ui: &mut Ui, id: &str, value: &mut String, options: &[(String, String)], width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.add(egui::TextEdit::singleline(value).desired_width(width));
        if !options.is_empty() {
            ui.push_id(id, |ui| {
                ui.menu_button("\u{E136}", |ui| {
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for (o, hint) in options {
                            let text = egui::RichText::new(o.as_str());
                            let resp = ui.horizontal(|ui| {
                                let r = ui.selectable_label(o == value, text);
                                if !hint.is_empty() {
                                    ui.weak(hint.as_str());
                                }
                                r
                            });
                            if resp.inner.clicked() {
                                *value = o.clone();
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        }
    });
}

/// Speaker field of Dialog / Choice actions: type a name or pick one from the map.
fn speaker_field(ui: &mut Ui, id: &str, speaker: &mut String, refs: &Refs) {
    labeled(ui, "speaker", |ui| {
        suggest_described(ui, &format!("{id}/speaker"), speaker, &refs.speakers, 160.0);
    });
}

/// The animated sprite of the Fly / Effect actions.
fn fx_sprite_ui(ui: &mut Ui, id: &str, s: &mut rustpg2d_engine::events::FxSprite, refs: &Refs, shake: bool) {
    labeled(ui, "sheet", |ui| suggest(ui, &format!("{id}/sheet"), &mut s.sheet, &refs.sheets, 120.0));
    // Named animations of the sheet (e.g. Aseprite tags) fill in the frames and speed.
    if let Some(tags) = refs.sheet_tags.get(&s.sheet).filter(|t| !t.is_empty()) {
        labeled(ui, "animation", |ui| {
            let current = tags.iter().find(|t| (t.first, t.last) == (s.first, s.last)).map_or("(frames below)".to_string(), |t| t.name.clone());
            egui::ComboBox::from_id_salt(format!("{id}/tag")).selected_text(current).show_ui(ui, |ui| {
                for t in tags {
                    if ui.selectable_label((t.first, t.last) == (s.first, s.last), format!("{}  ({}–{}, {} fps)", t.name, t.first, t.last, t.fps)).clicked() {
                        (s.first, s.last, s.fps) = (t.first, t.last, t.fps);
                        s.tag.clear();
                    }
                }
            });
        });
    }
    labeled(ui, "frames", |ui| {
        ui.add(egui::DragValue::new(&mut s.first).range(0..=999));
        ui.label("to");
        ui.add(egui::DragValue::new(&mut s.last).range(0..=999)).on_hover_text("0 to 0 = every frame of the sheet");
        ui.add(egui::DragValue::new(&mut s.fps).range(0.5..=60.0).speed(0.2).suffix(" fps"));
    });
    labeled(ui, "size", |ui| {
        ui.add(egui::DragValue::new(&mut s.scale).range(0.1..=10.0).speed(0.05).prefix("x "));
        ui.label("light");
        ui.add(egui::DragValue::new(&mut s.light).range(0.0..=16.0).speed(0.1).suffix(" tiles")).on_hover_text("0 = no glow");
        if s.light > 0.0 {
            ui.color_edit_button_srgb(&mut s.glow).on_hover_text("Color of the light");
        }
    });
    if shake {
        labeled(ui, "shake", |ui| {
            ui.add(egui::DragValue::new(&mut s.shake).range(0.0..=5.0).speed(0.05).suffix(" s"))
                .on_hover_text("Shakes the screen this long when it starts (explosions); 0 = no shake. Players can switch shaking off in the Options.");
        });
    }
}

/// A point in tile coordinates (10.5 = the middle of tile 10).
fn tile_point_ui(ui: &mut Ui, label: &str, x: &mut f32, y: &mut f32) {
    labeled(ui, label, |ui| {
        ui.add(egui::DragValue::new(x).speed(0.1).prefix("x "));
        ui.add(egui::DragValue::new(y).speed(0.1).prefix("y "));
        ui.weak("tiles");
    });
}

fn labeled(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        add(ui);
    });
}

/// The trigger list of `owner` (a map, zone, character, ...). Only events that reach that
/// owner are offered; triggers with another event get a warning and a fix.
pub fn triggers_ui(ui: &mut Ui, salt: &str, triggers: &mut Vec<Trigger>, refs: &Refs, owner: TriggerOwner) {
    let mut remove = None;
    let mut swap = None;
    let n = triggers.len();
    for (i, t) in triggers.iter_mut().enumerate() {
        let id = format!("{salt}/t{i}");
        let useless = !owner.allows(&t.on);
        let title = format!(
            "{}On {}{}  ·  {} cond, {} act{}",
            if useless { "\u{E4E0} " } else { "" },
            t.on.label(),
            match &t.on {
                TriggerOn::UseItem { item } if !item.is_empty() => format!(" [{item}]"),
                TriggerOn::Custom { name } => format!(" [{name}]"),
                TriggerOn::HpBelow { percent } => format!(" [{percent} %]"),
                TriggerOn::Hour { hour } => format!(" [{}]", hour_text(*hour)),
                _ => String::new(),
            },
            t.conditions.len(),
            t.actions.len(),
            if t.once { " · once" } else { "" }
        );
        egui::CollapsingHeader::new(title).id_salt(&id).default_open(n <= 2).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("On");
                egui::ComboBox::from_id_salt(format!("{id}/on")).selected_text(t.on.label()).show_ui(ui, |ui| {
                    for tpl in owner.templates() {
                        let same = std::mem::discriminant(&tpl) == std::mem::discriminant(&t.on);
                        if ui.selectable_label(same, tpl.label()).clicked() && !same {
                            t.on = tpl;
                        }
                    }
                });
                match &mut t.on {
                    TriggerOn::UseItem { item } => {
                        let mut opts = vec![String::new()];
                        opts.extend(refs.items.iter().cloned());
                        suggest(ui, &format!("{id}/item"), item, &opts, 90.0);
                    }
                    TriggerOn::Custom { name } => {
                        ui.add(egui::TextEdit::singleline(name).hint_text("event name").desired_width(110.0));
                    }
                    TriggerOn::Hour { hour } => {
                        ui.add(egui::DragValue::new(hour).range(0.0..=23.99).speed(0.05).custom_formatter(|v, _| hour_text(v as f32)).custom_parser(parse_hour)).on_hover_text(
                            "Runs when the in-game clock reaches this time while the player is on the map — also when time jumps past it (sleeping, Set time). Every day; tick \"once\" for a single time.",
                        );
                    }
                    TriggerOn::HpBelow { percent } => {
                        ui.add(egui::DragValue::new(percent).range(1..=99).suffix(" %")).on_hover_text(
                            "Runs when the character's HP falls below this share of its max HP (and it survives the hit) — e.g. a boss calling for help at 50 %. Again after healing above it.",
                        );
                    }
                    _ => {}
                }
                ui.checkbox(&mut t.once, "once");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("\u{E4F6}").on_hover_text("Delete trigger").clicked() {
                        remove = Some(i);
                    }
                    if i + 1 < n && ui.small_button("\u{E136}").clicked() {
                        swap = Some((i, i + 1));
                    }
                    if i > 0 && ui.small_button("\u{E13C}").clicked() {
                        swap = Some((i - 1, i));
                    }
                });
            });
            if useless {
                let fix = owner.replacement(&t.on);
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::YELLOW, format!("'{}' never happens in {}.", t.on.label(), owner.label()));
                    if ui.button(format!("Use '{}'", fix.label())).clicked() {
                        t.on = fix;
                    }
                });
            }
            if matches!(t.on, TriggerOn::UseItem { .. }) {
                ui.weak("(empty item = any item)");
            }
            ui.label("If all of:");
            ui.indent(format!("{id}/ci"), |ui| conditions_ui(ui, &id, &mut t.conditions, refs));
            ui.label("Do:");
            ui.indent(format!("{id}/ai"), |ui| actions_ui(ui, &id, &mut t.actions, refs));
        });
    }
    if let Some(i) = remove {
        triggers.remove(i);
    }
    if let Some((a, b)) = swap {
        triggers.swap(a, b);
    }
    ui.menu_button("+ Trigger", |ui| {
        for tpl in owner.templates() {
            if ui.button(tpl.label()).clicked() {
                triggers.push(Trigger::new(tpl));
                ui.close_menu();
            }
        }
    });
}

pub fn conditions_ui(ui: &mut Ui, salt: &str, conds: &mut Vec<Condition>, refs: &Refs) {
    let mut remove = None;
    for (i, c) in conds.iter_mut().enumerate() {
        let id = format!("{salt}/c{i}");
        ui.horizontal(|ui| {
            ui.checkbox(&mut c.negate, "not");
            match &mut c.cond {
                Cond::Flag { name } => {
                    ui.label("flag");
                    suggest(ui, &id, name, &refs.flags, 110.0);
                }
                Cond::Var { name, op, value } => {
                    ui.label("var");
                    ui.add(egui::TextEdit::singleline(name).desired_width(80.0));
                    egui::ComboBox::from_id_salt(format!("{id}/op")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(value));
                }
                Cond::HasItem { item, count } => {
                    ui.label("has");
                    ui.add(egui::DragValue::new(count).range(1..=9999));
                    suggest(ui, &id, item, &refs.items, 90.0);
                }
                Cond::Script { function } => {
                    ui.label("script");
                    suggest(ui, &id, function, &refs.functions, 110.0);
                    ui.label("()");
                }
                Cond::Quest { quest, check, stage } => {
                    ui.label("quest");
                    suggest(ui, &format!("{id}/q"), quest, &refs.quests, 100.0);
                    ui.label("is");
                    egui::ComboBox::from_id_salt(format!("{id}/qc")).selected_text(check.label()).show_ui(ui, |ui| {
                        for c in rustpg2d_engine::quest::QuestCheck::ALL {
                            ui.selectable_value(check, c, c.label());
                        }
                    });
                    if check.uses_stage() {
                        quest_stage_field(ui, &format!("{id}/qs"), quest, stage, refs);
                    }
                }
                Cond::Skill { skill, op, level } => {
                    ui.label("skill");
                    suggest(ui, &format!("{id}/sk"), skill, &refs.skills, 100.0);
                    egui::ComboBox::from_id_salt(format!("{id}/skop")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(level).range(0..=99));
                }
                Cond::Level { op, value } => {
                    ui.label("player level");
                    egui::ComboBox::from_id_salt(format!("{id}/lvop")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(value).range(1..=999));
                }
                Cond::Defeated { character, op, count } => {
                    ui.label("defeated");
                    suggest(ui, &format!("{id}/dc"), character, &refs.characters, 100.0);
                    egui::ComboBox::from_id_salt(format!("{id}/dcop")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(count).range(0..=100_000)).on_hover_text("How many characters of this kind the player has defeated (the bestiary's count)");
                }
                Cond::HasEffect { kind } => {
                    ui.label("player has");
                    effect_kind_combo(ui, &format!("{id}/fx"), kind);
                }
                Cond::KnowsSpell { spell } => {
                    ui.label("knows");
                    suggest(ui, &format!("{id}/sp"), spell, &refs.spells, 120.0);
                }
                Cond::TimeBetween { from, to } => {
                    ui.label("time from");
                    ui.add(egui::DragValue::new(from).range(0.0..=24.0).speed(0.25).suffix(" h"));
                    ui.label("to");
                    ui.add(egui::DragValue::new(to).range(0.0..=24.0).speed(0.25).suffix(" h"))
                        .on_hover_text("Hours 0–24; e.g. 20 to 6 = at night (wraps around midnight)");
                }
                Cond::PlayerRace { race } => {
                    ui.label("player is a");
                    suggest(ui, &format!("{id}/race"), race, &refs.races, 110.0);
                }
                Cond::FactionStanding { faction, op, value } => {
                    ui.label("standing with");
                    suggest(ui, &format!("{id}/fac"), faction, &refs.factions, 100.0);
                    egui::ComboBox::from_id_salt(format!("{id}/facop")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(value).range(-100..=100))
                        .on_hover_text("The player's standing: -100..100; at -20 or below the faction's characters attack the player");
                }
                Cond::PlayerSex { sex } => {
                    ui.label("player is");
                    egui::ComboBox::from_id_salt(format!("{id}/sex")).selected_text(sex.label()).show_ui(ui, |ui| {
                        for s in rustpg2d_engine::state::Sex::ALL {
                            ui.selectable_value(sex, s, s.label());
                        }
                    });
                }
                Cond::Pose { target, pose } => {
                    suggest(ui, &format!("{id}/t"), target, &refs.targets, 90.0);
                    ui.label("is");
                    egui::ComboBox::from_id_salt(format!("{id}/pose")).width(90.0).selected_text(match pose.as_str() {
                        "sit" => "sitting",
                        "lie" => "lying (asleep)",
                        "stand" => "at furniture",
                        _ => "on its feet",
                    }).show_ui(ui, |ui| {
                        for (p, l) in [("sit", "sitting"), ("lie", "lying (asleep)"), ("stand", "at furniture"), ("", "on its feet")] {
                            ui.selectable_value(pose, p.to_string(), l);
                        }
                    });
                }
                Cond::PropState { target, state } => {
                    ui.label("prop");
                    suggest(ui, &format!("{id}/t"), target, &refs.targets, 90.0);
                    ui.label("is in state");
                    suggest(ui, &format!("{id}/s"), state, &refs.states, 90.0);
                }
                Cond::PlayerHp { op, value } => {
                    ui.label("player HP");
                    egui::ComboBox::from_id_salt(format!("{id}/op")).width(40.0).selected_text(op.symbol()).show_ui(ui, |ui| {
                        for o in CmpOp::ALL {
                            ui.selectable_value(op, o, o.symbol());
                        }
                    });
                    ui.add(egui::DragValue::new(value));
                }
            }
            if ui.small_button("\u{E4F6}").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        conds.remove(i);
    }
    ui.push_id(format!("{salt}/addc"), |ui| {
        ui.menu_button("+ Condition", |ui| {
            for tpl in Cond::templates() {
                if ui.button(tpl.label()).clicked() {
                    conds.push(Condition::new(tpl));
                    ui.close_menu();
                }
            }
        });
    });
}

pub fn actions_ui(ui: &mut Ui, salt: &str, actions: &mut Vec<Action>, refs: &Refs) {
    let mut remove = None;
    let mut swap = None;
    let n = actions.len();
    for (i, a) in actions.iter_mut().enumerate() {
        let id = format!("{salt}/a{i}");
        egui::Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{}.", i + 1));
                ui.strong(a.label());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("\u{E4F6}").clicked() {
                        remove = Some(i);
                    }
                    if i + 1 < n && ui.small_button("\u{E136}").clicked() {
                        swap = Some((i, i + 1));
                    }
                    if i > 0 && ui.small_button("\u{E13C}").clicked() {
                        swap = Some((i - 1, i));
                    }
                });
            });
            action_fields(ui, &id, a, refs);
        });
    }
    if let Some(i) = remove {
        actions.remove(i);
    }
    if let Some((a, b)) = swap {
        actions.swap(a, b);
    }
    ui.push_id(format!("{salt}/adda"), |ui| {
        ui.menu_button("+ Action", |ui| {
            for tpl in Action::templates() {
                if ui.button(tpl.label()).clicked() {
                    actions.push(tpl);
                    ui.close_menu();
                }
            }
        });
    });
}

fn target_field(ui: &mut Ui, id: &str, target: &mut String, refs: &Refs) {
    labeled(ui, "target", |ui| suggest(ui, id, target, &refs.targets, 100.0));
}

fn action_fields(ui: &mut Ui, id: &str, a: &mut Action, refs: &Refs) {
    match a {
        Action::Dialog { speaker, text } => {
            speaker_field(ui, id, speaker, refs);
            ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(f32::INFINITY))
                .on_hover_text(TEXT_HELP);
        }
        Action::Choice { speaker, text, options, var } => {
            speaker_field(ui, id, speaker, refs);
            ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(f32::INFINITY));
            ui.label("Options (one per line):");
            let mut joined = options.join("\n");
            if ui.add(egui::TextEdit::multiline(&mut joined).desired_rows(2).desired_width(f32::INFINITY)).changed() {
                *options = joined.lines().map(str::to_string).collect();
            }
            labeled(ui, "store index in var", |ui| {
                ui.add(egui::TextEdit::singleline(var).desired_width(100.0));
            });
        }
        Action::GiveItem { item, count } | Action::TakeItem { item, count } => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(count).range(1..=9999));
                ui.label("x");
                suggest(ui, id, item, &refs.items, 110.0);
            });
        }
        Action::Equip { target, item } => {
            target_field(ui, id, target, refs);
            labeled(ui, "item", |ui| suggest(ui, &format!("{id}/eq"), item, &refs.items, 120.0));
            ui.weak("Goes into its slot; what was worn there is carried instead. The player takes it from the inventory (or gets one).");
        }
        Action::Unequip { target, slot } => {
            target_field(ui, id, target, refs);
            labeled(ui, "slot", |ui| {
                egui::ComboBox::from_id_salt(format!("{id}/slot")).selected_text(slot.label()).show_ui(ui, |ui| {
                    for s in rustpg2d_engine::defs::EquipSlot::ALL {
                        ui.selectable_value(slot, s, s.label());
                    }
                });
            });
        }
        Action::GiveItemTo { target, item, count } | Action::TakeItemFrom { target, item, count } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(count).range(1..=9999));
                ui.label("x");
                suggest(ui, &format!("{id}/it"), item, &refs.items, 110.0);
            });
        }
        Action::SetUiVisible { visible } => {
            ui.horizontal(|ui| {
                ui.radio_value(visible, false, "Hide");
                ui.radio_value(visible, true, "Show");
                ui.weak("the HUD (status, clock, quick bar, buttons); dialogs still show");
            });
        }
        Action::SetFlag { name, value } => {
            ui.horizontal(|ui| {
                suggest(ui, id, name, &refs.flags, 120.0);
                ui.checkbox(value, "set");
            });
        }
        Action::SetVar { name, op, value } => {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(90.0));
                egui::ComboBox::from_id_salt(format!("{id}/op")).width(50.0).selected_text(match op {
                    VarOp::Set => "=",
                    VarOp::Add => "+=",
                }).show_ui(ui, |ui| {
                    ui.selectable_value(op, VarOp::Set, "=");
                    ui.selectable_value(op, VarOp::Add, "+=");
                });
                ui.add(egui::DragValue::new(value));
            });
        }
        Action::Teleport { map, spawn } => {
            labeled(ui, "map", |ui| suggest(ui, &format!("{id}/m"), map, &refs.maps, 110.0));
            let spawns = refs.spawns.get(map.as_str()).cloned().unwrap_or_default();
            labeled(ui, "spawn", |ui| suggest(ui, &format!("{id}/s"), spawn, &spawns, 110.0));
        }
        Action::SetActive { target, active } => {
            target_field(ui, id, target, refs);
            ui.checkbox(active, "active");
        }
        Action::SetState { target, state } => {
            target_field(ui, id, target, refs);
            labeled(ui, "state", |ui| suggest(ui, &format!("{id}/state"), state, &refs.states, 100.0));
        }
        Action::GiveSkillPoints { amount } => {
            labeled(ui, "points", |ui| {
                ui.add(egui::DragValue::new(amount).range(-99..=99)).on_hover_text("Negative takes points away");
            });
        }
        Action::SetSkill { skill, level } => {
            labeled(ui, "skill", |ui| suggest(ui, &format!("{id}/sk"), skill, &refs.skills, 120.0));
            labeled(ui, "level", |ui| {
                ui.add(egui::DragValue::new(level).range(0..=99));
            });
        }
        Action::LearnSpell { spell } => {
            labeled(ui, "spell", |ui| suggest(ui, &format!("{id}/sp"), spell, &refs.spells, 120.0));
        }
        Action::TeachSpell { target, spell } => {
            target_field(ui, id, target, refs);
            labeled(ui, "spell", |ui| suggest(ui, &format!("{id}/sp"), spell, &refs.spells, 120.0));
            ui.weak("(a character learns it; as a companion it casts Heal and Bolt spells from its mana: Database › Characters › Magic)");
        }
        Action::AddMana { amount } => {
            labeled(ui, "mana", |ui| {
                ui.add(egui::DragValue::new(amount).range(-999..=999)).on_hover_text("Negative drains mana");
            });
        }
        Action::DamageAround { damage, radius } => {
            labeled(ui, "damage", |ui| {
                ui.add(egui::DragValue::new(damage).range(0..=999));
            });
            labeled(ui, "radius", |ui| {
                ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1).suffix(" tiles"));
            });
        }
        Action::Aim { function } => {
            labeled(ui, "then call", |ui| suggest(ui, id, function, &refs.functions, 130.0));
            ui.weak("The player clicks a target; the script function gets it as (x, y) in tiles.");
        }
        Action::Fly { sprite, x, y, speed, function } => {
            fx_sprite_ui(ui, id, sprite, refs, false);
            tile_point_ui(ui, "to", x, y);
            labeled(ui, "speed", |ui| {
                ui.add(egui::DragValue::new(speed).range(0.5..=60.0).speed(0.1).suffix(" tiles/s"));
            });
            labeled(ui, "then call", |ui| suggest(ui, &format!("{id}/fn"), function, &refs.functions, 130.0));
        }
        Action::Effect { sprite, x, y } => {
            fx_sprite_ui(ui, id, sprite, refs, true);
            tile_point_ui(ui, "at", x, y);
        }
        Action::DamageArea { x, y, radius, damage } => {
            tile_point_ui(ui, "at", x, y);
            labeled(ui, "radius", |ui| {
                ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1).suffix(" tiles"));
            });
            labeled(ui, "damage", |ui| {
                ui.add(egui::DragValue::new(damage).range(0..=999));
            });
        }
        Action::PlayerLight { radius, seconds, color, flicker } => {
            labeled(ui, "radius", |ui| {
                ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1).suffix(" tiles"));
            });
            labeled(ui, "for", |ui| {
                ui.add(egui::DragValue::new(seconds).range(0.5..=3600.0).speed(0.5).suffix(" s"));
            });
            labeled(ui, "color", |ui| {
                ui.color_edit_button_srgb(color);
                ui.checkbox(flicker, "flicker");
            });
        }
        Action::LightAt { x, y, radius, seconds, color, flicker } => {
            tile_point_ui(ui, "at", x, y);
            labeled(ui, "radius", |ui| {
                ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1).suffix(" tiles"));
            });
            labeled(ui, "for", |ui| {
                ui.add(egui::DragValue::new(seconds).range(0.1..=3600.0).speed(0.2).suffix(" s")).on_hover_text("Fades out during the last second");
            });
            labeled(ui, "color", |ui| {
                ui.color_edit_button_srgb(color);
                ui.checkbox(flicker, "flicker");
            });
        }
        Action::HealArea { x, y, radius, amount } => {
            tile_point_ui(ui, "at", x, y);
            labeled(ui, "radius", |ui| {
                ui.add(egui::DragValue::new(radius).range(0.5..=32.0).speed(0.1).suffix(" tiles"));
            });
            labeled(ui, "heal", |ui| {
                ui.add(egui::DragValue::new(amount).range(1..=999).suffix(" HP")).on_hover_text("The player and friendly characters in range");
            });
        }
        Action::SetTime { hour } => {
            labeled(ui, "time", |ui| {
                ui.add(egui::DragValue::new(hour).range(0.0..=24.0).speed(0.25).suffix(" h")).on_hover_text("Hours 0–24, e.g. 22 = 10 pm");
            });
        }
        Action::Quest { quest, op, stage } => {
            labeled(ui, "quest", |ui| suggest(ui, &format!("{id}/q"), quest, &refs.quests, 120.0));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(format!("{id}/qop")).selected_text(op.label()).show_ui(ui, |ui| {
                    for o in rustpg2d_engine::quest::QuestOp::ALL {
                        ui.selectable_value(op, o, o.label());
                    }
                });
                if *op == rustpg2d_engine::quest::QuestOp::SetStage {
                    quest_stage_field(ui, &format!("{id}/qs"), quest, stage, refs);
                }
            });
            if !quest.is_empty() && !refs.quests.contains(quest) {
                ui.colored_label(Color32::LIGHT_RED, "unknown quest (Database \u{E13A} Quests)");
            }
        }
        Action::Patrol { target, active, points } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.radio_value(active, true, "start");
                ui.radio_value(active, false, "stop");
            });
            if *active {
                if points.is_empty() {
                    ui.weak("walks the character's own route (map inspector \u{E13A} Patrol route)");
                } else {
                    let list: Vec<String> = points.iter().map(|(x, y)| format!("{x},{y}")).collect();
                    ui.horizontal(|ui| {
                        ui.weak(format!("route: {}", list.join(" \u{E13A} ")));
                        if ui.small_button("use own route").clicked() {
                            points.clear();
                        }
                    });
                }
            }
        }
        Action::OpenShop { target } => {
            target_field(ui, id, target, refs);
            ui.weak("(a merchant character)");
        }
        Action::SetFrame { target, frame } => {
            target_field(ui, id, target, refs);
            labeled(ui, "frame offset", |ui| {
                ui.add(egui::DragValue::new(frame).range(0..=255));
            });
        }
        Action::Cutscene { function } => {
            ui.horizontal(|ui| {
                labeled(ui, "function", |ui| suggest(ui, id, function, &refs.functions, 130.0));
                if ui.add_enabled(!function.is_empty(), egui::Button::new("\u{E3D0} Preview")).on_hover_text("Play-test just this cut-scene on the open map (saves first)").clicked() {
                    request_play_cutscene(function);
                }
            });
            ui.weak("Plays the script function as a cut-scene: the HUD hides, black bars slide in, everyone else waits until its walks, dialogs and waits are done (Scripts tab: + Cutscene template).");
        }
        Action::Dim { amount, seconds } => {
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(amount, 0.0..=1.0).text("dark"));
                ui.label("over");
                ui.add(egui::DragValue::new(seconds).range(0.0..=30.0).speed(0.1).suffix(" s"));
            });
            ui.weak("0 = normal, 1 = deep night: makes lights show by day (e.g. in a cut-scene, which brightens again when it ends). Not saved.");
        }
        Action::SetSpawners { map, zone, active } => {
            labeled(ui, "map", |ui| {
                suggest(ui, &format!("{id}/map"), map, &refs.maps, 110.0);
                ui.weak("(empty = this map)");
            });
            labeled(ui, "spawner zone", |ui| {
                ui.add(egui::TextEdit::singleline(zone).hint_text("all").desired_width(110.0));
            });
            ui.horizontal(|ui| {
                ui.radio_value(active, false, "Off");
                ui.radio_value(active, true, "On");
            });
            ui.weak("Off: no more enemies from these spawners (theirs on the current map vanish). Saved.");
        }
        Action::DefeatEnemies => {
            ui.weak("Every hostile character on the current map is defeated at once (loot drops, their 'Defeated' triggers run).");
        }
        Action::GiveXp { amount } => {
            labeled(ui, "experience", |ui| {
                ui.add(egui::DragValue::new(amount).range(-1_000_000..=1_000_000))
                    .on_hover_text("Experience points for the player (Project › Experience and levels). Negative takes them away; the level never drops.");
            });
        }
        Action::ApplyEffect { target, effect } => {
            target_field(ui, id, target, refs);
            effect_spec_ui(ui, &format!("{id}/fx"), effect);
        }
        Action::CureEffect { target, kind } => {
            target_field(ui, id, target, refs);
            labeled(ui, "effect", |ui| {
                let text = kind.map_or("all effects", |k| k.label());
                egui::ComboBox::from_id_salt(format!("{id}/cure")).selected_text(text).show_ui(ui, |ui| {
                    ui.selectable_value(kind, None, "all effects");
                    for k in EffectKind::ALL {
                        ui.selectable_value(kind, Some(k), k.label());
                    }
                });
            });
        }
        Action::Shake { seconds, strength } => {
            labeled(ui, "shake for", |ui| {
                ui.add(egui::DragValue::new(seconds).range(0.05..=10.0).speed(0.05).suffix(" s"));
                ui.label("strength");
                ui.add(egui::DragValue::new(strength).range(0.1..=5.0).speed(0.05).prefix("x "))
                    .on_hover_text("1 = like a hit on the player. Players can switch shaking off in the Options.");
            });
        }
        Action::SetWeather { map, weather } => {
            use rustpg2d_engine::map::{Weather, WeatherKind};
            labeled(ui, "map", |ui| {
                let mut opts = vec![String::new()];
                opts.extend(refs.maps.iter().cloned());
                suggest(ui, &format!("{id}/wmap"), map, &opts, 120.0);
            });
            ui.weak("(empty map = the map the player is on)");
            labeled(ui, "weather", |ui| {
                let current = weather.map_or("Map's own", |w| w.kind.label());
                egui::ComboBox::from_id_salt(format!("{id}/wkind")).selected_text(current).show_ui(ui, |ui| {
                    if ui.selectable_label(weather.is_none(), "Map's own").on_hover_text("Back to the weather set on the map").clicked() {
                        *weather = None;
                    }
                    for k in WeatherKind::ALL {
                        let on = weather.is_some_and(|w| w.kind == k);
                        if ui.selectable_label(on, k.label()).clicked() {
                            *weather = Some(Weather::new(k, weather.map_or(0.6, |w| w.intensity)));
                        }
                    }
                });
                if let Some(w) = weather.as_mut().filter(|w| w.kind != WeatherKind::None) {
                    let mut pct = (w.intensity * 100.0).round();
                    if ui.add(egui::Slider::new(&mut pct, 5.0..=100.0).suffix(" %")).changed() {
                        w.intensity = pct / 100.0;
                    }
                }
            });
            ui.weak("Kept in the save game: the map keeps this weather until it is set again.");
        }
        Action::Sleep { hours } => {
            ui.horizontal(|ui| {
                ui.label("hours");
                ui.add(egui::DragValue::new(hours).range(0.25..=48.0).speed(0.25));
            });
            ui.weak("the player sleeps (camping, an inn's rest): the screen fades, the clock moves on, HP and mana are restored; 'Woke up' triggers run after. Not with enemies nearby.");
        }
        Action::Lockpick { difficulty, var } => {
            ui.horizontal(|ui| {
                ui.label("difficulty");
                ui.add(egui::DragValue::new(difficulty).range(0..=10)).on_hover_text("Like a lock's 'Lockpicking' difficulty: higher = a smaller sweet spot and a faster marker (the player's lockpicking skill makes it easier)");
            });
            labeled(ui, "result into", |ui| suggest(ui, &format!("{id}/lpvar"), var, &refs.vars, 120.0));
            ui.weak("the lockpicking game; 1 (picked) or 0 goes into the variable, test it with a Variable condition afterwards");
        }
        Action::UnlockAchievement { id: ach } => {
            labeled(ui, "achievement", |ui| suggest(ui, &format!("{id}/ach"), ach, &refs.achievements, 140.0));
            ui.weak("unlocks it for the player (kept across save games; Database \u{203A} Achievements)");
        }
        Action::OpenCrafting { station, title } => {
            labeled(ui, "station", |ui| suggest(ui, &format!("{id}/station"), station, &refs.stations, 120.0));
            labeled(ui, "title", |ui| {
                ui.add(egui::TextEdit::singleline(title).desired_width(140.0).hint_text("Crafting"));
            });
            ui.weak("shows the recipes of this station (Database \u{203A} Recipes); empty station = recipes without one");
        }
        Action::Companion { target, join } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.radio_value(join, true, "joins");
                ui.radio_value(join, false, "leaves");
            });
            ui.weak(if *join {
                "a placed, friendly character follows the player from map to map and fights hostile characters near them (saved)"
            } else {
                "the companion stays where it is on its own map, or goes back to where it is placed"
            });
        }
        Action::Schedule { target, active } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.radio_value(active, true, "follow");
                ui.radio_value(active, false, "stop");
            });
            ui.weak(if *active {
                "the character heads for the stop of the current hour again (map inspector: Daily schedule)"
            } else {
                "the character stays where it is and does what its definition says (kept in the save game)"
            });
        }
        Action::Autosave => {
            ui.weak("Saves the game to the 'Autosave' slot once this event (dialogs, cut-scene) has finished. Map changes autosave by themselves.");
        }
        Action::EndCutscene => {
            ui.weak("(ends the cut-scene; added automatically after a cut-scene's actions)");
        }
        Action::WalkTo { target, x, y } | Action::Place { target, x, y } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.label("tile x");
                ui.add(egui::DragValue::new(x).range(0..=4096));
                ui.label("y");
                ui.add(egui::DragValue::new(y).range(0..=4096));
            });
        }
        Action::Approach { target, to } => {
            target_field(ui, id, target, refs);
            labeled(ui, "walks up to", |ui| suggest(ui, &format!("{id}/to"), to, &refs.targets, 100.0));
            ui.weak("(stops a tile before it; then both face each other)");
        }
        Action::Move { target, dx, dy } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                ui.label("by tiles dx");
                ui.add(egui::DragValue::new(dx).range(-64..=64));
                ui.label("dy");
                ui.add(egui::DragValue::new(dy).range(-64..=64));
            });
        }
        Action::Face { target, facing } => {
            target_field(ui, id, target, refs);
            egui::ComboBox::from_id_salt(format!("{id}/f")).selected_text(facing.label()).show_ui(ui, |ui| {
                for f in Facing::ALL {
                    ui.selectable_value(facing, f, f.label());
                }
            });
        }
        Action::Wait { seconds } => {
            labeled(ui, "seconds", |ui| {
                ui.add(egui::DragValue::new(seconds).range(0.0..=60.0).speed(0.05));
            });
        }
        Action::Emit { name } => {
            labeled(ui, "event", |ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(140.0));
            });
        }
        Action::RunScript { function } => {
            labeled(ui, "function", |ui| suggest(ui, id, function, &refs.functions, 130.0));
        }
        Action::PlaySound { sound } | Action::PlayMusic { sound } => {
            labeled(ui, "sound", |ui| suggest(ui, id, sound, &refs.sounds, 130.0));
            if matches!(a, Action::PlayMusic { .. }) {
                ui.weak("(empty = stop music)");
            }
        }
        Action::SetLocked { target, locked } => {
            target_field(ui, id, target, refs);
            ui.checkbox(locked, "locked");
            ui.weak("(unlocking applies the lock's effect and runs 'Unlocked' triggers; locking closes it again and runs 'Locked again' triggers)");
        }
        Action::Heal { amount } => {
            labeled(ui, "hit points", |ui| {
                ui.add(egui::DragValue::new(amount).range(-99..=99));
            });
            ui.weak("(negative = damage; on a healing item set to 'Use on: Player or companion' it heals whom the player chooses)");
        }
        Action::Emote { target, emotion, seconds, wait } => {
            target_field(ui, id, target, refs);
            labeled(ui, "emotion", |ui| suggest(ui, &format!("{id}/emo"), emotion, &refs.emotions, 120.0));
            labeled(ui, "seconds", |ui| {
                ui.add(egui::DragValue::new(seconds).range(-1.0..=60.0).speed(0.1)).on_hover_text("0 = 2.5 s; -1 = until 'Clear emotion'");
            });
            ui.checkbox(wait, "wait until it is over");
            if refs.emotions.is_empty() {
                ui.weak("(no emotions yet: add a sprite sheet of the kind Emotions; its animations are the emotions)");
            }
        }
        Action::ClearEmote { target } => {
            target_field(ui, id, target, refs);
        }
        Action::Carry { target, prop } => {
            target_field(ui, id, target, refs);
            labeled(ui, "prop", |ui| suggest(ui, &format!("{id}/carry"), prop, &refs.props, 120.0));
            ui.weak("(carried in both hands, drawn in the character's hands; empty = puts it down. While carrying it holds nothing else and the player can't attack)");
        }
        Action::ChangeStanding { faction, amount } => {
            labeled(ui, "faction", |ui| suggest(ui, &format!("{id}/fac"), faction, &refs.factions, 120.0));
            labeled(ui, "by", |ui| {
                ui.add(egui::DragValue::new(amount).range(-200..=200)).on_hover_text("Negative lowers it");
            });
            ui.weak("(the player's standing, -100..100: at -20 or below the faction's characters attack the player; Database › Project › Factions)");
        }
        Action::Mood { target, mood } => {
            target_field(ui, id, target, refs);
            labeled(ui, "mood", |ui| {
                ui.add(egui::TextEdit::singleline(mood).desired_width(120.0).hint_text("worried"));
            });
            ui.weak("(the portrait of that mood, from the character's definition, until changed or the event ends; empty = its usual portrait)");
        }
        Action::ShowImage { sheet, wait, scroll, speed, scale } => {
            labeled(ui, "picture", |ui| suggest(ui, &format!("{id}/sheet"), sheet, &refs.sheets, 140.0));
            ui.weak("(a sprite sheet, drawn whole: Database › Sprite sheets)");
            labeled(ui, "", |ui| {
                ui.checkbox(wait, "Wait until closed").on_hover_text(
                    "On: the event goes on once the player closes the picture (E, click or Esc). Off: it stays under the following dialogs until 'Hide image' or the end of the event",
                );
                ui.checkbox(scroll, "Scroll").on_hover_text("A picture taller than the window keeps the window's width and pans down and back up; the player can scroll it with the arrow keys or the mouse wheel");
            });
            labeled(ui, "size", |ui| {
                ui.add(egui::DragValue::new(scale).range(0.05..=16.0).speed(0.05).prefix("x "))
                    .on_hover_text("1 = the picture's own pixels (painted art); 2, 3 … for small pixel art. Too large for the window: scaled down to fit, aspect ratio kept");
                if *scroll {
                    ui.label("speed");
                    ui.add(egui::DragValue::new(speed).range(1.0..=2000.0).speed(1.0).suffix(" px/s"));
                }
            });
        }
        Action::HideImage => {
            ui.weak("(takes away the picture of 'Show image')");
        }
        Action::ShopStock { target, item, op, count, price } => {
            target_field(ui, id, target, refs);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(format!("{id}/op")).width(60.0).selected_text(if *op == VarOp::Add { "add" } else { "set to" }).show_ui(ui, |ui| {
                    ui.selectable_value(op, VarOp::Add, "add");
                    ui.selectable_value(op, VarOp::Set, "set to");
                });
                ui.add(egui::DragValue::new(count).range(0..=9999));
                ui.label("x");
                suggest(ui, &format!("{id}/it"), item, &refs.items, 110.0);
            });
            labeled(ui, "price", |ui| {
                ui.add(egui::DragValue::new(price).range(0..=99999)).on_hover_text("For a new line in the stock; 0 = the item's value");
            });
            ui.weak("(a merchant's stock: what villagers produce reaches their stall; 'set to' 0 removes the item)");
        }
        Action::OpenCompanion { target } => {
            target_field(ui, id, target, refs);
            ui.weak("(a companion: its window with its gear and bag opens after the event, e.g. after a dialog's \"Let me see your gear\")");
        }
        Action::CompanionOrder { target, order } => {
            target_field(ui, id, target, refs);
            labeled(ui, "order", |ui| {
                egui::ComboBox::from_id_salt(format!("{id}/order")).selected_text(order.as_str()).show_ui(ui, |ui| {
                    for o in ["attack", "defend", "passive", "stay", "follow"] {
                        ui.selectable_value(order, o.to_string(), o);
                    }
                });
            });
            ui.weak("(a companion by name, \"self\" or \"all\"; attack / defend / passive = how it fights, stay = wait here, follow = come along again)");
        }
        Action::HealCharacter { target, amount } => {
            target_field(ui, id, target, refs);
            labeled(ui, "hit points", |ui| {
                ui.add(egui::DragValue::new(amount).range(-99..=99));
            });
            ui.weak("(\"player\", \"self\" or a character's name; negative = damage)");
        }
        Action::If { conditions, then, otherwise } => {
            ui.label("If all of:");
            ui.indent(format!("{id}/ic"), |ui| conditions_ui(ui, id, conditions, refs));
            ui.label("Then:");
            ui.indent(format!("{id}/it"), |ui| actions_ui(ui, &format!("{id}/then"), then, refs));
            ui.label("Else:");
            ui.indent(format!("{id}/ie"), |ui| actions_ui(ui, &format!("{id}/else"), otherwise, refs));
        }
    }
}

// ---------------------------------------------------------------- visual sprite chooser

thread_local! {
    /// Debug-script request to open the chooser of the field with this id.
    static OPEN_CHOOSER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// A cut-scene to preview (the *Preview* button of a *Play cutscene* action).
    static PLAY_CUTSCENE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Asks the editor to play-test script function `function` as a cut-scene.
pub fn request_play_cutscene(function: &str) {
    PLAY_CUTSCENE.with(|r| *r.borrow_mut() = Some(function.to_string()));
}

/// The cut-scene preview requested this frame, if any.
pub fn take_play_cutscene() -> Option<String> {
    PLAY_CUTSCENE.with(|r| r.borrow_mut().take())
}

/// Opens the sprite chooser of field `id` the next time it is shown (editor debug scripts).
pub fn request_chooser(id: &str) {
    OPEN_CHOOSER.with(|r| *r.borrow_mut() = Some(id.to_string()));
}

/// State of an open "Choose a sprite" window (kept in egui memory per field).
#[derive(Clone, Default)]
struct Chooser {
    open: bool,
    sheet: String,
    pick: SpriteRef,
    filter: String,
    zoom: f32,
}

/// Who uses which frames of a sheet: frame → names (item icons, prop states).
fn frame_users(db: &rustpg2d_engine::defs::Database, sheet: &str, columns: u32) -> HashMap<u32, Vec<String>> {
    let mut out: HashMap<u32, Vec<String>> = HashMap::new();
    let mut add = |s: &SpriteRef, who: String| {
        if s.sheet != sheet {
            return;
        }
        let (w, h) = s.cells();
        for dy in 0..h {
            for dx in 0..w {
                out.entry(s.frame + dy * columns.max(1) + dx).or_default().push(who.clone());
            }
        }
    };
    for d in &db.items {
        add(&d.icon, format!("item {}", d.name));
    }
    for d in &db.props {
        add(&d.sprite, format!("prop {} ({})", d.name, d.default_state));
        for st in &d.states {
            add(&st.sprite, format!("prop {} ({})", d.name, st.name));
        }
    }
    out
}

/// A sprite as a large preview with a "Choose…" button that opens a visual chooser: all sheets
/// as thumbnails, the chosen sheet as a zoomable grid (frame numbers and users on hover),
/// click to pick (drag for several cells if `multi`), double-click to pick and close.
#[allow(clippy::too_many_arguments)]
pub fn sprite_field(ui: &mut Ui, id: &str, sprite: &mut SpriteRef, tex: &Textures, assets: &Assets, db: &rustpg2d_engine::defs::Database, multi: bool) -> bool {
    let key = egui::Id::new(("sprite_chooser", id));
    let mut st: Chooser = ui.ctx().data(|d| d.get_temp(key)).unwrap_or_default();
    let requested = OPEN_CHOOSER.with(|r| r.borrow().as_deref() == Some(id));
    if requested {
        OPEN_CHOOSER.with(|r| *r.borrow_mut() = None);
    }
    let mut changed = false;
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::click());
        ui.painter().rect_filled(r, 4.0, Color32::from_gray(40));
        let drawn = !sprite.sheet.is_empty() && paint_sprite(ui, tex, assets, sprite, r.shrink(4.0));
        if !drawn {
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "none", egui::FontId::proportional(14.0), Color32::GRAY);
        }
        ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0_f32, Color32::from_gray(80)), StrokeKind::Inside);
        ui.vertical(|ui| {
            if sprite.sheet.is_empty() {
                ui.weak("No sprite chosen yet.");
            } else {
                let (w, h) = sprite.cells();
                let size = if (w, h) == (1, 1) { String::new() } else { format!(" · {w}×{h} cells") };
                ui.label(format!("{} · frame {}{size}", sprite.sheet, sprite.frame));
            }
            let open_it = ui.button("\u{E2CA} Choose…").on_hover_text("Pick a frame visually from any sprite sheet").clicked() || resp.double_clicked() || requested;
            if open_it {
                st.open = true;
                st.pick = sprite.clone();
                st.sheet = if assets.sheets.contains_key(&sprite.sheet) {
                    sprite.sheet.clone()
                } else {
                    // Default to a sheet that looks right for the field, else the first one.
                    let mut ids: Vec<&String> = assets.sheets.keys().collect();
                    ids.sort();
                    ids.iter().find(|s| s.contains("item")).or(ids.first()).map(|s| s.to_string()).unwrap_or_default()
                };
                st.zoom = 0.0;
            }
        });
    });
    if st.open {
        let mut open = true;
        let mut done: Option<bool> = None;
        egui::Window::new("Choose a sprite").id(key.with("win")).open(&mut open).default_size([900.0, 560.0]).max_height(620.0).collapsible(false).resizable(true).show(ui.ctx(), |ui| {
            ui.set_min_width(860.0);
            // The current pick and the buttons first, so they never scroll out of view.
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                if !st.pick.sheet.is_empty() {
                    paint_sprite(ui, tex, assets, &st.pick, r.shrink(3.0));
                    ui.label(format!("{} · frame {}", st.pick.sheet, st.pick.frame));
                }
                if ui.add_enabled(!st.pick.sheet.is_empty(), egui::Button::new("\u{E182} Use this")).clicked() {
                    done = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    done = Some(false);
                }
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                // Sheets.
                ui.vertical(|ui| {
                    ui.set_width(200.0);
                    ui.add(egui::TextEdit::singleline(&mut st.filter).hint_text("\u{E30C} search sheets").desired_width(190.0));
                    egui::ScrollArea::vertical().id_salt(key.with("sheets")).max_height(440.0).show(ui, |ui| {
                        let mut ids: Vec<&String> = assets.sheets.keys().collect();
                        ids.sort();
                        let q = st.filter.to_lowercase();
                        for sid in ids.into_iter().filter(|s| q.is_empty() || s.to_lowercase().contains(&q)) {
                            let s = &assets.sheets[sid];
                            let on = st.sheet == *sid;
                            let row = ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                                ui.painter().rect_filled(r, 2.0, Color32::from_gray(45));
                                paint_frame(ui, tex, assets, sid, 0, r.shrink(2.0));
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(sid.as_str()).strong().color(if on { Color32::YELLOW } else { Color32::WHITE }));
                                    ui.weak(format!("{} frames · {}×{}{}", s.count, s.frame_w as u32, s.frame_h as u32, if s.scale > 1.0 { format!(" · {}×", s.scale) } else { String::new() }));
                                });
                            });
                            let resp = ui.interact(row.response.rect, key.with(("sheet", sid.as_str())), Sense::click());
                            if on {
                                ui.painter().rect_stroke(row.response.rect.expand(1.0), 3.0, Stroke::new(1.5_f32, Color32::YELLOW), StrokeKind::Outside);
                            }
                            if resp.clicked() {
                                st.sheet = sid.clone();
                                st.zoom = 0.0; // fit the new sheet
                            }
                        }
                    });
                });
                ui.separator();
                // The chosen sheet's frames.
                ui.vertical(|ui| {
                    // Fit the sheet's width (1x-4x) when it is first shown (thumbnails: 2).
                    if st.zoom <= 0.0 && !multi {
                        st.zoom = 2.0;
                    }
                    if st.zoom <= 0.0 {
                        // (World width: a 2x sheet is shown as big as a 1x one.)
                        let w = assets.sheets.get(&st.sheet).map_or(256.0, |s| s.image.width as f32 / s.scale);
                        st.zoom = ((ui.available_width() - 20.0) / w).floor().clamp(1.0, 4.0);
                    }
                    ui.horizontal(|ui| {
                        ui.label("Zoom");
                        ui.add(egui::Slider::new(&mut st.zoom, 1.0..=4.0).step_by(1.0));
                        ui.weak(if multi { "click a frame · drag for several cells · double-click to use it" } else { "click a frame · double-click to use it" });
                    });
                    let Some(s) = assets.sheets.get(&st.sheet) else {
                        ui.weak("Choose a sheet on the left.");
                        return;
                    };
                    let Some(t) = tex.sheets.get(&st.sheet) else { return };
                    let users = frame_users(db, &st.sheet, s.columns);
                    if !multi {
                        // Single frames: big wrapped thumbnails (readable even for long strips).
                        let cell = 28.0 + 16.0 * st.zoom;
                        egui::ScrollArea::vertical().id_salt(key.with("thumbs")).max_height(420.0).show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                                for f in 0..s.count {
                                    let (r, resp) = ui.allocate_exact_size(vec2(cell, cell + 12.0), Sense::click());
                                    let on = st.pick.sheet == st.sheet && st.pick.frame == f;
                                    let img = Rect::from_min_size(r.min, vec2(cell, cell));
                                    ui.painter().rect_filled(img, 3.0, if resp.hovered() { Color32::from_gray(70) } else { Color32::from_gray(50) });
                                    paint_frame(ui, tex, assets, &st.sheet, f, img.shrink(3.0));
                                    if users.contains_key(&f) {
                                        ui.painter().circle_filled(img.right_top() + vec2(-6.0, 6.0), 3.0, Color32::from_rgb(120, 200, 255));
                                    }
                                    ui.painter().text(pos2(r.center().x, r.max.y - 5.0), egui::Align2::CENTER_CENTER, f.to_string(), egui::FontId::monospace(10.0), Color32::GRAY);
                                    if on {
                                        ui.painter().rect_stroke(img, 3.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
                                    }
                                    let who = users.get(&f).map(|v| format!("\nused by: {}", v.join(", "))).unwrap_or_default();
                                    let resp = resp.on_hover_text(format!("frame {f}{who}"));
                                    if resp.clicked() || resp.double_clicked() {
                                        st.pick = SpriteRef::new(&st.sheet, f);
                                    }
                                    if resp.double_clicked() {
                                        done = Some(true);
                                    }
                                }
                            });
                        });
                        return;
                    }
                    let rows = s.count.div_ceil(s.columns.max(1));
                    // Zoom in world pixels: a high-resolution sheet shows at the same size, with its detail.
                    let z = st.zoom / s.scale;
                    let (cw, ch) = (s.frame_w * z, s.frame_h * z);
                    let size = vec2(s.image.width as f32 * z, s.image.height as f32 * z);
                    egui::ScrollArea::both().id_salt(key.with("grid")).max_height(420.0).max_width(ui.available_width()).show(ui, |ui| {
                        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
                        let p = ui.painter_at(rect);
                        // Checkerboard-ish backdrop so transparent pixels read.
                        p.rect_filled(rect, 0.0, Color32::from_gray(55));
                        for r in 0..rows {
                            for c in 0..s.columns {
                                if (r + c) % 2 == 0 {
                                    p.rect_filled(Rect::from_min_size(rect.min + vec2(c as f32 * cw, r as f32 * ch), vec2(cw, ch)), 0.0, Color32::from_gray(65));
                                }
                            }
                        }
                        p.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        for c in 1..s.columns {
                            let x = rect.min.x + c as f32 * cw;
                            p.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], Stroke::new(1.0_f32, Color32::from_white_alpha(30)));
                        }
                        for r in 1..rows {
                            let y = rect.min.y + r as f32 * ch;
                            p.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], Stroke::new(1.0_f32, Color32::from_white_alpha(30)));
                        }
                        // Frames already used by other definitions get a small marker.
                        for f in users.keys() {
                            let (c, r) = (f % s.columns.max(1), f / s.columns.max(1));
                            let m = rect.min + vec2(c as f32 * cw + cw - 7.0, r as f32 * ch + 3.0);
                            p.circle_filled(m + vec2(2.0, 2.0), 3.0, Color32::from_rgb(120, 200, 255));
                        }
                        let cell_at = |pos: egui::Pos2| {
                            let rel = pos - rect.min;
                            (((rel.x / cw).floor().max(0.0) as u32).min(s.columns - 1), ((rel.y / ch).floor().max(0.0) as u32).min(rows.max(1) - 1))
                        };
                        if let Some(h) = resp.hover_pos() {
                            let (c, r) = cell_at(h);
                            let f = r * s.columns + c;
                            p.rect_stroke(Rect::from_min_size(rect.min + vec2(c as f32 * cw, r as f32 * ch), vec2(cw, ch)), 0.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
                            let who = users.get(&f).map(|v| format!("\nused by: {}", v.join(", "))).unwrap_or_default();
                            resp.clone().on_hover_text(format!("frame {f}{who}"));
                        }
                        let drag_id = key.with("drag");
                        if resp.drag_started() || resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                ui.memory_mut(|m| m.data.insert_temp(drag_id, cell_at(pos)));
                            }
                        }
                        if resp.dragged() || resp.clicked() || resp.double_clicked() {
                            if let (Some(pos), Some(start)) = (resp.interact_pointer_pos(), ui.memory(|m| m.data.get_temp::<(u32, u32)>(drag_id))) {
                                let end = if multi { cell_at(pos) } else { start };
                                let (x0, y0) = (start.0.min(end.0), start.1.min(end.1));
                                st.pick = SpriteRef { sheet: st.sheet.clone(), frame: y0 * s.columns + x0, w: start.0.abs_diff(end.0) + 1, h: start.1.abs_diff(end.1) + 1 };
                            }
                        }
                        if resp.double_clicked() {
                            done = Some(true);
                        }
                        if st.pick.sheet == st.sheet {
                            let (c, r) = (st.pick.frame % s.columns.max(1), st.pick.frame / s.columns.max(1));
                            let (w, h) = st.pick.cells();
                            let sel = Rect::from_min_size(rect.min + vec2(c as f32 * cw, r as f32 * ch), vec2(w as f32 * cw, h as f32 * ch));
                            p.rect_stroke(sel, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
                        }
                    });
                });
            });
        });
        if done == Some(true) && !st.pick.sheet.is_empty() {
            if *sprite != st.pick {
                *sprite = st.pick.clone();
                changed = true;
            }
            st.open = false;
        } else if done == Some(false) || !open {
            st.open = false;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, st));
    changed
}

/// Edits an optional light: on / off, radius (tiles), color, strength, flicker and presets.
/// Returns true if it changed.
pub fn light_ui(ui: &mut Ui, id: &str, l: &mut Option<rustpg2d_engine::defs::LightSpec>, label: &str) -> bool {
    use rustpg2d_engine::defs::LightSpec;
    let before = l.clone();
    let mut on = l.is_some();
    if ui.checkbox(&mut on, label).changed() {
        *l = on.then(LightSpec::default);
    }
    if let Some(spec) = l {
        ui.horizontal_wrapped(|ui| {
            ui.label("radius");
            ui.add(egui::DragValue::new(&mut spec.radius).range(0.5..=24.0).speed(0.1).suffix(" tiles"));
            ui.label("color");
            egui::color_picker::color_edit_button_srgb(ui, &mut spec.color);
            ui.label("strength");
            let mut pct = (spec.intensity * 100.0).round();
            if ui.add(egui::DragValue::new(&mut pct).range(0.0..=100.0).suffix(" %")).changed() {
                spec.intensity = pct / 100.0;
            }
            ui.checkbox(&mut spec.flicker, "flickers").on_hover_text("Like a flame (torches, campfires)");
        });
        ui.horizontal(|ui| {
            ui.weak("presets:");
            let presets = [
                ("Torch", LightSpec::torch(4.0)),
                ("Lantern", LightSpec::new(5.5, [255, 220, 170], false)),
                ("Campfire", LightSpec::torch(6.0)),
                ("Window", LightSpec { intensity: 0.7, ..LightSpec::new(1.8, [255, 205, 130], false) }),
                ("Magic", LightSpec { intensity: 0.85, ..LightSpec::new(4.0, [150, 200, 255], false) }),
            ];
            for (name, p) in presets {
                if ui.small_button(name).clicked() {
                    *spec = p;
                }
            }
        });
        let _ = id;
    }
    *l != before
}

// ------------------------------------------------------------------ characters

/// Skin tones offered as one-click presets (colour features).
pub const SKIN_PRESETS: [[u8; 3]; 8] = [[0xf6, 0xd0, 0xb0], [0xf2, 0xc4, 0x9b], [0xe8, 0xb8, 0x8a], [0xd0, 0x90, 0x68], [0xb5, 0x79, 0x4c], [0x8d, 0x5a, 0x3b], [0x5a, 0x3a, 0x24], [0x7c, 0xb3, 0x42]];
/// Hair colours offered as presets (colour features).
pub const HAIR_PRESETS: [[u8; 3]; 10] = [
    [0x1e, 0x16, 0x12],
    [0x3a, 0x24, 0x18],
    [0x5d, 0x3a, 0x1a],
    [0x8a, 0x5a, 0x2e],
    [0xe0, 0xc0, 0x60],
    [0xd3, 0x54, 0x00],
    [0xc0, 0x39, 0x2b],
    [0xde, 0xde, 0xde],
    [0x3b, 0x3b, 0x9e],
    [0x2e, 0x8b, 0x57],
];

/// Clothing colours offered as presets (colour features).
pub const CLOTH_PRESETS: [[u8; 3]; 10] = [
    [0x2f, 0x4f, 0x8f],
    [0x8f, 0x2f, 0x2f],
    [0x2f, 0x6f, 0x3f],
    [0x6f, 0x4f, 0x2f],
    [0x5f, 0x2f, 0x6f],
    [0xc8, 0xa0, 0x30],
    [0x30, 0x30, 0x38],
    [0xd8, 0xd0, 0xc0],
    [0x2f, 0x7f, 0x8f],
    [0xb0, 0x5a, 0x20],
];

/// List editor of carried items (count + item).
pub fn carried_ui(ui: &mut Ui, id: &str, inv: &mut Vec<ItemStack>, items: &[String]) {
    let mut remove = None;
    for (k, s) in inv.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut s.count).range(1..=9999));
            ui.label("x");
            suggest(ui, &format!("{id}/inv{k}"), &mut s.item, items, 130.0);
            if ui.small_button("\u{E4F6}").on_hover_text("Remove").clicked() {
                remove = Some(k);
            }
        });
    }
    if let Some(k) = remove {
        inv.remove(k);
    }
    if ui.small_button("+ Item").clicked() {
        inv.push(ItemStack::new(items.first().map_or("", |s| s.as_str()), 1));
    }
}

/// The four directions of a character, side by side (idle frames).
pub fn character_preview(ui: &mut Ui, assets: &Assets, a: &rustpg2d_engine::defs::Appearance) {
    let a = assets.models.look_of(a);
    ui.horizontal(|ui| {
        for f in [Facing::Down, Facing::Left, Facing::Up, Facing::Right] {
            let (rect, _) = ui.allocate_exact_size(vec2(64.0, 96.0), Sense::hover());
            ui.painter().rect_filled(rect, 3.0, Color32::from_gray(36));
            if !paint_character(ui, assets, &a, f, rect.shrink(2.0)) {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::default(), Color32::RED);
            }
        }
    });
}

/// A combo box of the status effect kinds.
pub fn effect_kind_combo(ui: &mut egui::Ui, id: &str, kind: &mut EffectKind) -> bool {
    let before = *kind;
    egui::ComboBox::from_id_salt(id).selected_text(kind.label()).show_ui(ui, |ui| {
        for k in EffectKind::ALL {
            ui.selectable_value(kind, k, k.label()).on_hover_text(k.unit());
        }
    });
    *kind != before
}

/// A status effect: kind, strength (unless it has none) and duration.
pub fn effect_spec_ui(ui: &mut egui::Ui, id: &str, spec: &mut EffectSpec) {
    ui.horizontal(|ui| {
        if effect_kind_combo(ui, id, &mut spec.kind) {
            *spec = EffectSpec { seconds: spec.seconds, ..EffectSpec::template(spec.kind) };
        }
        if spec.kind != EffectKind::Stun {
            ui.add(egui::DragValue::new(&mut spec.strength).range(0.0..=1000.0).speed(0.1)).on_hover_text(spec.kind.unit());
            ui.weak(match spec.kind {
                EffectKind::Poison | EffectKind::Burn | EffectKind::Regen => "HP/s",
                EffectKind::Shield => "def",
                _ => "%",
            });
        }
        ui.label("for");
        ui.add(egui::DragValue::new(&mut spec.seconds).range(0.1..=3600.0).speed(0.1).suffix(" s"));
    });
}

/// An optional status effect with a checkbox (weapons: on hit, characters: on touch).
pub fn optional_effect_ui(ui: &mut egui::Ui, id: &str, label: &str, hint: &str, effect: &mut Option<EffectSpec>) {
    ui.horizontal(|ui| {
        let mut on = effect.is_some();
        if ui.checkbox(&mut on, label).on_hover_text(hint).changed() {
            *effect = on.then(|| EffectSpec::template(EffectKind::Poison));
        }
        if let Some(spec) = effect {
            effect_spec_ui(ui, id, spec);
        }
    });
}

/// "hh:mm" of an hour of the day (12.5 = "12:30").
pub fn hour_text(hour: f32) -> String {
    rustpg2d_engine::entity::hour_text(hour)
}

/// Reads "hh:mm" (or a plain number of hours) back.
fn parse_hour(s: &str) -> Option<f64> {
    match s.trim().split_once(':') {
        Some((h, m)) => Some(h.trim().parse::<f64>().ok()? + m.trim().parse::<f64>().ok()? / 60.0),
        None => s.trim().parse().ok(),
    }
}
