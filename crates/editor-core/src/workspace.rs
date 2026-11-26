//! The workspace: an open project with its definitions, its open maps and what was changed. The
//! window, the editor script and the agent server all work on one of these.
//!
//! A workspace can live without a window and without a graphics adapter: textures (and with
//! them thumbnails and map pictures) are loaded only when it was opened with [`Gpu::Yes`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use rustpg2d_engine::map::Map;
use rustpg2d_engine::project::{Project, ProjectConfig};
use rustpg2d_engine::render::Assets;

use crate::doc::MapDoc;
use crate::files;
use crate::history::DbHistory;
use crate::project_file::ProjectFile;

/// What is selected on the map being worked on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Selection {
    #[default]
    None,
    Entity(u32),
    Zone(u32),
    /// A rectangle of tiles `(x, y, w, h)` (copy / cut / delete).
    Region((u32, u32, u32, u32)),
}

/// What a map view shows besides the map.
#[derive(Clone, Copy, Debug)]
pub struct ViewOpts {
    pub grid: bool,
    pub collision: bool,
    pub zones: bool,
    pub minimap: bool,
    /// The map's darkness and lights at `hour` (outdoor maps).
    pub lighting: bool,
    pub hour: f32,
    pub weather: bool,
    pub parallax: bool,
    /// From straight above instead of isometric.
    pub topdown: bool,
}

impl Default for ViewOpts {
    fn default() -> ViewOpts {
        ViewOpts { grid: false, collision: false, zones: true, minimap: true, lighting: false, hour: 12.0, weather: false, parallax: true, topdown: false }
    }
}

/// Where a map view looks: the world point in its middle and screen pixels per world pixel.
#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub center: gfx::math::Vec2,
    pub zoom: f32,
}

/// A script as an editor shows it: the file and its text with the edits not written yet. Agents
/// and renames that rewrite the file keep it in step.
#[derive(Default)]
pub struct ScriptBuffer {
    pub script_file: Option<PathBuf>,
    pub script_text: String,
    pub script_saved: String,
    pub script_dirty: bool,
}

/// May textures be loaded (is `gfx` set up on this thread)?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gpu {
    Yes,
    /// No pictures: sprite previews stay empty, map overviews aren't built.
    No,
}

pub struct Workspace {
    pub project: Project,
    /// False without a project (the home screen): `project` is then an empty placeholder.
    pub project_open: bool,
    /// The project file this workspace's folder is the working copy of.
    pub project_file: Option<ProjectFile>,
    /// The project's maps by name, sorted.
    pub maps: Vec<String>,
    /// The map being worked on.
    pub doc: Option<MapDoc>,
    /// The other open maps, with their undo histories.
    pub parked: BTreeMap<String, MapDoc>,
    pub assets: Assets,
    pub gpu: Gpu,
    /// The definitions or project settings differ from the files.
    pub defs_dirty: bool,
    /// Definitions and project settings as last loaded / saved: `defs_dirty` is cleared again
    /// when the edits are reverted.
    pub defs_saved: Box<(rustpg2d_engine::defs::Database, ProjectConfig)>,
    /// Undo / redo of definition and project-setting changes.
    pub db_hist: DbHistory,
    /// What a load left out (unreadable definition entries or map objects), until dismissed.
    pub notices: Vec<String>,
    /// What loading the definitions left out, for the problems list.
    pub defs_warnings: Vec<String>,
    /// The last message for the person, and when (seconds of [`crate::now`]).
    pub status: (String, f64),
    /// The selection, camera and view options of the map being worked on.
    pub sel: Selection,
    pub cam: Cam,
    pub view: ViewOpts,
    /// The script open in an editor (named `db` as the 0.35 editor's Database state was, so the
    /// code that came over reads the same).
    pub db: ScriptBuffer,
    /// What the person is looking at and whether a question is open, as the window tells it
    /// (for agents asking about the editor's state).
    pub looking_at: String,
    pub asking: Option<String>,
    /// This workspace is the asset library itself.
    pub library_mode: bool,
    pub refs: crate::refs::Refs,
    pub map_refs: std::collections::HashMap<String, crate::refs::MapRefs>,
    pub problems: crate::problems::Problems,
    pub search: crate::search::Search,
    pub usages: crate::search::Usages,
    /// Generate art (Meshy): the jobs and the account's settings.
    pub art: crate::art::ArtState,
    pub art_settings: crate::art::ArtSettings,
    /// The asset library.
    pub lib: crate::library::LibState,
    /// Every undoable change in the order it was made.
    pub timeline: crate::timeline::Timeline,
    /// The agent server and what it saw change on disk.
    pub mcp: crate::mcp::McpState,
    pub watch: crate::mcp::watcher::Watch,
    /// The search index is older than the project.
    pub search_stale: bool,
    /// Counts the times script files were rewritten on disk by a rename: a script editor showing
    /// one reloads it when this moved.
    pub scripts_rev: u64,
    /// Something changed that the problems list, the search index and the pickers' lists
    /// depend on: whoever keeps those looks here and clears it.
    pub stale: bool,
}

/// Empty assets: no textures.
fn no_assets() -> Assets {
    Assets { tile_size: rustpg2d_engine::project::TILE_SIZE as f32, sheets: Default::default(), materials: Default::default(), material_frames: Default::default(), material_colors: Default::default(), generation: 0, errors: Vec::new(), models: Default::default() }
}

impl Workspace {
    /// No project open.
    pub fn empty() -> Workspace {
        let project = Project { root: PathBuf::new(), config: ProjectConfig::new(""), db: Default::default() };
        Workspace::with_project(project, false, Gpu::No)
    }

    fn with_project(project: Project, project_open: bool, gpu: Gpu) -> Workspace {
        let assets = if gpu == Gpu::Yes && project_open { Assets::load(&project) } else { no_assets() };
        let maps = if project_open { project.list_maps() } else { Vec::new() };
        let defs_saved = Box::new((project.db.clone(), project.config.clone()));
        Workspace { project, project_open, project_file: None, maps, doc: None, parked: BTreeMap::new(), assets, gpu, defs_dirty: false, defs_saved, db_hist: DbHistory::default(), notices: Vec::new(), defs_warnings: Vec::new(), status: (String::new(), 0.0), sel: Selection::None, cam: Cam { center: gfx::math::Vec2::ZERO, zoom: 1.0 }, view: ViewOpts::default(), db: ScriptBuffer::default(), looking_at: String::new(), asking: None, library_mode: false, refs: Default::default(), map_refs: Default::default(), problems: crate::problems::Problems::new(), search: Default::default(), usages: Default::default(), art: Default::default(), art_settings: { let s = files::load_settings(); crate::art::ArtSettings { model: s.meshy_model, budget: s.meshy_budget, ask: s.meshy_ask } }, lib: Default::default(), timeline: Default::default(), mcp: Default::default(), watch: Default::default(), search_stale: true, scripts_rev: 0, stale: true }
    }

    /// A workspace on the project folder `root` (`file`: the project file it is the working copy
    /// of). Projects of versions before 0.35 are refused: 0.35 converts them.
    pub fn new(root: PathBuf, gpu: Gpu) -> Result<Workspace> {
        if rustpg2d_engine::migrate::stored_tile_size(&root)? != rustpg2d_engine::project::TILE_SIZE || rustpg2d_engine::convert_tiles::needs_conversion(&root) || rustpg2d_engine::srd_convert::needs_conversion(&root) {
            bail!("{} was made with a version before 0.35: open and save it with R(ust)PG2D 0.35 first", root.display());
        }
        let (project, warnings) = Project::load_with_warnings(&root)?;
        let mut ws = Workspace::with_project(project, true, gpu);
        ws.note_defs_warnings(warnings);
        let first = if ws.maps.contains(&ws.project.config.start_map) { Some(ws.project.config.start_map.clone()) } else { ws.maps.first().cloned() };
        if let Some(m) = first {
            ws.open_map(&m);
        }
        if !ws.assets.errors.is_empty() {
            ws.set_status(format!("{} asset error(s), see terminal", ws.assets.errors.len()));
        }
        Ok(ws)
    }

    /// A workspace on the project file or folder at `path`. A folder is converted into a project
    /// file beside it first (the folder is left as it was), unless folders are allowed
    /// (`RUSTPG2D_FOLDER_PROJECTS`) or it is the repository's example.
    pub fn open_path(path: PathBuf, gpu: Gpu) -> Result<Workspace> {
        use crate::project_file::folders_allowed;
        if path.is_dir() {
            if !path.join("project.ron").is_file() {
                bail!("{} is not an R(ust)PG2D project (no project.ron)", path.display());
            }
            if folders_allowed() || crate::repo_mode::is_example(&path) {
                let ws = Workspace::new(path.clone(), gpu)?;
                files::remember_recent(&path);
                return Ok(ws);
            }
            let (file, made) = ProjectFile::convert_folder(&path)?;
            let mut ws = Workspace::open_path(file.clone(), gpu)?;
            files::forget_recent(&path);
            ws.set_status(if made {
                format!("Converted the project folder {} into the project file {} (the folder is left as it was)", path.display(), file.display())
            } else {
                format!("{} was converted before: opened its project file {}", path.display(), file.display())
            });
            return Ok(ws);
        }
        let mut pf = ProjectFile::open(&path)?;
        let mut ws = Workspace::new(pf.work.clone(), gpu)?;
        files::remember_recent(&pf.path);
        if let Some(n) = pf.note.take() {
            ws.set_status(n);
        }
        ws.project_file = Some(pf);
        Ok(ws)
    }

    /// Where the project is for the person: its project file, else its folder.
    pub fn project_location(&self) -> PathBuf {
        self.project_file.as_ref().map_or_else(|| self.project.root.clone(), |pf| pf.path.clone())
    }

    /// The folder holding the project file (or the project folder).
    pub fn project_parent(&self) -> Option<PathBuf> {
        let loc = crate::project_file::canonical(&self.project_location());
        loc.parent().map(Path::to_path_buf)
    }

    /// Writes the project file now if its working copy changed: before the project is closed or
    /// the program quits.
    pub fn finish_project_file(&mut self) {
        let Some(pf) = &mut self.project_file else { return };
        let res = pf.sync_now();
        let note = pf.note.take();
        if note.is_some() {
            // The work went to a separate copy: list it with the recent projects.
            files::remember_recent(&pf.path);
        }
        if let Err(e) = res {
            self.set_status(format!("Writing the project file failed: {e:#}"));
        } else if let Some(n) = note {
            self.set_status(n);
        }
    }

    /// Now and then: writes the project file (on a thread) when the working copy changed.
    pub fn project_file_tick(&mut self) {
        let Some(pf) = &mut self.project_file else { return };
        let was_error = pf.error.is_some();
        pf.tick(crate::now());
        if let Some(n) = pf.note.take() {
            files::remember_recent(&pf.path);
            self.set_status(n);
        } else if let Some(e) = pf.error.clone().filter(|_| !was_error) {
            self.set_status(e);
        }
    }

    /// Back to no project (the project file is written first).
    pub fn close_project(&mut self) {
        self.finish_project_file();
        *self = Workspace::empty();
    }

    /// Pixels of a tile.
    pub fn ts(&self) -> f32 {
        self.assets.tile_size
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        let s = s.into();
        println!("[editor] {s}");
        self.status = (s, crate::now());
    }

    /// Notes what loading `file` left out (its objects or entries that can't be read), after
    /// backing the file up: saving it again would drop them for good.
    pub fn note_load_warnings(&mut self, file: &Path, warnings: &[String]) {
        if warnings.is_empty() {
            return;
        }
        let rel = file.strip_prefix(&self.project.root).unwrap_or(file).display().to_string();
        // (Definition warnings already start with their file.)
        let details: Vec<&str> = warnings.iter().map(|w| w.strip_prefix(rel.as_str()).map_or(w.as_str(), str::trim_start)).collect();
        for d in &details {
            eprintln!("[editor] {rel}: {d}");
        }
        let backup = files::back_up_once(file).map(|b| format!(" (backup: {})", b.file_name().unwrap_or_default().to_string_lossy())).unwrap_or_default();
        let n = warnings.len();
        self.notices.push(format!("{rel}: {n} entr{} can't be read and will be lost when the file is saved again{backup}. {}", if n == 1 { "y" } else { "ies" }, details.join(" ")));
    }

    /// [`Self::note_load_warnings`] for the definitions: each warning starts with its file
    /// (`defs/items.ron entry 7 …`).
    pub fn note_defs_warnings(&mut self, warnings: Vec<String>) {
        self.defs_warnings = warnings.clone();
        let mut by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for w in warnings {
            let file = w.split_whitespace().next().unwrap_or("defs").to_string();
            by_file.entry(file).or_default().push(w);
        }
        for (file, ws) in by_file {
            let path = self.project.root.join(&file);
            self.note_load_warnings(&path, &ws);
        }
    }

    // ------------------------------------------------------------ maps

    /// Makes `name` the map being worked on. The map that was is kept open with its history.
    pub fn open_map(&mut self, name: &str) {
        if self.doc.as_ref().is_some_and(|d| d.map.name == name) {
            return;
        }
        if let Some(d) = self.doc.take() {
            self.parked.insert(d.map.name.clone(), d);
        }
        if let Some(d) = self.parked.remove(name) {
            self.doc = Some(d);
            return;
        }
        let mut warnings = Vec::new();
        match rustpg2d_engine::map::io::load_with(&self.project.map_path(name), &mut |w| warnings.push(w)) {
            Ok(map) => {
                self.note_load_warnings(&self.project.map_path(name), &warnings);
                self.load_doc(map);
                self.set_status(format!("Opened {name}"));
            }
            Err(e) => self.set_status(format!("Failed to open {name}: {e:#}")),
        }
    }

    /// Makes `map` the map being worked on (a new or loaded one).
    pub fn load_doc(&mut self, map: Map) {
        if let Some(d) = self.doc.take() {
            self.parked.insert(d.map.name.clone(), d);
        }
        self.doc = Some(MapDoc::new(map, &self.assets));
        self.stale = true;
    }

    /// Closes an open map without saving it. False when it isn't open.
    pub fn close_map(&mut self, name: &str) -> bool {
        if self.doc.as_ref().is_some_and(|d| d.map.name == name) {
            self.doc = None;
            if let Some(next) = self.parked.keys().next().cloned() {
                self.doc = self.parked.remove(&next);
            }
            return true;
        }
        self.parked.remove(name).is_some()
    }

    /// Every open map: the one being worked on first.
    pub fn open_maps(&self) -> Vec<&MapDoc> {
        self.doc.iter().chain(self.parked.values()).collect()
    }

    fn save_doc(project: &Project, doc: &mut MapDoc) -> Result<String> {
        doc.end_stroke();
        project.save_map(&doc.map)?;
        doc.mark_saved();
        Ok(doc.map.name.clone())
    }

    /// Writes the map being worked on.
    pub fn save_map(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        match Workspace::save_doc(&self.project, doc) {
            Ok(name) => {
                if !self.maps.contains(&name) {
                    self.maps.push(name.clone());
                    self.maps.sort();
                }
                self.stale = true;
                self.set_status(format!("Saved map {name}"));
            }
            Err(e) => self.set_status(format!("Save failed: {e:#}")),
        }
    }

    /// Writes the definitions and the project settings.
    pub fn save_defs(&mut self) {
        match self.project.save_defs().and_then(|_| self.project.save_config()) {
            Ok(()) => {
                self.defs_dirty = false;
                *self.defs_saved = (self.project.db.clone(), self.project.config.clone());
                // (What loading left out is gone now.)
                self.defs_warnings.clear();
                self.stale = true;
                self.set_status("Saved definitions");
            }
            Err(e) => self.set_status(format!("Saving definitions failed: {e:#}")),
        }
    }

    /// Is anything unsaved?
    pub fn is_dirty(&self) -> bool {
        self.defs_dirty || self.open_maps().iter().any(|d| d.is_dirty())
    }

    /// Writes everything that changed: every open map, the definitions, then (soon, on a thread)
    /// the project file.
    pub fn save_all(&mut self) {
        if self.doc.as_ref().is_some_and(|d| d.is_dirty()) {
            self.save_map();
        }
        let mut failed = Vec::new();
        for doc in self.parked.values_mut().filter(|d| d.is_dirty()) {
            if let Err(e) = Workspace::save_doc(&self.project, doc) {
                failed.push(format!("{}: {e:#}", doc.map.name));
            }
        }
        if !failed.is_empty() {
            self.set_status(format!("Save failed: {}", failed.join("; ")));
        }
        if self.defs_dirty {
            self.save_defs();
        }
        if let Some(pf) = &mut self.project_file {
            pf.check_soon();
        }
    }

    /// Loads textures and models again (art or asset definitions changed).
    pub fn reload_assets(&mut self) {
        self.stale = true;
        if self.gpu == Gpu::No {
            return;
        }
        self.assets = Assets::load(&self.project);
        if let Some(doc) = &mut self.doc {
            doc.rebuild_overview(&self.assets);
        }
        for doc in self.parked.values_mut() {
            doc.rebuild_overview(&self.assets);
        }
        let n = self.assets.errors.len();
        self.set_status(if n == 0 { "Assets reloaded".to_string() } else { format!("Assets reloaded with {n} error(s)") });
    }
}
