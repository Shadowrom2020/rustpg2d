//! The *Plugins* window (Assets › Plugins…): the project's plugins (`rustpg2d_engine::plugins`) in
//! load order: add one from a `.r2dplugin` file or the asset library (a newer version replaces the
//! one there, keeping its settings), switch it on and off, move it, fill in its parameters, export
//! it, send it to the library, remove it. Changes are saved at once (`project.ron`).

use std::path::Path;

use anyhow::{bail, Context, Result};
use egui_macroquad::egui;
use rustpg2d_engine::plugins::{self, ParamKind, ParamValue, PluginDef};

use crate::app::Editor;
use crate::files::Purpose;
use crate::ui::{self, ph};
use crate::widgets;

/// Something a click asks for, run once the window is drawn.
type Action = Box<dyn FnOnce(&mut Editor) -> Result<String>>;

#[derive(Default)]
pub struct PluginsUi {
    pub open: bool,
    /// The last result (text, ok).
    msg: Option<(String, bool)>,
    /// Plugin waiting for "Remove" to be confirmed.
    confirm_remove: Option<String>,
}

impl Editor {
    /// Saves `project.ron` after a change here (and counts it as saved).
    fn plugins_changed(&mut self) {
        if let Err(e) = self.project.save_config() {
            self.plugins_ui.msg = Some((format!("Saving project.ron failed: {e:#}"), false));
        }
        self.defs_saved.1 = self.project.config.clone();
        self.refresh_refs();
        self.problems.stale = true;
    }

    fn plugins_report(&mut self, r: Result<String>) {
        self.plugins_ui.msg = Some(match r {
            Ok(m) => (m, true),
            Err(e) => (format!("{e:#}"), false),
        });
    }

    fn installed_text(i: &plugins::Installed) -> String {
        match &i.replaced {
            Some(old) if *old != i.version => format!("Updated {} from {old} to {}", i.name, i.version),
            Some(_) => format!("Replaced {} ({})", i.name, i.version),
            None => format!("Added {} {}", i.name, i.version),
        }
    }

    /// Adds (or updates) a plugin from a `.r2dplugin` file. An older version than the one there
    /// is refused.
    pub fn add_plugin_file(&mut self, path: &Path) -> Result<String> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let (id, def, files) = plugins::unpack(&bytes)?;
        self.check_not_older(&id, &def)?;
        let i = plugins::install(&mut self.project, &id, &def, &files)?;
        self.plugins_changed();
        Ok(Self::installed_text(&i))
    }

    /// Adds (or updates) a plugin from the asset library's `plugins/<id>`.
    pub fn add_plugin_from_library(&mut self, id: &str) -> Result<String> {
        let lib = rustpg2d_engine::library::library_dir();
        let def = plugins::read_def(&lib, id)?;
        self.check_not_older(id, &def)?;
        let i = plugins::install_from(&mut self.project, &lib, id)?;
        self.plugins_changed();
        Ok(Self::installed_text(&i))
    }

    fn check_not_older(&self, id: &str, def: &PluginDef) -> Result<()> {
        if let Ok(have) = plugins::read_def(&self.project.root, id) {
            if plugins::compare_versions(&def.version, &have.version) == std::cmp::Ordering::Less {
                bail!("the project has {} {}, newer than {} (remove it first to go back)", have.name, have.version, def.version);
            }
        }
        Ok(())
    }

    /// Writes plugin `id` as a `.r2dplugin`.
    pub fn export_plugin(&mut self, id: &str, path: &Path) -> Result<String> {
        let bytes = plugins::pack(&self.project.root, id)?;
        std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))?;
        Ok(format!("Exported {id} to {}", path.display()))
    }

    /// Copies plugin `id` into the asset library (replacing the library's).
    pub fn send_plugin_to_library(&mut self, id: &str) -> Result<String> {
        let lib = rustpg2d_engine::library::library_dir();
        let files = rustpg2d_engine::pack::read_dir_files(&plugins::plugin_dir(&self.project.root, id))?;
        let dir = plugins::plugin_dir(&lib, id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        for (rel, b) in files {
            let p = dir.join(&rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&p, b)?;
        }
        Ok(format!("Sent {id} to the asset library"))
    }

    pub fn remove_plugin(&mut self, id: &str) -> Result<String> {
        plugins::remove(&mut self.project, id)?;
        self.plugins_changed();
        Ok(format!("Removed {id}"))
    }

    /// Switches a plugin on or off.
    pub fn set_plugin_enabled(&mut self, id: &str, on: bool) -> Result<String> {
        let r = self.project.config.plugins.iter_mut().find(|r| r.id == id).with_context(|| format!("no plugin '{id}' in the project"))?;
        r.enabled = on;
        self.plugins_changed();
        Ok(format!("{id} {}", if on { "on" } else { "off" }))
    }

    /// Moves a plugin one place earlier (`up`) or later in the load order.
    pub fn move_plugin(&mut self, id: &str, up: bool) -> Result<String> {
        let list = &mut self.project.config.plugins;
        let k = list.iter().position(|r| r.id == id).with_context(|| format!("no plugin '{id}' in the project"))?;
        let to = if up { k.checked_sub(1) } else { (k + 1 < list.len()).then_some(k + 1) };
        if let Some(to) = to {
            list.swap(k, to);
            self.plugins_changed();
        }
        Ok(format!("{id} is number {} now", to.unwrap_or(k) + 1))
    }

    /// Sets a parameter from text (`3`, `on`, `gold`); "-" goes back to the default.
    pub fn set_plugin_param(&mut self, id: &str, name: &str, text: &str) -> Result<String> {
        let def = plugins::read_def(&self.project.root, id)?;
        let p = def.parameters.iter().find(|p| p.name == name).with_context(|| format!("{id} has no parameter '{name}'"))?;
        let r = self.project.config.plugins.iter_mut().find(|r| r.id == id).with_context(|| format!("no plugin '{id}' in the project"))?;
        if text == "-" {
            r.parameters.remove(name);
        } else {
            let v = match p.kind {
                ParamKind::Number => ParamValue::Number(text.trim().parse().with_context(|| format!("'{text}' is no number"))?),
                ParamKind::Flag => ParamValue::Flag(matches!(text.trim(), "on" | "true" | "yes" | "1")),
                _ => ParamValue::Text(text.to_string()),
            };
            r.parameters.insert(name.to_string(), v);
        }
        self.plugins_changed();
        Ok(format!("{id}.{name} set"))
    }

    /// `status` `plugins=`: id:on|off:version[:api!] per plugin of the project, in order.
    pub fn plugins_status(&self) -> String {
        let list: Vec<String> = self
            .project
            .config
            .plugins
            .iter()
            .map(|r| match plugins::read_def(&self.project.root, &r.id) {
                Ok(d) => format!("{}:{}:{}{}", r.id, if r.enabled { "on" } else { "off" }, d.version, if plugins::api_fits(&d.api) { "" } else { ":api!" }),
                Err(_) => format!("{}:missing", r.id),
            })
            .collect();
        format!("[{}]", list.join(" "))
    }

    pub fn plugins_window(&mut self, ctx: &egui::Context) {
        if !self.plugins_ui.open {
            return;
        }
        let mut open = true;
        let mut act: Option<Action> = None;
        let mut dialog = None;
        let lib = rustpg2d_engine::library::library_dir();
        let lib_plugins: Vec<(String, PluginDef)> = plugins::installed(&lib).into_iter().filter_map(|id| plugins::read_def(&lib, &id).ok().map(|d| (id, d))).collect();
        egui::Window::new(ui::t(ph::PUZZLE_PIECE, "Plugins")).open(&mut open).default_width(560.0).show(ctx, |ui| {
            ui.weak("Add-ons with their own scripts, hooks, menu entries, HUD and assets. They stay in their folder (plugins/<id>), apart from the project's content: a newer version replaces them, keeping their settings. The game loads the ones that are on, in this order.");
            ui.horizontal(|ui| {
                if ui.button(ui::t(ph::FILE_PLUS, "Add from file…")).on_hover_text("A .r2dplugin file").clicked() {
                    dialog = Some(Purpose::AddPlugin);
                }
                ui.menu_button(ui::t(ph::BOOKS, "Add from library"), |ui| {
                    if lib_plugins.is_empty() {
                        ui.weak("The asset library has no plugins.");
                    }
                    for (id, d) in &lib_plugins {
                        let have = plugins::read_def(&self.project.root, id).ok();
                        let note = match &have {
                            Some(h) if plugins::compare_versions(&d.version, &h.version) == std::cmp::Ordering::Greater => format!(" (update from {})", h.version),
                            Some(_) => " (in the project)".to_string(),
                            None => String::new(),
                        };
                        if ui.button(format!("{} {}{note}", d.name, d.version)).on_hover_text(&d.description).clicked() {
                            let id = id.clone();
                            act = Some(Box::new(move |ed: &mut Editor| ed.add_plugin_from_library(&id)));
                            ui.close_menu();
                        }
                    }
                });
            });
            if let Some((m, ok)) = &self.plugins_ui.msg {
                ui.colored_label(if *ok { egui::Color32::from_rgb(120, 200, 120) } else { egui::Color32::from_rgb(230, 110, 100) }, m);
            }
            ui.separator();
            if self.project.config.plugins.is_empty() {
                ui.weak("No plugins yet.");
            }
            let n = self.project.config.plugins.len();
            let lists = Lists { sheets: self.refs.sheets.clone(), sounds: self.refs.sounds.clone(), items: self.refs.items.clone(), maps: self.refs.maps.clone() };
            for k in 0..n {
                let id = self.project.config.plugins[k].id.clone();
                let def = plugins::read_def(&self.project.root, &id);
                ui.push_id(("plugin", &id), |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut on = self.project.config.plugins[k].enabled;
                            if ui.checkbox(&mut on, "").on_hover_text("On: the game loads it").changed() {
                                let id = id.clone();
                                act = Some(Box::new(move |ed: &mut Editor| ed.set_plugin_enabled(&id, on)));
                            }
                            match &def {
                                Ok(d) => {
                                    ui.strong(&d.name);
                                    ui.weak(format!("{} · {id}{}", d.version, if d.author.is_empty() { String::new() } else { format!(" · {}", d.author) }));
                                }
                                Err(_) => {
                                    ui.colored_label(egui::Color32::from_rgb(230, 110, 100), format!("{id}: its folder is missing"));
                                }
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui::icon_button(ui, true, ph::TRASH, "Remove", "", "Removes the plugin's folder from the project").clicked() {
                                    self.plugins_ui.confirm_remove = Some(id.clone());
                                }
                                if ui::icon_button(ui, def.is_ok(), ph::UPLOAD_SIMPLE, "Send to library", "", "Copies it into the asset library (replacing the library's)").clicked() {
                                    let id = id.clone();
                                    act = Some(Box::new(move |ed: &mut Editor| ed.send_plugin_to_library(&id)));
                                }
                                if ui::icon_button(ui, def.is_ok(), ph::EXPORT, "Export…", "", "Writes a .r2dplugin file to share").clicked() {
                                    dialog = Some(Purpose::ExportPlugin(id.clone()));
                                }
                                if ui::icon_button(ui, k + 1 < n, ph::ARROW_DOWN, "Load later", "", "").clicked() {
                                    let id = id.clone();
                                    act = Some(Box::new(move |ed: &mut Editor| ed.move_plugin(&id, false)));
                                }
                                if ui::icon_button(ui, k > 0, ph::ARROW_UP, "Load earlier", "", "").clicked() {
                                    let id = id.clone();
                                    act = Some(Box::new(move |ed: &mut Editor| ed.move_plugin(&id, true)));
                                }
                            });
                        });
                        let Ok(d) = def else { return };
                        if !d.description.is_empty() {
                            ui.label(&d.description);
                        }
                        if !plugins::api_fits(&d.api) {
                            ui.colored_label(egui::Color32::from_rgb(230, 110, 100), format!("Needs script API {}; this R(ust)PG2D has {}: the game leaves it out.", d.api, rustpg2d_engine::script_api::API_VERSION));
                        }
                        let what: Vec<String> = [
                            (!d.hooks.is_empty()).then(|| format!("hooks: {}", d.hooks.iter().map(|h| h.hook.as_str()).collect::<Vec<_>>().join(", "))),
                            (!d.title_entries.is_empty() || !d.pause_entries.is_empty()).then(|| format!("{} menu entries", d.title_entries.len() + d.pause_entries.len())),
                            (!d.hud.is_empty()).then(|| format!("{} HUD elements", d.hud.len())),
                        ]
                        .into_iter()
                        .flatten()
                        .collect();
                        if !what.is_empty() {
                            ui.weak(what.join(" · "));
                        }
                        if d.parameters.is_empty() {
                            return;
                        }
                        egui::Grid::new("params").num_columns(2).show(ui, |ui| {
                            for p in &d.parameters {
                                let set = self.project.config.plugins[k].parameters.get(&p.name).cloned();
                                let mut v = p.value(set.as_ref());
                                let before = v.clone();
                                let label = ui.label(p.shown_label());
                                if !p.help.is_empty() {
                                    label.on_hover_text(&p.help);
                                }
                                ui.horizontal(|ui| {
                                    param_field(ui, &format!("{id}/{}", p.name), p, &mut v, &lists);
                                    if set.is_some() && ui.small_button("default").on_hover_text("Back to the plugin's default").clicked() {
                                        self.project.config.plugins[k].parameters.remove(&p.name);
                                        act = Some(Box::new(|_| Ok("Back to the default".into())));
                                    }
                                });
                                if v != before {
                                    self.project.config.plugins[k].parameters.insert(p.name.clone(), v);
                                    act = Some(Box::new(|_| Ok(String::new())));
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
            }
            if let Some(id) = self.plugins_ui.confirm_remove.clone() {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("Remove {id} and its folder?"));
                    if ui.button("Remove").clicked() {
                        act = Some(Box::new(move |ed: &mut Editor| ed.remove_plugin(&id)));
                        self.plugins_ui.confirm_remove = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.plugins_ui.confirm_remove = None;
                    }
                });
            }
        });
        if !open {
            self.plugins_ui.open = false;
        }
        if let Some(p) = dialog {
            self.open_file_dialog(p);
        }
        if let Some(f) = act {
            let r = f(self);
            match r {
                Ok(m) if m.is_empty() => self.plugins_changed(),
                Ok(m) if m == "Back to the default" => self.plugins_changed(),
                r => self.plugins_report(r),
            }
        }
    }

    /// The result of the window's file dialogs.
    pub fn plugin_file_chosen(&mut self, purpose: Purpose, path: &Path) {
        let r = match purpose {
            Purpose::AddPlugin => self.add_plugin_file(path),
            Purpose::ExportPlugin(id) => self.export_plugin(&id, path),
            _ => return,
        };
        self.plugins_report(r);
    }
}

/// The ids a parameter's field suggests.
struct Lists {
    sheets: Vec<String>,
    sounds: Vec<String>,
    items: Vec<String>,
    maps: Vec<String>,
}

/// The field of a parameter, by its kind.
fn param_field(ui: &mut egui::Ui, salt: &str, p: &plugins::ParamDef, v: &mut ParamValue, refs: &Lists) {
    match v {
        ParamValue::Number(n) => {
            let mut d = egui::DragValue::new(n).speed(0.1);
            if p.min != 0.0 || p.max != 0.0 {
                d = d.range(p.min..=p.max);
            }
            ui.add(d);
        }
        ParamValue::Flag(b) => {
            ui.checkbox(b, "");
        }
        ParamValue::Text(t) => {
            let list: Option<&Vec<String>> = match p.kind {
                ParamKind::Sprite => Some(&refs.sheets),
                ParamKind::Sound => Some(&refs.sounds),
                ParamKind::Item => Some(&refs.items),
                ParamKind::Map => Some(&refs.maps),
                _ => None,
            };
            match list {
                Some(l) => {
                    widgets::suggest(ui, salt, t, l, 160.0);
                }
                None => {
                    ui.add(egui::TextEdit::singleline(t).desired_width(160.0));
                }
            }
        }
    }
}
