//! Clean up unused files: the pictures, models, sounds, ground and fonts nothing in the game
//! names, and files in the asset folders no definition names. What is ticked is deleted after
//! a copy of it is put beside the project; what isn't is remembered as kept, so it isn't
//! offered ticked again. What counts as unused is the engine's (`unused.rs`: the same list
//! leaves the files out of an exported game).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use rustpg2d_engine::refactor::RefKind;
use rustpg2d_engine::unused::{self, Unused};

use crate::workspace::Workspace;

/// How an unused definition is named in lists and settings: `model:old_guard`.
pub fn def_key(kind: RefKind, id: &str) -> String {
    format!("{}:{id}", kind.name())
}

/// How a stray file is named: `file:sprites/old.png`.
pub fn file_key(file: &str) -> String {
    format!("file:{file}")
}

/// "1.4 MB", "12 KB".
pub fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

/// What a project has and doesn't use, with what is ticked to go.
#[derive(Clone, Debug, PartialEq)]
pub struct Cleanup {
    pub unused: Unused,
    /// By key: ticked (it is deleted).
    pub picks: BTreeMap<String, bool>,
}

/// One line of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: String,
    /// "3D model old_guard", "sprites/old.png".
    pub label: String,
    /// The files that go with it.
    pub files: Vec<String>,
    pub bytes: u64,
    /// A definition (else a file no definition names).
    pub def: Option<(RefKind, String)>,
}

impl Cleanup {
    pub fn entries(&self) -> Vec<Entry> {
        let mut out: Vec<Entry> = self.unused.defs.iter().map(|d| Entry { key: def_key(d.kind, &d.id), label: format!("{} {}", d.kind.label(), d.id), files: d.files.clone(), bytes: d.bytes, def: Some((d.kind, d.id.clone())) }).collect();
        out.extend(self.unused.files.iter().map(|(f, b)| Entry { key: file_key(f), label: f.clone(), files: vec![f.clone()], bytes: *b, def: None }));
        out
    }

    pub fn ticked(&self, key: &str) -> bool {
        self.picks.get(key).copied().unwrap_or(false)
    }

    /// The bytes what is ticked takes.
    pub fn ticked_bytes(&self) -> u64 {
        self.entries().iter().filter(|e| self.ticked(&e.key)).map(|e| e.bytes).sum()
    }

    /// `TICKED/ALL` for scripts.
    pub fn status(&self) -> String {
        format!("{}/{}", self.picks.values().filter(|v| **v).count(), self.picks.len())
    }
}

impl Workspace {
    /// The open project in the editor's settings: its project file, else its folder.
    fn cleanup_key(&self) -> String {
        self.project_location().display().to_string()
    }

    /// Looks for what the project doesn't use (the definitions are saved first: the list is
    /// of the project as it is on disk). What was kept before comes unticked.
    pub fn cleanup_find(&mut self) -> Result<Cleanup, String> {
        if !self.project_open {
            return Err("no project is open".into());
        }
        if self.is_dirty() {
            self.save_all();
        }
        let unused = unused::find_in(&self.project).map_err(|e| format!("{e:#}"))?.deletable();
        let kept = crate::files::load_settings().cleanup_kept.get(&self.cleanup_key()).cloned().unwrap_or_default();
        let mut c = Cleanup { unused, picks: BTreeMap::new() };
        c.picks = c.entries().into_iter().map(|e| (e.key.clone(), !kept.contains(&e.key))).collect();
        Ok(c)
    }

    /// Where the copy of what is deleted goes: `<name>-backup-cleanup[-N]` beside the project
    /// (not inside it: the project is to get smaller).
    fn cleanup_backup(&self) -> PathBuf {
        let at = self.project_location();
        let dir = at.parent().map_or_else(|| PathBuf::from("."), |d| d.to_path_buf());
        let stem = at.file_stem().map_or_else(|| "project".to_string(), |s| s.to_string_lossy().into_owned());
        let mut n = 1;
        loop {
            let p = dir.join(if n == 1 { format!("{stem}-backup-cleanup") } else { format!("{stem}-backup-cleanup-{n}") });
            if !p.exists() {
                return p;
            }
            n += 1;
        }
    }

    /// Deletes what is ticked (`delete`; else nothing) and remembers the rest as kept. Says
    /// what was done. The history is cleared: its steps would bring deleted definitions back
    /// without their files.
    pub fn cleanup_apply(&mut self, c: &Cleanup, delete: bool) -> Result<String, String> {
        let defs: BTreeSet<(RefKind, String)> = if delete { c.unused.defs.iter().filter(|d| c.ticked(&def_key(d.kind, &d.id))).map(|d| (d.kind, d.id.clone())).collect() } else { BTreeSet::new() };
        let files: BTreeSet<String> = if delete { c.unused.files.iter().filter(|(f, _)| c.ticked(&file_key(f))).map(|(f, _)| f.clone()).collect() } else { BTreeSet::new() };
        // What stays is remembered (per project, in the editor's settings).
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
        if defs.is_empty() && files.is_empty() {
            return Ok(if delete { "Nothing was ticked: nothing was deleted".to_string() } else { format!("Kept all {}: they aren't offered ticked again", c.picks.len()) });
        }
        let backup = self.cleanup_backup();
        match unused::delete(&mut self.project, &c.unused, &defs, &files, &backup) {
            Ok(done) => {
                self.defs_dirty = false;
                *self.defs_saved = (self.project.db.clone(), self.project.config.clone());
                self.forget_defs_steps();
                self.reload_assets();
                self.refresh_refs();
                self.watch_resnapshot();
                self.problems.stale = true;
                self.search_stale = true;
                self.stale = true;
                Ok(format!("Deleted {} unused definition(s) and {} file(s), {}; a copy is in {}", done.defs, done.files, size(done.bytes), backup.display()))
            }
            Err(e) => Err(format!("{e:#}; what was deleted before that is in {}", backup.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_nothing_uses_is_listed_deleted_with_a_copy_and_what_is_kept_is_remembered() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-cleanup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let game = dir.join("game");
        std::fs::create_dir_all(&game).unwrap();
        // (The settings the kept list is written into are the test's own.)
        crate::files::config_dir_here(Some(dir.join("config")));
        crate::files::create_project(&game).unwrap();
        let mut ws = Workspace::new(game.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        // Two sounds nothing plays, and a picture no definition names.
        for name in ["Old bell", "Old horn"] {
            let file = dir.join(format!("{}.ogg", name.to_lowercase().replace(' ', "_")));
            std::fs::write(&file, b"OggS").unwrap();
            ws.make_from_file(RefKind::Sound, &file, name).unwrap();
        }
        ws.history_frame(None);
        std::fs::create_dir_all(game.join("sprites")).unwrap();
        std::fs::write(game.join("sprites/stray.png"), rustpg2d_engine::imageio::Rgba::new(8, 8).encode().unwrap()).unwrap();
        let mut c = ws.cleanup_find().unwrap();
        assert!(!ws.is_dirty(), "the project is saved first");
        let keys: Vec<String> = c.entries().iter().map(|e| e.key.clone()).collect();
        assert!(keys.contains(&"sound:old_bell".to_string()) && keys.contains(&"sound:old_horn".to_string()) && keys.contains(&"file:sprites/stray.png".to_string()), "{keys:?}");
        assert!(c.picks.values().all(|v| *v), "everything is ticked the first time");
        // The horn is kept: the bell and the stray picture go, a copy of them beside the project.
        c.picks.insert("sound:old_horn".into(), false);
        let said = ws.cleanup_apply(&c, true).unwrap();
        assert!(said.starts_with("Deleted 1 unused definition(s) and 2 file(s)"), "{said}");
        assert!(ws.project.db.sound("old_bell").is_none() && ws.project.db.sound("old_horn").is_some());
        assert!(!game.join("sounds/old_bell.ogg").exists() && !game.join("sprites/stray.png").exists() && game.join("sounds/old_horn.ogg").is_file());
        let backup = dir.join("game-backup-cleanup");
        assert!(backup.join("sounds/old_bell.ogg").is_file() && backup.join("sprites/stray.png").is_file() && backup.join("defs/sounds.ron").is_file());
        assert!(!ws.can_undo(), "no step brings a deleted definition back without its file");
        // Looked at again: the horn is still unused, and comes unticked.
        let c = ws.cleanup_find().unwrap();
        assert_eq!(c.entries().iter().map(|e| (e.key.as_str(), c.ticked(&e.key))).collect::<Vec<_>>(), [("sound:old_horn", false)]);
        assert_eq!(c.status(), "0/1");
        // Deleting with nothing ticked deletes nothing; a second copy would get a name of its own.
        assert!(ws.cleanup_apply(&c, true).unwrap().contains("nothing was deleted"));
        assert!(ws.cleanup_backup().ends_with("game-backup-cleanup-2"));
        crate::files::config_dir_here(None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
