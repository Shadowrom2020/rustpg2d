# CLAUDE.md

Instructions for Claude Code sessions on **R(ust)PG2D**, a 2D top-down RPG engine with an editor
(Rust, macroquad + egui, Linux). The shared rules for everyone — branching concept, checks
before merging, commit messages — are in AGENTS.md and apply in full:

@AGENTS.md

## Working with the user

- The user asks for features in one or two sentences and expects them **working in the
  running binaries**, not just compiling. Verify every feature end to end (scripted runs +
  screenshots, see *Verification*) and report what was tested; say plainly what wasn't.
- Pick sensible defaults instead of asking, and mention them in the summary. Ask only when a
  decision really is the user's (e.g. which of two incompatible behaviours they want).
- **Branches:** follow AGENTS.md. Before any change: `git branch --show-current` and
  `git status`. Work on `feature/<name>` or `bugfix/<name>` created from an up-to-date
  `develop` (`hotfix/X.Y.Z` from `master` for urgent fixes of a release); never commit to
  `master` or `develop` directly — both are protected on GitHub, so pushes to them are refused
  and everything goes through pull requests. The user also commits and opens pull requests
  themselves, so fetch first and expect branches to have moved.
- **Commit, push, open / merge pull requests only when asked.** "Merge into develop" = push
  the branch, `gh pr create --base develop`, wait for CI (`gh pr checks --watch`), `gh pr merge
  --merge`, then `git checkout develop && git pull`. A release = `release/X.Y.Z` (SemVer: MINOR
  for features, PATCH for fixes) with the version bump + dated CHANGELOG section, PR into
  `master`, merge; the Release workflow tags, publishes the AppImage / Flatpak and opens the
  back-merge PR master → develop, which is merged too. Never tag by hand.
- The user's own data must not be changed without a backup: their projects (e.g. `~/Test`),
  released games (`~/releases`), the asset library (`~/.local/share/rustpg2d/library`) and
  `example_project/saves/`. Test on copies in the session scratchpad instead.
- `ROADMAP.md` holds the planned releases and their work items with design notes. When the
  user asks for the next feature or names a roadmap item, start from its block, follow *How to
  work from this roadmap* there and keep the item's status up to date.
- Keep `README.md` up to date with every user-visible feature (controls table, editor
  section, scripting API), and `Documentation/scripting.md` with every script API change
  (tests fail when an engine script function is missing there or in the editor's `API` table,
  and the document's code examples must compile).

## Workspace map

```
crates/engine/src/      library: everything shared by game and editor
  defs.rs               Database: tilesets, sheets, items, props (+states, container, lock),
                        characters (+shop), sounds, quests, structures; serde formats of defs/*.ron
  skills.rs             SkillSettings (project), SkillDef / SkillEffect, SpellDef / SpellEffect
  achievements.rs       AchievementDef (defs/achievements.ron), Progress = the player's unlocks
                        (achievements.ron next to settings.ron, not in save games)
  crafting.rs           RecipeDef (defs/recipes.ron): result, ingredients, station, skill level;
                        missing / craft / fits
  structure.rs          StructureDef::capture / place (multi-tile assets; editor-only, placing
                        writes ordinary tiles + entities)
  autotile.rs           auto-tiling terrains (TilesetDef.terrains: 16 tiles per N/E/S/W neighbour
                        mask + base tile + joins; Terrain.blob = 47-tile set with diagonals, BLOB_SLOT); paint() = the tile changes for one cell and its
                        neighbours (editor), retile() = a whole layer (gen_example: autotile_terrain)
  project.rs            ProjectConfig (project.ron), defs loading/saving (DEFS list),
                        TILE_SIZE = 32, CharacterCreation / SexChoice
  settings.rs           player options (Options menu): ~/.config/rustpg2d/<game>/settings.ron,
                        RUSTPG2D_SETTINGS_DIR overrides the folder (set it to a scratch dir in tests)
  render/character.rs   layered characters: every character is an Appearance (body sheet, skin,
                        hair, beard; monsters are bodies without hair); layers() in drawing
                        order (held behind, body, clothing, body armor, hair, beard, head armor,
                        held in front; outfits have a female cut, Armor::overlay_f, worn by
                        female body sheets); render/recolor.rs = skin (magenta) / hair (cyan)
                        reference ramps recolored to the chosen colors. SheetDef.playable = body offered
                        by the creator (Database::playable_bodies). Old projects: a
                        character's legacy `sheet` becomes its body (Database::upgrade_characters)
  fog.rs                fog of war: Explored (tiles per map, saved as a compact string), REVEAL_RADIUS
  state.rs              WorldState = save game (flags, vars, inventory, quests, shops,
                        player name/sex/look), Sex, expand_text() placeholders
  map/                  Map, tile layers, zones (spawn, portal, trigger, spawner, transition),
                        io.rs = map file format (postcard tiles + RON `Meta`)
  events/               GameEvent, TriggerOn, Cond, Action (mod.rs); ActionRunner (runner.rs)
  script.rs             Rhai engine and the script API
  script_refs.rs        ids inside scripts: string arguments of API calls (API_IDS), used for
                        renames (rename_in_script) and the editor's script check
  runtime/              Game: mod.rs (loop, actions, drawing), charwin (the tabbed character window:
                        Character = inventory_ui, Skills / Spells = skills; C/I, K, L, Tab), cutscene (Play
                        cutscene: HUD off, bars, walk_to / approach / place), combat, equipment, locks,
                        inventory_ui, shop_ui, crafting_ui (Open crafting window), achievements (checks, toasts), minigame (lockpicking, fishing), furniture (sitting, sleeping: PropDef.use_spot), gather (gathering props with a tool, regrowing prop states), farm (farm plots: planting seeds, crops growing, harvest), activities (characters seen fishing / hammering / reading at their spot), crafting_ui also checks RecipeDef.station_state against the station prop (cooking fires), orders (companion orders menu, G / pad LB), companion_ui (a companion's window: gear, bag, the player's inventory), npc_use (characters use the items they carry: potions when critical, cures, buffs; companion ammo), healing ("heal whom?" choice for heal spells / items with ItemDef.use_on, companions' HUD rows, Action::HealCharacter), journal, creator, patrol (routes; walk_to_tile = A* walking shared with schedule = daily routines, stops on other maps = visitors, furniture), companions (follow, fight, carried across maps via WorldState.companions), lights (light sources,
                        clock), skills (skill bonuses, mana, spells, aiming, K screen), fx (Fly /
                        Effect actions: scripted flying sprites and one-shot animations), menu (title/pause/options),
                        worldmap (M map screen), paperdoll (drawing layered characters, player model),
                        npc_items (NPC equipment / carried items: combat, drops, saved changes), mouse,
                        spawner, ui (dialogs, toasts), input (+ injection for autoplay)
  quest.rs, inventory.rs, entity.rs, path.rs (A*), render/ (Assets, chunked tile renderer;
                        lighting.rs = day/night ambient, LightPass light map, tile_lights;
                        parallax.rs = Map.parallax image layers behind / over the map)
  library.rs            asset library + .r2dpack bundles; refactor.rs = id renames / refs_of_*
  migrate.rs            converts old 16 px projects; release.rs = standalone game export
  examples/gen_example/ generates example_project/ and the editor's starter.r2dpack; music.rs
                        composes and renders the two music tracks (tiny synth, Ogg Vorbis via vorbis_rs)
  paths.rs              per-user folders per platform (XDG / %APPDATA%, %LOCALAPPDATA%, %USERPROFILE%)
  legacy.rs             the old name rust2d: adopt() copies a folder / file from the old location
                        (~/.config/rust2d, ~/.local/share/rust2d, ~/.cache/rust2d) on first use
  glibc_compat.rs       math functions newer glibc re-versioned, defined in the engine so the game
                        and editor binaries need only glibc 2.34 even when built on a rolling
                        release (check: `readelf -V target/release/rustpg2d-game`, weak needs don't count)
crates/game/src/main.rs game binary: CLI args, window, RUSTPG2D_AUTOPLAY script runner
crates/packaging/       AppImage (pinned type2 runtime + squashfs via backhand) and Flatpak bundles
                        (flatpak CLI, or `flatpak-spawn --host` inside a sandbox) from an AppSpec;
                        hero_icon, glibc_needed (ELF version needs)
crates/xtask/           `cargo xtask check-version | check-branch | release-notes` (no engine, builds in
                        seconds) and `cargo xtask-dist` (feature `dist`: toolkit AppImage + Flatpak)
.github/                workflows/ci.yml (required checks), workflows/release.yml (merge into master =
                        tag + GitHub Release with the toolkit AppImage / Flatpak + back-merge PR),
                        actions/packages (build + smoke test), dependabot, PR / issue templates
crates/editor/src/      editor binary
  main.rs               loop + RUSTPG2D_EDITOR_SCRIPT command runner
  app.rs                Editor state, menus, inspector, Pending/close-project logic, play-test
  graph.rs (Events mode: the map's triggers / scripts / maps as a graph; build() = nodes + edges),
  import/ (File › Import…: tiled.rs .tmx/.tmj → map + tilesets, lpc.rs walk cycle → character
  sheet, aseprite.rs JSON → sheet + SheetDef.tags; fixtures in crates/editor/tests/fixtures),
  canvas.rs (map view + tools), doc.rs (map doc + undo; Edit::Group), structures.rs (Structures
  tool, capture, thumbnails), clipboard.rs (region copy / cut / paste), terrain.rs (Terrain tool,
  Database › Tilesets › Terrains), database.rs (Database tabs),
  widgets.rs (sprite pickers, trigger/condition/action editors, shop/container/lock UIs),
  maps.rs (map switching, transitions), builder.rs (character builder: cards + preview, the
  New character / Customize window, `char…` script commands), models.rs (template chooser:
  copy a project / library character's look),
  script_editor.rs (Rhai editor: highlighting, completion, API table with params/docs),
  ui.rs (icon font, ribbon / tool-button helpers),
  new_project.rs (wizard), library_ui.rs (library Browse view, "Add from library" dialog,
  thumbnail grid), library_window.rs (`--library`: the editor in library mode on the asset
  library, opened as its own process), pixel.rs (pixel editor), wizard.rs (new asset),
  export.rs (release export dialog), package.rs (a game's AppSpec for crates/packaging; icon),
  files.rs (file dialogs, create_project, recent list, bundled example of packaged R(ust)PG2D)
example_project/        generated by gen_example — edit the generator, not these files
```

## Coding guide

- Match the surrounding style: long lines (~160 chars) are normal here, `///` doc comments on
  items, short comments that explain *why*. Rust 2021; clippy must pass with `-D warnings`.
- Errors: `anyhow::Result` with `.context(...)` in IO code; the game never panics on bad
  data — log with `eprintln!` (prefix like `[game]`, `[events]`) and carry on.
- Don't add dependencies without a good reason. Pinned stack: macroquad 0.4.16 (with
  `features = ["audio"]`), egui-macroquad 0.17.3 (egui 0.31) and with it egui-phosphor 0.9 (each
  later egui-phosphor needs a newer egui; Dependabot ignores all four), postcard + lz4_flex, ron,
  rhai, rfd, zip 9, png 0.18 (`imageio.rs`; macroquad keeps its own png 0.17 internally).
- **Compatibility is a hard rule** (maps, defs and save games of older versions must load):
  - New variants of `Action`, `TriggerOn`, `Cond`, `ZoneKind`, `OnUnlock` are **appended**
    at the end (map files store variant indices).
  - New fields get `#[serde(default)]` (or `default = "fn"`). New map object fields go in the
    RON `Meta` in `map/io.rs`; never edit the frozen `EntityV1` there.
  - Save games are `WorldState` RON: every new field needs a serde default.
- Tiles are always 32 px; sizes in code are in pixels at that size (`Game::u()` = ts/16 scales
  effects). Rhai reserves `var`, hence `get_var` / `set_var`.
- Game UI (macroquad's default font) is ASCII only: no `∞`, `▲`, `✔` in `draw_text`. The editor
  uses the Phosphor icon font (`crates/editor/src/ui.rs`: `ph::NAME` constants, `ui::t(icon,
  text)`, `tool_button` / `icon_button` / `toggle` with name + shortcut + how-to tooltips,
  `group` for ribbon groups, `kind_icon`). Don't put emoji or `→` in egui strings — most render
  as boxes; use a `ph::` icon (or `ph::CARET_RIGHT` for arrows in prose).
- egui closures: take what you need out of `self` first (or use thread_local request hooks as
  in `maps.rs`, `models.rs`, `widgets::request_chooser`) to avoid borrow conflicts.

### Where a new feature usually has to touch

- **New `Action` / `Cond` variant:** `events/mod.rs` (variant at the end, `label()`,
  `templates()`), execution in `runtime/mod.rs` (`execute`) or `Condition::eval` /
  `runner::conditions_met`, editor fields in `widgets.rs` (action fields / `conditions_ui`),
  `refactor.rs` if it references ids, and a Rhai function in `script.rs` if useful.
- **New script function with an id argument** (item, map, sound, quest, …): add it to
  `script_refs::API_IDS` so renames and the script check cover it, and to the `API` table in
  `editor/src/script_editor.rs` (parameters, argument kinds, description) for completion. Note `go` and a few
  other words are reserved in Rhai.
- **New definition fields:** update every struct literal (`gen_example/main.rs`,
  `gen_example/art/extra.rs`, `editor/database.rs`, `editor/wizard.rs`, tests in
  `library.rs` / `refactor.rs`), the editor form in `database.rs`, renames in `refactor.rs`,
  and dependencies in `refs_of_*` so library export carries what it needs.
- **New defs file:** `project.rs` (`DEFS`, load, save), `release.rs` (exported files),
  `library.rs` (`Database` literals).
- **Player-facing text:** run it through `WorldState::expand_text` (`{player}`,
  `{sir|madam}`, `{var}`) like dialogs, choices and quest texts.
- **Example content:** change `crates/engine/examples/gen_example/` and regenerate
  (`cargo run --example gen_example`); keep entity uids of existing maps stable by adding new
  entities at the end of a map function (saves reference uids).
- **Testing hooks:** add autoplay / editor-script commands and extend `Game::status()` so the
  feature can be verified from a script.

## Verification

```sh
cargo build --workspace            # ALWAYS after engine changes: the editor's play-test and
                                   # scripted game runs use target/debug/rustpg2d-game
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace  # keep release binaries current too (export, user runs)
```

Use the session scratchpad for everything temporary: copy `example_project` (minus `saves/`)
there and test on the copy. Set `RUSTPG2D_MUTE=1` and `RUSTPG2D_SETTINGS_DIR=<scratch>/settings` (the game's Options menu saves there), and for the editor
`RUSTPG2D_EDITOR_CONFIG=<scratch>/cfg RUSTPG2D_LIBRARY=<scratch>/lib`.

- **Game:** `RUSTPG2D_AUTOPLAY="wait 1; press e; hold right 0.5; rclick 6 3; type Name; shop buy 0; close; status; shot /path/a.png; quit" target/debug/rustpg2d-game <project> [--no-title] [--map M --spawn S | --pos X,Y]` (`--pos` is in pixels: tile × 32 + 16).
  Commands: `wait`, `press` (`press shift+g` holds Shift), `hold`, `pad BUTTON[+BUTTON]` / `padhold BUTTON SECS` (gamepad: `a b x y lb rb lt rt select start ls rs up down left right`; `status` prints `pad=`, `attacking=` (attack cooldown running) and `shake=` (seconds left, `off` when disabled); `weather=` kind:intensity(map|set), particles, rain sound volume); `schedule=` name:stop-time[>map](walking|inside|elsewhere|stopped)[:sit|:lie]@x,y per scheduled character; `pose=` sit|lie|stand#uid while on furniture; `gather=` def:hits/needed, `crops=` seed:stage/stages|ripe@x,y, `activities=` name:kind@x,y (characters at work now), `parallax=` NbehindMabove (layers drawn), `music_loop=` whole|stretched|plain|- (gapless music), `props=` def:state@x,y (props not in their first state), `day=`; `aim=heal:<name>` while choosing whom to heal (`-` otherwise); `achievements=` unlocked ids (menu page `achievements`); `companions=` name:hp/max[:down][:defend|:passive][:stay][:fighting]@x,y, `orders=` who:choice while the orders menu (G) is open, `compwin=` name[bag] while a companion window is open (`slot` / `slotdrag` then take `gear`, `bag`, `player`), `click`/`rclick X Y` (tiles), `clickenemy`, `slot`,
  `slotdrag` (`slotdrag player 1 out` drags a stack out of the window), `shop buy|sell N`, `type`, `close` (window close button), `settime H` / `settime +H` (H hours later: days pass, props regrow), `skillpoint|learn|cast|call ID` (call runs a script function), `console LINE` (a developer console line; the game needs `--dev`, which editor play-tests pass: hot reload + F4 console; `status` starts with `dev=off` or `dev=on console=open|closed reloads=N reload_error=B`), `opt ROW prev|next|row` (Options page arrows), `hud window|map` (the character window button / map), `wintab character|skills|spells` (click a tab of the open character window; keys `c`, `i`, `k`, `l`, `tab` too), `press h` (hide / show the HUD; status prints `ui_hidden=`; `menu=` the open menu page, e.g. `defeated` after a death, and `autosave=` none / pending / saved; `cutscene=` the playing cut-scene; start with `--cutscene FN` to preview one without the map's Map enter triggers), `minigame hit|miss|bite` (press E when the lockpick marker is in / far from the sweet spot, or when a fish bites; `status` prints `minigame=`), `craft K` (Make button of recipe row K in the open crafting window; `status` prints `crafting=`), `mapbtn close|zoomin|zoomout|center`, `drag X1 Y1 X2 Y2` / `wheel DY` (screen pixels), `status`,
  `spawners`, `shot`, `quit`. `status` prints player, creator, quests, shop, map, tile, hp,
  vars, inventory, flags. Load a crafted save with `press f9` from `<project>/saves/quick.ron`
  (a minimal `WorldState` RON) to start in a specific situation; loading never fires *Map enter*
  triggers, so a crafted save starts quietly.
- **Editor:** `RUSTPG2D_EDITOR_SCRIPT="wait 1; open village; select named tilda; mode db; tab quests 1; shot /path/b.png; quit" target/debug/rustpg2d-editor <project>`.
  Many commands exist (`grep '=> {' crates/editor/src/main.rs`): tools, placing, zones,
  transitions (`linkarea`, `trdraw`…), library (`libadd`, `tolib`, `libdialog KIND`, `libpick`,
  `libdialogadd`, `libwindow`, `libedit`…; scripted assets: `tolib` / `projexport KIND ID PATH` open the *Script functions* dialog → `scriptpick FN on|off`, `scriptpickok`, `scriptpickcancel`; `libimport` / `projimport PATH` import packages, `libfilter KIND`), structures (`structure ID`, `stplace X Y`,
  `stcapture X Y W H ID`, `stinfo`; structure editor: `stnew`, `stsize W H`, `stlayer N`,
  `sttool stamp|keep|clear|solid|walkable|auto|prop`, `ststamp TS X Y W H`, `stprop ID`,
  `stpaint X Y`), `hover X Y` (tool previews), terrains (`tool terrain`, `terrain NAME`, `terrainpaint X Y [erase]` = a Terrain-tool click / Shift+click, `terrainedit K [block]` = edit terrain K in Database › Tilesets, `terrainblob TILESET NAME on|off` = its Corners (47 tiles) toggle), `mapweather none|rain|snow|fog [PERCENT]` (the open map's weather), schedules (`scheduleedit on|off` = edit the selected character's daily schedule on the map, `click` adds stops; `stop K hour H|facing up|any|inside on|off|map NAME|-|prop NAME|-|tile X Y`; `status` prints `schedule=`), `viewopt weather on|off` (weather preview), `dbscroll Y` (scroll the Database form, a big Y = the end), `view CX CY ZOOM` / `viewopt zones|grid|collision|weather|parallax on|off` (camera and view toggles for screenshots), imports (`import tiled|lpc|aseprite FILE [ID] [KIND]` print the report, `importdialog`; `actionsheet SHEET` sets the selected entity's last Effect / Fly sheet; `tab KIND ID` also selects by id), Events graph (`mode events`, `graph scope map|selection`, `graph functions map|all`, `graphnode N` select, `graphedit N` toggle a trigger's editors, `graphtext N K TEXT` = edit action K's text in node N, `graphview N` zoom to a node, `graphdump` nodes + edges; `status` prints `graph=nodes,edges`; node indices are those of the last frame), `play [start]` (F5 / F6 play-test; the game inherits the environment, so set `RUSTPG2D_AUTOPLAY` to drive it), clipboard (`region X Y W H` selects tiles, `copy` / `cut` the region or selected entity, `paste X Y` = top-left corner, `paste` arms it for `hover` / `click`, `unpaste`; `status` prints `sel=`, `clipboard=`, `pasting=`), `lighting on|off [HOUR]` (lighting preview), `mapfog on|off` (map's fog of war), `addaction LABEL` (append an action template to the selected entity's trigger), `entequip ITEM` / `entskin RRGGBB` (selected NPC's own equipment / look), pixel editor (`px…`),
  character builder (`charnew [ID NAME]`, `charcustom` = selected NPC, `charset body|hair|beard|skin|haircolor|clothing|head|armor|weapon|offhand|carry|id|name VALUE` on the open builder else the selected character, `charrandom`, `chartemplate ID` (project or library character), `charcreate` / `charapply`, `charcancel`, `charreset`, `playable SHEET on|off`; `status` prints `builder=`),
  `example` (welcome screen's *Example project*: only in packaged R(ust)PG2D; copies to
  ~/rustpg2d/example_project — set HOME to a scratch dir in tests), export (`exportgame OUT [NAME]` = folder + zip for the host platform, `exportwin` / `exportall OUT [NAME]` = Windows / both, `exportpkg OUT appimage|flatpak|all [APP_ID]`,
  `exportdialog [appimage] [flatpak]`; test packages on scratch output: the AppImage runs directly,
  a Flatpak goes into `FLATPAK_USER_DIR=<scratch>` with `flatpak install --user --no-deps --bundle`
  and `flatpak run --user --runtime=org.freedesktop.Sdk//25.08 …`; remove the `~/.var/app/<id>`
  folder a test run creates; older distros: the AppImage in `debian:bookworm-slim` via docker with
  the X11 socket, `--ipc=host`, `LIBGL_ALWAYS_SOFTWARE=1`, `--appimage-extract-and-run`),
  sounds (`sndplay [ID]` / `sndstop [ID]` = the Sounds form's Play / Stop, default the selected sound; `status` prints `preview=` and `music=`),
  template chooser (`model def|inst|builder`, `modelpick`, `modeluse`), wizard (`newwizard`, `wiz`,
  `wizcreate`), `scriptsel`, `probe X Y` (egui layer at a point), `status`, `dump`.
- **Asset Library window:** run `rustpg2d-editor --library` (with `RUSTPG2D_LIBRARY=<scratch>/lib`)
  and a script directly; `libwindow` from the main editor passes `RUSTPG2D_LIBRARY_SCRIPT` to the
  child as its script.
- The editor script cannot click egui widgets; script commands call the same functions
  instead. For **real egui clicks/typing**, build a throwaway copy of the workspace in the
  scratchpad with a patched egui-macroquad (`[patch.crates-io]`) that exposes
  `inject_mouse` / `inject_text`, plus `eclick X Y` / `etype` commands — never patch the real
  repository. Add `ekey NAME [ctrl]` (egui key events) the same way for keyboard tests; after
  a Ctrl combination send a Ctrl key-up, or egui keeps ignoring typed text. There is no xdotool / Xvfb on this machine, and XTest on XWayland may prompt the
  user, so don't use it.
- **Windows builds:** cross-compile in a throwaway container (`rust:1-bookworm` +
  `gcc-mingw-w64-x86-64` + `rustup target add x86_64-pc-windows-gnu`; scratch `CARGO_HOME` /
  `CARGO_TARGET_DIR`, `--user $(id -u)`), never on the host. Run the exes with Wine and a scratch
  `WINEPREFIX`; on this machine Wine's EGL fails on the NVIDIA GPU, so use `env -u WAYLAND_DISPLAY
  __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json LIBGL_ALWAYS_SOFTWARE=1`.
  Paths for the game under Wine: `Z:/…` (shots, project folders).
- Look at screenshots with the Read tool; crop with PIL when details matter. Lighting:
  `RUSTPG2D_LIGHTMAP_DEBUG=1` draws the raw light map instead of multiplying it.
- Dialog text types out over time: wait ~2.5 s before reading a screenshot, and the first `E`
  only completes the text, the second closes it.

## Pitfalls seen before

- egui refuses textures wider or taller than 2048 px (the editor shrinks them with
  `widgets::color_image`): keep generated sheets under that, e.g. `items.png` is full at 61
  icons; new icons go on their own sheet (`food.png`).
- The project was called **rust2d** until 0.5.0 (technical name now `rustpg2d`; the local checkout
  may still be `~/Projects/rust2d`). The user's old folders under that name
  (`~/.config/rust2d`, `~/.local/share/rust2d`, `~/rust2d/releases`) are their data: never
  delete or rename them; `legacy::adopt` only copies from them. Test runs must set
  `RUSTPG2D_SETTINGS_DIR`, `RUSTPG2D_LIBRARY`, `RUSTPG2D_EDITOR_CONFIG` (and `HOME` for the
  example copy), or the adoption copies the user's real folders.
- A game binary older than the engine can't read new map data (`Unexpected variant …`);
  the editor warns when `rustpg2d-game --engine-build` differs. Rebuild the workspace.
- `example_project/saves/` may contain the user's saves — never delete, regenerate over or
  commit it (it is git-ignored).
- The starter pack merges library-only extras by id; the example project's version wins.
  Existing user libraries only gain *new* ids (`Bundle::add_missing`), except that an existing
  image the new pack only extends (more frames, old pixels unchanged) is replaced. The starter
  is applied when `starter.stamp` (rules version + pack hash) changes: bump `STARTER_RULES` in
  `library.rs` when the way starter sets are applied changes, so existing libraries rerun it.
- macroquad `Image::export_png` flips vertically (the generator pre-flips).
- Generated characters are drawn by `art.rs::draw_part` (Full = whole figure, used for skeleton
  bodies; Body / Hair / Beard / Outfit = layers): bodies use `SKIN_REF`, hair `HAIR_REF`; overlays
  get an outline only outside the body (`outline_outside`). `Look.female` gives the feminine
  body shape and the fitted outfit cut (`outfit_cuts` writes `outfit_X` and `outfit_X_f`);
  `HairStyle` / `BeardStyle` pick the hair / beard sheets. Held items use the walk animation's
  hand positions (`hand()`). Sheet ids: `body_male` / `body_female` / `body_goblin` /
  `body_goblin_female`, `hair_short|long|ponytail`, `beard_full|goatee`.
- Interaction: E checks the tile in front of the player (also across a counter / solid
  tile for NPCs); right-click walks there first. Place test players accordingly.
