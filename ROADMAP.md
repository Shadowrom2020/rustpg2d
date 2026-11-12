# R(ust)PG2D roadmap

Planned features of R(ust)PG2D, grouped into themed releases, with the design decisions and the
code hook points already worked out so that a session can start an item without re-exploring the
codebase. What is shipped is in [CHANGELOG.md](CHANGELOG.md); the rules for branches, versions
and releases are in [AGENTS.md](AGENTS.md); coding and verification rules in [CLAUDE.md](CLAUDE.md).

Status marks: `[ ]` open · `[~] feature/<branch>` in progress · `[x]` done, waiting for the release.
Shipped versions are not listed here: CHANGELOG.md is their record.

## How to work from this roadmap

Instructions for every session (people and coding agents) that implements roadmap items:

1. **Pick the item.** Unless the user names one, take the first open item of the lowest
   unreleased version, in the order listed (later items build on earlier ones). Read its whole
   block: *Goal*, *Design* (defaults are decided; keep them unless the user says otherwise),
   *Engine* (none, or the agreed engine addition), *Touch points*, *Example*, *Verify*. Line numbers are not given on purpose: find the named
   functions and structs with grep, they move.
2. **Content or engine?** Plan the item as content first (CLAUDE.md, *Content route*): events,
   scripts, definitions, structures and assets in the starter library and the example project.
   When an engine addition is unavoidable, propose the smallest generic one to the user and wait
   for the go-ahead before changing Rust code. Record the outcome in the item's *Engine* line:
   "none (content)" or the agreed addition and why.
3. **Branch.** `git checkout master && git pull --ff-only && git checkout -b feature/<item-slug>`
   (the slug is given in the item's heading). One item per branch; a release's items are
   separate pull requests into `master`.
4. **Mark it.** Change the item's mark to `[~] feature/<slug>` in the first commit of the branch
   and to `[x]` (version left empty until the release) in the last one. Keep the *Design* block
   true: if the implementation deviates, edit the block so the roadmap matches the code.
5. **Compatibility is a hard rule** (see CLAUDE.md): new `Action`, `Cond`, `TriggerOn`,
   `ZoneKind`, `OnUnlock` variants are appended; new fields get `#[serde(default)]`; new map
   object fields go into the RON `Meta` of `map/io.rs`; save games (`WorldState`) and
   definitions of every released version must still load. Add a test that parses an old RON
   without the new fields when you add fields to `WorldState` or a definition.
6. **A feature is done when** it works in the running binaries, its assets are in the starter
   library (and the example project when it shows it), is covered by the scripted checks in
   *Verify* (autoplay / editor script + screenshots, on a scratch copy of the example project),
   has tests where engine logic changed, is documented
   (the manual in `Documentation/manual/`, its `08-scripting.md` for every script function,
   the editor's `API` table, CHANGELOG *Unreleased*), and `cargo build / clippy -D warnings / test --workspace`
   pass. The example project (`example_project/`, edited with the editor or its AI agent
   server) shows the feature: add new entities instead of recreating existing ones, so uids
   stay stable.
7. **Commit, push and open pull requests only when the user asks.** When all items of a
   version are `[x]` (or the user decides to ship early), cut `release/X.Y.Z` as described in
   AGENTS.md and remove the version's section from this file (its design notes live on in the
   CHANGELOG and the code).
8. **Changing the roadmap** is fine: reorder, move items between versions, split or merge
   them, add new ideas to *Backlog*. Keep every item's block in the same shape. Don't delete
   design notes of open items; replace them.
9. **Record what you decided.** When the implementation deviates from the *Design* block, add
   a *Decided while building* paragraph to the item (before *Touch points*) instead of
   rewriting the design; later items and the release notes rely on it. The *Verify* block
   lists what was actually checked; say in the report what was compile-checked only (e.g.
   editor fields below the visible area, which the editor script cannot scroll to).
10. **Scripted checks that hold up.** Park the autoplay mouse before navigating menus
   (`drag 2 2 2 2` as the first command), or the real pointer over the test window changes
   the selection. Start from a crafted `saves/quick.ron` (`press f9`) to skip arrival
   cut-scenes; a pending dialog blocks autosaves and queued actions (`press e` twice). Add a
   `status()` field for every new state (`attacking=`, `shake=`, `pad=`, …) rather than
   reading screenshots for it; use screenshots for looks. Never delete with a relative path
   after `cd`; use a fresh scratch folder instead.
11. **Dependencies and packages.** A new crate that links a system library (gilrs → libudev)
    means: the CI apt lines (`ci.yml`, `release.yml`), the README's build prerequisites, a
    check that the release game still needs glibc ≤ 2.34 (`objdump -T target/release/
    rustpg2d-game | grep GLIBC_`), the Debian 12 container (`ldd`), and the Flatpak permissions
    (`--device`, `--socket`) in `editor/src/package.rs` and `xtask/src/dist.rs`.
12. **Example content.** `example_project/` and the starter library
    (`crates/editor/assets/starter_library/`) are plain files: edit them with the editor or its AI
    agent server (CLAUDE.md, *Example content*), send new or changed example assets to the
    starter library (`library_send` with `replace`, plus `drop_script_triggers` for assets whose
    triggers run the example's story scripts; the library set to that folder) and commit both together; existing entities keep their uids; give new example
    characters their own ids; never touch `example_project/saves/`.
13. **Branches that touch the same files.** Start the second item's branch from the first one
    (as *Save thumbnails* did from *Screen shake*, both in `menu.rs`), open the pull requests
    in that order, and say so in the pull request text. One pull request per item.
14. **Documentation per item**, all before the report: the manual chapter of the area
    (`Documentation/manual/`; controls in `15-players-guide.md`, events in `07-events.md`),
    a README highlight for notable features, `08-scripting.md` for every script function (a test
    fails otherwise), the editor's `API` table, CHANGELOG under the matching `###` heading of
    *Unreleased* (Gameplay, Editor, Fixes, Documentation, Maintenance), the roadmap mark.
15. **CI hiccups.** A job that fails within a minute with a download timeout
    (`static.rust-lang.org`) is the runner's network: `gh run rerun <id> --failed`.
16. **Cutting a release.** `release/X.Y.Z` from master: version in `Cargo.toml` (the build
    updates `Cargo.lock`), `## Unreleased` → `## X.Y.Z — date`, README *Current version*,
    the version's roadmap section removed from this file (the CHANGELOG is its record),
    `starter_history.ron` current (`cargo run -p rustpg2d-editor --example starter_history`; the
    tests fail otherwise), `cargo xtask check-version release/X.Y.Z`, pull request into master, merge after the
    checks: the Release workflow tags and publishes it.

## 0.19.0 — Lighter characters

Since 0.18 every character is a 3D model rendered into pictures. This release makes that cheaper
and nicer: the pose atlas keeps what is in use, models load when needed, characters turn
diagonally, and games without a portrait get their player's face as icon; in the editor, big
maps stay smooth when zoomed out, and the forms get click tests. The items follow the
review of *Caveats and performance characteristics* in `Documentation/architecture.md`; each one
updates the caveat lines it changes. Build order as listed (eight facings doubles the walk poses,
so it comes after the atlas).

- [x] Caveats section current — `feature/caveats-doc`

  **Goal.** The caveats in `Documentation/architecture.md` describe the engine as it is, and this
  section is on master.

  **Design.** Docs only: section 7 rewritten (3D models: loading at start, the pose atlas, the
  render budget, eviction by fill time, recoloured textures; the tile overview above 60,000
  visible tiles; `tr_ui` per draw). This 0.19.0 section added from the backlog ideas and the review (code signing stays in *Backlog*).

  **Engine.** None (documentation).

  **Verify.** Read through; every claim checked against the code (`render/mod.rs` `draw_layer`,
  `runtime/text.rs`, `render/model.rs` `alloc` / `pose`, `path.rs`, `patrol.rs`, `sys.rs`).

- [ ] Pose atlas by use — `feature/pose-atlas`

  **Goal.** Poses drawn every frame stay cached in crowded scenes, and a project can set how much
  GPU memory the atlas may take (small GPUs, big casts).

  **Design.** A cache hit in `Models::pose` stamps the cell's page with the current frame, so
  `alloc` reuses the least recently *drawn* page instead of the one filled longest ago; the unused
  `Cell.used` goes. Evictions are counted: `status` `models=` gains `/e<evictions>`.
  `ProjectConfig.model_atlas_mb` (serde default 32 = 8 pages of 1024 x 1024, clamped 8 to 128),
  set in *Database › Project* next to the dialog portraits; `Models` takes its page cap from it
  (`MAX_PAGES` becomes a field).

  **Engine.** Agreed with the plan for 0.19.0: the field, the eviction order, the counter.

  **Measured** (0.18.2 release build, example project, village at noon, walking loops): the atlas
  holds 170 poses on 2 pages after the save loads, 448 on 5 pages after 30 s, and fills all 8
  pages (810 poses) after about 4 minutes; eviction starts after that. Frames stay at 16.7 ms
  (vsync) with the worst under 24 ms and drawing at 0.2 to 0.7 ms, so a render costs little:
  this item is about keeping the right poses in long sessions and big casts, not frame time
  today.

  **Touch points.** `render/model.rs` (`Page.stamp`, `alloc`, `pose`, `stats`, `MAX_PAGES`),
  `render/mod.rs` (the `Models::load` call), `project.rs`, editor `database.rs` (project form),
  `mcp/tools.rs` (project settings description), the manual's project settings, architecture
  caveats.

  **Verify.** A unit test of the page choice (a pure `pick_page` factored out of `alloc`; no GL in
  tests). Autoplay on a scratch copy with `model_atlas_mb: 8` in a crowd (village square,
  `RUSTPG2D_MODEL_DEBUG=1`): evictions counted while the player's poses stay cached (renders
  steady while walking back and forth); shots.

- [ ] Models load when first drawn — `feature/lazy-models`

  **Goal.** Games start faster and use less memory: a model is read when a character needs it,
  not every model of the project at start (the example has 34 MB of GLB).

  **Design.** Measure first: launch to the first `status` (release build, example project, native
  and web). If loading all models costs under about 0.3 s natively, record the numbers here and
  close the item as documentation only. Otherwise `Models::load` keeps the definitions and the
  file reader, and a model is parsed on its first `pose` request; on map enter the player's model
  and those of the map's characters load at once, so nobody appears late while walking into
  view. Load errors still go to `Models::errors`. The editor's previews and the library window
  go through the same `pose` and keep working.

  **Engine.** Agreed with the plan for 0.19.0, subject to the measurement.

  **Measured** (0.18.2 release build, the example against a copy without models): native start
  to the first `status` 0.71 s against 0.49 s (models: 0.22 s), peak memory 281 against 243 MB;
  web (local server, headless Brave with SwiftShader) 1.8 s against 1.3 s. The web export is
  45.7 MB against 16.8 MB (`data.pack` 39 MB, uncompressed): over a real connection the
  download, not the parsing, is what players wait for. Textures are only 2 MB of the 33 MB of
  GLB; meshes and clips are the rest (gzip makes them about 38 % smaller).

  **Touch points.** `render/model.rs` (`loaded` loaded on demand), `render/mod.rs`, the runtime's
  map enter, editor `library_ui.rs` (`models_root`, `begin_frame`), architecture caveats.

  **Verify.** Start-up time before and after (native and web); `status` `models=` on the title and
  after map enter; a map change to a map with new characters, shot right after arrival (nobody
  missing); the character editor and the library window still show models.

- [ ] Eight facings when drawn — `feature/eight-facings`

  **Goal.** Characters walking diagonally are drawn turned diagonally (3D models can be drawn at
  any angle).

  **Design.** `entity::Facing` stays four-way: interaction, maps, save games and the editor's
  facing pickers are unchanged, so no file format changes. The renderer gets a direction of its
  own: `PoseReq.facing` becomes `dir` (0 to 7) and `facing_yaw` covers the eight; `paperdoll.rs`
  takes the diagonal from the walker's movement while it moves, from `Facing` otherwise.
  `ProjectConfig.eight_facings` (serde default on) turns it off. `status` `anims=` shows the
  direction.

  **Engine.** Agreed with the plan for 0.19.0.

  **Touch points.** `render/model.rs` (`PoseReq`, `PoseKey`, `facing_yaw`), `runtime/paperdoll.rs`
  (`model_cell`, the player's and NPCs' poses), `runtime/creator.rs` (preview), the editor's
  previews that build a `PoseReq`, the project form.

  **Verify.** Autoplay with `keydown right` + `keydown down` (and the other three diagonals): shots
  of the player and a patrolling character turned diagonally; the atlas stays within its budget;
  with the option off, four facings.

- [ ] Game icon from the player's model — `feature/model-icon`

  **Goal.** A game without a player portrait gets its player's head and shoulders as window and
  package icon instead of the R(ust)PG2D emblem.

  **Design.** `icon::hero_icon` runs without a GPU (packaging, `xtask`), so the editor renders the
  bust when it exports: a 256 px bust of the project player's model (`Models::pose` with `bust`),
  written as `icon.png` into the exported data. `hero_icon` takes the player portrait, else the
  project's `icon.png`, else the emblem; the game window, the AppImage, Flatpak, Windows and macOS
  packages and `cargo xtask-dist` all go through it. The export dialog shows the icon it will use.

  **Engine.** Agreed with the plan for 0.19.0.

  **Touch points.** `engine/src/icon.rs`, editor `export.rs` and `package.rs`, `release.rs` (the
  exported files), the manual's export chapter.

  **Verify.** `exportgame` and `exportpkg … appimage` on a scratch copy without a player portrait;
  the icon taken out of the AppImage (its `.DirIcon`) and looked at; the game window's icon in a
  native run.

- [ ] Editor: cached tile chunks when zoomed out — `feature/tile-chunks`

  **Goal.** The editor stays smooth zoomed out on big maps. Tiles are drawn one quad each, every
  frame (`render/mod.rs` `draw_layer`): in the game that is a few thousand quads, but the editor
  zoomed out draws up to 60,000 tiles per layer before it switches to the overview.

  **Design.** Measure first (release build): `canvas.rs` frame time with the village, and with a
  256 x 256 test map filled on four layers, zoomed out just below `OVERVIEW_THRESHOLD`. Under
  16 ms a frame on the big map: record the numbers here and close the item as documentation
  only. Otherwise each 32 x 32 chunk (`map::CHUNK_SIZE`) of a layer is rendered into a texture
  once, at the tile size, and drawn as one quad while the view is zoomed out further than 1:1
  (closer in, per-tile drawing stays: sharp and no memory). Edits mark their chunks dirty
  (`doc.rs`, every tile change and undo / redo, terrain retiling, structure placing, paste),
  as do tileset changes; animated tiles are left out of the cache and drawn per quad on top.
  Chunk textures are dropped when the map closes and capped (least recently drawn go first).
  Editor only: the game keeps `draw_layer`.

  **Engine.** Agreed for 0.19.0 (editor; a helper in `render/mod.rs` that draws one chunk of a
  layer, shared by the cache).

  **Touch points.** `editor/src/canvas.rs` (the layer loop), `editor/src/doc.rs` (dirty chunks),
  `render/mod.rs`, architecture caveats (tiles).

  **Verify.** Frame times before and after (editor `status`, the big test map and the village);
  screenshots zoomed out before and after an edit, with undo (the cache follows); an animated
  water tile still moves zoomed out.

- [ ] Editor UI tests with `egui_kittest` — `feature/editor-ui-tests`

  **Goal.** The editor's forms are tested by real clicks and typing, headless in CI, instead of
  only through script commands that call the same functions. This replaces the throwaway patched
  egui-macroquad that CLAUDE.md describes for real clicks.

  **Design.** `egui_kittest` (the release that matches egui 0.31, as a dev-dependency) drives an
  `egui::Context` without a window, finding widgets by label through AccessKit. The editor's
  forms take `&mut Editor`, whose `Assets` hold GPU textures, so a headless `Editor` comes first:
  `Editor::headless(project)` with assets that keep the images but create no textures (the
  sprite pickers and previews draw a placeholder). Then tests for the main forms on a scratch copy
  of the example: an item (rename, change a field, hold style), a character (model, a colour),
  project settings (a checkbox, a number), each checking the saved definition, plus undo. A test
  that needs a window is not added (CI has none outside the smoke run). If a headless `Editor`
  turns out to need deep changes, the fallback is a test example run under Xvfb in the *Runtime
  smoke* job; record which one was taken.

  **Engine.** Agreed for 0.19.0 (editor and the engine's `Assets`: a mode without textures).

  **Touch points.** `crates/editor/Cargo.toml` (dev-dependency), `editor/src/app.rs`
  (`Editor::headless`), `render/mod.rs` (`Assets` without textures), `editor/src/database.rs` and
  `widgets.rs` (texture use behind the placeholder), `crates/editor/tests/`, CLAUDE.md
  *Verification* (how to write such a test), architecture caveats (the UI test line), the
  manual's `17-testing.md` if creators see it.

  **Verify.** The new tests pass with `cargo test --workspace` locally and in CI (*Build, lint,
  test*); a deliberately broken form (a field not saved) makes one fail; clippy clean.

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented). Packaging work: needs the user's go-ahead.
