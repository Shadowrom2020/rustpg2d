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

## 0.8.0 — Scenery and sound

Theme: maps gain smoother terrain edges and depth, and music loops without a gap.

### [x] 47-tile corner auto-tiling — `feature/autotile-47`

**Goal.** Terrains with inner corners: the 16-tile sets of 0.4.0 only look at the four sides, so
a diagonal neighbour leaves a hard corner; 47-tile ("blob") sets look at all eight.

**Design.**
- `Terrain.blob: Vec<Option<u32>>` (serde default empty = 16-tile mode as today; 47 entries
  otherwise; a `Vec` because serde has no 47-element arrays). The mask uses eight neighbours,
  a diagonal counting only when both cardinals next to it are set, which leaves 47 distinct
  masks; a table maps the 256 raw masks to the 47 slots. `tile_for` falls back to the 16-tile
  set, then the inside, so a half-filled blob set still paints.
- `autotile::paint` / `retile` use the 8-neighbour mask for blob terrains (the cells to update
  stay the 3 × 3 around the change). `joins` and `base` work unchanged.
- Editor (*Database › Tilesets › Terrains*): a *Corners (47 tiles)* toggle; the slot grid becomes
  the common 7 × 7 blob template with a mask drawing in each empty slot; *Fill from template*
  assigns 47 slots from a block of the tileset in that standard order (how most blob tilesets
  are published).
- Imports: Tiled's *mixed* Wang sets with 47 tiles map onto it (`import/tiled.rs`, reported
  when it doesn't fit).

**Decided while building.** Slot order is ascending 8-bit mask value (bits N 1, E 2, S 4, W 8,
NE 16, SE 32, SW 64, NW 128), so slots 0–15 are the masks without corners in the 16-tile order and
slot 46 is the inside; there is no single published blob order, so the editor draws each slot's
mask (blue arms for corners) instead of claiming a standard. `tile_for` takes 8-bit masks for
every terrain (16-tile terrains ignore the corners); `paint` re-tiles a diagonal neighbour only
when its terrain is a blob terrain, so 16-tile maps change exactly as before. Tiled import read
no Wang sets before: it now reads them from `.tsx` / `.tsj` / embedded tilesets (comma Wang ids
only, Tiled ≥ 1.5). Only *Dirt path* and *Sand* got blob art (generator: the inner-corner notch is
where the two neighbours' bands meet, rounded); editor script `terrainblob TILESET NAME on|off`.

**Touch points.** `autotile.rs`, editor `terrain.rs`, `database.rs`, `import/tiled.rs`,
gen_example (art and terrain), README.

**Example.** The starter sand path gets a blob set, composed from quarter tiles in the generator
(inside, edge, outer corner, inner corner), and the village's paths are retiled with it.

**Verify (done).** Unit tests: the 256 → 47 table (47 distinct, closed under rotation), `paint`
on an L-shape gives the inner corner and re-tiles the diagonal neighbour, half-filled sets fall
back, old terrains without `blob` load and paint; Tiled fixture with a mixed and a corner Wang set.
Editor script: the Terrains form with the 7×7 grid and preview, `terrainpaint` of an L on the
village (screenshot: rounded inner corner); game screenshot of the village's road junctions.

### [x] Parallax layers — `feature/parallax`

**Goal.** Image layers that move at their own speed behind or over a map: a sky or abyss under
a cliff map, drifting cloud shadows or mist above it.

**Design.**
- `Map.parallax: Vec<ParallaxLayer { image: String /* project path */, factor: [f32; 2] /*
  0 = fixed to the screen, 1 = moves with the map */, offset: [f32; 2], drift: [f32; 2] /* px per
  second */, repeat: [bool; 2], above: bool, alpha: f32 }>` in the map's RON `Meta` (serde
  default empty).
- Drawing: `render/parallax.rs` (shared with the editor like `render/weather.rs`): layers with
  `above == false` after `clear_background` and before the tile layers, the others after the
  entities and before lighting / weather; source rect from the camera position × factor + drift ×
  time, tiled when repeating. Textures load with the map (`Assets`).
- Editor: *Map › Properties* gets a *Parallax layers* list (image picker, factor, drift, repeat,
  above, alpha, order); the canvas previews them (view toggle `viewopt parallax`).

**Decided while building.** A layer names a **sprite sheet** (`sheet`, drawn whole) instead of an
image path: sheets already load with the project, have pickers and travel with renames and the
library, so nothing new loads with the map. Added `scale` (cloud shadows are a small tileable
image drawn 4×). The image's origin is the view's top-left × (1 − factor) + offset + drift × time,
so factor 0 anchors it to the screen's top-left. Behind layers draw over the whole view (also past
the map's edge, in the editor too). The mine is pitch dark and has no empty cells, so its chasm
became a see-through *Abyss* terrain (appended to the dungeon tileset; Chasm unchanged) through
which the mist shows near the torch. Cloud shadows: factor 0.8, alpha 0.3. `status()`
`parallax=NbehindMabove`; editor `viewopt parallax`.

**Touch points.** `map/mod.rs`, `map/io.rs` (Meta), `render/{parallax (new),mod}.rs`,
`runtime/mod.rs` (draw order), editor `app.rs` (map properties), `canvas.rs`, `main.rs`
(`viewopt parallax`), `refactor.rs` / `library.rs` (image refs travel with maps), README.

**Example.** Cloud shadows drifting over the Wilds (above, alpha 0.15) and a dark mist behind
the mine, seen under empty tiles and beyond the map edge (behind, factor 0.3).

**Verify (done).** Autoplay: Wilds screenshots with drifting cloud shadows, mine screenshots at two
player positions (the mist in the chasm shifts against the floor), `parallax=1behind,0above` /
`0behind,1above`; editor screenshots of the mine with the preview and the map settings' list, and
with `viewopt parallax off`; map round trip test (and no `parallax` written for maps without
layers), origin / copies unit test.

### [x] Gapless music loops — `feature/gapless-music`

**Goal.** Music loops without the short silence at the loop point.

**Design.**
- Cause (found with the rain): quad-snd 0.2.8's mixer, after rewinding a looped sound inside a
  mix buffer, writes the rest of the buffer from index 0 again, so a loop not ending on a
  buffer boundary gets an overlap and a silent tail. The rain avoids it with a length of whole
  `MIX_BUFFER_FRAMES` (4096) at 44.1 kHz.
- Fix for music: sounds played with `play_music` are decoded by the engine (Ogg Vorbis with
  `lewton`, already compiled for audrey, as a direct dependency; WAV with a small reader),
  resampled to 44.1 kHz, and stretched to the next whole number of mix buffers (≤ 93 ms more:
  a pitch change below 0.1 % for tracks over a minute; tracks under 20 s are padded with a
  crossfade of their start instead), then handed to macroquad as WAV (`wav_bytes`). Done at load
  for sounds used as music by maps or *Play music* actions, lazily (once) for others.
- Should macroquad ship a fixed quad-snd, the workaround is skipped (keep the replay test of
  the mixer, `rain_loop_survives_quad_snds_mixer`, for music lengths).

**Decided while building.** No crossfade padding for short tracks: a track under 30 s is
repeated until it lasts 30 s, then stretched like the others (≤ 0.32 %, about 5 cents). Which
sounds are music isn't known without scanning every map, so files that are probably music (Ogg
over 64 KiB, WAV over 1 MiB) are prepared in a background thread at load, and any sound is
prepared synchronously the first time `play_music` gets it (`Audio::gapless`). A file that already
is whole buffers at 44.1 kHz (≤ 2 channels) is played as it is. Resampling is linear and wraps
around the loop point. `status()` `music_loop=whole|stretched|plain|-`. The example tracks get a
tempo a hair slower (`music.rs` `whole_buffers`) so they are whole buffers long.

**Touch points.** `audio.rs`, `engine/Cargo.toml` (`lewton`), README (audio notes).

**Example.** The two example tracks loop cleanly (they are generated: also render them at whole
buffer lengths in `gen_example/music.rs`).

**Verify (done).** Unit tests: stretched lengths are whole buffers (61.3 s at 44.1 kHz, 7 s at
22.05 kHz repeated to 35 s, mono WAV), the quad-snd mixer replay has a gap for a 41.7 s track and
none for its gapless loop, WAV / Ogg decoding, the example's Ogg tracks are whole buffers. Game run
with master volume 0 (real playback path): `music_loop=whole` for the example tracks,
`stretched` for a 35.3 s 22.05 kHz test WAV. Listening check: left to the user (unmuted run).

## 0.9.0 — Everywhere (platforms and reach)

Theme: R(ust)PG2D and the games made with it run where the players and creators are: Windows,
macOS, the web, and in their language. Postponed from 0.6.0 (2026-11-06) so the gameplay
releases come first; the design notes are unchanged.

### [ ] Windows export — `feature/windows`

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

### [ ] Editor on Windows — `feature/editor-windows` (follows *Windows export*)

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

### [ ] macOS builds — `feature/macos`

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

### [ ] Web export — `feature/web`

**Design.** Game only (`wasm32-unknown-unknown`): async asset loading (`audio.rs` `poll_now`
assumes instant loading), saves and settings in `localStorage`, no `std::env` / `current_exe`,
`SystemTime::now` → `web-time`, the exported game as an asset tree plus a generated
`index.html` for itch.io. Plan in detail only after Windows ships.

### [ ] Localization — `feature/localization`

**Design.** Content texts (dialogs, choices, item / prop / quest / skill names and descriptions)
through `lang/<code>.ron` lookups keyed by the original string with `expand_text` as the
chokepoint; the ~170 hard-coded UI strings in the runtime move to one table behind `t("…")`;
Options row *Language*; the editor exports a translation template. Large but mechanical.

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

## Shipped

See [CHANGELOG.md](CHANGELOG.md) for the details of every release.

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
