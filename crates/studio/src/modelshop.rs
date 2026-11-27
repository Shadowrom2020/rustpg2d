//! The model workshop, at the top of a 3D model's tab: the model as the game draws it, playing
//! any of its animations, and beside it what the check found with the fixes at hand: each
//! animation's clip picked from the clips its files hold, the clips guessed from their names,
//! a rig template for a model without a skeleton.
//!
//! What a model needs and how it is fixed is worked out in `editor_core::modelcheck`.

use eframe::egui::{self, Color32, Sense};
use eframe::egui_wgpu::RenderState;
use editor_core::modelcheck::{self, Checked, BASE, EXTRAS};
use editor_core::Workspace;
use rustpg2d_engine::defs::Appearance;
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::render::model::Gear;

use crate::charshop::{PREVIEW, SIDES};

pub struct ModelStage {
    pub facing: Facing,
    pub turning: bool,
    /// The animation shown.
    pub anim: String,
}

impl Default for ModelStage {
    fn default() -> ModelStage {
        ModelStage { facing: Facing::Down, turning: true, anim: "idle".into() }
    }
}

/// "attack_2h" -> "Attack 2h".
fn anim_label(anim: &str) -> String {
    let mut c = anim.replace('_', " ");
    if let Some(f) = c.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    c
}

/// What an animation is played for.
fn anim_help(anim: &str) -> &'static str {
    match anim {
        "walk" => "Walking: every figure needs it",
        "idle" => "Standing still: every figure needs it",
        "attack" => "A blow, a shot, a spell: every figure needs it",
        "hurt" => "Just hit: every figure needs it",
        "sit" => "On a chair or a bench",
        "sleep" => "Lying in a bed",
        "fish" | "farm" | "craft" | "chop" | "mine" | "sweep" | "read" => "At work: characters seen doing it at their spot, the player gathering",
        "attack_1h" | "attack_2h" | "attack_thrust" | "attack_bow" | "attack_cast" | "attack_unarmed" => "The blow with a weapon of that kind (an item's Attack animation names it)",
        "idle_1h" | "idle_2h" => "The battle stance with a weapon of that kind",
        "block_shield" | "block_weapon" => "A blow blocked or parried",
        "sneak" => "Sneaking",
        _ => "An animation of its own: scripts and routes play it by this name",
    }
}

impl ModelStage {
    /// The look the model is shown as: itself, or for a piece the first figure it fits, wearing it.
    fn look(ws: &Workspace, id: &str) -> Appearance {
        let db = &ws.project.db;
        match db.model(id).and_then(|m| m.part.as_ref()) {
            Some(part) => {
                let body = part.fits.iter().find(|b| ws.assets.models.has(b)).cloned().or_else(|| db.models.iter().find(|m| m.playable && m.part.is_none()).map(|m| m.id.clone())).unwrap_or_default();
                let mut a = Appearance { model: body, ..Default::default() };
                a.parts.insert(part.slot.clone(), id.to_string());
                ws.assets.models.look_of(&a)
            }
            None => Appearance { model: id.to_string(), ..Default::default() },
        }
    }

    /// The script's line: `ID ok|problemsN|not loaded clips=N rig=own|TEMPLATE|none anims=[walk=Clip …] missing=[…] shown=ANIM`.
    pub fn status(&self, ws: &Workspace, id: &str) -> String {
        let Some(def) = ws.project.db.model(id) else { return "-".into() };
        let c = ws.model_checked(id);
        if let Some(e) = &c.error {
            return format!("{id} not loaded: {e}");
        }
        let rig = match &def.rig {
            Some(r) => format!("{}@{}", r.template, r.turn),
            None if c.rigged => "own".to_string(),
            None => "none".to_string(),
        };
        let anims: Vec<String> = def.clips.iter().map(|m| format!("{}={}@{}", m.anim, m.clip, m.fps)).collect();
        format!("{id} {} clips={} rig={rig} anims=[{}] missing=[{}] shown={}", if c.problems() == 0 { "ok".to_string() } else { format!("problems{}", c.problems()) }, c.clips.len(), anims.join(" "), c.missing.join(" "), self.anim)
    }

    /// The script's words: `play ANIM CLIP|-`, `fps ANIM N`, `guess`, `rig TEMPLATE|- [TURN]`,
    /// `face front|left|back|right|turn`, `anim NAME`, `check` (prints the check's lines).
    pub fn word(&mut self, ws: &mut Workspace, id: &str, args: &[String]) -> Result<(), String> {
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
        match arg(0) {
            "" => {}
            "play" => ws.model_play(id, arg(1), Some(arg(2)).filter(|c| !c.is_empty() && *c != "-"))?,
            "fps" => ws.model_fps(id, arg(1), arg(2).parse().map_err(|_| "fps ANIM NUMBER")?)?,
            "guess" => println!("[model] guessed: {}", ws.model_guess(id)?.join(" ")),
            "rig" => ws.model_rig(id, arg(1), arg(2).parse().ok())?,
            "face" => match SIDES.iter().find(|(_, l)| l.eq_ignore_ascii_case(arg(1))) {
                Some((f, _)) => (self.facing, self.turning) = (*f, false),
                None if arg(1) == "turn" => self.turning = true,
                None => return Err("face front|left|back|right|turn".into()),
            },
            "anim" => self.anim = arg(1).to_string(),
            "check" => {
                let c = ws.model_checked(id);
                for (problem, line) in &c.lines {
                    println!("[model] {} {line}", if *problem { "!" } else { " " });
                }
            }
            other => return Err(format!("model {other}: play, fps, guess, rig, face, anim, check")),
        }
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str) {
        let Some(def) = ws.project.db.model(id).cloned() else { return };
        let checked = ws.model_checked(id);
        ui.horizontal_top(|ui| {
            self.preview(ui, ws, rs, thumbs, id, &def);
            ui.separator();
            ui.vertical(|ui| {
                if let Some(e) = &checked.error {
                    ui.label(egui::RichText::new("Workshop").strong());
                    if rs.is_some() {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                        ui.weak("Its file is named under Files below. Reload art (Assets menu) reads it again once it is there.");
                    } else {
                        ui.weak("Models are looked over where the editor can draw them.");
                    }
                    return;
                }
                self.check(ui, ws, id, &def, &checked);
                if !checked.piece {
                    ui.add_space(6.0);
                    self.animations(ui, ws, id, &def, &checked);
                }
            });
        });
    }

    fn preview(&mut self, ui: &mut egui::Ui, ws: &Workspace, rs: Option<&RenderState>, thumbs: &mut crate::thumbs::Thumbs, id: &str, def: &rustpg2d_engine::defs::ModelDef) {
        let t = ui.input(|i| i.time);
        let facing = if self.turning { SIDES[(t / 1.5) as usize % 4].0 } else { self.facing };
        ui.vertical(|ui| {
            let (rect, _) = ui.allocate_exact_size(PREVIEW, Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(32, 34, 40));
            if let Some(rs) = rs.filter(|_| ws.assets.models.has(id)) {
                let look = Self::look(ws, id);
                // (An animation it has no clip for is shown standing.)
                let anim = if def.part.is_some() || def.clip_of(&self.anim).is_some() { self.anim.as_str() } else { "idle" };
                crate::charshop::draw_figure(ui, rect, ws, rs, thumbs, &look, &Gear::default(), (anim, t as f32, false), facing);
            }
            ui.horizontal(|ui| {
                for (f, label) in SIDES {
                    if ui.selectable_label(!self.turning && self.facing == f, label).on_hover_text("Seen from this side").clicked() {
                        (self.facing, self.turning) = (f, false);
                    }
                }
                ui.toggle_value(&mut self.turning, "Turning").on_hover_text("Turns around, a side at a time");
            });
            if def.part.is_some() {
                ui.weak("A piece, shown on a figure it fits.");
            }
        });
    }

    /// What the check found, problems first, each with its fix where there is one.
    fn check(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, id: &str, def: &rustpg2d_engine::defs::ModelDef, c: &Checked) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Workshop").strong());
            match c.problems() {
                0 => ui.weak("Nothing is missing."),
                1 => ui.colored_label(ui.visuals().warn_fg_color, "1 thing to see to"),
                n => ui.colored_label(ui.visuals().warn_fg_color, format!("{n} things to see to")),
            };
        });
        for (_, line) in c.lines.iter().filter(|l| l.0) {
            ui.colored_label(ui.visuals().warn_fg_color, format!("• {line}"));
        }
        // No skeleton and no rig template: it can't move. The templates are offered.
        if !c.piece && !c.rigged {
            let templates = ws.rig_templates();
            ui.horizontal(|ui| {
                let l = ui.label("Skeleton").on_hover_text("This model has no bones of its own. A rig template lends it a skeleton and that skeleton's animations: four legs for an animal, wings for a bird…");
                let current = def.rig.as_ref().map(|r| r.template.clone()).unwrap_or_default();
                let shown = templates.iter().find(|t| t.0 == current).map_or_else(|| if current.is_empty() { "none: it stands still".to_string() } else { current.clone() }, |t| t.1.clone());
                let mut pick = None;
                egui::ComboBox::from_id_salt(("rig template", id)).selected_text(shown).width(200.0).show_ui(ui, |ui| {
                    if ui.selectable_label(current.is_empty(), "none: it stands still").clicked() {
                        pick = Some(String::new());
                    }
                    for (tid, name) in &templates {
                        if ui.selectable_label(*tid == current, name).clicked() {
                            pick = Some(tid.clone());
                        }
                    }
                })
                .response
                .labelled_by(l.id);
                if let Some(t) = pick.filter(|t| *t != current) {
                    if let Err(e) = ws.model_rig(id, &t, None) {
                        ws.set_status(e);
                    }
                }
                if let Some(rig) = &def.rig {
                    if ui.button("Turn it").on_hover_text(format!("A quarter further (now {}°): it must face the viewer in the front view for the skeleton to sit right", rig.turn)).clicked() {
                        if let Err(e) = ws.model_rig(id, &rig.template, Some(rig.turn + 90.0)) {
                            ws.set_status(e);
                        }
                    }
                }
            });
            if templates.is_empty() {
                ui.weak("The project has no rig templates: the asset library has them (3D models named \"Rig: …\").");
            }
        }
        egui::CollapsingHeader::new("What it is").id_salt(("model facts", id)).show(ui, |ui| {
            for (_, line) in c.lines.iter().filter(|l| !l.0) {
                ui.weak(line);
            }
        });
    }

    /// Rows of animations: its name (a click shows it played), the clip it plays, how fast.
    #[allow(clippy::too_many_arguments)]
    fn rows(&mut self, ui: &mut egui::Ui, id: &str, def: &rustpg2d_engine::defs::ModelDef, c: &Checked, salt: &str, anims: &[String], play: &mut Option<(String, Option<String>)>, fps: &mut Option<(String, f32)>) {
        egui::Grid::new(("model animations", id, salt)).num_columns(4).spacing([8.0, 3.0]).show(ui, |ui| {
            for anim in anims {
                let map = def.clip_of(anim);
                let there = map.is_some_and(|m| c.clips.iter().any(|k| k.0 == m.clip));
                let shown = ui.selectable_label(self.anim == *anim, anim_label(anim)).on_hover_text(format!("{}. Click to see it played.", anim_help(anim)));
                if shown.clicked() {
                    self.anim = anim.clone();
                }
                let text = match map {
                    Some(m) if there => m.clip.clone(),
                    Some(m) => format!("{} (not in its files)", m.clip),
                    None => "no clip".to_string(),
                };
                let text = if there { egui::RichText::new(text) } else { egui::RichText::new(text).color(ui.visuals().warn_fg_color) };
                egui::ComboBox::from_id_salt(("clip of", id, anim)).selected_text(text).width(230.0).show_ui(ui, |ui| {
                    for (clip, secs) in &c.clips {
                        if ui.selectable_label(map.is_some_and(|m| m.clip == *clip), format!("{clip}  ·  {secs:.1} s")).clicked() {
                            *play = Some((anim.clone(), Some(clip.clone())));
                        }
                    }
                })
                .response
                .labelled_by(shown.id);
                match map {
                    Some(m) => {
                        let mut f = m.fps;
                        if ui.add(egui::DragValue::new(&mut f).range(1.0..=60.0).speed(0.25).suffix(" a second")).on_hover_text("Frames a second it is drawn with: stepped, like pixel animation").changed() {
                            crate::shell::hold_step(ui.ctx(), ("model fps", id, anim.as_str()));
                            *fps = Some((anim.clone(), f));
                        }
                        if !BASE.contains(&anim.as_str()) && ui.small_button("Remove").on_hover_text("The model doesn't play this animation any more").clicked() {
                            *play = Some((anim.clone(), None));
                        }
                    }
                    None => {
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
    }

    /// Each animation with the clip it plays, picked from the clips the model's files hold.
    fn animations(&mut self, ui: &mut egui::Ui, ws: &mut Workspace, id: &str, def: &rustpg2d_engine::defs::ModelDef, c: &Checked) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Animations").strong());
            ui.weak(match c.clips.len() {
                0 => "its files hold no clips".to_string(),
                1 => "1 clip in its files".to_string(),
                n => format!("{n} clips in its files"),
            });
            let guessable = BASE.iter().chain(EXTRAS.iter()).any(|a| def.clip_of(a).is_none() && modelcheck::guess_for(a, &c.clips).is_some());
            if guessable && ui.button("Guess from the clips' names").on_hover_text("Every animation without a clip takes the clip whose name says it: \"Walking\" for walk, \"Hit_Reaction\" for hurt…").clicked() {
                match ws.model_guess(id) {
                    Ok(mapped) => ws.set_status(if mapped.is_empty() { "No clip's name says what it is for".to_string() } else { format!("Mapped {}", mapped.join(", ")) }),
                    Err(e) => ws.set_status(e),
                }
            }
        });
        if c.clips.is_empty() {
            ui.weak(if c.rigged { "A file with clips for its skeleton can be added under Files below (More clip files)." } else { "Without a skeleton it stands still: pick one above, its animations come with it." });
            return;
        }
        let base: Vec<String> = BASE.iter().map(|a| a.to_string()).collect();
        let others: Vec<String> = def.clips.iter().map(|m| m.anim.clone()).filter(|a| !BASE.contains(&a.as_str())).collect();
        let mut play: Option<(String, Option<String>)> = None;
        let mut fps: Option<(String, f32)> = None;
        self.rows(ui, id, def, c, "base", &base, &mut play, &mut fps);
        // (What every figure plays is always in view; a long list of the others folds away.)
        if !others.is_empty() {
            egui::CollapsingHeader::new(format!("Its other animations ({})", others.len())).id_salt(("other animations", id)).default_open(others.len() <= 6).show(ui, |ui| {
                self.rows(ui, id, def, c, "others", &others, &mut play, &mut fps);
            });
        }
        // One more animation: the kinds the game knows that it has none for yet.
        let free: Vec<&str> = EXTRAS.iter().copied().filter(|a| def.clip_of(a).is_none()).collect();
        if !free.is_empty() {
            egui::ComboBox::from_id_salt(("add animation", id)).selected_text("+ Animation").width(150.0).show_ui(ui, |ui| {
                for a in free {
                    if ui.selectable_label(false, anim_label(a)).on_hover_text(anim_help(a)).clicked() {
                        // (It starts with the clip its name suggests, else the first.)
                        let clip = modelcheck::guess_for(a, &c.clips).map(|g| g.clip).or_else(|| c.clips.first().map(|k| k.0.clone()));
                        play = Some((a.to_string(), clip));
                    }
                }
            })
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "Add an animation"));
        }
        if let Some((anim, clip)) = play {
            match ws.model_play(id, &anim, clip.as_deref()) {
                Ok(()) if clip.is_some() => self.anim = anim,
                Ok(()) => self.anim = "idle".into(),
                Err(e) => ws.set_status(e),
            }
        }
        if let Some((anim, f)) = fps {
            if let Err(e) = ws.model_fps(id, &anim, f) {
                ws.set_status(e);
            }
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

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

    struct State {
        ws: Workspace,
        stage: ModelStage,
        thumbs: crate::thumbs::Thumbs,
        id: String,
    }

    /// The example with a few of its models read (reading needs no graphics adapter).
    fn harness(tag: &str, id: &str, ids: &[&str]) -> (Harness<'static, State>, std::path::PathBuf) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-modelshop-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(&root.join("example_project"), &dir);
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        let mut few = rustpg2d_engine::defs::Database { models: ws.project.db.models.iter().filter(|m| ids.contains(&m.id.as_str())).cloned().collect(), ..Default::default() };
        few.reindex();
        let project = &ws.project;
        ws.assets.models = rustpg2d_engine::render::model::Models::load(&few, &|rel| std::fs::read(project.path(rel)));
        let state = State { ws, stage: ModelStage::default(), thumbs: Default::default(), id: id.to_string() };
        let h = Harness::builder().with_size(egui::vec2(1100.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                let held = ui.ctx().data_mut(|d| d.remove_temp::<u64>(crate::shell::held_step_id()));
                s.ws.history_frame(held.or(ui.ctx().memory(|m| m.focused()).map(|id| id.value())));
                let id = s.id.clone();
                s.stage.show(ui, &mut s.ws, None, &mut s.thumbs, &id);
            },
            state,
        );
        (h, dir)
    }

    #[test]
    fn a_missing_animation_is_said_and_given_a_clip_from_the_list() {
        let (mut h, dir) = harness("clips", "hero", &["hero"]);
        h.run();
        // The hero lacks nothing; what every figure plays is listed with its clip.
        h.get_by_label("Nothing is missing.");
        assert!(h.query_all_by_label_contains("clips in its files").next().is_some());
        // Being hurt plays nothing any more: said as the one thing to see to.
        h.state_mut().ws.model_play("hero", "hurt", None).unwrap();
        h.run();
        h.get_by_label("1 thing to see to");
        assert!(h.query_all_by_label_contains("no clip for the animation hurt").next().is_some());
        // The clip is picked from those its files hold.
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Hurt").click();
        h.run();
        h.get_by_label_contains("Running  ·").click();
        h.run();
        h.run();
        assert_eq!(h.state().ws.project.db.model("hero").unwrap().clip_of("hurt").map(|c| c.clip.as_str()), Some("Running"));
        h.get_by_label("Nothing is missing.");
        assert_eq!(h.state().stage.anim, "hurt", "what was picked is what is shown played");
        // Taken away again, the names of the clips say which one it is: one click.
        h.state_mut().ws.model_play("hero", "hurt", None).unwrap();
        h.run();
        h.get_by_label("Guess from the clips' names").click();
        h.run();
        h.run();
        assert_eq!(h.state().ws.project.db.model("hero").unwrap().clip_of("hurt").map(|c| c.clip.as_str()), Some("Hit_Reaction"));
        h.get_by_label("Nothing is missing.");
        // Each of these was a step of its own.
        assert!(h.state().ws.timeline.done.iter().any(|c| c.label.contains("hurt plays Running")), "{:?}", h.state().ws.timeline.done.iter().map(|c| &c.label).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_model_without_a_skeleton_is_offered_the_rig_templates() {
        // A creature of the example that stands on the four-legged template's skeleton.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let example = rustpg2d_engine::project::Project::load(&root.join("example_project")).unwrap();
        let id = example.db.models.iter().find(|m| m.rig.as_ref().is_some_and(|r| r.template == "rig_quadruped")).expect("a four-legged creature").id.clone();
        let (mut h, dir) = harness("rig", &id, &[&id, "rig_quadruped", "rig_bird"]);
        h.run();
        h.get_by_label("Nothing is missing.");
        h.get_by_label("Turn it");
        // Without the template: no skeleton, nothing to play, and the editor says what to do.
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Skeleton").click();
        h.run();
        h.get_by_label("none: it stands still").click();
        h.run();
        assert!(h.state().ws.project.db.model(&id).unwrap().rig.is_none());
        // (Here nothing reads the model again: the editor with a graphics adapter does.)
        let few = {
            let ws = &h.state().ws;
            let mut few = rustpg2d_engine::defs::Database { models: ws.project.db.models.iter().filter(|m| [id.as_str(), "rig_quadruped", "rig_bird"].contains(&m.id.as_str())).cloned().collect(), ..Default::default() };
            few.reindex();
            rustpg2d_engine::render::model::Models::load(&few, &|rel| std::fs::read(ws.project.path(rel)))
        };
        h.state_mut().ws.assets.models = few;
        h.run();
        h.get_by_label("4 things to see to");
        assert!(h.query_all_by_label_contains("pick one above").next().is_some());
        // The templates are offered by name.
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Skeleton").click();
        h.run();
        let offered = h.state().ws.rig_templates();
        let name = offered.iter().find(|t| t.0 == "rig_bird").expect("the bird template is offered").1.clone();
        h.get_by_label(name.as_str()).click();
        h.run();
        assert_eq!(h.state().ws.project.db.model(&id).unwrap().rig.as_ref().map(|r| r.template.as_str()), Some("rig_bird"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
