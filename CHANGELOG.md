# Changelog

All notable changes to R(ust)PG2D (formerly rust2d) (engine, game runner and editor). Versions follow
`MAJOR.MINOR.PATCH`; see AGENTS.md for how releases are made.

## Unreleased

### Maintenance
- **Faster releases.** The release builds run at the same time (macOS per architecture, Linux,
  web, Windows) instead of one after the other, and the *Packages* check only assembles them: a
  pull request into master takes about 15 minutes instead of 27. Merging it no longer builds
  everything again: the Release workflow publishes the packages that pull request's CI run built
  (the same tree), so the release and the back-merge pull request follow in about a minute
  instead of 30. Pushes to master and the back-merge only report the checks their trees already
  passed.

## 0.12.0 — 2026-11-09

A quality and reliability release: characters no longer search for paths every frame, files are
written whole or not at all, one bad entry no longer stops a project or map from loading, the
Windows game explains why it can't start, and CI now runs the game and the editor. Much shared
code was merged into one place each (`Documentation/architecture.md` describes the result).
Projects, maps and save games of earlier versions load unchanged.

### Fixes
- **Web exports with an outdated web runner.** The editor packed whatever `rustpg2d-game.wasm` it
  found, also one built from an older engine; the exported game then stopped at "The game could
  not start: loading map …". Every game runner now carries its engine build, and *Export game*
  refuses a runner (web, Linux, Windows, macOS) built from another engine, naming the command that
  rebuilds it. The web game's start-up error shows its whole cause on the page and in the browser
  console (it printed a literal `{e:#}` there).
- **Characters stop searching every frame.** A character blocked on its way planned a new path
  every frame from the first second until it gave up; a companion who found no way to the player
  (up a watchtower ladder, behind a shut door) searched again every frame too. Now one new plan
  per blockage, and companions wait a second between searches. "Can't reach patrol point" is
  logged once per waypoint, not every round.
- **Characters are placed by their own rules.** Companions catching up and characters getting up
  from furniture were put on free spots with the player's hitbox and rules, which could park them
  on a ladder tile where they could never move again.
- The log lines of the *Patrol*, *Companion*, *Schedule* and *Open shop* actions and the game's
  start-up error carry the usual `[events]` / `[game]` prefix.
- **Loading a game always lands in the save.** Quick-loading while a map change was fading out
  (or while the game asked about knocked-down companions) carried the loaded world into the old
  destination and dropped its *Map enter* triggers. The save's map is read first (a missing or
  broken map leaves the running game alone), and the load replaces the fade.
- A map that fails to load no longer leaves the game "resuming", which made the next map change
  skip its *Map enter* triggers and its autosave.
- Once-only triggers of companions, visitors and spawned characters are remembered by what lasts
  (the placed entity, or the spawner and the definition) instead of a runtime uid that differed
  every time: companions' once-triggers don't repeat, and saves stop collecting dead keys.
- Animations seeded by an entity's uid (working villagers, bobbing dropped items) no longer freeze
  for visitors and dropped items; walk and combat timers stop growing without bound.
- **Files are written whole or not at all.** Save games, settings, definitions, `project.ron`
  and the asset library are written to a temporary file next to the target and renamed over it
  (maps already were), so a crash or a full disk never leaves a truncated file behind.
- **Corrupt map files are refused instead of crashing the game**: the tile layers' chunk grids,
  the map size and the compressed size prefix are checked when a map is read.
- **One unreadable entry no longer blocks a whole project.** A definition entry or a map object
  that can't be read (a hand-edit typo, a newer version's variant) is left out with a log line
  (`[defs]`, `[map]`) and everything else loads; before, the game refused the project or the
  map. The editor shows what was left out in a banner and backs the file up (`items.ron.bak`,
  `village.map.bak`) before it could save without those entries.
- The editor logs when it cannot save its settings, the recent-projects list or a new script
  (these failures were silent), and the game logs why a save game is listed as unreadable.
- **Enemies walk home the way everyone walks**: a chaser returning to its post plans round
  solid props and waiting people like patrols do (it used to walk into them and give up after
  four seconds, making that spot its new home). A character's home moves when it arrives
  somewhere, not with every step.
- The companion window's item details show the stack count and the item's numbers (damage,
  defense, tool) like the inventory's do.
- **Lock texts can be translated**: the texts of locks and containers ("It's the elder's
  chest…") are collected into translation templates and shown in the chosen language; the
  example's German file has them.
- The mouse walk's "stuck" check is a tenth of a step at any frame rate (it was 0.2 px per frame,
  which stopped slow or climbing walks on fast displays). Nothing is persisted while no map is
  loaded (the fade after loading a game).

### Editor
- **AI agent server**: a tool that panics answers with an error and the editor keeps running
  (before, it took the editor down); a call the editor doesn't answer within 120 s
  (`RUSTPG2D_MCP_TIMEOUT` seconds) returns an error instead of hanging the agent; `play_test`
  removes its scratch folder however the run ends; files changed on disk are reloaded once each.
  The dropdown suggestions (`refresh_refs`) re-read only the map files that changed.
- The library report names the assets it added (up to eight): *2 added (smoke, smoke_square)*.
- **AI agent server**: `place_entity` puts a prop an even number of tiles wide on a tile edge like
  the editor does (it covered half a tile on each side before); the reply says which tiles it
  stands on.
- The editor script's `rename` knows every kind (`structure`, `skill`, `spell`, `recipe`, `quest`,
  `achievement` too; an unknown word is an error instead of renaming a map), and the agent
  server's kind words accept `npc` and `sprite_sheet` everywhere.

### Maintenance
- One walk over a project's triggers (`Project::for_each_trigger`, `TriggerSite`): the project
  check, the translation template and the editor's flag / variable / speaker collectors use it
  instead of six hand-written loops over definitions, maps, zones and entities.
- One Rhai lexer (`script_refs::lex`, every byte covered) serves the editor's highlighter, the
  reference finder, the function lists and completion; the four function finders are one. An
  unterminated string now ends with its line everywhere (the reference finder used to read on to
  the end of the file).
- The agent server's 54 tools are declared once (`mcp_tools!` in `tools.rs`: description,
  annotations, name, parameters, what the editor runs); the `Call` enum, its names and the
  dispatch are generated from it instead of kept in step by hand in four places.
- The inventory and companion windows share their slot grid, drag state (press, drag, drop,
  double click) and layout constants.
- The trade window's two lists, the crafting window and the menus share one `ScrollList`
  (selection, scrolling, double clicks) instead of three copies of the same arithmetic.
- The map file's object record (`map::MapObjects`) is also the editor's undo snapshot (one
  field list instead of three), and the inventory and companion windows share one item details
  footer.
- One implementation each of base64 (`hash.rs`; the fog of war, the web storage and the Tiled
  import had their own), the clock text (`entity::hour_text`; the game's clock and the editor
  rounded differently), the project-relative path check (`library::safe_rel`; the export now also
  refuses backslashes), the RON writer (`project::ron_text`; exported web projects were written
  with the platform's line endings), the `scripts/*.rhai` listing (`Project::script_files`, eight
  copies) and the `name_2`, `name_3` id generator (`refactor::unique_id`).
- The script API table (`engine/src/script_api.rs`: every function, its parameters' kinds and
  descriptions) is the one source for the editor's completion, the agent server's API resource,
  renames and the script check (id arguments are derived from it; the separate list is gone),
  translation templates (`Line` arguments: dialogs, questions, titles) and the editor's flag /
  variable collectors (which now also see `lockpick`'s variable).
- `Action::walk` / `walk_mut` visit nested `If` actions for the nine places that spelt the
  recursion out (translations, renames, the library's script roots, healing, NPC item use,
  the editor's flag / variable / speaker collectors, prop state renames).
- **Release builds** use thin LTO, one codegen unit and stripped symbols (smaller, faster
  binaries for the downloads and exported games). The Rust version is pinned
  (`rust-toolchain.toml`, `rust-version`, the CI workflows) and bumped on purpose.
- The game shows a start-up error (a project that doesn't load) in a message box on Windows
  instead of exiting silently, and reads the project once instead of twice; the engine
  fingerprint the editor compares with the game runner also covers the versions of the crates
  that define the file formats; the WAV reader can't overflow on 32-bit targets.
- Drawing and the frame: entities off screen aren't drawn, an entity's index is passed along
  instead of searched for twice per character, a character's activity is looked up once, the
  script host's view of the player is refreshed only before a script runs, key bindings are read
  without copying, translations are borrowed when there is none, NPCs look at their items every
  quarter second instead of every frame, projectiles collect their companion targets once per
  update, and companions' home maps are read from disk once per game.
- Path searches hash tile coordinates with one multiply instead of SipHash, pre-size their
  tables, tell "no route" from "out of budget" (a second search with fewer obstacles only runs
  when the first really tried everything), and line-of-sight checks walk the tiles a line crosses
  instead of stepping pixel by pixel.
- An occupancy grid (which tiles solid props and characters stand on), rebuilt once per frame,
  serves path planning and the broad phase of collision; before, every path search built a set
  of all entities' tiles and every movement step compared against every entity.
- The definitions the game looks up per entity per frame (items, props, characters, sheets,
  tilesets) are found through an id index instead of a scan of the list; a stale index falls
  back to the scan, so an edit in place can never return the wrong definition.
- `grid.rs`: one place for pixel ↔ tile conversions (`feet_tile`, `feet_spot`, `snap_feet`,
  `tiles_of_rect`); the game and the editor used five different offsets. Characters' rest spots
  moved one pixel (the editor's convention wins).
- One kind enum (`RefKind`, now with quests and achievements) with `name` / `file` / `label` /
  `parse`, a `Def` trait for the twelve definition lists and `with_def_kind!`: the library, the
  agent server, the editor's rename and the library window share one `Database` API (`has`,
  `ids_of`, `rename`, `copy_from`, …) instead of fifteen hand-written per-kind matches.
- The definition types implement `Default` (the file format's defaults, pinned by a test) and
  `fresh(id)` (the editor's new entry): a new field is added in one place instead of in every
  struct literal (26 of them, which had already drifted apart: the agent server's `def_template`
  gave other defaults than the editor's *+ New*).
- **Runtime smoke test** (`scripts/ci/smoke.sh`, CI job *Runtime smoke* on every pull request):
  the game and the editor run on a copy of the example project under Xvfb; checks the title
  screen, a map with the player, the clock, loading an old save, quick-saving, a map change with
  its autosave, the editor opening a map and the agent server's smoke client (`mcp_smoke`).
- **Compatibility fixtures**: `crates/engine/tests/fixtures/` holds the 0.9.0 example's
  definitions, maps of 0.9.0 and 0.10.0, a frozen format-version-1 map and save games;
  `tests/compat.rs` loads them, so a change that breaks old files fails the tests. Save games
  carry a `version` from now on (older saves have none; nothing changes for them).
- **Frame timing in `status`**: `fps=`, `frame_ms=avg/worst`, `update_ms=`, `draw_ms=` and
  `astar=searches/nodes` since the previous `status`; `RUSTPG2D_PERF=1` logs a `[perf]` line every
  second. For performance checks in scripted runs.
- `Documentation/architecture.md`: how the engine, the game runner and the editor fit together
  (technology, data model, the runtime's frame, the editor and its agent server, file formats and
  their compatibility rules, verification tooling, caveats). Linked from README and CLAUDE.md.

## 0.11.0 — 2026-11-09

A living village: every terrain a 47-tile set, many more assets, the village rebuilt as
Emberbrook with interiors and a daily life, four more quests, ladders and climbable
watchtowers. The example project and the editor's starter library are now plain files, edited
with the editor and its AI agent server (three new tools).

### Gameplay
- **Ladders**: tiles marked as ladders in *Database → Tilesets* (*Pick ladder tiles in the
  grid*) are climbed by the player — only up and down, slower, seen from behind, no attacks
  (`status` prints `climbing=`). Characters keep off them. Old tilesets load unchanged.
- **Time reached** (`TriggerOn::Hour`): a map trigger that runs when the in-game clock reaches a
  time of day while the player is on the map — every day, also when time jumps past it by
  sleeping or *Set time* (not when it is set back). The editor's trigger form takes the time as
  hh:mm. The village uses it for the dawn fog, the morning cluck and the noon bell.

### Editor
- **AI agent server: three new tools.** `write_asset` writes an image (PNG) or sound (WAV, Ogg
  Vorbis) into the project's `sprites/`, `tilesets/` or `sounds/` — from Base64 or by copying a
  file — after checking that it decodes, and reloads the art. `capture_structure` makes a new
  structure from a map region (tiles, collision, entities). `library_send` sends project assets
  with everything they need to the asset library; with `replace` it updates the library's
  versions of the same ids (definitions, files, script functions) instead of adding copies, and
  `drop_script_triggers` leaves triggers that run the game's own scripts behind.

### Fixes
- **Characters can't shut the player in any more.** Walking into a friendly character for a
  moment makes it step aside — to a free tile beside it, or ahead out of a narrow passage. In the
  example, the guard dismissed in the graveyard's gate and then told the shrine was lit froze
  there for good (`stop_patrol` on the spot), boxing the player in; loading such a save, walk
  into him and he makes way.
- A merchant behind a counter **prop** (a market stall) can be talked to across it, like across
  a solid counter tile, from in front of any part of the counter (not only straight across from
  the merchant); a prop in front that does nothing on E gives way to them. Right-clicking such a
  counter talks to the merchant too, and right-clicking the merchant walks to the counter in
  front instead of round the back of the stall.
- Characters that start on the same spot (several villagers leaving one door at the same time)
  could block each other for good: a character may now always move away from characters it
  already overlaps, and a character blocked on its way plans around the people in it.

### Maintenance
- Starter library updates: an image of the same size that only adds art in empty cells counts
  as an upgrade too (before, only larger images did), and an upgraded tileset's new tiles bring
  their solid / ladder / light / animation flags; existing libraries get the tower tiles the next
  time the editor opens them (`STARTER_RULES` 7).
- Autoplay `talkto NAME`: right-clicks a named character wherever it stands (characters that
  wander).
- `status()` prints `sounds=`: the last six sound effects started (also while muted), for
  automated play-tests.
- **The example project and the starter library are plain files.** The generator that wrote
  them from code (`crates/engine/examples/gen_example/`) is gone: `example_project/` is edited
  like any project (with the editor and its AI agent server), and the editor's starter asset
  library is the library folder `crates/editor/assets/starter_library/`, embedded by the
  editor's `build.rs` instead of a generated `starter.r2dpack`. Its content is unchanged. The
  library stamp is now a hash of those files, so existing libraries check the starter set once
  more after updating (nothing is added). Tests take over the generator's checks: the German
  translation has no leftovers, the starter library is complete and consistent, and it holds
  every asset of the example project. The `vorbis_rs` dev-dependency went with the generator.

### Example project
- **Climbable watchtowers**: the two towers at the east gate are built from tiles now (new tiles
  in `terrain.png`'s fifth row) with a ladder up to a lookout deck under the roof; the
  barracks has its east wall back (the old tower prop stood over it). The starter library
  gains the structure *Lookout Tower*.
- The guard sent back to his post (*Go back to your post*) walks his road again instead of
  staying where he was dismissed, and once the shrine is lit he goes off duty and stands
  by the barracks door (`guard_rest()`) instead of stopping wherever he stands.
- **Furniture**: the beds are drawn in the same three-quarter view as the rest of the art (an
  upright headboard with posts, a quilted blanket, a footboard with legs and a shadow; the
  sideways bed too), the tavern's tables have chairs whose backrests show over guests sitting
  with their back to you, and Brother Ansel sleeps in his own chamber beside the chapel's nave
  instead of a bed by the pews. The starter library has the new beds.
- **Four village quests**: *The Lost Cat* (Mira; clues from Bram, Old Finn and the miller lead
  to Biscuit in the mill), *Bread for the Festival* (Bram; flour from the raided cart on the
  forest road and its two outlaws, the Festival Loaf baked at his oven), *The Herbalist's
  Remedy* (Ida; feverleaf by the forest path, bitterroot on the pond bank and a moonpetal at
  night, brewed at Wren's table, cures Pip) and *Rumours at the Tavern* (Greta's rumour for a
  round; Brom's old friend Halvar; his locket by the bones in the old mine). Each has its
  achievement (Cat Person, Festival Baker, Village Healer, Old Debts); rewards in coins, items
  and skill points. Script function `cat_clue()`. German for every text.
- **Emberbrook**, the village, rebuilt (80 × 60): a river with two bridges, a cobbled market square
  (well, statue, notice board, produce / fish / bread stalls), the Copper Kettle tavern, bakery,
  smithy, apothecary with herb beds, chapel with bell tower and graveyard, the elder's house,
  Tilda's shop, Mira's home, the crier's home, the barracks, the fisher's hut with dock, boat and
  pond, an orchard, and west of the river the windmill and mill, fields, farmhouse, chicken coop
  and animal pen; a town wall with watchtowers and braziers at the east gate. **Seven interiors**
  (`tavern`, `bakery`, `smithy`, `apothecary`, `chapel`, `home_mira`, `mill`) with their stations
  (kitchen hearth, bread oven, forge, anvil, alchemy table, millstone), beds and furniture; the
  tavern has its own music and inn beds for paying guests (5 coins at the bar; one night).
  About twenty villagers with **daily routines** across the village and the interiors (stalls,
  work with tools in hand, an evening at the tavern, their own beds at night), children, a dog,
  chickens, sheep and a cow; dialog lines by time of day and pose. Dawn mist, the hens at six and
  the noon bell come from *Time reached* triggers. The forest gained feverleaf by the path.
- **The Workshop map is gone**: its stations and supplies moved into the village (smithy and
  apothecary chests, the herb beds, the fields and the seed barrel, the tavern's pantry, beds in
  every house). The village's north edge went with it; the east edge to the forest moved with the
  street (y 29-31).
- Save games made in the village before 0.11.0 don't fit the new village (its entities are new);
  saves made on other maps load as before.
- **Every terrain is a 47-tile set**: the terrain tileset's sheet is 16 tiles wide (512 × 1760 px)
  with one 7×7 blob block per terrain, so all fourteen terrains (Dirt path, Pond, Sand, Shore,
  Forest floor, Snowfield, Forest path, Stream, Flagstones, Rug and the new **Cobblestone,
  Farmland, Meadow and Carpet**) get inner corners; the 16-tile blocks are gone (the 16 slots are
  filled from the blob set as the fallback for *Corners* off). The library's desert, mountains,
  city and dungeon tilesets got the same treatment (Oasis, Hardpan, Snowfield, Mountain path,
  Frozen lake, Canal, Plaza, Earth, Chasm, Abyss). New single tiles for the village: cobbles,
  tilled soil, flower bed, carpet, light stone floor, north–south fence and fence post, gate
  pillar, hedge, dock planks and dock end, bridge railings, slate and thatched roofs with eaves,
  chimneys on red and slate roofs, stone house walls with a lit window, a door and a stained-glass
  chapel window, a counter, a grave mound, a cross-beamed timber wall, a dark window, straw.
  The unused `tilesets/desert.png` was removed from the example project.
- **Village assets** (in the example project's definitions and the starter library): about 60
  new props on `sprites/village.png` — market stalls (produce, fish, bread; the seller stands
  behind the counter), windmill, watchtower, bell tower, apple trees and beehives (picked by
  hand, back a day later), a bread oven, kitchen hearth, fireplace, forge and brazier (lit, with
  light; coal relights them), a millstone (grinds wheat into flour), shelves, racks, kegs, tavern
  tables, stools (one picture, four facings) and pews to sit on, altar, candle stands, a bell rope
  and the bell tower ring the new bell, farm and yard things, hanging shop signs (the tavern's
  glows), feverleaf, bitterroot and the moonpetal that can only be picked at night. The props of
  the old library sheet `props2.png` (street lamp, crate, pot, signpost, tombstone, lever,
  haystack, bones, treasure, wall torch, crystals, sack, cactus, palm, market stall, fountain,
  tent, cart) moved into the example. Villagers with outfits and drawn portraits: baker,
  innkeeper, herbalist, priest, miller, fisher, town crier, blacksmith, farmer (male and female;
  the merchants among them sell their goods), villager and town guard (female); children
  (`child_boy`, `child_girl`) and animals (`chicken`, `dog`, `cat`, `cat_ginger`, `sheep`, `cow`)
  as their own body sheets, with a sound and a line when talked to. Goods on `items2.png`:
  flour, egg, cheese, milk, honey, moonpetal, feverleaf, bitterroot, herbal remedy, silver locket,
  festival loaf, ale, broom and hoe (held in the hand like the smith's hammer and the pickaxe now
  are). Recipes `grind_flour`, `bake_festival_bread`, `brew_remedy`. Sounds `bell`, `cluck`,
  `bark`, `meow`, `baa`, `moo`; a third music track, *The Copper Kettle* (`music_tavern`).
  16 structures: tavern, bakery, stone smithy, chapel, thatched farmhouse, windmill and mill
  house, market row, animal pen, graveyard, orchard, fishing dock, town gate, and furnished
  tavern, smithy, bakery and chapel rooms. Eight more portraits. German for all new texts.
- **Asset library**: the starter rules are at version 6, so existing libraries are checked again
  for new assets; the reorganized terrain sheet is not an extension of the old one, so a library
  created before 0.11.0 keeps its old terrain art and 16-tile terrains until it is rebuilt from
  the starter set (back it up, delete `~/.local/share/rustpg2d/library`, open the editor).
  The library's male merchant, wizard, nomad and bandit outfits use the sheets
  `outfit_<id>_m` / `outfit_<id>_m_f`: their female cut had the same id as the female version's
  outfit. The farmer, blacksmith and the female villager and town guard come from the example
  project now (with their own outfits).

## 0.10.0 — 2026-11-08

Agents: AI agents build content and adventures from inside the editor.

### Editor
- **AI agent server (MCP)**: *Assets › AI agent server…* serves the Model Context Protocol from
  the running editor (Streamable HTTP on 127.0.0.1:7743, loopback only; *Start with the editor*
  remembered in `editor.ron`). Agents (Claude Code, Cursor, Claude Desktop via `mcp-remote`) get
  51 tools: project info and settings; maps as text grids with legends, screenshots, tiles,
  terrain, collision, entities, zones, triggers, map settings, structures, map links, rename;
  definitions (list, get, template, upsert, delete, the events reference) with validation of ids,
  files and trigger events; scripts (read, write, check, API); translations; `play_test` (the game
  with an autoplay script, status lines and screenshots back); the asset library. Edits show in the
  editor at once, each write is one undo step and saves. Resources `rustpg2d://docs/*`, prompts
  `design_quest` / `build_map`, a call log in the window, a status-bar indicator. Editor script:
  `mcp on|off [PORT]`, `mcpwindow`; `status` prints `mcp=`.
- **Project files are watched**: changes on disk to `project.ron`, `defs/`, `maps/`, `scripts/`
  and `lang/` (by an agent or another program) reload what has no unsaved edits; otherwise a banner
  asks *Reload* / *Keep mine* (`watchanswer reload|keep`; `status` prints `watch=`).

### Maintenance
- New editor dependencies: `rmcp` 3 (the MCP SDK), `tokio`, `axum`, `serde_json`, `schemars`.
  `Project::reload`, `refactor::refs_of_triggers`. Documentation: `Documentation/mcp.md`,
  `events.md`, `autoplay.md` (also served to agents); tests keep them complete.

## 0.9.0 — 2026-11-08

Reach: the web, other languages, Windows and macOS — games export for browsers, Windows and
macOS, can be translated, and the editor runs on Windows and macOS too.

### Gameplay
- **Languages**: games can offer translations (`lang/<code>.ron`, keyed by the original text);
  players pick one in *Options › Language*, the default is the system's (or browser's) language.
  R(ust)PG2D's own texts come translated for German. The example project is fully translated
  into German (and its script texts use placeholders instead of glued strings).

### Editor
- **Web export**: *File › Export game* has *Platforms* Linux / Web. The web version
  (`<game>-web/`: `index.html`, the game compiled to WebAssembly, macroquad's loader,
  `data.pack` with the game's files; `<game>-web.zip` for itch.io) runs in current browsers;
  saves, settings and achievements are kept in the browser's local storage; right-click works as
  in the game (no browser context menu). Tiles and sprites are drawn from a source rectangle
  inset by 1/64 texel, so fractional scales (browser zoom, high-DPI screens, zoom levels) no
  longer show thin lines of neighbouring tiles along edges. The packages ship
  `rustpg2d-game.wasm`, so the packaged editor exports web games. Editor script: `exportweb`.
- **Windows export**: *Platforms › Windows*; a Windows export is `<game>-windows/<game>.exe`
  with its data (and `<game>-windows.zip`), saves in `%APPDATA%\<game>\saves`. The game runs as
  a window program (no console window); when it can't start (e.g. no OpenGL driver in a virtual
  machine) a message box says why. The R(ust)PG2D packages ship the Windows game runner
  (`rustpg2d-game.exe`, cross-compiled with MinGW-w64), so the Linux editor exports Windows
  games. Editor script: `exportwin`.
- **macOS export**: *Platforms › macOS*; a macOS export is the application `<Game Name>.app`
  (data in `Contents/Resources`, Info.plist, the icon chosen under *Icon* or the player
  character) in `<game>-macos/` (and `<game>-macos.zip`), for macOS 11 or newer on Apple Silicon
  and Intel; saves in `~/Library/Application Support/<game>/saves`. The export signs the app ad
  hoc on Linux (sealed like `codesign --sign -`, passes `codesign --verify`); it isn't notarized,
  so players allow it once (*Privacy & Security › Open Anyway*; the bundle's README explains it).
  The R(ust)PG2D packages ship the universal game runner (`rustpg2d-game-macos`, built by CI on
  a Mac). Editor script: `exportmac`; `exportall` exports all four platforms.
- Exported zips date their files with the export time (they were dated 1980).
- **The editor on Windows and macOS**: R(ust)PG2D now also runs on Windows 10 / 11 (a zip:
  `rustpg2d-editor.exe`, the game runners, the example) and macOS 11+ on Apple Silicon and Intel
  (`R(ust)PG2D.app`, signed ad hoc, not notarized). Every editor exports games for Linux,
  Windows, macOS and the web (the other platforms' game runners ship next to it). Per-user
  folders follow each system (`%APPDATA%` / `%LOCALAPPDATA%`, `~/Library/Application Support`);
  shortcuts use Cmd on macOS (and the editor writes them so); mouse wheels and trackpads zoom
  the map and the pixel editor in steps per notch; file dialogs on macOS are modal; window icons
  (the emblem; games: the player character). Windows programs have no console window and show a
  message box when they can't start (no OpenGL). Editor script: `browse open|folder|icon`.
- *Database › Project › Languages*: the game's language, its translations with their progress,
  **Export translation template** / **Update template** (keeps translations, adds new texts,
  reports unused ones) and *Play-test in* (a language for play-tests). Editor script:
  `langtemplate CODE`, `playlang CODE|-`. Exports include the translation files.

### Fixes
- The game no longer panics when it quits on Windows (gamepad thread joined at exit).

### Maintenance
- `engine/src/sys.rs`: the game's file and clock access in one place (native `std::fs`; on the
  web an in-memory file table plus `localStorage`). The engine builds for
  `wasm32-unknown-unknown` (Rhai without random seeds and clock there, no gamepads, no zip).
- CI: clippy for the wasm game on every pull request; the Packages job builds it and checks it
  ships in the AppImage.
- `engine/src/paths.rs`: per-user folders per platform (XDG on Linux; `%APPDATA%`,
  `%LOCALAPPDATA%`, `%USERPROFILE%` on Windows; `~/Library/Application Support` and
  `~/Library/Caches` on macOS) for settings, saves, the library and caches.
- `engine/src/hash.rs` (SHA-256, SHA-1, Base64; moved from the packaging crate) and
  `engine/src/macsign.rs` (ad-hoc signing of macOS apps).
- `engine/src/icon.rs` (window icons; `hero_icon` / `emblem` moved from the packaging crate) and
  `engine/src/desktop.rs` (Windows start-up: console, error message box). RON files and the
  recent-projects list are written with LF on every platform; `.gitattributes` keeps LF; the
  engine fingerprint hashes `/` paths.
- Downloads: `cargo xtask-dist` also writes `rustpg2d-X.Y.Z-windows-x86_64.zip` and the macOS
  app; CI: a *Windows* job (clippy + tests with MSVC), *macOS* builds the universal editor and
  game, *macOS app* signs (`codesign`) and zips the app the Packages job assembled; the Release
  workflow publishes all four downloads (jobs: version, macOS build, packages, macOS app,
  publish). *Actions › CI › Run workflow* runs all platform jobs on any branch.
- CI: clippy for the Windows game on every pull request; a *macOS* job (on pull requests into
  master and in the Release workflow) lints the game, runs the engine's tests on macOS and builds
  the universal runner; the Packages / Release jobs cross-compile the Windows runner, take the
  macOS one and check both ship in the AppImage.

## 0.8.0 — 2026-11-08

### Gameplay
- **Gapless music loops**: music no longer has a short silence where it loops (a quad-snd mixer
  quirk). The engine decodes music (Ogg Vorbis via `lewton`, WAV), resamples it to 44.1 kHz and
  stretches it to whole mixer buffers (under 0.16 % for tracks of a minute; tracks under 30 s are
  repeated first); long files are prepared in the background at start. The example's two tracks
  are rendered at whole-buffer lengths and play unchanged.

### Editor
- **47-tile corner auto-tiling**: terrains can have a *Corners (47 tiles)* set that also looks at
  the diagonal neighbours (`Terrain.blob`), giving inner corners where a terrain bends. The
  Terrains form shows the 7×7 blob template with side and corner arms, *Fill from template
  (7×7)*; empty slots fall back to the 16-tile set. Tiled imports turn *mixed* Wang sets into
  47-tile terrains and *edge* sets into 16-tile ones (*corner* sets are reported).
- Example: the starter *Dirt path* and *Sand* terrains got blob sets; the village's roads and
  sandy pond are retiled with them.
- **Parallax layers** (map settings): images moving at their own speed behind the tiles or over
  the map (`Map.parallax`: sprite sheet, follow factor, drift, offset, repeat, size, opacity),
  drawn by the game and previewed in the editor (*View › Parallax layers*). Example: cloud
  shadows over the Wilds, mist far below the mine's chasm, which uses the dungeon tileset's new
  see-through *Abyss* terrain.

## 0.7.0 — 2026-11-08

### Gameplay
- **Gathering and mining**: props can be gathered with a tool (E or a melee swing; hits with a
  progress bar, then their loot and a "used up" state) and **regrow** after some in-game hours,
  also while the player is elsewhere. A new skill effect **Gathering** needs fewer hits per level.
  The clock counts **days** (`WorldState.day`). Recipes can need **tools** (not used up).
- Example: ore veins (copper, iron, silver, gems, coal) in the old mine, a Pickaxe and Smith's
  Tongs in a crate at its entrance, the Mining skill; a **smelter** by the mine entrance (ore and
  coal into copper / iron / silver ingots, tongs needed, silver at Mining 2). Alchemy needs a
  **Mortar and Pestle** (Tilda) and a new **Alchemy** skill for Antidotes (1) and Swiftness
  Potions (2, was Arcana 1); herb bushes and mushroom patches by the forest path are picked by
  hand and grow back by the next morning. Ore, ingots and rubies sell at Tilda's.
- **Blacksmithing**: recipes can make **better results** at higher levels of their skill
  (`RecipeDef.quality`; the crafting window shows the next step), and equipped items can be
  ingredients (upgrades). Example: an **anvil** in the Workshop with a Smith's Hammer and the
  tongs: Iron Sword (a Fine Iron Sword from Smithing 2), Iron Helmet, Chainmail, arrows,
  pickaxe and a Short Sword upgrade; the **Smithing** skill; Tilda sells the hammer.
- **Farming**: props can be **farm plots** and items **seeds** (a crop with growth stages and a
  harvest). E plants, tells the time left or harvests; crops grow over in-game hours, also
  while the player is away, half again as fast in rain (`EntityState.planted`). Example: wheat,
  carrot and pumpkin seeds at Tilda's and a field with a sack of seeds in the Workshop.
- **Cooking**: recipes can need their station prop in a state (`RecipeDef.station_state`, e.g. a
  "lit" campfire), items can be **food**, and the new skill effect **Food** makes food heal more.
  Example: a campfire in the Workshop (lit with coal, burns 8 hours), five dishes (Grilled
  Trout, Bread, Carrot Soup, Pumpkin Stew, Apple Pie), a pantry chest and the Cooking skill.
- **Schedules across maps and with furniture**: a schedule stop can be on another map (the
  character walks out through the door / portal leading there and turns up as a visitor with its
  look, items and triggers) and can name furniture to sit on or sleep in. The player can't take
  furniture in use; talking to it talks to its user. New condition **Pose** (sitting / lying /
  at furniture); a sleeper without a matching line shows "… is asleep.". Example: Tomas rests on
  the west bench in the afternoon and sleeps in the elder's guest bed at night.
- **Activities**: characters can be seen fishing, hoeing, hammering, chopping, mining, sweeping
  or reading at a schedule stop or all day where they are placed, with a tool in the hand and
  small effects (line and bobber, soil, sparks, chips, dust, a book); talking interrupts them for
  a moment. Example: Tomas fishes at the big pond in the evening, the elder reads by the well,
  and a smith and a farmer work in the Workshop.
- Example: **the Workshop**, a new map north of the village (a road through the tree line): a
  crafting testing ground with a supply chest of tools, a herb garden with an alchemy table, a
  quarry with every vein and a smelter, a bed to pass the night, and signs.

### Fixes
- The editor no longer crashes on images wider or taller than the graphics card allows (2048 px
  in egui): they are shown at a reduced size instead.

## 0.6.0 — 2026-11-06

### Gameplay
- **Healing companions**: a *Heal* spell or a healing item set to *Use on: Player or companion*
  asks whom to heal while companions are on the map: a "heal whom?" bar pauses the game; one
  click on a name in the bar, the character or its HUD row heals at once (Left / Right and E
  work too; Esc cancels, nothing is spent). Items can
  also be *for companions only*. Companions get HUD rows with their head and HP under the
  player's panel. New action **Heal character** and script `heal_npc(name, amount)`.
- **Knocked-down companions**: at 0 HP a companion falls and lies where it fell (greyed, *DOWN*
  in its HUD row) instead of going home; enemies ignore it and any heal gets it back up. Leaving
  the map asks "Leave … behind?" (it then limps home); save games keep it down in its place.
  *Project settings → Companions at 0 HP*: stays down until healed (default), gets up after N
  seconds with 1 HP, or goes home at once (the old behaviour).
- **Companion orders**: G (pad LB) opens a menu that pauses the game: *Attack* (the default),
  *Defend* (only what hunts the player or the companion), *Passive* (no fighting, backs away),
  *Stay here* / *Follow me*, for one companion or all. Orders are saved and shown in the HUD
  rows; a staying companion catches up on a map change. New action **Companion order** and
  scripts `order(name, order)`, `order_of(name)`, `is_staying(name)`; the guard's dialog in the
  example offers *Wait here* / *Stay out of fights*.
- **Companion inventory**: a companion window (its HUD row, *Gear...* in the orders menu, or
  `open_companion(name)` / the **Open companion window** action) shows its gear, its bag and the
  player's inventory: hand items over and back, equip it (weapons and armor change how it
  fights). Characters use the items they carry themselves (companions, other NPCs and enemies):
  healing when their health is critical (35 % or less, or one more hit like the last would fell
  them), cures when poisoned or burning, buffs in a fight; a bow the player gave them shoots arrows from
  their bag (the HUD row counts them). Characters can be marked *can't be given gear*. The
  guard's dialog in the example offers "Let me see your gear".
- **Portrait images**: characters can have a drawn portrait (*Database → Characters → Portrait*,
  a placed character's own in the inspector, the player's in *Project*) shown in dialogs and the
  companions' HUD rows instead of the automatic bust; *New blank portrait* opens the pixel editor
  on a 64 x 64 frame. **Moods**: more portraits per character by name, shown with
  `say(name, text, #{ mood: "worried" })`, the new **Set mood** action or `mood(name, mood)`.
  The example's elder (calm and worried) and Tilda have drawn portraits.
- Gamepad: LB no longer opens the character window from the map (RB and Y do); it opens the
  companion orders.
- Example: the Red Potion and a new **Bandage** (Tilda, 6 coins) work on the guard. The Red
  Potion no longer says "You're already at full health" (it may be meant for someone else).

### Fixes
- Loading a save while a companion was knocked down no longer asks "Leave … behind?".
- Healing friendly characters with *Heal area* now keeps a companion's new HP in the save game.

## 0.5.0 — 2026-11-06

### Gameplay
- **Weather**: each map has a weather (map settings: clear, rain, snow or fog, with a strength).
  Rain falls in streaks with splashes, darkens the map and plays a built-in rain loop; snow
  drifts in flakes; fog moves over the map in soft clouds. The particles are drawn over the lit
  scene and dim at night. The rain sound follows the new **Ambience volume** option
  (environmental sounds, separate from music and effects). The **Set weather** action and `set_weather(kind, [intensity])`,
  `set_map_weather(map, kind, intensity)`, `weather()`, `weather_intensity()` change and read it;
  changes are kept in save games (`WorldState.weather`). The editor has *View › Weather
  preview*. Example: fog over Whisperwood until the shrine is lit, rain in the Wilds.
  Editor script `mapweather`, `viewopt weather`; game `status` prints `weather=`.
- **NPC schedules**: a placed character can have a *Daily schedule* (map inspector): stops with a
  time, a tile, an optional facing and *inside* (the character goes indoors there, out of sight
  until the next stop). At each time it walks to the stop (A*, like patrols) and stays there,
  wandering around it if its behaviour is Wander; entering a map puts scheduled characters at
  the stop of the hour. Stops are placed, dragged and removed on the map like patrol waypoints
  and drawn as a dashed loop with their times. The **Schedule** action (follow / stop, kept in
  save games: `EntityState.schedule_off`) and `stop_schedule(target)` / `follow_schedule(target)`
  control it. Example: Tomas in the village spends the day at the shop, the duck pond and the big
  pond and goes home into the elder's house at night; his lines depend on the time of day.
  Editor script `scheduleedit`, `stop K hour|facing|inside V`, `status` prints `schedule=`; game
  `status` prints `schedule=`.
- **Companions**: the **Companion** action and `follow(target)` / `stop_following(target)`
  (plus `is_companion(name)`, `companion_count()`) let a placed, friendly character follow the
  player from map to map (`WorldState.companions`: where it is placed and its HP). Companions
  walk behind the player, fight hostile characters nearby with their weapon, are hunted and hurt
  by enemies, regain HP when it is calm, and run their own triggers when talked to. Knocked out
  they go home; dismissed they stay (own map) or go home. Example: the town guard comes along to
  Whisperwood once the elder's quest is accepted. Game `status` prints `companions=`.
- **Crafting**: recipes (`defs/recipes.ron`, *Database › Recipes*: result and count,
  ingredients, station, optional skill level) are made in a crafting window opened by the new
  **Open crafting** action / `open_crafting(station[, title])`; `can_craft(recipe)` checks one.
  The window shows each recipe's ingredients with what the player has and a *Make* button
  (Shift: ten). Recipes are asset-library assets (with their items) and follow item renames.
  Example: an alchemy table in Tilda's shop with four brews, Healing Herbs and Red Caps along the
  forest path (new items), herbs for sale at Tilda's. Game `status` prints `crafting=`, autoplay
  `craft K` clicks a recipe's *Make* button.
- **Achievements**: `defs/achievements.ron` (*Database › Achievements*: name, description, icon,
  hidden, unlock conditions), checked twice a second while playing; the **Unlock achievement**
  action and `unlock_achievement(id)` / `achievement_unlocked(id)` for the rest. Unlocks belong
  to the player (`achievements.ron` next to the settings), show a toast (also over the pause
  menu) and are listed on a new *Achievements* page of the title and pause menus with their
  dates and a *Reset*. Example: six achievements, one of them hidden. Game `status` prints
  `achievements=`.
- **Minigames**: picking a lock is a short game now (three pins: press while the marker is in
  the green spot; harder locks have smaller spots and faster markers, the Lockpick skill — level 1 or more — eases
  them, three misses and it holds); experts still pick at once, and the key is used when the
  player has it. The **Lockpicking game** action / `lockpick(difficulty, var)` plays it for
  any puzzle. **Fishing** spots (a new zone kind over the water, with the item needed and a catch table;
  fish shadows glide through it): facing it from the shore, cast, wait for the bite, strike,
  reel in without snapping the line. Example: a Fishing Rod at
  Tilda's, Trout and Old Boot, the village pond as a fishing spot. Game `status` prints
  `minigame=`; autoplay `minigame hit|miss|bite` presses at the right (or wrong) moment.
- **Tool slot**: a sixth equipment slot for tools. Items get a *Tool kind* (*Database › Items ›
  Equipment: Tool*; `ItemDef.tool`, `Equipment.tool`, both optional in older files) and go into
  it; activities ask for them, and the character holds the tool's *Held sprite* in the right
  hand while using it. Fishing puts the rod into the Tool slot by itself if it is in the bag and
  shows it in the hand with the line leaving its tip (example: `held_rod`). `unequip(target,
  "tool")`.
- **Dialog portraits**: dialogs and choices show the speaker's head and shoulders, drawn from
  their layered model; the speaker is found from the speaker name (`{player}`, `{self}`, a
  character on the map or a definition whose name fits). *Database › Project › Dialog
  portraits* (`ProjectConfig::dialog_portraits`, on by default) turns them off. The example's
  intro has a line of the hero's.
- **Furniture**: props can be furniture (`PropDef.use_spot`: pose *Sit* / *Lie* / *Stand*,
  spot, facing, wake-up hour, a cover sprite drawn over a sleeper, conditions). The player sits
  on chairs and benches (HP and mana come back faster) and sleeps in beds: the night passes
  (clock, HP, mana, effects, respawns, schedules), the new *Woke up* trigger runs, the game
  autosaves; not with enemies nearby. **Sleep** action / `sleep(hours)`, `is_sitting()`,
  `is_sleeping()`. Chairs face any of the four directions (a cover draws a backrest
  over the sitter), beds stand upright or sideways (the sleeper is drawn turned). Example: the
  elder's house has an upright bed (instead of the bed tiles), a sideways guest bed and a table
  with four chairs; a chair at Tilda's, benches by the village road. Game `status` prints `pose=`.
- **Changed:** picking a lock you have the key for uses the key (it used to pick it).
- **Changed:** the player's attacks (melee, arrows, spells, clicking to attack) only hit hostile
  characters. Friendly characters with HP (companions, the guard) can't be hurt by the player;
  make a huntable animal hostile with 0 damage.

### Fixes
- Patrolling (and scheduled) characters plan their way around solid props such as the village
  statue instead of walking into them and getting stuck.

### Maintenance
- **rust2d is now R(ust)PG2D** (another project already uses the name rust2d). The technical
  name is `rustpg2d`: binaries `rustpg2d-editor` / `rustpg2d-game`, crates `rustpg2d-engine` /
  `rustpg2d-packaging`, environment variables `RUSTPG2D_*`, packages
  `rustpg2d-X.Y.Z-x86_64.AppImage` / `rustpg2d-X.Y.Z.flatpak`, Flatpak id
  `io.github.Shadowrom2020.rustpg2d` (install it anew; the old Flatpak can be removed),
  repository `Shadowrom2020/rustpg2d`.
- Folders move to the new name and are copied over on first use, the old ones are left alone:
  game settings and achievements (`~/.config/rust2d/<game>` → `~/.config/rustpg2d/<game>`, also
  for games exported with older versions once they are exported again), the asset library
  (`~/.local/share/rustpg2d/library`), the editor's recent projects and the AppImage runtime
  cache. An example copy in `~/rust2d/example_project` keeps being opened there. Save games,
  projects, maps and `.r2dpack` packages are unchanged.
- Game exports suggest the Flatpak id `org.rustpg2d.<game>`. To update a game already shipped as a
  Flatpak, type its old id (`org.rust2d.<game>`) into the export dialog so it stays the same app
  and its players keep their saves.

## 0.4.0 — 2026-11-06

### Editor
- **Copy, cut and paste map regions**: drag a rectangle with the Select tool on empty space (or
  Shift+drag anywhere); Ctrl+C copies its tiles on all layers, collision overrides and entities,
  Ctrl+X also empties it, Del only empties it. Ctrl+V shows the copy under the mouse and each
  click pastes it as one undo step until Esc or right-click. The clipboard is kept when another
  map is opened. Pasted named entities get a free name. A selected entity can be copied alone.
  The region's inspector has Copy, Cut, Paste, Clear and *Save as structure…*; the Edit menu has
  Cut, Copy and Paste. Editor script commands `region`, `copy`, `cut`, `paste`, `unpaste`;
  `status` prints `sel=`, `clipboard=` and `pasting=`.
- Structure ghosts (Structures tool and paste preview) draw characters with all their layers.
- **Import** (*File › Import…*): Tiled maps (`.tmx` / `.tmj` with their tilesets, repacked to
  32 px; layers, collision layer, solid tiles, tile animations, trigger zones and `spawn:` points
  from object layers), LPC character sheets (the walk cycle in the character layout, as a body,
  hair, outfit… sheet) and Aseprite sprite sheets (frames in a grid, tags as named animations).
  A window reports what was imported and left out; nothing is overwritten. Sprite sheets have
  **animations** (*Database › Sprite sheets*), offered by the *Effect* / *Fly* actions and
  played by `effect()` / `fly()` with `#{ tag: "name" }`. Editor script `import tiled|lpc|aseprite`,
  `importdialog`, `actionsheet`; `tab KIND ID` selects by id. New editor dependencies:
  `quick-xml`, `flate2` (both pure Rust, already in the lock file), `rhai` (JSON).
- **Events graph**: the new *Events* tab shows the open map's triggers as a graph with their
  owners, readable conditions and actions (dialog lines, choices with options), the script
  functions they run (with their dialog lines, choices, events and flags) and the maps they
  lead to; edges for script calls, custom events, flags / variables tested elsewhere, and
  teleports / portals. Nodes can be dragged; the pencil edits a trigger with the inspector's
  editors (undoable); function nodes open the script editor at the function. Scope: whole map
  or the selected object; optionally all script functions. Engine: `script_refs::list_args`
  (string arrays such as `choice()` options). Editor script `mode events`, `graph`,
  `graphnode`, `graphedit`, `graphtext`, `graphview`, `graphdump`; `status` prints `graph=`.
- **Auto-tiling terrains**: the new Terrain tool (A) paints a terrain and picks each cell's
  edge tile from its four neighbours, also re-tiling the neighbours; Shift erases to the
  terrain's base tile, right-click picks a terrain, each stroke is one undo step. Terrains are
  defined in *Database › Tilesets › Terrains* (16 tiles per neighbour combination, a base
  tile, terrains it joins, fill from a 4×4 block, a preview). Terrains can lie on each other
  (sand joins the shore water inside it). Every starter tileset has terrains with new edge
  tiles: terrain (10), dungeon (2), desert (2), mountains (3), city (2). All example maps use
  them: soft roads, ponds, shores, forest floor, streams, snowfields, flagstones, rugs, earth
  patches and a chasm, plus a new duck pond on the village green. Existing asset libraries get
  the terrains, their solid flags and animations with the starter set. Editor script
  `tool terrain`, `terrain NAME`, `terrainpaint X Y [erase]`, `terrainedit K`, `viewopt`.
- **Script hot reload and developer console**: play-tests from the editor start the game with
  the new `--dev` option. Saved scripts reload in the running game within half a second; a
  script with an error leaves the previous scripts running and shows the error in a toast and
  the console. F4 opens a console that pauses the game: `call`, `give`, `take`, `flag`, `var`,
  `settime`, `teleport`, `hp`, `xp`, `effects`, `status`, `reload`, `clear`, `help`, and any other
  line runs as Rhai with the project's functions. Exported games ignore `--dev`. Autoplay has
  `console LINE`, and `status` starts with `dev=`; the editor script has `play [start]`.

### Fixes
- Database › Tilesets (also in the Asset Library window): the tile grid could only be seen in a
  thin strip at the end of the form, and the scrollbar stopped before the tileset's last rows.
  The grid is now part of the form (it scrolls only sideways for very wide images).
- Lists and panels (the Database lists, the script file list, the map's left panel and
  inspector, the Project form, the pixel editor's side panel, the library grid) shrank to their
  widest entry: the scrollbar sat right after the longest name and the mouse wheel did nothing
  over the empty space next to it. They now fill their panel.

## 0.3.0 — 2026-11-06

### Gameplay
- **Experience and levels** (optional per project, *Database › Project › Experience and levels*):
  defeated characters give the XP set in their definition (*Combat › Experience*), completed quests
  their *Experience reward*. Levels raise max HP, mana and skill points; the XP each level needs
  grows by a factor per level. The status frame shows the level and an XP bar, the character window
  the level and the XP to the next one, and a message announces each level. New: the *Give
  experience* action, the *Player level* condition and the script functions `give_xp(n)`, `xp()` and
  `level()`. Older projects keep it off; older saves start at level 1. The example project uses it.
- **Status effects**: poison, burning, regeneration, slow, haste, stun, strength, weakness and
  shield, timed, on the player and on characters. Weapons apply one on hit (*On hit* in the weapon
  form), hostile characters on touch (*On touch* in the character form, or the effect of a melee
  weapon they hold); the *Apply effect* / *Cure effect* actions, the *Player has effect* condition
  and the script functions `apply_effect`, `cure` and `has_effect` do the rest. Badges with the
  seconds left in the status frame, markers over characters, tints for poison / burning / stun;
  the player's effects are saved. Example: poisonous slimes, an *Antidote*, an *Elixir of
  Strength* and a *Swiftness Potion* at Tilda's (new icons); skeletons in the starter library slow.
- **Enemy behaviour**: *Chase player* characters notice the player in a clear line within their
  sight (through walls and trees only within half of it), hunt on for a few seconds after losing
  them, and slide around obstacles instead of getting stuck on a corner. New per character: *Leash*
  (give up and walk home), *Flee below %* and, for shooters, *Keep distance*. The *HP below %*
  trigger (characters) and `hp_of(target)` for scripts let bosses react to their HP. Example: goblin
  archers keep their distance, flee once wounded and stay near their camp; a *Bandit Chief* in the
  south-east of Whisperwood calls two outlaws at half his HP.
- **Bestiary**: the journal (J) has a second tab listing every kind of character the player has
  defeated, with the count, a portrait, the new *Description* of characters, health, damage,
  defense, experience, weapon, touch effect and drops. The *Defeated count* condition and
  `defeated_count(character)` read the counts (saved). The example's monsters have descriptions.
- **Dialog log**: the journal's third tab, *Dialogs*, shows the last 1000 dialog lines of the game
  (speaker, text, and the options the player picked), newest at the bottom; scroll with Up / Down,
  the mouse wheel, PgUp / PgDn and Home / End. The log is saved with the game.
- **Gamepads**: any controller Linux knows works next to keyboard and mouse - stick / d-pad to move
  and navigate, A interact / confirm, B back, X attack, Y character window, RB / LB tabs, LT run,
  RT switch weapon, Start pause, Select map (full table in the README). The game reads named actions
  now instead of fixed keys (the groundwork for rebinding); keyboard controls are unchanged.
  Flatpaks of games and of rust2d get `--device=all` (gamepads need /dev/input). Building rust2d
  needs `libudev-dev` (the game links `libudev`, present on every desktop Linux).
- **Key rebinding**: *Options › Controls* lists every action with its keys. Enter and a key adds a
  key (up to three per action), Del clears, *Reset to defaults* restores them; keys that clash with
  another action where both are used show in orange with the clash named. Saved per game in
  `settings.ron` (only the changed actions); the Options page's own reset keeps the keys. Escape,
  F3 and the gamepad buttons stay fixed.
- **Screen shake and hit flash**: a hurt player gets a short camera shake and a red flash;
  explosions shake the screen (`shake` option of `effect()` and the *Effect* action; the example's
  Fireball uses it); scenes shake it with `shake(seconds[, strength])` or the new *Shake the
  screen* action. *Options › Shake & flash* switches both off (accessibility).
- **Save thumbnails**: every save (also quick saves, autosaves and *Save and quit*) stores a
  small picture of the world next to the save file (`<slot>.png`, without HUD, dialogs and menus);
  the Load and Save pages show it beside the map, play time and age. Older saves show a
  placeholder; deleting a save deletes its picture.

### Fixes
- Triggers fired by an action were lost: the *Defeated* triggers of characters defeated by *Damage
  area*, `damage_around()` or *Defeat all enemies* (e.g. quest counters) now run, as documented.
- Editor: the Database tab bar wraps onto a second line in narrow windows; it used to widen the
  forms below it past the window's right border (description boxes, separators, hints).

### Maintenance
- **Faster, cheaper CI**: pull requests into develop run only the branch policy (now seconds:
  xtask builds without the engine unless `--features dist`, `cargo xtask-dist`) and one
  unoptimized clippy + test job (no separate build); the version check and the AppImage / Flatpak
  build run for pull requests into master (release / hotfix branches) only, or by hand with
  *Actions › Release › Run workflow* (build only). Merges into develop no longer rerun CI;
  Dependabot checks monthly.
- **png 0.18** for reading and writing images in the engine (pixel editor, asset library,
  package icons); an image too large to decode is reported instead of panicking. New tests decode
  every example image and check lossless round trips.
- Dependabot no longer proposes egui / egui-phosphor updates: they follow egui-macroquad, which
  is still on egui 0.31 (egui-phosphor 0.14 needs egui 0.36).

## 0.2.0 — 2026-11-05

### Project
- **MIT License** (Copyright (c) 2026 Jürgen Herrmann): the project has a `LICENSE` file (it was
  declared MIT in Cargo.toml only). It ships in the rust2d AppImage and Flatpak, and every
  exported game includes it as `rust2d-LICENSE.txt` for the engine inside (also in the game's
  zip, AppImage and Flatpak); the license of the game itself stays the creator's choice.

### Development and releases
- **GitHub workflow**: CI on every pull request (branch policy, build + clippy + tests, version
  check, AppImage / Flatpak packaging with a smoke test); `master` and `develop` are protected
  (pull requests and passing checks for everyone, merge commits only, release tags can't be
  moved); Dependabot updates; pull request and issue templates.
- **Bugfix and hotfix branches**: `bugfix/<name>` (from develop) and `hotfix/X.Y.Z` (from master,
  PATCH only) next to `feature/` and `release/`.
- **Semantic Versioning**: the version is in the release / hotfix branch name; CI checks it is
  valid SemVer, above the latest release and matches Cargo.toml and the CHANGELOG.
- **Automatic releases**: merging into master tags the release, publishes it on GitHub with the
  rust2d editor (+ game runner and example project) as **AppImage** and **Flatpak**, and opens the
  back-merge pull request into develop.
- `cargo xtask` (`dist`, `check-version`, `check-branch`, `release-notes`); the packaging code is
  shared by the editor's game export and the toolkit packages (`crates/packaging`).
- Packaged rust2d: the welcome screen's **Example project** copies the bundled example to
  `~/rust2d/example_project` and opens it; game exports work from inside the AppImage and the
  Flatpak (there via the host's `flatpak`).
- The editor binary, too, runs on glibc 2.34 (`acosf` added to the portable math functions,
  which moved into the engine); the glibc check reads the ELF version needs and ignores weak ones.

### Releasing games
- **AppImage and Flatpak**: *File → Export game* can also build Linux packages of the game: an
  **AppImage** (one file for most distributions; the editor writes it itself, only the official
  AppImage runtime is downloaded once, checked against a pinned SHA-256 and cached) and a
  **Flatpak bundle** (sandboxed, on the freedesktop 25.08 or 26.08 runtime, installed by players
  with `flatpak install --user GAME.flatpak`; needs the `flatpak` command, not the runtime).
  Both get a desktop entry and an icon — the player character, or a PNG of the creator's choice.
  Editor commands `exportpkg` and `exportdialog appimage flatpak`.
- **Runs on older distributions**: the game executable needed the glibc of the system it was
  built on (2.44 on a current rolling release), so exports didn't start on Debian 12 or Ubuntu
  22.04. It now needs only glibc 2.34 (five math functions that newer glibc versions
  re-versioned are built into the game), and the export dialog shows what the executable needs.
  Verified: the example's AppImage on Debian 12, its Flatpak on the 25.08 runtime.

## 0.1.0 — 2026-11-05

### Saving and defeat
- **Defeat menu**: a defeated player no longer reloads the quicksave automatically. After the
  red fade a menu offers *Continue from last save* (the newest save of any slot), *Start over*
  (a new game from the start as the same hero: name, sex and look stay) and *Quit to main
  menu*. Without a save, *Continue* is greyed out and *Start over* is selected.
- **Autosave**: every map change saves to the new *Autosave* slot, once the new map's *Map
  enter* triggers and cut-scenes have finished ("Autosaved" toast). It shows in the load
  lists as *Autosave* and doesn't change the slot *Save and quit* writes to.
- **`autosave()` / *Autosave* action**: scripts and triggers ask for an autosave; it is written
  as soon as the running event (dialogs, waits, cut-scene) is over.

### Audio
- **Ogg Vorbis music**: sounds and music can be WAV or Ogg Vorbis files (`.ogg`); the Sounds
  form and the README say so, and a file in a format the game can't play (Opus, FLAC, MP3)
  is reported by name when the project loads instead of just "could not decode".
- **New music**: the example project's and the starter library's two tracks are replaced by
  composed, rendered pieces — *Market Day* (a lilting lute, recorder and tabor dance for the
  village) and *Whisperwood* (a slow drone, harp, recorder and bells for the forest) — saved as
  Ogg Vorbis at 44.1 kHz and looping seamlessly. They are written by the example generator's
  new `music.rs` (a small synthesizer and note notation) instead of the old square-wave loops.
- **Stop button**: the Sounds form (Database and the Asset Library window) has *Stop* next to
  *Play*; Play previews a file once and a new preview stops the previous one.

## 0.0.9 — 2026-11-05

### Characters and equipment
- **Layered characters**: every character is a body (people: in underwear, with recolorable
  skin and hair, a hair style and a beard; monsters: the whole creature) dressed by what it
  wears. A new **Clothing** equipment slot dresses them (outfit overlays, with a **female cut**
  worn by female bodies), armor is drawn over the clothing, and **weapons, shields and lights
  are visible in the hands** for every facing and walk frame (melee swings sweep the weapon).
  The pre-drawn sheet option is gone: a character of an old project keeps its sheet as its body.
- **NPCs have equipment and carried items** (per definition and per placed NPC): visible, used in
  combat (weapon, armor defense), dropped when defeated; actions *Equip item*, *Unequip slot*,
  *Give item to character*, *Take item from character* and script functions `equip`, `unequip`,
  `give_item_to`, `take_item_from` change them (saved).
- The **character creator** offers skin color, hair style, hair color and, for men, a **beard**
  (lists set in the editor); the pre-drawn looks are gone.
- **Playable bodies**: body sheets have a *Playable* flag; the creator's new **Body** row offers
  the playable bodies of the chosen sex (e.g. human or goblin; never a slime). Set it in the
  character builder, the sprite sheet form or *Project › Character creator*. The example and
  the starter library mark people's bodies playable and monsters' not; existing libraries get
  the flag when the editor applies the new starter set.
- The player character's equipment and carried items are what a new game starts with; saves from
  before clothing get the player's clothing once.

### Gameplay
- **Cut-scenes**: the action *Play cutscene* / `cutscene("fn")` plays a script function as a
  cut-scene (HUD hidden, black bars, everyone else waits; the HUD comes back afterwards).
  New actions and script functions *Walk to tile* `walk_to`, *Walk up to character* `approach`
  and *Place at tile* `place`. The example's new game opens with one: the hero walks into town
  from the west and the guard sends them to Elder Rowan. Editor: *+ Cutscene* template and
  *Play as cutscene* in the Scripts tab, *Preview* on the action, `rust2d-game --cutscene FN`.
- **More for scenes**: `dim(amount, seconds)` / action *Dim the scene* (lights show by day),
  `disable_spawners(map[, zone])` / `enable_spawners(...)` / action *Spawners on / off* (saved),
  `defeat_enemies()` / action *Defeat all enemies*, a light `color` for effects; effects and timed lights keep playing during cut-scenes.
- **Hide the interface**: H hides / shows the HUD (status frame, clock, quick bar, buttons);
  dialogs and windows the player opens still show. Scripts: `hide_ui()`, `show_ui()`;
  triggers: the new action *Show / hide UI* (e.g. for cut-scenes). Not saved.
- **Status frame** (HUD, top left): a framed panel with a small portrait of the hero, the hero's
  name, the hearts, the mana bar, the defense and the ammo of an equipped ranged weapon. The
  weapon box in the bottom-left corner is gone (the held weapon is drawn on the hero).
- **Character window**: inventory, character sheet, skills and spells are one window with the
  tabs *Character* (C / I), *Skills* (K) and *Spells* (L); one HUD button (next to the map
  button) replaces the three inventory / character / skills buttons. Tab cycles the tabs, the
  key of the open tab or Esc closes it. Container loot windows stay separate.
- Trade window: the mouse wheel moves the selection up / down (on the list under the pointer,
  else the active side); the list scrolls along.
- **Scripted spell effects**: new script functions (also trigger actions) `aim(fn)` (magic
  targeting; the click calls `fn(x, y)`, a spell's cost is paid only then), `fly(sheet, x, y, fn,
  #{ speed, frames, fps, scale, light })` (an animated, glowing sprite flies to a point and calls
  `fn(x, y)`), `effect(sheet, x, y, #{ ... })` (an animation played once, e.g. an explosion) and
  `damage_area(x, y, radius, damage)`. Script ids of sprite sheets in `fly` / `effect` are checked
  and renamed with the sheet.
- **Script light and healing**: `light_at(x, y, radius, seconds, #{ color, flicker })` (a timed
  light at a point), `heal_area(x, y, radius, amount)` (the player and friendly characters),
  `player_light(radius, seconds, #{ color, flicker })`, and `player_x()`, `player_y()`,
  `player_facing()`, `player_max_hp()`. `player_light` and `damage_around` accept integers and
  decimals in any mix (before, `player_light(5, 30.5)` failed).

### Editor
- **Character builder**: characters are assembled visually — a walking preview (facing buttons
  or turning, the four idle directions) beside rows of clickable cards for the body, skin
  swatches, hair style, hair color, beard, clothing (shown worn), head / body armor, weapon,
  left hand, plus the carried items; *Randomize* and *Start from…* (copy the look and equipment
  of a character of the project or the asset library — library sheets and items are brought
  along). It is the *Look & equipment* part of the Characters form (project and library),
  opens as a window for **New character…** (id, name, *Create*) and for a placed NPC's own
  look (*Customize this one…* in the map inspector, *Use the definition's* clears it). Items get
  a *Held sprite* and outfits a *Female cut* sheet, sheets a *Kind* and bodies a *Female body*
  flag, the armor slot *Clothing*; the Project tab sets the creator's bodies and its skin, hair
  style, hair color and beard choices with the same cards. The New-asset wizard no longer makes
  characters. Editor-script commands `charnew`, `charset`, `charrandom`, `chartemplate`,
  `charcreate` / `charapply`, `charcancel`, `charcustom`, `charreset`, `entequip`, `entskin`.
- Trigger actions *Aim (then run script)*, *Fly sprite to point*, *Play effect*, *Damage area*,
  *Light at point*, *Heal area*; *Light around player* gained color and flicker. The script editor
  completes all new functions (sprite sheets for `fly` / `effect`), and `queue_set_var`, which was
  missing; a test now fails when an engine function is missing from the completion table.
- Script editor: the parameter hint also works for the project's own script functions, and right
  after `(` or `,` an id argument (item, map, spawn point, sheet, script function, ...) opens its
  suggestions at once, inserted with quotes.

### Asset library
- **Scripted skills and spells in the library and packages**: *Add to library* / *Export…* of
  assets that use scripts open a *Script functions* dialog; the functions the assets need
  (and what those call or pass as callbacks) are ticked, other functions only go along when
  ticked. Functions travel in packages and the library (`scripts/library.rhai`), are appended to
  a project's `scripts/library.rhai` when it takes the asset (existing functions are kept), follow
  id renames, and are pruned when the library no longer needs them. Assets the functions use by
  id (sprite sheets, sounds, items) come along. The starter library now has the Fireball spell
  and the Scavenging skill with their functions.

### Example project and starter library
- Every character is a layered model: shared bodies (male, female, goblin, **female goblin**),
  short / long / **ponytail** hair, **full beard and goatee**; outfits as clothing items in both
  cuts; held sprites for sword, spear, axe, bow, crossbow, shield, lantern and torch; slimes,
  skeletons, bats and ghosts are bodies. **Female bodies are more feminine** (narrow waist, bust,
  slim limbs, rounder chin, fitted outfits). The guard holds a spear and wears a helmet, goblin
  archers and the new **goblin scouts** (female, in the wilds) carry a crude bow (and drop it
  with their rags), the villager carries apples, Tilda sells green and red tunics, the creator
  offers human and goblin bodies, 6 skin tones, 4 hair styles, 6 hair colors and 3 beard choices.
- The forest shrine is a 3x2 monolith with lamp pillars on a larger stone platform
  (both statues on it); touching it on the elder's quest plays a cut-scene with light effects
  (new `magic_burst` sheet, `chime` sound), defeats the monsters left in the forest and switches
  off its spawners. Tilda's slime hunt is done then (she rewards it); not yet taken, she only
  thanks the hero.
- **Fireball** spell (Tome of the Fireball, sold by Tilda; needs Arcana 3): click a target, a
  fireball flies there and explodes, damaging everything within 2 tiles; embers glow on briefly. New sprite sheets
  `fireball` and `explosion` (also in the starter library) and sounds `fireball`, `explosion`.

### Documentation
- `Documentation/scripting.md`: the complete scripting guide and API reference (how scripts
  are run, immediate vs. queued functions, state, targets, units, every function, recipes,
  limits, a Rhai primer). Tests keep it complete and its examples compiling.

### Fixes
- Fix: arriving on a map (edge, spawn, transition, loaded save) inside something solid - e.g.
  a gate post at the village's east edge when coming back from the forest - left the player
  stuck; the player now arrives on the nearest free tile.
- Fix: boulders showed a stray dark mark at their upper left (the open double gate's post cap
  spilled into the boulder's cells on the props sheet).
- Fix: spell, skill, scroll and book icons were blank in libraries created before those icons
  existed (the library kept its older, narrower `items` sheet image). Starter updates now replace
  an existing sprite sheet or tileset image when the new one only adds frames and leaves the
  existing part unchanged; images the user edited stay as they are. Existing libraries apply
  this once on the next start.
- Fix: `wait()` accepts whole numbers too (`wait(1)` used to be a script error).

## 0.0.8 — 2026-11-04

### Gameplay
- **Skills and magic**, both optional per project (Database › Project › Skills & magic). Skills
  (Database › Skills) have a max level and an effect per level: melee / ranged / spell damage, defense, speed, lockpicking
  (locks get a pick difficulty), haggling, max mana, mana regeneration, none, or a script function
  `name(level)`. Skill points come from the new *Give skill points* action / `give_skill_points()`
  and are spent on the Skills screen (K). Magic adds mana (bar under the hearts, regenerates, mana
  potions via *Add mana*) and spells (Database › Spells: cost, cooldown, required skill, effect:
  heal, bolt, blast, light or script) learned from items that *teach* them (scrolls, books) or the
  *Learn spell* action, cast from the Magic tab or the quick bar. New conditions *Skill level*,
  *Knows spell*; actions *Set skill level*, *Learn spell*, *Damage around player*, *Light around
  player*; script functions `skill`, `skill_points`, `knows_spell`, `mana`, `set_skill`,
  `give_skill_points`, `learn_spell`, `add_mana`, `damage_around`, `player_light`.
  Bolt spells are aimed with the mouse (magic crosshair; click an enemy to fire, X straight
  ahead, right-click / Esc cancels).

- **Options menu** on the title screen and in the pause menu: fullscreen, zoom, show FPS,
  master / music / effects volume and dialog text speed (slow, normal, fast, instant), with a
  reset to defaults. Settings apply immediately and are stored per game in
  `~/.config/rust2d/<game>/settings.ron` (XDG config folder; `RUST2D_SETTINGS_DIR` overrides it).
- **Map screen** (M or the new map button bottom right): the current map from above with the
  player's position and facing and labelled exits to other maps; pauses the game while open.
  Scroll with the arrow keys / WASD, by dragging the map or the scrollbars, or the mouse wheel;
  zoom with + / -, Space centers on the player.
- **Fog of war** per map (map inspector › Map screen): the map screen only shows the explored
  area (about 6 tiles around everywhere the player has been); kept per map in the save game.

### Fixes
- Changing a map's music or lighting in the inspector now marks the map as changed and can
  be undone (before, only saving together with another change kept it).
- Loading a saved game no longer runs the map's *Map enter* triggers again (a load resumes the
  game; the player didn't enter the map).

### Example project
- Fog of war on the forest, the wilds and the mine; the village and its buildings have none.
- Eight skills (Swordsmanship, Archery, Toughness, Lockpicking, Haggling, Arcana, Spellcraft and the
  scripted Scavenging),
  four spells (Heal, Light, Spark, Fire Burst), scrolls, spell books and blue mana potions; quests
  reward skill points; the forest chest can be picked, the mine's chest only picked.

## 0.0.7 — 2026-11-04

### Gameplay
- **Lighting, day and night**: maps are *Always bright* (default), *Outdoor* (day and night with
  warm dusk and dawn; darker at night) or *Dark* (dungeons, caves: pitch black at 100 %). Light
  sources — tiles (wall torches, lamps, windows), props per state, characters, items (equipped or
  on the ground) and single placed entities — light soft circles with their own radius, color,
  strength and optional flicker. The clock runs while playing (day length and start hour in the
  project), is saved, shown top right on outdoor maps and available to events (*Set time of day*,
  *Time of day* condition) and scripts (`hour()`, `is_night()`, `set_hour()`).
- **Left hand slot**: a fourth equipment slot for a shield (defense) or a light (lantern, torch).
  Weapons can be *two-handed*; equipping one puts the left-hand item away and vice versa.

### Editor
- **Icons, ribbon and grouped menus**: the editor uses the Phosphor icon font. Map mode has a
  tool ribbon under the menu (Edit · Select · Paint · Collision · Objects · Layer · View · Play)
  with icon buttons whose tooltips give name, shortcut and a one-line how-to; the status bar shows
  the active tool's how-to; the left panel holds the tool's options with the layers underneath.
  Menus are grouped by task (File, Edit, Map, Assets, View, Play). Database tabs, library kinds,
  the pixel editor's tools and all action buttons have matching icons (no more box glyphs).
- **Structures**: multi-tile assets placed with one click — houses, wall and fence segments,
  rooms, ponds, groves, a plaza and more. New *Structures* map tool (H) with a categorized
  palette, a preview that follows the mouse and one undo step per placement. A structure sets
  tiles on all layers, collision overrides (walkable doors) and places its entities (signs,
  barrels, chests). *📐 New from map area…* turns a rectangle of the map into a new structure;
  *Database → Structures* lists them, with a **structure editor**: assemble a structure from
  tileset stamps (drag-select tiles in a tileset, paint them on the ground / detail / overhead
  layer), mark cells that clear the map's tile, set collision, place props, resize, undo / redo.
  *✨ New asset… → Structure* (empty grid or a copy), *🏠 New structure* in the Asset Library
  window and *➕ New…* in the palette start one. The editor has grouped tools on the left, a
  layer stack (Overhead / Detail / Ground with descriptions, show / hide and what each holds),
  zoom, and a tileset with stamp preview or a prop gallery on the right. Ctrl+Z / Ctrl+Y undo
  and redo in it. Structures live in `defs/structures.ron`, can
  be added from / to the asset library and exported in packages. Games need nothing new: placing
  writes ordinary tiles and entities.
- **Asset Library window**: the library opens in its own window (top bar *📂 Asset Library ↗*,
  or `rust2d-editor --library`). *Browse* shows all library assets with import, export and
  remove; *Edit* is the Database editor for library assets; the pixel editor and the new-asset
  wizard create and change library assets in place. Changes the main editor makes to the library
  are picked up automatically.
- **📥 Add from library…** dialogs in the Database tabs, next to the tileset and placement
  palettes and in the File menu: show the library's tilesets (or props, characters, items, sprite
  sheets, sounds) as thumbnails, pick one or more and add them with everything they need. The old
  *Library* mode of the main editor is replaced by these.
- **Removing assets from the library asks for confirmation** first. The dialog lists the assets
  and says up front when other library assets still use them (removal deletes their files and
  can't be undone).
- **Pixel editor: Ctrl+C / Ctrl+X / Ctrl+V** copy, cut and paste the selected frame (cut + paste
  moves a frame). Pasting now makes the frame exactly the copied one (transparent pixels
  included) instead of drawing the copy over the old content.
- **Pixel editor: *Select frame* is the default tool** (shortcut S): clicking into the image
  selects the frame under the cursor instead of drawing; pick the pencil (B) or another tool to
  draw.

### Example project
- Day and night in the village, forest and wilds: lit windows, the guard's lantern, the shrine's
  glow. Tilda sells a lantern, torches and a wooden shield; spear, bow and crossbow are
  two-handed. A pitch-dark old mine in the forest with wall torches, slimes and a treasure room.
- 18 structures from the terrain tileset (cottages, shop front, stone walls, fences, a walled
  yard, a fenced garden, bedroom / storage / small rooms, pond, bridge, well square, groves,
  rocks). The starter library adds 16 more from the city and dungeon tilesets and its props.

### Fixes
- Slimes showed a thin line of their own body above them when facing left, right or up (the
  sprite's bottom spilled into the next row of the sheet). Erasing in the art generator never
  worked either: key rings, the leather cap's opening and similar cut-outs are now transparent.

## 0.0.6 — 2026-11-04

### Gameplay
- **Loot tables**: defeated characters drop items. Each line of a table is an item, a chance in
  percent and an amount (min–max). Character definitions have a default table, placed characters
  can use their own, and enemy spawners pass their table on to the characters they spawn. Drops
  land where the character fell and stay (also in save games) until picked up.

### Editor
- **Unsaved markers follow undo**: maps, definitions, scripts and the pixel editor only count as
  changed (• marker, save prompt on switching maps or closing) while they really differ from the
  saved files. Undoing back to the saved state, or editing a field and changing it back, clears
  the marker again.
- **Mouse-wheel scrolling** in panels, lists, the inspector and the script editor moves about three
  lines per notch (it was barely half a pixel).

### Fixes
- Dropped items and spawned characters could get the same internal id (both started at
  0x4000_0000), which could mix them up. Dropped items now use their own range; piles in saves
  from 0.0.5 are renumbered when loaded.

### Example project
- Slimes drop coins and apples, goblin archers arrows and coins (replacing the scripted rewards).
  In the forest, the slime den adds potions and the goblin camp has its own table: arrows, coins,
  bolts, sometimes a leather cap and rarely a crossbow.

## 0.0.5 — 2026-11-04

0.0.4 was prepared but never released; its changes are part of this release.

### Player character
- **Character creator** after *New game*: the player chooses a name, male / female and a look
  (animated preview). Name, sex and look are kept in save games and used for the player's sprite.
- Games can **lock the player's sex** (*Player chooses* / *Always male* / *Always female*,
  Database → Project → Character creator).
- Dialogs, choices and quest texts can **address the player**: `{player}` for the name,
  `{sir|madam}` for male / female forms. New *Player sex* condition; scripts have
  `player_name()`, `player_sex()`, `is_female()`, `is_male()` and `text()`.

### Gameplay
- **Drop items**: in the inventory, G drops the selected stack (Shift+G one item), or drag a
  stack out of the window. Dropped items stay where they are — also after leaving the map and in
  save games — until picked up again.
- **Patrol routes**: placed characters can walk a route of waypoints (loop or back and forth,
  pause, speed), finding their way around obstacles. They stop briefly when talked to and
  continue where they were after map changes and loading. New *Patrol* action and
  `patrol()` / `stop_patrol()` in scripts.
- `say(SELF, "...")` and a `{self}` speaker show the name of the trigger's owner as speaker.

### Editor
- **New project wizard**: game name, location, player sex, character creator, optional starter
  assets from the library, first map (name and size), pixel scale and inventory size.
- **Script editor**: syntax highlighting, line numbers, auto-completion (engine functions with
  parameters, script functions, keywords, variables — and inside string arguments the project's
  items, maps, spawn points, quests, sounds, prop states, flags, variables, entity names and
  script functions), a parameter hint for the current call, auto-indent, closing brackets and
  quotes, Ctrl+click to jump to a function.
- **Script check**: unknown ids in script calls (e.g. `give_item("potoin")`) are listed under
  *Problems* and underlined in the code.
- **Renames update scripts**: renaming an item, map or sound also rewrites the matching script
  calls (backups as `<file>.rhai.bak`).
- **Patrol routes** are drawn and edited on the map (click to add, drag to move, right-click to
  remove waypoints).
- **Speaker dropdown** in *Dialog* / *Choice* actions with `{self}`, `{player}`, the characters
  on the map and speakers already used — names can still be typed by hand.
- **Trigger lists only offer events that can happen** for their owner (e.g. map triggers:
  *Map enter* and *Custom event*). Older triggers with an impossible event are marked ⚠ with a
  one-click fix.
- Creator looks and character models can be picked from the asset library.

### Example project
- The player creates a character (six looks); NPCs address the player by name and sex.
- A town guard patrols the village road and stops once the shrine is lit.

### Development
- Branching concept in AGENTS.md (feature → develop → release → master) and CLAUDE.md with
  instructions for Claude Code sessions.

## 0.0.3 — 2026-11-03

- **Trading**: merchants (stock, quantities and prices per character definition or placed
  character, currency, sell rate), item values, trade window, *Open shop* action and
  `open_shop()`; merchant stock is saved.
- **Character models**: choose a look from the project or the asset library for a character
  definition or a single placed NPC.
- **Quests**: Database → Quests, *Quest* action and condition, quest script API, quest journal on
  J (attack is X now).
- Example: Tilda's shop (female merchant, counter, sign) and a second quest *Pest Control*; NPCs
  can be talked to across counters.
- Closing the game window asks whether to save first.
- Editor: the Scripts tab lists functions per file and jumps to them, opens the first script and
  saves when switching files.

## 0.0.2 — 2026-11-03

- Larger starter library: desert, mountains, city and dungeon tilesets, more props, male and
  female variants of every humanoid character.
- Database: visual sprite chooser, embedded sprite-sheet pixel editor, create and edit art for
  items, props and characters.
- Maps: Ctrl+M quick switcher, Ctrl+PgUp / PgDn cycling, a camera per map, double-click a portal
  to jump to its target.
- Two-way *map transition* zones drawn with the Transition tool (T), linked across both maps.
- Play-tests warn when `rust2d-game` was built from a different engine.

## 0.0.1 — 2026-11-03

First version: a small 2D engine for top-down RPGs on Linux, written in Rust (macroquad + egui).

- Engine: chunked tile maps up to 1024×1024 (32 px tiles); items, props (with named states),
  characters, tilesets, sprite sheets and sounds; data-driven triggers and actions plus Rhai
  scripting; real-time melee and ranged combat, inventory, equipment, containers, key locks,
  quick bar, save games, title screen and pause menu; asset library and `.r2dpack` packages;
  standalone release bundles; conversion of older 16 px projects.
- Game: `rust2d-game` runs a project or a released bundle.
- Editor: `rust2d-editor` for maps, entities, zones, transitions and definitions, with an asset
  library, pixel editor, new-asset wizard and game export.
- Example project: a short quest with generated art, sounds and music.
