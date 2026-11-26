//! One history over everything: the definitions' steps ([`crate::history`]) and each open map's
//! ([`crate::doc`]) in the order they were made, each with a name. *Undo* takes back the latest
//! change wherever it was; a document can also undo its own latest change.
//!
//! Nothing is funnelled through here: forms, tools and agents change things and call
//! [`Workspace::history_frame`], which notices the new steps. [`Workspace::begin`] names what
//! comes next (commands do it with their title).

use std::collections::HashMap;

use crate::workspace::Workspace;

/// Entries kept.
const MAX_CHANGES: usize = 400;

/// Where a change was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Where {
    /// The definitions or the project settings.
    Defs,
    Map(String),
}

impl Where {
    pub fn describe(&self) -> String {
        match self {
            Where::Defs => "definitions".into(),
            Where::Map(m) => format!("map {m}"),
        }
    }
}

/// One undoable change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub at: Where,
    /// What was done, for menus and the history panel ("Rename item", "Paint ground").
    pub label: String,
}

#[derive(Default)]
pub struct Timeline {
    /// Changes made, oldest first.
    pub done: Vec<Change>,
    /// Changes undone, the next to redo last.
    pub undone: Vec<Change>,
    /// How many steps each open map could undo at the last look.
    seen: HashMap<String, usize>,
    /// The name for the changes the next look finds.
    label: Option<String>,
}

impl Timeline {
    /// `history=done/undone` for `status`.
    pub fn status(&self) -> String {
        format!("{}/{}", self.done.len(), self.undone.len())
    }
}

impl Workspace {
    /// Names the changes made from here until the next [`Workspace::history_frame`].
    pub fn begin(&mut self, label: &str) {
        self.timeline.label = Some(label.to_string());
    }

    /// Notes new steps: of the definitions when `defs_step`, and of every open map whose undo
    /// history grew since the last look.
    pub(crate) fn timeline_frame(&mut self, defs_step: bool) {
        let label = self.timeline.label.take();
        let mut new = Vec::new();
        if defs_step {
            new.push(Change { at: Where::Defs, label: label.clone().unwrap_or_else(|| "Change definitions".into()) });
        }
        let depths: Vec<(String, usize)> = self.doc.iter().chain(self.parked.values()).map(|d| (d.map.name.clone(), d.undo_depth())).collect();
        for (name, depth) in &depths {
            let was = self.timeline.seen.get(name).copied().unwrap_or(0);
            if *depth > was {
                for _ in was..*depth {
                    new.push(Change { at: Where::Map(name.clone()), label: label.clone().unwrap_or_else(|| "Edit map".into()) });
                }
            } else if *depth < was {
                // Steps went away behind our back (the history was cleared by a rename, or the
                // map was undone directly): its newest entries go with them.
                let mut drop = was - *depth;
                let mut k = self.timeline.done.len();
                while drop > 0 && k > 0 {
                    k -= 1;
                    if self.timeline.done[k].at == Where::Map(name.clone()) {
                        self.timeline.done.remove(k);
                        drop -= 1;
                    }
                }
            }
        }
        self.timeline.seen = depths.into_iter().collect();
        if !new.is_empty() {
            self.timeline.undone.clear();
            self.timeline.done.extend(new);
            let over = self.timeline.done.len().saturating_sub(MAX_CHANGES);
            self.timeline.done.drain(..over);
            self.stale = true;
            self.search_stale = true;
        }
    }

    fn doc_named(&mut self, name: &str) -> bool {
        self.doc.as_ref().is_some_and(|d| d.map.name == name) || self.parked.contains_key(name)
    }

    /// Applies one change backwards or forwards where it was made. False when its step is gone.
    fn replay(&mut self, change: &Change, undo: bool) -> bool {
        match &change.at {
            Where::Defs => {
                let can = if undo { self.db_hist.can_undo() } else { self.db_hist.can_redo() };
                if can {
                    if undo {
                        self.db_undo();
                    } else {
                        self.db_redo();
                    }
                }
                can
            }
            Where::Map(name) => {
                if !self.doc_named(name) {
                    return false;
                }
                let assets = &self.assets;
                let doc = match self.doc.as_mut().filter(|d| d.map.name == *name) {
                    Some(d) => d,
                    None => self.parked.get_mut(name).expect("checked above"),
                };
                let can = if undo { doc.can_undo() } else { doc.can_redo() };
                if can {
                    if undo {
                        doc.undo(assets);
                    } else {
                        doc.redo(assets);
                    }
                    self.timeline.seen.insert(name.clone(), doc.undo_depth());
                }
                can
            }
        }
    }

    /// Takes back the latest change, wherever it was made. Says what it was.
    pub fn undo(&mut self) -> Option<String> {
        self.history_frame(None);
        while let Some(change) = self.timeline.done.pop() {
            if self.replay(&change, true) {
                let msg = format!("Undid: {} ({})", change.label, change.at.describe());
                self.timeline.undone.push(change);
                self.stale = true;
                return Some(msg);
            }
        }
        None
    }

    /// Makes the latest undone change again.
    pub fn redo(&mut self) -> Option<String> {
        while let Some(change) = self.timeline.undone.pop() {
            if self.replay(&change, false) {
                let msg = format!("Redid: {} ({})", change.label, change.at.describe());
                self.timeline.done.push(change);
                self.stale = true;
                return Some(msg);
            }
        }
        None
    }

    /// Takes back the latest change made at `at` (a document's own undo), leaving later changes
    /// elsewhere alone.
    pub fn undo_at(&mut self, at: &Where) -> Option<String> {
        self.history_frame(None);
        let k = self.timeline.done.iter().rposition(|c| c.at == *at)?;
        let change = self.timeline.done.remove(k);
        if self.replay(&change, true) {
            let msg = format!("Undid: {} ({})", change.label, change.at.describe());
            self.timeline.undone.push(change);
            self.stale = true;
            return Some(msg);
        }
        None
    }

    /// Makes the latest undone change of `at` again.
    pub fn redo_at(&mut self, at: &Where) -> Option<String> {
        let k = self.timeline.undone.iter().rposition(|c| c.at == *at)?;
        let change = self.timeline.undone.remove(k);
        if self.replay(&change, false) {
            let msg = format!("Redid: {} ({})", change.label, change.at.describe());
            self.timeline.done.push(change);
            self.stale = true;
            return Some(msg);
        }
        None
    }

    pub fn can_undo(&self) -> bool {
        !self.timeline.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.timeline.undone.is_empty()
    }
}
