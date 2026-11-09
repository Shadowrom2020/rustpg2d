//! The tool catalogue agents see: one thin `#[tool]` per tool that turns its parameters into a
//! [`Call`] for the main thread. The parameter structs' doc comments become the JSON schema
//! descriptions, so they are written for the agent. Also the prompts.
//!
//! Conventions (also in `Documentation/mcp.md`): coordinates are tiles (origin top-left, x right,
//! y down; an entity stands on the tile of its feet); rects are `x, y, w, h`; `map` omitted means
//! the map open in the editor; definitions, entities, zones and triggers are JSON in the serde
//! form of the engine's types (externally tagged enums), so what `get_*` returns is valid input.

use std::collections::BTreeMap;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Prompt, PromptArgument};
use rmcp::{tool, tool_router, ErrorData};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::exec::Call;
use super::server::Handler;

/// The parameters abbreviated for the call log.
fn summary<T: Serialize>(p: &T) -> String {
    let mut v = serde_json::to_value(p).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        m.retain(|_, v| !v.is_null());
    }
    let s = serde_json::to_string(&v).unwrap_or_default();
    if s.chars().count() > 80 {
        format!("{}…", s.chars().take(79).collect::<String>())
    } else {
        s
    }
}

// ---------------------------------------------------------------- parameter structs

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct UndoParams {
    /// Steps to undo / redo (default 1, at most 50).
    pub steps: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatchParams {
    /// A JSON merge patch (RFC 7396): fields to change; `null` removes a field.
    pub patch: Value,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetMapParams {
    /// Map name; omitted = the map open in the editor.
    pub map: Option<String>,
    /// Region (tiles). Omitted = the whole map when it has at most 80x50 tiles.
    pub x: Option<u32>,
    pub y: Option<u32>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    /// Layer indices to print (default: all).
    pub layers: Option<Vec<usize>>,
    /// Print the collision grid (default true).
    pub collision: Option<bool>,
    /// List the entities and zones in the region (default true).
    pub objects: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ObjectParams {
    /// Map name; omitted = the open map.
    pub map: Option<String>,
    /// The entity's or zone's uid (from get_map).
    pub uid: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TilesetParams {
    /// Tileset id.
    pub tileset: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ViewImageParams {
    /// "tileset" or "sheet".
    pub kind: String,
    /// The tileset's or sprite sheet's id.
    pub id: String,
    /// Draw cell borders and tile / frame indices over the image (default true).
    pub grid: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// Map name; omitted = the open map.
    pub map: Option<String>,
    /// Region (tiles) to frame; omitted = the whole map.
    pub x: Option<u32>,
    pub y: Option<u32>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    /// Zoom (1 = 32 px per tile); omitted = fit the region into the canvas.
    pub zoom: Option<f32>,
    /// Show the tile grid / collision overlay / zones and edges (defaults: as the person set them).
    pub grid: Option<bool>,
    pub collision: Option<bool>,
    pub zones: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FillSpec {
    pub tileset: String,
    /// Tile index in the tileset (row * columns + column).
    pub index: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateMapParams {
    /// Map name: letters, digits, '_' and '-'.
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Fill the ground layer with this tile.
    pub fill: Option<FillSpec>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetTilesParams {
    pub map: Option<String>,
    /// Layer index (0 = Ground, 1 = Detail, 2 = Overhead in new maps; see get_map).
    pub layer: usize,
    pub tileset: String,
    /// Cells as [x, y, index]; index -1 erases the cell.
    pub cells: Vec<[i64; 3]>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PaintGridParams {
    pub map: Option<String>,
    pub layer: usize,
    /// Top-left tile of the grid.
    pub x: i32,
    pub y: i32,
    pub tileset: String,
    /// Glyph (one character, not '.') to tile index; -1 erases.
    pub legend: BTreeMap<String, i64>,
    /// Rows of glyphs, as get_map prints them; '.' leaves a cell as it is.
    pub rows: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FillRectParams {
    pub map: Option<String>,
    pub layer: usize,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub tileset: String,
    /// Tile index; -1 erases.
    pub index: i64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PaintTerrainParams {
    pub map: Option<String>,
    /// Layer index (default 0).
    pub layer: Option<usize>,
    /// Terrain name (tileset_legend lists them).
    pub terrain: String,
    /// Cells [x, y] to paint, and / or a rect [x, y, w, h].
    pub cells: Option<Vec<[i32; 2]>>,
    pub rect: Option<[u32; 4]>,
    /// Remove the terrain there instead (default false).
    pub erase: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetCollisionParams {
    pub map: Option<String>,
    pub cells: Option<Vec<[i32; 2]>>,
    pub rect: Option<[u32; 4]>,
    /// "auto" (from the tiles), "solid" or "walkable".
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RectParams {
    pub map: Option<String>,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResizeParams {
    pub map: Option<String>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlaceEntityParams {
    pub map: Option<String>,
    /// "prop", "character" or "item".
    pub kind: String,
    /// Definition id.
    pub def: String,
    /// Tile the entity stands on.
    pub x: i32,
    pub y: i32,
    /// Name scripts and triggers refer to (optional).
    pub name: Option<String>,
    /// "down", "left", "right" or "up".
    pub facing: Option<String>,
    /// A prop's state (default: the definition's).
    pub state: Option<String>,
    /// Stack size of an item pickup.
    pub count: Option<u32>,
    /// Further fields as a merge patch on the entity JSON (triggers, container, shop, schedule, …).
    pub patch: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpdateParams {
    pub map: Option<String>,
    pub uid: u32,
    /// JSON merge patch on the object as get_object returns it (`uid` can't change; entities take `tile_x` / `tile_y`).
    pub patch: Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddZoneParams {
    pub map: Option<String>,
    pub name: String,
    /// ZoneKind JSON: "Spawn", "Trigger", {"Portal":{"target_map":..,"target_spawn":..}}, {"Spawner":{"character":..,"count":..,"respawn":"Never"|"OnEnter"|{"After":{"seconds":..}}}}, {"Transition":{"target_map":..,"target_zone":..}}, {"FishingSpot":{"rod":..}}.
    pub kind: Value,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Triggers (JSON array, see events_reference).
    pub triggers: Option<Value>,
    /// Loot table of a spawner / fishing spot (JSON array of {item, chance, min, max}).
    pub loot: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LinkMapsParams {
    pub map: Option<String>,
    /// Transition rect on `map` [x, y, w, h].
    pub rect: [u32; 4],
    pub to_map: String,
    /// Transition rect on `to_map`.
    pub to_rect: [u32; 4],
    /// Zone name on both sides (default: made from the map names).
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TriggersParams {
    pub map: Option<String>,
    /// Who owns the triggers: {"map":true}, {"entity":UID}, {"zone":UID} or {"def":{"kind":"item"|"prop"|"character","id":..}}.
    pub owner: Value,
    /// The complete trigger list (JSON, see events_reference).
    pub triggers: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AddTriggerParams {
    pub map: Option<String>,
    /// See set_triggers.
    pub owner: Value,
    /// One trigger (JSON).
    pub trigger: Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MapSettingsParams {
    pub map: Option<String>,
    /// Merge patch over {background: [r,g,b], music: sound id, lighting: {mode: "Bright"|"Outdoor"|"Dark", darkness}, fog: bool, weather: {kind: "None"|"Rain"|"Snow"|"Fog", intensity}, parallax: [..], edges: {north|south|east|west: {target_map, offset, depth} | null}}.
    pub patch: Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlaceStructureParams {
    pub map: Option<String>,
    /// Structure id (list_defs structures).
    pub structure: String,
    /// Top-left tile.
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenameParams {
    /// "tileset", "sheet", "item", "prop", "character", "sound", "map", "structure", "skill", "spell" or "recipe".
    pub kind: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListDefsParams {
    /// "tilesets", "sheets", "items", "props", "characters", "sounds", "quests", "structures", "skills", "spells", "recipes" or "achievements".
    pub kind: String,
    /// Only ids / names containing this text.
    pub filter: Option<String>,
    /// First row of the page (pages have 200 rows).
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DefParams {
    /// Definition kind (see list_defs).
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DefTemplateParams {
    pub kind: String,
    /// Id of the new definition (default "new_<kind>").
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UpsertDefParams {
    pub kind: String,
    /// The definition as JSON (get_def / def_template shape). With `merge`, only the fields to change (plus `id`).
    pub def: Value,
    /// Merge into the existing definition instead of replacing it (default false).
    pub merge: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeleteDefParams {
    pub kind: String,
    pub id: String,
    /// Delete even when maps, scripts or other definitions still use it (default false).
    pub force: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct EventsRefParams {
    /// "events", "conditions" or "actions" (default: all three).
    pub what: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadScriptParams {
    /// File name in scripts/ (e.g. "main.rhai").
    pub file: String,
    /// Line range (1-based, inclusive); default: the whole file.
    pub from_line: Option<usize>,
    pub to_line: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WriteScriptParams {
    /// File name in scripts/ ("*.rhai"; created if new).
    pub file: String,
    /// The whole file's text.
    pub text: String,
    /// Overwrite even if the person has unsaved edits to this file open in the editor (default false).
    pub force: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct CheckScriptParams {
    /// One file; default: all scripts.
    pub file: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FilterParams {
    /// Only entries containing this text.
    pub filter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LangParams {
    /// Language code like "de", "fr", "pt-br".
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadTranslationParams {
    pub code: String,
    /// Only texts without a translation yet (default false).
    pub missing_only: Option<bool>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WriteTranslationParams {
    pub code: String,
    /// Original text to translation (merged into the file; other entries stay).
    pub texts: BTreeMap<String, String>,
    /// Translations of the game runner's own texts ("English text": "translation"; `{}` keeps a value).
    pub ui: Option<BTreeMap<String, String>>,
    /// The language's name in its own language (e.g. "Deutsch").
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlayParams {
    /// Autoplay commands separated by ';' (see the autoplay resource), e.g. "wait 2; press e; wait 2; status; shot a". `quit` is appended.
    pub script: String,
    /// Start on this map (default: the open map).
    pub map: Option<String>,
    /// Spawn zone to start at; else `pos`; else the map's "start" spawn or its centre.
    pub spawn: Option<String>,
    /// Tile to start at [x, y].
    pub pos: Option<[i32; 2]>,
    /// Start a new game from the project's start map instead (default false).
    pub from_start: Option<bool>,
    /// Show the title screen first (default false: the game starts right away).
    pub title: Option<bool>,
    /// Language code for the run.
    pub lang: Option<String>,
    /// Seconds before the game is stopped (default 60, at most 300).
    pub timeout_secs: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct LibraryListParams {
    /// "tileset", "sheet", "item", "prop", "character", "sound", "structure", "skill", "spell" or "recipe"; default all.
    pub kind: Option<String>,
    pub filter: Option<String>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetId {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LibraryAddParams {
    /// Library assets to copy into the project (with everything they need).
    pub assets: Vec<AssetId>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LibrarySendParams {
    /// Project assets to send to the asset library (with everything they need).
    pub assets: Vec<AssetId>,
    /// Replace the library's version of assets it already has (same id: definition, files, script functions) instead of adding the project's version under a new id. Default false.
    pub replace: Option<bool>,
    /// Leave the triggers of items, props and characters that run the project's script functions behind (story scripts stay with the game); skills' and spells' functions still travel. Default false.
    pub drop_script_triggers: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WriteAssetParams {
    /// Project-relative path of the file: under sprites/, tilesets/ or sounds/, ending in .png, .wav or .ogg. An existing file is replaced.
    pub path: String,
    /// The file's contents in Base64 …
    pub base64: Option<String>,
    /// … or the absolute path of a file on this computer to copy.
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CaptureStructureParams {
    /// Map to capture from (default: the open map).
    pub map: Option<String>,
    /// Top-left tile and size of the region.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Id of the new structure.
    pub id: String,
    pub name: Option<String>,
    /// Category in the editor's Structures palette (e.g. "Buildings").
    pub category: Option<String>,
    /// Include the ground layer (default true; false for things that sit on any ground, e.g. a market stall).
    pub ground: Option<bool>,
}

// ---------------------------------------------------------------- the tools

type R = Result<CallToolResult, ErrorData>;

#[tool_router(vis = "pub(crate)")]
impl Handler {
    /// The open project at a glance: name, start map, player, languages, every map with its size
    /// and spawns, definition counts, scripts, the editor's state and the project check. Start here.
    #[tool(annotations(read_only_hint = true))]
    async fn project_info(&self) -> R {
        self.run(Call::ProjectInfo, String::new()).await
    }

    /// The editor's state: open map and selection, unsaved changes, undo / redo, open script, last status message.
    #[tool(annotations(read_only_hint = true))]
    async fn editor_state(&self) -> R {
        self.run(Call::EditorState, String::new()).await
    }

    /// Saves the open map, the definitions and pixel art (write tools already save; use after undo / redo).
    #[tool(annotations(idempotent_hint = true))]
    async fn save(&self) -> R {
        self.run(Call::Save, String::new()).await
    }

    /// Undoes the last edit(s) of the open map (yours or the person's). The map is then unsaved: call save.
    #[tool]
    async fn undo(&self, Parameters(p): Parameters<UndoParams>) -> R {
        self.run(Call::Undo(p), String::new()).await
    }

    /// Redoes undone edit(s) of the open map.
    #[tool]
    async fn redo(&self, Parameters(p): Parameters<UndoParams>) -> R {
        self.run(Call::Redo(p), String::new()).await
    }

    /// Changes project settings (project.ron) with a merge patch: name, start_map, start_spawn, player, language, languages, day_minutes, start_hour, inventory_size, creation, sfx, skills, progression, dialog_portraits, …
    #[tool(annotations(idempotent_hint = true))]
    async fn set_project_config(&self, Parameters(p): Parameters<PatchParams>) -> R {
        let s = summary(&p);
        self.run(Call::SetProjectConfig(p), s).await
    }

    /// Every map: size, tilesets, entity and zone counts, spawn zones and transitions to other maps.
    #[tool(annotations(read_only_hint = true))]
    async fn list_maps(&self) -> R {
        self.run(Call::ListMaps, String::new()).await
    }

    /// A map region as text: one grid per layer with a legend (glyph = tileset#index, terrain name, solid), the collision grid, and the entities and zones there with their uids. At most 80x50 tiles per call.
    #[tool(annotations(read_only_hint = true))]
    async fn get_map(&self, Parameters(p): Parameters<GetMapParams>) -> R {
        let s = summary(&p);
        self.run(Call::GetMap(p), s).await
    }

    /// One entity or zone as JSON (the shape update_entity / update_zone patch).
    #[tool(annotations(read_only_hint = true))]
    async fn get_object(&self, Parameters(p): Parameters<ObjectParams>) -> R {
        let s = summary(&p);
        self.run(Call::GetObject(p), s).await
    }

    /// A tileset's tiles: columns and count (index = row * columns + column), solid tiles, animations, light tiles and its terrains (for paint_terrain). view_image shows it.
    #[tool(annotations(read_only_hint = true))]
    async fn tileset_legend(&self, Parameters(p): Parameters<TilesetParams>) -> R {
        let s = summary(&p);
        self.run(Call::TilesetLegend(p), s).await
    }

    /// The image of a tileset or sprite sheet (PNG), with cell borders and indices drawn in by default.
    #[tool(annotations(read_only_hint = true))]
    async fn view_image(&self, Parameters(p): Parameters<ViewImageParams>) -> R {
        let s = summary(&p);
        self.run(Call::ViewImage(p), s).await
    }

    /// A rendered picture (PNG) of a map region as the editor draws it, optionally with grid, collision overlay and zones.
    #[tool(annotations(read_only_hint = true))]
    async fn screenshot_map(&self, Parameters(p): Parameters<ScreenshotParams>) -> R {
        let s = summary(&p);
        self.run(Call::ScreenshotMap(p), s).await
    }

    /// A picture (PNG) of the whole editor window as the person sees it.
    #[tool(annotations(read_only_hint = true))]
    async fn screenshot_editor(&self) -> R {
        self.run(Call::ScreenshotEditor, String::new()).await
    }

    /// Creates a map (layers Ground, Detail, Overhead), saves it and opens it in the editor.
    #[tool]
    async fn create_map(&self, Parameters(p): Parameters<CreateMapParams>) -> R {
        let s = summary(&p);
        self.run(Call::CreateMap(p), s).await
    }

    /// Sets single tiles of a layer ([x, y, index] cells; index -1 erases). One undo step.
    #[tool]
    async fn set_tiles(&self, Parameters(p): Parameters<SetTilesParams>) -> R {
        let s = summary(&p);
        self.run(Call::SetTiles(p), s).await
    }

    /// Paints a block of tiles from text rows with a glyph legend (the format get_map prints); '.' leaves a cell unchanged.
    #[tool]
    async fn paint_grid(&self, Parameters(p): Parameters<PaintGridParams>) -> R {
        let s = summary(&p);
        self.run(Call::PaintGrid(p), s).await
    }

    /// Fills a rectangle of a layer with one tile (index -1 erases).
    #[tool]
    async fn fill_rect(&self, Parameters(p): Parameters<FillRectParams>) -> R {
        let s = summary(&p);
        self.run(Call::FillRect(p), s).await
    }

    /// Paints a terrain (auto-tiling: edges and corners pick themselves) over cells and / or a rect; erase removes it.
    #[tool]
    async fn paint_terrain(&self, Parameters(p): Parameters<PaintTerrainParams>) -> R {
        let s = summary(&p);
        self.run(Call::PaintTerrain(p), s).await
    }

    /// Overrides collision for cells / a rect: "solid", "walkable" or back to "auto" (from the tiles).
    #[tool]
    async fn set_collision(&self, Parameters(p): Parameters<SetCollisionParams>) -> R {
        let s = summary(&p);
        self.run(Call::SetCollision(p), s).await
    }

    /// Clears a rectangle: all layers' tiles, collision overrides and the entities standing there.
    #[tool(annotations(destructive_hint = true))]
    async fn clear_region(&self, Parameters(p): Parameters<RectParams>) -> R {
        let s = summary(&p);
        self.run(Call::ClearRegion(p), s).await
    }

    /// Resizes a map (the top-left area is kept).
    #[tool]
    async fn resize_map(&self, Parameters(p): Parameters<ResizeParams>) -> R {
        let s = summary(&p);
        self.run(Call::ResizeMap(p), s).await
    }

    /// Places a prop, character or item pickup on a tile; returns its uid and JSON.
    #[tool]
    async fn place_entity(&self, Parameters(p): Parameters<PlaceEntityParams>) -> R {
        let s = summary(&p);
        self.run(Call::PlaceEntity(p), s).await
    }

    /// Changes a placed entity with a merge patch (name, tile_x / tile_y, facing, state, triggers, container, lock, shop, schedule, patrol, equipment, …).
    #[tool(annotations(idempotent_hint = true))]
    async fn update_entity(&self, Parameters(p): Parameters<UpdateParams>) -> R {
        let s = summary(&p);
        self.run(Call::UpdateEntity(p), s).await
    }

    /// Removes a placed entity.
    #[tool(annotations(destructive_hint = true))]
    async fn remove_entity(&self, Parameters(p): Parameters<ObjectParams>) -> R {
        let s = summary(&p);
        self.run(Call::RemoveEntity(p), s).await
    }

    /// Adds a zone (spawn point, portal, trigger area, spawner, transition, fishing spot) with optional triggers; returns its uid.
    #[tool]
    async fn add_zone(&self, Parameters(p): Parameters<AddZoneParams>) -> R {
        let s = summary(&p);
        self.run(Call::AddZone(p), s).await
    }

    /// Changes a zone with a merge patch (name, x, y, w, h, kind, triggers, loot).
    #[tool(annotations(idempotent_hint = true))]
    async fn update_zone(&self, Parameters(p): Parameters<UpdateParams>) -> R {
        let s = summary(&p);
        self.run(Call::UpdateZone(p), s).await
    }

    /// Removes a zone.
    #[tool(annotations(destructive_hint = true))]
    async fn remove_zone(&self, Parameters(p): Parameters<ObjectParams>) -> R {
        let s = summary(&p);
        self.run(Call::RemoveZone(p), s).await
    }

    /// Connects two maps with a pair of transition zones (walking into one arrives at the other, keeping the position along it).
    #[tool]
    async fn link_maps(&self, Parameters(p): Parameters<LinkMapsParams>) -> R {
        let s = summary(&p);
        self.run(Call::LinkMaps(p), s).await
    }

    /// Replaces the triggers of a map, a placed entity, a zone or a definition (dialogs, choices, quests, scripts…; events_reference has the shapes).
    #[tool(annotations(idempotent_hint = true))]
    async fn set_triggers(&self, Parameters(p): Parameters<TriggersParams>) -> R {
        let s = summary(&p);
        self.run(Call::SetTriggers(p), s).await
    }

    /// Appends one trigger to a map, entity, zone or definition.
    #[tool]
    async fn add_trigger(&self, Parameters(p): Parameters<AddTriggerParams>) -> R {
        let s = summary(&p);
        self.run(Call::AddTrigger(p), s).await
    }

    /// Changes a map's settings with a merge patch: background colour, music, lighting, fog of war, weather, parallax layers, edge links.
    #[tool(annotations(idempotent_hint = true))]
    async fn set_map_settings(&self, Parameters(p): Parameters<MapSettingsParams>) -> R {
        let s = summary(&p);
        self.run(Call::SetMapSettings(p), s).await
    }

    /// Stamps a structure (a ready-made multi-tile piece: house, well, …) onto a map at a top-left tile.
    #[tool]
    async fn place_structure(&self, Parameters(p): Parameters<PlaceStructureParams>) -> R {
        let s = summary(&p);
        self.run(Call::PlaceStructure(p), s).await
    }

    /// Makes a new structure from a map region: its tiles (all layers), collision and entities, ready for place_structure.
    #[tool]
    async fn capture_structure(&self, Parameters(p): Parameters<CaptureStructureParams>) -> R {
        let s = summary(&p);
        self.run(Call::CaptureStructure(p), s).await
    }

    /// Renames a definition or map and every reference to it (maps, definitions, scripts).
    #[tool]
    async fn rename(&self, Parameters(p): Parameters<RenameParams>) -> R {
        let s = summary(&p);
        self.run(Call::Rename(p), s).await
    }

    /// Lists the definitions of one kind: id, name and key facts, one per line (paged).
    #[tool(annotations(read_only_hint = true))]
    async fn list_defs(&self, Parameters(p): Parameters<ListDefsParams>) -> R {
        let s = summary(&p);
        self.run(Call::ListDefs(p), s).await
    }

    /// One definition as JSON (the shape upsert_def takes).
    #[tool(annotations(read_only_hint = true))]
    async fn get_def(&self, Parameters(p): Parameters<DefParams>) -> R {
        let s = summary(&p);
        self.run(Call::GetDef(p), s).await
    }

    /// A complete, valid definition of a kind to fill in and pass to upsert_def, with notes on its fields.
    #[tool(annotations(read_only_hint = true))]
    async fn def_template(&self, Parameters(p): Parameters<DefTemplateParams>) -> R {
        let s = summary(&p);
        self.run(Call::DefTemplate(p), s).await
    }

    /// Creates or replaces a definition (or merges fields into it); checks the ids and files it refers to, then saves.
    #[tool(annotations(idempotent_hint = true))]
    async fn upsert_def(&self, Parameters(p): Parameters<UpsertDefParams>) -> R {
        let s = summary(&p);
        self.run(Call::UpsertDef(p), s).await
    }

    /// Deletes a definition unless something still uses it (then it lists the users; `force` deletes anyway). Files are kept.
    #[tool(annotations(destructive_hint = true))]
    async fn delete_def(&self, Parameters(p): Parameters<DeleteDefParams>) -> R {
        let s = summary(&p);
        self.run(Call::DeleteDef(p), s).await
    }

    /// The event system's JSON shapes: trigger events per owner, every condition and every action with example values.
    #[tool(annotations(read_only_hint = true))]
    async fn events_reference(&self, Parameters(p): Parameters<EventsRefParams>) -> R {
        let s = summary(&p);
        self.run(Call::EventsReference(p), s).await
    }

    /// The project's script files with the functions each defines.
    #[tool(annotations(read_only_hint = true))]
    async fn list_scripts(&self) -> R {
        self.run(Call::ListScripts, String::new()).await
    }

    /// A script file's text (or a line range).
    #[tool(annotations(read_only_hint = true))]
    async fn read_script(&self, Parameters(p): Parameters<ReadScriptParams>) -> R {
        let s = summary(&p);
        self.run(Call::ReadScript(p), s).await
    }

    /// Writes a whole script file (Rhai), then checks it: compile errors and unknown ids are reported.
    #[tool(annotations(destructive_hint = true))]
    async fn write_script(&self, Parameters(p): Parameters<WriteScriptParams>) -> R {
        let s = summary(&p);
        self.run(Call::WriteScript(p), s).await
    }

    /// Checks the scripts: compile errors, unknown item / map / quest / … ids, duplicate functions.
    #[tool(annotations(read_only_hint = true))]
    async fn check_script(&self, Parameters(p): Parameters<CheckScriptParams>) -> R {
        let s = summary(&p);
        self.run(Call::CheckScript(p), s).await
    }

    /// The scripting API: every engine function with its parameters and a description, plus the project's own functions.
    #[tool(annotations(read_only_hint = true))]
    async fn scripting_api(&self, Parameters(p): Parameters<FilterParams>) -> R {
        let s = summary(&p);
        self.run(Call::ScriptingApi(p), s).await
    }

    /// Adds a language to the project and writes lang/<code>.ron with every text to translate (existing translations are kept).
    #[tool]
    async fn translation_template(&self, Parameters(p): Parameters<LangParams>) -> R {
        let s = summary(&p);
        self.run(Call::TranslationTemplate(p), s).await
    }

    /// The texts of a translation file (original → translation), optionally only the untranslated ones.
    #[tool(annotations(read_only_hint = true))]
    async fn read_translation(&self, Parameters(p): Parameters<ReadTranslationParams>) -> R {
        let s = summary(&p);
        self.run(Call::ReadTranslation(p), s).await
    }

    /// Writes translations into lang/<code>.ron (merged; the language is added to the project if new).
    #[tool(annotations(idempotent_hint = true))]
    async fn write_translation(&self, Parameters(p): Parameters<WriteTranslationParams>) -> R {
        let s = summary(&p);
        self.run(Call::WriteTranslation(p), s).await
    }

    /// Saves everything and runs the game with an autoplay script (keys, clicks, `status`, `shot NAME` screenshots); returns the game's output lines and the screenshots. The game opens a window on the person's screen.
    #[tool]
    async fn play_test(&self, Parameters(p): Parameters<PlayParams>) -> R {
        let s = summary(&p);
        self.run(Call::PlayTest(p), s).await
    }

    /// Assets in the shared library (ready-made tiles, props, characters, items, sounds, structures…) with what each needs.
    #[tool(annotations(read_only_hint = true))]
    async fn library_list(&self, Parameters(p): Parameters<LibraryListParams>) -> R {
        let s = summary(&p);
        self.run(Call::LibraryList(p), s).await
    }

    /// One library asset as JSON with its dependencies.
    #[tool(annotations(read_only_hint = true))]
    async fn library_info(&self, Parameters(p): Parameters<DefParams>) -> R {
        let s = summary(&p);
        self.run(Call::LibraryInfo(p), s).await
    }

    /// Copies library assets (and everything they need: sheets, sounds, script functions) into the project.
    #[tool]
    async fn library_add(&self, Parameters(p): Parameters<LibraryAddParams>) -> R {
        let s = summary(&p);
        self.run(Call::LibraryAdd(p), s).await
    }

    /// Sends project assets (and everything they need) to the shared asset library; with replace, the library's versions of the same ids are updated.
    #[tool]
    async fn library_send(&self, Parameters(p): Parameters<LibrarySendParams>) -> R {
        let s = summary(&p);
        self.run(Call::LibrarySend(p), s).await
    }

    /// Writes an image (PNG) or sound (WAV / Ogg Vorbis) file into the project (sprites/, tilesets/, sounds/) and reloads the art; then point a definition at it with upsert_def.
    #[tool]
    async fn write_asset(&self, Parameters(p): Parameters<WriteAssetParams>) -> R {
        let s = format!("{} ({})", p.path, if p.base64.is_some() { "base64" } else { "copy" });
        self.run(Call::WriteAsset(p), s).await
    }
}

// ---------------------------------------------------------------- prompts

/// The prompts offered to agents.
pub fn prompts() -> Vec<Prompt> {
    vec![
        Prompt::new(
            "design_quest",
            Some("Design and build a quest: definitions, dialogs, triggers, scripts, then a play-test"),
            Some(vec![
                PromptArgument::new("theme").with_description("What the quest is about").with_required(true),
                PromptArgument::new("giver").with_description("The character who gives it (id or name; optional)"),
                PromptArgument::new("reward").with_description("Items, xp or skills the player earns (optional)"),
            ]),
        ),
        Prompt::new(
            "build_map",
            Some("Build a new map: terrain, structures, props, a spawn, links to other maps, then a screenshot"),
            Some(vec![
                PromptArgument::new("name").with_description("Map name (letters, digits, _ -)").with_required(true),
                PromptArgument::new("setting").with_description("What the place is: a forest clearing, a harbour town, a cave…").with_required(true),
                PromptArgument::new("width").with_description("Tiles (default 40)"),
                PromptArgument::new("height").with_description("Tiles (default 30)"),
                PromptArgument::new("connect_to").with_description("An existing map to link with a transition (optional)"),
            ]),
        ),
    ]
}

/// The text of prompt `name` with the arguments filled in.
pub fn prompt_text(name: &str, arg: &dyn Fn(&str) -> String) -> Option<String> {
    match name {
        "design_quest" => Some(format!(
            "Design and build a quest for this R(ust)PG2D project.\nTheme: {theme}\nQuest giver: {giver}\nReward: {reward}\n\n\
Work in these steps, reading before writing:\n\
1. `project_info`, then `list_defs characters`, `list_defs items`, `list_defs quests` to see what exists and the naming style.\n\
2. `def_template quests` and `upsert_def quests` for the quest (title, description, 2-4 stages, xp).\n\
3. `events_reference` for the shapes, then `set_triggers` on the giver (find them with `get_map` / `get_object`): an Interact trigger with Dialog, a Choice, Quest{{op: Start}}; further triggers gated by Quest conditions for the later stages and the reward (GiveItem, GiveXp, Quest{{op: Complete}}).\n\
4. Anything the stages need on the maps: items to find (`place_entity`), a zone with an Enter trigger (`add_zone`), an enemy to defeat (a Spawner zone + Defeated condition).\n\
5. Script logic that triggers can't express: `write_script` into scripts/<quest>.rhai and `check_script`.\n\
6. `play_test` with a script that talks to the giver (`press e` twice per dialog page, `wait 2.5` between) and ends with `status` to confirm `quests=` shows the quest; take a `shot`.\n\
Keep every id lowercase with underscores; use {{player}} in dialog texts for the player's name.",
            theme = arg("theme"),
            giver = if arg("giver").is_empty() { "choose a fitting character from the project".to_string() } else { arg("giver") },
            reward = if arg("reward").is_empty() { "something modest that fits the theme".to_string() } else { arg("reward") },
        )),
        "build_map" => Some(format!(
            "Build a new map in this R(ust)PG2D project.\nName: {name}\nSetting: {setting}\nSize: {w} x {h} tiles\nConnect to: {connect}\n\n\
Steps:\n\
1. `project_info` and `list_maps`; `tileset_legend` of the tilesets in use (terrains are the easiest way to paint ground), `list_defs structures` and `list_defs props`.\n\
2. `create_map` with a ground fill, then `paint_terrain` for paths / water / grass variety and `paint_grid` or `fill_rect` for details; keep the Overhead layer for what should draw above characters.\n\
3. `place_structure` for buildings and `place_entity` for props and characters (give important ones names); `set_collision` where tiles alone don't block.\n\
4. `add_zone` a Spawn named \"start\" (and others for arrivals), then `link_maps` to the map to connect, with matching rects on both edges.\n\
5. `set_map_settings` for music, lighting (Outdoor / Dark) and weather.\n\
6. `screenshot_map` to look at the result and fix what looks off; `play_test` with `wait 1; status; shot a` to see it in the game.",
            name = arg("name"),
            setting = arg("setting"),
            w = if arg("width").is_empty() { "40".to_string() } else { arg("width") },
            h = if arg("height").is_empty() { "30".to_string() } else { arg("height") },
            connect = if arg("connect_to").is_empty() { "nothing yet".to_string() } else { arg("connect_to") },
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documentation/mcp.md names every tool, and every tool has a description.
    #[test]
    fn docs_list_every_tool() {
        let doc = include_str!("../../../../Documentation/mcp.md");
        let tools = Handler::tool_router().list_all();
        assert!(tools.len() >= 50, "{} tools", tools.len());
        for t in &tools {
            assert!(t.description.as_deref().is_some_and(|d| !d.is_empty()), "{} has no description", t.name);
            assert!(doc.contains(&format!("`{}", t.name)), "Documentation/mcp.md doesn't mention `{}`", t.name);
        }
        for p in prompts() {
            assert!(doc.contains(&p.name), "Documentation/mcp.md doesn't mention the prompt {}", p.name);
            assert!(prompt_text(&p.name, &|_| String::new()).is_some());
        }
    }

    /// Documentation/autoplay.md names every autoplay command of the game runner.
    #[test]
    fn autoplay_docs_are_complete() {
        let src = include_str!("../../../game/src/main.rs");
        let doc = include_str!("../../../../Documentation/autoplay.md");
        let start = src.find("while let Some(cmd) = self.cmds.pop_front()").expect("the autoplay loop");
        let body = &src[start..];
        let end = body.find("\n        }\n").unwrap_or(body.len());
        let mut names = Vec::new();
        for line in body[..end].lines() {
            let t = line.trim();
            if t.starts_with('"') && t.contains("=>") {
                for part in t.split("=>").next().unwrap_or("").split('|') {
                    let n = part.trim().trim_matches('"');
                    if !n.is_empty() {
                        names.push(n.to_string());
                    }
                }
            }
        }
        assert!(names.len() > 20, "{names:?}");
        for n in names {
            assert!(doc.contains(&format!("`{n}")) || doc.contains(&format!(" {n} ")) || doc.contains(&format!("`{n} ")) || doc.contains(&format!("{n}\\|")) || doc.contains(&format!("\\|{n}")), "Documentation/autoplay.md doesn't mention `{n}`");
        }
    }
}
