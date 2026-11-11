//! Template chooser ("Start from…"): copies the look and equipment of a character of this
//! project or of the asset library onto a character definition, a placed NPC or the open
//! character builder. A library character's sheets and items are brought into the project first.

use std::cell::RefCell;

use anyhow::{Context, Result};
use egui_macroquad::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke, StrokeKind};
use rustpg2d_engine::defs::{Appearance, CharacterDef};
use rustpg2d_engine::library::Bundle;
use rustpg2d_engine::project::TILE_SIZE;
use rustpg2d_engine::refactor::{refs_of_character, RefKind};
use rustpg2d_engine::state::Equipment;

use crate::app::Editor;
use crate::doc::Objects;
use crate::widgets;

/// What gets the template's look.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelTarget {
    /// A character definition (every instance without its own look).
    Def(String),
    /// One placed character on the open map (entity uid).
    Instance(u32),
    /// The open character builder window.
    Builder,
}

/// A character to copy the look from.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelPick {
    /// A character of the project.
    Project(String),
    /// A character of the asset library (its sheets and items are copied into the project).
    Library(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gender {
    All,
    Male,
    Female,
}

pub struct ModelChooser {
    target: ModelTarget,
    library: bool,
    search: String,
    gender: Gender,
    pick: Option<ModelPick>,
}

thread_local! {
    static REQUEST: RefCell<Option<ModelTarget>> = const { RefCell::new(None) };
}

/// Opens the template chooser for `target` (called from forms that can't reach the editor).
pub fn request_model(target: ModelTarget) {
    REQUEST.with(|r| *r.borrow_mut() = Some(target));
}

const THUMB: f32 = 64.0;

impl Editor {
    /// Gives `target` the look and equipment of `pick`; library characters' sheets and items are
    /// copied into the project first.
    pub fn apply_model(&mut self, target: &ModelTarget, pick: &ModelPick) -> Result<String> {
        let (mut appearance, mut equipment, label): (Appearance, Equipment, String) = match pick {
            ModelPick::Project(id) => {
                let c = self.project.db.character(id).with_context(|| format!("no character '{id}' in the project"))?;
                (c.appearance.clone(), c.equipment.clone(), c.name.clone())
            }
            ModelPick::Library(cid) => {
                self.lib.ensure_loaded();
                let lib = self.lib.lib.as_ref().context("the asset library is not available")?;
                let c = lib.db.character(cid).with_context(|| format!("no character '{cid}' in the library"))?.clone();
                let mut b = Bundle::collect(&lib.db, &lib.root, TILE_SIZE, "library", &refs_of_character(&c))?;
                b.rescale(TILE_SIZE)?;
                let root = self.project.root.clone();
                let report = b.merge_into(&mut self.project.db, &root)?;
                if !report.added.is_empty() {
                    self.defs_dirty = true;
                }
                // Ids may have been taken by different assets of the project.
                let (mut a, mut eq) = (c.appearance.clone(), c.equipment.clone());
                for (kind, old, new) in &report.renamed {
                    match kind {
                        RefKind::Sheet => {
                            if &a.body == old {
                                a.body = new.clone();
                            }
                        }
                        RefKind::Item => {
                            for s in eq.stacks_mut().filter(|s| s.item == *old) {
                                s.item = new.clone();
                            }
                        }
                        _ => {}
                    }
                }
                (a, eq, c.name.clone())
            }
        };
        let msg = match target {
            ModelTarget::Def(id) => {
                let d = self.project.db.characters.iter_mut().find(|d| d.id == *id).with_context(|| format!("no character '{id}'"))?;
                d.appearance = std::mem::take(&mut appearance);
                d.equipment = std::mem::take(&mut equipment);
                self.defs_dirty = true;
                format!("{id} now looks like {label}. Save the definitions to keep it.")
            }
            ModelTarget::Builder => {
                let b = self.builder.as_mut().context("the character builder is not open")?;
                b.appearance = std::mem::take(&mut appearance);
                b.equipment = std::mem::take(&mut equipment);
                format!("Started from {label}.")
            }
            ModelTarget::Instance(uid) => {
                let doc = self.doc.as_mut().context("no map open")?;
                let before = Objects::of(&doc.map);
                let e = doc.map.entities.iter_mut().find(|e| e.uid == *uid).with_context(|| format!("no entity #{uid}"))?;
                e.appearance = Some(std::mem::take(&mut appearance));
                e.equipment = Some(std::mem::take(&mut equipment));
                doc.record_objects(before, "look");
                format!("#{uid} now looks like {label}.")
            }
        };
        self.reload_assets();
        Ok(msg)
    }

    /// Shows the template chooser window (opened via [`request_model`]).
    pub fn model_window(&mut self, ctx: &egui::Context) {
        if let Some(t) = REQUEST.with(|r| r.borrow_mut().take()) {
            self.model = Some(ModelChooser { target: t, library: false, search: String::new(), gender: Gender::All, pick: None });
        }
        let Some(mut st) = self.model.take() else { return };
        if st.library || matches!(st.pick, Some(ModelPick::Library(_))) {
            self.lib.ensure_loaded();
            self.lib.textures(ctx);
        }
        let title = match &st.target {
            ModelTarget::Def(id) => format!("Start '{id}' from a character"),
            ModelTarget::Instance(uid) => format!("Start the look of #{uid} from a character"),
            ModelTarget::Builder => "Start from a character".to_string(),
        };
        let mut open = true;
        let mut done: Option<bool> = None;
        egui::Window::new(title).id(egui::Id::new("model_chooser")).open(&mut open).collapsible(false).default_size(vec2(760.0, 560.0)).show(ctx, |ui| {
            // Preview of the pick in all four directions + buttons.
            ui.horizontal(|ui| {
                for row in 0..4u32 {
                    let (r, _) = ui.allocate_exact_size(vec2(56.0, 72.0), Sense::hover());
                    ui.painter().rect_filled(r, 3.0, Color32::from_gray(45));
                    if let Some(p) = &st.pick {
                        self.paint_model(ui, p, row, r.shrink(3.0));
                    }
                }
                ui.vertical(|ui| {
                    ui.label(match &st.pick {
                        None => "Pick a character below.".to_string(),
                        Some(ModelPick::Project(s)) => format!("Project: {}", self.project.db.character(s).map_or(s.clone(), |c| c.name.clone())),
                        Some(ModelPick::Library(c)) => format!("Library: {} (its sheets and items are copied into the project)", self.lib_char_name(c)),
                    });
                    ui.weak("Copies the figure, colours and equipment; the target's other settings stay.");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(st.pick.is_some(), egui::Button::new("\u{E182} Use this look")).clicked() {
                            done = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            done = Some(false);
                        }
                    });
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.library, false, "This project");
                ui.selectable_value(&mut st.library, true, "Asset library");
                ui.separator();
                ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("\u{E30C} search").desired_width(160.0));
                ui.selectable_value(&mut st.gender, Gender::All, "All");
                ui.selectable_value(&mut st.gender, Gender::Male, "Male");
                ui.selectable_value(&mut st.gender, Gender::Female, "Female");
            });
            let q = st.search.to_lowercase();
            let gender_ok = |db: &rustpg2d_engine::defs::Database, c: &CharacterDef| match st.gender {
                Gender::All => true,
                Gender::Male => !db.female_body(&c.appearance.body),
                Gender::Female => db.female_body(&c.appearance.body),
            };
            // (pick, label, tooltip)
            let entries: Vec<(ModelPick, String, String)> = if st.library {
                match &self.lib.lib {
                    None => {
                        ui.colored_label(Color32::LIGHT_RED, self.lib.error.clone().unwrap_or_else(|| "The asset library is not available.".into()));
                        Vec::new()
                    }
                    Some(lib) => {
                        let mut v: Vec<_> = lib
                            .db
                            .characters
                            .iter()
                            .filter(|c| gender_ok(&lib.db, c))
                            .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q) || c.id.contains(&q))
                            .map(|c| (ModelPick::Library(c.id.clone()), c.name.clone(), format!("{} ({})\nbody: {}", c.name, c.id, c.appearance.body)))
                            .collect();
                        v.sort_by(|a, b| a.1.cmp(&b.1));
                        v
                    }
                }
            } else {
                let db = &self.project.db;
                let mut v: Vec<_> = db
                    .characters
                    .iter()
                    .filter(|c| gender_ok(db, c))
                    .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q) || c.id.contains(&q))
                    .map(|c| (ModelPick::Project(c.id.clone()), c.name.clone(), format!("{} ({})\nbody: {}", c.name, c.id, c.appearance.body)))
                    .collect();
                v.sort_by(|a, b| a.1.cmp(&b.1));
                v
            };
            if entries.is_empty() && (self.lib.lib.is_some() || !st.library) {
                ui.weak("Nothing matches.");
            }
            egui::ScrollArea::vertical().id_salt("model_grid").max_height(380.0).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (pick, label, tip) in &entries {
                        let (r, resp) = ui.allocate_exact_size(vec2(THUMB + 20.0, THUMB + 40.0), Sense::click());
                        let img = Rect::from_min_size(r.min + vec2(10.0, 2.0), vec2(THUMB, THUMB + 16.0));
                        let on = st.pick.as_ref() == Some(pick);
                        ui.painter().rect_filled(r, 4.0, if resp.hovered() { Color32::from_gray(70) } else { Color32::from_gray(48) });
                        self.paint_model(ui, pick, 0, img);
                        let label = label.replace(" (female)", " (f)").replace(" (male)", " (m)");
                        let short: String = label.chars().take(15).collect();
                        ui.painter().text(pos2(r.center().x, r.max.y - 10.0), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(12.0), Color32::LIGHT_GRAY);
                        if on {
                            ui.painter().rect_stroke(r, 4.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
                        }
                        let resp = resp.on_hover_text(tip);
                        if resp.clicked() || resp.double_clicked() {
                            st.pick = Some(pick.clone());
                        }
                        if resp.double_clicked() {
                            done = Some(true);
                        }
                    }
                });
            });
            ui.weak("Click to preview, double-click (or \u{E182}) to use.");
        });
        match done {
            Some(true) => {
                if let Some(p) = st.pick.clone() {
                    let r = self.apply_model(&st.target, &p);
                    self.show_result(r);
                }
            }
            Some(false) => {}
            None if open => self.model = Some(st),
            None => {}
        }
    }

    /// Script hook: switch the open chooser to the library (or project) tab and preview a pick.
    pub fn model_script(&mut self, library: bool, pick: Option<ModelPick>) {
        if let Some(t) = REQUEST.with(|r| r.borrow_mut().take()) {
            self.model = Some(ModelChooser { target: t, library: false, search: String::new(), gender: Gender::All, pick: None });
        }
        if let Some(st) = &mut self.model {
            st.library = library;
            if pick.is_some() {
                st.pick = pick;
            }
        }
    }

    /// Script hook: use the previewed pick of the open chooser.
    pub fn model_script_use(&mut self) {
        let Some(st) = self.model.take() else { return };
        if let Some(p) = st.pick {
            let r = self.apply_model(&st.target, &p);
            self.show_result(r);
        }
    }

    fn lib_char_name(&self, id: &str) -> String {
        self.lib.lib.as_ref().and_then(|l| l.db.character(id)).map_or(id.to_string(), |c| c.name.clone())
    }

    /// Paints the idle frame of direction `row` (0 down, 1 left, 2 right, 3 up) of a pick.
    fn paint_model(&self, ui: &egui::Ui, pick: &ModelPick, row: u32, rect: Rect) {
        let facing = [rustpg2d_engine::entity::Facing::Down, rustpg2d_engine::entity::Facing::Left, rustpg2d_engine::entity::Facing::Right, rustpg2d_engine::entity::Facing::Up][row as usize % 4];
        match pick {
            ModelPick::Project(id) => {
                let layers = self.char_layers(id, None, facing, 0);
                widgets::paint_character(ui, &self.tex, &self.assets, &layers, rect);
            }
            ModelPick::Library(c) => {
                self.lib.paint_character(ui, c, facing, rect);
            }
        }
    }
}
