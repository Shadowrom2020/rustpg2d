//! *Clean up unused files* (Assets menu, and when a project closes): the 3D models, sprite
//! sheets, sounds, materials and fonts the project defines but nothing uses, and stray files in
//! its asset folders ([`rustpg2d_engine::unused`]). Ticked ones are deleted after a backup
//! beside the project; unticked ones are remembered as kept (per project, in the editor's
//! settings) and not offered again when the project closes. Exports leave unused assets out
//! either way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use egui_macroquad::egui;
use rustpg2d_engine::project::Project;
use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::unused::{self, Unused};

use crate::app::{Editor, Pending};
use crate::ui::{self, ph};

/// The open cleanup window.
pub struct Cleanup {
    /// What it offers.
    pub unused: Unused,
    /// Ticked (= delete) per entry ([`def_key`], [`file_key`]).
    pub picks: BTreeMap<String, bool>,
    /// Entries kept before (shown unticked, with a note).
    pub kept: BTreeSet<String>,
    /// What happens after it (the project closing); None = opened from the menu.
    pub then: Option<Pending>,
    /// When the project closes: the project as saved (the editor's copy may hold discarded edits).
    pub disk: Option<Project>,
}

/// An unused definition's entry.
pub fn def_key(kind: RefKind, id: &str) -> String {
    format!("{}:{id}", kind.name())
}

/// A stray file's entry.
pub fn file_key(path: &str) -> String {
    format!("file:{path}")
}

/// `1.2 MB`, `34 KB`.
fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

impl Cleanup {
    fn entries(&self) -> Vec<(String, String, u64)> {
        let mut out: Vec<(String, String, u64)> = self.unused.defs.iter().map(|d| (def_key(d.kind, &d.id), d.files.join(", "), d.bytes)).collect();
        out.extend(self.unused.files.iter().map(|(f, b)| (file_key(f), String::new(), *b)));
        out
    }

    /// The ticked definitions and files.
    fn ticked(&self) -> (BTreeSet<(RefKind, String)>, BTreeSet<String>) {
        let on = |k: &str| self.picks.get(k).copied().unwrap_or(false);
        let defs = self.unused.defs.iter().filter(|d| on(&def_key(d.kind, &d.id))).map(|d| (d.kind, d.id.clone())).collect();
        let files = self.unused.files.iter().filter(|(f, _)| on(&file_key(f))).map(|(f, _)| f.clone()).collect();
        (defs, files)
    }

    fn ticked_bytes(&self) -> u64 {
        self.entries().iter().filter(|(k, _, _)| self.picks.get(k).copied().unwrap_or(false)).map(|e| e.2).sum()
    }

    /// For the editor script: one line per entry.
    pub fn listing(&self) -> String {
        self.entries().iter().map(|(k, files, b)| format!("[cleanup] {}{k} {} {files}", if self.picks.get(k).copied().unwrap_or(false) { "x " } else { "  " }, size(*b))).collect::<Vec<_>>().join("\n")
    }

    /// For `status`: `open:TICKED/ALL[:close]`.
    pub fn status(&self) -> String {
        let n = self.picks.values().filter(|v| **v).count();
        format!("open:{n}/{}{}", self.picks.len(), if self.then.is_some() { ":close" } else { "" })
    }
}

impl Editor {
    /// The key of the open project in the editor's settings: its project file, else its folder.
    fn cleanup_key(&self) -> String {
        self.project_file.as_ref().map_or_else(|| self.project.root.display().to_string(), |pf| pf.path.display().to_string())
    }

    fn cleanup_kept(&self) -> BTreeSet<String> {
        crate::files::load_settings().cleanup_kept.get(&self.cleanup_key()).cloned().unwrap_or_default()
    }

    /// Where a cleanup's backup goes: `<name>-backup-cleanup[-N]` beside the project file (or folder).
    fn cleanup_backup(&self) -> PathBuf {
        let at = self.project_file.as_ref().map_or_else(|| self.project.root.clone(), |pf| pf.path.clone());
        let dir = at.parent().map_or_else(|| PathBuf::from("."), |d| d.to_path_buf());
        let stem = at.file_stem().map_or_else(|| "project".to_string(), |s| s.to_string_lossy().into_owned());
        crate::app::free_backup(&dir, &stem, "cleanup", "")
    }

    /// Assets › Clean up unused files…: everything unused, the entries kept before unticked.
    pub fn open_cleanup(&mut self) {
        if self.defs_dirty {
            // The cleanup reads and writes the definitions as saved.
            self.save_defs();
        }
        match unused::find_in(&self.project) {
            Ok(u) => {
                let unused = u.deletable();
                let kept = self.cleanup_kept();
                let mut c = Cleanup { unused, picks: BTreeMap::new(), kept, then: None, disk: None };
                c.picks = c.entries().into_iter().map(|(k, _, _)| (k.clone(), !c.kept.contains(&k))).collect();
                self.cleanup = Some(c);
            }
            Err(e) => self.set_status(format!("The project can't be checked for unused files: {e:#}")),
        }
    }

    /// When the project closes (`p`): opens the cleanup window if the project has unused assets
    /// it hasn't kept before, and gives `p` back otherwise (the window goes on with it).
    pub(crate) fn offer_cleanup(&mut self, p: Pending) -> Option<Pending> {
        let leaving = match &p {
            Pending::Quit | Pending::OpenProject(_) | Pending::CloseProject => true,
            // (An empty path opens the new project wizard first: the project stays open.)
            Pending::NewProject(path) => !path.as_os_str().is_empty(),
            _ => false,
        };
        if !leaving || !self.project_open || self.library_mode || std::mem::take(&mut self.cleanup_passed) {
            return Some(p);
        }
        // The project as saved: after "Discard" the editor's copy still holds the discarded edits.
        let disk = match Project::load(&self.project.root) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[editor] cleanup: can't read the project: {e:#}");
                return Some(p);
            }
        };
        let found = match unused::find_in(&disk) {
            Ok(u) => u.deletable(),
            Err(e) => {
                eprintln!("[editor] cleanup: {e:#}");
                return Some(p);
            }
        };
        let kept = self.cleanup_kept();
        let unused = Unused {
            defs: found.defs.into_iter().filter(|d| !kept.contains(&def_key(d.kind, &d.id))).collect(),
            files: found.files.into_iter().filter(|(f, _)| !kept.contains(&file_key(f))).collect(),
        };
        if unused.is_empty() {
            return Some(p);
        }
        let mut c = Cleanup { unused, picks: BTreeMap::new(), kept, then: Some(p), disk: Some(disk) };
        c.picks = c.entries().into_iter().map(|(k, _, _)| (k, true)).collect();
        self.cleanup = Some(c);
        None
    }

    /// Answers the window: `delete` = deletes the ticked entries (else keeps everything); the
    /// unticked ones are remembered as kept. A closing project then closes.
    pub fn cleanup_answer(&mut self, delete: bool) {
        let Some(c) = self.cleanup.take() else { return };
        let (defs, files) = if delete { c.ticked() } else { Default::default() };
        // Remember what stays.
        let key = self.cleanup_key();
        let mut s = crate::files::load_settings();
        let kept = s.cleanup_kept.entry(key.clone()).or_default();
        for (k, on) in &c.picks {
            if delete && *on {
                kept.remove(k);
            } else {
                kept.insert(k.clone());
            }
        }
        if kept.is_empty() {
            s.cleanup_kept.remove(&key);
        }
        crate::files::save_settings(&s);
        if !defs.is_empty() || !files.is_empty() {
            let backup = self.cleanup_backup();
            let from_menu = c.disk.is_none();
            let res = match c.disk {
                Some(mut disk) => unused::delete(&mut disk, &c.unused, &defs, &files, &backup),
                None => unused::delete(&mut self.project, &c.unused, &defs, &files, &backup),
            };
            match res {
                Ok(done) => {
                    if from_menu {
                        // The definitions on disk are the editor's now; undo steps would bring deleted ones back.
                        self.defs_dirty = false;
                        *self.defs_saved = (self.project.db.clone(), self.project.config.clone());
                        self.db_hist.clear();
                        self.reload_assets();
                    }
                    self.set_status(format!("Deleted {} unused definition(s) and {} file(s), {}; a copy is in {}", done.defs, done.files, size(done.bytes), backup.display()));
                }
                Err(e) => {
                    self.set_status(format!("Cleaning up failed: {e:#}; the files that were deleted are in {}", backup.display()));
                    // Don't close over a half-done cleanup.
                    return;
                }
            }
        }
        if let Some(p) = c.then {
            self.cleanup_passed = true;
            self.request(p);
        }
    }

    /// The cleanup window, while open.
    pub fn cleanup_window(&mut self, ctx: &egui::Context) {
        let Some(c) = &mut self.cleanup else { return };
        let closing = c.then.is_some();
        let mut answer: Option<Option<bool>> = None;
        let entries = c.entries();
        egui::Window::new(ui::t(ph::BROOM, "Clean up unused files")).collapsible(false).resizable(true).default_width(560.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            if entries.is_empty() {
                ui.label("Nothing to clean up: the project uses every model, sprite sheet, sound, material and font it has.");
            } else {
                ui.label("The project has these but nothing uses them: no map, character, item, script or other definition names them. Ticked ones are deleted; a copy goes into a backup folder beside the project first. Exported games leave them out either way.");
                ui.weak("Unticked ones are kept and not offered again when the project closes (this window, from Assets, shows everything).");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.small_button("Tick all").clicked() {
                        c.picks.values_mut().for_each(|v| *v = true);
                    }
                    if ui.small_button("Untick all").clicked() {
                        c.picks.values_mut().for_each(|v| *v = false);
                    }
                    ui.weak(format!("{} of {} ticked, {}", c.picks.values().filter(|v| **v).count(), entries.len(), size(c.ticked_bytes())));
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                    egui::Grid::new("cleanup_list").num_columns(3).striped(true).show(ui, |ui| {
                        for (k, files, bytes) in &entries {
                            let (kind, id) = k.split_once(':').unwrap_or(("", k));
                            let on = c.picks.entry(k.clone()).or_insert(false);
                            let text = if kind == "file" { format!("{id}  (no definition)") } else { format!("{kind}  {id}") };
                            ui.checkbox(on, text).on_hover_text(if files.is_empty() { "Its file is used by something else: only the definition goes".to_string() } else { format!("Deletes {files}") });
                            ui.weak(size(*bytes));
                            ui.weak(if c.kept.contains(k) { "kept before" } else { "" });
                            ui.end_row();
                        }
                    });
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                let any = c.picks.values().any(|v| *v);
                let del = if closing { "Delete ticked and close" } else { "Delete ticked" };
                if ui.add_enabled(any, egui::Button::new(ui::t(ph::TRASH, del))).clicked() {
                    answer = Some(Some(true));
                }
                if closing {
                    if ui.button("Keep all and close").clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button("Cancel").on_hover_text("Stays in the project").clicked() {
                        answer = Some(None);
                    }
                } else if ui.button("Close").on_hover_text("Deletes nothing").clicked() {
                    answer = Some(None);
                }
            });
        });
        match answer {
            Some(Some(delete)) => self.cleanup_answer(delete),
            Some(None) => self.cleanup = None,
            None => {}
        }
    }
}
