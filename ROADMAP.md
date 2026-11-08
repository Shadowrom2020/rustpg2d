# R(ust)PG2D roadmap

Planned features of R(ust)PG2D, grouped into themed releases, with the design decisions and the
code hook points already worked out so that a session can start an item without re-exploring the
codebase. What is shipped is in [CHANGELOG.md](CHANGELOG.md); the rules for branches, versions
and releases are in [AGENTS.md](AGENTS.md); coding and verification rules in [CLAUDE.md](CLAUDE.md).

Status marks: `[ ]` open · `[~] feature/<branch>` in progress · `[x] X.Y.Z` shipped.

## How to work from this roadmap

Instructions for every session (people and coding agents) that implements roadmap items:

1. **Pick the item.** Unless the user names one, take the first open item of the lowest
   unreleased version, in the order listed (later items build on earlier ones). Read its whole
   block: *Goal*, *Design* (defaults are decided; keep them unless the user says otherwise),
   *Touch points*, *Example*, *Verify*. Line numbers are not given on purpose: find the named
   functions and structs with grep, they move.
2. **Branch.** `git checkout develop && git pull --ff-only && git checkout -b feature/<item-slug>`
   (the slug is given in the item's heading). One item per branch; a release's items are
   separate pull requests into `develop`.
3. **Mark it.** Change the item's mark to `[~] feature/<slug>` in the first commit of the branch
   and to `[x]` (version left empty until the release) in the last one. Keep the *Design* block
   true: if the implementation deviates, edit the block so the roadmap matches the code.
4. **Compatibility is a hard rule** (see CLAUDE.md): new `Action`, `Cond`, `TriggerOn`,
   `ZoneKind`, `OnUnlock` variants are appended; new fields get `#[serde(default)]`; new map
   object fields go into the RON `Meta` of `map/io.rs`; save games (`WorldState`) and
   definitions of every released version must still load. Add a test that parses an old RON
   without the new fields when you add fields to `WorldState` or a definition.
5. **A feature is done when** it works in the running binaries, is covered by the
   scripted checks in *Verify* (autoplay / editor script + screenshots, on a scratch copy of
   the example project), has tests where the engine logic allows, is documented
   (`README.md`, `Documentation/scripting.md` for every script function, the editor's `API`
   table, CHANGELOG *Unreleased*), and `cargo build / clippy -D warnings / test --workspace`
   pass. The example project (`crates/engine/examples/gen_example/`) shows the feature: add
   new entities at the end of a map function so uids stay stable, then regenerate.
6. **Commit, push and open pull requests only when the user asks.** When all items of a
   version are `[x]` (or the user decides to ship early), cut `release/X.Y.Z` as described in
   AGENTS.md, fill in the version in the marks, and move the version's section to *Shipped*
   at the end of this file (one line per item).
7. **Changing the roadmap** is fine: reorder, move items between versions, split or merge
   them, add new ideas to *Backlog*. Keep every item's block in the same shape. Don't delete
   design notes of open items; replace them.
8. **Record what you decided.** When the implementation deviates from the *Design* block, add
   a *Decided while building* paragraph to the item (before *Touch points*) instead of
   rewriting the design; later items and the release notes rely on it. The *Verify* block
   lists what was actually checked; say in the report what was compile-checked only (e.g.
   editor fields below the visible area, which the editor script cannot scroll to).
9. **Scripted checks that hold up.** Park the autoplay mouse before navigating menus
   (`drag 2 2 2 2` as the first command), or the real pointer over the test window changes
   the selection. Start from a crafted `saves/quick.ron` (`press f9`) to skip arrival
   cut-scenes; a pending dialog blocks autosaves and queued actions (`press e` twice). Add a
   `status()` field for every new state (`attacking=`, `shake=`, `pad=`, …) rather than
   reading screenshots for it; use screenshots for looks. Never delete with a relative path
   after `cd`; use a fresh scratch folder instead.
10. **Dependencies and packages.** A new crate that links a system library (gilrs → libudev)
    means: the CI apt lines (`ci.yml`, `release.yml`), the README's build prerequisites, a
    check that the release game still needs glibc ≤ 2.34 (`objdump -T target/release/
    rustpg2d-game | grep GLIBC_`), the Debian 12 container (`ldd`), and the Flatpak permissions
    (`--device`, `--socket`) in `editor/src/package.rs` and `xtask/src/dist.rs`.
11. **Example content.** Regenerate with `cargo run -p rustpg2d-engine --example gen_example`
    and commit `example_project/` and `crates/editor/assets/starter.r2dpack` together; new
    entities go at the end of a map function; an example id overrides the starter library's
    (give new example characters their own ids); never touch `example_project/saves/`.
12. **Branches that touch the same files.** Start the second item's branch from the first one
    (as *Save thumbnails* did from *Screen shake*, both in `menu.rs`), open the pull requests
    in that order, and say so in the pull request text. One pull request per item.
13. **Documentation per item**, all before the report: README (feature bullet; controls, events
    and scripting summaries), `Documentation/scripting.md` for every script function (a test
    fails otherwise), the editor's `API` table, CHANGELOG under the matching `###` heading of
    *Unreleased* (Gameplay, Editor, Fixes, Maintenance), the roadmap mark.
14. **CI hiccups.** A job that fails within a minute with a download timeout
    (`static.rust-lang.org`) is the runner's network: `gh run rerun <id> --failed`.
15. **Cutting a release.** `release/X.Y.Z` from develop: version in `Cargo.toml` (the build
    updates `Cargo.lock`), `## Unreleased` → `## X.Y.Z — date`, README *Current version*,
    the version's roadmap section replaced by one line per item under *Shipped* (its design
    notes live on in the CHANGELOG and the code), `cargo xtask check-version release/X.Y.Z`,
    pull request into master, merge after the four checks, merge the back-merge pull request.

## 0.9.0 — Reach: the web and other languages

Theme: games made with R(ust)PG2D reach players without an install (a browser tab, itch.io)
and in their own language. Windows and macOS moved to the backlog on 2026-11-08 (see
*Postponed: Windows and macOS*): they come back when hardware to develop and test on is there.
Order: web export first (it changes how the game loads data, which localization then uses).

### [ ] Web export — `feature/web`

**Goal.** *File › Export game* gains *Web*: a folder (and zip) with an `index.html`, the game
compiled to WebAssembly and the game data, that runs in current browsers (Firefox, Chrome,
Safari) from any static web server or as an itch.io HTML game. Same game, same saves format.

**Design.**
- Target `wasm32-unknown-unknown` for the **game runner only** (the editor stays native).
  macroquad / miniquad support the web through their JS loader (`mq_js_bundle.js`, shipped
  with the export); `Conf` window size becomes the canvas, `high_dpi` kept.
- **Loading data.** The runtime reads the project with `std::fs` in 17 engine files; on the web
  everything comes from the export's `data/` over HTTP. Introduce a small virtual file layer
  (`engine/src/vfs.rs`: `read(rel) -> Result<Vec<u8>>`, `exists`, `list(dir)`) used by
  `Project::load`, map loading, `Assets::load`, `Audio::load`, scripts; native = `std::fs`
  under the root, web = an in-memory map filled before the game starts from `data/files.ron`
  (a manifest the export writes) with `macroquad::file::load_file` (async; the game's `main`
  already is). Writes (saves, settings, achievements) go through the same layer: native =
  files as today, web = `localStorage` keys `rustpg2d/<slug>/<path>` via a tiny JS plugin
  (miniquad `#[no_mangle]` imports; save files are RON text, thumbnails PNG → base64).
- **Platform gaps.** `audio.rs` prepares music loops on a `std::thread` (no threads on the
  web: do it on first play there), `menu.rs` uses `SystemTime::now` (save dates: `web-time` or
  `js_sys::Date` through the plugin, chosen while building), `std::env` (autoplay, flags) is
  native-only (`cfg`), gilrs is off on the web (`cfg`; the browser Gamepad API through
  miniquad later), `glibc_compat` is Linux-only already. Rhai, ron, postcard, lz4_flex and png
  build for wasm. Audio needs a first click (browsers block sound before it): the title screen
  shows *Click to start* on the web.
- **Export.** `release::Platform::Web`: `<slug>-web/` with `index.html` (canvas, loader, title,
  a loading bar), `rustpg2d-game.wasm` (`wasm-opt -Oz` when available), `mq_js_bundle.js`, the
  plugin JS, `data/` + `files.ron`; zip `<slug>-web.zip` (itch.io wants the `index.html` at the
  zip root). The editor looks for `rustpg2d-game.wasm` next to itself / in
  `target/wasm32-unknown-unknown/release`; CI builds it in the Packages job and the AppImage /
  Flatpak ship it, so the Linux editor exports web games.
- **Size.** Measure the wasm (target < 10 MB uncompressed); data is fetched once per page load
  (no streaming of maps in this item).

**Touch points.** `engine/src/{vfs (new),project,release,settings,achievements,audio}.rs`,
`map/io.rs`, `render/mod.rs` (`Assets::load`), `runtime/{mod,menu}.rs`, `game/src/main.rs`,
`editor/src/export.rs`, `packaging` / `xtask/src/dist.rs` (ship the wasm), `.github/actions/
packages`, README (*Releasing a game › Web*).

**Example.** The example project exports for the web unchanged; the README links how to try it
(`python3 -m http.server` in the export folder).

**Verify.** Unit tests: the manifest lists exactly the exported files; vfs native vs. memory
backends; save → localStorage key round trip (plugin mocked). Export the example for the web,
serve it locally, open it in a browser on this machine (Firefox / Chromium): title screen, new
game, walk, a dialog, music after the first click, quick save, reload the page, *Continue*.
Screenshots of the browser. CI: `cargo build --target wasm32-unknown-unknown -p rustpg2d-game`
on every PR (catches native-only code early).

### [ ] Localization — `feature/localization`

**Goal.** A game can be played in several languages: creators write it in one language, export
a translation template, have it translated, and players pick the language in *Options*. The
runtime's own texts (menus, HUD, windows) are translated too (English and German shipped).

**Design.**
- **Content.** Translations live in `lang/<code>.ron` (e.g. `lang/de.ron`): `{ "original text":
  "translation" }` keyed by the original string, so existing projects need no ids and a missing
  entry falls back to the original. Covered: dialog lines, choices, item / prop / character /
  quest / skill / spell / recipe / achievement names and descriptions, quest stages, signs,
  script `say` / `choice` texts (they reach the runtime as strings). The lookup happens in
  `WorldState::expand_text` (already the chokepoint for every player-facing content text) and
  in the few name lookups that bypass it (inventory, shop, journal: find them with grep).
- **Runtime UI.** The ~170 `draw_text` strings in `runtime/` move into one table
  (`runtime/ui_text.rs`, `t("Continue")`, English as the key) with built-in English and German;
  a project can override or add languages in its own `lang/<code>.ron` (same file, a `ui`
  section). Text widths are measured already (`measure_text`), so longer German strings only
  need the few fixed-width buttons checked.
- **Choosing.** `ProjectConfig.languages` (codes offered, first = the original); Options row
  *Language* (saved in `settings.ron`, default: the system language when offered — `LANG` on
  Linux, `navigator.language` on the web — else the original). The game font stays ASCII-only
  today: languages with letters outside it (ä, ö, ü, é…) need a font with them — ship a small
  pixel TTF covering Latin-1 + Latin Extended-A with the engine and use it for all game text
  (macroquad `load_ttf_font_from_bytes`), which also fixes `ä` in names typed by players.
- **Editor.** *Database › Project › Languages* (codes), *File › Export translation template…*
  (writes `lang/<code>.ron` with every content string, existing translations kept, new ones
  empty, unused ones listed at the end), a *Translations* check (missing / outdated entries per
  language); the play-test can start in a language (`--lang de`).
- Save games store no texts, so they work across languages.

**Touch points.** `engine/src/{lang (new),state,project,settings}.rs`, `runtime/{ui_text (new),
menu,ui,inventory_ui,shop_ui,journal,…}.rs`, a font file under `crates/engine/assets/`,
`editor/src/{database,app,main}.rs`, `script.rs` (nothing new expected: texts pass through
`expand_text`), gen_example (a German `lang/de.ron` for the example, at least the village),
README, `Documentation/scripting.md` (how script texts are translated).

**Example.** The example project offers English and German; the village and the first quest are
translated completely.

**Verify.** Unit tests: lookup with fallback, template export keeps existing translations and
lists unused ones, `expand_text` placeholders inside translated text. Autoplay with `--lang de`
(or the Options row): the title menu, a dialog of the elder, the inventory and a quest in the
journal are German (status prints `lang=de`, screenshots); switching back to English at runtime.
Editor script: export the template, check its entries.

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS packages (today: unsigned, documented).
- Gamepad button glyphs in the HUD and hints (Steam Deck); rebinding gamepad buttons.
- Companions learn spells from scrolls and books given to them and cast heal / bolt spells by
  the rules of *Companion inventory* (`Companion.spells`, a mana pool from the definition).
- Activities that produce something: the fishing NPC sells fresh trout at a market stall the
  next morning (a trigger at the end of an *NPC routines* activity).
- **Swimming and ladders** (moved out of 0.5.0 and 0.8.0; design notes kept, branch
  `feature/swim-climb`): water the player can swim across and ladder tiles to climb.
  **Goal.** Water the player can swim across (slowly, no attacks) and ladder tiles the player
  climbs (vertical only, slower, climbing pose), so maps gain height and water as terrain
  instead of walls.

  **Design.**
  - `TilesetDef.water: Vec<u32>` and `TilesetDef.ladder: Vec<u32>` (`#[serde(default)]`, like
    `solid`); the Tilesets form's grid gets two more pick modes (today: solid, animation, light).
    `Loaded` gets `water` / `ladder` grids next to `solid` (`Assets::compute_solid` pattern). A
    water tile marked swimmable is **not** solid for the player only: `rect_blocked(.., swimmer:
    bool)`; NPCs, pathfinding, projectiles and sight keep treating it as today (solid if listed in
    `solid`). Creators mark a tile either solid (a wall of water) or water (swimmable).
  - Swimming: when the player's feet are on water: speed × 0.5, no running, no attacking (toast
    once), no interacting with props, no fishing, no sitting; the paperdoll is drawn with the
    lower 40 % of the frame clipped (`draw_frame_part_in`, as the seated pose clips) and a 2 px
    bob, a blue tint; a splash `effect` on entering; `status()` `swimming=true`. Leaving water
    restores everything. Companions swim along (same rule for them).
  - Ladders: on a ladder tile only Up / Down move (× 0.6 speed), the player faces Up (climbing
    pose = the Up row with the walk cycle), interact is allowed (so a ladder top can be a portal
    zone, which is how "climbing down into the mine" works). `status()` `climbing=true`.
  - Save games don't change (position is enough). Old projects: no water / ladder tiles → nothing
    changes.

  **Touch points.** `defs.rs`, `render/mod.rs` (`TilesetTex` flags), `runtime/{mod,paperdoll,
  companions,minigame,furniture}.rs`, `editor/src/database.rs` (tileset grid modes) and
  `canvas.rs` (overlay colours), gen_example (mark the village pond and the Wilds' lakes
  swimmable, keep the forest stream solid; a ladder tile in the dungeon tileset and a ladder in
  the mine leading to a lower shaft via portals), README.

  **Example.** The village pond becomes swimmable (its fishing spot covers the water: fishing is
  from the shore, not while swimming); the mine gets a ladder down to a small lower shaft with the
  pick-only chest moved there.

  **Verify.** Autoplay: walk into the pond, `status` `swimming=true` and the tile is water, speed
  halved (tiles per second from two statuses), `press x` doesn't attack, screenshot of the
  half-submerged hero; the forest stream still blocks; in the mine `hold up` on the ladder
  climbs (`climbing=true`), `hold left` doesn't move; an NPC never enters water (`ai=` positions).

### Postponed: Windows and macOS (2026-11-08)

Taken out of 0.9.0 until hardware to develop and test on is there. *Windows export* and *Editor
on Windows* had been built and merged into develop (#65, #66, plus #67: a software OpenGL
fallback after a driverless Windows VM failed to start) and were **rolled back** (revert of
318b9d4, 1b2d37a, 9e91848); `git revert` of those reverts brings the code back as a starting
point. What that attempt found, for whoever picks it up:
- Cross-compiling with MinGW-w64 (`x86_64-pc-windows-gnu`) from Linux works for game and editor
  (no C dependencies); a `windows-latest` job (MSVC) passed clippy and tests.
- gilrs panics at every quit on Windows (its `Drop` joins a thread that is gone at process
  exit): never drop the `Gilrs` (`ManuallyDrop`).
- Windows without a GPU driver (VMs) only has OpenGL 1.1: miniquad panics with
  "WGL_ARB_pixel_format is required". Mesa's llvmpipe (mesa-dist-win) as `mesa\opengl32.dll`,
  pre-loaded before miniquad loads `opengl32.dll`, fixed it (+23 MB per package).
- RON's pretty printer writes CRLF on Windows (set `new_line("\n")`); zip entries need
  `System::Unix` or a Linux game zipped on Windows loses its executable bit; `std::fs::copy`
  returns 0 under Wine; one mouse-wheel notch is 120 on Windows (1 on Linux); GUI-subsystem
  binaries still write to a redirected stdout (`--engine-build` works), `AttachConsole` for
  terminals; the engine fingerprint must hash `/` paths; add `.gitattributes` (LF).
- Under Wine on this machine: Mesa software GL env (`__EGL_VENDOR_LIBRARY_FILENAMES=…50_mesa.json
  LIBGL_ALWAYS_SOFTWARE=1`, without `WAYLAND_DISPLAY`).

#### Windows export — `feature/windows`

**Design.** Gate the `std::os::unix` uses (`editor/files.rs` copy permissions,
`packaging/lib.rs` symlink and executable bit, `release.rs` chmod); Windows paths for saves and
settings (`release::user_saves_dir`, `Settings::dir`: `%APPDATA%`); `.exe` handling in
`export::game_binary` and `release::export` (`<slug>-windows.zip`, README text per platform).
CI builds `rustpg2d-game.exe` with `x86_64-pc-windows-gnu` in the Packages and Release jobs and
the toolkit packages ship it next to the Linux runner, so the Linux editor exports Windows games
(*File → Export game* gains a *Windows* checkbox). Followed by *Editor on Windows* and *macOS
builds* below.

**Verify.** Export from the editor produces the zip; run the exe under Wine for a `status` run
if Wine is available, otherwise a CI smoke test on `windows-latest`.

#### Editor on Windows — `feature/editor-windows` (follows *Windows export*)

**Goal.** The editor itself runs on Windows, with play-test, export and the asset library, so
Windows creators get the whole toolkit; the release ships a Windows zip of editor + game +
example.

**Design.**
- Gate the Unix-only calls: `editor/files.rs` `copy_tree` permissions, `packaging/lib.rs`
  `symlink` (Flatpak) and `set_executable` (AppImage) behind `#[cfg(unix)]` with Windows
  returning "not available on this platform" errors; the export dialog hides AppImage / Flatpak
  there. Paths: a `dirs`-style helper in `engine/src/paths.rs` (new, no crate): config
  `%APPDATA%\rustpg2d`, data `%LOCALAPPDATA%\rustpg2d`, cache `%LOCALAPPDATA%\rustpg2d\cache`, home
  `%USERPROFILE%`; used by `Settings::dir`, `release::user_saves_dir`, `library::library_dir`,
  `files::config_dir`, `example_copy`, the file dialog start folder, `new_project.rs`.
- Executables: `std::env::consts::EXE_SUFFIX` in `play_with`, `export::game_binary`,
  `library_window.rs`, `release::export` (`<slug>.exe`, `<slug>-windows.zip`, README per
  platform); `bundled_example` accepts the zip layout (`example_project/` next to the exes).
  `#![windows_subsystem = "windows"]` on both binaries, so the `--engine-build` probe writes
  its answer to a temp file instead of stdout (or the editor reads `ENGINE_BUILD` from the
  game's resources: simplest = a `--engine-build FILE` argument).
- Input: mouse-wheel units per platform in `editor/main.rs` (Windows reports 120 per notch:
  scale by 1/120; macOS pixels), `modifiers.command` instead of `ctrl` in `canvas.rs`,
  `pixel.rs`, `database.rs` (Ctrl stays Ctrl on Windows, Cmd on macOS). Window icon via
  `Conf::icon` for both binaries (the emblem from `packaging::emblem`).
- Repo hygiene: `.gitattributes` (`* text=auto eol=lf`) so `include_str!(LICENSE)` and the
  `ENGINE_BUILD` hash (which hashes path strings; make it use `/` always) are stable; tests that
  assume ELF (`packaging` `glibc_scan`, `release` `export_bundle` exe name) get `cfg` gates or
  platform branches.
- CI: a `windows-latest` job (clippy + test `--locked`; MSVC compiles the `vorbis_rs` C code for
  gen_example) on PRs into master; the Packages and Release jobs add a Windows build
  (`cargo build --release` on `windows-latest`, `cargo xtask dist --windows` zips editor, game,
  example, LICENSE, README) and upload `rustpg2d-<version>-windows-x86_64.zip`.
- Known gaps noted in the README: no code signing (SmartScreen warning), gamepads through
  Windows.Gaming.Input (gilrs default), file dialogs native.

**Touch points.** `engine/src/{paths (new),settings,release,library,build.rs}`, `editor/src/
{files,app,export,library_window,main,canvas,pixel,database}.rs`, `packaging/src/lib.rs`,
`game/src/main.rs`, `xtask/src/dist.rs`, `.github/workflows/*`, `.gitattributes`, README.

**Verify.** CI green on `windows-latest`; the zip from CI runs on a Windows machine or VM: the
editor opens the bundled example, play-tests it, exports a Windows game; `cargo test` on
Windows. From Linux: Wine run of the exes for a `status` smoke test if Wine is installed.

#### macOS builds — `feature/macos`

**Goal.** Game and editor build and run on macOS (Apple Silicon and Intel); the release ships a
`.app` bundle for the editor (game runner and example inside) and games can be exported for
macOS from any platform's editor.

**Design.**
- Paths: `~/Library/Application Support/rustpg2d` (config and data), `~/Library/Caches/rustpg2d`
  in `paths.rs`. Bundle layout: `R(ust)PG2D.app/Contents/{MacOS/rustpg2d-editor,
  MacOS/rustpg2d-game, Resources/example_project, Info.plist, R(ust)PG2D.icns}`; `bundled_example`
  and `bundled_root` look in `../Resources`. Exported games: `<slug>.app` with the data in
  `Contents/Resources` (`release::bundled_root` checks there first on macOS) zipped as
  `<slug>-macos.zip`.
- Input: Cmd for shortcuts (`modifiers.command`), trackpad scrolling (pixel deltas, momentum:
  the canvas zoom accumulates deltas instead of zooming per event), `rfd` on the main thread
  (`run_on_main` needs NSApplication; the blocking dialog on a worker thread must become
  `AsyncFileDialog` polled from the frame loop, which also helps Windows).
- Rendering: miniquad's macOS backend (Metal or OpenGL per feature); keep OpenGL (default) and
  note the deprecation; high-DPI already on.
- CI: a `macos-latest` job (clippy + test) on PRs into master; Packages / Release build universal
  binaries (`aarch64-apple-darwin` + `x86_64-apple-darwin`, `lipo`), assemble the `.app` with
  `cargo xtask dist --macos` (plist generated, icon from the emblem via `iconutil` on the
  runner), zip it; unsigned (Gatekeeper: right-click › Open; documented). Notarization is out of
  scope.
- Gamepads: gilrs uses IOKit on macOS; no extra permissions.

**Touch points.** `engine/src/{paths,release}.rs`, `editor/src/{files,app,export,canvas,
main}.rs`, `xtask/src/dist.rs`, `.github/workflows/*`, README.

**Verify.** CI green on `macos-latest`; the `.app` from CI starts on a Mac (user's or a CI
smoke test with `RUSTPG2D_AUTOPLAY` under the runner's window server), opens the example and
exports a game; `cargo test` on macOS.

## Shipped

See [CHANGELOG.md](CHANGELOG.md) for the details of every release.

### 0.8.0 — Scenery and sound (2026-11-08)

- [x] 47-tile corner auto-tiling (inner corners; Tiled Wang sets) — `feature/autotile-47`
- [x] Parallax layers (sky / abyss behind, cloud shadows over the map) — `feature/parallax`
- [x] Gapless music loops — `feature/gapless-music`

### 0.7.0 — Trades and town life (2026-11-08)

- [x] Mining and gathering (tools in recipes, days, regrowing props; the Workshop map) — `feature/mining`
- [x] Blacksmithing — `feature/blacksmithing`
- [x] Farming — `feature/farming`
- [x] Cooking — `feature/cooking`
- [x] Schedules across maps and with furniture — `feature/schedule-maps`
- [x] NPC routines: visible activities — `feature/activities`

### 0.6.0 — The party (2026-11-06)

- [x] Healing companions — `feature/heal-companions`
- [x] Knocked-down companions — `feature/downed-companions`
- [x] Commands for companions — `feature/companion-orders`
- [x] Companion inventory (characters use the items they carry) — `feature/companion-inventory`
- [x] Portrait images — `feature/portrait-images`

### 0.5.0 — Living world and adventure (2026-11-06)

- [x] Weather — `feature/weather`
- [x] NPC schedules — `feature/schedules`
- [x] Companions — `feature/companions`
- [x] Crafting — `feature/crafting`
- [x] Achievements — `feature/achievements`
- [x] Minigames: lockpicking and fishing (with the Tool slot) — `feature/minigames`
- [x] The player uses furniture — `feature/furniture`
- [x] Dialog portraits — `feature/portraits`
- [x] Renamed from rust2d to R(ust)PG2D (`rustpg2d`) — `feature/rename-rustpg2d`

### 0.4.0 — The workbench (2026-11-06)

- [x] Region copy, cut and paste — `feature/region-clipboard`
- [x] Script hot reload and in-game console — `feature/live-scripts`
- [x] Auto-tiling terrains (all starter tilesets, all example maps) — `feature/autotile`
- [x] Event / dialog graph editor — `feature/event-graph`
- [x] Imports: Tiled, LPC, Aseprite — `feature/imports`

### 0.3.0 — Heroes, foes and feel (2026-11-06)

- [x] Experience and levels — `feature/progression`
- [x] Status effects — `feature/status-effects`
- [x] Enemy AI — `feature/enemy-ai`
- [x] Bestiary — `feature/bestiary`
- [x] Dialog log — `feature/dialog-log`
- [x] Input actions and gamepad — `feature/gamepad`
- [x] Key rebinding — `feature/rebinding`
- [x] Screen shake and damage flash — `feature/juice`
- [x] Save thumbnails — `feature/save-thumbnails`

### 0.0.1 to 0.2.0

Editor, combat, quests, shops, skills and magic, lighting, cut-scenes, packaging, GitHub
workflow (before this roadmap existed).
