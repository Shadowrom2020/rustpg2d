//! *Clean up unused files…*: what the project has and doesn't use, to tick; what is ticked
//! is deleted (a copy goes beside the project), what isn't is remembered as kept.
//!
//! The logic is `editor_core::cleanup`.

use eframe::egui;
use editor_core::cleanup::{size, Cleanup};
use editor_core::Workspace;

pub struct CleanupWindow {
    pub cleanup: Cleanup,
}

impl CleanupWindow {
    /// Looks through the project (it is saved first).
    pub fn open(ws: &mut Workspace) -> Result<CleanupWindow, String> {
        Ok(CleanupWindow { cleanup: ws.cleanup_find()? })
    }

    /// The script's lines: one an entry (`x sound:old_bell 12 KB sounds/old_bell.ogg`).
    pub fn listing(&self) -> Vec<String> {
        self.cleanup.entries().iter().map(|e| format!("{} {} {} {}", if self.cleanup.ticked(&e.key) { "x" } else { "-" }, e.key, size(e.bytes), e.files.join(", "))).collect()
    }

    /// The window. Returns false once it is answered or closed.
    pub fn show(&mut self, ctx: &egui::Context, ws: &mut Workspace) -> bool {
        let mut answer: Option<bool> = None;
        let mut open = true;
        let entries = self.cleanup.entries();
        egui::Window::new("Clean up unused files").open(&mut open).collapsible(false).resizable(true).default_width(620.0).anchor(egui::Align2::CENTER_CENTER, [0.0, -30.0]).order(egui::Order::Foreground).show(ctx, |ui| {
            if entries.is_empty() {
                ui.label("Nothing is unused: every picture, model, sound, ground and font of the project is named by something, and no file lies about in their folders.");
                if ui.button("Close").clicked() {
                    answer = Some(false);
                }
                return;
            }
            ui.label("These are in the project and nothing in the game names them: no definition, map, setting or script. An exported game leaves them out anyway; deleting them makes the project smaller.");
            ui.weak("A copy of what is deleted is put beside the project first. What you leave unticked is remembered as kept.");
            ui.horizontal(|ui| {
                if ui.small_button("Tick all").clicked() {
                    self.cleanup.picks.values_mut().for_each(|v| *v = true);
                }
                if ui.small_button("Tick none").clicked() {
                    self.cleanup.picks.values_mut().for_each(|v| *v = false);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
                for e in &entries {
                    ui.horizontal(|ui| {
                        let mut on = self.cleanup.ticked(&e.key);
                        if ui.checkbox(&mut on, e.label.as_str()).on_hover_text(if e.def.is_some() { "A definition nothing names, with the files only it uses" } else { "A file in an asset folder that no definition names" }).changed() {
                            self.cleanup.picks.insert(e.key.clone(), on);
                        }
                        ui.weak(size(e.bytes));
                        if e.def.is_some() && !e.files.is_empty() {
                            ui.weak(e.files.join(", "));
                        }
                    });
                }
            });
            ui.separator();
            let ticked = self.cleanup.picks.values().filter(|v| **v).count();
            ui.horizontal(|ui| {
                if ui.add_enabled(ticked > 0, egui::Button::new(format!("Delete the {ticked} ticked ({})", size(self.cleanup.ticked_bytes())))).on_hover_text("The history is emptied: its steps would bring deleted definitions back without their files").clicked() {
                    answer = Some(true);
                }
                if ui.button("Keep all").on_hover_text("Deletes nothing and remembers them as kept").clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(delete) = answer {
            match ws.cleanup_apply(&self.cleanup, delete) {
                Ok(said) => ws.set_status(said),
                Err(e) => ws.set_status(format!("Cleaning up failed: {e}")),
            }
            return false;
        }
        open
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use rustpg2d_engine::refactor::RefKind;

    struct State {
        ws: Workspace,
        win: Option<CleanupWindow>,
    }

    #[test]
    fn what_is_ticked_is_deleted_and_the_window_closes() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-cleanupwin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let game = dir.join("game");
        std::fs::create_dir_all(&game).unwrap();
        // (What is kept is noted in settings of the test's own.)
        editor_core::files::config_dir_here(Some(dir.join("config")));
        editor_core::files::create_project(&game).unwrap();
        let mut ws = Workspace::new(game.clone(), editor_core::Gpu::No).unwrap();
        ws.history_frame(None);
        for name in ["Old bell", "Old horn"] {
            let file = dir.join(format!("{}.ogg", name.to_lowercase().replace(' ', "_")));
            std::fs::write(&file, b"OggS").unwrap();
            ws.make_from_file(RefKind::Sound, &file, name).unwrap();
        }
        ws.history_frame(None);
        let win = CleanupWindow::open(&mut ws).unwrap();
        assert_eq!(win.listing().len(), 2);
        let mut h = Harness::builder().with_size(egui::vec2(900.0, 700.0)).build_ui_state(
            |ui, s: &mut State| {
                if let Some(mut w) = s.win.take() {
                    if w.show(ui.ctx(), &mut s.ws) {
                        s.win = Some(w);
                    }
                }
            },
            State { ws, win: Some(win) },
        );
        h.run();
        // Both are listed and ticked; the horn is unticked, the rest deleted.
        h.get_by_label("Sound old_bell");
        h.get_by_label("Sound old_horn").click();
        h.run();
        h.get_by_label_contains("Delete the 1 ticked").click();
        h.run_steps(3);
        assert!(h.state().win.is_none(), "answered: the window is gone");
        let s = h.state();
        assert!(s.ws.project.db.sound("old_bell").is_none() && s.ws.project.db.sound("old_horn").is_some());
        assert!(dir.join("game-backup-cleanup/sounds/old_bell.ogg").is_file() && !game.join("sounds/old_bell.ogg").exists());
        assert!(s.ws.status.0.starts_with("Deleted 1 unused definition(s) and 1 file(s)"), "{}", s.ws.status.0);
        editor_core::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
