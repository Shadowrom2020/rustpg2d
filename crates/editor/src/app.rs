//! Editor state and egui panels.

use std::collections::HashMap;
use std::path::PathBuf;

use egui_macroquad::egui::{self, pos2, vec2, Color32, Sense, Stroke, StrokeKind};
use macroquad::prelude::{get_time, Vec2};
use rustpg2d_engine::entity::{EntityInstance, EntityKind, Facing};
use rustpg2d_engine::map::{Edge, EdgeLink, Map, Respawn, Tile, Zone, ZoneKind, MAX_MAP_SIZE};
use rustpg2d_engine::audio::Audio;
use rustpg2d_engine::defs::{ContainerSpec, LockSpec, ShopSpec, SpriteRef};
use rustpg2d_engine::entity::{PatrolMode, PatrolRoute, ScheduleStop};

/// Definitions remembered in the entity palette's "Recent" row.
const RECENT_MAX: usize = 10;
use rustpg2d_engine::project::Project;
use rustpg2d_engine::refactor::{self, RefKind};
use rustpg2d_engine::render::Assets;
use rustpg2d_engine::script::ScriptHost;

use crate::database::DbState;
use crate::files;
use crate::doc::{MapDoc, Objects};
use crate::ui::{self, ph};
use crate::widgets::{self, Refs, Textures};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Map,
    Database,
    /// The shared asset library.
    Library,
    /// The pixel editor.
    Pixel,
    /// The open map's events as a graph.
    Events,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Default: click selects entities / zones, drag moves them. Never paints.
    Select,
    Brush,
    Rect,
    Fill,
    Erase,
    Pick,
    Collision,
    Entity,
    Zone,
    /// Drag an exit area, pick the target map, drag the arrival area there.
    Link,
    /// Place multi-tile structures (houses, walls, rooms…) with one click.
    Structure,
    /// Paint an auto-tiling terrain: edge tiles follow the neighbours.
    Terrain,
}

impl Tool {
    pub const ALL: [(Tool, &'static str, &'static str); 12] = [
        (Tool::Select, "Select", "V"),
        (Tool::Brush, "Brush", "B"),
        (Tool::Rect, "Rect", "R"),
        (Tool::Fill, "Fill", "F"),
        (Tool::Erase, "Erase", "E"),
        (Tool::Pick, "Pick", "I"),
        (Tool::Terrain, "Terrain", "A"),
        (Tool::Collision, "Collision", "C"),
        (Tool::Entity, "Entities", "N"),
        (Tool::Zone, "Zones", "Z"),
        (Tool::Link, "Transition", "T"),
        (Tool::Structure, "Structures", "H"),
    ];
    pub fn paints_tiles(self) -> bool {
        matches!(self, Tool::Brush | Tool::Rect | Tool::Fill | Tool::Erase | Tool::Pick)
    }
    pub fn icon(self) -> &'static str {
        use crate::ui::ph;
        match self {
            Tool::Select => ph::CURSOR,
            Tool::Brush => ph::PAINT_BRUSH,
            Tool::Rect => ph::RECTANGLE,
            Tool::Fill => ph::PAINT_BUCKET,
            Tool::Erase => ph::ERASER,
            Tool::Pick => ph::EYEDROPPER,
            Tool::Collision => ph::WALL,
            Tool::Entity => ph::SHAPES,
            Tool::Zone => ph::BOUNDING_BOX,
            Tool::Link => ph::SIGNPOST,
            Tool::Structure => ph::HOUSE,
            Tool::Terrain => ph::MOUNTAINS,
        }
    }
    pub fn name(self) -> &'static str {
        Tool::ALL.iter().find(|t| t.0 == self).map_or("", |t| t.1)
    }
    pub fn key(self) -> &'static str {
        Tool::ALL.iter().find(|t| t.0 == self).map_or("", |t| t.2)
    }
    /// One line on how to use the tool (tooltips and the status bar).
    pub fn help(self) -> &'static str {
        match self {
            Tool::Select => "Click selects an entity or zone, drag moves it, click again cycles through stacked objects; drag on empty space (or Shift+drag) selects a region to copy (Ctrl+C), cut (Ctrl+X) and paste (Ctrl+V); right-click deselects",
            Tool::Brush => "Click or drag paints the stamp on the active layer; right-click picks the tile under the cursor",
            Tool::Rect => "Drag a rectangle to fill it with the stamp; Shift+drag erases",
            Tool::Fill => "Click fills the connected area of identical tiles with the stamp",
            Tool::Erase => "Click or drag empties tiles on the active layer",
            Tool::Pick => "Click picks the tile under the cursor as the stamp (right-click does this with any paint tool)",
            Tool::Collision => "Left: force solid · right: force walkable · Shift+left: back to the tileset default",
            Tool::Entity => "Pick a prop, character or item in the panel and click the map to place it; right-click or Esc stops placing",
            Tool::Zone => "Drag to draw a zone (spawn, portal, trigger, spawner); click selects one, drag moves it",
            Tool::Link => "Drag over the exit area, choose the target map, then drag the arrival area on that map",
            Tool::Structure => "Pick a structure in the panel and click the map to place it; New from map area captures a rectangle as a structure",
            Tool::Terrain => "Pick a terrain in the panel, then click or drag: edge tiles follow the neighbours; Shift erases, right-click picks the terrain under the cursor",
        }
    }
}

/// Selected rectangle of tiles in a tileset (the stamp).
#[derive(Clone, Debug)]
pub struct Brush {
    pub tileset: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Selection {
    None,
    Entity(u32),
    Zone(u32),
    /// A rectangle of tiles `(x, y, w, h)` dragged with the Select tool (copy / cut / delete).
    Region((u32, u32, u32, u32)),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ZoneTemplate {
    Spawn,
    Portal,
    Trigger,
    Spawner,
    Transition,
}

pub struct ViewOpts {
    pub grid: bool,
    pub collision: bool,
    pub zones: bool,
    pub dim_layers: bool,
    pub minimap: bool,
    /// Shows the map's darkness and lights at `hour` (outdoor maps) in the editor.
    pub lighting: bool,
    pub hour: f32,
    /// Shows the map's weather (particles, darker rain) in the editor.
    pub weather: bool,
    /// Draws the map's parallax layers (behind the tiles and over the map).
    pub parallax: bool,
}

pub struct Cam {
    pub center: Vec2,
    pub zoom: f32,
}

pub struct NewMapDialog {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fill: bool,
}

pub enum Pending {
    Open(String),
    /// Open a map and focus a zone (by name; "" = none).
    OpenAt(String, String),
    New,
    Quit,
    OpenProject(PathBuf),
    NewProject(PathBuf),
    /// Back to the welcome screen without a project.
    CloseProject,
}

thread_local! {
    /// "Go to" requests from inspector widgets (map, zone name), handled after the panel.
    static GOTO: std::cell::RefCell<Option<(String, String)>> = const { std::cell::RefCell::new(None) };
}

/// Answer to the "Unsaved changes" question.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    None,
    Save,
    Discard,
    Cancel,
}

/// The tile list edited on the map with clicks (`Editor::route_edit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    Patrol,
    Schedule,
}

pub struct Editor {
    /// Transition tool draft.
    pub link: Option<crate::maps::LinkDraft>,
    pub quick: crate::maps::QuickSwitch,
    /// Open character model chooser.
    pub model: Option<crate::models::ModelChooser>,
    /// The character builder window (new characters, a placed NPC's own look).
    pub builder: Option<crate::builder::Builder>,
    /// Open new project wizard.
    pub new_project: Option<crate::new_project::NewProjectWizard>,
    /// Character whose patrol route is being edited on the map (clicks add waypoints).
    pub route_edit: Option<u32>,
    /// What `route_edit` edits: the patrol route or the daily schedule's stops.
    pub route_kind: RouteKind,
    /// Waypoint being dragged while editing a route, and the map objects before the drag.
    pub route_drag: Option<(usize, crate::doc::Objects)>,
    /// Debug: screen points to report the egui layer under (script command `probe X Y`).
    pub probe: Vec<egui::Pos2>,
    /// Camera (center, zoom) per map, restored when switching back.
    pub map_cams: HashMap<String, (Vec2, f32)>,
    /// Last click on a portal (uid, time) to detect double-clicks.
    pub last_portal_click: Option<(u32, f64)>,
    /// False on the welcome screen: `project` is then an empty placeholder.
    pub project_open: bool,
    pub lib: crate::library_ui::LibState,
    pub px: crate::pixel::PixelState,
    pub wizard: Option<crate::wizard::Wizard>,
    pub export: Option<crate::export::ExportDialog>,
    pub project: Project,
    pub assets: Assets,
    pub audio: Audio,
    /// Inline "rename map" field in the map properties.
    pub map_rename: Option<String>,
    /// Play-tests start in this language (`--lang`; "" = the player's / system's choice).
    pub play_lang: String,
    /// The last translation template export's result (Database › Project › Languages).
    pub lang_msg: Option<String>,
    pub tex: Textures,
    pub tex_dirty: bool,
    pub doc: Option<MapDoc>,
    pub maps: Vec<String>,
    pub mode: Mode,
    pub tool: Tool,
    pub layer: usize,
    pub brush: Brush,
    pub cam: Cam,
    pub sel: Selection,
    pub place_kind: EntityKind,
    /// Definition placed by clicking empty space ("" = nothing: clicks only select).
    pub place_def: String,
    /// Entity tool palette: kind tab (None = all kinds) and search text.
    pub place_tab: Option<EntityKind>,
    pub place_filter: String,
    /// Structures tool: the armed structure id ("" = none), palette search, capture state.
    pub place_structure: String,
    pub structure_filter: String,
    pub capture: crate::structures::CaptureState,
    /// File › Import… window and the last import's result (ok, text).
    pub import: Option<crate::import::ImportDialog>,
    pub import_report: Option<(bool, String)>,
    /// Events mode: layout, scope and editing state of the graph.
    pub graph: crate::graph::GraphState,
    /// Terrain tool: the armed terrain (tileset id, index into its terrains).
    pub terrain: Option<(String, usize)>,
    /// Copied region / entity (Ctrl+C / Ctrl+X), pasted with Ctrl+V; kept across maps.
    pub clipboard: Option<rustpg2d_engine::defs::StructureDef>,
    /// Ctrl+V armed the clipboard: the Select tool shows it at the mouse, clicks paste it.
    pub pasting: bool,
    /// Recently placed / picked definitions, newest first.
    pub place_recent: Vec<(EntityKind, String)>,
    pub zone_template: ZoneTemplate,
    pub view: ViewOpts,
    pub status: (String, f64),
    pub new_map: Option<NewMapDialog>,
    pub resize: Option<(u32, u32)>,
    pub pending: Option<Pending>,
    pub defs_dirty: bool,
    /// Definitions and project settings as last loaded / saved: `defs_dirty` is cleared again
    /// when the edits are reverted (e.g. undone in a text field, a checkbox toggled back).
    pub(crate) defs_saved: Box<(rustpg2d_engine::defs::Database, rustpg2d_engine::project::ProjectConfig)>,
    /// This editor is the Asset Library window (`--library`): `project` is the library.
    pub library_mode: bool,
    /// Lighting preview: renderer and the map's tile lights (refreshed twice a second).
    pub light_pass: Option<rustpg2d_engine::render::lighting::LightPass>,
    /// Weather preview particles.
    pub weather_fx: rustpg2d_engine::render::weather::WeatherFx,
    pub light_cache: (f64, String, Vec<rustpg2d_engine::render::lighting::Light>),
    /// Main editor: the Asset Library window it started.
    pub lib_window: crate::library_window::LibWindow,
    /// Library mode: modification time of the library's definitions as last loaded / saved,
    /// when it was last checked, and whether another editor changed them meanwhile.
    pub lib_stamp: Option<std::time::SystemTime>,
    pub lib_check: f64,
    pub lib_changed: bool,
    pub db: DbState,
    pub refs: Refs,
    pub pointer_over_ui: bool,
    pub keyboard_busy: bool,
    /// Scroll the palette / map entity list to the armed definition / selection once.
    pub palette_scroll_to_armed: bool,
    pub on_map_scroll: bool,
    pub canvas_rect: egui::Rect,
    pub hover_tile: Option<(i32, i32)>,
    pub hover_world: Vec2,
    /// Where the left button went down (world), to tell clicks from drags.
    pub press_world: Vec2,
    /// Select tool: objects under the last click and whether the clicked one was already
    /// selected (a plain click then cycles to the next one).
    pub click_stack: Option<(Vec<Selection>, bool)>,
    pub drag: Option<crate::canvas::Drag>,
    pub quit: bool,
    pub palette_drag: Option<(u32, u32)>,
    /// Native file dialog currently open.
    pub file_dialog: Option<files::PendingDialog>,
    /// A form asked for a file dialog this frame.
    pub browse_request: Option<files::Purpose>,
}

impl Editor {
    /// An editor with the project at `root` open.
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        // Projects from before tiles were fixed at 32 px are converted first (with a backup),
        // restoring library art that was shrunken for them.
        let mut converted = None;
        if rustpg2d_engine::migrate::stored_tile_size(&root)? != rustpg2d_engine::project::TILE_SIZE {
            let lib = rustpg2d_engine::library::open_library(&rustpg2d_engine::library::library_dir(), Some(crate::library_ui::STARTER)).ok();
            converted = rustpg2d_engine::migrate::upgrade(&root, lib.as_ref())?;
        }
        let project = Project::load(&root)?;
        let mut ed = Self::with_project(project, true);
        let first = if ed.maps.contains(&ed.project.config.start_map) {
            Some(ed.project.config.start_map.clone())
        } else {
            ed.maps.first().cloned()
        };
        if let Some(m) = first {
            ed.open_map(&m);
        }
        ed.refresh_refs();
        if !ed.assets.errors.is_empty() {
            ed.set_status(format!("{} asset error(s), see terminal", ed.assets.errors.len()));
        }
        files::remember_recent(&root);
        if let Some(c) = converted {
            println!("[editor] {}", c.summary());
            ed.set_status(c.summary());
        }
        Ok(ed)
    }

    /// The welcome screen: no project open.
    pub fn empty() -> Self {
        let project = Project { root: PathBuf::new(), config: rustpg2d_engine::project::ProjectConfig::new(""), db: Default::default() };
        Self::with_project(project, false)
    }

    pub(crate) fn with_project(project: Project, project_open: bool) -> Self {
        let assets = Assets::load(&project);
        let audio = Audio::load(&project);
        let maps = project.list_maps();
        let brush_ts = project.db.tilesets.first().map(|t| t.id.clone()).unwrap_or_default();
        Editor {
            link: None,
            quick: Default::default(),
            model: None,
            new_project: None,
            route_edit: None,
            route_drag: None,
            route_kind: RouteKind::Patrol,
            probe: Vec::new(),
            map_cams: HashMap::new(),
            last_portal_click: None,
            project_open,
            assets,
            audio,
            map_rename: None,
            play_lang: String::new(),
            lang_msg: None,
            tex: Textures::default(),
            tex_dirty: true,
            doc: None,
            maps,
            mode: Mode::Map,
            tool: Tool::Select,
            layer: 0,
            brush: Brush { tileset: brush_ts, x: 0, y: 0, w: 1, h: 1 },
            cam: Cam { center: Vec2::ZERO, zoom: 2.0 },
            sel: Selection::None,
            place_kind: EntityKind::Prop,
            place_def: String::new(),
            place_tab: None,
            place_filter: String::new(),
            place_structure: String::new(),
            clipboard: None,
            terrain: None,
            graph: Default::default(),
            import: None,
            import_report: None,
            pasting: false,
            structure_filter: String::new(),
            capture: Default::default(),
            place_recent: Vec::new(),
            zone_template: ZoneTemplate::Spawn,
            view: ViewOpts { grid: true, collision: false, zones: true, dim_layers: false, minimap: true, lighting: false, hour: 22.0, weather: false, parallax: true },
            light_pass: None,
            weather_fx: Default::default(),
            light_cache: (-1.0, String::new(), Vec::new()),
            status: (String::new(), 0.0),
            new_map: None,
            resize: None,
            pending: None,
            defs_dirty: false,
            defs_saved: Box::new((project.db.clone(), project.config.clone())),
            library_mode: false,
            lib_window: Default::default(),
            lib_stamp: None,
            lib_check: 0.0,
            lib_changed: false,
            db: DbState::default(),
            lib: Default::default(),
            px: Default::default(),
            wizard: None,
            builder: None,
            export: None,
            refs: Refs::default(),
            pointer_over_ui: false,
            keyboard_busy: false,
            palette_scroll_to_armed: false,
            on_map_scroll: false,
            canvas_rect: egui::Rect::NOTHING,
            hover_tile: None,
            hover_world: Vec2::ZERO,
            press_world: Vec2::ZERO,
            click_stack: None,
            drag: None,
            quit: false,
            palette_drag: None,
            file_dialog: None,
            browse_request: None,
            project,
        }
    }

    /// Closes the project (after unsaved changes were handled) and shows the welcome screen.
    pub fn close_project(&mut self) {
        self.audio.stop_music();
        let rect = self.canvas_rect;
        *self = Editor::empty();
        self.canvas_rect = rect;
    }

    pub fn ts(&self) -> f32 {
        self.assets.tile_size
    }

    pub fn set_status(&mut self, s: impl Into<String>) {
        let s = s.into();
        println!("[editor] {s}");
        self.status = (s, get_time());
    }

    // ------------------------------------------------------------ files

    pub fn open_map(&mut self, name: &str) {
        // Remember where we looked on the map we leave.
        if let Some(d) = &self.doc {
            self.map_cams.insert(d.map.name.clone(), (self.cam.center, self.cam.zoom));
        }
        match self.project.load_map(name) {
            Ok(map) => {
                self.load_doc(map);
                if let Some(&(center, zoom)) = self.map_cams.get(name) {
                    self.cam.center = center;
                    self.cam.zoom = zoom;
                }
                self.set_status(format!("Opened {name}"));
            }
            Err(e) => self.set_status(format!("Failed to open {name}: {e:#}")),
        }
    }

    fn load_doc(&mut self, map: Map) {
        let ts = self.ts();
        self.cam.center = macroquad::math::vec2(map.width as f32 * ts / 2.0, map.height as f32 * ts / 2.0);
        self.layer = self.layer.min(map.layers.len().saturating_sub(1));
        self.sel = Selection::None;
        self.drag = None;
        self.doc = Some(MapDoc::new(map, &self.assets));
        self.fit_view();
        self.refresh_refs();
    }

    pub fn fit_view(&mut self) {
        let Some(doc) = &self.doc else { return };
        let ts = self.ts();
        let (w, h) = (doc.map.width as f32 * ts, doc.map.height as f32 * ts);
        self.cam.center = macroquad::math::vec2(w / 2.0, h / 2.0);
        let r = self.canvas_rect;
        let (cw, ch) = if r.is_positive() && r.width() > 50.0 {
            (r.width(), r.height())
        } else {
            (macroquad::window::screen_width() * 0.6, macroquad::window::screen_height() * 0.8)
        };
        self.cam.zoom = ((cw / w).min(ch / h) * 0.9).clamp(0.02, 4.0);
        if r.is_positive() {
            // Center the map inside the canvas area rather than the whole window.
            let sc = macroquad::math::vec2(macroquad::window::screen_width() / 2.0, macroquad::window::screen_height() / 2.0);
            let off = (macroquad::math::vec2(r.center().x, r.center().y) - sc) / self.cam.zoom;
            self.cam.center -= off;
        }
    }

    pub fn save_map(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        doc.end_stroke();
        match self.project.save_map(&doc.map) {
            Ok(()) => {
                doc.mark_saved();
                let name = doc.map.name.clone();
                if !self.maps.contains(&name) {
                    self.maps.push(name.clone());
                    self.maps.sort();
                }
                self.refresh_refs();
                self.set_status(format!("Saved map {name}"));
            }
            Err(e) => self.set_status(format!("Save failed: {e:#}")),
        }
    }

    pub fn save_defs(&mut self) {
        match self.project.save_defs().and_then(|_| self.project.save_config()) {
            Ok(()) => {
                self.defs_dirty = false;
                *self.defs_saved = (self.project.db.clone(), self.project.config.clone());
                if self.library_mode {
                    self.mark_library_saved();
                    self.lib.tex_dirty = true;
                }
                self.set_status("Saved definitions");
            }
            Err(e) => self.set_status(format!("Saving definitions failed: {e:#}")),
        }
    }

    pub fn save_all(&mut self) {
        if self.doc.as_ref().is_some_and(|d| d.is_dirty()) {
            self.save_map();
        }
        if self.defs_dirty {
            self.save_defs();
        }
        if self.px.is_dirty() {
            self.save_pixel();
        }
    }

    pub fn reload_assets(&mut self) {
        self.lib.tex_dirty = true; // library thumbnails (the library window shows its own art)
        self.assets = Assets::load(&self.project);
        self.audio.stop_music();
        self.audio = Audio::load(&self.project);
        self.tex_dirty = true;
        if let Some(doc) = &mut self.doc {
            doc.rebuild_overview(&self.assets);
        }
        self.refresh_refs();
        let n = self.assets.errors.len() + self.audio.errors.len();
        self.set_status(if n == 0 { "Assets reloaded".to_string() } else { format!("Assets reloaded with {n} error(s)") });
    }

    /// Rebuilds dropdown suggestions (items, maps, spawns, script functions, ...).
    pub fn refresh_refs(&mut self) {
        let mut spawns: HashMap<String, Vec<String>> = HashMap::new();
        let mut transitions: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
        // Names used anywhere in the project (for the script editor's completion).
        let mut all_names: Vec<String> = Vec::new();
        let mut all_flags: Vec<String> = Vec::new();
        let mut all_vars: Vec<String> = Vec::new();
        for m in &self.maps {
            let loaded;
            let map = match &self.doc {
                Some(d) if &d.map.name == m => &d.map,
                _ => match self.project.load_map(m) {
                    Ok(x) => {
                        loaded = x;
                        &loaded
                    }
                    Err(_) => continue,
                },
            };
            all_names.extend(map.entities.iter().filter(|e| !e.name.is_empty()).map(|e| e.name.clone()));
            collect_map_names(map, &mut all_flags, &mut all_vars);
            let zones = map.zones.clone();
            spawns.insert(m.clone(), zones.iter().filter(|z| z.kind == ZoneKind::Spawn).map(|z| z.name.clone()).collect());
            transitions.insert(
                m.clone(),
                zones
                    .iter()
                    .filter_map(|z| match &z.kind {
                        ZoneKind::Transition { target_map, target_zone } => Some((z.name.clone(), target_map.clone(), target_zone.clone())),
                        _ => None,
                    })
                    .collect(),
            );
        }
        let db = &self.project.db;
        for ts in db.items.iter().map(|d| &d.on_use).chain(db.props.iter().map(|d| &d.triggers)).chain(db.characters.iter().map(|d| &d.triggers)) {
            collect_names(ts, &mut all_flags, &mut all_vars);
        }
        for v in [&mut all_names, &mut all_flags, &mut all_vars] {
            v.retain(|s| !s.is_empty());
            v.sort();
            v.dedup();
        }
        let mut scripts = ScriptHost::new();
        scripts.load_dir(&self.project.scripts_dir());
        let mut targets = vec!["self".to_string(), "player".to_string()];
        let mut flags = Vec::new();
        if let Some(doc) = &self.doc {
            targets.extend(doc.map.entities.iter().filter(|e| !e.name.is_empty()).map(|e| e.name.clone()));
            collect_flags(&doc.map, &mut flags);
        }
        flags.sort();
        flags.dedup();
        self.refs = Refs {
            items: self.project.db.items.iter().map(|i| i.id.clone()).collect(),
            maps: self.maps.clone(),
            targets,
            functions: scripts.function_names(),
            spawns,
            transitions,
            current_map: self.doc.as_ref().map(|d| d.map.name.clone()).unwrap_or_default(),
            flags,
            sounds: self.project.db.sounds.iter().map(|s| s.id.clone()).collect(),
            characters: self.project.db.characters.iter().map(|c| c.id.clone()).collect(),
            states: {
                let mut v: Vec<String> = self.project.db.props.iter().flat_map(|p| p.state_names()).collect();
                v.sort();
                v.dedup();
                v
            },
            entity_names: all_names,
            speakers: self.doc.as_ref().map(|d| speakers_of(&d.map, &self.project.db)).unwrap_or_default(),
            all_flags,
            vars: all_vars,
            quests: self.project.db.quests.iter().map(|q| q.id.clone()).collect(),
            quest_stages: self.project.db.quests.iter().map(|q| (q.id.clone(), q.stages.clone())).collect(),
            skills: self.project.db.skills.iter().map(|q| q.id.clone()).collect(),
            spells: self.project.db.spells.iter().map(|q| q.id.clone()).collect(),
            achievements: self.project.db.achievements.iter().map(|a| a.id.clone()).collect(),
            stations: {
                let mut v: Vec<String> = self.project.db.recipes.iter().map(|r| r.station.clone()).filter(|s| !s.is_empty()).collect();
                v.sort();
                v.dedup();
                v
            },
            sheets: self.project.db.sheets.iter().map(|q| q.id.clone()).collect(),
            sheet_tags: self.project.db.sheets.iter().filter(|q| !q.tags.is_empty()).map(|q| (q.id.clone(), q.tags.clone())).collect(),
        };
    }

    /// Renames a definition id (or a map) and rewrites every reference in the project:
    /// definitions, project config, all map files and the open map. Saves immediately.
    /// Follows a renamed prop state in placed props (open map and maps on disk): their start
    /// state, lock / container overrides and their own triggers aimed at "self".
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
            crate::database::rename_state_in_triggers(&mut e.triggers, old, new);
            before != format!("{:?}{:?}{:?}{:?}", e.state, e.lock, e.container, e.triggers)
        };
        let mut changed = 0;
        let open = self.doc.as_ref().map(|d| d.map.name.clone());
        if let Some(doc) = &mut self.doc {
            let n = doc.map.entities.iter_mut().map(|e| fix(e) as usize).sum::<usize>();
            if n > 0 {
                changed += n;
                doc.mark_changed();
            }
        }
        for m in self.maps.clone() {
            if Some(&m) == open.as_ref() {
                continue;
            }
            let Ok(mut map) = self.project.load_map(&m) else { continue };
            let n = map.entities.iter_mut().map(|e| fix(e) as usize).sum::<usize>();
            if n > 0 && self.project.save_map(&map).is_ok() {
                changed += n;
            }
        }
        self.defs_dirty = true;
        self.set_status(format!("Renamed state '{old}' \u{E13A} '{new}' of '{prop}' ({changed} placed prop(s) updated). Save the definitions to keep it."));
    }

    pub fn rename_id(&mut self, kind: RefKind, old: &str, new: &str) {
        let new = new.trim();
        if new == old || new.is_empty() {
            return;
        }
        if !valid_name(new) {
            self.set_status("Ids may only contain letters, digits, '_' and '-'");
            return;
        }
        let db = &mut self.project.db;
        let taken = match kind {
            RefKind::Tileset => db.tilesets.iter().any(|d| d.id == new),
            RefKind::Sheet => db.sheets.iter().any(|d| d.id == new),
            RefKind::Item => db.items.iter().any(|d| d.id == new),
            RefKind::Prop => db.props.iter().any(|d| d.id == new),
            RefKind::Character => db.characters.iter().any(|d| d.id == new),
            RefKind::Sound => db.sounds.iter().any(|d| d.id == new),
            RefKind::Map => self.maps.iter().any(|m| m == new),
            RefKind::Structure => db.structures.iter().any(|d| d.id == new),
            RefKind::Skill => db.skills.iter().any(|d| d.id == new),
            RefKind::Spell => db.spells.iter().any(|d| d.id == new),
            RefKind::Recipe => db.recipes.iter().any(|d| d.id == new),
        };
        if taken {
            self.set_status(format!("'{new}' is already used"));
            return;
        }
        // The thing itself.
        macro_rules! rename_def {
            ($list:expr) => {
                if let Some(d) = $list.iter_mut().find(|d| d.id == old) {
                    d.id = new.to_string();
                }
            };
        }
        match kind {
            RefKind::Tileset => rename_def!(db.tilesets),
            RefKind::Sheet => rename_def!(db.sheets),
            RefKind::Item => rename_def!(db.items),
            RefKind::Prop => rename_def!(db.props),
            RefKind::Character => rename_def!(db.characters),
            RefKind::Sound => rename_def!(db.sounds),
            RefKind::Structure => rename_def!(db.structures),
            RefKind::Skill => rename_def!(db.skills),
            RefKind::Spell => rename_def!(db.spells),
            RefKind::Recipe => rename_def!(db.recipes),
            RefKind::Map => {
                if let Err(e) = self.project.rename_map_file(old, new) {
                    self.set_status(format!("Rename failed: {e:#}"));
                    return;
                }
                for m in &mut self.maps {
                    if m == old {
                        *m = new.to_string();
                    }
                }
                self.maps.sort();
            }
        }
        let mut refs = refactor::rename_in_db(&mut self.project.db, &mut self.project.config, kind, old, new);

        // Every map on disk, then the open one (which may have unsaved edits).
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
        if let Some(doc) = &mut self.doc {
            if refactor::rename_in_map(&mut doc.map, kind, old, new) > 0 {
                // Old snapshots still contain the old id.
                doc.mark_changed();
                doc.clear_history();
            }
        }

        // Editor state that refers to ids.
        match kind {
            RefKind::Tileset if self.brush.tileset == old => self.brush.tileset = new.to_string(),
            RefKind::Prop | RefKind::Character | RefKind::Item if self.place_def == old => self.place_def = new.to_string(),
            RefKind::Structure if self.place_structure == old => self.place_structure = new.to_string(),
            _ => {}
        }
        self.save_defs();
        if matches!(kind, RefKind::Tileset | RefKind::Sheet | RefKind::Sound) {
            self.reload_assets();
        }
        self.refresh_refs();
        let mut msg = format!("Renamed '{old}' \u{E13A} '{new}': {refs} reference(s) updated in {maps_changed} map file(s)");
        // Ids passed to script functions (give_item("old"), teleport("old", ...), ...).
        let changed = self.rename_in_scripts(kind, old, new, &mut errors);
        if !changed.is_empty() {
            msg.push_str(&format!(", scripts: {}", changed.join(", ")));
        }
        if !errors.is_empty() {
            msg = format!("{msg}; errors: {}", errors.join("; "));
        }
        self.set_status(msg);
    }

    /// Applies a rename to the project's scripts (backups as `<file>.rhai.bak`). Returns
    /// "file:line,line" for every changed file. The open script editor follows.
    fn rename_in_scripts(&mut self, kind: RefKind, old: &str, new: &str, errors: &mut Vec<String>) -> Vec<String> {
        let mut changed = Vec::new();
        let Ok(rd) = std::fs::read_dir(self.project.scripts_dir()) else { return changed };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect();
        files.sort();
        for f in files {
            // The open file's editor text may have unsaved edits: rename in it as well.
            let open = self.db.script_file.as_ref() == Some(&f);
            let Ok(disk) = std::fs::read_to_string(&f) else { continue };
            let (text, lines) = rustpg2d_engine::script_refs::rename_in_script(&disk, kind, old, new);
            if open {
                let (buf, _) = rustpg2d_engine::script_refs::rename_in_script(&self.db.script_text, kind, old, new);
                self.db.script_text = buf;
                self.db.script_dirty = self.db.script_text != self.db.script_saved;
            }
            if lines.is_empty() {
                continue;
            }
            let mut bak = f.clone().into_os_string();
            bak.push(".bak");
            let res = std::fs::write(&bak, &disk).and_then(|_| std::fs::write(&f, &text));
            if let Err(e) = res {
                errors.push(format!("{}: {e}", f.display()));
                continue;
            }
            if open {
                self.db.script_saved = text;
                self.db.script_dirty = self.db.script_text != self.db.script_saved;
            }
            let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
            let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            changed.push(format!("{name}:{}", lines.join(",")));
        }
        changed
    }

    /// Writes `lang/<code>.ron` with every content text of the project (saving everything first,
    /// so the maps' texts are current): existing translations are kept, new texts are added
    /// empty. Adds the language to the project's translations. Returns a summary.
    pub fn export_translation(&mut self, code: &str) -> Result<String, String> {
        let code = code.trim().to_lowercase();
        if code.len() < 2 || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!("\"{code}\" is not a language code (like de, fr or pt-br)"));
        }
        if code == self.project.config.language {
            return Err(format!("{code} is the game's own language"));
        }
        if !self.project.config.languages.contains(&code) {
            self.project.config.languages.push(code.clone());
            self.defs_dirty = true;
        }
        self.save_all();
        let existing = rustpg2d_engine::lang::load_file(&self.project.root, &code);
        let (file, unused) = rustpg2d_engine::lang::template(&self.project, &code, existing);
        let path = self.project.root.join("lang").join(format!("{code}.ron"));
        rustpg2d_engine::project::write_ron(&path, &file).map_err(|e| format!("{e:#}"))?;
        let open = file.texts.values().filter(|v| v.is_empty()).count();
        let mut msg = format!("lang/{code}.ron: {} texts, {open} not translated yet", file.texts.len());
        if !unused.is_empty() {
            msg += &format!(", {} no longer used (kept, at the end of the list in your editor: delete them by hand)", unused.len());
        }
        Ok(msg)
    }

    pub fn play(&mut self, here: bool) {
        self.play_with(here, None);
    }

    /// Play-tests script function `function` as a cut-scene on the open map (the start map
    /// without one), right after arriving - without the map's Map enter triggers.
    pub fn play_cutscene(&mut self, function: &str) {
        self.play_with(true, Some(function));
    }

    fn play_with(&mut self, here: bool, cutscene: Option<&str>) {
        self.save_all();
        let runner = rustpg2d_engine::release::Platform::host().game_runner();
        let exe = std::env::current_exe().ok().map(|p| p.with_file_name(runner));
        let Some(exe) = exe.filter(|p| p.exists()) else {
            self.set_status(format!("{runner} not found next to the editor; run `cargo build -p rustpg2d-game`"));
            return;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg(&self.project.root);
        // Developer play-test: scripts reload when saved, F4 opens the console.
        cmd.arg("--dev");
        if !here || cutscene.is_some() {
            cmd.arg("--no-title");
        }
        if let Some(f) = cutscene {
            cmd.args(["--cutscene", f]);
        }
        if !self.play_lang.is_empty() {
            cmd.args(["--lang", &self.play_lang]);
        }
        if here {
            if let Some(doc) = &self.doc {
                cmd.args(["--map", &doc.map.name]);
                let spawn = match self.sel {
                    Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid && z.kind == ZoneKind::Spawn),
                    _ => None,
                };
                match spawn {
                    Some(z) => cmd.args(["--spawn", &z.name]),
                    None => cmd.args(["--pos", &format!("{},{}", self.cam.center.x, self.cam.center.y)]),
                };
            }
        }
        // A game built from a different engine may not understand this editor's map data
        // (e.g. `cargo run -p rustpg2d-editor` does not rebuild rustpg2d-game).
        let game_build = std::process::Command::new(cmd.get_program())
            .arg("--engine-build")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        let stale = game_build.as_deref() != Some(rustpg2d_engine::ENGINE_BUILD);
        match cmd.spawn() {
            Ok(_) if stale => self.set_status(
                "Play-test started, but rustpg2d-game was built from a different engine version; if it fails, run `cargo build -p rustpg2d-game` (or `cargo build --workspace`)",
            ),
            Ok(_) if cutscene.is_some() => self.set_status(format!("Playing cut-scene {}()", cutscene.unwrap_or_default())),
            Ok(_) => self.set_status("Play-test started"),
            Err(e) => self.set_status(format!("Failed to start game: {e}")),
        }
    }

    // ------------------------------------------------------------ files

    /// Replaces the editor state with another project.
    pub fn switch_project(&mut self, root: PathBuf) {
        // (Callers go through `request`, so unsaved changes were saved or discarded.)
        if !root.join("project.ron").exists() {
            self.set_status(format!("{} is not an R(ust)PG2D project (no project.ron)", root.display()));
            return;
        }
        match Editor::new(root.clone()) {
            Ok(mut e) => {
                self.audio.stop_music();
                e.canvas_rect = self.canvas_rect;
                *self = e;
                self.fit_view();
                self.set_status(format!("Opened project {}", root.display()));
            }
            Err(e) => self.set_status(format!("Could not open project: {e:#}")),
        }
    }

    pub fn open_file_dialog(&mut self, purpose: files::Purpose) {
        if self.file_dialog.is_some() {
            self.set_status("A file dialog is already open");
            return;
        }
        let start = match purpose {
            files::Purpose::NewProjectParent => self.new_project.as_ref().map(|w| PathBuf::from(w.parent.trim())).filter(|p| p.is_dir()),
            files::Purpose::OpenProject | files::Purpose::NewProject => match self.project.root.parent() {
                Some(p) if self.project_open => Some(p.to_path_buf()),
                _ => rustpg2d_engine::paths::home(),
            },
            _ => Some(self.project.root.clone()),
        };
        self.file_dialog = Some(files::PendingDialog::open(purpose, start));
    }

    /// Applies the result of a file dialog (also used directly by the debug script).
    pub fn handle_file(&mut self, purpose: files::Purpose, path: PathBuf) {
        use files::Purpose as P;
        let root = self.project.root.clone();
        let import = |sub: &str| files::import_asset(&root, &path, sub);
        match purpose {
            P::ImportPackage { into_library } => {
                let r = self.import_package(into_library, &path);
                self.show_result(r);
            }
            P::ExportPackage { from_library: true, assets } => {
                let r = self.export_package(&assets, &path);
                self.show_result(r);
            }
            P::ExportPackage { from_library: false, assets } => self.send_project_assets(assets, crate::library_ui::SendTo::Package(path)),
            P::ExportGameFolder => {
                if let Some(d) = &mut self.export {
                    d.out = path.display().to_string();
                }
            }
            P::ExportIcon => {
                if let Some(d) = &mut self.export {
                    d.opts.icon = path.display().to_string();
                }
            }
            P::Import(kind) => self.import_and_report(kind, &path),
            P::OpenProject => self.request(Pending::OpenProject(path)),
            P::NewProject => self.request(Pending::NewProject(path)),
            P::NewProjectParent => {
                if let Some(w) = &mut self.new_project {
                    w.parent = path.display().to_string();
                }
            }
            P::TilesetImage(ref id) | P::SheetImage(ref id) | P::SoundFile(ref id) => {
                let id = id.clone();
                let sub = match purpose {
                    P::TilesetImage(_) => "tilesets",
                    P::SheetImage(_) => "sprites",
                    _ => "sounds",
                };
                let rel = match import(sub) {
                    Ok(rel) => rel,
                    Err(e) => {
                        self.set_status(format!("Import failed: {e:#}"));
                        return;
                    }
                };
                let db = &mut self.project.db;
                match purpose {
                    P::TilesetImage(_) => db.tilesets.iter_mut().filter(|d| d.id == id).for_each(|d| d.image = rel.clone()),
                    P::SheetImage(_) => db.sheets.iter_mut().filter(|d| d.id == id).for_each(|d| d.image = rel.clone()),
                    _ => db.sounds.iter_mut().filter(|d| d.id == id).for_each(|d| d.file = rel.clone()),
                }
                self.defs_dirty = true;
                self.reload_assets();
                self.set_status(format!("Using {rel}"));
            }
        }
    }

    /// Shows the outcome of a library / package action in the status bar and the library page.
    pub fn show_result(&mut self, r: anyhow::Result<String>) {
        let msg = match r {
            Ok(m) => m,
            Err(e) => format!("Error: {e:#}"),
        };
        self.set_status(msg.clone());
        self.lib.report = Some(msg);
    }

    fn poll_file_dialog(&mut self) {
        if let Some(req) = self.browse_request.take() {
            self.open_file_dialog(req);
        }
        let Some(d) = &self.file_dialog else { return };
        let Some(result) = d.poll() else { return };
        let purpose = d.purpose.clone();
        self.file_dialog = None;
        if let Some(path) = result {
            self.handle_file(purpose, path);
        }
    }

    // ------------------------------------------------------------ helpers

    /// Brush tile at stamp offset (dx, dy), registering the tileset on the map.
    pub fn stamp_tile(&mut self, dx: u32, dy: u32) -> Tile {
        let Some(doc) = &mut self.doc else { return Tile::EMPTY };
        let Some(ts) = self.assets.tilesets.get(&self.brush.tileset) else { return Tile::EMPTY };
        let slot = doc.map.tileset_slot(&self.brush.tileset);
        let (x, y) = (self.brush.x + dx % self.brush.w, self.brush.y + dy % self.brush.h);
        Tile::new(slot, y * ts.columns + x)
    }

    /// Snaps an entity's feet position to the grid. Props an even number of tiles wide are
    /// centered on a tile edge, odd ones on a tile center (so they cover whole tiles).
    pub fn snap_entity(&self, world: Vec2, free: bool, tiles_wide: u32) -> Vec2 {
        let ts = self.ts();
        if free {
            return world.round();
        }
        let ty = (world.y / ts).floor();
        let x = if tiles_wide.is_multiple_of(2) { (world.x / ts).round() * ts } else { ((world.x / ts).floor() + 0.5) * ts };
        macroquad::math::vec2(x, (ty + 1.0) * ts - 1.0)
    }

    /// Sprite of an entity definition (props / items; characters use their idle frame, layered
    /// ones a `@char:` marker that `widgets::paint_sprite` paints from their layers).
    pub fn def_sprite(&self, kind: EntityKind, def: &str) -> Option<rustpg2d_engine::defs::SpriteRef> {
        let db = &self.project.db;
        match kind {
            EntityKind::Prop => db.prop(def).map(|d| d.sprite.clone()),
            EntityKind::Item => db.item(def).map(|d| d.icon.clone()),
            EntityKind::Character => db.character(def).map(crate::widgets::character_sprite),
        }
    }

    /// Layers of a character definition, or of a placed character with its own look / items.
    pub fn char_layers(&self, def: &str, inst: Option<&rustpg2d_engine::entity::EntityInstance>, facing: rustpg2d_engine::entity::Facing, col: u32) -> Vec<rustpg2d_engine::render::character::Layer> {
        use rustpg2d_engine::render::character;
        let Some(d) = self.project.db.character(def) else { return Vec::new() };
        character::layers(&self.project.db, character::appearance_of(d, inst), character::equipment_of(d, inst), facing, col, false)
    }

    /// Draws character layers with the bottom-center at `feet` (map view, macroquad).
    pub fn draw_char_layers(&self, layers: &[rustpg2d_engine::render::character::Layer], feet: Vec2, tint: macroquad::color::Color) -> bool {
        let mut any = false;
        for l in layers {
            any |= self.assets.draw_frame_colored(&l.sheet, l.frame, feet, tint, l.skin, l.hair);
        }
        any
    }

    /// Width of an entity definition's sprite in whole tiles (for snapping).
    pub fn def_tiles_wide(&self, kind: EntityKind, def: &str) -> u32 {
        let ts = self.ts();
        self.def_sprite(kind, def).map_or(1, |s| (self.assets.sprite_size(&s, ts).x / ts).round().max(1.0) as u32)
    }

    pub fn place_entity(&mut self, pos: Vec2) {
        if self.place_def.is_empty() {
            self.set_status("Choose a definition to place first");
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        let uid = doc.map.alloc_uid();
        doc.map.entities.push(EntityInstance::new(uid, self.place_kind, &self.place_def, pos.x, pos.y));
        doc.record_objects(before, "place");
        self.sel = Selection::Entity(uid);
        self.remember_placed();
    }

    /// Arms a definition for placing (empty `def` = stop placing).
    pub fn arm(&mut self, kind: EntityKind, def: &str) {
        self.place_kind = kind;
        self.place_def = def.to_string();
        if def.is_empty() {
            self.set_status("Placing off: clicks select");
        } else {
            self.remember_placed();
        }
    }

    pub fn disarm(&mut self) {
        if !self.place_def.is_empty() {
            self.arm(self.place_kind, "");
        }
    }

    fn remember_placed(&mut self) {
        let cur = (self.place_kind, self.place_def.clone());
        self.place_recent.retain(|r| *r != cur);
        self.place_recent.insert(0, cur);
        self.place_recent.truncate(RECENT_MAX);
    }

    /// Definitions shown in the entity palette (current tab + search): (kind, id, name, sprite).
    pub fn palette_entries(&self) -> Vec<(EntityKind, String, String, SpriteRef)> {
        let db = &self.project.db;
        let q = self.place_filter.trim().to_lowercase();
        EntityKind::ALL
            .into_iter()
            .filter(|k| self.place_tab.is_none_or(|t| t == *k))
            .flat_map(|k| -> Vec<(EntityKind, String, String, SpriteRef)> {
                match k {
                    EntityKind::Prop => db.props.iter().map(|d| (k, d.id.clone(), d.name.clone(), d.sprite.clone())).collect(),
                    EntityKind::Character => db.characters.iter().map(|d| (k, d.id.clone(), d.name.clone(), crate::widgets::character_sprite(d))).collect(),
                    EntityKind::Item => db.items.iter().map(|d| (k, d.id.clone(), d.name.clone(), d.icon.clone())).collect(),
                }
            })
            .filter(|(_, id, name, _)| q.is_empty() || id.to_lowercase().contains(&q) || name.to_lowercase().contains(&q))
            .collect()
    }

    /// Arms the next / previous palette entry (`[` / `]`).
    pub fn cycle_place(&mut self, step: i32) {
        let entries = self.palette_entries();
        if entries.is_empty() {
            return;
        }
        let n = entries.len() as i32;
        let cur = entries.iter().position(|(k, id, ..)| *k == self.place_kind && *id == self.place_def);
        let i = match cur {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step > 0 => 0,
            None => n - 1,
        } as usize;
        let (k, id, name, _) = entries[i].clone();
        self.arm(k, &id);
        self.set_status(format!("Placing {name}"));
    }

    pub fn delete_selection(&mut self) {
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        match self.sel {
            Selection::Entity(uid) => doc.map.entities.retain(|e| e.uid != uid),
            Selection::Zone(uid) => doc.map.zones.retain(|z| z.uid != uid),
            // A region is emptied and stays selected (its own undo step).
            Selection::Region(r) => return doc.clear_region(r, &self.assets),
            Selection::None => return,
        }
        doc.record_objects(before, "delete");
        self.sel = Selection::None;
    }

    pub fn duplicate_selection(&mut self) {
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        match self.sel {
            Selection::Entity(uid) => {
                let Some(mut e) = doc.map.entity(uid).cloned() else { return };
                e.uid = doc.map.alloc_uid();
                e.x += ts;
                if !e.name.is_empty() {
                    e.name = format!("{}_copy", e.name);
                }
                self.sel = Selection::Entity(e.uid);
                doc.map.entities.push(e);
            }
            Selection::Zone(uid) => {
                let Some(mut z) = doc.map.zones.iter().find(|z| z.uid == uid).cloned() else { return };
                z.uid = doc.map.alloc_uid();
                z.x = (z.x + 1).min(doc.map.width - 1);
                z.name = format!("{}_copy", z.name);
                self.sel = Selection::Zone(z.uid);
                doc.map.zones.push(z);
            }
            Selection::Region(_) | Selection::None => return,
        }
        doc.record_objects(before, "duplicate");
    }

    // ------------------------------------------------------------ egui

    pub fn ui(&mut self, ctx: &egui::Context) {
        for p in std::mem::take(&mut self.probe) {
            let layer = ctx.layer_id_at(p);
            let ppp = ctx.pixels_per_point();
            println!("[probe] {p:?}: layer {layer:?} (pixels per point {ppp})");
        }
        self.poll_file_dialog();
        if self.defs_dirty && self.defs_saved.0 == self.project.db && self.defs_saved.1 == self.project.config {
            self.defs_dirty = false; // edited and changed back
        }
        if self.tex_dirty {
            self.tex = Textures::build(ctx, &self.assets);
            self.tex_dirty = false;
        }
        if self.library_mode {
            self.library_menu_bar(ctx);
            self.watch_library();
            if self.mode == Mode::Map {
                self.mode = Mode::Library;
            }
            if self.mode != Mode::Pixel && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                self.save_all();
            }
        } else {
            self.menu_bar(ctx);
        }
        if !self.project_open {
            self.status_bar(ctx);
            self.welcome(ctx);
            self.new_project_window(ctx);
            self.dialogs(ctx);
            self.canvas_rect = ctx.available_rect();
            self.pointer_over_ui = true;
            self.keyboard_busy = ctx.wants_keyboard_input();
            return;
        }
        self.status_bar(ctx);
        self.tex.set_characters(&self.project.db);
        match self.mode {
            Mode::Map => {
                egui::SidePanel::left("left").resizable(true).default_width(290.0).show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("left_scroll").auto_shrink(false).show(ui, |ui| self.left_panel(ui));
                });
                egui::SidePanel::right("right").resizable(true).default_width(340.0).show(ctx, |ui| {
                    egui::ScrollArea::vertical().id_salt("right_scroll").auto_shrink(false).show(ui, |ui| self.inspector(ui));
                });
            }
            Mode::Database => {
                egui::CentralPanel::default().show(ctx, |ui| crate::database::ui(self, ui));
            }
            Mode::Library => self.library_ui(ctx),
            Mode::Pixel => self.pixel_ui(ctx),
            Mode::Events => self.events_ui(ctx),
        }
        self.wizard_window(ctx);
        self.builder_window(ctx);
        self.apply_builder_requests();
        if let Some(f) = crate::widgets::take_play_cutscene() {
            self.play_cutscene(&f);
        }
        self.map_windows(ctx);
        self.model_window(ctx);
        self.new_project_window(ctx);
        self.handle_transition_request();
        if let Some((m, zone)) = GOTO.with(|g| g.borrow_mut().take()) {
            self.goto_map(&m, Some(&zone));
        }
        self.export_window(ctx);
        self.capture_window(ctx);
        self.import_window(ctx);
        self.confirm_remove_dialog(ctx);
        self.script_pick_dialog(ctx);
        self.add_from_library_dialog(ctx);
        self.canvas_rect = ctx.available_rect();
        self.dialogs(ctx);
        self.pointer_over_ui = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
        self.keyboard_busy = ctx.wants_keyboard_input();
    }

    /// Start screen when no project is open.
    /// Opens (a copy of) the bundled example project.
    pub fn open_example(&mut self) {
        match files::example_copy() {
            Ok(p) => self.request(Pending::OpenProject(p)),
            Err(e) => self.set_status(format!("Example project: {e:#}")),
        }
    }

    fn welcome(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading(egui::RichText::new("R(ust)PG2D editor").size(34.0));
                ui.label("Maps, characters, props, items and events for top-down RPGs.");
                ui.add_space(24.0);
                if ui.add_sized([260.0, 36.0], egui::Button::new("\u{E6A2} New project…")).on_hover_text("Choose an empty folder for the project").clicked() {
                    self.request(Pending::NewProject(PathBuf::new()));
                }
                ui.add_space(6.0);
                if ui.add_sized([260.0, 36.0], egui::Button::new("\u{E256} Open project…")).on_hover_text("Choose a folder containing project.ron").clicked() {
                    self.open_file_dialog(files::Purpose::OpenProject);
                }
                // Packaged R(ust)PG2D (AppImage / Flatpak): the example game, copied out to be editable.
                if files::bundled_example().is_some() {
                    ui.add_space(6.0);
                    if ui
                        .add_sized([260.0, 36.0], egui::Button::new(ui::t(ph::GAME_CONTROLLER, "Example project")))
                        .on_hover_text("Opens the example game; the first time it is copied to ~/rustpg2d/example_project")
                        .clicked()
                    {
                        self.open_example();
                    }
                }
                let recent = files::recent_projects();
                if !recent.is_empty() {
                    ui.add_space(24.0);
                    ui.label(egui::RichText::new("Recent projects").strong());
                    for p in recent {
                        let name = p.file_name().map_or(p.display().to_string(), |n| n.to_string_lossy().into_owned());
                        if ui.add_sized([360.0, 28.0], egui::Button::new(&name)).on_hover_text(p.display().to_string()).clicked() {
                            self.request(Pending::OpenProject(p));
                        }
                    }
                }
                ui.add_space(24.0);
                ui.weak("Tip: the Asset Library (Assets menu, once a project is open) has ready-made tiles, characters, props, items and structures.");
            });
        });
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        if !self.project_open {
            egui::TopBottomPanel::top("menu").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New project…").clicked() {
                            self.request(Pending::NewProject(PathBuf::new()));
                            ui.close_menu();
                        }
                        if ui.button("Open project…").clicked() {
                            self.open_file_dialog(files::Purpose::OpenProject);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            self.request(Pending::Quit);
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    ui.label(egui::RichText::new("(no project)").weak());
                });
            });
            return;
        }
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button(ui::t(ph::FILE_PLUS, "New project…")).clicked() {
                        self.request(Pending::NewProject(PathBuf::new()));
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::FOLDER_OPEN, "Open project…")).clicked() {
                        self.open_file_dialog(files::Purpose::OpenProject);
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::X, "Close project")).clicked() {
                        self.request(Pending::CloseProject);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::FLOPPY_DISK, "Save all   Ctrl+S")).on_hover_text("Saves the open map, the definitions and the image in the pixel editor").clicked() {
                        self.save_all();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::DOWNLOAD_SIMPLE, "Import…")).on_hover_text("A Tiled map (.tmx / .tmj), an LPC character sheet or an Aseprite sprite sheet").clicked() {
                        self.import = Some(Default::default());
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::PACKAGE, "Export game (standalone)…")).clicked() {
                        self.open_export();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::POWER, "Quit")).clicked() {
                        self.request(Pending::Quit);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let (u, r) = self.doc.as_ref().map_or((false, false), |d| (d.can_undo(), d.can_redo()));
                    if ui.add_enabled(u, egui::Button::new(ui::t(ph::ARROW_ARC_LEFT, "Undo   Ctrl+Z"))).clicked() {
                        if let Some(d) = &mut self.doc {
                            d.undo(&self.assets);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(r, egui::Button::new(ui::t(ph::ARROW_ARC_RIGHT, "Redo   Ctrl+Y"))).clicked() {
                        if let Some(d) = &mut self.doc {
                            d.redo(&self.assets);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let copyable = matches!(self.sel, Selection::Region(_) | Selection::Entity(_));
                    if ui.add_enabled(copyable, egui::Button::new(ui::t(ph::SCISSORS, "Cut   Ctrl+X"))).on_hover_text("A region dragged with the Select tool, or the selected entity").clicked() {
                        self.cut_selection();
                        ui.close_menu();
                    }
                    if ui.add_enabled(copyable, egui::Button::new(ui::t(ph::COPY, "Copy   Ctrl+C"))).on_hover_text("A region dragged with the Select tool, or the selected entity").clicked() {
                        self.copy_selection();
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.clipboard.is_some() && self.doc.is_some(), egui::Button::new(ui::t(ph::CLIPBOARD_TEXT, "Paste   Ctrl+V"))).clicked() {
                        self.start_paste();
                        ui.close_menu();
                    }
                    ui.separator();
                    let dup = matches!(self.sel, Selection::Entity(_) | Selection::Zone(_));
                    if ui.add_enabled(dup, egui::Button::new(ui::t(ph::COPY_SIMPLE, "Duplicate   Ctrl+D"))).clicked() {
                        self.duplicate_selection();
                        ui.close_menu();
                    }
                    if ui.add_enabled(self.sel != Selection::None, egui::Button::new(ui::t(ph::TRASH, "Delete   Del"))).clicked() {
                        self.delete_selection();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Map", |ui| {
                    if ui.button(ui::t(ph::PLUS, "New map…")).clicked() {
                        self.request(Pending::New);
                        ui.close_menu();
                    }
                    ui.menu_button(ui::t(ph::MAP_TRIFOLD, "Open map"), |ui| {
                        for m in self.maps.clone() {
                            if ui.button(&m).clicked() {
                                self.request(Pending::Open(m));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.weak("Ctrl+M: search maps · Ctrl+PgUp / PgDn: previous / next");
                    ui.separator();
                    if ui.button(ui::t(ph::FLOPPY_DISK, "Save map")).clicked() {
                        self.save_map();
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::CORNERS_OUT, "Fit map to view   Home")).clicked() {
                        self.fit_view();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Assets", |ui| {
                    if ui.button(ui::t(ph::SPARKLE, "New asset…")).on_hover_text("A prop, item, character, tileset, sprite sheet or structure for this project").clicked() {
                        self.wizard = Some(crate::wizard::Wizard::new(&self.project));
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::DOWNLOAD_SIMPLE, "Add from library…")).on_hover_text("Copies assets from the shared asset library into this project").clicked() {
                        self.open_add_dialog(None);
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::PACKAGE, "Import asset package into project…")).clicked() {
                        self.open_file_dialog(files::Purpose::ImportPackage { into_library: false });
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::BOOKS, "Asset Library window…")).on_hover_text("Opens the shared asset library in its own window: edit, create, import and export library assets").clicked() {
                        self.open_library_window();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(ui::t(ph::FLOPPY_DISK, "Save definitions")).clicked() {
                        self.save_defs();
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::ARROW_CLOCKWISE, "Reload assets & scripts")).clicked() {
                        self.reload_assets();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.view.grid, "Grid   G");
                    ui.checkbox(&mut self.view.collision, "Collision overlay");
                    ui.checkbox(&mut self.view.zones, "Zones & edges");
                    ui.checkbox(&mut self.view.dim_layers, "Dim inactive layers");
                    ui.checkbox(&mut self.view.minimap, "Minimap");
                    ui.checkbox(&mut self.view.lighting, "Lighting preview");
                    ui.checkbox(&mut self.view.weather, "Weather preview");
                    ui.checkbox(&mut self.view.parallax, "Parallax layers");
                });
                ui.menu_button("Play", |ui| {
                    if ui.button(ui::t(ph::PLAY, "Play here   F5")).clicked() {
                        self.play(true);
                        ui.close_menu();
                    }
                    if ui.button(ui::t(ph::FAST_FORWARD, "Play from start   F6")).clicked() {
                        self.play(false);
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui::tab(ui, self.mode == Mode::Map, ph::MAP_TRIFOLD, "Map").on_hover_text("Maps: tiles, entities, zones, transitions").clicked().then(|| self.mode = Mode::Map);
                ui::tab(ui, self.mode == Mode::Events, ph::GRAPH, "Events").on_hover_text("The open map's events as a graph: triggers, dialogs and choices, the scripts they run, events, flags and variables, where they lead").clicked().then(|| self.mode = Mode::Events);
                ui::tab(ui, self.mode == Mode::Database, ph::DATABASE, "Database").on_hover_text("Definitions: items, props, characters, tilesets, sprite sheets, sounds, structures, quests, project settings, scripts").clicked().then(|| self.mode = Mode::Database);
                let px_label = if self.px.is_dirty() { "Pixel editor •" } else { "Pixel editor" };
                ui::tab(ui, self.mode == Mode::Pixel, ph::IMAGE, px_label).on_hover_text("Draw and edit the project's images").clicked().then(|| self.mode = Mode::Pixel);
                if ui.button(ui::t(ph::BOOKS, &format!("Asset Library {}", ph::ARROW_SQUARE_OUT))).on_hover_text("Opens the shared asset library in its own window: edit, create, import and export library assets").clicked() {
                    self.open_library_window();
                }
                ui.separator();
                self.map_dropdown(ui);
                if self.defs_dirty {
                    ui.colored_label(Color32::YELLOW, "defs •");
                }
            });
        });
        if self.mode == Mode::Map {
            self.ribbon(ctx);
        }
    }

    /// Map mode: the tool ribbon under the menu (grouped icon buttons with tooltips).
    fn ribbon(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("ribbon").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let (u, r) = self.doc.as_ref().map_or((false, false), |d| (d.can_undo(), d.can_redo()));
                ui::group(ui, "Edit", |ui| {
                    if ui::icon_button(ui, u, ph::ARROW_ARC_LEFT, "Undo", "Ctrl+Z", "").clicked() {
                        if let Some(d) = &mut self.doc {
                            d.undo(&self.assets);
                        }
                    }
                    if ui::icon_button(ui, r, ph::ARROW_ARC_RIGHT, "Redo", "Ctrl+Y", "").clicked() {
                        if let Some(d) = &mut self.doc {
                            d.redo(&self.assets);
                        }
                    }
                });
                let groups: [(&str, &[Tool]); 4] = [
                    ("Select", &[Tool::Select]),
                    ("Paint", &[Tool::Brush, Tool::Rect, Tool::Fill, Tool::Erase, Tool::Pick, Tool::Terrain]),
                    ("Collision", &[Tool::Collision]),
                    ("Objects", &[Tool::Entity, Tool::Zone, Tool::Link, Tool::Structure]),
                ];
                for (caption, tools) in groups {
                    ui::group(ui, caption, |ui| {
                        for &t in tools {
                            if ui::tool_button(ui, self.tool == t, t.icon(), t.name(), t.key(), t.help()).clicked() {
                                self.tool = t;
                            }
                        }
                    });
                }
                ui::group(ui, "Layer", |ui| {
                    let names: Vec<String> = self.doc.as_ref().map_or(Vec::new(), |d| d.map.layers.iter().map(|l| l.name.clone()).collect());
                    let current = names.get(self.layer).cloned().unwrap_or_else(|| "–".into());
                    egui::ComboBox::from_id_salt("ribbon_layer").selected_text(format!("{}. {current}", self.layer + 1)).width(130.0).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut self.layer, i, format!("{}. {n}", i + 1));
                        }
                    })
                    .response
                    .on_hover_text("The layer paint tools work on (keys 1–9). Visibility, order and new layers: Layers in the left panel");
                });
                ui::group(ui, "View", |ui| {
                    ui::toggle(ui, &mut self.view.grid, ph::GRID_FOUR, "Grid", "G", "Tile grid over the map");
                    ui::toggle(ui, &mut self.view.collision, ph::WALL, "Collision overlay", "", "Red = blocked, green = walkable override");
                    ui::toggle(ui, &mut self.view.zones, ph::BOUNDING_BOX, "Zones & edges", "", "Zone rectangles, labels and edge links");
                    ui::toggle(ui, &mut self.view.dim_layers, ph::STACK, "Dim inactive layers", "", "Fades every layer except the active one");
                    ui::toggle(ui, &mut self.view.minimap, ph::MAP_TRIFOLD, "Minimap", "", "Overview in the corner of the map");
                    ui::toggle(ui, &mut self.view.lighting, ph::LIGHTBULB, "Lighting preview", "", "Shows the map's darkness and its lights as in the game (outdoor maps at the hour chosen next to it)");
                    ui::toggle(ui, &mut self.view.weather, ph::CLOUD_RAIN, "Weather preview", "", "Shows the map's rain, snow or fog as in the game (set it in the map's settings: nothing selected)");
                    ui::toggle(ui, &mut self.view.parallax, ph::IMAGES, "Parallax layers", "", "Shows the map's parallax layers (sky, mist, cloud shadows) as in the game; set them in the map's settings (nothing selected)");
                    let outdoor = self.doc.as_ref().is_some_and(|d| d.map.lighting.mode == rustpg2d_engine::map::LightMode::Outdoor);
                    if self.view.lighting && outdoor {
                        let mut h = self.view.hour;
                        let label = format!("{:02}:{:02}", h as u32, (h.fract() * 60.0) as u32);
                        if ui.add(egui::Slider::new(&mut h, 0.0..=23.99).show_value(false)).on_hover_text(format!("Preview time of day: {label}")).changed() {
                            self.view.hour = h;
                        }
                        ui.label(egui::RichText::new(label).small());
                    }
                    if ui::icon_button(ui, self.doc.is_some(), ph::CORNERS_OUT, "Fit map to view", "Home", "").clicked() {
                        self.fit_view();
                    }
                });
                ui::group(ui, "Play", |ui| {
                    if ui::icon_button(ui, self.doc.is_some(), ph::PLAY, "Play here", "F5", "Saves everything and starts the game at the camera position (or the selected spawn point)").clicked() {
                        self.play(true);
                    }
                    if ui::icon_button(ui, self.doc.is_some(), ph::FAST_FORWARD, "Play from start", "F6", "Saves everything and starts the game at the project's start map").clicked() {
                        self.play(false);
                    }
                });
            });
            ui.add_space(2.0);
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let (Some(doc), Some((x, y))) = (&self.doc, self.hover_tile) {
                    ui.monospace(format!("tile {x},{y}"));
                    if doc.map.in_bounds(x, y) {
                        let t = doc.map.layers.get(self.layer).map(|l| l.get(x as u32, y as u32));
                        if let Some((slot, i)) = t.and_then(|t| t.parts()) {
                            ui.monospace(format!("[{}#{}]", doc.map.tilesets.get(slot).map_or("?", |s| s.as_str()), i));
                        }
                    }
                    ui.separator();
                }
                if !self.library_mode && self.project_open {
                    ui.monospace(format!("zoom {:.0}%", self.cam.zoom * 100.0));
                }
                if let Some(doc) = &self.doc {
                    ui.separator();
                    ui.monospace(format!("{}x{}  ents {}  zones {}", doc.map.width, doc.map.height, doc.map.entities.len(), doc.map.zones.len()));
                }
                ui.separator();
                ui.monospace(format!("{} fps", macroquad::time::get_fps()));
                if self.mode == Mode::Map && self.project_open && !self.library_mode {
                    ui.separator();
                    ui.label(egui::RichText::new(format!("{} {}", self.tool.icon(), self.tool.name())).strong());
                    ui.weak(ui::keys(self.tool.help()));
                }
                if self.file_dialog.is_some() {
                    ui.separator();
                    ui.colored_label(Color32::YELLOW, "File dialog open…");
                }
                if get_time() - self.status.1 < 6.0 {
                    ui.separator();
                    ui.label(&self.status.0);
                }
            });
        });
    }

    /// Applies the answer to the "Unsaved changes" question for `p`.
    pub fn settle_pending(&mut self, p: Pending, choice: Choice) {
        match choice {
            Choice::None => self.pending = Some(p),
            Choice::Cancel => {}
            Choice::Save | Choice::Discard => {
                if choice == Choice::Save {
                    self.save_all();
                } else {
                    if let Some(d) = &mut self.doc {
                        d.mark_saved();
                    }
                    self.defs_dirty = false;
                    self.px.doc = None;
                }
                if self.unsaved_for(&p) {
                    // Saving failed: stay (the status bar shows the error).
                    self.set_status("Some changes could not be saved; nothing was closed");
                } else {
                    self.run_pending(p);
                }
            }
        }
    }

    /// Whether anything `p` would throw away is still unsaved.
    fn unsaved_for(&self, p: &Pending) -> bool {
        let leaving_project = matches!(p, Pending::Quit | Pending::OpenProject(_) | Pending::NewProject(_) | Pending::CloseProject);
        self.doc.as_ref().is_some_and(|d| d.is_dirty()) || (leaving_project && (self.defs_dirty || self.px.is_dirty()))
    }

    pub fn request(&mut self, p: Pending) {
        if self.unsaved_for(&p) {
            self.pending = Some(p);
        } else {
            self.run_pending(p);
        }
    }

    fn run_pending(&mut self, p: Pending) {
        match p {
            Pending::OpenProject(dir) => self.switch_project(dir),
            // Unsaved work is settled at this point; now the wizard asks for name, place and settings.
            Pending::NewProject(dir) if dir.as_os_str().is_empty() => self.open_new_project_wizard(),
            Pending::NewProject(dir) => match files::create_project(&dir) {
                Ok(()) => {
                    // Opening replaces the whole editor state, closing the old project.
                    self.close_project();
                    self.switch_project(dir);
                    self.set_status("New project created. Add art with Assets \u{E13A} Add from library (or Database \u{E13A} Tilesets), then paint a map.");
                }
                Err(e) => self.set_status(format!("Could not create project: {e:#}")),
            },
            Pending::Open(m) => self.open_map(&m),
            Pending::OpenAt(m, zone) => {
                self.open_map(&m);
                if !zone.is_empty() {
                    self.focus_zone(&zone);
                }
            }
            Pending::New => {
                self.new_map = Some(NewMapDialog { name: "new_map".into(), width: 64, height: 64, fill: true });
            }
            Pending::Quit => self.quit = true,
            Pending::CloseProject => {
                self.close_project();
                self.set_status("Project closed");
            }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.pending.take() {
            let mut choice = Choice::None;
            let mut what = Vec::new();
            if let Some(d) = self.doc.as_ref().filter(|d| d.is_dirty()) {
                what.push(format!("map '{}'", d.map.name));
            }
            if self.defs_dirty {
                what.push("definitions".to_string());
            }
            if let Some(d) = self.px.doc.as_ref().filter(|d| d.is_dirty()) {
                what.push(format!("image {}", d.rel));
            }
            egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(format!("Unsaved changes: {}.", what.join(", ")));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Choice::Save;
                    }
                    if ui.button("Discard").clicked() {
                        choice = Choice::Discard;
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Choice::Cancel;
                    }
                });
            });
            match choice {
                Choice::None => self.pending = Some(p),
                c => self.settle_pending(p, c),
            }
        }

        if let Some(mut d) = self.new_map.take() {
            let mut open = true;
            let mut create = false;
            egui::Window::new("New map").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                egui::Grid::new("newmap").show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut d.name);
                    ui.end_row();
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut d.width).range(1..=MAX_MAP_SIZE));
                    ui.end_row();
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut d.height).range(1..=MAX_MAP_SIZE));
                    ui.end_row();
                });
                ui.checkbox(&mut d.fill, "Fill ground with the current brush tile");
                let valid = valid_name(&d.name);
                let exists = self.maps.contains(&d.name);
                if !valid {
                    ui.colored_label(Color32::LIGHT_RED, "Use letters, digits, '_' or '-'");
                } else if exists {
                    ui.colored_label(Color32::LIGHT_RED, "A map with this name exists");
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(valid && !exists, egui::Button::new("Create")).clicked() {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
                        open = false;
                    }
                });
            });
            if create {
                let mut map = Map::new(&d.name, d.width, d.height);
                self.doc = None;
                if d.fill {
                    if let Some(ts) = self.assets.tilesets.get(&self.brush.tileset) {
                        let slot = map.tileset_slot(&self.brush.tileset);
                        let t = Tile::new(slot, self.brush.y * ts.columns + self.brush.x);
                        for y in 0..map.height {
                            for x in 0..map.width {
                                map.layers[0].set(x, y, t);
                            }
                        }
                    }
                }
                self.load_doc(map);
                self.save_map();
            } else if open {
                self.new_map = Some(d);
            }
        }

        if let Some((mut w, mut h)) = self.resize.take() {
            let mut open = true;
            let mut apply = false;
            egui::Window::new("Resize map").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut w).range(1..=MAX_MAP_SIZE));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut h).range(1..=MAX_MAP_SIZE));
                });
                ui.weak("The top-left area is kept. Undoable.");
                ui.horizontal(|ui| {
                    apply = ui.button("Apply").clicked();
                    open = !ui.button("Cancel").clicked();
                });
            });
            if apply {
                if let Some(doc) = &mut self.doc {
                    let before = doc.map.clone();
                    doc.map.resize(w, h);
                    doc.record_map(before, &self.assets);
                }
            } else if open {
                self.resize = Some((w, h));
            }
        }
    }

    // ------------------------------------------------------------ left panel

    /// Left panel: the active tool's options, then the map's layers.
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        self.tool_options(ui);
        ui.add_space(6.0);
        ui.separator();
        egui::CollapsingHeader::new(ui::t(ph::STACK, "Layers")).default_open(true).show(ui, |ui| self.layers_panel(ui));
    }

    /// Visibility, order, names and the active layer of the open map.
    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(doc) = &mut self.doc {
            let n = doc.map.layers.len();
            let mut layer_op: Option<(&str, usize)> = None;
            for i in 0..n {
                let l = &mut doc.map.layers[i];
                ui.horizontal(|ui| {
                    ui.checkbox(&mut l.visible, "").on_hover_text("Visible");
                    if ui.selectable_label(self.layer == i, format!("{}. {}", i + 1, l.name)).clicked() {
                        self.layer = i;
                    }
                    if l.above_entities {
                        ui.weak("overhead");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if n > 1 && ui.small_button(ph::TRASH).on_hover_text("Delete layer").clicked() {
                            layer_op = Some(("del", i));
                        }
                        if i + 1 < n && ui.small_button(ph::ARROW_DOWN).on_hover_text("Move down (drawn later)").clicked() {
                            layer_op = Some(("down", i));
                        }
                        if i > 0 && ui.small_button(ph::ARROW_UP).on_hover_text("Move up (drawn earlier)").clicked() {
                            layer_op = Some(("up", i));
                        }
                    });
                });
            }
            if let Some(l) = doc.map.layers.get_mut(self.layer) {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut l.name).desired_width(110.0));
                    ui.checkbox(&mut l.above_entities, "overhead").on_hover_text("Drawn above characters, never collides");
                });
            }
            if ui.button(ui::t(ph::PLUS, "Layer")).clicked() {
                layer_op = Some(("add", 0));
            }
            if let Some((op, i)) = layer_op {
                let before = doc.map.clone();
                match op {
                    "add" => {
                        let name = format!("Layer {}", n + 1);
                        doc.map.add_layer(&name, false);
                        self.layer = n;
                    }
                    "del" => {
                        doc.map.layers.remove(i);
                        self.layer = self.layer.min(doc.map.layers.len() - 1);
                    }
                    "up" => {
                        doc.map.layers.swap(i, i - 1);
                        self.layer = i - 1;
                    }
                    _ => {
                        doc.map.layers.swap(i, i + 1);
                        self.layer = i + 1;
                    }
                }
                doc.record_map(before, &self.assets);
            }
        }
    }

    /// The active tool's panel (palette, placement list, structures, help).
    fn tool_options(&mut self, ui: &mut egui::Ui) {
        match self.tool {
            Tool::Select => {
                ui.heading("Select");
                ui.label("Click an entity or zone to select it, drag to move it. Click again to cycle through objects on the same spot. Clicking empty space or right-click deselects.");
                ui.label("Drag on empty space (Shift+drag anywhere) to select a region of tiles: Ctrl+C copies it (all layers, collision, entities), Ctrl+X cuts it, Del clears it. Ctrl+V pastes: the copy follows the mouse, each click places it, Esc stops. The clipboard is kept when you open another map.");
                ui.weak("Nothing is painted with this tool. Pick a tile below (or press B / R / F) to paint; Esc returns here.");
                ui.separator();
                self.palette(ui);
            }
            Tool::Terrain => self.terrain_palette(ui),
            t if t.paints_tiles() => self.palette(ui),
            Tool::Collision => {
                ui.heading("Collision");
                ui.label("Left: force solid\nRight: force walkable\nShift+Left: reset to tileset default");
                ui.weak("Red = blocked, green = walkable override. Tileset tiles flagged solid block automatically (edit flags in Database \u{E13A} Tilesets).");
            }
            Tool::Entity => self.entity_palette(ui),
            Tool::Structure => self.structure_palette(ui),
            Tool::Link => {
                ui.heading("Transition");
                ui.label("Link two maps with a pair of transition zones, e.g. where a road leaves one map and enters the other:");
                ui.label("1. Drag over the exit (the road end).");
                ui.label("2. Choose the target map.");
                ui.label("3. On the target map, drag the other end.");
                ui.weak("Each end leads to the other; players arrive one tile inside the other end (so they don't bounce back). Transition zones can also be drawn with the Zones tool and linked in the inspector (Leads to map / Other end, \u{E3B4} Draw the other end, \u{E5DE} Go to, Unlink). Portals and spawn points still work as before.");
            }
            Tool::Zone => {
                ui.heading("Zones");
                ui.label("Drag on the map to create:");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Spawn, "Spawn point");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Portal, "Portal (teleport on enter)");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Trigger, "Trigger area (events)");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Spawner, "Enemy spawner (invisible)");
                ui.radio_value(&mut self.zone_template, ZoneTemplate::Transition, "Map transition (link it to another map in the inspector)");
                ui.weak("Click a zone to select it, drag to move. Map-edge transitions are set in the map properties (right panel, nothing selected).");
            }
            _ => {}
        }
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Tileset");
            self.add_from_library_button(ui, Some(rustpg2d_engine::refactor::RefKind::Tileset));
        });
        let mut ids: Vec<String> = self.project.db.tilesets.iter().map(|t| t.id.clone()).collect();
        ids.sort();
        egui::ComboBox::from_id_salt("brush_ts").selected_text(self.brush.tileset.as_str()).show_ui(ui, |ui| {
            for id in &ids {
                if ui.selectable_label(*id == self.brush.tileset, id).clicked() {
                    self.brush = Brush { tileset: id.clone(), x: 0, y: 0, w: 1, h: 1 };
                }
            }
        });
        let (Some(tex), Some(ts)) = (self.tex.tilesets.get(&self.brush.tileset), self.assets.tilesets.get(&self.brush.tileset)) else {
            ui.label("No tileset");
            return;
        };
        ui.weak("Click a tile, or drag to select a multi-tile stamp. Right-click on the map picks a tile.");
        let tsz = self.assets.tile_size;
        let scale = ((ui.available_width() - 8.0) / ts.image.width as f32).clamp(1.0, 3.0).floor();
        let size = vec2(ts.image.width as f32 * scale, ts.image.height as f32 * scale);
        let cell = tsz * scale;
        let rows = ts.count.div_ceil(ts.columns);
        egui::ScrollArea::both().id_salt("palette").max_height(420.0).show(ui, |ui| {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 0.0, Color32::from_gray(30));
            p.image(tex.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            let cell_at = |pos: egui::Pos2| -> (u32, u32) {
                let rel = pos - rect.min;
                (((rel.x / cell).floor().max(0.0) as u32).min(ts.columns - 1), ((rel.y / cell).floor().max(0.0) as u32).min(rows - 1))
            };
            if let Some(pos) = resp.interact_pointer_pos() {
                let (cx, cy) = cell_at(pos);
                if (resp.drag_started() || resp.clicked() || self.palette_drag.is_none())
                    && (resp.drag_started() || resp.clicked()) {
                        self.palette_drag = Some((cx, cy));
                    }
                if let Some((sx, sy)) = self.palette_drag {
                    self.brush.x = sx.min(cx);
                    self.brush.y = sy.min(cy);
                    self.brush.w = sx.abs_diff(cx) + 1;
                    self.brush.h = sy.abs_diff(cy) + 1;
                    // Picking a tile is the explicit way to start painting.
                    if matches!(self.tool, Tool::Erase | Tool::Pick | Tool::Select) {
                        self.tool = Tool::Brush;
                    }
                }
            }
            if resp.drag_stopped() || resp.clicked() {
                self.palette_drag = None;
            }
            for &s in &self.project.db.tileset(&self.brush.tileset).map(|d| d.solid.clone()).unwrap_or_default() {
                let r = egui::Rect::from_min_size(rect.min + vec2((s % ts.columns) as f32 * cell, (s / ts.columns) as f32 * cell), vec2(cell, cell));
                p.line_segment([r.right_top() + vec2(-6.0, 1.0), r.right_top() + vec2(-1.0, 1.0)], Stroke::new(2.0_f32, Color32::RED));
            }
            let sel = egui::Rect::from_min_size(
                rect.min + vec2(self.brush.x as f32 * cell, self.brush.y as f32 * cell),
                vec2(self.brush.w as f32 * cell, self.brush.h as f32 * cell),
            );
            p.rect_stroke(sel, 0.0, Stroke::new(2.0_f32, Color32::YELLOW), StrokeKind::Inside);
        });
        ui.label(format!("Stamp {}x{} at ({}, {})", self.brush.w, self.brush.h, self.brush.x, self.brush.y));
    }

    fn entity_palette(&mut self, ui: &mut egui::Ui) {
        // What a click on empty space does.
        ui.horizontal(|ui| {
            ui.heading("Place");
            let kind = self.place_tab.map(|k| match k {
                EntityKind::Prop => rustpg2d_engine::refactor::RefKind::Prop,
                EntityKind::Character => rustpg2d_engine::refactor::RefKind::Character,
                EntityKind::Item => rustpg2d_engine::refactor::RefKind::Item,
            });
            self.add_from_library_button(ui, kind);
            if !self.place_def.is_empty() && ui.button("\u{E4F6} Stop placing").on_hover_text("Right-click or Esc on the map").clicked() {
                self.disarm();
            }
        });
        let armed = self.def_sprite(self.place_kind, &self.place_def).zip(self.def_name(self.place_kind, &self.place_def));
        ui.horizontal(|ui| match armed {
            Some((sprite, name)) => {
                widgets::sprite_thumb(ui, &self.tex, &self.assets, &sprite, 28.0, true);
                ui.label(egui::RichText::new(format!("{name}  ({})", self.place_kind.label().to_lowercase())).strong());
            }
            None => {
                ui.weak("Nothing: clicks select / move. Pick a definition below.");
            }
        });
        ui.weak("Click empty space to place (Shift = no snapping) · right-click / Esc: stop placing · [ ] or PgUp / PgDn: previous / next · click to select, drag to move, Del deletes, Ctrl+D duplicates.");

        // Recently used definitions: one click to switch back.
        if !self.place_recent.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Recent");
                for (k, id) in self.place_recent.clone() {
                    let Some(sprite) = self.def_sprite(k, &id) else { continue };
                    let on = self.place_kind == k && self.place_def == id;
                    let name = self.def_name(k, &id).unwrap_or_else(|| id.clone());
                    if widgets::sprite_thumb(ui, &self.tex, &self.assets, &sprite, 32.0, on).on_hover_text(&name).clicked() {
                        if on { self.disarm() } else { self.arm(k, &id) }
                    }
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("\u{E30C}");
            let r = ui.add(egui::TextEdit::singleline(&mut self.place_filter).hint_text("search name or id").desired_width(150.0));
            if r.changed() {
                self.palette_scroll_to_armed = false;
            }
            if !self.place_filter.is_empty() && ui.small_button("\u{E4F6}").on_hover_text("Clear search").clicked() {
                self.place_filter.clear();
            }
        });
        ui.horizontal(|ui| {
            if ui.selectable_label(self.place_tab.is_none(), "All").clicked() {
                self.place_tab = None;
            }
            for k in EntityKind::ALL {
                if ui.selectable_label(self.place_tab == Some(k), k.label()).clicked() {
                    self.place_tab = Some(k);
                }
            }
        });
        let entries = self.palette_entries();
        let mut clicked: Option<(EntityKind, String)> = None;
        egui::ScrollArea::vertical().id_salt("place_list").max_height(330.0).auto_shrink([false, true]).show(ui, |ui| {
            if entries.is_empty() {
                ui.weak("No matching definitions.");
            }
            let mut last_kind = None;
            for (k, id, name, sprite) in &entries {
                if self.place_tab.is_none() && last_kind != Some(*k) {
                    ui.label(egui::RichText::new(format!("{}s", k.label())).strong());
                    last_kind = Some(*k);
                }
                let on = self.place_kind == *k && self.place_def == *id;
                let row = ui.horizontal(|ui| {
                    let a = widgets::sprite_thumb(ui, &self.tex, &self.assets, sprite, 32.0, on);
                    let b = ui.selectable_label(on, format!("{name}  ·  {id}"));
                    a.clicked() || b.clicked()
                });
                if row.inner {
                    clicked = Some((*k, id.clone()));
                }
                if on && self.palette_scroll_to_armed {
                    row.response.scroll_to_me(None);
                }
            }
        });
        self.palette_scroll_to_armed = false;
        if let Some((k, id)) = clicked {
            // Clicking the armed definition again stops placing.
            if self.place_kind == k && self.place_def == id { self.disarm() } else { self.arm(k, &id) }
        }
        if ui.button("Edit definitions…").clicked() {
            self.mode = Mode::Database;
        }

        // Entities already on the map: click to select and jump there.
        ui.separator();
        let q = self.place_filter.trim().to_lowercase();
        let Some(doc) = &self.doc else { return };
        let list: Vec<(u32, String, f32, f32)> = doc
            .map
            .entities
            .iter()
            .filter(|e| self.place_tab.is_none_or(|t| t == e.kind))
            .filter(|e| q.is_empty() || e.def.to_lowercase().contains(&q) || e.name.to_lowercase().contains(&q))
            .map(|e| (e.uid, e.label(), e.x, e.y))
            .collect();
        let total = doc.map.entities.len();
        let mut select = None;
        egui::CollapsingHeader::new(format!("On this map ({} of {total})", list.len())).id_salt("on_map").default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("on_map_list").max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                for (uid, label, x, y) in &list {
                    let on = self.sel == Selection::Entity(*uid);
                    let r = ui.selectable_label(on, label);
                    if r.clicked() {
                        select = Some((*uid, *x, *y));
                    }
                    if on && self.on_map_scroll {
                        r.scroll_to_me(None);
                    }
                }
            });
        });
        self.on_map_scroll = false;
        if let Some((uid, x, y)) = select {
            self.sel = Selection::Entity(uid);
            self.cam.center = macroquad::math::vec2(x, y);
        }
    }

    /// Display name of a definition.
    pub fn def_name(&self, kind: EntityKind, def: &str) -> Option<String> {
        let db = &self.project.db;
        match kind {
            EntityKind::Prop => db.prop(def).map(|d| d.name.clone()),
            EntityKind::Character => db.character(def).map(|d| d.name.clone()),
            EntityKind::Item => db.item(def).map(|d| d.name.clone()),
        }
    }

    // ------------------------------------------------------------ inspector

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = &mut self.doc else {
            ui.label("No map open. Use Map \u{E13A} New map.");
            return;
        };
        // Speakers follow the map while it is edited (new characters, renamed entities, ...).
        self.refs.speakers = speakers_of(&doc.map, &self.project.db);
        let before = Objects::of(&doc.map);
        // For schedule stops: the project's maps, this map, its furniture (named props with a use spot).
        let maps_list = self.maps.clone();
        let item_ids: Vec<String> = self.project.db.items.iter().map(|d| d.id.clone()).collect();
        let map_name = doc.map.name.clone();
        let prop_names: Vec<String> = doc.map.entities.iter().filter(|e| e.kind == EntityKind::Prop && !e.name.is_empty() && self.project.db.prop(&e.def).is_some_and(|d| d.use_spot.is_some())).map(|e| e.name.clone()).collect();
        let ts = self.assets.tile_size;
        let refs = &self.refs;
        let mut delete = false;
        let mut duplicate = false;
        let mut customize: Option<u32> = None;
        let mut arm: Option<(EntityKind, String)> = None;
        let mut rename_map: Option<(String, String)> = None;
        let mut region_req: Option<crate::clipboard::RegionReq> = None;
        match self.sel {
            Selection::Entity(uid) => {
                let db = &self.project.db;
                let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) else {
                    self.sel = Selection::None;
                    return;
                };
                ui.horizontal(|ui| {
                    ui.heading(format!("{} #{}", e.kind.label(), e.uid));
                    if ui.button("Place more like this").on_hover_text("Use this definition for placing (Entities tool)").clicked() {
                        arm = Some((e.kind, e.def.clone()));
                    }
                });
                egui::Grid::new("ent").num_columns(2).show(ui, |ui| {
                    ui.label("Definition");
                    egui::ComboBox::from_id_salt("ent_def").selected_text(e.def.as_str()).show_ui(ui, |ui| {
                        for id in db.ids(e.kind) {
                            ui.selectable_value(&mut e.def, id.clone(), id);
                        }
                    });
                    ui.end_row();
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut e.name).hint_text("for actions/scripts"));
                    ui.end_row();
                    ui.label("Position");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut e.x).speed(1.0));
                        ui.add(egui::DragValue::new(&mut e.y).speed(1.0));
                        ui.weak(format!("tile {},{}", (e.x / ts).floor(), (e.y / ts).floor()));
                    });
                    ui.end_row();
                    ui.label("Facing");
                    egui::ComboBox::from_id_salt("ent_face").selected_text(e.facing.label()).show_ui(ui, |ui| {
                        for f in Facing::ALL {
                            ui.selectable_value(&mut e.facing, f, f.label());
                        }
                    });
                    ui.end_row();
                    ui.label("Active");
                    ui.checkbox(&mut e.active, "visible at start");
                    ui.end_row();
                    if let Some(d) = (e.kind == EntityKind::Prop).then(|| db.prop(&e.def)).flatten() {
                        ui.label("State");
                        ui.horizontal(|ui| {
                            let cur = if e.state.is_empty() { d.default_state.clone() } else { e.state.clone() };
                            egui::ComboBox::from_id_salt("ent_state").selected_text(&cur).show_ui(ui, |ui| {
                                for (k, n) in d.state_names().into_iter().enumerate() {
                                    let label = if k == 0 { format!("{n} (default)") } else { n.clone() };
                                    if ui.selectable_label(n == cur, label).clicked() {
                                        e.state = if k == 0 { String::new() } else { n };
                                        e.frame = 0;
                                    }
                                }
                            });
                            if d.state_index(&cur).is_none() {
                                ui.colored_label(Color32::LIGHT_RED, "unknown state");
                            }
                        });
                        ui.end_row();
                        if e.frame > 0 {
                            ui.label("Frame offset");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut e.frame).range(0..=255));
                                ui.weak("(old; choose a state instead)");
                            });
                            ui.end_row();
                        }
                    }
                    if e.kind == EntityKind::Item {
                        ui.label("Count");
                        ui.add(egui::DragValue::new(&mut e.count).range(1..=9999));
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    duplicate = ui.button("Duplicate").clicked();
                    delete = ui.button("Delete").clicked();
                });
                {
                    ui.separator();
                    let def_light = match e.kind {
                        EntityKind::Prop => db.prop(&e.def).and_then(|d| d.state_index(&e.state).and_then(|i| d.state_light(i)).cloned()),
                        EntityKind::Character => db.character(&e.def).and_then(|d| d.light.clone()),
                        EntityKind::Item => db.item(&e.def).and_then(|d| d.light.clone()),
                    };
                    let what = if def_light.is_some() { "Own light (instead of the definition's)" } else { "Gives light (just this one)" };
                    egui::CollapsingHeader::new(ui::t(ph::LIGHTBULB, "Light")).default_open(e.light.is_some()).show(ui, |ui| {
                        if def_light.is_some() && e.light.is_none() {
                            ui.weak("Uses the definition's light.");
                        }
                        widgets::light_ui(ui, &format!("ent_light{}", e.uid), &mut e.light, what);
                    });
                }
                if let Some(d) = (e.kind == EntityKind::Character).then(|| db.character(&e.def)).flatten() {
                    // Look and items of this NPC: its definition's, or its own (set in the builder).
                    ui.separator();
                    let own = e.appearance.is_some() || e.equipment.is_some() || e.inventory.is_some();
                    egui::CollapsingHeader::new(ui::t(ph::USER, "Look & equipment")).default_open(true).show(ui, |ui| {
                        use rustpg2d_engine::render::character;
                        widgets::character_preview(ui, &self.tex, &self.assets, db, character::appearance_of(d, Some(e)), character::equipment_of(d, Some(e)));
                        let mut what = Vec::new();
                        if e.appearance.is_some() {
                            what.push("look");
                        }
                        if e.equipment.is_some() {
                            what.push("equipment");
                        }
                        if e.inventory.is_some() {
                            what.push("carried items");
                        }
                        if own {
                            ui.label(format!("Own {} (the rest is the definition's).", what.join(", ")));
                        } else {
                            ui.weak("The definition's look, equipment and carried items.");
                        }
                        ui.horizontal(|ui| {
                            if ui.button(ui::t(ph::USER, "Customize this one…")).on_hover_text("Give just this placed character its own body, colors, hair, clothes, gear and carried items").clicked() {
                                customize = Some(e.uid);
                            }
                            if own && ui.button("Use the definition's").on_hover_text("Drop this one's own look, equipment and carried items").clicked() {
                                e.appearance = None;
                                e.equipment = None;
                                e.inventory = None;
                            }
                        });
                    });
                    // A drawn portrait just for this one (dialogs, the companion HUD).
                    egui::CollapsingHeader::new(ui::t(ph::USER_SQUARE, "Portrait")).default_open(e.portrait.is_some()).show(ui, |ui| {
                        match &mut e.portrait {
                            Some(p) => {
                                widgets::sprite_field(ui, &format!("ent_portrait{}", e.uid), p, &self.tex, &self.assets, db, false);
                                if ui.button("Use the definition's").clicked() {
                                    e.portrait = None;
                                }
                            }
                            None => {
                                ui.weak(if d.portrait.is_empty() { "The head and shoulders of its look (the definition has no drawn portrait)." } else { "The definition's portrait." });
                                if ui.button("Own portrait…").on_hover_text("Give just this placed character its own drawn portrait").clicked() {
                                    e.portrait = Some(d.portrait.clone());
                                    widgets::request_chooser(&format!("ent_portrait{}", e.uid));
                                }
                            }
                        }
                    });
                }
                if e.kind == EntityKind::Character {
                    ui.separator();
                    ui.heading("Merchant");
                    let def_shop = db.character(&e.def).and_then(|d| d.shop.clone());
                    let mut set: Option<Option<ShopSpec>> = None;
                    match &mut e.shop {
                        Some(spec) => {
                            widgets::shop_ui(ui, &format!("shop{uid}"), spec, refs, &self.tex, &self.assets, db);
                            let label = if def_shop.is_some() { "Reset to the definition's stock" } else { "Not a merchant" };
                            if ui.button(label).clicked() {
                                set = Some(None);
                            }
                        }
                        None => match def_shop {
                            Some(d) => {
                                ui.weak(format!("Uses the definition's stock: {} item(s).", d.items.len()));
                                if ui.button("Customize stock & prices").clicked() {
                                    set = Some(Some(d));
                                }
                            }
                            None => {
                                ui.weak("Not a merchant.");
                                if ui.button("Make this a merchant").clicked() {
                                    set = Some(Some(ShopSpec::default()));
                                }
                            }
                        },
                    }
                    if let Some(v) = set {
                        e.shop = v;
                    }

                    // Loot of this character (its own table, or the definition's).
                    if db.character(&e.def).is_some_and(|d| d.max_hp > 0) {
                        ui.separator();
                        ui.heading("Loot");
                        let def_loot = db.character(&e.def).map(|d| d.loot.clone()).unwrap_or_default();
                        let mut set: Option<Option<Vec<rustpg2d_engine::defs::LootEntry>>> = None;
                        match &mut e.loot {
                            Some(table) => {
                                widgets::loot_ui(ui, &format!("loot{uid}"), table, refs, &self.tex, &self.assets, db);
                                if ui.button("Use the definition's loot").clicked() {
                                    set = Some(None);
                                }
                            }
                            None => {
                                ui.weak(loot_summary(&def_loot, db, "Uses the definition's loot"));
                                if ui.button("Customize loot").on_hover_text("Give this character its own loot table").clicked() {
                                    set = Some(Some(def_loot.clone()));
                                }
                            }
                        }
                        if let Some(v) = set {
                            e.loot = v;
                        }
                    }

                    ui.separator();
                    ui.heading("Patrol route");
                    let tile = ((e.x / ts).floor() as i32, ((e.y - 1.0) / ts).floor() as i32);
                    let mut remove_route = false;
                    match &mut e.patrol {
                        None => {
                            ui.weak("No route: the character follows its definition's behaviour.");
                            if ui.button("\u{E3D4} Add a patrol route").on_hover_text("Then click tiles on the map to add waypoints").clicked() {
                                e.patrol = Some(PatrolRoute { points: vec![tile], ..Default::default() });
                                self.route_edit = Some(uid);
                                self.route_kind = RouteKind::Patrol;
                            }
                        }
                        Some(r) => {
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut r.mode, PatrolMode::Loop, PatrolMode::Loop.label());
                                ui.radio_value(&mut r.mode, PatrolMode::PingPong, PatrolMode::PingPong.label());
                            });
                            ui.horizontal(|ui| {
                                ui.label("Pause");
                                ui.add(egui::DragValue::new(&mut r.pause).range(0.0..=30.0).speed(0.1).suffix(" s"));
                                ui.label("Speed");
                                ui.add(egui::DragValue::new(&mut r.speed).range(0.0..=400.0).speed(1.0)).on_hover_text("Pixels per second; 0 = the character's speed");
                            });
                            let editing = self.route_edit == Some(uid) && self.route_kind == RouteKind::Patrol;
                            ui.horizontal(|ui| {
                                let label = if editing { "\u{E182} Done editing" } else { "\u{E3B4} Edit on map" };
                                if ui.selectable_label(editing, label).on_hover_text("Click tiles to add waypoints, drag one to move it, right-click to remove it, Esc when done").clicked() {
                                    self.route_edit = if editing { None } else { Some(uid) };
                                    self.route_kind = RouteKind::Patrol;
                                }
                                remove_route = ui.button("Remove route").clicked();
                            });
                            let mut del = None;
                            for (k, p) in r.points.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.monospace(format!("{:>2}. tile {},{}", k + 1, p.0, p.1));
                                    if ui.small_button("\u{E4F6}").clicked() {
                                        del = Some(k);
                                    }
                                });
                            }
                            if let Some(k) = del {
                                r.points.remove(k);
                            }
                            if r.points.len() < 2 {
                                ui.colored_label(egui::Color32::YELLOW, "Add at least two waypoints.");
                            }
                            ui.weak("Talking to the character pauses it; the 'Patrol' action or patrol(target) / stop_patrol(target) in scripts start and stop it.");
                        }
                    }
                    if remove_route {
                        e.patrol = None;
                        self.route_edit = None;
                    }

                    ui.separator();
                    ui.heading("Activity");
                    ui.horizontal(|ui| {
                        crate::widgets::activity_ui(ui, &format!("entact{uid}"), &mut e.activity, &item_ids);
                    });
                    ui.weak("What it is seen doing where it stands (facing as placed), when it has no schedule; schedule stops have their own.");
                    ui.separator();
                    ui.heading("Daily schedule");
                    if e.schedule.is_empty() {
                        ui.weak("No schedule: the character stays around where it is placed.");
                        if ui.button(ui::t(ph::CLOCK, "Add a schedule")).on_hover_text("Then click tiles on the map to add the places it goes to during the day").clicked() {
                            e.schedule = vec![ScheduleStop::new(8.0, tile.0, tile.1)];
                            self.route_edit = Some(uid);
                            self.route_kind = RouteKind::Schedule;
                        }
                    } else {
                        let editing = self.route_edit == Some(uid) && self.route_kind == RouteKind::Schedule;
                        let mut remove = false;
                        ui.horizontal(|ui| {
                            let label = if editing { ui::t(ph::CHECK, "Done editing") } else { ui::t(ph::MAP_PIN, "Edit on map") };
                            if ui.selectable_label(editing, label).on_hover_text("Click tiles to add stops, drag one to move it, right-click to remove it, Esc when done").clicked() {
                                self.route_edit = if editing { None } else { Some(uid) };
                                self.route_kind = RouteKind::Schedule;
                            }
                            remove = ui.button("Remove schedule").clicked();
                        });
                        let mut del = None;
                        let n = e.schedule.len();
                        let all_maps = maps_list.clone();
                        let this_map = map_name.clone();
                        let props_here = prop_names.clone();
                        ui.vertical(|ui| {
                            for (k, s) in e.schedule.iter_mut().enumerate() {
                              ui.group(|ui| {
                               ui.horizontal(|ui| {
                                // The hour as a time: dragged in quarter hours, typed as 7.5 or 7:30.
                                ui.add(
                                    egui::DragValue::new(&mut s.hour)
                                        .range(0.0..=23.75)
                                        .speed(0.05)
                                        .custom_formatter(|h, _| rustpg2d_engine::entity::hour_text(h as f32))
                                        .custom_parser(|t| match t.split_once(':') {
                                            Some((h, m)) => Some(h.trim().parse::<f64>().ok()? + m.trim().parse::<f64>().ok()? / 60.0),
                                            None => t.trim().parse().ok(),
                                        }),
                                )
                                .on_hover_text("From this time on (in-game clock) the character goes here");
                                if ui.ctx().input(|i| !i.pointer.any_down()) {
                                    s.hour = (s.hour * 4.0).round() / 4.0;
                                }
                                // The stop's map: this one (tiles clicked on the map) or another (tiles typed).
                                let shown = if s.map.is_empty() { "this map".to_string() } else { s.map.clone() };
                                egui::ComboBox::from_id_salt(("stopmap", uid, k)).width(90.0).selected_text(shown).show_ui(ui, |ui| {
                                    ui.selectable_value(&mut s.map, String::new(), "this map");
                                    for m in all_maps.iter().filter(|m| **m != this_map) {
                                        ui.selectable_value(&mut s.map, m.clone(), m);
                                    }
                                })
                                .response
                                .on_hover_text("Another map: the character walks to this map's door / portal leading there and turns up on that map (its look, items and talks come along)");
                                if s.map.is_empty() {
                                    ui.monospace(format!("tile {},{}", s.x, s.y));
                                } else {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::DragValue::new(&mut s.x).range(0..=1023).prefix("x "));
                                        ui.add(egui::DragValue::new(&mut s.y).range(0..=1023).prefix("y "));
                                    });
                                }
                               });
                               ui.horizontal(|ui| {
                                let face = s.facing.map_or("Any", |f| f.label());
                                egui::ComboBox::from_id_salt(("stopface", uid, k)).width(70.0).selected_text(face).show_ui(ui, |ui| {
                                    ui.selectable_value(&mut s.facing, None, "Any");
                                    for f in Facing::ALL {
                                        ui.selectable_value(&mut s.facing, Some(f), f.label());
                                    }
                                })
                                .response
                                .on_hover_text("The direction it faces once there");
                                ui.checkbox(&mut s.inside, "inside").on_hover_text("It goes indoors here (e.g. at a door): out of sight until the next stop");
                                ui.label("on");
                                let names: Vec<String> = if s.map.is_empty() { props_here.clone() } else { Vec::new() };
                                crate::widgets::suggest(ui, &format!("stopprop{uid}_{k}"), &mut s.prop, &names, 90.0);
                               });
                               ui.horizontal(|ui| {
                                ui.label("doing");
                                crate::widgets::activity_ui(ui, &format!("stopact{uid}_{k}"), &mut s.activity, &item_ids);
                                if ui.add_enabled(n > 1, egui::Button::new(ph::TRASH).small()).on_hover_text("Remove this stop").clicked() {
                                    del = Some(k);
                                }
                               });
                              });
                            }
                        });
                        if let Some(k) = del {
                            e.schedule.remove(k);
                        }
                        if remove {
                            e.schedule.clear();
                            if self.route_kind == RouteKind::Schedule {
                                self.route_edit = None;
                            }
                        }
                        ui.weak("At each time the character walks to its stop and stays there (wandering around it if its behaviour is Wander). A stop can be on another map (it leaves through the door / portal leading there) and can name furniture (a prop's name with a use spot: a bed, a chair) it then sleeps or sits on. Entering a map puts it at the stop of the hour. Use the Time of day and Pose conditions for what it says at night or asleep. The Schedule action (scripts: stop_schedule / follow_schedule) stops and resumes it. A schedule replaces the patrol route.");
                    }
                }
                if e.kind == EntityKind::Prop {
                    ui.separator();
                    ui.heading("Container");
                    let def_spec = db.prop(&e.def).and_then(|d| d.container.clone());
                    let mut set: Option<Option<ContainerSpec>> = None;
                    match &mut e.container {
                        Some(spec) => {
                            let states = db.prop(&e.def).map(|d| d.state_names()).unwrap_or_default();
                            widgets::container_ui(ui, &format!("cont{uid}"), spec, refs, &self.tex, &self.assets, db, &states);
                            let label = if def_spec.is_some() { "Reset to the definition's container" } else { "Not a container" };
                            if ui.button(label).clicked() {
                                set = Some(None);
                            }
                        }
                        None => match def_spec {
                            Some(d) => {
                                ui.weak(format!("Uses the definition's container: {} slots, {} starting stack(s).", d.size, d.items.len()));
                                if ui.button("Customize size & contents").clicked() {
                                    set = Some(Some(d));
                                }
                            }
                            None => {
                                ui.weak("Not a container.");
                                if ui.button("Make this a container").clicked() {
                                    set = Some(Some(ContainerSpec::default()));
                                }
                            }
                        },
                    }
                    if let Some(v) = set {
                        e.container = v;
                    }

                    ui.separator();
                    ui.heading("Lock");
                    let def_lock = db.prop(&e.def).and_then(|d| d.lock.clone());
                    let def_prop = db.prop(&e.def).cloned();
                    let mut set_lock: Option<Option<LockSpec>> = None;
                    match &mut e.lock {
                        Some(lock) => {
                            widgets::lock_ui(ui, &format!("lock{uid}"), lock, refs, &self.tex, &self.assets, db, def_prop.as_ref());
                            let label = if def_lock.is_some() { "Use the definition's lock" } else { "Remove lock" };
                            if ui.button(label).clicked() {
                                set_lock = Some(None);
                            }
                        }
                        None => match def_lock {
                            Some(l) => {
                                let key = db.item(&l.key).map_or(l.key.clone(), |d| d.name.clone());
                                ui.weak(format!("Uses the definition's lock (key: {key})."));
                                if ui.button("Customize lock (other key, effect, messages)").clicked() {
                                    set_lock = Some(Some(l));
                                }
                            }
                            None => {
                                ui.weak("Not locked.");
                                if ui.button("Make this lockable").clicked() {
                                    let key = refs.items.iter().find(|i| i.contains("key")).cloned().unwrap_or_default();
                                    set_lock = Some(Some(LockSpec { key, ..Default::default() }));
                                }
                            }
                        },
                    }
                    if let Some(v) = set_lock {
                        e.lock = v;
                    }
                }
                ui.separator();
                let n_def = db.def_triggers(e.kind, &e.def).len();
                ui.heading("Triggers");
                if e.kind == EntityKind::Item {
                    ui.weak("Interacting picks the item up; triggers below run too.");
                }
                if n_def > 0 {
                    ui.weak(format!("+ {n_def} trigger(s) from the definition (Database)."));
                }
                let owner = match e.kind {
                    EntityKind::Character => rustpg2d_engine::events::TriggerOwner::Character,
                    EntityKind::Prop => rustpg2d_engine::events::TriggerOwner::Prop,
                    EntityKind::Item => rustpg2d_engine::events::TriggerOwner::Pickup,
                };
                widgets::triggers_ui(ui, &format!("e{uid}"), &mut e.triggers, refs, owner);
            }
            Selection::Zone(uid) => {
                let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) else {
                    self.sel = Selection::None;
                    return;
                };
                ui.heading(format!("{} zone #{}", z.kind.label(), z.uid));
                zone_fields(ui, z, refs, doc.map.width, doc.map.height);
                // Spawners pass their loot table on to the characters they spawn.
                if let ZoneKind::FishingSpot { .. } = &z.kind {
                    ui.separator();
                    ui.heading("Catch");
                    let table = z.loot.get_or_insert_with(Vec::new);
                    widgets::loot_ui(ui, &format!("zfish{uid}"), table, refs, &self.tex, &self.assets, &self.project.db);
                }
                if let ZoneKind::Spawner { character, .. } = &z.kind {
                    let db = &self.project.db;
                    let def_loot = db.character(character).map(|d| d.loot.clone()).unwrap_or_default();
                    ui.separator();
                    ui.heading("Loot of spawned characters");
                    let mut set: Option<Option<Vec<rustpg2d_engine::defs::LootEntry>>> = None;
                    match &mut z.loot {
                        Some(table) => {
                            widgets::loot_ui(ui, &format!("zloot{uid}"), table, refs, &self.tex, &self.assets, db);
                            if ui.button("Use the character's loot").clicked() {
                                set = Some(None);
                            }
                        }
                        None => {
                            ui.weak(loot_summary(&def_loot, db, &format!("Uses {character}'s loot")));
                            if ui.button("Customize loot").on_hover_text("Every character from this spawner drops from this table").clicked() {
                                set = Some(Some(def_loot.clone()));
                            }
                        }
                    }
                    if let Some(v) = set {
                        z.loot = v;
                    }
                }
                ui.horizontal(|ui| {
                    duplicate = ui.button("Duplicate").clicked();
                    delete = ui.button("Delete").clicked();
                });
                ui.separator();
                ui.heading("Triggers");
                ui.weak("Zones react to Enter, Leave and Interact (pressing E while inside).");
                widgets::triggers_ui(ui, &format!("z{uid}"), &mut z.triggers, refs, rustpg2d_engine::events::TriggerOwner::Zone);
            }
            Selection::Region(r) => region_req = crate::clipboard::region_inspector(ui, &doc.map, r, self.clipboard.as_ref()),
            Selection::None => {
                let map = &mut doc.map;
                ui.heading(format!("Map: {}", map.name));
                ui.horizontal(|ui| {
                    ui.label(format!("{} x {} tiles", map.width, map.height));
                    if ui.button("Resize…").clicked() {
                        self.resize = Some((map.width, map.height));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Background");
                    ui.color_edit_button_srgb(&mut map.background);
                });
                ui.horizontal(|ui| {
                    ui.label("Music");
                    widgets::suggest(ui, "map_music", &mut map.music, &refs.sounds, 130.0);
                    if !map.music.is_empty() && ui.small_button("\u{E3D0}").on_hover_text("Preview").clicked() {
                        self.audio.play_music(&map.music);
                    }
                    if self.audio.current_music().is_some() && ui.small_button("\u{E46C}").on_hover_text("Stop").clicked() {
                        self.audio.stop_music();
                    }
                });
                ui.horizontal(|ui| match &mut self.map_rename {
                    None => {
                        if ui.button("Rename map…").on_hover_text("Renames the file and updates portals, edge links, teleports and the project start").clicked() {
                            self.map_rename = Some(map.name.clone());
                        }
                    }
                    Some(buf) => {
                        ui.add(egui::TextEdit::singleline(buf).desired_width(120.0));
                        if ui.button("Apply").clicked() {
                            rename_map = Some((map.name.clone(), buf.clone()));
                        }
                        if ui.button("Cancel").clicked() {
                            self.map_rename = None;
                        }
                    }
                });
                ui.separator();
                ui.heading("Lighting");
                {
                    use rustpg2d_engine::map::LightMode;
                    let l = &mut map.lighting;
                    ui.horizontal_wrapped(|ui| {
                        ui.radio_value(&mut l.mode, LightMode::Bright, "Always bright").on_hover_text("No darkness; lights are not drawn");
                        ui.radio_value(&mut l.mode, LightMode::Outdoor, "Outdoor (day & night)").on_hover_text("Bright by day, darker at night (the game clock runs on)");
                        ui.radio_value(&mut l.mode, LightMode::Dark, "Dark (dungeon, cave)").on_hover_text("Always dark: only light sources show anything");
                    });
                    if l.mode != LightMode::Bright {
                        ui.horizontal(|ui| {
                            ui.label(if l.mode == LightMode::Outdoor { "Night darkness" } else { "Darkness" });
                            let mut pct = (l.darkness * 100.0).round();
                            if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %")).changed() {
                                l.darkness = pct / 100.0;
                            }
                        });
                        ui.weak("Lights: tiles (Database › Tilesets › Light tiles), props per state, characters and items (Database), and the player's equipped light. Preview: View › Lighting preview.");
                    }
                }
                ui.separator();
                ui.heading("Weather");
                {
                    use rustpg2d_engine::map::WeatherKind;
                    let w = &mut map.weather;
                    ui.horizontal_wrapped(|ui| {
                        for k in WeatherKind::ALL {
                            ui.radio_value(&mut w.kind, k, k.label());
                        }
                    });
                    if w.kind != WeatherKind::None {
                        ui.horizontal(|ui| {
                            ui.label("Strength");
                            let mut pct = (w.intensity * 100.0).round();
                            if ui.add(egui::Slider::new(&mut pct, 5.0..=100.0).suffix(" %")).changed() {
                                w.intensity = pct / 100.0;
                            }
                        });
                    }
                    ui.weak("Rain also darkens the map and plays a rain sound. The Set weather action and set_weather() change it during the game (kept in save games). Preview: View › Weather preview.");
                }
                ui.separator();
                parallax_ui(ui, &mut map.parallax, &self.project.db, &self.tex);
                ui.separator();
                ui.heading("Map screen");
                ui.checkbox(&mut map.fog, "Fog of war").on_hover_text(
                    "The in-game map (M) only shows what the player has explored: about 6 tiles around everywhere they have been. Off: the whole map is shown right away.",
                );
                ui.weak(if map.fog { "Players uncover this map by exploring it." } else { "The map screen shows this whole map." });
                ui.separator();
                ui.heading("Edge transitions");
                ui.weak("Walking into a linked edge moves the player to the target map, keeping the position along the edge (+offset).");
                for e in Edge::ALL {
                    let link = map.edges.get_mut(e);
                    ui.push_id(e.label(), |ui| {
                        ui.horizontal(|ui| {
                            let mut on = link.is_some();
                            if ui.checkbox(&mut on, e.label()).changed() {
                                *link = on.then(|| EdgeLink::new(refs.maps.first().map_or("", |s| s.as_str())));
                            }
                            if let Some(l) = link {
                                widgets::suggest(ui, "edge_map", &mut l.target_map, &refs.maps, 90.0);
                                if ui.small_button("\u{E5DE}").on_hover_text(format!("Go to {}", l.target_map)).clicked() {
                                    GOTO.with(|g| *g.borrow_mut() = Some((l.target_map.clone(), String::new())));
                                }
                            }
                        });
                        if let Some(l) = link {
                            ui.horizontal(|ui| {
                                ui.add_space(24.0);
                                ui.label("offset");
                                ui.add(egui::DragValue::new(&mut l.offset).range(-1024..=1024)).on_hover_text("Tiles added along the edge");
                                ui.label("depth");
                                ui.add(egui::DragValue::new(&mut l.depth).range(1..=16)).on_hover_text("Thickness of the transition strip in tiles");
                            });
                        }
                    });
                }
                ui.separator();
                ui.heading("Map triggers");
                ui.weak("Map enter and custom events.");
                widgets::triggers_ui(ui, "map", &mut map.triggers, refs, rustpg2d_engine::events::TriggerOwner::Map);
                ui.separator();
                ui.collapsing(format!("Entities ({})", map.entities.len()), |ui| {
                    for e in &map.entities {
                        if ui.selectable_label(false, e.label()).clicked() {
                            self.sel = Selection::Entity(e.uid);
                            self.cam.center = macroquad::math::vec2(e.x, e.y);
                        }
                    }
                });
                ui.collapsing(format!("Zones ({})", map.zones.len()), |ui| {
                    for z in &map.zones {
                        if ui.selectable_label(false, format!("#{} {} ({})", z.uid, z.name, z.kind.label())).clicked() {
                            self.sel = Selection::Zone(z.uid);
                            let r = z.pixel_rect(ts);
                            self.cam.center = macroquad::math::vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
                        }
                    }
                });
            }
        }
        doc.record_objects(before, "inspector");
        if let Some((old, new)) = rename_map {
            self.map_rename = None;
            if doc.is_dirty() {
                self.save_map();
            }
            self.rename_id(RefKind::Map, &old, &new);
        }
        if let Some(req) = region_req {
            self.region_request(req);
        }
        if duplicate {
            self.duplicate_selection();
        }
        if delete {
            self.delete_selection();
        }
        if let Some(uid) = customize {
            self.open_builder(crate::builder::Target::Instance(uid));
        }
        if let Some((k, def)) = arm {
            self.tool = Tool::Entity;
            self.arm(k, &def);
            self.palette_scroll_to_armed = true;
        }
    }
}

fn zone_fields(ui: &mut egui::Ui, z: &mut Zone, refs: &Refs, mw: u32, mh: u32) {
    egui::Grid::new("zone").num_columns(2).show(ui, |ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut z.name);
        ui.end_row();
        ui.label("Kind");
        let mut k = match z.kind {
            ZoneKind::Spawn => 0,
            ZoneKind::Portal { .. } => 1,
            ZoneKind::Trigger => 2,
            ZoneKind::Spawner { .. } => 3,
            ZoneKind::Transition { .. } => 4,
            ZoneKind::FishingSpot { .. } => 5,
        };
        let old = k;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut k, 0, "Spawn");
                ui.radio_value(&mut k, 1, "Portal");
                ui.radio_value(&mut k, 4, "Map transition");
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut k, 2, "Trigger");
                ui.radio_value(&mut k, 3, "Enemy spawner");
                ui.radio_value(&mut k, 5, "Fishing spot");
            });
        });
        if k != old {
            z.kind = match k {
                0 => ZoneKind::Spawn,
                1 => ZoneKind::Portal { target_map: refs.maps.first().cloned().unwrap_or_default(), target_spawn: String::new() },
                2 => ZoneKind::Trigger,
                4 => ZoneKind::Transition { target_map: String::new(), target_zone: String::new() },
                5 => ZoneKind::FishingSpot { rod: String::new() },
                _ => default_spawner(refs),
            };
        }
        ui.end_row();
        ui.label("Tile x / y");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut z.x).range(0..=mw - 1));
            ui.add(egui::DragValue::new(&mut z.y).range(0..=mh - 1));
        });
        ui.end_row();
        ui.label("Size w / h");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut z.w).range(1..=mw));
            ui.add(egui::DragValue::new(&mut z.h).range(1..=mh));
        });
        ui.end_row();
        let zone_name = z.name.clone();
        if let ZoneKind::Transition { target_map, target_zone } = &mut z.kind {
            ui.label("Leads to map");
            egui::ComboBox::from_id_salt("tr_map").selected_text(if target_map.is_empty() { "(not linked)" } else { target_map.as_str() }).show_ui(ui, |ui| {
                for m in &refs.maps {
                    if ui.selectable_label(target_map == m, m).clicked() {
                        *target_map = m.clone();
                        target_zone.clear();
                    }
                }
            });
            ui.end_row();
            if !target_map.is_empty() {
                ui.label("Other end");
                let others: Vec<(String, String, String)> = refs.transitions.get(target_map.as_str()).cloned().unwrap_or_default();
                egui::ComboBox::from_id_salt("tr_zone").selected_text(if target_zone.is_empty() { "(choose)" } else { target_zone.as_str() }).show_ui(ui, |ui| {
                    for (name, _, _) in &others {
                        ui.selectable_value(target_zone, name.clone(), name);
                    }
                });
                ui.end_row();
                // Status: does the other end exist and lead back here?
                let me = refs.current_map.clone();
                let other = others.iter().find(|(n, _, _)| n == target_zone);
                ui.label("");
                match other {
                    Some((_, m, z)) if *m == me && *z == zone_name => {
                        ui.colored_label(Color32::LIGHT_GREEN, "\u{E182} linked both ways");
                    }
                    Some(_) => {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::YELLOW, "the other end leads elsewhere");
                            if ui.button("Link back").on_hover_text("Make the other end lead here").clicked() {
                                crate::maps::request_transition(crate::maps::TransitionReq::LinkBack { zone: zone_name.clone(), map: target_map.clone(), other: target_zone.clone() });
                            }
                        });
                    }
                    None if target_zone.is_empty() => {
                        ui.colored_label(Color32::YELLOW, "choose the other end, or draw it");
                    }
                    None => {
                        ui.colored_label(Color32::LIGHT_RED, format!("'{target_map}' has no transition '{target_zone}'"));
                    }
                }
                ui.end_row();
            }
            ui.label("");
            ui.horizontal_wrapped(|ui| {
                let map_for_draw = if target_map.is_empty() { refs.maps.iter().find(|m| **m != refs.current_map).cloned().unwrap_or_default() } else { target_map.clone() };
                if !map_for_draw.is_empty() && ui.button(format!("\u{E3B4} Draw the other end on {map_for_draw}")).on_hover_text("Opens that map: drag the other end there; both ends get linked").clicked() {
                    crate::maps::request_transition(crate::maps::TransitionReq::DrawOther { zone: zone_name.clone(), map: map_for_draw });
                }
                if !target_map.is_empty() {
                    if ui.button(format!("\u{E5DE} Go to {target_map}")).clicked() {
                        GOTO.with(|g| *g.borrow_mut() = Some((target_map.clone(), target_zone.clone())));
                    }
                    if ui.button("Unlink").on_hover_text("Also unlinks the other end if it leads here").clicked() {
                        crate::maps::request_transition(crate::maps::TransitionReq::Unlink { zone: zone_name.clone() });
                    }
                }
            });
            ui.end_row();
        }
        if let ZoneKind::Portal { target_map, target_spawn } = &mut z.kind {
            ui.label("Target map");
            widgets::suggest(ui, "pmap", target_map, &refs.maps, 120.0);
            ui.end_row();
            ui.label("Target spawn");
            let spawns = refs.spawns.get(target_map.as_str()).cloned().unwrap_or_default();
            widgets::suggest(ui, "pspawn", target_spawn, &spawns, 120.0);
            ui.end_row();
            ui.label("");
            if ui.button(format!("\u{E5DE} Go to {target_map}")).on_hover_text("Open the target map at the target spawn (or double-click the portal)").clicked() {
                GOTO.with(|g| *g.borrow_mut() = Some((target_map.clone(), target_spawn.clone())));
            }
            ui.end_row();
        }
        if let ZoneKind::FishingSpot { rod } = &mut z.kind {
            ui.label("Needs item");
            ui.horizontal(|ui| {
                widgets::suggest(ui, "fish_rod", rod, &refs.items, 120.0);
                if rod.is_empty() {
                    ui.weak("(nothing: anyone can fish)");
                }
            })
            .response
            .on_hover_text("The rod (or net) the player must have to fish here");
            ui.end_row();
        }
        if let ZoneKind::Spawner { character, count, respawn } = &mut z.kind {
            ui.label("Character");
            widgets::suggest(ui, "sp_char", character, &refs.characters, 120.0);
            ui.end_row();
            ui.label("How many");
            ui.add(egui::DragValue::new(count).range(1..=100)).on_hover_text("Characters alive at the same time");
            ui.end_row();
            ui.label("Respawn");
            ui.vertical(|ui| {
                let mut mode = match respawn {
                    Respawn::Never => 0,
                    Respawn::OnEnter => 1,
                    Respawn::After { .. } => 2,
                };
                let old = mode;
                ui.radio_value(&mut mode, 0, "Never (defeated ones stay dead, saved)");
                ui.radio_value(&mut mode, 1, "Each time the map is entered");
                ui.radio_value(&mut mode, 2, "After a delay");
                if mode != old {
                    *respawn = match mode {
                        0 => Respawn::Never,
                        1 => Respawn::OnEnter,
                        _ => Respawn::After { seconds: 30.0 },
                    };
                }
                if let Respawn::After { seconds } = respawn {
                    ui.horizontal(|ui| {
                        ui.label("seconds");
                        ui.add(egui::DragValue::new(seconds).range(0.5..=3600.0).speed(0.5));
                    });
                }
            });
            ui.end_row();
        }
    });
    match z.kind {
        ZoneKind::Spawn => {
            ui.weak("Spawn points are referenced by name from portals, teleports and the project start.");
        }
        ZoneKind::Spawner { .. } => {
            ui.weak("Invisible in the game. Characters appear on random walkable tiles inside the zone (never right next to the player). Their 'Defeated' triggers come from the character definition.");
        }
        ZoneKind::FishingSpot { .. } => {
            ui.weak("Draw it over the water where the fish are. Facing it from the shore (up to 3 tiles away), the player presses E to cast: the bobber lands on the zone, then wait for the bite, strike, and hold E to reel in without breaking the line. Fish shadows glide through the zone now and then to show players where to fish. The catch comes from the table below (chances are weights: every fish landed gives one entry).");
        }
        _ => {}
    }
}

/// "Uses …: Coin 50% (1-3), Apple 10%" for a loot table that isn't customized here.
fn loot_summary(table: &[rustpg2d_engine::defs::LootEntry], db: &rustpg2d_engine::defs::Database, prefix: &str) -> String {
    if table.is_empty() {
        return format!("{prefix}: nothing.");
    }
    let parts: Vec<String> = table
        .iter()
        .map(|l| {
            let name = db.item(&l.item).map_or(l.item.clone(), |d| d.name.clone());
            let amount = if l.min == l.max { if l.min > 1 { format!(" ×{}", l.min) } else { String::new() } } else { format!(" ({}–{})", l.min, l.max) };
            format!("{name} {}%{amount}", l.chance)
        })
        .collect();
    format!("{prefix}: {}.", parts.join(", "))
}

pub fn default_spawner(refs: &Refs) -> ZoneKind {
    let character = refs.characters.iter().find(|c| c.contains("slime") || c.contains("enemy")).or(refs.characters.first()).cloned().unwrap_or_default();
    ZoneKind::Spawner { character, count: 3, respawn: Respawn::OnEnter }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Flag names used anywhere on a map (for suggestions).
/// Flag and variable names used by triggers (conditions and actions, also nested in "If").
pub fn collect_names(ts: &[rustpg2d_engine::events::Trigger], flags: &mut Vec<String>, vars: &mut Vec<String>) {
    use rustpg2d_engine::events::{Action, Cond, Condition};
    fn conds(cs: &[Condition], flags: &mut Vec<String>, vars: &mut Vec<String>) {
        for c in cs {
            match &c.cond {
                Cond::Flag { name } => flags.push(name.clone()),
                Cond::Var { name, .. } => vars.push(name.clone()),
                _ => {}
            }
        }
    }
    fn actions(a: &[Action], flags: &mut Vec<String>, vars: &mut Vec<String>) {
        for a in a {
            match a {
                Action::SetFlag { name, .. } => flags.push(name.clone()),
                Action::SetVar { name, .. } => vars.push(name.clone()),
                Action::Choice { var, .. } => vars.push(var.clone()),
                Action::If { conditions, then, otherwise } => {
                    conds(conditions, flags, vars);
                    actions(then, flags, vars);
                    actions(otherwise, flags, vars);
                }
                _ => {}
            }
        }
    }
    for t in ts {
        conds(&t.conditions, flags, vars);
        actions(&t.actions, flags, vars);
    }
}

fn collect_flags(map: &Map, out: &mut Vec<String>) {
    let mut vars = Vec::new();
    collect_map_names(map, out, &mut vars);
}

/// Flags and variables used by a map's triggers (map, entities, zones).
fn collect_map_names(map: &Map, flags: &mut Vec<String>, vars: &mut Vec<String>) {
    collect_names(&map.triggers, flags, vars);
    for e in &map.entities {
        collect_names(&e.triggers, flags, vars);
    }
    for z in &map.zones {
        collect_names(&z.triggers, flags, vars);
    }
}

/// Dialog speakers to offer for a map: placeholders, the characters on it (definition
/// names and named instances) and speakers its dialogs already use.
pub fn speakers_of(map: &Map, db: &rustpg2d_engine::defs::Database) -> Vec<(String, String)> {
    use rustpg2d_engine::events::{Action, Trigger};
    let mut out: Vec<(String, String)> = vec![("{self}".into(), "name of the trigger's owner".into()), ("{player}".into(), "the player's name".into())];
    let mut add = |v: String, hint: String| {
        if !v.trim().is_empty() && !out.iter().any(|(o, _)| *o == v) {
            out.push((v, hint));
        }
    };
    for e in map.entities.iter().filter(|e| e.kind == EntityKind::Character) {
        if !e.name.is_empty() {
            let mut c = e.name.chars();
            let nice: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
            add(nice.replace('_', " "), format!("'{}' on this map", e.name));
        }
        if let Some(d) = db.character(&e.def) {
            add(d.name.clone(), "character on this map".into());
        }
    }
    fn used(acts: &[Action], out: &mut Vec<String>) {
        for a in acts {
            match a {
                Action::Dialog { speaker, .. } | Action::Choice { speaker, .. } => out.push(speaker.clone()),
                Action::If { then, otherwise, .. } => {
                    used(then, out);
                    used(otherwise, out);
                }
                _ => {}
            }
        }
    }
    let mut names = Vec::new();
    let mut trig = |ts: &[Trigger]| ts.iter().for_each(|t| used(&t.actions, &mut names));
    trig(&map.triggers);
    map.entities.iter().for_each(|e| trig(&e.triggers));
    map.zones.iter().for_each(|z| trig(&z.triggers));
    names.sort();
    for n in names {
        add(n, "used in a dialog here".into());
    }
    out
}

/// *Parallax layers* of the map settings: image layers moving at their own speed behind the
/// tiles or over the map (`Map::parallax`).
fn parallax_ui(ui: &mut egui::Ui, layers: &mut Vec<rustpg2d_engine::map::ParallaxLayer>, db: &rustpg2d_engine::defs::Database, tex: &crate::app::Textures) {
    use rustpg2d_engine::map::ParallaxLayer;
    ui.heading("Parallax layers");
    ui.weak("Images moving at their own speed: behind the tiles (a sky or abyss, seen under empty tiles and past the map's edge) or over the map (cloud shadows, mist). The image is a sprite sheet (Database › Sprite sheets), drawn whole. Preview: View › Parallax layers.");
    let sheets: Vec<String> = db.sheets.iter().map(|s| s.id.clone()).collect();
    let (mut remove, mut swap) = (None, None);
    let n = layers.len();
    for (k, l) in layers.iter_mut().enumerate() {
        ui.push_id(("parallax", k), |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", k + 1));
                    crate::widgets::suggest(ui, "plx_sheet", &mut l.sheet, &sheets, 130.0);
                    if let Some(t) = tex.sheets.get(&l.sheet) {
                        let size = t.size_vec2();
                        let h = 28.0;
                        ui.image((t.id(), egui::vec2(h * size.x / size.y.max(1.0), h).min(egui::vec2(80.0, h))));
                    } else if !l.sheet.is_empty() {
                        ui.colored_label(egui::Color32::from_rgb(240, 120, 90), "no such sheet");
                    }
                    if k > 0 && ui.small_button(crate::ui::ph::ARROW_UP).on_hover_text("Draw earlier (further back)").clicked() {
                        swap = Some(k - 1);
                    }
                    if k + 1 < n && ui.small_button(crate::ui::ph::ARROW_DOWN).on_hover_text("Draw later (further in front)").clicked() {
                        swap = Some(k);
                    }
                    if ui.small_button(crate::ui::ph::TRASH).on_hover_text("Remove this layer").clicked() {
                        remove = Some(k);
                    }
                });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut l.above, false, "Behind the map");
                    ui.radio_value(&mut l.above, true, "Over the map");
                });
                ui.horizontal(|ui| {
                    ui.label("Follows").on_hover_text("How much the layer moves with the map (x, y): 100 % = with the ground, 0 % = fixed to the screen, in between = far away");
                    for a in 0..2 {
                        let mut pct = (l.factor[a] * 100.0).round();
                        if ui.add(egui::DragValue::new(&mut pct).range(0.0..=200.0).suffix(" %").speed(1.0)).changed() {
                            l.factor[a] = pct / 100.0;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Drift").on_hover_text("Constant movement in pixels per second (x, y): drifting clouds");
                    ui.add(egui::DragValue::new(&mut l.drift[0]).speed(0.5).suffix(" px/s"));
                    ui.add(egui::DragValue::new(&mut l.drift[1]).speed(0.5).suffix(" px/s"));
                });
                ui.horizontal(|ui| {
                    ui.label("Offset").on_hover_text("Where the image starts (pixels)");
                    ui.add(egui::DragValue::new(&mut l.offset[0]).speed(1.0));
                    ui.add(egui::DragValue::new(&mut l.offset[1]).speed(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Repeat");
                    ui.checkbox(&mut l.repeat[0], "x");
                    ui.checkbox(&mut l.repeat[1], "y");
                    ui.label("Size");
                    ui.add(egui::DragValue::new(&mut l.scale).range(0.05..=16.0).speed(0.05).suffix("×"));
                });
                ui.horizontal(|ui| {
                    ui.label("Opacity");
                    let mut pct = (l.alpha * 100.0).round();
                    if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %")).changed() {
                        l.alpha = pct / 100.0;
                    }
                });
            });
        });
    }
    if let Some(k) = remove {
        layers.remove(k);
    }
    if let Some(k) = swap {
        layers.swap(k, k + 1);
    }
    if ui.button(crate::ui::t(crate::ui::ph::PLUS, "Parallax layer")).clicked() {
        layers.push(ParallaxLayer { sheet: sheets.first().cloned().unwrap_or_default(), ..Default::default() });
    }
}
