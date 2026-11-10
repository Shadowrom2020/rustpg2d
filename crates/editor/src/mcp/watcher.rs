//! The project file watcher: agents (and other programs) may change `project.ron`, `defs/*.ron`,
//! `maps/*.map`, `scripts/*.rhai` and `lang/*.ron` on disk while the editor is open. Once a
//! second the files' modification times and sizes are compared with the last snapshot; clean
//! documents reload at once, unsaved ones make a banner ask (Reload / Keep mine). The editor's
//! own saves re-snapshot, so it never reacts to itself.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::app::Editor;

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

impl Editor {
    /// The files on disk now match the editor (after the project opened or the editor saved).
    pub fn watch_resnapshot(&mut self) {
        if !self.project_open || self.library_mode {
            return;
        }
        self.watch.stamps = stamps(&self.project.root);
        self.watch.changed.clear();
        self.watch.ready = true;
    }

    /// Called every frame (with a project open): checks once a second.
    pub fn watch_project(&mut self) {
        if !self.project_open || self.library_mode {
            return;
        }
        let now = macroquad::time::get_time();
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
                                self.db_hist.clear();
                                self.reload_assets();
                                reloaded.push("definitions");
                            }
                        }
                        Err(e) => self.set_status(format!("Definitions changed on disk but can't be read: {e:#}")),
                    }
                }
                Changed::Map(name) if Some(&name) == open_map.as_ref() => {
                    if self.doc.as_ref().is_some_and(|d| d.is_dirty()) && !force {
                        blocked.push(format!("maps/{name}.map"));
                        continue;
                    }
                    if self.project.map_path(&name).exists() {
                        let (sel, layer) = (self.sel, self.layer);
                        self.open_map(&name);
                        self.sel = sel;
                        self.layer = layer.min(self.doc.as_ref().map_or(0, |d| d.map.layers.len().saturating_sub(1)));
                        reloaded.push("the open map");
                    } else {
                        self.doc = None;
                        map_list = true;
                        reloaded.push("the open map (deleted on disk: closed)");
                    }
                }
                Changed::Map(_) => map_list = true,
                Changed::Script(p) => {
                    if self.db.script_file.as_ref() == Some(&p) {
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
            if let Some(d) = &mut self.doc {
                d.mark_saved();
            }
            self.defs_dirty = false;
            self.db.script_dirty = false;
            self.watch_apply(changes, true);
        } else {
            self.watch.stamps = stamps(&self.project.root);
        }
    }

    /// The yellow banner under the menu while files changed on disk conflict with unsaved edits.
    pub fn watch_banner(&mut self, ctx: &egui_macroquad::egui::Context) {
        if self.watch.changed.is_empty() {
            return;
        }
        use egui_macroquad::egui;
        let mut answer = None;
        egui::TopBottomPanel::top("watch_banner").frame(egui::Frame::default().fill(egui::Color32::from_rgb(70, 60, 20)).inner_margin(6.0)).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(egui::Color32::YELLOW, format!("Changed on disk (by an agent or another program): {} — you have unsaved edits to the same.", self.watch.changed.join(", ")));
                if ui.button("Reload (discard my edits)").clicked() {
                    answer = Some(true);
                }
                if ui.button("Keep mine").on_hover_text("Keeps the editor's state; saving later overwrites the files").clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(reload) = answer {
            self.watch_answer(reload);
        }
    }
}
