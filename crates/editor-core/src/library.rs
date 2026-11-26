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
