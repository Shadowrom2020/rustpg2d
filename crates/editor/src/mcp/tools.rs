//! The tool catalogue agents see: one thin `#[tool]` per tool that turns its parameters into a
//! [`Call`] for the main thread. The parameter structs' doc comments become the JSON schema
//! descriptions, so they are written for the agent. Also the prompts.
//!
//! Conventions (also in `Documentation/manual/13-ai-agents.md`): coordinates are tiles (origin top-left, x right,
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

use super::exec::{Reply, ShotSpec};
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
pub struct GenerateArtParams {
    /// "model" (a rigged, animated 3D character: a person), "creature" (an unrigged 3D model of an animal or creature; animate it with fit_rig) or "gear" (the 3D model of the existing item `id`: with a description from a clean picture drawn first, its icon as the reference; without one from the icon); pictures: "icon", "prop", "portrait" or "image".
    pub kind: String,
    /// Id the asset will get (letters, digits, _).
    pub id: String,
    /// What it looks like: build, colours, clothes, what it holds (characters), the object (icons, props).
    pub description: String,
    /// 3D characters: "none", "slash", "chop", "thrust", "bow", "club" or "cast" (default none).
    pub attack: Option<String>,
    /// Props: width and height in tiles (1-4, default 1 x 1).
    pub tiles: Option<[u32; 2]>,
    /// Portraits: the character id whose dialog portrait it becomes when applied.
    pub portrait_for: Option<String>,
    /// 3D characters ("model"): a woman (her sitting clip; the model is marked as a woman's).
    pub female: Option<bool>,
    /// 3D characters ("model"): extra animations, 3 credits each: "craft" (working at an anvil), "farm" (bending to pick crops), "chop" (chopping wood), "sleep" (lying in a bed).
    pub extras: Option<Vec<String>>,
    /// 3D characters and creatures ("model", "creature"): a figure sheet id whose standing frames (front, side, back) are modelled, instead of drawing a turnaround picture first.
    pub from: Option<String>,
    /// Must equal the cost in credits that art_settings / a refused call names: the creator's money.
    pub confirm_credits: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ArtJobParams {
    /// The job number (art_jobs lists them).
    pub job: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ArtApplyParams {
    pub job: u32,
    /// Id of the new asset (default: the job's id).
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ArtRedoParams {
    pub job: u32,
    /// "walk", "attack" or "picture"; in 3D jobs "views", "model", "rig" or "clips" (the steps after it are made again too).
    pub picture: String,
    /// Must equal the cost a refused call names (one picture; in 3D jobs the step and the steps after it).
    pub confirm_credits: Option<u32>,
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditFramesParams {
    /// The sprite sheet's id.
    pub sheet: String,
    /// "mirror_rows" (Right rows = mirrored Left rows), "copy_row", "clear_row", "duplicate"
    /// (a frame into the next column), "nudge" (move a frame's pixels), "replace_color",
    /// "fill_standing" (empty idle / hurt frames get the standing walk frame); figures: "set_grip",
    /// "clear_grips" (a frame's, or all), "estimate_hands", "swap_hands" (right and left grips trade
    /// places in a row, or in every facing of an anim).
    pub op: String,
    /// mirror_rows / swap_hands: the animation ("walk", "idle", "attack", "hurt"); mirror_rows: omitted = all.
    pub anim: Option<String>,
    /// duplicate / nudge / replace_color (scope "frame"): the frame (row * columns + column).
    pub frame: Option<u32>,
    /// copy_row: the source row; clear_row / replace_color (scope "row") / swap_hands: the row.
    pub row: Option<u32>,
    /// copy_row: the target row.
    pub to_row: Option<u32>,
    /// nudge: pixels right / down (negative: left / up).
    pub dx: Option<i32>,
    pub dy: Option<i32>,
    /// replace_color: the colour to replace and the new one, [r, g, b].
    pub from: Option<[u8; 3]>,
    pub to: Option<[u8; 3]>,
    /// replace_color: "frame", "row" or "sheet" (default).
    pub scope: Option<String>,
    /// set_grip: "right" or "left"; the grip in the frame's image pixels, the direction a held
    /// item points (degrees: 0 right, 90 down, -90 up), its size (default 1) and whether it is in
    /// front of the figure (default true).
    pub hand: Option<String>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub angle: Option<f32>,
    pub size: Option<f32>,
    pub front: Option<bool>,
    /// estimate_hands: estimate every frame again (default: only frames without grips).
    pub all: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportModelParams {
    /// A `.glb` file on this computer (absolute path): copied into the project's models/.
    pub path: String,
    /// Add its clips to this existing model instead of making a new one.
    #[serde(default)]
    pub clips_for: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ModelInfoParams {
    /// The 3D model's id.
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FitRigParams {
    /// The unrigged 3D model (an animal, a monster).
    pub model: String,
    /// A rigged model whose skeleton and clips it takes: the starter library's rig_quadruped, rig_bird, rig_flyer, rig_blob, rig_floating, or any rigged model; "-" removes the template. Omit to keep it.
    #[serde(default)]
    pub template: Option<String>,
    /// Turns the model about its up axis first (degrees) so it faces the front like the template. Omit to keep.
    #[serde(default)]
    pub turn: Option<f32>,
    /// Joints moved after the fit (replaces the list): offsets in parts of the model's height, x right, y up, z front. Omit to keep.
    #[serde(default)]
    pub joints: Option<Vec<JointNudgeParam>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct JointNudgeParam {
    pub joint: String,
    pub offset: [f32; 3],
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SheetColorsParams {
    /// The sprite sheet's id.
    pub sheet: String,
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
    /// "tilesets", "sheets", "items", "props", "characters", "sounds", "quests", "structures", "skills", "spells", "recipes", "achievements" or "races".
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
pub struct ProblemsParams {
    /// Only "error" or "warning" entries; default: both.
    pub severity: Option<String>,
    /// Only one area: "project", "definitions", "maps", "scripts" or "translations"; default: all.
    pub area: Option<String>,
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

/// The tool catalogue, once: each tool's description (its doc comment) and annotations, its
/// name, its [`Call`] variant with the parameter type, the expression the editor's main thread
/// runs for it (`ed` is the [`Editor`](crate::app::Editor), `p` the parameters) and, where the
/// generic one won't do, the summary for the call log. This generates the `#[tool]` methods of
/// [`Handler`], the [`Call`] enum, [`Call::name`] and `Editor::mcp_exec`, so a new tool is one
/// entry here plus its `mcp_*` function.
macro_rules! mcp_tools {
    (@summary $p:ident) => {
        summary(&$p)
    };
    (@summary $p:ident, $s:expr) => {
        $s
    };
    (
        plain ( $ed:ident ) { $( $(#[$pa:meta])* $pname:ident : $pvar:ident => $pexec:expr ; )* }
        with_params { $( $(#[$wa:meta])* $wname:ident : $wvar:ident ( $pb:ident : $wparams:ty ) => $wexec:expr $(, summary = $wsum:expr)? ; )* }
    ) => {
        /// A tool call (the parameter structs are those above), executed on the main thread.
        pub enum Call {
            $( $pvar, )*
            $( $wvar($wparams), )*
        }

        impl Call {
            /// The tool's name (the call log).
            pub fn name(&self) -> &'static str {
                match self {
                    $( Call::$pvar => stringify!($pname), )*
                    $( Call::$wvar(_) => stringify!($wname), )*
                }
            }
        }

        #[tool_router(vis = "pub(crate)")]
        impl Handler {
            $(
                $(#[$pa])*
                async fn $pname(&self) -> R {
                    self.run(Call::$pvar, String::new()).await
                }
            )*
            $(
                $(#[$wa])*
                async fn $wname(&self, Parameters($pb): Parameters<$wparams>) -> R {
                    let s = mcp_tools!(@summary $pb $(, $wsum)?);
                    self.run(Call::$wvar($pb), s).await
                }
            )*
        }

        impl crate::app::Editor {
            /// Runs a tool call on the main thread (between frames, from `mcp_drain`).
            pub fn mcp_exec(&mut self, call: Call) -> Result<Reply, String> {
                let $ed = self;
                match call {
                    $( Call::$pvar => $pexec, )*
                    $( Call::$wvar($pb) => $wexec, )*
                }
            }
        }
    };
}

mcp_tools! {
plain(ed) {
    /// The open project at a glance: name, start map, player, languages, every map with its size
    /// and spawns, definition counts, scripts, the editor's state and the project check. Start here.
    #[tool(annotations(read_only_hint = true))]
    project_info: ProjectInfo => ed.mcp_project_info();
    /// The editor's state: open map and selection, unsaved changes, undo / redo, open script, last status message.
    #[tool(annotations(read_only_hint = true))]
    editor_state: EditorState => ed.mcp_editor_state();
    /// Saves the open map, the definitions and pixel art (write tools already save; use after undo / redo).
    #[tool(annotations(idempotent_hint = true))]
    save: Save => ed.mcp_save();
    /// Every map: size, tilesets, entity and zone counts, spawn zones and transitions to other maps.
    #[tool(annotations(read_only_hint = true))]
    list_maps: ListMaps => ed.mcp_list_maps();
    /// A picture (PNG) of the whole editor window as the person sees it.
    #[tool(annotations(read_only_hint = true))]
    screenshot_editor: ScreenshotEditor => Ok(Reply::Shot(ShotSpec { rect: None, zoom: None, grid: None, collision: None, zones: None, text: "The editor window.".into() }));
    /// The project's script files with the functions each defines.
    #[tool(annotations(read_only_hint = true))]
    list_scripts: ListScripts => ed.mcp_list_scripts();
    /// Generate art with the creator's Meshy account: whether a key is set (never the key), the model and its price per picture, the session's budget and what is left, the sizes and attacks for characters, the prices of 3D steps.
    #[tool(annotations(read_only_hint = true))]
    art_settings: ArtSettings => ed.mcp_art_settings();
    /// Every art job of this session: pictures and their progress, credits spent, finished or failed, added as which asset.
    #[tool(annotations(read_only_hint = true))]
    art_jobs: ArtJobs => ed.mcp_art_jobs();
}
with_params {
    /// Undoes the last edit(s) of the open map (yours or the person's). The map is then unsaved: call save.
    #[tool]
    undo: Undo(p: UndoParams) => ed.mcp_undo(p, false), summary = String::new();
    /// Redoes undone edit(s) of the open map.
    #[tool]
    redo: Redo(p: UndoParams) => ed.mcp_undo(p, true), summary = String::new();
    /// Changes project settings (project.ron) with a merge patch: name, start_map, start_spawn, player, language, languages, day_minutes, start_hour, inventory_size, creation, sfx, skills, progression, dialog_portraits, model_atlas_mb (MB of GPU memory for rendered character poses, 8 to 128), …
    #[tool(annotations(idempotent_hint = true))]
    set_project_config: SetProjectConfig(p: PatchParams) => ed.mcp_set_project_config(p);
    /// A map region as text: one grid per layer with a legend (glyph = tileset#index, terrain name, solid), the collision grid, and the entities and zones there with their uids. At most 80x50 tiles per call.
    #[tool(annotations(read_only_hint = true))]
    get_map: GetMap(p: GetMapParams) => ed.mcp_get_map(p);
    /// One entity or zone as JSON (the shape update_entity / update_zone patch).
    #[tool(annotations(read_only_hint = true))]
    get_object: GetObject(p: ObjectParams) => ed.mcp_get_object(p);
    /// A tileset's tiles: columns and count (index = row * columns + column), solid tiles, animations, light tiles and its terrains (for paint_terrain). view_image shows it.
    #[tool(annotations(read_only_hint = true))]
    tileset_legend: TilesetLegend(p: TilesetParams) => ed.mcp_tileset_legend(p);
    /// The image of a tileset or sprite sheet (PNG), with cell borders and indices drawn in by default.
    #[tool(annotations(read_only_hint = true))]
    view_image: ViewImage(p: ViewImageParams) => ed.mcp_view_image(p);
    /// The exact colours of a sprite sheet's image, most used first, with their pixel counts, the animation rows they appear in (figures) and the colour feature they belong to: pick a part's shades for a figure's colour features (upsert_def sheet `features: [{name, shades, colors}]`).
    #[tool(annotations(read_only_hint = true))]
    sheet_colors: SheetColors(p: SheetColorsParams) => ed.mcp_sheet_colors(p);
    /// Imports a rigged 3D character model (a .glb from Meshy, Blender, Mixamo…) as a new model (its clips matched to the animations walk / idle / attack / hurt / sit / … by name), or with clips_for adds the file's clips to that model. Characters use a model with upsert_def `appearance: {model}`; model_info shows what it has. Saves.
    #[tool]
    import_model: ImportModel(p: ImportModelParams) => ed.mcp_import_model(p);
    /// A 3D model: triangles, bones, hands, its clips with their length, which clip each animation plays, and its texture's colours (most used first, the shades for colour features: upsert_def model `features: [{name, shades, colors}]`).
    #[tool(annotations(read_only_hint = true))]
    model_info: ModelInfo(p: ModelInfoParams) => ed.mcp_model_info(p);
    /// Animates an unrigged model (an animal, a monster: Meshy rigs only people) with a rig template: the template's skeleton is stretched over the model and its walk, idle, attack and hurt clips play. Also turns the model to face the front and moves single joints. Returns the fitted joints; check the result with a play test or the Models tab. Saves.
    #[tool]
    fit_rig: FitRig(p: FitRigParams) => ed.mcp_fit_rig(p);
    /// Edits the frames of a sprite sheet's image and a figure's hand grips (4 columns, 16 rows: walk, idle, attack, hurt; facings down, left, right, up): mirror Left rows into Right rows, copy / clear a row, duplicate a frame into the next column, nudge a frame's pixels, replace a colour (e.g. split a shade two colour features share), fill empty idle / hurt frames with the standing frame, set_grip / clear_grips / estimate_hands (where held weapons, tools and props are drawn). Grips move with the frames. Writes the PNG and the definition and reloads the art; view_image shows the result.
    #[tool]
    edit_frames: EditFrames(p: EditFramesParams) => ed.mcp_edit_frames(p);
    /// Starts drawing art with Meshy (characters in the project's style: a walk grid, plus an attack grid; icons, props, portraits, painted images; 3D: a rigged and animated 3D character, an unrigged creature model, an existing item's 3D model from its icon): returns the job number at once. Costs credits: call without confirm_credits first to learn the price, ask the person, then call with confirm_credits = that price. Watch it with art_jobs, look at it with art_result, add it with art_apply.
    #[tool]
    generate_art: GenerateArt(p: GenerateArtParams) => ed.mcp_generate_art(p);
    /// A finished art job's result as an image (characters: the 16-row sheet; walk / idle / attack / hurt rows) to judge before adding it; the raw Meshy pictures too.
    #[tool(annotations(read_only_hint = true))]
    art_result: ArtResult(p: ArtJobParams) => ed.mcp_art_result(p);
    /// Adds a finished art job to the project: the image and its sprite sheet, and a character / item / prop using it (if none has that id); 3D jobs: the model files (models/), the 3D model and a character using it, or the item's 3D model. Saves.
    #[tool]
    art_apply: ArtApply(p: ArtApplyParams) => ed.mcp_art_apply(p);
    /// Draws one picture of a finished job again (a bad frame, the wrong look) and converts it again; costs one picture (confirm_credits). In 3D jobs it makes a step again with the steps after it.
    #[tool]
    art_redo: ArtRedo(p: ArtRedoParams) => ed.mcp_art_redo(p);
    /// A rendered picture (PNG) of a map region as the editor draws it, optionally with grid, collision overlay and zones.
    #[tool(annotations(read_only_hint = true))]
    screenshot_map: ScreenshotMap(p: ScreenshotParams) => ed.mcp_screenshot_map(p).map(Reply::Shot);
    /// Creates a map (layers Ground, Detail, Overhead), saves it and opens it in the editor.
    #[tool]
    create_map: CreateMap(p: CreateMapParams) => ed.mcp_create_map(p);
    /// Sets single tiles of a layer ([x, y, index] cells; index -1 erases). One undo step.
    #[tool]
    set_tiles: SetTiles(p: SetTilesParams) => ed.mcp_set_tiles(p);
    /// Paints a block of tiles from text rows with a glyph legend (the format get_map prints); '.' leaves a cell unchanged.
    #[tool]
    paint_grid: PaintGrid(p: PaintGridParams) => ed.mcp_paint_grid(p);
    /// Fills a rectangle of a layer with one tile (index -1 erases).
    #[tool]
    fill_rect: FillRect(p: FillRectParams) => ed.mcp_fill_rect(p);
    /// Paints a terrain (auto-tiling: edges and corners pick themselves) over cells and / or a rect; erase removes it.
    #[tool]
    paint_terrain: PaintTerrain(p: PaintTerrainParams) => ed.mcp_paint_terrain(p);
    /// Overrides collision for cells / a rect: "solid", "walkable" or back to "auto" (from the tiles).
    #[tool]
    set_collision: SetCollision(p: SetCollisionParams) => ed.mcp_set_collision(p);
    /// Clears a rectangle: all layers' tiles, collision overrides and the entities standing there.
    #[tool(annotations(destructive_hint = true))]
    clear_region: ClearRegion(p: RectParams) => ed.mcp_clear_region(p);
    /// Resizes a map (the top-left area is kept).
    #[tool]
    resize_map: ResizeMap(p: ResizeParams) => ed.mcp_resize_map(p);
    /// Places a prop, character or item pickup on a tile; returns its uid and JSON.
    #[tool]
    place_entity: PlaceEntity(p: PlaceEntityParams) => ed.mcp_place_entity(p);
    /// Changes a placed entity with a merge patch (name, tile_x / tile_y, facing, state, triggers, container, lock, shop, schedule, patrol, equipment, …).
    #[tool(annotations(idempotent_hint = true))]
    update_entity: UpdateEntity(p: UpdateParams) => ed.mcp_update_entity(p);
    /// Removes a placed entity.
    #[tool(annotations(destructive_hint = true))]
    remove_entity: RemoveEntity(p: ObjectParams) => ed.mcp_remove_entity(p);
    /// Adds a zone (spawn point, portal, trigger area, spawner, transition, fishing spot) with optional triggers; returns its uid.
    #[tool]
    add_zone: AddZone(p: AddZoneParams) => ed.mcp_add_zone(p);
    /// Changes a zone with a merge patch (name, x, y, w, h, kind, triggers, loot).
    #[tool(annotations(idempotent_hint = true))]
    update_zone: UpdateZone(p: UpdateParams) => ed.mcp_update_zone(p);
    /// Removes a zone.
    #[tool(annotations(destructive_hint = true))]
    remove_zone: RemoveZone(p: ObjectParams) => ed.mcp_remove_zone(p);
    /// Connects two maps with a pair of transition zones (walking into one arrives at the other, keeping the position along it).
    #[tool]
    link_maps: LinkMaps(p: LinkMapsParams) => ed.mcp_link_maps(p);
    /// Replaces the triggers of a map, a placed entity, a zone or a definition (dialogs, choices, quests, scripts…; events_reference has the shapes).
    #[tool(annotations(idempotent_hint = true))]
    set_triggers: SetTriggers(p: TriggersParams) => ed.mcp_set_triggers(p);
    /// Appends one trigger to a map, entity, zone or definition.
    #[tool]
    add_trigger: AddTrigger(p: AddTriggerParams) => ed.mcp_add_trigger(p);
    /// Changes a map's settings with a merge patch: background colour, music, lighting, fog of war, weather, parallax layers, edge links.
    #[tool(annotations(idempotent_hint = true))]
    set_map_settings: SetMapSettings(p: MapSettingsParams) => ed.mcp_set_map_settings(p);
    /// Stamps a structure (a ready-made multi-tile piece: house, well, …) onto a map at a top-left tile.
    #[tool]
    place_structure: PlaceStructure(p: PlaceStructureParams) => ed.mcp_place_structure(p);
    /// Makes a new structure from a map region: its tiles (all layers), collision and entities, ready for place_structure.
    #[tool]
    capture_structure: CaptureStructure(p: CaptureStructureParams) => ed.mcp_capture_structure(p);
    /// Renames a definition or map and every reference to it (maps, definitions, scripts).
    #[tool]
    rename: Rename(p: RenameParams) => ed.mcp_rename(p);
    /// Lists the definitions of one kind: id, name and key facts, one per line (paged).
    #[tool(annotations(read_only_hint = true))]
    list_defs: ListDefs(p: ListDefsParams) => ed.mcp_list_defs(p);
    /// One definition as JSON (the shape upsert_def takes).
    #[tool(annotations(read_only_hint = true))]
    get_def: GetDef(p: DefParams) => ed.mcp_get_def(p);
    /// A complete, valid definition of a kind to fill in and pass to upsert_def, with notes on its fields.
    #[tool(annotations(read_only_hint = true))]
    def_template: DefTemplate(p: DefTemplateParams) => ed.mcp_def_template(p);
    /// Creates or replaces a definition (or merges fields into it); checks the ids and files it refers to, then saves.
    #[tool(annotations(idempotent_hint = true))]
    upsert_def: UpsertDef(p: UpsertDefParams) => ed.mcp_upsert_def(p);
    /// Every place a definition (kind: items, props, …) or a map (kind: map) is used: placed entities and spawners, map / zone / entity triggers, other definitions, script lines, the project settings; each with where to find it.
    #[tool(annotations(read_only_hint = true))]
    find_usages: FindUsages(p: DefParams) => ed.mcp_find_usages(p);
    /// Deletes a definition unless something still uses it (then it lists the users; `force` deletes anyway). Files are kept.
    #[tool(annotations(destructive_hint = true))]
    delete_def: DeleteDef(p: DeleteDefParams) => ed.mcp_delete_def(p);
    /// The event system's JSON shapes: trigger events per owner, every condition and every action with example values.
    #[tool(annotations(read_only_hint = true))]
    events_reference: EventsReference(p: EventsRefParams) => ed.mcp_events_reference(p);
    /// A script file's text (or a line range).
    #[tool(annotations(read_only_hint = true))]
    read_script: ReadScript(p: ReadScriptParams) => ed.mcp_read_script(p);
    /// Writes a whole script file (Rhai), then checks it: compile errors and unknown ids are reported.
    #[tool(annotations(destructive_hint = true))]
    write_script: WriteScript(p: WriteScriptParams) => ed.mcp_write_script(p);
    /// Checks the scripts: compile errors, unknown item / map / quest / … ids, duplicate functions.
    #[tool(annotations(read_only_hint = true))]
    check_script: CheckScript(p: CheckScriptParams) => ed.mcp_check_script(p);
    /// Everything broken in the project (the editor's Problems panel): unknown ids in definitions, maps and scripts, missing image / sound files, portals and transitions to missing maps / spawns / zones, script errors, maps nothing leads to, characters without a body, untranslated texts, objects a file had to leave out; errors and warnings, each with where it is.
    #[tool(annotations(read_only_hint = true))]
    problems: Problems(p: ProblemsParams) => ed.mcp_problems(p);
    /// The scripting API: every engine function with its parameters and a description, plus the project's own functions.
    #[tool(annotations(read_only_hint = true))]
    scripting_api: ScriptingApi(p: FilterParams) => ed.mcp_scripting_api(p);
    /// Adds a language to the project and writes lang/<code>.ron with every text to translate (existing translations are kept).
    #[tool]
    translation_template: TranslationTemplate(p: LangParams) => ed.mcp_translation_template(p);
    /// The texts of a translation file (original → translation), optionally only the untranslated ones.
    #[tool(annotations(read_only_hint = true))]
    read_translation: ReadTranslation(p: ReadTranslationParams) => ed.mcp_read_translation(p);
    /// Writes translations into lang/<code>.ron (merged; the language is added to the project if new).
    #[tool(annotations(idempotent_hint = true))]
    write_translation: WriteTranslation(p: WriteTranslationParams) => ed.mcp_write_translation(p);
    /// Saves everything and runs the game with an autoplay script (keys, clicks, `status`, `shot NAME` screenshots); returns the game's output lines and the screenshots. The game opens a window on the person's screen.
    #[tool]
    play_test: PlayTest(p: PlayParams) => super::play::plan(ed, p).map(|pl| Reply::PlayPlan(Box::new(pl)));
    /// Assets in the shared library (ready-made tiles, props, characters, items, sounds, structures…) with what each needs.
    #[tool(annotations(read_only_hint = true))]
    library_list: LibraryList(p: LibraryListParams) => ed.mcp_library_list(p);
    /// One library asset as JSON with its dependencies.
    #[tool(annotations(read_only_hint = true))]
    library_info: LibraryInfo(p: DefParams) => ed.mcp_library_info(p);
    /// Copies library assets (and everything they need: sheets, sounds, script functions) into the project.
    #[tool]
    library_add: LibraryAdd(p: LibraryAddParams) => ed.mcp_library_add(p);
    /// Sends project assets (and everything they need) to the shared asset library; with replace, the library's versions of the same ids are updated.
    #[tool]
    library_send: LibrarySend(p: LibrarySendParams) => ed.mcp_library_send(p);
    /// Writes an image (PNG) or sound (WAV / Ogg Vorbis) file into the project (sprites/, tilesets/, sounds/) and reloads the art; then point a definition at it with upsert_def.
    #[tool]
    write_asset: WriteAsset(p: WriteAssetParams) => ed.mcp_write_asset(p), summary = format!("{} ({})", p.path, if p.base64.is_some() { "base64" } else { "copy" });
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

    /// Documentation/manual/13-ai-agents.md names every tool, and every tool has a description.
    #[test]
    fn docs_list_every_tool() {
        let doc = include_str!("../../../../Documentation/manual/13-ai-agents.md");
        let tools = Handler::tool_router().list_all();
        assert!(tools.len() >= 50, "{} tools", tools.len());
        for t in &tools {
            assert!(t.description.as_deref().is_some_and(|d| !d.is_empty()), "{} has no description", t.name);
            assert!(doc.contains(&format!("`{}", t.name)), "Documentation/manual/13-ai-agents.md doesn't mention `{}`", t.name);
        }
        for p in prompts() {
            assert!(doc.contains(&p.name), "Documentation/manual/13-ai-agents.md doesn't mention the prompt {}", p.name);
            assert!(prompt_text(&p.name, &|_| String::new()).is_some());
        }
    }

    /// Documentation/manual/17-testing.md names every autoplay command of the game runner.
    #[test]
    fn autoplay_docs_are_complete() {
        let src = include_str!("../../../game/src/main.rs");
        let doc = include_str!("../../../../Documentation/manual/17-testing.md");
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
            assert!(doc.contains(&format!("`{n}")) || doc.contains(&format!(" {n} ")) || doc.contains(&format!("`{n} ")) || doc.contains(&format!("{n}\\|")) || doc.contains(&format!("\\|{n}")), "Documentation/manual/17-testing.md doesn't mention `{n}`");
        }
    }
}
