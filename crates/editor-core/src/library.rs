//! The asset library as the workspace uses it: opened when first needed, assets copied from it
//! into the project and from the project into it.

use anyhow::{Context, Result};
use rustpg2d_engine::library::{self, AssetRef, Bundle};
use rustpg2d_engine::project::Project;

use crate::workspace::Workspace;

/// The asset library, opened on first use.
#[derive(Default)]
pub struct LibState {
    pub lib: Option<Project>,
    pub error: Option<String>,
    /// The library changed: whoever shows its pictures loads them again.
    pub tex_dirty: bool,
}

impl LibState {
    pub fn ensure_loaded(&mut self) {
        if self.lib.is_some() || self.error.is_some() {
            return;
        }
        let dir = library::library_dir();
        match crate::starter::open_library(&dir) {
            Ok(p) => {
                self.lib = Some(p);
                self.tex_dirty = true;
            }
            Err(e) => self.error = Some(format!("Cannot open the asset library at {}: {e:#}", library::library_dir().display())),
        }
    }
}

/// What a new game starts with, from the asset library (those it has): ground materials,
/// scatter sets, walls, the hero and the heroine's figure, a few items and props.
pub const BASICS: [(rustpg2d_engine::refactor::RefKind, &str); 25] = {
    use rustpg2d_engine::refactor::RefKind::*;
    [
        (Material, "grass"),
        (Material, "meadow"),
        (Material, "dirt"),
        (Material, "sand"),
        (Material, "water"),
        (Material, "cobblestone"),
        (Material, "flagstones"),
        (Material, "wood_floor"),
        (Scatter, "forest"),
        (Scatter, "rocks"),
        (Prop, "stone_wall"),
        (Prop, "wood_fence"),
        (Character, "hero"),
        (Model, "hero"),
        (Model, "heroine"),
        (Item, "coin"),
        (Item, "apple"),
        (Item, "potion"),
        (Item, "sword"),
        (Item, "key"),
        (Prop, "chest"),
        (Prop, "sign"),
        (Prop, "barrel"),
        (Prop, "well"),
        (Prop, "boulder"),
    ]
};

impl Workspace {
    /// Adds library assets (with their dependencies) to the project.
    pub fn library_to_project(&mut self, assets: &[AssetRef]) -> Result<String> {
        self.lib.ensure_loaded();
        let lib = self.lib.lib.as_ref().context("library not available")?;
        let mut b = Bundle::collect_with_needed_scripts(&lib.db, &lib.root, rustpg2d_engine::project::TILE_SIZE, "library", assets)?;
        b.rescale(rustpg2d_engine::project::TILE_SIZE)?;
        let root = self.project.root.clone();
        let report = b.merge_into(&mut self.project.db, &root)?;
        self.defs_dirty = true;
        self.after_scripts_merged();
        self.reload_assets();
        Ok(format!("Added to the project: {}. Save the definitions to keep them.", report.summary()))
    }

    /// Gives a project that was just made what a game starts with, from the asset library
    /// (what of [`BASICS`] the library has): ground to paint with, a hero who is the player's
    /// character, a few props and items; its first map gets grass. `name`: what the game is
    /// called. Everything is saved. Returns what was done, in words.
    pub fn start_with_basics(&mut self, name: &str) -> Result<String, String> {
        if !name.trim().is_empty() {
            self.project.config.name = name.trim().to_string();
            self.defs_dirty = true;
        }
        self.lib.ensure_loaded();
        let mut said = Vec::new();
        let want: Vec<AssetRef> = match &self.lib.lib {
            Some(lib) => BASICS.iter().map(|(k, id)| (*k, id.to_string())).filter(|a| rustpg2d_engine::library::exists(&lib.db, a)).collect(),
            None => Vec::new(),
        };
        if want.is_empty() {
            said.push("the asset library has none of the basics: the project starts empty".to_string());
        } else {
            self.library_to_project(&want).map_err(|e| format!("{e:#}"))?;
            said.push(format!("{} things from the asset library", want.len()));
            let db = &self.project.db;
            let has = |id: &str| db.model(id).is_some();
            let (male, female) = (if has("hero") { "hero" } else { "" }, if has("heroine") { "heroine" } else { "" });
            let hero = db.character("hero").is_some();
            let c = &mut self.project.config;
            c.srd = !rustpg2d_engine::srd::rules::srd_content(db).is_empty();
            if hero {
                c.player = "hero".into();
                said.push("the hero is the player's character".into());
            }
            c.creation.body_male = male.into();
            c.creation.body_female = female.into();
            // Its first map: grass, when that came along and nothing is painted yet.
            let start = self.project.config.start_map.clone();
            if self.project.db.material("grass").is_some() && self.maps.contains(&start) {
                self.open_map(&start);
                if let Some(doc) = self.doc.as_mut().filter(|d| d.map.name == start && d.map.materials.is_empty()) {
                    if let Some(v) = doc.map.material_value("grass") {
                        doc.map.ground.fill(v);
                        doc.mark_changed();
                        said.push(format!("grass on the map {start}"));
                    }
                }
            }
        }
        self.defs_dirty = true;
        self.save_all();
        Ok(said.join(", "))
    }

    /// A new definition that starts as an asset of the library: the asset comes into the project
    /// with what it needs (pictures, sounds, models, the script functions its events call) and
    /// the new one is it under the name given: its id is made from the name. With no name the
    /// asset is added as it is. A library asset the project has already stays, and the new one
    /// is a copy of the project's. One step of the history. Returns the new one's id.
    pub fn make_from_library(&mut self, kind: rustpg2d_engine::refactor::RefKind, lib_id: &str, name: &str) -> Result<String, String> {
        let name = name.trim();
        self.lib.ensure_loaded();
        if !self.lib.lib.as_ref().is_some_and(|l| l.db.has(kind, lib_id)) {
            return Err(match &self.lib.error {
                Some(e) => format!("the asset library can't be read: {e}"),
                None => format!("the asset library has no {} \"{lib_id}\"", kind.label().to_lowercase()),
            });
        }
        let had = self.project.db.has(kind, lib_id);
        if had && name.is_empty() {
            return Err(format!("the project has \"{lib_id}\" already: give the new one a name to make a copy of it"));
        }
        self.library_to_project(&[(kind, lib_id.to_string())]).map_err(|e| format!("{e:#}"))?;
        let id = if name.is_empty() {
            lib_id.to_string()
        } else if !had && crate::defs::id_from_name(name) == lib_id {
            // (Called what the library's own is filed under: that one, under the name typed.)
            if let Some(mut value) = self.def_value(kind, lib_id) {
                if let Some(field) = ["name", "title"].into_iter().find(|f| value.get(*f).is_some_and(|v| v.is_string())) {
                    value[field] = serde_json::Value::String(name.to_string());
                    self.set_def_value(kind, lib_id, value)?;
                }
            }
            lib_id.to_string()
        } else {
            let id = self.make_def(kind, name, Some(lib_id))?;
            if !had {
                // (Only the new one was asked for: what it was made from goes again, what it needs stays.)
                self.delete_def(kind, lib_id, true)?;
            }
            id
        };
        self.begin(&format!("New {} from the library: {}", kind.label().to_lowercase(), if name.is_empty() { lib_id } else { name }));
        self.refresh_refs();
        self.stale = true;
        Ok(id)
    }

    /// Sends project assets (with their dependencies and the script functions they need) to the
    /// asset library without asking: the AI agent server's `library_send`. `replace` overwrites
    /// the library's version of assets it already has (updating a library from the project its
    /// assets come from, e.g. the starter library from the example project) instead of adding
    /// them under new ids; `drop_script_triggers` leaves triggers that run the project's
    /// script functions behind.
    pub fn project_to_library(&mut self, assets: &[AssetRef], replace: bool, drop_script_triggers: bool) -> Result<String> {
        let b = Bundle::collect_for_library(&self.project.db, &self.project.root, rustpg2d_engine::project::TILE_SIZE, &self.project.config.name, assets, drop_script_triggers)?;
        self.lib.ensure_loaded();
        let lib = self.lib.lib.as_mut().with_context(|| self.lib.error.clone().unwrap_or_else(|| "library not available".into()))?;
        let report = if replace {
            let root = lib.root.clone();
            let r = b.replace_into(&mut lib.db, &root)?;
            lib.save_defs()?;
            r
        } else {
            library::add_to_library(lib, b)?
        };
        self.lib.tex_dirty = true;
        Ok(format!("Library ({}): {}", lib.root.display(), report.summary()))
    }

    /// A merge may have appended functions to the project's scripts/library.rhai: an unchanged
    /// open copy of it in the script editor is reloaded.
    pub(crate) fn after_scripts_merged(&mut self) {
        let path = self.project.scripts_dir().join(library::LIBRARY_SCRIPT);
        if self.db.script_file.as_deref() == Some(path.as_path()) && !self.db.script_dirty {
            self.db.script_text = std::fs::read_to_string(&path).unwrap_or_default();
            self.db.script_saved = self.db.script_text.clone();
        }
    }
}
