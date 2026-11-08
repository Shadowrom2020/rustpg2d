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

## 0.9.0 — Reach: the web, other languages, Windows and macOS

Theme: games made with R(ust)PG2D reach players without an install (a browser tab, itch.io),
in their own language, and on Windows and macOS — and creators get the editor on Windows and
macOS too. Windows and macOS had moved to the backlog on 2026-11-08 (the first attempt is
described under *Editor on Windows and macOS* below); they came back the same day, without a
software OpenGL fallback, once a Mac was there to test on. Order: web export first (it changes
how the game loads data, which localization then uses), then the Windows and macOS exports (one
pull request), then the editor on both (one pull request).

### [x] Web export — `feature/web`

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

**Decided while building.** The layer is `engine/src/sys.rs` (`read`, `write`, `exists`,
`files_in`, `remove_file`, `modified_secs`, `unix_now`), used by the game's code paths only
(the editor keeps `std::fs`). Instead of fetching each file with a `files.ron` manifest, the
export writes **one `data.pack`** (magic + postcard list of path / bytes; `release::web_pack`):
one request instead of ~90 round trips. No `web-time` / `js_sys`: R(ust)PG2D's own JS plugin
(`engine/web/rustpg2d.js`, registered with `miniquad_add_plugin`) provides `Date.now`,
`localStorage` (keys `rustpg2d:<page folder>:<path>`, values base64) and the page's query
string. Dependencies on wasm: Rhai with `default-features = false, features = ["std",
"no_time"]` (its default pulls getrandom / web-time, i.e. wasm-bindgen, which macroquad's loader
can't satisfy), gilrs and zip native-only (`library`, `migrate` and `release::export` are
`cfg(not(wasm32))`), and `.cargo/config.toml` passes `--allow-undefined` to the wasm linker
(current Rust no longer does; the loader provides miniquad's / quad-snd's imports). Sounds are
decoded asynchronously by the browser, so `audio::preload` awaits them before `Game::new`; the
gapless-loop stretching is skipped on the web (Web Audio loops seamlessly). No *Click to start*
screen: macroquad's audio JS resumes the audio context on the first click / key press by
itself. The title screen has no *Exit* on the web. Automated runs: `index.html?autoplay=…`,
output (and `shot` as a base64 PNG) in the browser console. After the first test: no browser
context menu on right-click (the page cancels `contextmenu`), and source rectangles of all tile
and sprite draws are inset by 1/64 texel (`render::inset`): at fractional scales nearest
sampling could pick the neighbouring tile's texel at an edge (thin green / brown lines; seen in
the browser at device scale 1.25 even with software GL, gone after the change). Not done: `wasm-opt` (not
installed here), gamepads (the browser Gamepad API), dev console / hot reload.

**Touch points.** `engine/src/{vfs (new),project,release,settings,achievements,audio}.rs`,
`map/io.rs`, `render/mod.rs` (`Assets::load`), `runtime/{mod,menu}.rs`, `game/src/main.rs`,
`editor/src/export.rs`, `packaging` / `xtask/src/dist.rs` (ship the wasm), `.github/actions/
packages`, README (*Releasing a game › Web*).

**Example.** The example project exports for the web unchanged; the README links how to try it
(`python3 -m http.server` in the export folder).

**Verify (done).** Unit tests: `export_web` (page, loader, pack contents without saves / unused
files, the chosen name, the zip's root), `data.pack` round trip, `sys` native files, base64.
Editor script `exportweb` (from the dev build and from the AppImage); the export served locally
and opened in **headless Brave** (SwiftShader WebGL) and **headless Firefox**: title screen;
new game, the intro's dialogs, autosave and quick save; a second visit with the same browser
profile: the save list with thumbnails and dates, *Continue* loads the quick save (village,
position, HUD). No audio decode errors in the console. CI: clippy for wasm32 on every PR,
Packages builds the wasm and checks the AppImage has it. Not verified: hearing the music in a
browser, Safari, itch.io itself, a phone.

### [x] Localization — `feature/localization`

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

**Decided while building.** **No new font**: macroquad's default font (ProggyClean) has all of
Latin-1 (ä ö ü ß é ç ñ …), so German and the other Western European languages draw as they are;
other scripts are out of scope. **Runtime texts are translated when drawn**, not at ~480 call
sites: the runtime imports `draw_text` / `measure_text` from `runtime/text.rs`, which look the
whole finished string up (`lang::tr_ui`); `ui` keys with `{}` match texts with values (`"Lv {}"`,
reordering with `{0}`), keys need ≥ 2 letters of their own, results are cached. Dialog lines,
speakers and choices (content, already translated) use macroquad's `draw_text` directly, so a
`ui` template can't touch them. **Content** is translated at `WorldState::expand_text` (before the
placeholders) and by translating the database's names / descriptions when the language is set
(`lang::translate_db`, the game keeps the original). The current language is global
(`lang::set`). `ProjectConfig.language` (default `en`) + `languages`; `Settings.language`;
`--lang`; Options row *Language* (always shown; cycles the offered languages); `status()` `lang=`.
The editor's section lives in *Database › Project › Languages* (no File menu entry); the template
is `lang::template` (keeps translations, reports unused keys, doesn't delete them). Exports carry
`lang/<code>.ron` (`release::check` warns about an offered language without a file). Glued
script strings can't be translated: the example now uses placeholders, `scripting.md` says so.
The example is translated completely (314 texts, `gen_example/lang_de.rs`), not just the village.

**Touch points.** `engine/src/{lang (new),state,project,settings}.rs`, `runtime/{ui_text (new),
menu,ui,inventory_ui,shop_ui,journal,…}.rs`, a font file under `crates/engine/assets/`,
`editor/src/{database,app,main}.rs`, `script.rs` (nothing new expected: texts pass through
`expand_text`), gen_example (a German `lang/de.ron` for the example, at least the village),
README, `Documentation/scripting.md` (how script texts are translated).

**Example.** The example project offers English and German; the village and the first quest are
translated completely.

**Verify (done).** Unit tests: content lookup with fallback (empty = missing), `ui` templates with
values and reordering, keys without own words ignored, the built-in German parses with matching
placeholders, the template on the example (texts of every source, kept translations, unused
keys, the German file complete), exports carry the language files. Autoplay with `--lang de`:
title, character creator, the intro dialog, inventory, journal, pause menu (screenshots, `lang=de`),
Mira's scripted line with `{player}`; *Options › Language* switches at runtime and is saved;
`LANG=de_DE` starts German. Editor: `langtemplate fr` (312 texts), the Languages section
(screenshot), a play-test with `playlang de`. Web: the export in headless Brave with a German
browser language starts German (title, intro dialog).

### [x] Windows export — `feature/windows-export`

**Goal.** *File › Export game* gains *Windows*: `<game>-windows/<game>.exe` with its data (and a
zip) for Windows 10 / 11 (64-bit), exported from the Linux editor.

**Design.** The game runner cross-compiled with MinGW-w64 (`x86_64-pc-windows-gnu`; no C
dependencies) by the Packages and Release jobs and shipped in the toolkit packages next to the
Linux runner; `release::Platform` (folder, exe, zip, README per platform);
`engine/src/paths.rs` for per-user folders (`%APPDATA%` saves and settings); the zip writes
Unix attributes whatever the host. The rolled-back attempt's findings (*Editor on Windows and macOS* below) apply.

**Decided while building.** Revived from the reverted #65 without the software OpenGL fallback
(Mesa, +23 MB): the game is a GUI-subsystem program (no console window; `AttachConsole` when
started from a terminal), and a panic hook shows a message box instead of a silent exit — an
OpenGL text when the failure comes from the GL loader (a VM without 3D acceleration), else the
error. Gamepads: the `Gilrs` is never dropped (`ManuallyDrop`), so quitting doesn't panic.

**Touch points.** `engine/src/{paths,release,settings,library}.rs`, `runtime/input.rs`,
`game/src/main.rs`, `editor/src/{export,files,main}.rs`, `packaging/src/lib.rs`,
`xtask/src/dist.rs`, `.github/{actions/packages,workflows/ci}.yml`, README.

**Verify.** Unit test (exe check, folder, zip, CRLF README). The exe cross-built in a
throwaway container ran under Wine (with Mesa from the host): title, new game, quick save in
`%APPDATA%\<game>\saves`, clean exit (code 0); without OpenGL it shows the message box.
Exports from the packaged editor (AppImage with all runners) start under Wine. Not verified on
real Windows (the user's VM of the earlier attempt had no 3D acceleration: it now gets the
message box; a VM with 3D acceleration or a real PC is needed to play).

### [x] macOS export — `feature/windows-export`

**Goal.** *File › Export game* gains *macOS*: an application `<Game Name>.app` for macOS 11+ on
Apple Silicon and Intel, exported from the Linux editor.

**Design.** `Platform::MacOS`: `<game>-macos/<Game Name>.app/Contents/{MacOS/<game>,
Resources/data, Resources/<game>.icns, Info.plist, PkgInfo}` plus README and license, zipped with
the executable bit; `release::bundled_root` also looks in `../Resources/data`; saves in
`~/Library/Application Support/<game>/saves` (`paths.rs`). The runner is a universal binary
(`lipo` of the aarch64 and x86_64 builds) that only a Mac can build: `.github/workflows/
macos-game.yml` (called by CI for pull requests into master and by the Release workflow) builds
it on `macos-latest` (plus clippy and the engine's tests on macOS) and hands it to the Packages
job as an artifact; `cargo xtask-dist --macos-game FILE --require-macos` ships it next to the
editor. The icon: the export dialog's *Icon* (or the player character) as `.icns` with 128,
256 and 512 px PNG entries.

**Decided while building.** The linker signs the arm64 slice of the runner ad hoc, but that
signature doesn't cover the bundle around it (`spctl`: "code has no resources but signature
indicates they must be present"), which macOS reports as a damaged app for downloads. So the
export **signs the app ad hoc itself** (`engine/src/macsign.rs`, no Mac needed): it writes
`_CodeSignature/CodeResources` (codesign's rules; SHA-1 `files`, SHA-256 `files2`) and gives
every slice a new code directory (pages, Info.plist, empty requirements, CodeResources;
`LC_CODE_SIGNATURE` added to the unsigned x86_64 slice). `hash.rs` holds SHA-256 (moved from
packaging), SHA-1 and Base64. Not notarized: players allow the first start once (*Privacy &
Security › Open Anyway*), as the bundle's README explains. Zip entries are dated with the export
time (all exports; they were dated 1980).

**Touch points.** `engine/src/{release,paths,macsign (new),hash (new)}.rs`,
`editor/src/{export,main}.rs`, `packaging/src/lib.rs`, `xtask/src/dist.rs`,
`.github/workflows/{macos-game (new),ci,release}.yml`, `.github/actions/packages/action.yml`,
README.

**Verify.** Unit tests: the bundle layout, Info.plist, icns, README, zip modes, replacing an
earlier export of a renamed game; the signer on synthetic thin / universal Mach-O files (page
hashes, special slots, the added load command, re-signing is stable) and the sealed resources.
On the test Mac (M2, macOS 27.2): the export from the Linux editor and from the packaged editor
passes `codesign --verify --deep --strict` for both slices; the app starts directly and via
`open` from a quarantined zip (translocated), plays a new game, quick-saves to
`~/Library/Application Support/<game>/saves` and *Continue* restores it; clippy and the 155
engine tests pass on macOS; the Mac's and Linux's runners report the same engine build. Not
verified: the x86_64 slice running (no Intel Mac, no Rosetta), the Gatekeeper dialog of a real
browser download (macOS let the test copies run), the CI job itself (runs on the release pull
request).

### [x] Editor on Windows and macOS — `feature/editor-desktop`

**Goal.** The whole toolkit on Windows 10 / 11 and macOS 11+ (Apple Silicon and Intel): the
editor with play-tests, exports for every platform and the asset library; the release ships a
Windows zip and a macOS app next to the AppImage and Flatpak.

**Design.** Revives *Editor on Windows* (#66, reverted by #68) and adds macOS:
- Per-user folders through `engine/src/paths.rs` (config, data, cache, home per system);
  play-tests start the host's `rustpg2d-game` next to the editor; the bundled example is found
  in the AppImage / Flatpak prefix, next to the exe (Windows zip) or in the app's
  `Contents/Resources` (macOS).
- Runner names per host (`Platform::game_runner`): the host's own is `rustpg2d-game`(`.exe`), the
  others ship as `rustpg2d-game-linux`, `rustpg2d-game.exe`, `rustpg2d-game-macos`; in the
  macOS app only Mach-O code sits in `Contents/MacOS` (editor, game runner), the foreign
  runners, the web game and the example in `Contents/Resources`.
- Windows: GUI subsystem for both binaries, `engine/src/desktop.rs` attaches to a parent console
  and shows a message box on a panic (an OpenGL text when the GL loader failed: no software
  fallback); window icons (`engine/src/icon.rs`); LF for RON, `.gitattributes`, `/` paths in the
  engine fingerprint.
- macOS: Cmd for the editor's shortcuts (`ui::command_down`; egui maps Cmd itself) and "Cmd+"
  in its texts (`ui::keys`, applied by `ui::t` and tooltips); file dialogs run modally on the
  main thread (AppKit refuses other threads); wheel / trackpad: the map zooms by
  `delta / WHEEL_NOTCH` steps (1 Linux, 120 Windows, 10 macOS; trackpads zoom smoothly), the
  pixel editor accumulates to whole steps, egui scrolls 2 points per trackpad point; `-psn_…`
  arguments from Finder are ignored.
- Downloads (`cargo xtask-dist`): `rustpg2d-X.Y.Z-windows-x86_64.zip` (editor, game, the Linux /
  macOS / web runners, example, README, license) and `macos/R(ust)PG2D.app` (Info.plist, icns,
  universal editor + game, foreign runners, example) as a tar. CI: *macOS* builds the universal
  binaries, *Packages* assembles everything on Linux, *macOS app* signs the app with
  `codesign --deep --sign -` (the nested game runner needs a real signer; exported games use
  `macsign.rs`) and zips it with `ditto`; the Release workflow publishes the four downloads; a
  *Windows* job runs clippy and the tests with MSVC; *Actions › CI › Run workflow* runs all of
  them on any branch.

**Decided while building.** No Mesa fallback (the user's choice): a driverless VM shows the
message box. The macOS app is signed on a Mac (CI) rather than by `macsign.rs`, because nested
code (the game runner) must be sealed with its own signature and requirement. Findings of the
first Windows attempt that still apply:
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

**Touch points.** `engine/src/{paths,desktop (new),icon (new),release,project,library}.rs`,
`engine/build.rs`, `editor/src/{main,app,files,export,ui,canvas,pixel,database,new_project}.rs`,
`game/src/main.rs`, `packaging/src/lib.rs`, `xtask/src/dist.rs`, `.github/{actions/packages,
workflows/{ci,release,macos-build,macos-app}}.yml`, `.gitattributes`, README, CLAUDE.md, AGENTS.md.

**Verify.** Clippy for Linux, the web, Windows (game + editor); the workspace tests; clippy and
the engine tests on the Mac. Windows (Wine, MinGW build): the editor opens the example, exports
all four platforms (the Linux zip keeps its executable bit and runs; the macOS app passes
`codesign --verify` on the Mac), play-tests; the Windows download unzipped: the welcome screen's
*Example project*, exports, play-test. macOS (M2, macOS 27.2, 1080p display): the app built
like CI (xtask on Linux, `codesign` on the Mac) verifies with the nested runner validated; the
editor from it shows the welcome screen with *Example project*, opens the copied example,
exports all four platforms (the exported app verifies), play-tests with the nested runner; a
file dialog opens as a modal panel of the editor (seen in the window list). Linux: the AppImage
exports all four and play-tests. Not verified: Retina displays (the test Mac has none), trackpad
and Cmd shortcuts by hand, real Windows hardware, the Gatekeeper / SmartScreen prompts of real
downloads, the CI jobs themselves (run on the release pull request or by hand).

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented).
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
