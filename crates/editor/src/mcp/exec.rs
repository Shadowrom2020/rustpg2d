//! Agent tool calls executed on the editor's main thread: [`Call`] is what `tools.rs` sends,
//! [`Reply`] what comes back. `Editor::mcp_exec` dispatches; the project / session tools live
//! here, maps in `exec_maps.rs`, definitions / scripts / languages / library in `exec_defs.rs`.
//! Shared rules: replies are plain text bounded by [`MAX_TEXT`], writes refuse to run while the
//! editor waits for an answer (`pending`), end with [`Editor::mcp_after_write`] (save, refresh,
//! warnings) and are one undo step each.

use serde::Serialize;
use serde_json::Value;

use crate::app::{Editor, Mode, Selection};

use super::tools::*;

/// A tool call (the parameter structs are those of `tools.rs`).
pub enum Call {
    ProjectInfo,
    EditorState,
    Save,
    Undo(UndoParams),
    Redo(UndoParams),
    SetProjectConfig(PatchParams),
    ListMaps,
    GetMap(GetMapParams),
    GetObject(ObjectParams),
    TilesetLegend(TilesetParams),
    ViewImage(ViewImageParams),
    ScreenshotMap(ScreenshotParams),
    ScreenshotEditor,
    CreateMap(CreateMapParams),
    SetTiles(SetTilesParams),
    PaintGrid(PaintGridParams),
    FillRect(FillRectParams),
    PaintTerrain(PaintTerrainParams),
    SetCollision(SetCollisionParams),
    ClearRegion(RectParams),
    ResizeMap(ResizeParams),
    PlaceEntity(PlaceEntityParams),
    UpdateEntity(UpdateParams),
    RemoveEntity(ObjectParams),
    AddZone(AddZoneParams),
    UpdateZone(UpdateParams),
    RemoveZone(ObjectParams),
    LinkMaps(LinkMapsParams),
    SetTriggers(TriggersParams),
    AddTrigger(AddTriggerParams),
    SetMapSettings(MapSettingsParams),
    PlaceStructure(PlaceStructureParams),
    Rename(RenameParams),
    ListDefs(ListDefsParams),
    GetDef(DefParams),
    DefTemplate(DefTemplateParams),
    UpsertDef(UpsertDefParams),
    DeleteDef(DeleteDefParams),
    EventsReference(EventsRefParams),
    ListScripts,
    ReadScript(ReadScriptParams),
    WriteScript(WriteScriptParams),
    CheckScript(CheckScriptParams),
    ScriptingApi(FilterParams),
    TranslationTemplate(LangParams),
    ReadTranslation(ReadTranslationParams),
    WriteTranslation(WriteTranslationParams),
    PlayTest(PlayParams),
    LibraryList(LibraryListParams),
    LibraryInfo(DefParams),
    LibraryAdd(LibraryAddParams),
}

/// What a screenshot should show (resolved after the next frame, `exec_maps.rs`).
pub struct ShotSpec {
    /// Tiles of the open map to frame; None = the whole window as it is.
    pub rect: Option<(u32, u32, u32, u32)>,
    pub zoom: Option<f32>,
    pub grid: Option<bool>,
    pub collision: Option<bool>,
    pub zones: Option<bool>,
    pub text: String,
}

/// A tool's answer.
pub enum Reply {
    Text(String),
    /// Text and PNG images.
    TextImages(String, Vec<Vec<u8>>),
    /// A play-test the agent thread runs (the game process), see `play.rs`.
    PlayPlan(Box<super::play::PlayPlan>),
    /// A screenshot taken once the next frame is drawn.
    Shot(ShotSpec),
}

/// Longest text reply (characters): more is cut with a note.
pub const MAX_TEXT: usize = 48_000;
/// Rows per page of listings.
pub const PAGE: usize = 200;

impl Call {
    pub fn name(&self) -> &'static str {
        match self {
            Call::ProjectInfo => "project_info",
            Call::EditorState => "editor_state",
            Call::Save => "save",
            Call::Undo(_) => "undo",
            Call::Redo(_) => "redo",
            Call::SetProjectConfig(_) => "set_project_config",
            Call::ListMaps => "list_maps",
            Call::GetMap(_) => "get_map",
            Call::GetObject(_) => "get_object",
            Call::TilesetLegend(_) => "tileset_legend",
            Call::ViewImage(_) => "view_image",
            Call::ScreenshotMap(_) => "screenshot_map",
            Call::ScreenshotEditor => "screenshot_editor",
            Call::CreateMap(_) => "create_map",
            Call::SetTiles(_) => "set_tiles",
            Call::PaintGrid(_) => "paint_grid",
            Call::FillRect(_) => "fill_rect",
            Call::PaintTerrain(_) => "paint_terrain",
            Call::SetCollision(_) => "set_collision",
            Call::ClearRegion(_) => "clear_region",
            Call::ResizeMap(_) => "resize_map",
            Call::PlaceEntity(_) => "place_entity",
            Call::UpdateEntity(_) => "update_entity",
            Call::RemoveEntity(_) => "remove_entity",
            Call::AddZone(_) => "add_zone",
            Call::UpdateZone(_) => "update_zone",
            Call::RemoveZone(_) => "remove_zone",
            Call::LinkMaps(_) => "link_maps",
            Call::SetTriggers(_) => "set_triggers",
            Call::AddTrigger(_) => "add_trigger",
            Call::SetMapSettings(_) => "set_map_settings",
            Call::PlaceStructure(_) => "place_structure",
            Call::Rename(_) => "rename",
            Call::ListDefs(_) => "list_defs",
            Call::GetDef(_) => "get_def",
            Call::DefTemplate(_) => "def_template",
            Call::UpsertDef(_) => "upsert_def",
            Call::DeleteDef(_) => "delete_def",
            Call::EventsReference(_) => "events_reference",
            Call::ListScripts => "list_scripts",
            Call::ReadScript(_) => "read_script",
            Call::WriteScript(_) => "write_script",
            Call::CheckScript(_) => "check_script",
            Call::ScriptingApi(_) => "scripting_api",
            Call::TranslationTemplate(_) => "translation_template",
            Call::ReadTranslation(_) => "read_translation",
            Call::WriteTranslation(_) => "write_translation",
            Call::PlayTest(_) => "play_test",
            Call::LibraryList(_) => "library_list",
            Call::LibraryInfo(_) => "library_info",
            Call::LibraryAdd(_) => "library_add",
        }
    }
}

/// A text reply, cut to [`MAX_TEXT`].
pub(crate) fn text(s: impl Into<String>) -> Result<Reply, String> {
    Ok(Reply::Text(clip(s.into())))
}

pub(crate) fn clip(mut s: String) -> String {
    if s.chars().count() > MAX_TEXT {
        let cut = s.char_indices().nth(MAX_TEXT).map_or(s.len(), |(i, _)| i);
        s.truncate(cut);
        s.push_str("\n… (cut: ask for a smaller region or the next page)");
    }
    s
}

/// Pretty JSON of a value (definitions, objects).
pub(crate) fn json(v: &impl Serialize) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|e| format!("(not serializable: {e})"))
}

/// " Did you mean …?" for an unknown id: candidates containing the text, else the first ten.
pub(crate) fn suggestions<'a>(wanted: &str, candidates: impl IntoIterator<Item = &'a str>) -> String {
    let all: Vec<&str> = candidates.into_iter().collect();
    let w = wanted.to_lowercase();
    let mut close: Vec<&str> = all.iter().copied().filter(|c| c.to_lowercase().contains(&w) || w.contains(&c.to_lowercase())).take(10).collect();
    if close.is_empty() {
        close = all.iter().copied().take(10).collect();
    }
    if close.is_empty() {
        String::new()
    } else {
        format!(" Known: {}{}", close.join(", "), if all.len() > close.len() { ", …" } else { "" })
    }
}

/// Page `offset..offset+PAGE` of `lines` with a trailer when more follow.
pub(crate) fn page(lines: Vec<String>, offset: usize) -> String {
    let total = lines.len();
    let mut out: Vec<String> = lines.into_iter().skip(offset).take(PAGE).collect();
    if offset + out.len() < total {
        out.push(format!("… {} more (offset {} for the next page)", total - offset - out.len(), offset + PAGE));
    }
    if out.is_empty() {
        out.push(if total == 0 { "(none)".into() } else { format!("(offset {offset} is past the end: {total} entries)") });
    }
    out.join("\n")
}

impl Editor {
    pub fn mcp_exec(&mut self, call: Call) -> Result<Reply, String> {
        match call {
            Call::ProjectInfo => self.mcp_project_info(),
            Call::EditorState => self.mcp_editor_state(),
            Call::Save => self.mcp_save(),
            Call::Undo(p) => self.mcp_undo(p, false),
            Call::Redo(p) => self.mcp_undo(p, true),
            Call::SetProjectConfig(p) => self.mcp_set_project_config(p),
            Call::ListMaps => self.mcp_list_maps(),
            Call::GetMap(p) => self.mcp_get_map(p),
            Call::GetObject(p) => self.mcp_get_object(p),
            Call::TilesetLegend(p) => self.mcp_tileset_legend(p),
            Call::ViewImage(p) => self.mcp_view_image(p),
            Call::ScreenshotMap(p) => self.mcp_screenshot_map(p).map(Reply::Shot),
            Call::ScreenshotEditor => Ok(Reply::Shot(ShotSpec { rect: None, zoom: None, grid: None, collision: None, zones: None, text: "The editor window.".into() })),
            Call::CreateMap(p) => self.mcp_create_map(p),
            Call::SetTiles(p) => self.mcp_set_tiles(p),
            Call::PaintGrid(p) => self.mcp_paint_grid(p),
            Call::FillRect(p) => self.mcp_fill_rect(p),
            Call::PaintTerrain(p) => self.mcp_paint_terrain(p),
            Call::SetCollision(p) => self.mcp_set_collision(p),
            Call::ClearRegion(p) => self.mcp_clear_region(p),
            Call::ResizeMap(p) => self.mcp_resize_map(p),
            Call::PlaceEntity(p) => self.mcp_place_entity(p),
            Call::UpdateEntity(p) => self.mcp_update_entity(p),
            Call::RemoveEntity(p) => self.mcp_remove_entity(p),
            Call::AddZone(p) => self.mcp_add_zone(p),
            Call::UpdateZone(p) => self.mcp_update_zone(p),
            Call::RemoveZone(p) => self.mcp_remove_zone(p),
            Call::LinkMaps(p) => self.mcp_link_maps(p),
            Call::SetTriggers(p) => self.mcp_set_triggers(p),
            Call::AddTrigger(p) => self.mcp_add_trigger(p),
            Call::SetMapSettings(p) => self.mcp_set_map_settings(p),
            Call::PlaceStructure(p) => self.mcp_place_structure(p),
            Call::Rename(p) => self.mcp_rename(p),
            Call::ListDefs(p) => self.mcp_list_defs(p),
            Call::GetDef(p) => self.mcp_get_def(p),
            Call::DefTemplate(p) => self.mcp_def_template(p),
            Call::UpsertDef(p) => self.mcp_upsert_def(p),
            Call::DeleteDef(p) => self.mcp_delete_def(p),
            Call::EventsReference(p) => self.mcp_events_reference(p),
            Call::ListScripts => self.mcp_list_scripts(),
            Call::ReadScript(p) => self.mcp_read_script(p),
            Call::WriteScript(p) => self.mcp_write_script(p),
            Call::CheckScript(p) => self.mcp_check_script(p),
            Call::ScriptingApi(p) => self.mcp_scripting_api(p),
            Call::TranslationTemplate(p) => self.mcp_translation_template(p),
            Call::ReadTranslation(p) => self.mcp_read_translation(p),
            Call::WriteTranslation(p) => self.mcp_write_translation(p),
            Call::PlayTest(p) => super::play::plan(self, p).map(|pl| Reply::PlayPlan(Box::new(pl))),
            Call::LibraryList(p) => self.mcp_library_list(p),
            Call::LibraryInfo(p) => self.mcp_library_info(p),
            Call::LibraryAdd(p) => self.mcp_library_add(p),
        }
    }

    /// Reading needs an open project (not the library window).
    pub(crate) fn mcp_need_project(&self) -> Result<(), String> {
        if !self.project_open {
            return Err("no project is open in the editor (File > Open project…)".into());
        }
        if self.library_mode {
            return Err("this is the Asset Library window: agents work with the main editor's project".into());
        }
        Ok(())
    }

    /// Writing also needs the editor not to be waiting for an answer.
    pub(crate) fn mcp_writable(&self) -> Result<(), String> {
        self.mcp_need_project()?;
        if self.pending.is_some() {
            return Err("the editor is waiting for the person to answer its 'Unsaved changes' question; try again when it is answered".into());
        }
        Ok(())
    }

    /// Ends a write: the stroke, saving, refreshed suggestions, the watcher's snapshot, and the
    /// project check's warnings appended to `done`.
    pub(crate) fn mcp_after_write(&mut self, done: String) -> Result<Reply, String> {
        if let Some(doc) = &mut self.doc {
            doc.end_stroke();
        }
        self.save_all();
        self.refresh_refs();
        self.watch_resnapshot();
        let mut out = done;
        if self.doc.as_ref().is_some_and(|d| d.is_dirty()) || self.defs_dirty {
            out.push_str("\nWarning: saving failed, see the editor's status line.");
        }
        let (_, check) = rustpg2d_engine::release::check(&self.project);
        for e in check.errors {
            out.push_str(&format!("\nProject error: {e}"));
        }
        for w in check.warnings {
            out.push_str(&format!("\nProject warning: {w}"));
        }
        text(out)
    }

    fn mcp_project_info(&mut self) -> Result<Reply, String> {
        if !self.project_open {
            return text("No project is open in the editor (its welcome screen shows). Ask the person to open one; nothing else works until then.");
        }
        let cfg = &self.project.config;
        let db = &self.project.db;
        let mut s = format!("Project \"{}\" at {} (engine build {})\n", cfg.name, self.project.root.display(), rustpg2d_engine::ENGINE_BUILD);
        s += &format!(
            "Start: map \"{}\", spawn \"{}\"; player character: \"{}\"; language: {}{}; day length {} min, start hour {}\n",
            cfg.start_map,
            cfg.start_spawn,
            cfg.player,
            cfg.language,
            if cfg.languages.is_empty() { String::new() } else { format!(" (translations: {})", cfg.languages.join(", ")) },
            cfg.day_minutes,
            cfg.start_hour
        );
        s += &format!("Maps ({}):\n", self.maps.len());
        for m in &self.maps {
            match self.mcp_load_map(m) {
                Ok(map) => {
                    let spawns: Vec<&str> = map.zones.iter().filter(|z| z.kind == rustpg2d_engine::map::ZoneKind::Spawn).map(|z| z.name.as_str()).collect();
                    s += &format!("  {m}: {}x{} tiles, {} entities, {} zones, spawns: {}\n", map.width, map.height, map.entities.len(), map.zones.len(), if spawns.is_empty() { "-".to_string() } else { spawns.join(", ") });
                }
                Err(e) => s += &format!("  {m}: cannot load ({e})\n"),
            }
        }
        s += &format!(
            "Definitions: {} tilesets, {} sprite sheets, {} items, {} props, {} characters, {} sounds, {} quests, {} structures, {} skills, {} spells, {} recipes, {} achievements (list_defs KIND)\n",
            db.tilesets.len(),
            db.sheets.len(),
            db.items.len(),
            db.props.len(),
            db.characters.len(),
            db.sounds.len(),
            db.quests.len(),
            db.structures.len(),
            db.skills.len(),
            db.spells.len(),
            db.recipes.len(),
            db.achievements.len()
        );
        let scripts = self.mcp_script_files();
        if scripts.is_empty() {
            s += "Scripts: none (scripts/*.rhai)\n";
        } else {
            let parts: Vec<String> = scripts
                .iter()
                .map(|p| {
                    let n = std::fs::read_to_string(p).map(|t| rustpg2d_engine::script_refs::functions(&t).len()).unwrap_or(0);
                    format!("{} ({n} functions)", p.file_name().unwrap_or_default().to_string_lossy())
                })
                .collect();
            s += &format!("Scripts: {}\n", parts.join(", "));
        }
        s += &format!("Asset library: {} (library_list)\n", rustpg2d_engine::library::library_dir().display());
        s += &self.mcp_state_line();
        let (_, check) = rustpg2d_engine::release::check(&self.project);
        if check.errors.is_empty() && check.warnings.is_empty() {
            s += "Project check: OK";
        } else {
            for e in &check.errors {
                s += &format!("Project error: {e}\n");
            }
            for w in &check.warnings {
                s += &format!("Project warning: {w}\n");
            }
        }
        text(s)
    }

    /// One line about the editor's own state.
    fn mcp_state_line(&self) -> String {
        let map = match &self.doc {
            Some(d) => format!("map \"{}\" open ({})", d.map.name, if d.is_dirty() { "unsaved edits" } else { "saved" }),
            None => "no map open".to_string(),
        };
        format!(
            "Editor: {map}; definitions {}; {}{}; mode {}; {} agent calls so far\n",
            if self.defs_dirty { "unsaved" } else { "saved" },
            if self.pending.is_some() { "waiting for an answer to 'Unsaved changes'" } else { "no dialog pending" },
            if self.watch.changed.is_empty() { String::new() } else { format!("; files changed on disk: {}", self.watch.changed.join(", ")) },
            match self.mode {
                Mode::Map => "map",
                Mode::Database => "database",
                Mode::Library => "library",
                Mode::Pixel => "pixel editor",
                Mode::Events => "events graph",
            },
            self.mcp.calls
        )
    }

    fn mcp_editor_state(&mut self) -> Result<Reply, String> {
        if !self.project_open {
            return text("No project is open (welcome screen).");
        }
        let mut s = self.mcp_state_line();
        if let Some(d) = &self.doc {
            let sel = match self.sel {
                Selection::None => "nothing".to_string(),
                Selection::Entity(uid) => d.map.entities.iter().find(|e| e.uid == uid).map_or(format!("entity {uid}"), |e| format!("entity {uid} ({} \"{}\")", e.def, e.name)),
                Selection::Zone(uid) => d.map.zones.iter().find(|z| z.uid == uid).map_or(format!("zone {uid}"), |z| format!("zone {uid} \"{}\"", z.name)),
                Selection::Region((x, y, w, h)) => format!("tile region {x},{y} {w}x{h}"),
            };
            s += &format!("Selection: {sel}; layer {}; undo {}, redo {}\n", self.layer, if d.can_undo() { "possible" } else { "nothing" }, if d.can_redo() { "possible" } else { "nothing" });
        }
        if self.mode == Mode::Database {
            s += &format!("Database tab: {:?}\n", self.db.tab);
        }
        if let Some(f) = &self.db.script_file {
            s += &format!("Script open in the editor: {}{}\n", f.file_name().unwrap_or_default().to_string_lossy(), if self.db.script_dirty { " (unsaved edits)" } else { "" });
        }
        s += &format!("Last status: {}\n", self.status.0);
        s += &format!("Agent server: {}\n", self.mcp.status());
        text(s)
    }

    fn mcp_save(&mut self) -> Result<Reply, String> {
        self.mcp_writable()?;
        let before = (self.doc.as_ref().is_some_and(|d| d.is_dirty()), self.defs_dirty, self.px.is_dirty());
        self.save_all();
        self.watch_resnapshot();
        let mut done = Vec::new();
        if before.0 {
            done.push("map");
        }
        if before.1 {
            done.push("definitions");
        }
        if before.2 {
            done.push("pixel art");
        }
        let still = self.doc.as_ref().is_some_and(|d| d.is_dirty()) || self.defs_dirty;
        text(format!(
            "Saved: {}.{}",
            if done.is_empty() { "nothing was unsaved".to_string() } else { done.join(", ") },
            if still { " Something could not be saved: see the editor's status line." } else { "" }
        ))
    }

    fn mcp_undo(&mut self, p: UndoParams, redo: bool) -> Result<Reply, String> {
        self.mcp_writable()?;
        let steps = p.steps.unwrap_or(1).clamp(1, 50);
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let mut n = 0;
        for _ in 0..steps {
            if redo {
                if !doc.can_redo() {
                    break;
                }
                doc.redo(assets);
            } else {
                if !doc.can_undo() {
                    break;
                }
                doc.undo(assets);
            }
            n += 1;
        }
        let dirty = doc.is_dirty();
        text(format!(
            "{} {n} step(s) on map \"{}\".{}",
            if redo { "Redid" } else { "Undid" },
            doc.map.name,
            if dirty { " The map now has unsaved changes: call `save` to write them (or the person saves)." } else { " The map matches its saved state." }
        ))
    }

    fn mcp_set_project_config(&mut self, p: PatchParams) -> Result<Reply, String> {
        self.mcp_writable()?;
        let cfg = &self.project.config;
        let new: rustpg2d_engine::project::ProjectConfig = super::patch::apply(cfg, &p.patch)?;
        if new.tile_size != cfg.tile_size {
            return Err("tile_size can't change: tiles are always 32 px".into());
        }
        if !self.maps.contains(&new.start_map) {
            return Err(format!("start_map \"{}\" is not a map.{}", new.start_map, suggestions(&new.start_map, self.maps.iter().map(String::as_str))));
        }
        let spawns = self.refs.spawns.get(&new.start_map).cloned().unwrap_or_default();
        if !spawns.contains(&new.start_spawn) {
            return Err(format!("start_spawn \"{}\" is not a spawn zone of {}.{}", new.start_spawn, new.start_map, suggestions(&new.start_spawn, spawns.iter().map(String::as_str))));
        }
        if !new.player.is_empty() && self.project.db.character(&new.player).is_none() {
            return Err(format!("player \"{}\" is not a character.{}", new.player, suggestions(&new.player, self.project.db.characters.iter().map(|c| c.id.as_str()))));
        }
        let changed: Vec<String> = match (serde_json::to_value(cfg), serde_json::to_value(&new)) {
            (Ok(Value::Object(a)), Ok(Value::Object(b))) => b.iter().filter(|(k, v)| a.get(*k) != Some(v)).map(|(k, _)| k.clone()).collect(),
            _ => vec![],
        };
        self.project.config = new;
        self.defs_dirty = true;
        self.mcp_after_write(format!("Project settings changed: {}.", if changed.is_empty() { "nothing differs".to_string() } else { changed.join(", ") }))
    }

    /// The map `name`: the open document's (with unsaved edits) or loaded from disk.
    pub(crate) fn mcp_load_map(&self, name: &str) -> Result<rustpg2d_engine::map::Map, String> {
        match &self.doc {
            Some(d) if d.map.name == name => Ok(d.map.clone()),
            _ => self.project.load_map(name).map_err(|e| format!("{e:#}")),
        }
    }

    /// `scripts/*.rhai`, sorted.
    pub(crate) fn mcp_script_files(&self) -> Vec<std::path::PathBuf> {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(self.project.scripts_dir()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect();
        files.sort();
        files
    }
}
