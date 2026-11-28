//! A project's plugins as the editor works with them: what each is (its `plugin.ron`), on or
//! off, its parameters' values, its place in the order; one added from the asset library or a
//! plugin file, one taken out (its folder kept aside first). The engine's `plugins.rs` is the
//! format and what the game does with them.

use std::path::{Path, PathBuf};

use rustpg2d_engine::plugins::{self, ParamValue, PluginDef};

use crate::workspace::Workspace;

/// A plugin of the project.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginRow {
    pub id: String,
    pub enabled: bool,
    /// Its `plugin.ron`; else why it can't be read (its folder is gone, the file is broken).
    pub def: Result<PluginDef, String>,
    /// It needs a newer script API than this editor's game has: the game leaves it out.
    pub too_new: bool,
    /// The parameters the project set (the others hold their defaults).
    pub values: std::collections::BTreeMap<String, ParamValue>,
}

/// A plugin the asset library has.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryPlugin {
    pub id: String,
    pub def: PluginDef,
    /// The version the project has of it (`None`: it isn't in the project).
    pub in_project: Option<String>,
}

impl LibraryPlugin {
    /// The library's is newer than the project's.
    pub fn newer(&self) -> bool {
        self.in_project.as_ref().is_some_and(|v| plugins::compare_versions(&self.def.version, v) == std::cmp::Ordering::Greater)
    }
}

impl Workspace {
    /// The project's plugins in their order (the order their hooks run in).
    pub fn plugins(&self) -> Vec<PluginRow> {
        self.project
            .config
            .plugins
            .iter()
            .map(|r| {
                let def = plugins::read_def(&self.project.root, &r.id).map_err(|e| format!("{e:#}"));
                let too_new = def.as_ref().is_ok_and(|d| !plugins::api_fits(&d.api));
                PluginRow { id: r.id.clone(), enabled: r.enabled, def, too_new, values: r.parameters.clone() }
            })
            .collect()
    }

    /// The plugins the asset library has, with what the project has of each.
    pub fn library_plugins(&mut self) -> Vec<LibraryPlugin> {
        self.lib.ensure_loaded();
        let Some(root) = self.lib.lib.as_ref().map(|l| l.root.clone()) else { return Vec::new() };
        plugins::installed(&root)
            .into_iter()
            .filter_map(|id| {
                let def = plugins::read_def(&root, &id).ok()?;
                let in_project = self.project.config.plugins.iter().any(|r| r.id == id).then(|| plugins::read_def(&self.project.root, &id).map(|d| d.version).unwrap_or_default());
                Some(LibraryPlugin { id, def, in_project })
            })
            .collect()
    }

    /// What a plugin's arrival or leaving changes around it: its scripts are the game's now
    /// (or no longer), and the settings are written (its files are in the project already).
    fn plugins_changed(&mut self) {
        self.defs_dirty = true;
        self.save_defs();
        self.refresh_refs();
        self.problems.stale = true;
        self.scripts_rev += 1;
    }

    fn plugin_installed(&mut self, done: plugins::Installed) -> String {
        self.plugins_changed();
        match done.replaced {
            Some(old) if old != done.version => format!("{} {} replaces {old}; its place, on or off and its values stay", done.name, done.version),
            Some(_) => format!("{} {} is in the project again as it came", done.name, done.version),
            None => format!("{} {} is in the project, switched on", done.name, done.version),
        }
    }

    /// Adds the library's plugin `id` to the project (a newer version replaces the one there).
    pub fn add_plugin_from_library(&mut self, id: &str) -> Result<String, String> {
        self.lib.ensure_loaded();
        let root = self.lib.lib.as_ref().map(|l| l.root.clone()).ok_or("there is no asset library")?;
        if !plugins::installed(&root).iter().any(|p| p == id) {
            return Err(format!("the asset library has no plugin \"{id}\""));
        }
        self.keep_plugin_aside(id)?;
        self.begin(&format!("Add the plugin {id}"));
        let done = plugins::install_from(&mut self.project, &root, id).map_err(|e| format!("{e:#}"))?;
        Ok(self.plugin_installed(done))
    }

    /// Adds the plugin of a plugin file (`.r2dplugin`).
    pub fn add_plugin_file(&mut self, file: &Path) -> Result<String, String> {
        let bytes = std::fs::read(file).map_err(|e| format!("reading {}: {e}", file.display()))?;
        let (id, def, files) = plugins::unpack(&bytes).map_err(|e| format!("{}: {e:#}", file.display()))?;
        if !plugins::valid_id(&id) {
            return Err(format!("\"{id}\" is no plugin's id"));
        }
        self.keep_plugin_aside(&id)?;
        self.begin(&format!("Add the plugin {id}"));
        let done = plugins::install(&mut self.project, &id, &def, &files).map_err(|e| format!("{e:#}"))?;
        Ok(self.plugin_installed(done))
    }

    /// A plugin's folder copied under `editor/backup/<time>/plugins/` before it is replaced or
    /// removed (an undo brings its entry back, not its files). Returns where, if it was there.
    fn keep_plugin_aside(&self, id: &str) -> Result<Option<PathBuf>, String> {
        let dir = plugins::plugin_dir(&self.project.root, id);
        if !dir.is_dir() {
            return Ok(None);
        }
        let (y, mo, d, h, mi, s) = rustpg2d_engine::starter_update::utc_now();
        let to = self.project.root.join("editor").join("backup").join(format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")).join("plugins").join(id);
        for (rel, bytes) in rustpg2d_engine::pack::read_dir_files(&dir).map_err(|e| format!("{e:#}"))? {
            let p = to.join(&rel);
            std::fs::create_dir_all(p.parent().unwrap_or(&to)).map_err(|e| e.to_string())?;
            std::fs::write(&p, bytes).map_err(|e| format!("keeping {rel}: {e}"))?;
        }
        Ok(Some(to))
    }

    /// Takes a plugin out of the project: its entry and its folder (kept aside first).
    pub fn remove_plugin(&mut self, id: &str) -> Result<String, String> {
        if !self.project.config.plugins.iter().any(|r| r.id == id) {
            return Err(format!("the project has no plugin \"{id}\""));
        }
        let kept = self.keep_plugin_aside(id)?;
        self.begin(&format!("Remove the plugin {id}"));
        plugins::remove(&mut self.project, id).map_err(|e| format!("{e:#}"))?;
        self.plugins_changed();
        Ok(match kept {
            Some(to) => format!("{id} is out of the project; its files are kept in {}", to.strip_prefix(&self.project.root).unwrap_or(&to).display()),
            None => format!("{id} is out of the project"),
        })
    }

    fn plugin_ref(&mut self, id: &str) -> Result<&mut plugins::PluginRef, String> {
        self.project.config.plugins.iter_mut().find(|r| r.id == id).ok_or_else(|| format!("the project has no plugin \"{id}\""))
    }

    /// Switches a plugin on or off (off: the game leaves it out; its values stay).
    pub fn set_plugin_enabled(&mut self, id: &str, on: bool) -> Result<(), String> {
        if self.plugin_ref(id)?.enabled != on {
            self.begin(&format!("Switch the plugin {id} {}", if on { "on" } else { "off" }));
            self.plugin_ref(id)?.enabled = on;
            self.defs_dirty = true;
        }
        Ok(())
    }

    /// Sets a parameter of a plugin (`None`: back to the plugin's default).
    pub fn set_plugin_param(&mut self, id: &str, name: &str, value: Option<ParamValue>) -> Result<(), String> {
        let def = plugins::read_def(&self.project.root, id).map_err(|e| format!("{e:#}"))?;
        let param = def.parameters.iter().find(|p| p.name == name).ok_or_else(|| format!("{} has no parameter \"{name}\" (it has: {})", def.name, def.parameters.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")))?;
        // (A value that is the default isn't kept: the plugin's next version may change it.)
        let value = value.filter(|v| *v != param.default_value());
        if let Some(v) = &value {
            if param.value(Some(v)) != *v {
                return Err(format!("{} takes a {}", param.shown_label(), param.kind.label()));
            }
        }
        if self.plugin_ref(id)?.parameters.get(name) == value.as_ref() {
            return Ok(());
        }
        self.begin_unnamed(&format!("Change {} of the plugin {id}", param.shown_label()));
        let r = self.plugin_ref(id)?;
        match value {
            Some(v) => {
                r.parameters.insert(name.to_string(), v);
            }
            None => {
                r.parameters.remove(name);
            }
        }
        self.defs_dirty = true;
        Ok(())
    }

    /// Moves a plugin one place up or down in the order (hooks of the same priority run in it).
    pub fn move_plugin(&mut self, id: &str, up: bool) -> Result<(), String> {
        let list = &self.project.config.plugins;
        let at = list.iter().position(|r| r.id == id).ok_or_else(|| format!("the project has no plugin \"{id}\""))?;
        let to = if up { at.checked_sub(1) } else { (at + 1 < list.len()).then_some(at + 1) };
        if let Some(to) = to {
            self.begin(&format!("Move the plugin {id}"));
            self.project.config.plugins.swap(at, to);
            self.defs_dirty = true;
        }
        Ok(())
    }

    /// `[calendar:on:1.0 minimap:off:1.2]` for scripts.
    pub fn plugins_status(&self) -> String {
        let rows: Vec<String> = self.plugins().iter().map(|p| format!("{}:{}:{}", p.id, if p.enabled { "on" } else { "off" }, p.def.as_ref().map_or("?", |d| d.version.as_str()))).collect();
        format!("[{}]", rows.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_is_added_set_up_moved_and_taken_out() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rustpg2d-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        ws.lib.lib = Some(rustpg2d_engine::project::Project::load(&root.join("crates/editor/assets/starter_library")).unwrap());
        assert!(ws.plugins().is_empty());
        // The library's plugins are offered; one it doesn't have is refused.
        let offered = ws.library_plugins();
        assert!(offered.iter().any(|p| p.id == "calendar" && p.in_project.is_none() && !p.def.parameters.is_empty()), "{:?}", offered.iter().map(|p| &p.id).collect::<Vec<_>>());
        assert!(ws.add_plugin_from_library("no_such_plugin").is_err());
        // Added: its files are the project's, it is on, and the settings are written.
        let said = ws.add_plugin_from_library("calendar").unwrap();
        assert!(said.starts_with("Calendar ") && said.ends_with("switched on"), "{said}");
        ws.add_plugin_from_library("minimap").unwrap();
        ws.history_frame(None);
        assert!(dir.join("plugins/calendar/plugin.ron").is_file());
        assert_eq!(ws.plugins_status().split(' ').count(), 2);
        assert!(rustpg2d_engine::project::Project::load(&dir).unwrap().config.plugins.iter().any(|r| r.id == "calendar" && r.enabled));
        assert!(ws.library_plugins().iter().any(|p| p.id == "calendar" && p.in_project.is_some() && !p.newer()));
        // A parameter: checked against its kind; its default isn't kept as a value.
        ws.set_plugin_param("calendar", "days_per_season", Some(ParamValue::Number(30.0))).unwrap();
        assert!(ws.set_plugin_param("calendar", "days_per_season", Some(ParamValue::Text("many".into()))).unwrap_err().contains("number"));
        assert!(ws.set_plugin_param("calendar", "moons", Some(ParamValue::Number(2.0))).unwrap_err().contains("no parameter"));
        ws.set_plugin_param("calendar", "show_year", Some(ParamValue::Flag(false))).unwrap();
        ws.history_frame(None);
        let row = ws.plugins().into_iter().find(|p| p.id == "calendar").unwrap();
        assert_eq!(row.values.get("days_per_season"), Some(&ParamValue::Number(30.0)));
        assert!(!row.values.contains_key("show_year"), "false is its default");
        // Off, and moved behind the other one: steps, undone as steps.
        ws.set_plugin_enabled("calendar", false).unwrap();
        ws.history_frame(None);
        ws.move_plugin("calendar", false).unwrap();
        ws.history_frame(None);
        assert!(ws.plugins_status().starts_with("[minimap:on:") && ws.plugins_status().contains("calendar:off:"), "{}", ws.plugins_status());
        ws.undo().unwrap();
        ws.undo().unwrap();
        assert!(ws.plugins_status().starts_with("[calendar:on:"), "{}", ws.plugins_status());
        // Added again: its values stay. Taken out: its folder is kept aside.
        ws.add_plugin_from_library("calendar").unwrap();
        assert_eq!(ws.plugins().iter().find(|p| p.id == "calendar").unwrap().values.get("days_per_season"), Some(&ParamValue::Number(30.0)));
        let said = ws.remove_plugin("calendar").unwrap();
        assert!(said.contains("editor/backup/"), "{said}");
        assert!(!dir.join("plugins/calendar").exists() && ws.plugins().iter().all(|p| p.id != "calendar"));
        let kept = std::fs::read_dir(dir.join("editor/backup")).unwrap().flatten().any(|e| e.path().join("plugins/calendar/plugin.ron").is_file());
        assert!(kept, "its files are kept under editor/backup");
        assert!(ws.remove_plugin("calendar").is_err());
        // A plugin file brings one in too.
        let file = dir.join("hard_mode.r2dplugin");
        std::fs::write(&file, plugins::pack(&root.join("crates/editor/assets/starter_library"), "hard_mode").unwrap()).unwrap();
        let said = ws.add_plugin_file(&file).unwrap();
        assert!(said.ends_with("switched on"), "{said}");
        assert!(ws.plugins().iter().any(|p| p.id == "hard_mode" && p.def.is_ok()));
        assert!(ws.add_plugin_file(&dir.join("nothing.r2dplugin")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
