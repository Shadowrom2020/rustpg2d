//! Undo / redo for definitions and project settings (the Database mode, the character builder,
//! the agent server's writes): once per frame the editor compares them with the previous frame
//! and turns a change into a step (a snapshot of the state before it). Typing into the same
//! text field is one step. Renames rewrite maps and scripts on disk as well, so their steps
//! undo by renaming back instead of restoring a snapshot. Map edits keep their own history
//! (`doc.rs`); Ctrl+Z / Ctrl+Y follow the open mode.

use std::path::PathBuf;

use egui_macroquad::egui;
use rustpg2d_engine::defs::Database;
use rustpg2d_engine::project::ProjectConfig;
use rustpg2d_engine::refactor::RefKind;

use crate::app::Editor;

/// Steps kept (each holds a copy of the definitions).
const MAX_STEPS: usize = 100;
/// More typing into the same field within this many seconds belongs to the same step.
const MERGE_SECS: f64 = 1.5;

type State = Box<(Database, ProjectConfig)>;

/// What a step did besides changing definitions.
#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    /// An id (or map) renamed everywhere: undone by renaming back.
    Rename { kind: RefKind, old: String, new: String },
    /// A prop state renamed, placed props on the maps following: the definitions are restored
    /// and the maps renamed back.
    StateRename { prop: String, old: String, new: String },
}

struct Step {
    /// The definitions on the other side of the step (before it on the undo stack, after it on
    /// the redo stack).
    state: State,
    extra: Option<Extra>,
}

#[derive(Default)]
pub struct DbHistory {
    /// The project these steps belong to.
    root: Option<PathBuf>,
    /// The definitions as the previous frame left them.
    last: Option<State>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The text field the latest step was typed into, and when.
    typing: Option<(egui::Id, f64)>,
    /// A rename made since the last frame (recorded with the step it caused).
    pending: Option<Extra>,
    /// Undo / redo in progress: the renames they replay aren't new steps.
    replaying: bool,
}

impl DbHistory {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// `dbundo=undo/redo` for the editor script's `status`.
    pub fn status(&self) -> String {
        format!("{}/{}", self.undo.len(), self.redo.len())
    }

    /// Forgets every step (another project, definitions reloaded from disk).
    pub fn clear(&mut self) {
        *self = DbHistory::default();
    }

    /// A rename just happened (called by the rename code; ignored while undo / redo replay one).
    pub fn record(&mut self, extra: Extra) {
        if !self.replaying {
            self.pending = Some(extra);
        }
    }

    /// Compares the definitions with the previous frame's and records a step when they changed.
    /// `focus`: the text field that has the keyboard now.
    fn observe(&mut self, root: &PathBuf, db: &Database, config: &ProjectConfig, focus: Option<egui::Id>, now: f64) {
        if self.root.as_ref() != Some(root) {
            self.clear();
            self.root = Some(root.clone());
        }
        let Some(last) = &self.last else {
            self.last = Some(Box::new((db.clone(), config.clone())));
            return;
        };
        let changed = last.0 != *db || last.1 != *config;
        if !changed && self.pending.is_none() {
            return;
        }
        let extra = self.pending.take();
        // Typing on in the same field: the step that field's first change made covers it.
        let same_field = extra.is_none()
            && focus.is_some()
            && self.typing.is_some_and(|(id, t)| Some(id) == focus && now - t < MERGE_SECS)
            && self.undo.last().is_some_and(|s| s.extra.is_none());
        if !same_field {
            let before = self.last.take().expect("checked above");
            self.undo.push(Step { state: before, extra });
            if self.undo.len() > MAX_STEPS {
                self.undo.remove(0);
            }
        }
        self.typing = focus.map(|f| (f, now));
        self.redo.clear();
        self.last = Some(Box::new((db.clone(), config.clone())));
    }

    /// After undo / redo: the current state is the new baseline.
    fn rebase(&mut self, db: &Database, config: &ProjectConfig) {
        self.last = Some(Box::new((db.clone(), config.clone())));
        self.typing = None;
        self.pending = None;
    }
}

impl Editor {
    /// Once per frame: records definition changes as undo steps, and Ctrl+Z / Ctrl+Y in the
    /// Database mode (a text field with the keyboard keeps its own undo).
    pub fn db_history_frame(&mut self, ctx: &egui::Context) {
        if !self.project_open {
            return;
        }
        let focus = ctx.memory(|m| m.focused());
        let now = crate::ui::now();
        let root = self.project.root.clone();
        self.db_hist.observe(&root, &self.project.db, &self.project.config, focus, now);
        if self.mode == crate::app::Mode::Database && !ctx.wants_keyboard_input() {
            let (undo, redo) = ctx.input(|i| {
                let cmd = i.modifiers.command;
                (cmd && !i.modifiers.shift && i.key_pressed(egui::Key::Z), cmd && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))))
            });
            if undo {
                self.db_undo();
            } else if redo {
                self.db_redo();
            }
        }
    }

    /// Undoes the latest definition change.
    pub fn db_undo(&mut self) {
        let Some(step) = self.db_hist.undo.pop() else { return };
        let now: State = Box::new((self.project.db.clone(), self.project.config.clone()));
        let label = self.db_replay(&step, true);
        self.db_hist.redo.push(Step { state: now, extra: step.extra });
        self.db_restored(&format!("Undid {label}"));
    }

    /// Redoes the latest undone definition change.
    pub fn db_redo(&mut self) {
        let Some(step) = self.db_hist.redo.pop() else { return };
        let now: State = Box::new((self.project.db.clone(), self.project.config.clone()));
        let label = self.db_replay(&step, false);
        self.db_hist.undo.push(Step { state: now, extra: step.extra });
        self.db_restored(&format!("Redid {label}"));
    }

    /// Applies a step in one direction; returns what it was, for the status line.
    fn db_replay(&mut self, step: &Step, undo: bool) -> String {
        self.db_hist.replaying = true;
        let assets_before = asset_defs(&self.project.db);
        let label = match &step.extra {
            Some(Extra::Rename { kind, old, new }) => {
                let (from, to) = if undo { (new, old) } else { (old, new) };
                if let Err(e) = self.try_rename_id(*kind, from, to) {
                    self.set_status(format!("Couldn't rename back: {e}"));
                }
                format!("the rename '{old}' \u{E13A} '{new}'")
            }
            Some(Extra::StateRename { prop, old, new }) => {
                self.restore_defs(&step.state);
                let (from, to) = if undo { (new, old) } else { (old, new) };
                self.rename_prop_state(prop, from, to);
                format!("the state rename '{old}' \u{E13A} '{new}' of '{prop}'")
            }
            None => {
                self.restore_defs(&step.state);
                "a change to the definitions".to_string()
            }
        };
        if asset_defs(&self.project.db) != assets_before {
            self.reload_assets();
        }
        self.db_hist.replaying = false;
        label
    }

    fn restore_defs(&mut self, state: &State) {
        self.project.db = state.0.clone();
        self.project.config = state.1.clone();
        self.project.db.reindex();
    }

    /// After undo / redo: dirty flag, selection, references, status.
    fn db_restored(&mut self, msg: &str) {
        self.db_hist.rebase(&self.project.db, &self.project.config);
        self.defs_dirty = self.defs_saved.0 != self.project.db || self.defs_saved.1 != self.project.config;
        let n = crate::database::tab_len(&self.project.db, self.db.tab);
        if n > 0 && self.db.sel >= n {
            self.db.sel = n - 1;
        }
        self.refresh_refs();
        self.set_status(msg);
    }
}

/// The definitions whose files the editor loads as images and sounds.
fn asset_defs(db: &Database) -> (Vec<rustpg2d_engine::defs::TilesetDef>, Vec<rustpg2d_engine::defs::SheetDef>, Vec<rustpg2d_engine::defs::SoundDef>) {
    (db.tilesets.clone(), db.sheets.clone(), db.sounds.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::defs::ItemDef;

    fn db_with(name: &str) -> Database {
        Database { items: vec![ItemDef { id: "potion".into(), name: name.into(), ..Default::default() }], ..Default::default() }
    }

    #[test]
    fn changes_become_steps_and_typing_merges() {
        let (root, cfg) = (PathBuf::from("/p"), ProjectConfig::new("t"));
        let id = egui::Id::new("name field");
        let mut h = DbHistory::default();
        h.observe(&root, &db_with("Potion"), &cfg, None, 0.0);
        assert!(!h.can_undo(), "the first frame is the baseline");
        // Typing "Potion!" letter by letter into one field: one step.
        h.observe(&root, &db_with("Potion!"), &cfg, Some(id), 1.0);
        h.observe(&root, &db_with("Potion!!"), &cfg, Some(id), 1.5);
        assert_eq!(h.undo.len(), 1);
        assert_eq!(h.undo[0].state.0.items[0].name, "Potion");
        // A pause, or another field (or none), starts a new step.
        h.observe(&root, &db_with("Potion!!!"), &cfg, Some(id), 5.0);
        h.observe(&root, &db_with("Elixir"), &cfg, None, 5.1);
        assert_eq!(h.undo.len(), 3);
        // Another project: the history is gone.
        h.observe(&PathBuf::from("/q"), &db_with("X"), &cfg, None, 6.0);
        assert!(!h.can_undo());
    }

    #[test]
    fn renames_are_their_own_steps() {
        let (root, cfg) = (PathBuf::from("/p"), ProjectConfig::new("t"));
        let id = egui::Id::new("f");
        let mut h = DbHistory::default();
        h.observe(&root, &db_with("A"), &cfg, Some(id), 0.0);
        h.observe(&root, &db_with("AB"), &cfg, Some(id), 0.1);
        h.record(Extra::Rename { kind: RefKind::Item, old: "potion".into(), new: "elixir".into() });
        h.observe(&root, &db_with("AB"), &cfg, Some(id), 0.2);
        assert_eq!(h.undo.len(), 2, "never merged into the typing step, even without a visible change");
        assert!(matches!(h.undo[1].extra, Some(Extra::Rename { .. })));
        h.replaying = true;
        h.record(Extra::Rename { kind: RefKind::Item, old: "elixir".into(), new: "potion".into() });
        assert!(h.pending.is_none(), "renames replayed by undo aren't recorded");
    }
}
