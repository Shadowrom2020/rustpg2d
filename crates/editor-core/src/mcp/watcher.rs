//! The project file watcher: agents (and other programs) may change `project.ron`, `defs/*.ron`,
//! `maps/*.map`, `scripts/*.rhai` and `lang/*.ron` on disk while the editor is open. Once a
//! second the files' modification times and sizes are compared with the last snapshot; clean
//! documents reload at once, unsaved ones make a banner ask (Reload / Keep mine). The editor
//! never reacts to itself: its saves note the files they wrote ([`Workspace::watch_note`]), agent
//! writes re-snapshot, and a changed open map or script that holds exactly what the editor has is
//! taken as it is (the editor's other writes).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::workspace::Workspace;

#[derive(Default)]
pub struct Watch {
    stamps: BTreeMap<PathBuf, (SystemTime, u64)>,
    last_check: f64,
    /// Files changed on disk that wait for the person's answer (relative names).
    pub changed: Vec<String>,
    /// The snapshot was taken (false right after the project opened).
    ready: bool,
}

impl Watch {
    /// `watch=` for the editor script's `status` line.
    pub fn status(&self) -> String {
        if self.changed.is_empty() {
            "clean".into()
        } else {
            format!("changed:{}", self.changed.join(","))
        }
    }
}

/// What changed, by the editor part that has to react.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Changed {
    Config,
    Defs,
    Map(String),
    Script(PathBuf),
    Lang,
}

fn stamps(root: &std::path::Path) -> BTreeMap<PathBuf, (SystemTime, u64)> {
    let mut out = BTreeMap::new();
    let mut add = |p: PathBuf| {
        if let Ok(m) = std::fs::metadata(&p) {
            if m.is_file() {
                out.insert(p, (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
            }
        }
    };
    add(root.join("project.ron"));
    for (dir, ext) in [("defs", "ron"), ("maps", "map"), ("scripts", "rhai"), ("lang", "ron")] {
        for e in std::fs::read_dir(root.join(dir)).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == ext) {
                add(p);
            }
        }
    }
    out
}

fn classify(root: &std::path::Path, p: &std::path::Path) -> Option<Changed> {
    let rel = p.strip_prefix(root).ok()?;
    let mut parts = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string());
    let (first, second) = (parts.next()?, parts.next());
    Some(match (first.as_str(), second) {
        ("project.ron", None) => Changed::Config,
        ("defs", Some(_)) => Changed::Defs,
        ("maps", Some(f)) => Changed::Map(f.trim_end_matches(".map").to_string()),
        ("scripts", Some(_)) => Changed::Script(p.to_path_buf()),
        ("lang", Some(_)) => Changed::Lang,
        _ => return None,
    })
}

impl Workspace {
    /// The files on disk now match the editor (after the project opened or the editor saved).
    pub fn watch_resnapshot(&mut self) {
        if !self.project_open || self.library_mode {
            return;
        }
        self.watch.stamps = stamps(&self.project.root);
        self.watch.changed.clear();
        self.watch.ready = true;
    }

    /// The editor itself wrote `path`: its new state is no change from disk.
    pub fn watch_note(&mut self, path: &std::path::Path) {
        if !self.watch.ready {
            return;
        }
        match std::fs::metadata(path) {
            Ok(m) if m.is_file() => {
                self.watch.stamps.insert(path.to_path_buf(), (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
            }
            _ => {
                self.watch.stamps.remove(path);
            }
        }
    }

    /// The editor itself wrote the definitions and the settings ([`Project::save_defs`] and
    /// `save_config` write every file that exists, and the visual scripts' code).
    ///
    /// [`Project::save_defs`]: rustpg2d_engine::project::Project::save_defs
    pub fn watch_note_defs(&mut self) {
        let root = self.project.root.clone();
        self.watch_note(&root.join("project.ron"));
        for name in rustpg2d_engine::project::DEFS {
            self.watch_note(&root.join("defs").join(format!("{name}.ron")));
        }
        self.watch_note(&root.join(rustpg2d_engine::visual::FILE));
    }

    /// Called every frame (with a project open): checks once a second.
    pub fn watch_project(&mut self) {
        if !self.project_open || self.library_mode {
            return;
        }
        let now = crate::now();
        if now - self.watch.last_check < 1.0 {
            return;
        }
        self.watch.last_check = now;
        if !self.watch.ready {
            self.watch_resnapshot();
            return;
        }
        if !self.watch.changed.is_empty() {
            return; // waiting for the banner's answer
        }
        let fresh = stamps(&self.project.root);
        let mut changes = Vec::new();
        for (p, s) in &fresh {
            if self.watch.stamps.get(p) != Some(s) {
                if let Some(c) = classify(&self.project.root, p) {
                    changes.push(c);
                }
            }
        }
        for p in self.watch.stamps.keys() {
            if !fresh.contains_key(p) {
                if let Some(c) = classify(&self.project.root, p) {
                    changes.push(c);
                }
            }
        }
        if changes.is_empty() {
            return;
        }
        // (`dedup` only drops neighbours; the same file can be found twice far apart.)
        let mut once: Vec<Changed> = Vec::with_capacity(changes.len());
        for c in changes {
            if !once.contains(&c) {
                once.push(c);
            }
        }
        self.watch_apply(once, false);
    }

    /// Applies disk changes: reloads what is clean; with `force` also what has unsaved edits
    /// (dropping them); otherwise the dirty parts wait for the banner.
    fn watch_apply(&mut self, changes: Vec<Changed>, force: bool) {
        let mut blocked = Vec::new();
        let mut reloaded = Vec::new();
        let open_map = self.doc.as_ref().map(|d| d.map.name.clone());
        let mut map_list = false;
        for c in changes {
            match c {
                Changed::Config | Changed::Defs => {
                    if self.defs_dirty && !force {
                        blocked.push(if c == Changed::Config { "project.ron".to_string() } else { "defs/".to_string() });
                        continue;
                    }
                    let before = (self.project.db.clone(), self.project.config.clone());
                    match self.project.reload() {
                        Ok(()) => {
                            self.defs_dirty = false;
                            *self.defs_saved = (self.project.db.clone(), self.project.config.clone());
                            // Only the editor's own save (the files hold what it has): nothing to
                            // reload or say. Otherwise the undo steps refer to what was there before.
                            if self.project.db != before.0 || self.project.config != before.1 {
                                self.forget_defs_steps();
                                self.reload_assets();
                                reloaded.push("definitions");
                            }
                        }
                        Err(e) => self.set_status(format!("Definitions changed on disk but can't be read: {e:#}")),
                    }
                }
                // A map that is open: the one in front, or one beside it.
                Changed::Map(name) if Some(&name) == open_map.as_ref() || self.parked.contains_key(&name) => {
                    let front = Some(&name) == open_map.as_ref();
                    let path = self.project.map_path(&name);
                    let doc = if front { self.doc.as_ref() } else { self.parked.get(&name) };
                    // The file holds the map as the editor has it (the editor wrote it): nothing
                    // to reload (reloading would also drop the map's undo history).
                    let same = doc.is_some_and(|d| std::fs::read(&path).ok().zip(rustpg2d_engine::map::io::encode(&d.map).ok()).is_some_and(|(disk, mine)| disk == mine));
                    if same {
                        continue;
                    }
                    if doc.is_some_and(|d| d.is_dirty()) && !force {
                        blocked.push(format!("maps/{name}.map"));
                        continue;
                    }
                    if path.exists() {
                        let sel = self.sel;
                        if self.reload_map(&name) {
                            reloaded.push(if front { "the open map" } else { "a map open beside it" });
                        }
                        self.sel = sel;
                    } else {
                        self.close_map(&name);
                        map_list = true;
                        reloaded.push("a map (deleted on disk: closed)");
                    }
                }
                Changed::Map(_) => map_list = true,
                Changed::Script(p) => {
                    let same = std::fs::read_to_string(&p).is_ok_and(|t| t == self.db.script_text);
                    if self.db.script_file.as_ref() == Some(&p) && !same {
                        if self.db.script_dirty && !force {
                            blocked.push(format!("scripts/{}", p.file_name().unwrap_or_default().to_string_lossy()));
                            continue;
                        }
                        if let Ok(text) = std::fs::read_to_string(&p) {
                            self.db.script_text = text.clone();
                            self.db.script_saved = text;
                            self.db.script_dirty = false;
                            reloaded.push("the open script");
                        }
                    }
                    map_list = true; // refresh_refs below picks up the functions
                }
                Changed::Lang => reloaded.push("translations"),
            }
        }
        if map_list {
            self.maps = self.project.list_maps();
            if !reloaded.contains(&"definitions") {
                self.refresh_refs();
            }
        }
        if map_list || !reloaded.is_empty() {
            self.problems.stale = true;
        }
        if !reloaded.is_empty() {
            reloaded.dedup();
            self.set_status(format!("Changed on disk, reloaded: {}", reloaded.join(", ")));
        }
        blocked.sort();
        blocked.dedup();
        self.watch.changed = blocked;
        if self.watch.changed.is_empty() {
            self.watch.stamps = stamps(&self.project.root);
        }
    }

    /// The banner's answer: `reload` drops the unsaved edits and loads the files, else the
    /// editor's state stays (saving later overwrites the files).
    pub fn watch_answer(&mut self, reload: bool) {
        let names = std::mem::take(&mut self.watch.changed);
        if reload {
            let mut changes = Vec::new();
            for n in &names {
                let c = match n.as_str() {
                    "project.ron" => Changed::Config,
                    "defs/" => Changed::Defs,
                    s if s.starts_with("maps/") => Changed::Map(s.trim_start_matches("maps/").trim_end_matches(".map").to_string()),
                    s if s.starts_with("scripts/") => Changed::Script(self.project.scripts_dir().join(s.trim_start_matches("scripts/"))),
                    _ => continue,
                };
                changes.push(c);
            }
            // (What is reloaded gives its unsaved changes up: said so by the banner.)
            for c in &changes {
                if let Changed::Map(name) = c {
                    if let Some(d) = self.doc.iter_mut().chain(self.parked.values_mut()).find(|d| d.map.name == *name) {
                        d.mark_saved();
                    }
                }
            }
            self.defs_dirty = false;
            self.db.script_dirty = false;
            self.watch_apply(changes, true);
        } else {
            self.watch.stamps = stamps(&self.project.root);
        }
    }


}

#[cfg(test)]
mod tests {
    use crate::Workspace;

    /// A look at the files now (the watcher looks once a second).
    fn look(ws: &mut Workspace) {
        ws.watch.last_check = -10.0;
        ws.watch_project();
    }

    /// Another program writes `name` with another size. (Its file gets a time of its own: two
    /// writes within one tick of the file system's clock, of files of the same length, look
    /// alike to the watcher, and a test writes faster than any program a person waits for.)
    fn outside(ws: &Workspace, name: &str, w: u32, h: u32) {
        let mut m = ws.project.load_map(name).unwrap();
        m.resize(w, h);
        ws.project.save_map(&m).unwrap();
        let file = std::fs::File::options().write(true).open(ws.project.map_path(name)).unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60 * w as u64)).unwrap();
    }

    fn size(ws: &Workspace, name: &str) -> (u32, u32) {
        ws.open_maps().iter().find(|d| d.map.name == name).map(|d| (d.map.width, d.map.height)).unwrap()
    }

    #[test]
    fn maps_changed_by_another_program_are_read_again_or_asked_about() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-watcher-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.open_map("start");
        ws.create_map("cellar", 8, 6, None).unwrap();
        ws.history_frame(None);
        ws.watch_resnapshot();
        // The map in front, without changes of its own: read again as the file has it.
        outside(&ws, "cellar", 10, 7);
        look(&mut ws);
        assert_eq!((size(&ws, "cellar"), ws.watch.status().as_str()), ((10, 7), "clean"));
        assert!(ws.status.0.contains("the open map"), "{}", ws.status.0);
        // A map open beside it too.
        outside(&ws, "start", 12, 9);
        look(&mut ws);
        assert_eq!(size(&ws, "start"), (12, 9));
        assert_eq!(ws.doc.as_ref().unwrap().map.name, "cellar", "the map in front stays in front");
        // With changes of its own it is asked about: kept, the editor's stays (and is what a save writes)…
        ws.resize_map(11, 8).unwrap();
        outside(&ws, "cellar", 20, 20);
        look(&mut ws);
        assert_eq!((size(&ws, "cellar"), ws.watch.status().as_str()), ((11, 8), "changed:maps/cellar.map"));
        ws.watch_answer(false);
        assert_eq!((size(&ws, "cellar"), ws.watch.status().as_str()), ((11, 8), "clean"));
        assert!(ws.doc.as_ref().unwrap().is_dirty());
        // …reloaded, the file's is taken and nothing waits to be saved.
        outside(&ws, "cellar", 21, 21);
        look(&mut ws);
        assert_eq!(ws.watch.status(), "changed:maps/cellar.map");
        ws.watch_answer(true);
        assert_eq!(size(&ws, "cellar"), (21, 21));
        assert!(!ws.doc.as_ref().unwrap().is_dirty());
        // What the editor saves itself is no change from outside: the map keeps its undo steps.
        ws.resize_map(5, 5).unwrap();
        ws.history_frame(None);
        ws.save_all();
        look(&mut ws);
        assert_eq!((size(&ws, "cellar"), ws.watch.status().as_str()), ((5, 5), "clean"));
        assert!(ws.can_undo(), "saving doesn't cost the history");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The editor's own save, and a change made before the watcher looks again (an undo in the
    /// same frame): the files hold what the editor wrote, nothing was changed by another program.
    #[test]
    fn a_change_right_after_saving_is_no_change_on_disk() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-watcher-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.open_map("start");
        ws.history_frame(None);
        ws.watch_resnapshot();
        // Definitions (the settings' file) and the open map, saved and changed again at once.
        ws.project.config.name = "A game with a longer name".into();
        ws.resize_map(9, 9).unwrap();
        ws.history_frame(None);
        ws.save_all();
        ws.project.config.name = "Short".into();
        ws.resize_map(4, 4).unwrap();
        ws.history_frame(None);
        assert!(ws.is_dirty());
        look(&mut ws);
        assert_eq!(ws.watch.status(), "clean");
        assert_eq!((ws.project.config.name.as_str(), size(&ws, "start")), ("Short", (4, 4)), "nothing was read again");
        // Another program's change to the same file is still seen.
        let path = dir.join("project.ron");
        let text = std::fs::read_to_string(&path).unwrap().replace("A game with a longer name", "Theirs");
        std::fs::write(&path, text).unwrap();
        look(&mut ws);
        assert_eq!(ws.watch.status(), "changed:project.ron");
        let _ = std::fs::remove_dir_all(dir);
    }
}
