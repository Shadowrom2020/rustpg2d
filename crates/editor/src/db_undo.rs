//! Undo / redo for definitions and project settings (the Database mode, the character builder,
//! the agent server's writes): once per frame the editor compares them with the previous frame
//! and turns a change into a step. A step keeps what differs on its other side, not a copy of
//! everything (a project's definitions are megabytes): the lists that changed, and of a list in
//! which one definition changed only that one. Typing into the same text field is one step. Renames rewrite maps and scripts on disk as well, so their steps
//! undo by renaming back instead of restoring a snapshot. Map edits keep their own history
//! (`doc.rs`); Ctrl+Z / Ctrl+Y follow the open mode.

use std::path::PathBuf;

use egui_macroquad::egui;
use rustpg2d_engine::defs::Database;
use rustpg2d_engine::project::ProjectConfig;
use rustpg2d_engine::refactor::RefKind;

use crate::app::Editor;

/// Steps kept (each holds what it changed).
const MAX_STEPS: usize = 100;
/// More typing into the same field within this many seconds belongs to the same step.
const MERGE_SECS: f64 = 1.5;

pub type State = Box<(Database, ProjectConfig)>;

/// The lists of a [`Database`], each with the bit a [`Part`] marks it with.
macro_rules! each_list {
    ($m:ident) => {
        $m!(sheets, 0);
        $m!(items, 1);
        $m!(props, 2);
        $m!(characters, 3);
        $m!(sounds, 4);
        $m!(quests, 5);
        $m!(structures, 6);
        $m!(spells, 7);
        $m!(recipes, 8);
        $m!(achievements, 9);
        $m!(races, 10);
        $m!(models, 11);
        $m!(materials, 12);
        $m!(scatter, 13);
        $m!(fonts, 14);
        $m!(skins, 15);
        $m!(common_events, 16);
        $m!(backgrounds, 17);
        $m!(classes, 18);
        $m!(diseases, 19);
        $m!(cutscenes, 20);
        $m!(animations, 21);
        $m!(visual_scripts, 22);
    };
}
const LISTS: usize = 23;

/// Fails to compile when [`Database`] gains a list: add it to [`each_list`] then.
#[allow(dead_code)]
fn every_list_is_known(db: Database) {
    let Database { sheets: _, items: _, props: _, characters: _, sounds: _, quests: _, structures: _, spells: _, recipes: _, achievements: _, races: _, models: _, materials: _, scatter: _, fonts: _, skins: _, common_events: _, backgrounds: _, classes: _, diseases: _, cutscenes: _, animations: _, visual_scripts: _, index: _ } = db;
}

/// One side of a step: the lists that differ across it, as they are on this side, and the
/// project settings when they differ. A list in which a single definition differs holds only
/// that one. [`Part::swap`] exchanges it with the definitions, so the same part serves undo
/// and then redo.
#[derive(Default)]
struct Part {
    lists: Database,
    /// Bit k: list k is held.
    held: u32,
    /// List k holds one definition, the one at this position of the list.
    one: [Option<usize>; LISTS],
    config: Option<Box<ProjectConfig>>,
}

impl Part {
    /// What differs between `old` and the definitions now, as it is in `old`.
    fn diff(old: State, db: &Database, config: &ProjectConfig) -> Part {
        let mut part = Part::default();
        part.absorb(old, db, config);
        part
    }

    /// Adds what differs between `old` (the previous frame) and the definitions now and the
    /// part doesn't hold yet: more typing into a step's field can reach another definition.
    fn absorb(&mut self, old: State, db: &Database, config: &ProjectConfig) {
        let (mut old, old_config) = *old;
        macro_rules! take {
            ($f:ident, $k:expr) => {
                if old.$f != db.$f {
                    let bit = 1u32 << $k;
                    // Where they differ: one position of lists of the same length, else all of it.
                    let at = if old.$f.len() == db.$f.len() {
                        let mut differing = (0..old.$f.len()).filter(|&i| old.$f[i] != db.$f[i]);
                        match (differing.next(), differing.next()) {
                            (Some(i), None) => Some(i),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if self.held & bit == 0 {
                        self.held |= bit;
                        match at {
                            Some(i) => {
                                self.lists.$f = vec![old.$f.swap_remove(i)];
                                self.one[$k] = Some(i);
                            }
                            None => self.lists.$f = std::mem::take(&mut old.$f),
                        }
                    } else if let Some(i) = self.one[$k].filter(|i| at != Some(*i)) {
                        // Held as one definition, and now another one changed too: the whole
                        // list as it was before the step (the previous frame's, with the held
                        // definition back in its place).
                        let mut all = std::mem::take(&mut old.$f);
                        if let (Some(slot), Some(d)) = (all.get_mut(i), self.lists.$f.pop()) {
                            *slot = d;
                        }
                        self.lists.$f = all;
                        self.one[$k] = None;
                    }
                }
            };
        }
        each_list!(take);
        if self.config.is_none() && old_config != *config {
            self.config = Some(Box::new(old_config));
        }
    }

    /// Exchanges what the part holds with the definitions: they are on the step's other side
    /// now, and the part holds the side they were on.
    fn swap(&mut self, db: &mut Database, config: &mut ProjectConfig) {
        macro_rules! swap {
            ($f:ident, $k:expr) => {
                if self.held & (1u32 << $k) != 0 {
                    match self.one[$k] {
                        Some(i) => {
                            if let (Some(theirs), Some(mine)) = (db.$f.get_mut(i), self.lists.$f.first_mut()) {
                                std::mem::swap(theirs, mine);
                            }
                        }
                        None => std::mem::swap(&mut db.$f, &mut self.lists.$f),
                    }
                }
            };
        }
        each_list!(swap);
        if let Some(c) = &mut self.config {
            std::mem::swap(config, &mut **c);
        }
    }
}

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
    /// What the step changed, as it is on its other side (before it on the undo stack, after it
    /// on the redo stack). Nothing for a rename: that is undone by renaming back.
    part: Part,
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
        let before = self.last.take().expect("checked above");
        if same_field {
            if let Some(step) = self.undo.last_mut() {
                step.part.absorb(before, db, config);
            }
        } else {
            let part = if matches!(extra, Some(Extra::Rename { .. })) { Part::default() } else { Part::diff(before, db, config) };
            self.undo.push(Step { part, extra });
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

    /// Lends out the copy of the definitions as the last look left them (right after
    /// [`Editor::db_history_frame`]: as they are now), so the Database view needn't make its
    /// own every frame. [`DbHistory::give_back`] returns it.
    pub fn lend(&mut self) -> Option<State> {
        self.last.take()
    }

    /// Takes a lent copy back (unless the history made a new one meanwhile).
    pub fn give_back(&mut self, state: State) {
        if self.last.is_none() {
            self.last = Some(state);
        }
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
        let Some(mut step) = self.db_hist.undo.pop() else { return };
        let label = self.db_replay(&mut step, true);
        self.db_hist.redo.push(step);
        self.db_restored(&format!("Undid {label}"));
    }

    /// Redoes the latest undone definition change.
    pub fn db_redo(&mut self) {
        let Some(mut step) = self.db_hist.redo.pop() else { return };
        let label = self.db_replay(&mut step, false);
        self.db_hist.undo.push(step);
        self.db_restored(&format!("Redid {label}"));
    }

    /// Applies a step in one direction (its part then holds the side the definitions were on:
    /// the step goes onto the other stack as it is); returns what it was, for the status line.
    fn db_replay(&mut self, step: &mut Step, undo: bool) -> String {
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
                self.restore_defs(&mut step.part);
                let (from, to) = if undo { (new, old) } else { (old, new) };
                self.rename_prop_state(prop, from, to);
                format!("the state rename '{old}' \u{E13A} '{new}' of '{prop}'")
            }
            None => {
                self.restore_defs(&mut step.part);
                "a change to the definitions".to_string()
            }
        };
        if asset_defs(&self.project.db) != assets_before {
            self.reload_assets();
        }
        self.db_hist.replaying = false;
        label
    }

    fn restore_defs(&mut self, part: &mut Part) {
        part.swap(&mut self.project.db, &mut self.project.config);
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

/// The definitions whose files the editor loads as images and sounds (materials: only their
/// textures; edges and the rest are read while drawing).
type AssetDefs = (Vec<(String, String)>, Vec<rustpg2d_engine::defs::SheetDef>, Vec<rustpg2d_engine::defs::SoundDef>);
fn asset_defs(db: &Database) -> AssetDefs {
    (db.materials.iter().map(|m| (m.id.clone(), m.texture.clone())).collect(), db.sheets.clone(), db.sounds.clone())
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
        assert_eq!(h.undo[0].part.lists.items[0].name, "Potion");
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

    fn item(id: &str, name: &str) -> ItemDef {
        ItemDef { id: id.into(), name: name.into(), ..Default::default() }
    }

    fn state(db: &Database, cfg: &ProjectConfig) -> State {
        Box::new((db.clone(), cfg.clone()))
    }

    #[test]
    fn a_step_holds_only_what_changed_and_swaps_both_ways() {
        let cfg = ProjectConfig::new("t");
        let before = Database { items: vec![item("a", "A"), item("b", "B"), item("c", "C")], quests: vec![Default::default()], ..Default::default() };
        // One definition changed: the step holds just it.
        let mut db = before.clone();
        db.items[1].name = "Bee".into();
        let after = db.clone();
        let mut part = Part::diff(state(&before, &cfg), &db, &cfg);
        assert_eq!((part.held, part.one[1], part.lists.items.len()), (1 << 1, Some(1), 1));
        assert!(part.lists.quests.is_empty() && part.config.is_none(), "untouched lists and settings aren't kept");
        let mut c = cfg.clone();
        part.swap(&mut db, &mut c);
        assert!(db == before, "undone");
        part.swap(&mut db, &mut c);
        assert!(db == after, "redone");
        // A definition added and the settings changed: the whole list and the settings.
        let mut db = before.clone();
        db.items.insert(0, item("new", "New"));
        let mut c = cfg.clone();
        c.name = "other".into();
        let (after, after_cfg) = (db.clone(), c.clone());
        let mut part = Part::diff(state(&before, &cfg), &db, &c);
        assert_eq!((part.one[1], part.lists.items.len()), (None, 3));
        part.swap(&mut db, &mut c);
        assert!(db == before && c == cfg);
        part.swap(&mut db, &mut c);
        assert!(db == after && c == after_cfg);
    }

    #[test]
    fn typing_on_that_reaches_another_definition_is_undone_too() {
        let cfg = ProjectConfig::new("t");
        let before = Database { items: vec![item("a", "A"), item("b", "B")], props: vec![Default::default()], ..Default::default() };
        let mut db = before.clone();
        db.items[0].name = "A!".into();
        let mut part = Part::diff(state(&before, &cfg), &db, &cfg);
        // The same step goes on: the same definition again, then another one and another list.
        let frame = db.clone();
        db.items[0].name = "A!!".into();
        part.absorb(state(&frame, &cfg), &db, &cfg);
        assert_eq!(part.one[1], Some(0));
        let frame = db.clone();
        db.items[1].name = "B!".into();
        db.props[0].name = "Crate".into();
        part.absorb(state(&frame, &cfg), &db, &cfg);
        assert_eq!((part.one[1], part.lists.items.len(), part.one[2]), (None, 2, Some(0)));
        let mut c = cfg.clone();
        part.swap(&mut db, &mut c);
        assert!(db == before, "everything the step changed is back");
    }
}
