//! *Update asset library*: brings the starter set of this editor into the creator's asset
//! library. New assets are added and the ones the creator never changed are replaced; assets
//! they changed that the starter set changed too, and the starter's dropped assets, are listed
//! for them to decide (see `rustpg2d_engine::starter_update`). The library folder is backed up
//! first. Offered in *Assets › Update asset library…*, the Library window's *File* menu and a
//! banner when a newer starter set arrived.

use std::collections::BTreeSet;
use std::path::PathBuf;

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::library::{self, AssetRef, Bundle};
use rustpg2d_engine::project::Project;
use rustpg2d_engine::starter_update::{self as su, StarterHistory, StarterUpdate};

use crate::app::Editor;
use crate::ui::{self, ph};

/// The starter set's history, shipped with the editor (`cargo run -p rustpg2d-editor --example
/// starter_history` writes it).
pub const STARTER_HISTORY: &str = include_str!("../assets/starter_history.ron");

/// The update window and the "updates waiting" banner.
#[derive(Default)]
pub struct LibUpdate {
    pub open: bool,
    pub plan: Option<StarterUpdate>,
    /// Changed-by-you assets to replace with the starter's version.
    pub take: BTreeSet<AssetRef>,
    /// Retired assets to delete.
    pub delete: BTreeSet<AssetRef>,
    /// What the last update did.
    pub report: Option<String>,
    pub error: Option<String>,
    /// The banner text while a newer starter set waits.
    pub offer: Option<String>,
    /// Looked for a waiting starter set this session.
    checked: bool,
}

impl LibUpdate {
    /// `libupdate=` for `status`.
    pub fn status(&self) -> String {
        match (&self.plan, &self.report, self.open) {
            (_, Some(_), true) => "done".into(),
            (Some(p), None, true) => format!("open:{}", p.counts()),
            _ if self.offer.is_some() => "offered".into(),
            _ => "closed".into(),
        }
    }
}

fn starter() -> anyhow::Result<(Bundle, StarterHistory)> {
    Ok((su::starter_bundle(crate::library_ui::STARTER)?, StarterHistory::parse(STARTER_HISTORY)?))
}

fn list_label(a: &AssetRef) -> String {
    format!("{}  {}  ({})", ui::kind_icon(a.0), a.1, a.0.name())
}

impl Editor {
    /// The library folder the update works on: the open one in the Library window, else the
    /// shared library.
    fn update_library_root(&self) -> PathBuf {
        if self.library_mode {
            self.project.root.clone()
        } else {
            library::library_dir()
        }
    }

    /// The library as saved on disk (the Library window's own folder; the main editor's shared
    /// library, with a changed starter set's new ids applied).
    fn update_library(&self) -> anyhow::Result<Project> {
        if self.library_mode {
            library::open_library(&self.project.root, None)
        } else {
            library::open_library(&library::library_dir(), Some(crate::library_ui::STARTER))
        }
    }

    /// Compares the library with the starter set and opens the window.
    pub fn open_library_update(&mut self) {
        let u = &mut self.lib_update;
        u.open = true;
        u.report = None;
        u.error = None;
        u.plan = None;
        if self.library_mode && self.defs_dirty {
            u.error = Some("Save or undo your changes to the library first (File › Save).".into());
            return;
        }
        let planned = (|| -> anyhow::Result<StarterUpdate> {
            let (starter, history) = starter()?;
            let lib = self.update_library()?;
            Ok(su::plan(&lib, &starter, &history))
        })();
        let u = &mut self.lib_update;
        match planned {
            Ok(p) => {
                u.take.clear();
                // Retired assets nobody changed and nothing needs are ticked for deletion.
                u.delete = p.retired.iter().filter(|r| r.untouched && r.used_by.is_empty()).map(|r| r.asset.clone()).collect();
                u.plan = Some(p);
            }
            Err(e) => u.error = Some(format!("Can't compare the library with the starter set: {e:#}")),
        }
    }

    /// Runs the update with the choices made in the window.
    pub fn apply_library_update(&mut self) {
        let Some(plan) = self.lib_update.plan.clone() else { return };
        let take: Vec<AssetRef> = self.lib_update.take.iter().cloned().collect();
        let delete: Vec<AssetRef> = self.lib_update.delete.iter().cloned().collect();
        let done = (|| -> anyhow::Result<su::UpdateReport> {
            let (starter, history) = starter()?;
            let mut lib = self.update_library()?;
            su::apply(&mut lib, &starter, &plan, &history, &take, &delete)
        })();
        match done {
            Ok(r) => {
                su::clear_pending(&self.update_library_root());
                self.lib_update.offer = None;
                let summary = r.summary();
                self.set_status(format!("Asset library updated: {summary}"));
                self.lib_update.report = Some(summary);
                self.lib_update.plan = None;
                if self.library_mode {
                    self.reload_library();
                } else {
                    self.lib.reload();
                }
            }
            Err(e) => self.lib_update.error = Some(format!("The update failed: {e:#}")),
        }
    }

    /// Once per session: does the library wait for a newer starter set's updates? (Only an
    /// existing library: the check doesn't create one.)
    pub fn check_library_update(&mut self) {
        if self.lib_update.checked {
            return;
        }
        self.lib_update.checked = true;
        let root = self.update_library_root();
        if !root.join("project.ron").exists() {
            return;
        }
        if !self.library_mode {
            // Opening applies a changed starter set (new ids) and leaves the pending mark.
            let _ = library::open_library(&root, Some(crate::library_ui::STARTER));
        }
        if !su::pending(&root) {
            return;
        }
        let plan = (|| -> anyhow::Result<StarterUpdate> {
            let (starter, history) = starter()?;
            Ok(su::plan(&self.update_library()?, &starter, &history))
        })();
        match plan {
            Ok(p) if p.updated.is_empty() && p.edited.is_empty() && p.retired.is_empty() => su::clear_pending(&root),
            Ok(p) => {
                let mut parts = Vec::new();
                if !p.updated.is_empty() {
                    parts.push(format!("{} updated", p.updated.len()));
                }
                if !p.edited.is_empty() {
                    parts.push(format!("{} also changed by you", p.edited.len()));
                }
                if !p.retired.is_empty() {
                    parts.push(format!("{} no longer included", p.retired.len()));
                }
                self.lib_update.offer = Some(format!("This version of R(ust)PG2D brings newer starter assets for your asset library: {}.", parts.join(", ")));
            }
            Err(e) => eprintln!("[editor] checking the asset library for updates: {e:#}"),
        }
    }

    /// The banner offering the update.
    pub fn library_update_banner(&mut self, ctx: &egui::Context) {
        let Some(text) = self.lib_update.offer.clone() else { return };
        let (mut open, mut later) = (false, false);
        egui::TopBottomPanel::top("library_update_banner").frame(egui::Frame::default().fill(Color32::from_rgb(30, 55, 75)).inner_margin(6.0)).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(ph::BOOKS).size(16.0));
                ui.label(text);
                if ui.button(ui::t(ph::ARROWS_CLOCKWISE, "Update…")).on_hover_text("Shows what would change and lets you decide").clicked() {
                    open = true;
                }
                if ui.button("Not now").on_hover_text("Assets › Update asset library… does it later").clicked() {
                    later = true;
                }
            });
        });
        if open {
            self.open_library_update();
        }
        if later {
            su::clear_pending(&self.update_library_root());
            self.lib_update.offer = None;
        }
    }

    /// The *Update asset library* window.
    pub fn library_update_window(&mut self, ctx: &egui::Context) {
        if !self.lib_update.open {
            return;
        }
        let mut open = true;
        let (mut run, mut close) = (false, false);
        let root = self.update_library_root();
        egui::Window::new(ui::t(ph::ARROWS_CLOCKWISE, "Update asset library")).id(egui::Id::new("library_update")).open(&mut open).default_width(560.0).default_height(520.0).show(ctx, |ui| {
            let u = &mut self.lib_update;
            if let Some(e) = &u.error {
                ui.colored_label(Color32::from_rgb(235, 110, 100), e);
                return;
            }
            if let Some(r) = &u.report {
                ui.label(egui::RichText::new("The asset library was updated.").strong());
                ui.add(egui::Label::new(r).wrap());
                if ui.button("Close").clicked() {
                    close = true;
                }
                return;
            }
            let Some(p) = u.plan.clone() else { return };
            if p.is_empty() {
                ui.label(format!("Your asset library has everything of this version's starter set.\n{}", root.display()));
                if ui.button("Close").clicked() {
                    close = true;
                }
                return;
            }
            ui.label(format!(
                "Brings this version's starter assets into your asset library: {} new, {} updated, {} changed by you, {} no longer included.",
                p.added.len(),
                p.updated.len(),
                p.edited.len(),
                p.retired.len()
            ));
            ui.weak("Your projects keep the assets they have; only the library changes.");
            ui.separator();
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                let info = |ui: &mut egui::Ui, title: String, list: &[AssetRef], hint: &str| {
                    if list.is_empty() {
                        return;
                    }
                    egui::CollapsingHeader::new(title).id_salt(hint).show(ui, |ui| {
                        ui.weak(hint);
                        for a in list {
                            ui.label(list_label(a));
                        }
                    });
                };
                info(ui, format!("New ({})", p.added.len()), &p.added, "Added to the library.");
                info(ui, format!("Updated ({})", p.updated.len()), &p.updated, "Newer versions of assets you never changed: replaced.");
                if !p.edited.is_empty() {
                    ui.label(egui::RichText::new(format!("Changed by you and in the starter set ({})", p.edited.len())).strong());
                    ui.weak("Your version stays unless you tick it; ticked ones get the starter's version.");
                    for a in &p.edited {
                        let mut on = u.take.contains(a);
                        if ui.checkbox(&mut on, list_label(a)).on_hover_text("Use the starter's version (yours is in the backup)").changed() {
                            if on {
                                u.take.insert(a.clone());
                            } else {
                                u.take.remove(a);
                            }
                        }
                    }
                }
                if !p.retired.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("No longer in the starter set ({})", p.retired.len())).strong());
                    ui.weak("Ticked ones are deleted from the library.");
                    for r in &p.retired {
                        let mut on = u.delete.contains(&r.asset);
                        ui.horizontal(|ui| {
                            let used = !r.used_by.is_empty();
                            let resp = ui.add_enabled(!used, egui::Checkbox::new(&mut on, list_label(&r.asset)));
                            if resp.changed() {
                                if on {
                                    u.delete.insert(r.asset.clone());
                                } else {
                                    u.delete.remove(&r.asset);
                                }
                            }
                            if used {
                                ui.weak(format!("used by {}", r.used_by.join(", ")));
                            } else if !r.untouched {
                                ui.weak("changed by you");
                            }
                        });
                    }
                }
            });
            ui.separator();
            ui.weak(format!("A backup of the library is made first, next to {}.", root.display()));
            ui.horizontal(|ui| {
                if ui.button(ui::t(ph::ARROWS_CLOCKWISE, "Update")).clicked() {
                    run = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if run {
            self.apply_library_update();
        }
        if close || !open {
            self.lib_update.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_history_covers_the_starter() {
        let b = su::starter_bundle(crate::library_ui::STARTER).unwrap();
        let h = StarterHistory::parse(STARTER_HISTORY).unwrap();
        let missing = h.missing(&b);
        assert!(missing.is_empty(), "crates/editor/assets/starter_history.ron lacks {missing:?}: run `cargo run -p rustpg2d-editor --example starter_history`");
    }
}
