# R(ust)PG2D architecture

How the engine, the game runner and the editor fit together: the technology, the data model, the
runtime's frame, the editor and its agent server, the file formats and their compatibility rules,
the verification tooling, and the caveats worth knowing. Written for people (and agents) changing
the code; [README.md](../README.md) is the user manual, [CLAUDE.md](../CLAUDE.md) the working rules
and the module-by-module map.

## 1. Technology

| Crate | Lines (0.11) | Role |
|---|---|---|
| `crates/engine` | ~35 000 | library shared by game and editor: definitions, maps, save games, the runtime (`runtime/`), rendering, scripting, library bundles, exports |
| `crates/game` | ~550 | the game binary: command line, window, the `RUSTPG2D_AUTOPLAY` runner |
| `crates/editor` | ~26 000 | the egui editor, the asset library window, the AI agent (MCP) server |
| `crates/packaging` | ~500 | AppImage and Flatpak bundles from an `AppSpec` |
| `crates/xtask` | ~700 | `cargo xtask check-version / check-branch / release-notes`, `cargo xtask-dist` |

Rust 2021, clippy with `-D warnings`. The pinned stack and why:

- **macroquad 0.4.16** (`audio` feature): window, OpenGL drawing, input, sound. Everything the game
  draws goes through `draw_texture_ex`; text is macroquad's default font (Latin-1 only).
- **egui-macroquad 0.17.3** = egui 0.31, with **egui-phosphor 0.9** (icon font). Later egui-phosphor
  versions need a newer egui, so all four are pinned and excluded from Dependabot.
- **rhai 1.27**: the scripting language (`script.rs`); the editor also uses it to parse JSON imports.
- **serde + ron 0.12**: every text format (project, definitions, map objects, saves, settings,
  translations). **postcard + lz4_flex**: the binary tile data inside map files.
- **png 0.18** (`imageio.rs`) for images the editor and library decode themselves; macroquad keeps
  its own png 0.17 for textures. **lewton** decodes Ogg Vorbis music so loops can be gapless.
- **libm** supplies the math functions newer glibc re-versioned (`glibc_compat.rs`), so release
  binaries built on a rolling distribution still need only glibc 2.34.
- Editor only: **rfd** (xdg-portal file dialogs), **quick-xml** + **flate2** (Tiled imports),
  **rmcp 3** + **tokio** + **axum 0.8** + **schemars** (the MCP server), **rustpg2d-packaging**.
- Targets: Linux (AppImage, Flatpak), Windows (MinGW cross-build), macOS (universal app, signed
  ad hoc), the game also for `wasm32-unknown-unknown` (`sys.rs` abstracts files and the clock).

Build profiles (`Cargo.toml`): `dev` compiles our crates at opt-level 1 and dependencies at 2, so
debug builds play at full speed; CI's test job overrides it to 0 for speed.

## 2. Data model

**Project** = a folder: `project.ron` (`ProjectConfig`: name, player, inventory size, skills,
languages, …), `defs/*.ron` (twelve lists, `project::DEFS`), `maps/*.map`, `sprites/`, `tilesets/`,
`sounds/`, `scripts/*.rhai`, `lang/<code>.ron`, and the ignored `saves/`. `Project::load` reads it
all into `Database` (`defs.rs`), one `Vec` per kind, referenced by string ids everywhere
(`refactor.rs` knows every reference for renames and library exports: `refs_of_*`).

**Maps** (`map/`): `Map` = size, tileset slots, tile layers, a collision grid, entities, zones,
edge links, triggers and settings (music, lighting, fog, weather, parallax). Tiles live in
`TileLayer`s of sparse 32×32-tile chunks (`Option<Box<Chunk>>`), maps up to 1024×1024 tiles. The
file (`map/io.rs`) is `MAGIC` + version + lz4 of a postcard `Core` (name, size, tileset slots,
layers, collision, and a RON string `Meta` with everything else). Format version 2 has been the
format since 0.0.1; v1 (all postcard) is still decoded.

**Save games** (`state.rs`): `WorldState` as RON in `saves/<slot>.ron` (plus `autosave`): flags,
variables, inventory, equipment, quests, shops, containers, per-map entity states, dropped items,
fired once-triggers, companions, explored fog, the clock, the player's look. Everything the player
can change lives here; nothing else is persisted.

**Settings** (`settings.rs`): per user and game, `~/.config/rustpg2d/<game>/settings.ron`
(`RUSTPG2D_SETTINGS_DIR` overrides). Achievements unlocked go next to it.

**Asset library** (`library.rs`): a project folder without maps at
`~/.local/share/rustpg2d/library` (`RUSTPG2D_LIBRARY` overrides). The editor's starter set is the
committed folder `crates/editor/assets/starter_library/`, embedded by `crates/editor/build.rs` and
applied to a library when its `starter.stamp` (rules version + hash of the files) differs: new ids
are added, images the new set only extends are replaced, structures wait for their tileset art.
`.r2dpack` files are zip bundles of assets with a manifest, for sharing between projects.

**Translations** (`lang.rs`): `lang/<code>.ron` with `texts` (content: dialogs, names, quest texts,
collected by `content_texts`) and `ui` (runtime strings; English is the key). The game swaps the
`Database` for a translated copy (`translate_db`).

## 3. The runtime (`crates/engine/src/runtime/`)

`Game` (`mod.rs`) owns the project (with the translated `Database` and the original), `Assets`,
`Audio`, `world: WorldState` (what gets saved), `loaded: Loaded` (the current map's runtime state:
map, per-tile `solid` and `ladder` flags, the `entities`, tile lights), a cache of the last four
maps, the player's `Body`, the `ActionRunner` and the UI windows' states.

`RtEntity` is a flat struct per character, prop or dropped item on the current map: kind,
definition id, uid, `Body` (feet position, facing, hitbox, speed, animation timer), combat, patrol,
schedule, companion, visitor, AI and status-effect state. **Indices are valid within a frame only;
anything that lives longer stores a uid.** Uid ranges: placed entities are small numbers, spawned
characters start at `1 << 30`, companions at `0x5000_0000`, visitors (scheduled characters from
other maps) at `0x5800_0000`, dropped items at `0x6000_0000`. `persist(i)` writes an entity's
state back into `world.entities[map]`.

### Frame

`Game::frame` polls the gamepad, takes `dt = frame time, capped at 50 ms` (variable step), then
`update(dt)` and `draw()`. `update` runs, in order:

1. screen shake, thumbnail capture, the script-side player mirror;
2. modal screens that pause the world and return early: console, character creator, minigame,
   sleeping, "heal whom?", orders menu, pause menu, world map;
3. play time, achievements, the clock (`advance_hours`, crops regrow), *Time reached* triggers;
4. debug toggle, quick save / quick load;
5. the fade state machine (`finish_transition` switches the map at full black);
6. pending events, autosave, game over;
7. UI windows take input first, then the mouse (`mouse.rs`: click-to-move with A*, right-click
   interact, clickable UI);
8. the action runner (taken out with `mem::take`; actions queued meanwhile are appended);
9. `frozen` = runner busy, fading or modal;
10. `update_player`, fog reveal, `update_entities`, `update_combat`, projectiles, spawners,
    effects and weather, gathering.

`update_entities` handles every character by priority: scripted `move_target`, item use,
knockback, speed effects, companion, chase AI, schedule, activity, patrol, then a wander fallback.
Walking characters share `walk_to_tile` (`patrol.rs`): an A* path (`path.rs`, budget per
waypoint), one tile at a time through `try_move`, with re-planning when blocked and giving up
after a few seconds. The player moves through `try_move` too: solid tiles, solid entities and the
player block; ladders block characters and are climbed by the player (vertical only, slower).

`draw` runs: pixel camera, parallax layers behind, tile layers below entities, fish shadows,
entities and the player Y-sorted, combat / gathering / activity / FX / mouse overlays, layers above
entities, parallax above, the light pass when it is dark or the weather tints, weather, the save
thumbnail, the debug overlay, the fade, HUD, windows, dialog, menus, console.

The windows share their building blocks in `runtime/ui.rs`: `ScrollList` (selection, scrolling,
double-click; the trade, crafting and menu lists), `slot_grid` with `DragState` (the item slots of
the inventory and companion windows, one drag-and-drop state machine), `draw_item_detail` (the
item footer), dialogs and toasts. A new window composes these instead of copying a loop.

### Maps, events, scripts

Map changes: `transition(map, arrival)` starts a fade; `finish_transition` → `take_map` (cache or
disk) → `enter_map`: the old map's patrols are persisted, `solid` / `ladder` / tile lights are
computed from the layers, entities are built (placed ones with their saved state, companions,
visitors, dropped items), scheduled characters are placed where their day says, the arrival spot
is made free, spawners and regrowth run, music starts, *Map enter* fires. `load_game` replaces the
world and re-enters the save's map the same way, without *Map enter*.

Events: `fire(event, source)` collects matching triggers (definition, instance, companion and
visitor triggers), checks `once` keys in `world.fired` and the conditions, and pushes the actions
to the `ActionRunner` (`events/runner.rs`). The runner handles waits, `If`, variables and items
itself and sends the rest to `Game::execute`, which answers Continue, Block (a dialog, a walk) or
Insert (more actions). Scripts (`script.rs`) run in a Rhai engine limited to 2 million operations
and 64 call levels; the `WorldState` is moved into the script context and back around each call.
Queued script functions (`say`, `walk_to`, …) become actions on the runner; immediate ones read and
change state directly.

### Rendering and assets

`render/mod.rs` draws the tiles in view one `draw_texture_ex` each (sparse chunks skip empty
areas; macroquad batches draws by texture). Characters are layered (`render/character.rs`): body,
clothing, armor, hair, beard, held items, each recolored from reference ramps (`recolor.rs`) and
cached per sheet, skin and hair colour. `render/lighting.rs` renders the lights into a
half-resolution target (ambient colour, one additive glow per light in view) and multiplies it
over the scene. `Assets::load` loads every tileset and sheet up front, computing per-tile average
colours (the world map) and the solid / ladder flags; a missing image becomes a checkerboard.

## 4. The editor (`crates/editor/src/`)

`app.rs` holds the `Editor`: the open project, the open map as a `MapDoc` (`doc.rs`, with undo
groups), the view, the tools (`canvas.rs`), the Database forms (`database.rs`), the library views,
structures, the character builder, imports and exports. Script commands (`RUSTPG2D_EDITOR_SCRIPT`,
`main.rs`) call the same functions the UI calls, which is how the editor is tested.

The **AI agent server** (`mcp/`) is an MCP server (rmcp, Streamable HTTP on `127.0.0.1:7743`) on a
tokio thread. Tool calls are queued to the main thread and executed between frames (`mcp_drain`:
up to 40 calls or 8 ms per frame), so a tool sees the editor's state like a user action; every
write is one undo step and saves the project. Screenshots are taken two frames later. A file
watcher polls the project once a second (mtime and size) and reloads what changed on disk when the
editor has no unsaved edits to it. `play_test` runs the sibling `rustpg2d-game` binary with an
autoplay script and returns its status lines and screenshots. The 54 tools are declared once in
`tools.rs` (`mcp_tools!`: description, annotations, parameters and the editor function each one
runs); the `Call` enum, the tool names and the main-thread dispatch are generated from that block.
Scripts are read by one lexer (`script_refs::lex` in the engine): the editor's highlighter, the
reference finder, the function lists and completion all work on its tokens.

## 5. Formats and compatibility rules

Projects, maps, definitions and saves of older versions must keep loading (AGENTS.md). What that
means per format:

- **RON structs** (definitions, map `Meta`, saves, settings, project): new fields get
  `#[serde(default)]` (or `default = "fn"`); never rename a field without an alias; variants of
  enums stored by name (`Action`, `Cond`, `TriggerOn`, `ZoneKind`, `OnUnlock` in v2 maps and in
  definitions) may be added anywhere by format, but are **appended** by convention because v1
  maps stored them by index.
- **postcard** (map `Core`, `TileLayer`, `Chunk`): positional and without defaults. **These
  structs are frozen**: a new layer or chunk property goes into the RON `Meta`, or the format
  version is bumped with a reader for the old one.
- **Saves**: `WorldState` fields all default; unknown fields are ignored, so a newer save loads in
  an older game minus what it does not know.
- **Libraries**: existing libraries only gain new ids from a changed starter set (images that are
  pure extensions are replaced); bump `STARTER_RULES` when the way starter sets are applied changes.
- **Script API**: functions are never removed or change meaning within a major version; a new one
  is registered in `script.rs` and described once in the `API` table of `script_api.rs` (parameter
  kinds: `Arg::Id` for ids, `Arg::Line` for a line players read, `Arg::Text` for other words),
  which drives the editor's completion, renames, the script check, translation templates and the
  flag / variable collectors; `Documentation/scripting.md` must describe it too (a test checks).

## 6. Verification tooling

| Tool | What it drives | Where it runs |
|---|---|---|
| `RUSTPG2D_AUTOPLAY="…"` | the game: key presses, clicks, saves, `status` (a line with every runtime state field), screenshots | every feature is verified with it; `Documentation/autoplay.md` |
| `RUSTPG2D_EDITOR_SCRIPT="…"` | the editor: tools, Database tabs, library, exports, MCP on/off, screenshots | the same |
| `crates/editor/examples/mcp_smoke.rs` | a real MCP client against the running editor: tools, resources, prompts, `write_asset`, `upsert_def`, `capture_structure`, `library_send`, `play_test` | by hand with a scratch library |
| `cargo test --workspace` | unit tests: formats, library merges, scripting, translations, starter-library consistency; `crates/engine/tests/compat.rs` loads files of earlier versions (`tests/fixtures/`) | CI on every pull request |
| `scripts/ci/smoke.sh` | the game and the editor on a scratch copy of the example project: title, a map, the clock, an old save, quick save, a map change with its autosave, the editor opening a map, the MCP smoke client | CI's *Runtime smoke* job (Xvfb) on every pull request; locally with a display |
| CI (`.github/workflows/ci.yml`) | *Branch policy*, *Build, lint, test* on every PR; *Version*, *Packages* (release builds, AppImage/Flatpak/Windows/macOS, an Xvfb smoke run of game and editor), *Windows*, *macOS*, *macOS app* on PRs into master | GitHub Actions |
| Release workflow | tags, publishes the packages, opens the back-merge PR | merges into master |

## 7. Caveats and performance characteristics

Known properties of the current implementation (see the CHANGELOG for what each release changed):

- Tiles are drawn one quad each, every frame; there is no mesh cache. Large views at high zoom-out
  cost proportionally more. Entities outside the view (with a margin for tall sprites) are
  skipped.
- Text uses macroquad's default font: Latin-1 glyphs only, measured per draw.
- The hot definition lookups (`db.item(id)`, `prop`, `character`, `sheet`, `tileset`) go through
  an id index that verifies its hits and falls back to a scan, so an index left stale by an
  in-place edit is slower, never wrong; `Database::reindex` refreshes it.
- Pathfinding (`path.rs`) runs on the tile grid with a node budget per search and tells "no
  route" from "out of budget"; walkers (`patrol.rs::walk_to_tile`, shared by patrols, schedules,
  companions and chasers walking home) re-plan once per blockage and give up after four seconds.
  Planning and the broad phase of collision read the per-frame occupancy grid
  (`runtime/occupancy.rs`); code that moves an entity by more than a tile within a frame calls
  `occupy(i)`.
- Pixel ↔ tile conversions follow `grid.rs` (feet one pixel above the tile's bottom edge; props an
  even number of tiles wide stand on a tile edge).
- `status` prints frame timing (`fps=`, `frame_ms=`, `update_ms=`, `draw_ms=`) and the number of
  path searches (`astar=`) since the previous `status`; `RUSTPG2D_PERF=1` logs them every second.
- Files are written whole or not at all (`sys::write`: temporary file + rename). A definition
  entry or a map object that can't be read is left out with a log line, and the editor backs the
  file up before it could save without it.
- The recolored-character cache is never evicted (bounded by the sheets × colours actually used).
- The editor's egui UI has no automated click tests; editor script commands call the same
  functions instead. The game and the editor do run in CI (`scripts/ci/smoke.sh`).
- A game binary older than the engine cannot read new map data; the editor compares
  `rustpg2d-game --engine-build` with its own fingerprint (the engine sources plus the versions
  of the crates that define the file formats).
