//! The Asset Library window: `rustpg2d-editor --library [DIR]` runs the editor in library mode on
//! the shared asset library (a project-like folder). It reuses the Database forms, the pixel
//! editor and the new-asset wizard to edit and create library assets in place; the Browse view
//! (`library_ui.rs`) imports, exports and removes. The main editor starts it as a separate
//! process (its own OS window) and re-reads the library whenever it needs it.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::library::{self, AssetRef};

use crate::app::{Editor, Mode, Pending};
use crate::database::Tab;
use crate::files;
use crate::ui::{self, ph};

/// Newest modification time of the library's definition files (to notice changes made by
/// another editor process, e.g. the main editor's "Add to library").
pub fn defs_stamp(root: &Path) -> Option<SystemTime> {
    std::fs::read_dir(root.join("defs")).ok()?.flatten().filter_map(|e| e.metadata().ok()?.modified().ok()).max()
}

/// Running Asset Library window started by this editor.
#[derive(Default)]
pub struct LibWindow {
    child: Option<std::process::Child>,
}

impl Editor {
    /// The editor in library mode on the asset library folder `dir`.
    pub fn library_window(dir: PathBuf) -> anyhow::Result<Self> {
        let lib = library::open_library(&dir, Some(crate::library_ui::STARTER))?;
        let mut ed = Self::with_project(lib, true);
        ed.library_mode = true;
        ed.mode = Mode::Library;
        ed.lib.tex_dirty = true;
        ed.lib_stamp = defs_stamp(&dir);
        ed.refresh_refs();
        ed.set_status("Asset library opened");
        Ok(ed)
    }

    /// Main editor: opens the Asset Library window (a second editor process), unless the one
    /// started before is still open.
    pub fn open_library_window(&mut self) {
        if let Some(c) = &mut self.lib_window.child {
            if matches!(c.try_wait(), Ok(None)) {
                self.set_status("The Asset Library window is already open");
                return;
            }
        }
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => return self.set_status(format!("Cannot start the Asset Library window: {e}")),
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--library").env_remove("RUSTPG2D_EDITOR_SCRIPT");
        // Testing hook: a script for the library window.
        if let Ok(s) = std::env::var("RUSTPG2D_LIBRARY_SCRIPT") {
            cmd.env("RUSTPG2D_EDITOR_SCRIPT", s);
        }
        match cmd.spawn() {
            Ok(c) => {
                self.lib_window.child = Some(c);
                self.set_status("Asset Library window opened — saved changes there show up here in 'Add from library'");
            }
            Err(e) => self.set_status(format!("Cannot start the Asset Library window: {e}")),
        }
    }

    /// Library mode: the definitions on disk now match the editor (after saving, importing,
    /// removing).
    pub fn mark_library_saved(&mut self) {
        self.defs_dirty = false;
        *self.defs_saved = (self.project.db.clone(), self.project.config.clone());
        self.lib_stamp = defs_stamp(&self.project.root);
        self.lib_changed = false;
    }

    /// Library mode: re-reads the library from disk (dropping unsaved definition edits).
    pub fn reload_library(&mut self) {
        match library::open_library(&self.project.root, None) {
            Ok(lib) => {
                self.project = lib;
                self.mark_library_saved();
                self.lib.selected.retain(|a| library::exists(&self.project.db, a));
                self.lib.tex_dirty = true;
                self.reload_assets();
                self.set_status("Library reloaded");
            }
            Err(e) => self.set_status(format!("Reloading the library failed: {e:#}")),
        }
    }

    /// Library mode: notices changes another editor wrote to the library (checked about once a
    /// second). Without unsaved edits they are loaded right away, otherwise a banner asks.
    pub fn watch_library(&mut self) {
        let now = macroquad::time::get_time();
        if now - self.lib_check < 1.0 {
            return;
        }
        self.lib_check = now;
        let stamp = defs_stamp(&self.project.root);
        if stamp == self.lib_stamp || self.lib_changed {
            return;
        }
        if self.defs_dirty {
            self.lib_changed = true;
        } else {
            self.reload_library();
            self.set_status("The library was changed in another editor window; reloaded");
        }
    }

    /// Library mode: edit an asset in the Edit (Database) view.
    pub fn edit_library_asset(&mut self, a: &AssetRef) {
        let Some(tab) = Tab::of(a.0) else { return };
        let Some(i) = self.project.db.index_of(a.0, &a.1) else { return };
        self.db.tab = tab;
        self.db.sel = i;
        self.mode = Mode::Database;
    }

    /// Library mode's top bar: File menu and the Browse / Edit / Pixel editor views.
    pub fn library_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button(ui::t(ph::FLOPPY_DISK, "Save   Ctrl+S")).clicked() {
                        self.save_all();
                        ui.close_menu();
                    }
                    if ui.add_enabled(!self.defs_dirty, egui::Button::new(ui::t(ph::ARROW_CLOCKWISE, "Reload from disk"))).on_hover_text("Re-reads the library (save or undo your edits first)").clicked() {
                        self.reload_library();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::SPARKLE, "New asset…")).clicked() {
                        self.wizard = Some(crate::wizard::Wizard::new(&self.project));
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::DOWNLOAD_SIMPLE, "Import package…")).clicked() {
                        self.open_file_dialog(files::Purpose::ImportPackage { into_library: true });
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::X, "Close window")).clicked() {
                        self.request(Pending::Quit);
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui::tab(ui, self.mode == Mode::Library, ph::SQUARES_FOUR, "Browse").on_hover_text("All library assets: select, import, export, remove").clicked().then(|| self.mode = Mode::Library);
                ui::tab(ui, self.mode == Mode::Database, ph::PENCIL_SIMPLE, "Edit").on_hover_text("Definitions of the library's assets").clicked().then(|| self.mode = Mode::Database);
                let px_label = if self.px.is_dirty() { "Pixel editor •" } else { "Pixel editor" };
                ui::tab(ui, self.mode == Mode::Pixel, ph::IMAGE, px_label).on_hover_text("Draw and edit the library's images").clicked().then(|| self.mode = Mode::Pixel);
                ui.separator();
                ui.label(egui::RichText::new(ui::t(ph::BOOKS, "Asset Library")).strong());
                if self.defs_dirty {
                    ui.colored_label(Color32::YELLOW, "unsaved •");
                }
            });
            if self.lib_changed {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::YELLOW, "The library was changed in another editor window.");
                    if ui.button(ui::t(ph::ARROW_CLOCKWISE, "Reload (discard my edits)")).clicked() {
                        self.reload_library();
                    }
                    if ui.button("Keep mine").on_hover_text("Saving then overwrites the other window's changes to the definitions").clicked() {
                        self.lib_changed = false;
                        self.lib_stamp = defs_stamp(&self.project.root);
                    }
                });
            }
        });
    }
}
