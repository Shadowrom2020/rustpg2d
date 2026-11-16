# R(ust)PG2D roadmap

Planned features of R(ust)PG2D, grouped into themed releases, with the design decisions and the
code hook points already worked out so that a session can start an item without re-exploring the
codebase. What is shipped is in [CHANGELOG.md](CHANGELOG.md); the rules for branches, versions
and releases are in [AGENTS.md](AGENTS.md); coding and verification rules in [CLAUDE.md](CLAUDE.md).

Status marks: `[ ]` open · `[~] feature/<branch>` in progress · `[x]` done, waiting for the release.
Shipped versions are not listed here: CHANGELOG.md is their record.

## Compatibility until 1.0.0

The user's policy (2026-11-16): until 1.0.0, compatibility is wanted but not absolute.

- **Stay compatible where it is reasonable.** The usual rules come first. Append new enum
  variants, give new fields serde defaults, put new map object fields in `Meta`, and add a
  fixture of the last release when a format changes. Most items below need nothing more.
- **Break compatibility only when it costs too much.** That means keeping it would take more
  work than the feature itself, would bend the feature's design (two ways of storing one thing,
  dead fields), or the feature can't be built compatibly at all.
  - The break is part of the item's engine proposal, so the user agrees to it before the work
    starts.
  - The item records the break in a *Decided while building* paragraph.
- **A break comes with a way across where possible.** The editor converts older projects on
  open, with a backup, as `migrate.rs` and `convert_tiles.rs` already do.
  - What can't be converted (usually save games) goes on a **Breaking:** line in the release's
    CHANGELOG section.
  - The manual says what creators have to do.
- **From 1.0.0 on, compatibility is a hard rule again** (see the *1.0.0* section):
  - everything made with any 1.x release loads in later 1.x releases;
  - every format change comes with a migration step;
  - script functions are deprecated rather than removed until 2.0.

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
5. **Compatibility** follows *Compatibility until 1.0.0* below. The default is still CLAUDE.md's
   rules: new `Action`, `Cond`, `TriggerOn`, `ZoneKind`, `OnUnlock` variants are appended; new
   fields get `#[serde(default)]`; new map object fields go into the RON `Meta` of `map/io.rs`;
   save games (`WorldState`) and definitions of every released version still load. Add a test
   that parses an old RON without the new fields when you add fields to `WorldState` or a
   definition. A break is only made when the policy allows it and the user agreed to it.
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

## The road to 1.0.0

Planned from the comparison with commercial RPG makers in `Comparison.md` (2026-11). The
order weighs how many creators each step unlocks against its size:

1. **Text and looks:** whether a game can be made for a market at all.
2. **Commands and plugins:** the event commands and plugins creators expect.
3. **The classic JRPG:** a party and turn-based battles.
4. **Reach:** platforms, editor comfort and a way in for RPG Maker users.
5. **Content:** one content release.
6. **1.0.0:** the formats freeze.

Each version has a theme, and items may move between versions.

Almost every item below changes the engine. The user agreed to the direction with this roadmap,
and each item's *Engine* line names the addition. When an item starts, confirm its details with
the user in a short plan before changing Rust code (CLAUDE.md, *Content first*). Build whatever
can be content as content: the library and example parts of each item.

## 0.24.0 — Every Script

Games can be written in any language:

- project fonts with full Unicode;
- text that wraps in languages without spaces;
- dialog codes for colour, icons and pacing;
- the engine's own texts in more languages.

- [x] Project fonts — `feature/project-fonts`

  **Goal.** A project chooses its fonts (TTF / OTF). Any script a font covers shows in dialogs,
  menus and the HUD, not only Latin-1.

  **Design.**
  - **Font roles:** `ProjectConfig.fonts` has the roles `dialog`, `ui`, `hud` and `title`. Each
    role is a font file under `fonts/` plus a pixel size. When the role's font lacks a glyph,
    each character falls back through `fallback`, a list of files (for example a CJK font).
  - **Default:** an empty role means macroquad's built-in ProggyClean, as today, so old projects
    look the same.
  - **Drawing:** `runtime/text.rs` becomes the only way the runtime draws text, including dialog
    lines, which call macroquad directly today. It picks the role's font, splits a string into
    runs by the first font that has each glyph, and draws the runs at whole pixels. Pixel fonts
    are drawn at whole multiples of their size, smooth fonts at any size.
  - **Loading and export:** fonts are loaded once with `load_ttf_font_from_bytes` and travel in
    exports and `data.pack`, web included.
  - **Editor:** a *Fonts* page in the project settings shows a preview line in each of the
    project's languages.
  - **Problems list:** warns about texts with characters no project font covers.

  **Engine.** The `fonts` field, font loading in `Assets`, run splitting in `runtime/text.rs`, the
  settings page and the problem check. A new library asset kind, `Font`. Compatible: an empty
  field means today's font.

  **Decided while building.**
  - **Fonts are definitions:** `defs/fonts.ron` with `FontDef` (file, `pixel` grid, `scale`,
    `offset_y`, `license`, `credit`) and `RefKind::Font`, so the library, packages, renames and
    the agent server handle them like other assets. `ProjectConfig.fonts` names font ids per role.
    *Database › Fonts* is the form (with a sample set in the font through egui); the roles are a
    *Fonts* section of *Database › Project*.
  - **Loading:** `text::install_project` at game start, not `Assets`. Coverage comes from the
    font's cmap through `ttf-parser` (already in the lockfile through fontdue). Roles are guards:
    `text::role(Role::Dialog)` at the top of `draw_dialog`, `draw_barks`, `draw_captions`; `Hud`
    in `draw_hud`, the quickbar, HUD buttons, the clock, toasts, damage numbers; `Title` for the
    game's name on the title screen.
  - **Content:** `draw_plain` / `measure_plain` draw text that is already translated. The direct
    macroquad calls (dialog box, key glyphs, `render::wrap_text`) use them; the developer console
    keeps the built-in font.
  - **Exports:** only the fonts the roles use, with their licence files.
  - **`write_asset`** writes `fonts/*.ttf|otf|txt`. The problems list names uncovered characters
    with their code points, because the editor's own font lacks most of them.
  - **The example:** monogram (extended, 577 glyphs, CC0, `fonts/monogram_credits.txt`) as the
    `ui` font of every role, 16 px grid, scale 1.3: body text and headings at 32 px, small hints
    at 16 px. The eight game screenshots that show text were retaken; the fishing one has none.
  - **Not changed:** without project fonts the game draws exactly as before. The exception is
    the text pieces of lines with key glyphs, which now sit on whole pixels like all other text.

  **Touch points.**
  - `project.rs`;
  - `runtime/text.rs`: every `macroquad::text::draw_text` / `measure_text` in `runtime/` goes
    through it (grep for them);
  - `runtime/ui.rs` (dialogs);
  - `render/` (Assets);
  - `release.rs` / `pack.rs` (fonts exported);
  - `library.rs` (asset kind);
  - the editor's project settings (`app.rs`) and `problems.rs`;
  - manual 02 and 12.

  **Example.** The example keeps its pixel look: a pixel font that covers Latin, Greek and
  Cyrillic as its `ui` / `dialog` font, also in the starter library. Check that the licence is
  the OFL or similar.

  **Verify.**
  - Autoplay a scratch translation with ä, ж, ω and 日本 (plus a CJK fallback font); take
    screenshots of a dialog, the pause menu and the HUD.
  - `status` prints `fonts=role:file…` and `missing_glyphs=N`.
  - An old project draws exactly as before: compare screenshots against master.

  Checked:
  - **Fusion Pixel on a scratch copy:** dialog, menu, inventory, journal and title, and a
    Japanese scratch translation mixing Japanese and Latin letters in one line;
    `missing_glyphs=0`.
  - **monogram in the example:** dialog, menu, options, inventory, skills, spells, journal, map,
    HUD at night, shop and crafting.
  - **Without fonts, against a build of master:** the dialog box is pixel-identical; the menu,
    inventory and journal differ only in the background and in the text of key-glyph lines.
  - **Exports:** a Linux export reads its font from `data.pack`; the web export too (headless
    Brave); the Windows runner under Wine and a macOS build on the test Mac draw the example's dialog in
    monogram.
  - **Smoke test:** `scripts/ci/smoke.sh` passes.
  - **Editor:** the Fonts tab and its sample, the project roles, `import font`, and the glyph
    warning per language.
  - **Tests:** unit tests (sizes, roles, coverage, run splitting, renames) and the editor UI test
    `a_font_is_defined_and_chosen`.

- [x] Starter library download — `feature/starter-pack`

  **Goal.** The starter asset library leaves the editor binary (the user's decision when the CJK
  fonts came: they add 20 MB). It becomes one file per release, which only compatible editors
  load.

  **Design.**
  - **The file:** `rustpg2d-starter-library-X.Y.Z.r2dlib`, a zip (`engine/src/starter_pack.rs`):
    - `starter.ron`: the version and `FORMAT`;
    - `history.ron`: the starter history;
    - `library/…`: the files of `crates/editor/assets/starter_library`.
  - **Building it:** `xtask dist` writes it (CI's *Packages* job), and `xtask starter-pack DIR`
    writes only it. The release publishes it and lists it in `SHA256SUMS`.
  - **Finding it** (`crates/editor/src/starter.rs`), in this order:
    1. `RUSTPG2D_STARTER_PACK=<file>` (`installed` = only the installed one);
    2. the repository folder in repository mode;
    3. the installed pack in `starter/` beside the asset library;
    4. a development build's source folder.

    `starter::open_library` replaces `open_library(dir, starter_for(dir))`.
  - **Installing it** (*Assets › Starter asset library…*, offered at start while missing,
    `starter_ui.rs`):
    - downloads the pack of the editor's version from the GitHub release (`RUSTPG2D_STARTER_URL`
      for tests) on a thread, with progress and a SHA-256 check against `SHA256SUMS`;
    - or loads it from a file;
    - only `FORMAT` equal to the editor's is accepted;
    - the asset library then gets the new assets.
  - **Fonts:** Fusion Pixel 12 px proportional in four forms (ja, ko, zh_hans, zh_hant), with one
    licence text holding the OFL of Fusion Pixel and of its three source fonts. They are
    library-only.

  **Engine.** Agreed: `starter_pack.rs` (format, read, write, `SHA256SUMS` lookup); the editor
  modules; `build.rs` removed; xtask and the CI workflows.

  **Verify.**
  - **Size:** the release editor is 28 MB instead of 88 MB.
  - **First start** without a starter library: the window is offered; `starterinstall` from a
    file fills an empty library (5 fonts, 464 props).
  - **Downloads from a local server:**
    - installs, `starter=0.23.0(installed)/installed`;
    - a wrong SHA-256 is refused;
    - format 2 is refused with its version;
    - a `SHA256SUMS` without the pack says so;
    - a missing release (404) says so.
  - **Update asset library** from the installed pack: nothing to update.
  - **Tests:** the pack round trip and the `SHA256SUMS` lookup; the editor's starter tests read
    the source folder.

- [x] Text for every language — `feature/unicode-text`

  **Goal.** Languages without spaces, such as Japanese and Chinese, wrap correctly. Right-to-left
  text comes later (backlog).

  **Design.**
  - **Line breaking:** follows Unicode rules: a break may fall between CJK characters, never
    before closing punctuation. Dialogs, choices, the journal, toasts and tooltips all use it;
    today they split at spaces.
  - **Text input:** the name in the creator and 0.26's text input accept any character the fonts
    have (macroquad's `get_char_pressed` already delivers them). IME composition is out of
    scope; the manual says so.
  - **Language menu:** names each language in its own words (`Deutsch`, `日本語`) from a small
    built-in table.
  - **Editor:** the dialog preview uses the project's fonts.

  **Engine.** One wrap function in `runtime/text.rs`, used wherever text wraps. Options: the small
  `unicode-linebreak` crate, or a hand-written CJK rule if the user prefers no new crate.

  **Decided while building.**
  - **Hand-written rule,** no new crate. `text::wrap` / `wrap_counted`:
    - break at spaces;
    - break before and after any character of the CJK ranges (`breaks_anywhere`): ideographs,
      kana, full-width forms;
    - never before closing punctuation (kinsoku, small kana, `ー`) or after opening punctuation;
    - Korean wraps at its spaces.

    `render::wrap_text` and the speech bubbles use it.
  - **Typing out:** the dialog reveals text using each line's count of source characters
    (`wrap_counted`). It used to subtract one assumed space per line, which would have skipped a
    character per Japanese line.
  - **Line spacing:** `text::line_step` gives taller lines (a CJK fallback at 24 px in a 24 px
    line pitch) a 4 px gap in the journal's quest and bestiary texts. Other fixed-pitch layouts are
    unchanged; scale the fallback down if they look tight.
  - **Language names:** the built-in table covers pt-BR, cs, hu, ro, tr, el, ru, uk, ja, ko and
    zh-Hans / zh-Hant; codes match in any case.
  - **Text input** already took any character (`get_char_pressed`); IME composition is out of
    scope (manual).
  - **Editor:** there is no dialog preview. Instead the editor adds the project's role and
    fallback fonts to egui as lowest-priority fallbacks (`ui::add_project_fonts`), so forms and
    lists show Japanese and the like.

  **Touch points.** `runtime/ui.rs`, `journal.rs`, `menu.rs` (language names), `lang.rs`, the
  editor's dialog preview.

  **Verify.** A Japanese scratch translation of one dialog and one quest: screenshots of wrapped
  lines. A unit test of the break points.

  Checked:
  - **Unit tests:** Latin, Japanese kinsoku, mixed Latin / Japanese, Korean, character counts.
  - **A scratch copy of the example** (monogram + Fusion Pixel ja fallback, a Japanese translation
    of Greta's greeting, its choices, the first quest and some menu texts), with screenshots of:
    - the dialog wrapped mid-sentence, and halfway typed out;
    - the journal, with spaced lines (English unchanged);
    - the pause menu;
    - Options showing 日本語;
    - アレックス typed into the creator;
    - the editor's language list showing 日本語.

- [ ] Dialog message codes — `feature/message-codes`

  **Goal.** Dialog lines with colour, inline icons, pauses and speed, like RPG Maker's `\C[n]`,
  `\I[n]` and `\.`.

  **Design.**
  - **Syntax:** codes in braces, like the existing placeholders, so translators see one syntax:
    - `{c:gold}…{c}` sets a colour: a named colour of the UI skin or `#rrggbb`;
    - `{i:item_id}` shows the item's icon inline;
    - `{w:0.5}` pauses;
    - `{s:2}` sets the typing speed;
    - `{name:…}` overrides the name box.
  - **Box position:** a new field of *Dialog* / *Choice* (serde default): `bottom` (the default),
    `middle` or `top`. Scripts get `say_at`.
  - **Rendering:** `expand_text` leaves codes alone; the dialog renderer parses them after
    expansion.
  - **Problems list:** warns about unknown codes, and about translations that drop a code.

  **Engine.**
  - the parser and renderer in `runtime/ui.rs`;
  - the position field;
  - the problem check;
  - the editor's Dialog field gets a small toolbar (colour, icon, pause) and a preview.

  **Example.** A few dialogs colour item names, and Greta's room-key line shows the key's icon.

  **Verify.**
  - Autoplay screenshots of a coloured line with an icon and a pause, at the top and at the
    bottom.
  - `status` `dialog=` shows the plain text.
  - Unit tests of the parser.

- [ ] Runtime texts in more languages — `feature/runtime-languages`

  **Goal.** The engine's own texts (menus, HUD, windows, toasts) ship in more languages than
  German, so creators only translate their content.

  **Design.**
  - **Languages:** `engine/lang/<code>.ron` (runtime texts only) for fr, es, pt-BR, it, pl, ru,
    ja and zh-Hans. `lang::tr_ui` already loads such files.
  - **Quality:** the translations are drafts made with AI help. The manual says so and asks for
    native reviewers.
  - **Tests:** keep every file complete, as for `de.ron`.
  - **Fonts:** ru, ja and zh need project fonts, so this item comes after them.

  **Engine.** Data only: new built-in language files, plus the list of built-in runtime languages.

  **Verify.** Screenshots of each language's pause menu, inventory and options page; the
  completeness test.

## 0.25.0 — Your Own Look

Games stop looking like every other R(ust)PG2D game: window frames, colours, the title screen,
the menus and the HUD all belong to the creator.

- [ ] UI skin — `feature/ui-skin`

  **Goal.** One place to restyle every window, menu, dialog and HUD element without code.

  **Design.**
  - **The skin:** `ProjectConfig.skin`, a `UiSkin`. The default reproduces today's look. It
    holds:
    - a window frame picture drawn as a nine-slice, with its corner size;
    - a background colour or picture, with opacity;
    - text colours: normal, dim, highlight, positive, negative, plus named colours for the dialog
      codes;
    - the selection cursor: a picture or a highlight colour;
    - styles for buttons and bars (HP, mana, XP): colours or pictures;
    - the frames of the dialog box and the name box;
    - the toast style.
  - **Drawing:** every runtime window takes its frame, text colours and cursor from one module,
    `runtime/skin.rs`, instead of colour literals.
  - **Editor:** a *Look* page edits the skin, with a live preview of a dialog, a menu and the HUD.
  - **Library:** skins are assets (pictures plus settings), so a creator can pick one from the
    library.

  **Engine.**
  - the skin struct and `runtime/skin.rs`;
  - the colour and frame literals of the runtime windows move into the skin: big but mechanical
    (grep `draw_rectangle` and colour constants in `runtime/`);
  - the editor page;
  - the library asset kind.

  Compatible.

  **Example.** The example gets a wooden frame that matches its interiors. The starter library
  gets it too, plus a stone skin and the default.

  **Verify.**
  - Screenshots of every window: inventory, character, skills, journal, shop, crafting, dialog,
    choice, pause menu, options and the map.
  - With the default skin, they match master pixel for pixel.
  - With the example's skin, they show its frame.
  - `status` prints `skin=`.

- [ ] Title screen and menus — `feature/title-and-menus`

  **Goal.** Each project designs its title screen and pause menu: background, logo, music, which
  entries appear, their order, and custom entries.

  **Design.**
  - **`ProjectConfig.title`:**
    - a background: a picture, or a map with a camera point and a time of day, animated;
    - a logo picture and its position;
    - the music;
    - the entries (New game, Continue, Load, Options, Credits, Quit), plus custom entries that
      run a script function;
    - the entries' order and alignment.
  - **`ProjectConfig.menu`:** the same for the pause menu, with the built-in pages plus custom
    entries.
  - **Credits:** the *Credits* entry shows 0.26's scrolling text (a text page until then).

  **Engine.** The two config blocks. `menu.rs` builds its rows from them (the defaults are today's
  rows). The editor page with a preview.

  **Example.** The title screen shows the village at dusk, with a logo.

  **Verify.**
  - Autoplay screenshots of the title screen and the pause menu.
  - A custom entry runs its script (check `status` vars).
  - Keyboard and gamepad navigation work over the reordered entries.

- [ ] Script UI — `feature/script-ui`

  **Goal.** Creators build their own HUD elements and simple windows from scripts: a quest
  tracker, a gold counter, a boss bar, a custom status page.

  **Design.**
  - **HUD elements:** scripts set them once instead of Rhai drawing every frame. That is cheap and
    saved with the game. The calls:
    - `hud_text(id, x, y, text)`, with placeholders such as `{coins}` expanded every frame;
    - `hud_image(id, x, y, sprite)`;
    - `hud_bar(id, x, y, w, var, max_var, colour)`;
    - `hud_remove(id)`.

    Elements anchor to corners or the centre and use the skin's colours and fonts.
  - **Script windows:** `window_open(id, title, rows)`, where a row is text, an image or a button.
    A button runs a function.
  - **Pause menu:** 0.25's custom entries open these windows.
  - **Saving:** elements live in `WorldState.hud` (serde default), so they survive saving.

  **Engine.** The element store and its drawing, the window, and the script functions in
  `script.rs`, `script_api.rs` and `08-scripting.md`.

  **Example.**
  - a boss bar in the mine;
  - a *Village life* page in the pause menu: friendship, your home, achievements.

  **Verify.**
  - Autoplay: `status` prints `hud=id…`; screenshots.
  - A button runs its function.
  - Save and load keep the elements.

## 0.26.0 — Director's Tools

The event commands creators expect from other makers.

- [ ] Picture layers — `feature/pictures`

  **Goal.** Several pictures on screen at once, moved, faded, rotated and tinted with easing, like
  RPG Maker's *Show picture* and *Move picture*.

  **Design.**
  - **Actions:**
    - *Show picture* {layer, sprite, x, y, origin, scale, opacity, rotation, tint, blend};
    - *Move picture* {layer, target values, seconds, easing (linear / in / out / in-out),
      wait};
    - *Erase picture* {layer}.
  - **Layers:** numbered 1 to 100, drawn above the map and below the UI, in number order. A
    picture sits on the screen (the default) or on the map.
  - **Show image stays** as the full-screen viewer with scrolling.
  - **Saving:** pictures are saved in `WorldState.pictures`.
  - **Scripts:** `picture_show`, `picture_move` and `picture_erase`.

  **Engine.** Three appended actions, `runtime/pictures.rs`, the editor fields, the script
  functions.

  **Example.** A title card in the intro cut-scene.

  **Verify.** `status` prints `pictures=layer:sprite@x,y:alpha`; screenshots mid-move; save and
  load keep the pictures.

- [ ] Screen tint, flash and fade — `feature/screen-effects`

  **Goal.** Colour the whole screen for a mood, a flash or a transition.

  **Design.**
  - **Tint screen** {rgba, seconds, wait}: stays until changed, is saved, and is applied after the
    light map.
  - **Flash screen** {colour, seconds}.
  - **Fade out** {seconds, colour} and **Fade in** {seconds}.
  - **Map changes:** a fade-out lasts across a map change until *Fade in*, as with RPG Maker's
    transfer with fade.
  - **Existing actions:** *Shake* and *Dim* stay.

  **Engine.** Four appended actions, the drawing in `runtime/juice.rs` / `lights.rs`, script
  functions.

  **Verify.** Screenshots of the tint, the flash and the fade; `status` prints `screen=`.

- [ ] Timers — `feature/timers`

  **Goal.** A countdown, shown on screen or hidden, that fires when it runs out.

  **Design.**
  - **Timer** {name, seconds, shown, on end: a script function or a custom event}, and
    **Stop timer**.
  - **Condition:** *Timer* (running / seconds left).
  - **Display:** drawn in the skin's style.
  - **Pausing:** timers pause in menus and dialogs.
  - **Saving:** timers are saved.
  - **Scripts:** `timer_start`, `timer_left` and `timer_stop`.

  **Engine.** Two appended actions, one condition, the runtime timers, script functions.

  **Example.** A timed errand: bring the soup to the elder while it's hot.

  **Verify.** `status` prints `timers=name:secs`; the end fires (a flag); screenshots.

- [ ] Number and text input — `feature/input-actions`

  **Goal.** Ask the player for a number or a word.

  **Design.**
  - **Input number** {text, var, digits, min, max}.
  - **Input text** {text, text var, max length}. Variables only hold numbers today, so text
    variables come with it: `WorldState.texts` (serde default), shown as `{name}` placeholders,
    plus a *Text variable* condition and the `get_text` / `set_text` script functions.

  **Engine.** Two appended actions, text variables, the input windows.

  **Example.** The shrine asks a riddle whose answer is a number; the player names their house.

  **Verify.** Autoplay `type` / `press`; `status` vars and `texts=`; screenshots.

- [ ] Move routes — `feature/move-routes`

  **Goal.** Choreograph characters step by step without scripts.

  **Design.**
  - **Action:** *Move route* {who: the player, a uid or a name; steps; repeat; skip when blocked;
    wait}.
  - **Steps:**
    - move N tiles in a direction, or toward / away from the player;
    - turn (a direction, toward the player, at random);
    - wait;
    - jump;
    - speed;
    - play an animation clip;
    - sound;
    - walk through walls (on / off);
    - run a script.
  - **Editor:** a list editor with one button per step, as in RPG Maker.

  **Engine.** One appended action, `runtime/routes.rs` (built on `try_move` / `walk_to_tile`), the
  editor's step list.

  **Verify.** `status` prints `routes=who:step/steps`; a route in a cut-scene; a blocked route
  skips on.

- [ ] Common events — `feature/common-events`

  **Goal.** Reusable action lists, picked in the editor instead of written as script functions.

  **Design.**
  - **Definitions:** a new `defs/common_events.ron` of named action lists. Each has an optional
    trigger:
    - *Parallel while flag*: runs again and again while the flag is set;
    - *Autorun while flag*: blocks the player while it runs;
    - none: only when called.
  - **Calling:** the *Call common event* action and the `call_common(id)` script function.
  - **Editor:** a Database tab with the trigger editors.
  - **Bookkeeping:** renames and references, plus library assets.

  **Engine.** A new defs file (`project.rs` DEFS, `release.rs`, `library.rs`), one action, the
  runner of parallel / autorun events, the Database tab.

  **Example.** A reward routine the example's quests share becomes a common event.

  **Verify.** A called event, a parallel one and an autorun one in autoplay; `status` prints
  `common=`.

- [ ] Credits and the camera — `feature/credits-camera`

  **Goal.** Scrolling text for credits and story intros, and camera moves for cut-scenes. Today
  the camera always follows the player.

  **Design.**
  - **Scroll text** {text, speed, background}: ends at the end or on a key.
  - **Camera** {to a tile / follow a uid / the player, seconds, easing, zoom}: back on the player
    when the cut-scene ends.

  **Engine.** Two appended actions, a camera target in `runtime/mod.rs`, script functions.

  **Example.** Credits from the title screen; the intro pans over the village.

  **Verify.** Screenshots mid-pan and mid-credits; `status` prints `camera=`.

- [ ] Music effects and sound control — `feature/music-effects`

  **Goal.** Fanfares, plus control over how sounds play.

  **Design.**
  - **Play music effect** {sound}: a fanfare. The background music pauses and resumes after.
  - **Play sound:** gets pitch, pan and volume (serde defaults).
  - **Play music:** gets a crossfade time.
  - **Fanfares:** victory and level-up fanfares in the project's sounds.

  **Engine.** One appended action, two fields, `audio` mixing.

  **Verify.** `status` `music_fade=` and `sounds=`; a level-up plays its fanfare.

## 0.27.0 — Plugins

Systems become add-ons: scripts can change the engine's outcomes, and plugins package them with
their settings.

- [ ] Script hooks — `feature/script-hooks`

  **Goal.** Scripts change what the engine does: damage, healing, item use, levels, prices,
  saving.

  **Design.**
  - **Registration:** the project declares hooks in `project.ron` `hooks`; a plugin declares them
    in its `plugin.ron` (hook name, function, priority).
  - **Calls:** hooks run in priority order. Each gets a map (`#{attacker, target, amount, element,
    …}`) and returns a changed map, or `()` to keep it.
  - **Hooks:** `damage`, `heal`, `item_use`, `xp`, `level_up`, `buy`, `sell`, `craft`,
    `map_enter`, `new_game`, `save`, `load`, `menu_open`, `effect_apply`, and later
    `battle_start` / `battle_end` and `encounter`.
  - **Cost:** only registered hooks are called, under a Rhai operation limit.

  **Engine.** `Script::call_hook` returning a value (today's `call` / `call_args` return actions),
  the call sites, `08-scripting.md`.

  **Verify.**
  - A damage-doubling hook in autoplay: `status` hp.
  - A shop price hook.
  - Perf: `update_ms` with ten hooks.

- [ ] Plugin packages — `feature/plugins`

  **Goal.** Shareable add-ons with settings, which stay separate from the project's own content
  and can be updated.

  **Design.**
  - **Contents:** `plugins/<id>/plugin.ron` holds:
    - the name, version, author and description;
    - `api`: the script API version it needs;
    - `parameters`: typed (number, text, on/off, sprite, sound, item, map …) with defaults;
    - hooks, menu entries and HUD elements;
    - its scripts and assets.
  - **Project:** `project.ron` `plugins` lists {id, enabled, parameters} in load order.
  - **Scripts:** read parameters with `plugin_param(name)`.
  - **Editor:** a *Plugins* window to add from a file (`.r2dplugin`, the `data.pack` format) or
    the library, switch plugins on and off, set their order, fill in parameter forms, update by
    version, and see API mismatches in the problems list.
  - **Export:** includes the enabled plugins.
  - **Compared with library bundles:** bundles copy assets into the project; plugins stay
    separate and updatable.

  **Engine.** Plugin loading (scripts, assets, hooks), the window, the library asset kind,
  export.

  **Verify.** Install, configure, update and remove a plugin on a scratch project; an exported
  game runs it.

- [ ] A versioned script API — `feature/api-versions`

  **Goal.** Plugins and scripts know which functions they can count on.

  **Design.**
  - **The API table:** entries in `script_api.rs` get `since` and `deprecated` (with the
    replacement).
  - **Scripts:** get `api_version()`.
  - **Editor:** completion marks deprecated functions, and the script check warns about them.
  - **Plugins:** a plugin's `api` is checked when it loads.
  - **Manual:** 08 shows "since 0.27".
  - **Policy:** until 1.0, a function may change, with a *Breaking:* line. From 1.0, deprecated
    functions keep working until 2.0.

  **Engine.** The table fields, the checks, the manual generator / test.

  **Verify.** A deprecated call warns in the check and still runs.

- [ ] Starter plugins — `feature/starter-plugins`

  **Goal.** Show the plugin model with useful examples.

  **Design.** The starter library gets a few plugins:
  - a minimap;
  - styles for damage numbers;
  - a calendar HUD;
  - a hard mode (an enemy damage hook with a parameter).

  **Engine.** None (content).

  **Verify.** Each plugin in autoplay with screenshots.

## 0.28.0 — The Party

Heroes grow by class, damage follows formulas, and companions become a real party.

- [ ] Classes, parameter curves and traits — `feature/classes`

  **Goal.** RPG Maker's classes: stats per level from a curve, skills learned at levels, and
  traits.

  **Design.**
  - **Classes:** a new `defs/classes.ron`. A `ClassDef` has a name, a curve per stat (level 1,
    maximum level, curve shape), skills and spells learned at levels, the equipment kinds it may
    use, and traits.
  - **Traits:** a list shared by classes, items (gear), races and effects:
    - element rates;
    - effect resistance;
    - a stat in percent;
    - the attack element;
    - extra attacks;
    - the encounter rate.

    Races keep their stat changes as traits.
  - **The player's class:** a default in `progression`, or a choice in the creator; stored as
    `WorldState.player_class`.
  - **Compatibility:** no class means today's stats and progression.

  **Engine.** The defs file, the trait evaluation (`progression.rs`, `combat.rs`), the editor tab,
  the creator choice.

  **Verify.** Level-ups follow the curve (`status` stats); a trait resists an effect.

- [ ] Damage formulas and elements — `feature/damage-formulas`

  **Goal.** Damage creators can shape.

  **Design.**
  - **Elements:** listed in the project settings.
  - **Fields:** skills, spells and weapons get a `formula`, a Rhai expression over `a` (the
    attacker) and `b` (the target), such as `a.atk * 4 - b.def * 2`, plus an element, a variance
    in percent and a critical chance.
  - **Default:** an empty formula means today's damage.
  - **Cost:** expressions are compiled once and cached.
  - **Traits:** element rates come from the traits.
  - **Display:** damage numbers show the element's colour and critical hits.

  **Engine.** The fields, the evaluation in `combat.rs` / `skills.rs`, the form fields with a test
  calculator.

  **Verify.** Unit tests of formulas; autoplay hits with an element weakness (`status` hp).

- [ ] Party members — `feature/party`

  **Goal.** Up to four heroes with their own levels, classes, skills and gear.

  **Design.**
  - **Recruiting:** companions become party members when `CharacterDef.party` is set, or through
    a recruit action. Each member has their own level, XP, class, skills, spells and gear
    (companions already have gear and a bag).
  - **Party page:** a page in the character window chooses whose page is shown, the formation
    order, and who follows. Reserve members wait at a place of their own (the inn).
  - **Size:** the project sets the active members (default: 3 besides the player).
  - **XP:** shared.
  - **Leader:** the player stays the leader; controlling another member is in the backlog.
  - **Saving:** `WorldState.party`; today's companions convert on load.

  **Engine.** The party data, the page, XP sharing, the action / script functions (`party_add`,
  `party_remove`, `party_has`).

  **Example.** Dagny (warrior) and Finn (ranger) join the party with classes.

  **Verify.** Recruit both, level up, swap the follower, equip each; save and load; `status`
  prints `party=`.

## 0.29.0 — Turn by Turn

An optional turn-based battle mode, front view, alongside real-time combat.

- [ ] Turn-based battles — `feature/turn-battles`

  **Goal.** RPG Maker's front-view battles as a mode creators can choose per encounter.

  **Design.**
  - **Troops:** a new `defs/troops.ron`. A `TroopDef` has:
    - enemies (a character plus a place on the battle screen);
    - a background: a picture, or the map dimmed behind;
    - music;
    - battle events: conditions (turn N, enemy HP %, member HP %, flag) leading to actions.
  - **Starting a battle:**
    - *Start battle* {troop, can escape, on win, on lose: game over or continue}, and
      `start_battle()`;
    - touching an enemy with `CharacterDef.troop` set starts that troop instead of real-time
      combat;
    - real-time combat stays the default.
  - **The battle screen:**
    - enemies as large front-facing poses of their 3D models;
    - the party's status window;
    - a command window per member: Attack, Skill, Spell, Item, Guard, Escape;
    - target choice;
    - turn order by speed (with a random spread).
  - **Rules:** 0.28's damage formulas; effects with turn counts; animations from 0.31's editor
    (effect sheets until then).
  - **Enemy AI:** `CharacterDef.actions` lists actions with ratings and conditions.
  - **The end:**
    - victory: XP, gold, loot-table drops and level-ups;
    - defeat: game over, or carry on.

  **Engine.** Large:
  - a new `runtime/turns/` (not `battle.rs`, which is the battle stance and music): a state
    machine, enemy AI, drawing;
  - the defs file;
  - the action;
  - the editor's Troops tab with a battle preview;
  - autoplay commands (`battle cmd N`, `battle target N`).

  **Verify.**
  - Autoplay a battle to victory and one to defeat, with screenshots of each command window.
  - `status` prints `turns=round:actor:hp…`.
  - Battle events fire.

- [ ] Random encounters — `feature/encounters`

  **Goal.** Battles that find the player, by region, as in RPG Maker.

  **Design.**
  - **Zone:** `ZoneKind::Encounters` (appended) with a table of {troop, weight} and the average
    steps between battles.
  - **Map-wide:** the map settings may hold a table too.
  - **Steps:** counted in tiles walked.
  - **Switching:** *Encounters on / off* (action and script).
  - **Rate:** a trait can change it, such as a repellent.
  - **Real-time games:** spawner zones already cover them.

  **Engine.** The zone kind, step counting, the action.

  **Verify.** Walk in a zone until a battle starts (`status`); an off zone stays quiet.

- [ ] The training ring — content

  **Goal.** The example shows the mode in play.

  **Design.** The hunter offers practice duels in a clearing, as turn-based battles against one
  or two enemies. A short quest step leads there. Turn-based sample projects are in 0.32.

  **Engine.** None (content on the two items above).

  **Verify.** The duel won and lost in autoplay; German texts.

## 0.30.0 — Everywhere

Touch screens, phones, Steam and signed downloads.

- [ ] Touch controls — `feature/touch`

  **Goal.** The game plays on touch screens: tablets, phones in the browser, and touch laptops.

  **Design.**
  - **When:** `settings.touch` is `auto`, `on` or `off`; `auto` switches on at the first touch.
  - **Controls:**
    - a virtual stick on the left half;
    - buttons: attack, interact, menu, plus configurable ones;
    - tap a tile to walk there and interact (right-click does this for the mouse today);
    - tap a UI row to click it;
    - long press = right-click;
    - pinch to zoom in photo mode.
  - **Look:** buttons in the skin's style.

  **Engine.** `runtime/input.rs` (macroquad `touches()`), `mouse.rs`, UI hit tests; autoplay
  `touch X Y`, `touchhold`, `swipe`.

  **Verify.** Autoplay touch commands: walk, talk, fight, shop and menus; screenshots of the
  buttons; `status` prints `touch=`.

- [ ] Mobile web — `feature/mobile-web`

  **Goal.** Web exports work on phones.

  **Design.**
  - **Phone basics:** touch (from above), a full-screen button, a web manifest (installable, with
    the icon), a landscape hint, UI scale for small screens, sound unlocked by the first tap.
  - **Download size** (moved from the backlog): today `data.pack` is fetched whole before the
    game starts, and the 3D models are 29 of the example's 46 MB.
    - Models become separate files, fetched on map enter (the player's model and those of the
      map's characters) and when a character first needs one.
    - `data.pack` keeps everything else and shows its download progress.
    - Desktop stays as it is.

  **Engine.** `sys.rs` (web), `web/rustpg2d.js`, `release::export_web`, model loading in
  `render/model.rs`.

  **Verify.** Headless Brave with a phone viewport and touch emulation: start, walk, talk.
  Measure the start download size before and after.

- [ ] Steamworks — `feature/steamworks`

  **Goal.** Games released on Steam use its achievements, cloud saves and rich presence.

  **Design.**
  - **Export:** a *Steam* option in the export dialog, with the App ID.
  - **Features:**
    - achievements unlock through Steam (the engine's ids map one to one);
    - cloud saves: Steam Auto-Cloud on the save folder, documented, or the remote storage API;
    - rich presence shows the map's title;
    - the overlay.
  - **Runner:** a separate variant, `rustpg2d-game-steam` (cargo feature `steam`, the
    `steamworks` crate, with the redistributable `steam_api` library beside the exe), so other
    exports don't carry it.

  **Engine.** Needs the user's go-ahead for the dependency. Test with Steam's development app
  (480); test exports on Linux and Windows.

  **Verify.** An achievement unlocks in Steam's overlay with app 480; the export contains the
  library.

- [ ] Signed builds — `feature/signed-builds` (moved from the backlog)

  **Goal.** Downloads that don't scare players: Windows today is unsigned, macOS only ad-hoc
  signed.

  **Design.**
  - **Exports:** Windows exports are signed with the creator's certificate (`osslsigncode` on
    Linux, set up in the export dialog). macOS exports are signed with a Developer ID and
    notarized (`rcodesign` can do both from Linux with an App Store Connect API key).
  - **R(ust)PG2D's own downloads:** signed the same way in CI.

  **Engine.** Packaging only. Needs the user's certificates and accounts; ask first.

  **Verify.** `osslsigncode verify`; `spctl -a -vv` on the test Mac.

- [ ] Android export — `feature/android`

  **Goal.** Games as Android apps.

  **Design.**
  - **Runner:** macroquad runs on Android through `cargo-quad-apk` (or its docker image), built
    in CI.
  - **Export:** writes an APK / AAB with the game's `data.pack` in its assets, the icon and the
    app id, signed with the creator's keystore.
  - **Controls:** touch, from above.
  - **iOS:** stays in the backlog (it needs a Mac and an Apple developer account).

  **Engine.**
  - `sys.rs`: Android paths (saves in the app's data folder);
  - `release.rs`: `Platform::Android`;
  - a CI job.

  The riskiest item of the road. If it grows too big, it moves after 1.0.

  **Verify.** The example on an Android emulator (`emulator -no-window`, `adb exec-out screencap`):
  start, walk, save, load.

## 0.31.0 — Studio Comfort

Faster ways to build what the earlier releases made possible.

- [ ] Cut-scene timeline — `feature/cutscene-timeline`

  **Goal.** Build cut-scenes in a track view instead of script functions.

  **Design.**
  - **The asset:** a new `defs/cutscenes.ron`, with tracks:
    - one per actor: walk to, face, animation, emotion, bark;
    - camera (0.26);
    - dialog;
    - sound and music;
    - pictures and screen (0.26).

    Each entry has a time, or runs after the one before.
  - **Playing:** the existing cut-scene runtime plays it, turned into actions with waits.
  - **Editor:** a timeline window with a map preview and scrubbing.
  - **Compatible:** *Play cutscene* takes a cut-scene asset or, as today, a script function.

  **Engine.** The defs file, the conversion, the timeline window.

  **Example.** The intro cut-scene rebuilt as a timeline.

  **Verify.** The intro plays as before (screenshots at the same moments); editor screenshots of
  the timeline.

- [ ] Database table view — `feature/db-table`

  **Goal.** Change many entries at once: balance prices, damage, HP.

  **Design.**
  - **The view:** a table toggle on the Database tabs, with one row per entry and columns for the
    main fields.
  - **Sorting:** by any column.
  - **Bulk editing:** select several rows, then set a value, add a percentage or apply a formula,
    as one undo step.

  **Engine.** Editor only (`database.rs`); a UI test in `ui_tests.rs`.

  **Verify.** The UI test: sort, set prices of three items, undo, save and reload.

- [ ] Test-play from here — `feature/testplay-setup`

  **Goal.** Test any moment of the game without playing up to it.

  **Design.**
  - **Starting:** right-click on the map › *Play from here*.
  - **Test setup window:** level, items, flags and variables, quests at a step, time of day, the
    party. It is saved per project in the editor's settings.
  - **How it starts:** the editor writes a crafted save, as autoplay tests do with `quick.ron`,
    and starts the game with it.

  **Engine.** Editor plus a game argument to load a given save (`--load FILE`).

  **Verify.** Editor script `playhere X Y`; the game starts there with the items (`status`).

- [ ] Skill animation editor — `feature/animations`

  **Goal.** Animations for skills, spells, items and battles, timed with sound and flashes.

  **Design.**
  - **The asset:** a new `defs/animations.ron`. An `AnimationDef` has:
    - timed cels: frames of a sprite sheet with position, scale, rotation, opacity and blend;
    - particle emitters: count, speed, spread, lifetime, colour over life, gravity;
    - flashes of the target or the screen;
    - sounds at frames.
  - **Uses:** skills, spells, items, *Play effect* (an animation where a sheet is accepted) and
    turn-based battles.
  - **Editor:** a timeline and a preview on a target dummy.
  - **Library:** starter animations: slash, fire, ice, heal, thunder, buff.

  **Engine.** The defs file, the player in `runtime/fx.rs`, the editor.

  **Verify.** Each starter animation in autoplay screenshots mid-play; `status` prints `fx=`.

## 0.32.0 — Coming from RPG Maker

A way in for RPG Maker users: their projects, their terms, and small games to learn from.

- [ ] RPG Maker MV / MZ importer — `feature/rpgmaker-import`

  **Goal.** People switch with their work.

  **Design.**
  - **Source:** *File › Import › RPG Maker MV / MZ project*: the `data/*.json` and `img/` folders.
    - Only the creator's own files are imported.
    - The RTP's files are licensed for use with RPG Maker only, so the importer skips them unless
      the creator confirms a licence that allows it. The manual says so.
  - **Maps:**
    - tilesets (A1–A5 autotiles, B–E) go through the tile converter of `convert_tiles.rs`:
      materials from autotile groups, decoration props from B–E tiles;
    - passability becomes collision;
    - regions become zones.
  - **Database:**
    - actors → characters (with a class);
    - classes → classes (0.28);
    - skills, items, weapons and armours → their counterparts;
    - enemies → characters;
    - troops → troops (0.29);
    - states → effects;
    - animations: images only.
  - **Events:** pages become triggers with conditions.
    - Commands with a counterpart become actions: Show text, Choices, Control switches /
      variables, Transfer, Change items / gold, Conditional branch, Wait, Play SE / BGM, Show
      picture, Tint, Move route, Common event, and so on.
    - Any other command becomes a comment action naming the original.
    - Switches become flags (named after the switch names), and variables become variables.
  - **Plugins** aren't converted; the report lists them.
  - **Report:** a window lists what was imported, converted and skipped.

  **Engine.** Editor: `import/rpgmaker.rs`. Fixtures: a small MV / MZ project made for the tests,
  without RTP files.

  **Verify.** Import the fixtures with the editor script `import rpgmaker DIR`, then play the
  imported map: walk, talk, a switch, a transfer.

- [ ] Tutorials and an FAQ for RPG Maker users — `feature/tutorials`

  **Goal.** A learning path.

  **Design.**
  - **Step-by-step manual chapters** with screenshots: "Your first quest", "Your first dungeon"
    and "Your first turn-based battle".
  - **"Coming from RPG Maker":** the terms side by side:
    - switches → flags;
    - self switches → prop states;
    - common events → common events;
    - plugins → plugins;
    - parallel processes → parallel common events.
  - **README:** links to them.

  **Engine.** None (docs). The screenshots are taken with editor and autoplay scripts.

  **Verify.** Follow each tutorial on a fresh project and fix every step that doesn't match.

- [ ] Sample projects — `feature/sample-projects`

  **Goal.** Small games to learn from, besides Emberbrook.

  **Design.**
  - **The samples:**
    - a turn-based JRPG start: three maps, a party, a dungeon and turn-based battles;
    - a short dungeon crawl.
  - **Where:** offered on the welcome screen like the example, and packed the same way
    (`files.rs`, the bundled example).
  - **Library:** their new assets go to the starter library.

  **Engine.** None (content); a list of samples for the welcome screen.

  **Verify.** Each sample played through in autoplay; the problems list is empty for both.

- [ ] A community place — ask the user

  **Goal.** Somewhere for creators to meet, show their games and share assets.

  **Design.** GitHub Discussions, a showcase page, issue templates (some exist), and a curated
  index of community assets and plugins in the repository.

  **Engine.** None. These are the user's settings and decisions.

  **Verify.** None.

## 0.33.0 — A Bigger Library

A content release for the starter library. Generating 3D models costs Meshy credits: ask first.

- [ ] Material and decoration sets — `feature/biome-sets`

  **Goal.** More worlds than a green valley.

  **Design.** Snow, desert, dungeon, cave and swamp: materials, props, scatter sets and
  structures for each. The example gets a cave level in the mine with them.

  **Engine.** None (content).

  **Verify.** A screenshot of each set on a test map; the example's cave in autoplay.

- [ ] Monsters and animals — `feature/bestiary`

  **Goal.** Enemies and wildlife with walk, idle, attack and hurt animations.

  **Design.** A slime, a skeleton, a bat, a spider, a bear and a boar, plus farm animals. Each one
  costs Meshy credits (ask), and each is a character in the library with a loot table.

  **Engine.** None (content).

  **Verify.** `model_sheet` contact sheets; each one fights in autoplay.

- [ ] Swappable hair and clothing — `feature/model-parts`

  **Goal.** The character generator changes shapes, not only colours.

  **Design.**
  - **Parts:** an `Appearance` gets parts (a hair model and an outfit model) skinned to the same
    rig as the body.
  - **Drawing:** `render/model.rs` draws them in one pose.
  - **Creator:** offers parts per sex.

  **Engine.** Parts on a shared skeleton in `model.rs` / `render/model.rs`, the appearance field,
  the creator and editor choosers. Generating the parts costs Meshy credits.

  **Verify.** Contact sheets with three hair styles; the creator in autoplay.

- [ ] Music and sounds — `feature/more-audio`

  **Goal.** Enough music and effects for a whole game.

  **Design.**
  - **Music:** battle, dungeon, town and night, plus the 0.26 fanfares.
  - **Effects:** UI, magic, footsteps on new materials.
  - **Sources:** CC0 or made for the project, each with its licence noted in the library.

  **Engine.** None (content).

  **Verify.** The `sounds=` status in autoplay; license notes.

## 1.0.0 — Stable

The promise creators need for a commercial release: what works with 1.0 keeps working through
1.x.

- [ ] Format review and the migration chain — `feature/format-freeze`

  **Goal.** Every format is versioned, and every change from here on comes with a migration.

  **Design.**
  - **Versions:** every file format gets a version number: `project.ron` (`format`), the defs
    files, maps (`FORMAT_VERSION` exists) and saves (`WorldState.version` exists).
  - **Migration chain:** `migrate.rs` gets one step per format change (N → N+1), run on open with a
    backup and a report. Save games get their own chain at load.
  - **Review before freezing:** every format is reviewed once. Pre-1.0 leftovers kept only for
    compatibility (the legacy `body` of `Appearance`, `EntityV1`) become migration steps where a
    conversion can replace them.
  - **Old versions:** decide which pre-1.0 versions 1.0 still opens. The default is: projects of
    0.20 and later through the migration chain, older ones through the existing converters.
    Confirm with the user.
  - **Fixtures:** 1.0.0 files of every format.

  **Engine.** `migrate.rs`, the version fields, the compat fixtures and tests.

  **Verify.** Every fixture of every earlier version opens (or is refused with a clear message,
  as decided). A 0.23.0 project migrates and plays.

- [ ] The stability promise — `feature/stability-promise`

  **Goal.** Creators know what 1.x guarantees.

  **Design.**
  - **Manual:** a new *Compatibility* page.
    - What 1.x keeps: projects, maps, definitions, save games (exported games' saves too) and
      the script API.
    - What may change: the default look of the built-in UI and default balance values.
    - How deprecation works.
  - **Rules:** AGENTS.md and CLAUDE.md switch back to hard compatibility, with migrations.
  - **This file:** *Compatibility until 1.0.0* is removed.

  **Engine.** None (docs and rules).

  **Verify.** None (docs).

- [ ] Release candidates — `release/1.0.0-rc.N`

  **Goal.** 1.0.0 ships without known blocking bugs.

  **Design.** A bug bash:
  - the example and the samples played through on every platform: Linux (AppImage, Flatpak),
    Windows, macOS, web, and Android if shipped;
  - performance on big maps;
  - the manual read through, with every screenshot current;
  - the problems list empty for the example and the samples.

  Release candidates (`1.0.0-rc.N`) follow until nothing blocks; then 1.0.0.

  **Engine.** Fixes only.

  **Verify.** The checklist above, recorded in the release pull request.

## Backlog (unscheduled ideas)

- **Right-to-left text** (Arabic, Hebrew): shaping and bidirectional runs in `runtime/text.rs`,
  after 0.24's fonts.
- **iOS export:** needs a Mac and an Apple developer account; after Android (0.30).
- **Side-view battles** for the turn-based mode (0.29): the 3D models' animations seen from the
  side.
- **Controlling a party member:** switch the leader, after 0.28.
- **Vehicles:** a boat, a ship, an airship over the world map; a world-map mode with a smaller
  character and a different scale.
- **A shortcut sheet in the editor,** and gamepad remapping in its play-test preview.
- **Consoles:** need publisher programmes; not planned.
- **Visible helmets, hats, backpacks and quivers** (moved from 0.20.0):
  - item models on the head and back bones of the 3D characters (`HoldStyle::Head` / `Back`;
    `Gear` with head and back items);
  - placed in `render/model.rs` like held items;
  - needs new Meshy item models (about 33 credits each; ask first);
  - the full design block is in git history (ROADMAP.md, 0.20.0).
- **Dungeons generated from structures** (a theme for a later release): a dungeon map kind that
  stitches captured rooms and corridors (`structure.rs`) into a new layout per visit, with
  enemies, chests and loot from tables.
- **Mounts:** riding a horse (two models in one pose: the rider seated on the mount's back bone),
  faster on roads.
- **New Game+:** start again from a finished save, keeping level and items.
- **Seasons** (moved from 0.20.0):
  - a year of N days;
  - tile variants per season (autumn / winter tiles without repainting);
  - crops by season and snow in winter;
  - a *Season is* condition and `season()`;
  - the full design block is in git history (ROADMAP.md, 0.20.0).
