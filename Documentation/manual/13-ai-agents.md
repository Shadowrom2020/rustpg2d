# 13. AI agents

The editor has a built-in **AI agent server**. It lets an AI assistant such as Claude Code, Cursor or
Claude Desktop work in your open project: read and draw maps, place characters, write triggers,
quests and scripts, create art and sounds, translate, and play-test the result. You keep the editor
open and watch; every change appears at once and is one undo step (Ctrl+Z).

## Starting the server and connecting

*Assets › AI agent server…* in the editor: *Start* listens on `http://127.0.0.1:7743/mcp`
(Streamable HTTP; the port is adjustable, *Start with the editor* remembers the choice in the
editor's settings). The window shows ready-made client configurations with *Copy* buttons:

| Client | Configuration |
|---|---|
| Claude Code | `claude mcp add --transport http rustpg2d http://127.0.0.1:7743/mcp` |
| Cursor, VS Code | `.cursor/mcp.json` / `.vscode/mcp.json`: `{"mcpServers": {"rustpg2d": {"url": "http://127.0.0.1:7743/mcp"}}}` |
| Claude Desktop, other stdio-only clients | `{"mcpServers": {"rustpg2d": {"command": "npx", "args": ["-y", "mcp-remote", "http://127.0.0.1:7743/mcp"]}}}` (needs Node.js) |

The window also lists every call the agents made (tool, time, ok / error, the parameters).
Editor script commands for tests: `mcp on [PORT]`, `mcp off`, `mcpwindow`; `status` prints
`mcp=127.0.0.1:7743 calls=N` or `mcp=off`.

**Security.** The server binds to the loopback address only and has no authentication:
anything on the same computer that can reach the port can edit the open project. Stop the
server when you don't use it; don't forward the port.

## Asking

Then ask in plain words, for example: "Add a fisherman to the village who sells trout in the morning
and fishes at the pond in the afternoon", or "Build a small cave map behind the waterfall with three
bats and a chest".

## Conventions the tools share

- **Coordinates are tiles**: origin top-left, x to the right, y down. Rectangles are `x, y, w, h`.
  An entity's position is the tile its feet stand on (`tile_x` / `tile_y` in its JSON; the engine
  keeps pixels in `x` / `y`). Tiles are 32 px.
- **`map`** may be omitted in map tools: then the map open in the editor is meant. Naming another
  map saves the open one and opens the named one (the editor follows the agent).
- **Layers** are indices: new maps have `0` Ground, `1` Detail, `2` Overhead (drawn above
  characters); `get_map` prints the names.
- **Ids** are strict. An unknown id is an error that lists known ids containing the text.
- **JSON shapes** are serde's form of the engine's types: externally tagged enums
  (`"Spawn"`, `{"Portal": {"target_map": "village", "target_spawn": "start"}}`,
  `{"Dialog": {"speaker": "Elder", "text": "Hello, {player}."}}`). What `get_def`,
  `get_object` and `events_reference` return is valid input for `upsert_def`, `update_*` and the
  trigger tools. Patches are JSON merge patches (RFC 7396): fields replace, `null` removes, nested
  objects merge, arrays replace.
- **Writes** validate first (ids, files, trigger events allowed for the owner), then apply, save
  the project, refresh the editor's suggestions and return what changed plus the project check's
  warnings. Each write is one undo step; `undo` / `redo` then `save` are tools too.
- **Replies are bounded**: grids at most 80x50 tiles per call (ask for regions), listings paged by
  200 rows (`offset`), text cut at 48 k characters, images at most 1280 px and 6 per play-test.
- Writes are refused while the editor waits for the person to answer its *Unsaved changes*
  question, and nothing works without an open project (the welcome screen).

## Tools

### Project and session
| Tool | Does |
|---|---|
| `project_info` | Name, start map / spawn, player, languages, every map with size and spawns, definition counts, scripts, the editor's state, the project check. Start here. |
| `editor_state` | Open map, selection, unsaved changes, undo / redo, open script, last status message, server state. |
| `save` | Saves the open map, definitions and pixel art (writes already save). |
| `undo {steps}` / `redo {steps}` | Steps on the open map (then unsaved: call `save`). |
| `set_project_config {patch}` | project.ron: `name`, `start_map`, `start_spawn`, `player`, `language`, `languages`, `day_minutes`, `start_hour`, `inventory_size`, `creation`, `sfx`, `skills`, `progression`, `dialog_portraits`, … (`tile_size` can't change). |

### Reading maps
| Tool | Does |
|---|---|
| `list_maps` | Size, tilesets, layers, entity / zone counts, spawns and where each map leads. |
| `get_map {map, x, y, w, h, layers, collision, objects}` | A region as text: one glyph grid per layer with column / row rulers and a legend (`a = terrain#235 terrain "Dirt path" solid x25`), the collision grid (`#` blocked, `.` free, `S` / `W` overrides) and the objects there (`E uid kind def "name" @x,y …`, `Z uid "name" kind @x,y wxh`). Whole map when it fits 80x50. |
| `get_object {map, uid}` | An entity or zone as JSON. |
| `tileset_legend {tileset}` | Columns and tile count (index = row · columns + column), solid tiles, animations, light tiles, terrains. |
| `view_image {kind, id, grid}` | The PNG of a tileset or sprite sheet, with cell borders drawn in (small images are shown enlarged). |
| `edit_frames {sheet, op, anim, frame, row, to_row, dx, dy, from, to, scope, hand, x, y, angle, size, front, all}` | Frame operations on a sheet's image, for animating figures: `mirror_rows` (Right rows = the mirrored Left rows; of `anim` or all), `copy_row` / `clear_row`, `duplicate` (a frame into the next column), `nudge` (move a frame's pixels), `replace_color` (`from` → `to` in a `frame`, `row` or the whole `sheet`), `fill_standing` (empty idle / hurt frames get the standing frame). Hand marks of figures ([Art › Hands](10-art-and-sound.md#hands)): `set_grip` (`frame`, `hand` right / left, `x`, `y` in the frame's pixels, `angle` = the grip axis in degrees, 0 right, 90 down; `size`, `front`), `clear_grips` (a `frame`'s, or all), `estimate_hands` (frames without marks, `all` for every frame), `swap_hands` (right and left trade places in a `row`, or in every facing of an `anim`). The operations keep marks in step with the pixels. Writes the PNG (and the sheet's marks) and reloads the art. |
| `sheet_colors {sheet}` | The exact colours of a sprite sheet's image, most used first, with pixel counts, the animation rows they appear in (figures) and the colour feature they belong to: the shades to put into a figure's colour features (`upsert_def` sheet `features`). |
| `screenshot_map {map, x, y, w, h, zoom, grid, collision, zones}` | A PNG of the region as the editor draws it. |
| `screenshot_editor` | A PNG of the whole editor window. |

### Writing maps (one undo step each)
| Tool | Does |
|---|---|
| `create_map {name, width, height, fill}` | A new map (layers Ground / Detail / Overhead), saved and opened. |
| `set_tiles {map, layer, tileset, cells: [[x, y, index]]}` | Single tiles; index −1 erases. |
| `paint_grid {map, layer, x, y, tileset, legend, rows}` | Tiles from text rows with a glyph legend — the format `get_map` prints; `.` leaves a cell unchanged. |
| `fill_rect {map, layer, x, y, w, h, tileset, index}` | One tile over a rectangle. |
| `paint_terrain {map, layer, terrain, cells, rect, erase}` | Auto-tiling terrain (edges and corners pick themselves). |
| `set_collision {map, cells, rect, value}` | `solid`, `walkable` or back to `auto`. |
| `clear_region {map, x, y, w, h}` | Empties all layers, overrides and entities in a rectangle. |
| `resize_map {map, width, height}` | Keeps the top-left area. |
| `place_entity {map, kind, def, x, y, name, facing, state, count, patch}` | A prop, character or item pickup; returns its uid and JSON. Like the editor, a prop an even number of tiles wide stands on the tile edge between `x` and `x + 1` (covering both); its `tile_x` then reads `x + 1`. |
| `update_entity {map, uid, patch}` | Merge patch (`tile_x` / `tile_y` move it; `uid` is fixed). |
| `remove_entity {map, uid}` | |
| `add_zone {map, name, kind, x, y, w, h, triggers, loot}` | `kind`: `"Spawn"`, `"Trigger"`, `{"Portal": {target_map, target_spawn}}`, `{"Spawner": {character, count, respawn}}`, `{"Transition": {target_map, target_zone}}`, `{"FishingSpot": {rod}}`. A taken name gets a suffix. |
| `update_zone {map, uid, patch}` / `remove_zone {map, uid}` | |
| `link_maps {map, rect, to_map, to_rect, name}` | A pair of transition zones between two maps (the other map is written to disk directly). |
| `set_triggers {map, owner, triggers}` / `add_trigger {map, owner, trigger}` | `owner`: `{"map": true}`, `{"entity": UID}`, `{"zone": UID}` or `{"def": {"kind": "item"\|"prop"\|"character", "id": …}}`. Events must suit the owner (`events_reference`); ids in actions and conditions must exist; functions not defined yet are warnings. |
| `set_map_settings {map, patch}` | `background`, `music`, `lighting {mode: Bright\|Outdoor\|Dark, darkness}`, `fog`, `weather {kind: None\|Rain\|Snow\|Fog, intensity}`, `parallax`, `edges {north\|south\|east\|west: {target_map, offset, depth} \| null}`. |
| `place_structure {map, structure, x, y}` | Stamps a structure (tiles, collision, entities). |
| `capture_structure {map, x, y, w, h, id, name, category, ground}` | A new structure from a map region: its tiles (all layers; `ground: false` leaves the ground layer out), collision overrides and entities. |
| `rename {kind, old, new}` | A definition or map and every reference to it, scripts included. |

### Definitions
| Tool | Does |
|---|---|
| `list_defs {kind, filter, offset}` | `tilesets`, `sheets`, `items`, `props`, `characters`, `sounds`, `quests`, `structures`, `skills`, `spells`, `recipes`, `achievements`, `races`: id and key facts per line. |
| `get_def {kind, id}` | JSON. |
| `def_template {kind, id}` | A complete new definition to fill in, with notes on the fields. |
| `upsert_def {kind, def, merge}` | Create or replace (or merge fields into) a definition; checks referenced ids, image / sound files and the definition's own triggers; tilesets, sheets and sounds reload the editor's textures. |
| `find_usages {kind, id}` | Every place a definition (or a map: kind `map`) is used: placed entities and spawners, map / zone / entity triggers, other definitions, script lines, project settings, each with its map and uid, definition or script line. |
| `delete_def {kind, id, force}` | Refuses while definitions, maps or scripts use it unless `force`; files are kept. |
| `write_asset {path, base64 \| source}` | Writes an image (`.png`) or sound (`.wav`, `.ogg`) under `sprites/`, `tilesets/` or `sounds/` — from Base64 or by copying a file on this computer — after checking that it decodes, and reloads the art. Replacing a file updates every definition that uses it; a new file needs a definition (`upsert_def`). |
| `events_reference {what}` | Allowed events per owner, every condition and action as JSON with example values. |

### Scripts
| Tool | Does |
|---|---|
| `list_scripts` | Files in `scripts/` with their functions. |
| `read_script {file, from_line, to_line}` | |
| `write_script {file, text, force}` | Writes a whole `.rhai` file (refuses to overwrite a dirty file open in the editor unless `force`), then checks it. |
| `check_script {file}` | Compile errors, unknown ids (items, maps, quests, …), duplicate functions. |
| `problems {severity, area}` | The editor's *Problems* list: everything broken in the project (unknown ids in definitions, maps and scripts, missing files, broken portals, script errors, unreachable maps, characters without a body, untranslated texts), each with its place; `severity` `error` / `warning`, `area` `project`, `definitions`, `maps`, `scripts` or `translations`. |
| `scripting_api {filter}` | Every engine function with parameter kinds and description, plus the project's functions. The resource `rustpg2d://docs/scripting` is the full reference. |

### Languages
| Tool | Does |
|---|---|
| `translation_template {code}` | Adds the language and writes `lang/<code>.ron` with every text (existing translations kept). |
| `read_translation {code, missing_only, offset}` | Original → translation pairs. |
| `write_translation {code, texts, ui, name}` | Merges translations in (new texts of the project become keys first). |

### Play-testing
`play_test {script, map, spawn, pos, from_start, title, lang, timeout_secs}` saves everything and
runs the game (`rustpg2d-game … --dev`) with an autoplay script: commands separated by `;` such as
`wait 2`, `press e`, `hold right 1`, `click 6 3`, `type Name`, `status`, `shot NAME` — see the
resource `rustpg2d://docs/autoplay` ([Testing your game](17-testing.md)). `quit` is appended. The reply has the game's output lines
(`[autoplay] …` status lines with the player's position, hp, quests, inventory, flags, …) and the
screenshots as images. The game opens a real window on the person's screen for the run.

### Asset library
| Tool | Does |
|---|---|
| `library_list {kind, filter, offset}` | Ready-made assets (tilesets, sheets, items, props, characters, sounds, structures, skills, spells, recipes) with what each needs and whether it is in the project. |
| `library_info {kind, id}` | One asset as JSON with its dependencies. |
| `library_add {assets: [{kind, id}]}` | Copies assets with everything they need (sheets, sounds, script functions) into the project. |
| `library_send {assets: [{kind, id}], replace, drop_script_triggers}` | Sends project assets with everything they need to the library. An id the library has already gets a numbered copy, or with `replace` its definition, files and script functions are updated. `drop_script_triggers` leaves the triggers of items, props and characters that run the project's script functions behind (story scripts stay with the game). The library is `$RUSTPG2D_LIBRARY` or the user's own one. |

### Drawing art
With a Meshy key in *Edit › Settings… › Meshy*, agents draw art in the project's style the way
the Art window does ([Art and sound › Generate art](10-art-and-sound.md#generate-art-with-meshy)).
Pictures cost the person's credits: `generate_art` without `confirm_credits` only names the price,
so an assistant asks before it spends, and the session's budget holds for agents too. Every job
also shows in the editor's Art window and status bar.

| Tool | Does |
|---|---|
| `art_settings` | Whether a key is set (never the key), the model and its price per picture, the session's budget and what is left, character sizes and attacks, which sheets set the style. |
| `generate_art {kind, id, description, size, attack, tiles, portrait_for, confirm_credits}` | Starts a job (`character`, `icon`, `prop`, `portrait`, `image`; `portrait_for`: the character whose dialog portrait it becomes) and returns its number at once; refuses unless `confirm_credits` equals the price, or past the budget. |
| `art_jobs` | Every job of the session: its pictures and their progress, credits, done or failed, added as which asset. |
| `art_result {job}` | A finished job as images: the converted art (characters: the 16-row sheet) and Meshy's raw pictures, to judge before adding it. |
| `art_apply {job, id}` | Adds it to the project: the image, its sprite sheet and a character / item / prop using it; people's figures (drawn with empty hands) get estimated hand marks (check them with `view_image` and fix them with `edit_frames`). Saves. |
| `art_redo {job, picture, confirm_credits}` | Draws one picture (`walk`, `attack`, `picture`) again and converts again. |

In a chat this reads like: *"Draw an orc shaman with a skull staff for the swamp camp."* The
assistant checks `art_settings`, tells you the price (6 credits with an attack), starts the job
once you agree, watches `art_jobs`, looks at `art_result`, redraws a bad picture if you want, and
adds it with `art_apply`, then places the new character on a map.

## Resources and prompts

Resources: `rustpg2d://docs/mcp` (this chapter), `rustpg2d://docs/scripting`
([Scripting](08-scripting.md)), `rustpg2d://docs/events` ([Events](07-events.md)),
`rustpg2d://docs/autoplay` ([Testing your game](17-testing.md)), `rustpg2d://project` (the project
summary). Prompts: `design_quest (theme, giver, reward)` and `build_map (name, setting, width,
height, connect_to)` — step-by-step instructions over the tools above.

## Files changed on disk

Agents (and other programs) may also write the project's files directly. The editor checks
`project.ron`, `defs/*.ron`, `maps/*.map`, `scripts/*.rhai` and `lang/*.ron` once a second:
when a changed file has no unsaved edits in the editor, it is reloaded with a status note
(definitions, the open map, the open script, the map list). When it has, a yellow banner asks
*Reload (discard my edits)* or *Keep mine* (saving later overwrites the file). The editor's own
saves never trigger this. Editor script: `watchanswer reload|keep`; `status` prints
`watch=clean` or `watch=changed:<files>`.

## Troubleshooting

- *Port in use*: pick another port in the window; clients then need the new URL.
- *403*: the server accepts requests addressed to `127.0.0.1` / `localhost` only (Host header).
- *play_test: game not found*: the game runner must sit next to the editor (packaged downloads do;
  from source run `cargo build -p rustpg2d-game`). A warning about a different engine build means
  the game was built from another checkout: rebuild it.
- *Flatpak*: the editor's sandbox has network access, so the port is reachable from the host;
  the game for play-tests runs inside the sandbox like F5 does.
- *the editor did not answer within 120 s*: the editor's main thread was busy or blocked (a
  modal dialog, a long reload). The call is not cancelled; it may still run later. The limit is
  `RUSTPG2D_MCP_TIMEOUT` seconds in the editor's environment.
- *internal error in TOOL … panicked*: a bug in the tool (please report it with the call). The
  editor keeps running; the call's undo step is closed, so read the map or definition again.
- Nothing works: is a project open? `project_info` says so.
