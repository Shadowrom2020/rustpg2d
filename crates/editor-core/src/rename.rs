//! Renaming ids: a definition, a map or a prop's state, with every reference in the definitions,
//! the project settings, every map and the scripts following.

use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::refactor::{self, RefKind};

use crate::history::Extra;
use crate::workspace::Workspace;

/// Ids are letters, digits, `_` and `-`.
pub fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Renames a prop state in "Set prop state" actions and "Prop state" conditions aimed at "self".
pub fn rename_state_in_triggers(ts: &mut [rustpg2d_engine::events::Trigger], old: &str, new: &str) {
    use rustpg2d_engine::events::{Action, Cond};
    fn actions(acts: &mut [Action], old: &str, new: &str) {
        rustpg2d_engine::events::walk_actions_mut(acts, &mut |a| match a {
            Action::SetState { target, state } if (target == "self" || target.is_empty()) && state == old => *state = new.to_string(),
            Action::If { conditions, .. } => conds(conditions, old, new),
            _ => {}
        });
    }
    fn conds(cs: &mut [rustpg2d_engine::events::Condition], old: &str, new: &str) {
        for c in cs {
            if let Cond::PropState { target, state } = &mut c.cond {
                if (target == "self" || target.is_empty()) && state == old {
                    *state = new.to_string();
                }
            }
        }
    }
    for t in ts {
        conds(&mut t.conditions, old, new);
        actions(&mut t.actions, old, new);
    }
}

impl Workspace {
    /// Follows a renamed prop state in placed props (open maps and maps on disk): their start
    /// state, lock / container overrides and their own triggers aimed at "self". (The state
    /// itself was renamed in the definition by the caller.)
    pub fn rename_prop_state(&mut self, prop: &str, old: &str, new: &str) {
        let fix = |e: &mut EntityInstance| {
            if e.kind != EntityKind::Prop || e.def != prop {
                return false;
            }
            let before = format!("{:?}{:?}{:?}{:?}", e.state, e.lock, e.container, e.triggers);
            if e.state == old {
                e.state = new.to_string();
            }
            if let Some(l) = &mut e.lock {
                if let rustpg2d_engine::defs::OnUnlock::ToState { state } = &mut l.on_unlock {
                    if state == old {
                        *state = new.to_string();
                    }
                }
            }
            if let Some(c) = &mut e.container {
                if c.open_state == old {
                    c.open_state = new.to_string();
                }
            }
            rename_state_in_triggers(&mut e.triggers, old, new);
            before != format!("{:?}{:?}{:?}{:?}", e.state, e.lock, e.container, e.triggers)
        };
        let mut changed = 0;
        let mut open = Vec::new();
        for doc in self.doc.iter_mut().chain(self.parked.values_mut()) {
            open.push(doc.map.name.clone());
            let n = doc.map.entities.iter_mut().map(|e| fix(e) as usize).sum::<usize>();
            if n > 0 {
                changed += n;
                doc.mark_changed();
            }
        }
        for m in self.maps.clone() {
            if open.contains(&m) {
                continue;
            }
            let Ok(mut map) = self.project.load_map(&m) else { continue };
            let n = map.entities.iter_mut().map(|e| fix(e) as usize).sum::<usize>();
            if n > 0 && self.project.save_map(&map).is_ok() {
                changed += n;
            }
        }
        self.defs_dirty = true;
        self.db_hist.record(Extra::StateRename { prop: prop.to_string(), old: old.to_string(), new: new.to_string() });
        self.set_status(format!("Renamed state '{old}' to '{new}' of '{prop}' ({changed} placed prop(s) updated). Save the definitions to keep it."));
    }

    /// [`Self::try_rename_id`], its outcome as the status message.
    pub fn rename_id(&mut self, kind: RefKind, old: &str, new: &str) {
        match self.try_rename_id(kind, old, new) {
            Ok(msg) => self.set_status(msg),
            Err(e) => self.set_status(e),
        }
    }

    /// Renames `old` to `new` everywhere (its definition or map file, project settings, every
    /// map, scripts) and says what changed. The definitions are saved at once: the maps on disk
    /// already name the new id.
    pub fn try_rename_id(&mut self, kind: RefKind, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new == old || new.is_empty() {
            return Err("the new id is empty or the same as the old one".into());
        }
        if !valid_name(new) {
            return Err("Ids may only contain letters, digits, '_' and '-'".into());
        }
        let taken = if kind == RefKind::Map { self.maps.iter().any(|m| m == new) } else { self.project.db.has(kind, new) };
        if taken {
            return Err(format!("'{new}' is already used"));
        }
        // The thing itself.
        if kind == RefKind::Map {
            if let Err(e) = self.project.rename_map_file(old, new) {
                return Err(format!("Rename failed: {e:#}"));
            }
            for m in &mut self.maps {
                if m == old {
                    *m = new.to_string();
                }
            }
            self.maps.sort();
            // An open map of that name follows.
            if let Some(mut d) = self.parked.remove(old) {
                d.map.name = new.to_string();
                self.parked.insert(new.to_string(), d);
            }
            if let Some(d) = self.doc.as_mut().filter(|d| d.map.name == old) {
                d.map.name = new.to_string();
            }
        } else {
            self.project.db.rename(kind, old, new);
        }
        let mut refs = refactor::rename_in_db(&mut self.project.db, &mut self.project.config, kind, old, new);

        // Every map on disk, then the open ones (which may have unsaved edits).
        let mut maps_changed = 0;
        let mut errors = Vec::new();
        for name in self.maps.clone() {
            match self.project.load_map(&name) {
                Ok(mut map) => {
                    let n = refactor::rename_in_map(&mut map, kind, old, new);
                    if n > 0 {
                        refs += n;
                        maps_changed += 1;
                        if let Err(e) = self.project.save_map(&map) {
                            errors.push(format!("{name}: {e:#}"));
                        }
                    }
                }
                Err(e) => errors.push(format!("{name}: {e:#}")),
            }
        }
        for doc in self.doc.iter_mut().chain(self.parked.values_mut()) {
            if refactor::rename_in_map(&mut doc.map, kind, old, new) > 0 {
                // Old snapshots still contain the old id.
                doc.mark_changed();
                doc.clear_history();
            }
        }
        self.save_defs();
        if matches!(kind, RefKind::Material | RefKind::Sheet | RefKind::Sound) {
            self.reload_assets();
        }
        self.refresh_refs();
        let mut msg = format!("Renamed '{old}' to '{new}': {refs} reference(s) updated in {maps_changed} map file(s)");
        // Ids passed to script functions (give_item("old"), teleport("old", ...), ...).
        let changed = self.rename_in_scripts(kind, old, new, &mut errors);
        if !changed.is_empty() {
            msg.push_str(&format!(", scripts: {}", changed.join(", ")));
        }
        if !errors.is_empty() {
            msg = format!("{msg}; errors: {}", errors.join("; "));
        }
        self.meta_renamed(kind, old, new);
        self.db_hist.record(Extra::Rename { kind, old: old.to_string(), new: new.to_string() });
        Ok(msg)
    }

    /// Applies a rename to the project's scripts (backups as `<file>.rhai.bak`). Returns
    /// "file:line,line" for every changed file. Whoever shows a script reloads it when
    /// [`Workspace::scripts_rev`] moved.
    fn rename_in_scripts(&mut self, kind: RefKind, old: &str, new: &str, errors: &mut Vec<String>) -> Vec<String> {
        let mut changed = Vec::new();
        for f in self.project.script_files() {
            let Ok(disk) = std::fs::read_to_string(&f) else { continue };
            let (text, lines) = rustpg2d_engine::script_refs::rename_in_script(&disk, kind, old, new);
            if lines.is_empty() {
                continue;
            }
            let mut bak = f.clone().into_os_string();
            bak.push(".bak");
            if let Err(e) = std::fs::write(&bak, &disk).and_then(|_| std::fs::write(&f, &text)) {
                errors.push(format!("{}: {e}", f.display()));
                continue;
            }
            let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
            let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            changed.push(format!("{name}:{}", lines.join(",")));
        }
        if !changed.is_empty() {
            self.scripts_rev += 1;
        }
        changed
    }
}
