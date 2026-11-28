//! The item workshop, at the top of an item's tab: its own 3D model (a sword, a bow, a
//! backpack) taken from a file, shown in the hand of the game's hero, where and how it is
//! held set by buttons, and its inventory icon rendered from the model.
//!
//! What the buttons do is `editor_core::models` (`use_item_model`, `item_hold`,
//! `render_item_icon`): written into the definitions at once, as the files they come with are.

use eframe::egui::{self, Color32, Sense};
use eframe::egui_wgpu::RenderState;
use editor_core::Workspace;
use rustpg2d_engine::defs::{EquipSlot, HoldStyle};
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::render::character::{Anim, LAYOUT_FRAMES};
use rustpg2d_engine::render::model::{anim_req, Gear};

const PREVIEW: egui::Vec2 = egui::vec2(210.0, 230.0);
const SIDES: [(Facing, &str); 4] = [(Facing::Down, "Front"), (Facing::Left, "Left"), (Facing::Up, "Back"), (Facing::Right, "Right")];
const ANIMS: [(&str, &str); 3] = [("idle", "Standing"), ("walk", "Walking"), ("attack", "Striking")];

pub struct ItemStage {
    anim: &'static str,
    facing: Facing,
    turning: bool,
    /// What the last button said.
    pub said: String,
}

impl Default for ItemStage {
    fn default() -> Self {
        ItemStage { anim: "idle", facing: Facing::Down, turning: true, said: String::new() }
    }
}

/// The hero's gear with `item` where it is held or worn.
fn gear_with(ws: &Workspace, item: &str) -> Gear {
    let mut gear = Gear { ready: true, ..Gear::default() };
    match ws.project.db.slot_of(item) {
        Some(EquipSlot::Offhand) => gear.offhand = Some(item.to_string()),
        Some(EquipSlot::Back) => gear.back = Some(item.to_string()),
        Some(EquipSlot::Weapon | EquipSlot::Tool) | None => gear.weapon = Some(item.to_string()),
        Some(_) => gear.worn.push(item.to_string()),
    }
    gear
}

impl ItemStage {
    /// The script's line: `ID model=FILE|- hold=STYLE size=PX at=FRACTION anim= said=…`.
    pub fn status(&self, ws: &Workspace, id: &str) -> String {
        let Some(item) = ws.project.db.item(id) else { return format!("{id} gone") };
        let model = match &item.model {
            Some(m) => {
                let at = ws.assets.models.item_points(&m.file).map_or(0.0, |(pts, _)| editor_core::models::hold_fraction(m, &pts).0);
                format!("model={} hold={} size={} at={at:.2}", m.file, m.hold.label().replace(' ', "_"), m.size)
            }
            None => "model=-".to_string(),
        };
        format!("{id} {model} icon={}:{} anim={} said={}", item.icon.sheet, item.icon.frame, self.anim, if self.said.is_empty() { "-" } else { &self.said })
    }

    /// The script's words: `model FILE` (a `.glb`: copied into the project), `hold
    /// estimate|turnhandle|turnfront|flip|at FRACTION|style NAME|size PX`, `icon` (rendered
    /// from the model), `anim idle|walk|attack`, `face front|left|back|right|turn`.
    pub fn word(&mut self, ws: &mut Workspace, id: &str, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "model" => self.take_file(ws, id, std::path::Path::new(&args[1..].join(" ")))?,
            "hold" => self.said = ws.item_hold(id, arg(1), arg(2)),
            "icon" => self.said = ws.render_item_icon(id),
            "anim" => self.anim = ANIMS.iter().find(|(a, _)| *a == arg(1)).map(|(a, _)| *a).ok_or("anim idle|walk|attack")?,
            "face" => match arg(1) {
                "turn" => self.turning = true,
                side => {
                    self.facing = SIDES.iter().find(|(_, l)| l.eq_ignore_ascii_case(side)).map(|(f, _)| *f).ok_or("face front|left|back|right|turn")?;
                    self.turning = false;
                }
            },
            other => return Err(format!("item {other}: model, hold, icon, anim, face")),
        }
        Ok(())
    }

    /// A model file picked for the item: copied into the project's `models` folder, its hold
    /// point estimated from its shape.
    fn take_file(&mut self, ws: &mut Workspace, id: &str, file: &std::path::Path) -> Result<(), String> {
        if !file.is_file() {
            return Err(format!("there is no file {}", file.display()));
        }
        let rel = editor_core::files::import_asset(&ws.project.root, file, "models").map_err(|e| format!("{e:#}"))?;
        self.said = ws.use_item_model(id, &rel);
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str) {
        let Some(item) = ws.project.db.item(id).cloned() else { return };
        let mut todo: Option<(&'static str, String)> = None;
        let mut file: Option<std::path::PathBuf> = None;
        ui.horizontal_top(|ui| {
            // The hero with it in the hand (or on the arm, the back, the body).
            if item.model.is_some() || !item.wear.is_empty() {
                ui.vertical(|ui| {
                    let t = ui.input(|i| i.time);
                    let facing = if self.turning { SIDES[(t / 1.5) as usize % 4].0 } else { self.facing };
                    let (rect, _) = ui.allocate_exact_size(PREVIEW, Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(32, 34, 40));
                    if let Some(rs) = rs {
                        let hero = ws.project.db.character(&ws.project.config.player).map(|c| c.appearance.clone()).unwrap_or_default();
                        let look = ws.assets.models.look_of(&hero);
                        let gear = gear_with(ws, id);
                        // (A swing is played over and over, with a breath between; standing and walking go on.)
                        let frame = match self.anim {
                            "walk" => Anim::Walk((t * 7.0) as u32 % LAYOUT_FRAMES),
                            "attack" => {
                                let k = (t * 6.0) as u32 % 7;
                                if k < 4 { Anim::Attack(k) } else { Anim::Idle(0) }
                            }
                            _ => Anim::Idle(0),
                        };
                        let anim = anim_req(frame, t as f32);
                        crate::charshop::draw_figure(ui, rect, ws, rs, thumbs, &look, &gear, anim, facing);
                    }
                    ui.horizontal(|ui| {
                        for (a, label) in ANIMS {
                            ui.selectable_value(&mut self.anim, a, label);
                        }
                    });
                    ui.horizontal(|ui| {
                        for (f, label) in SIDES {
                            if ui.selectable_label(!self.turning && self.facing == f, label).clicked() {
                                (self.facing, self.turning) = (f, false);
                            }
                        }
                        ui.toggle_value(&mut self.turning, "Turning");
                    });
                });
            }
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Its 3D model");
                    match &item.model {
                        Some(m) => ui.label(m.file.as_str()),
                        None => ui.weak("none: in a hand it isn't shown (a weapon without one is swung as an empty hand)"),
                    };
                    let key = crate::pick::Key::FormFile(egui::Id::unique(("item model", id)).value());
                    if let Some(picked) = crate::pick::browse(ui, key, if item.model.is_some() { "Another file…" } else { "From a file…" }, "A 3D model (.glb) of the item alone: it is copied into the project, and where it is held is found from its shape", || crate::pick::Want::File("3D models", &["glb"]), None) {
                        file = Some(picked);
                    }
                });
                if let Some(m) = &item.model {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        let by = ui.label("Held").id;
                        egui::ComboBox::from_id_salt("hold style").selected_text(m.hold.label()).show_ui(ui, |ui| {
                            for s in HoldStyle::ALL {
                                let tip = match s {
                                    HoldStyle::Grip => "In the fist, its handle along the hand: swords, axes, tools, bows",
                                    HoldStyle::Upright => "Held up, its handle pointing up whatever the arm does: torches, staffs",
                                    HoldStyle::Hang => "Hanging from the hand: lanterns, buckets",
                                    HoldStyle::Strap => "Strapped to the forearm: shields",
                                    HoldStyle::Back => "On the upper back: backpacks",
                                };
                                if ui.selectable_label(m.hold == s, s.label()).on_hover_text(tip).clicked() && m.hold != s {
                                    todo = Some(("style", s.label().to_string()));
                                }
                            }
                        })
                        .response
                        .labelled_by(by);
                        let by = ui.label("Its length").on_hover_text("Of the whole model in the game's pixels: a sword about 22, a spear 50").id;
                        let mut size = m.size;
                        if ui.add(egui::DragValue::new(&mut size).range(2.0..=160.0).speed(0.25).suffix(" px")).labelled_by(by).changed() {
                            todo = Some(("size", size.to_string()));
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Find where it is held").on_hover_text("From the model's shape: the thin end is the handle. Do this first; the buttons beside it mend what it got wrong.").clicked() {
                            todo = Some(("estimate", String::new()));
                        }
                        if ui.button("Other end").on_hover_text("It is held at the wrong end: the blade is in the fist").clicked() {
                            todo = Some(("flip", String::new()));
                        }
                        if ui.button("Turn the handle").on_hover_text("Its handle runs along another of the model's three directions").clicked() {
                            todo = Some(("turnhandle", String::new()));
                        }
                        if ui.button("Turn the front").on_hover_text("A quarter around the handle: the edge of a blade, a bow's back, a shield's face should point where the knuckles do").clicked() {
                            todo = Some(("turnfront", String::new()));
                        }
                    });
                    // How far along its length the hand is.
                    if let Some((pts, _)) = ws.assets.models.item_points(&m.file) {
                        let mut at = editor_core::models::hold_fraction(m, &pts).0;
                        ui.horizontal(|ui| {
                            let by = ui.label("The hand is").id;
                            let r = ui.add(egui::Slider::new(&mut at, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} % along it", v * 100.0))).labelled_by(by).on_hover_text("From the handle's end (0 %) to the tip (100 %): a sword near its end, a bow in the middle");
                            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                                todo = Some(("at", at.to_string()));
                            }
                        });
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Render its icon").on_hover_text("Draws the model as the item's inventory icon (into the sheet icons_3d, which is made when it isn't there)").clicked() {
                            todo = Some(("icon", String::new()));
                        }
                        ui.weak("What is set here is written at once, with the model's file.");
                    });
                }
                if !self.said.is_empty() {
                    ui.weak(&self.said);
                }
            });
        });
        if let Some(picked) = file {
            if let Err(e) = self.take_file(ws, id, &picked) {
                self.said = e;
            }
        }
        if let Some((op, arg)) = todo {
            self.said = if op == "icon" { ws.render_item_icon(id) } else { ws.item_hold(id, op, &arg) };
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
        stage: ItemStage,
    }

    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (p, name) = (e.path(), e.file_name());
            if name == "saves" {
                continue;
            }
            if p.is_dir() {
                copy(&p, &to.join(&name));
            } else {
                std::fs::copy(&p, to.join(&name)).unwrap();
            }
        }
    }

    #[test]
    fn an_item_takes_a_model_file_and_its_hold_is_set_by_the_buttons() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-itemshop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir.join("game"));
        let mut ws = Workspace::new(dir.join("game"), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        // An item without a model of its own, and a model file from outside the project.
        ws.make_def(rustpg2d_engine::refactor::RefKind::Item, "Old blade", None).unwrap();
        ws.history_frame(None);
        let outside = dir.join("old_blade.glb");
        std::fs::copy(root.join("example_project/models/iron_sword.glb"), &outside).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 500.0)).build_ui_state(
            |ui, s: &mut State| {
                let mut thumbs = crate::thumbs::Thumbs::default();
                s.stage.show(ui, &mut s.ws, None, &mut thumbs, "old_blade");
            },
            State { ws, stage: ItemStage::default() },
        );
        h.run_steps(2);
        assert!(h.query_all_by_label_contains("none: in a hand it isn't shown").next().is_some());
        assert!(h.query_all_by_label("Find where it is held").next().is_none(), "nothing to hold yet");
        // The file is picked: copied into models/, the item names it, its hold point is found.
        crate::pick::answer_next(&outside);
        h.get_by_label("From a file…").click();
        h.run_steps(4);
        let model = h.state().ws.project.db.item("old_blade").unwrap().model.clone().expect("the item has its model");
        assert_eq!(model.file, "models/old_blade.glb");
        assert!(dir.join("game/models/old_blade.glb").is_file());
        assert!(h.state().stage.said.contains("has the 3D model"), "{}", h.state().stage.said);
        // Its controls are there now, and the script's line says what it has. (What the
        // buttons do to the hold needs the model as the game loads it: tried in the window.)
        for label in ["Find where it is held", "Other end", "Turn the handle", "Turn the front", "Render its icon"] {
            assert!(h.query_all_by_label(label).next().is_some(), "no \"{label}\"");
        }
        assert!(h.state().stage.status(&h.state().ws, "old_blade").starts_with("old_blade model=models/old_blade.glb hold=Grip"), "{}", h.state().stage.status(&h.state().ws, "old_blade"));
        // What is no file is refused by the script's word; an animation is one of three.
        {
            let s = h.state_mut();
            assert!(s.stage.word(&mut s.ws, "old_blade", &["model".into(), "/nowhere/x.glb".into()]).is_err());
            assert!(s.stage.word(&mut s.ws, "old_blade", &["anim".into(), "walk".into()]).is_ok() && s.stage.word(&mut s.ws, "old_blade", &["anim".into(), "dance".into()]).is_err());
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
