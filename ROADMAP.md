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

## 0.23.0 — A Home in Emberbrook

The village grows inward: the tavern gets its guest rooms upstairs, every house an interior with
its residents sleeping in their own beds, and the player can buy the empty stone house and
furnish it. The elder's third quest leads there. The German translation is checked screen by
screen.

- [x] Sleep translation and German pass — `bugfix/sleep-translation`

  **Goal.** Everything the example shows is German when played in German, the bed's sleep
  question too.

  **Design.** The engine builds the bed's *Choice* ("Sleep until 07:00?", "Sleep", "Not now") with
  the runtime translation (`lang::tr_ui`, whose templates fill in the hour): content translation
  can't know texts the engine fills in. Every other engine-built dialog does the same. Script
  texts glued together at run time become placeholders (`set_var` + `{coins}`). Then the example
  is played in German screen by screen (signposts by day and night, beds, shops, crafting,
  journal, quests) and every English text found is fixed.

  **Engine.** Agreed: the sleep choice through `tr_ui`. Found on the way and fixed as part of the
  pass: map titles (`ProjectConfig.map_titles`, the map settings' *Title*), translated crafting
  titles / tool names, character descriptions, furniture refuse texts, save-list and *Continue*
  lines, stacked map-screen labels, text at whole pixels.

  **Touch points.** `runtime/furniture.rs`, `lang.rs` (a test), `example_project/scripts/main.rhai`,
  `lang/de.ron`.

  **Verify.** Autoplay in German (`--lang de`) with screenshots of the sleep question, every
  signpost (day, night, nothing found yet), shops, crafting, the journal; `translation_template de`
  complete.

- [x] Furniture and a player home — `feature/furniture`

  **Goal.** Items that place furniture in a house the player owns (backlog: player housing), kept
  in the save.

  **Design.** `ItemDef.furniture` = the prop it places. `WorldState.homes` (maps the player owns,
  `set_home(map, owned)` / `is_home(map)`) and `WorldState.furniture` (per map: placed props with
  uids from `FURNITURE_UID_BASE`; entering a map adds them to its entities, so containers, states
  and triggers work as on placed props). Using a furniture item at home puts
  its prop on the free tile in front (used up); elsewhere "Furniture goes in your home." Placed
  furniture offers *Pick up* (with *Use* for beds, chairs, chests; a chest with things in it
  refuses). `status` prints `homes=` and `furniture=`.

  **Engine.** Agreed: the item field, the two world fields, placing and picking up, the two
  script functions, the item form's field, renames and dependencies.

  **Touch points.** `defs.rs` (`ItemDef`), `state.rs`, `runtime/mod.rs` (`use_item`,
  `enter_map`), a new `runtime/housing.rs`, `script.rs` / `script_api.rs`, editor `database.rs`,
  `refactor.rs`, manual 05, 08, 09, compat fixture of a 0.22.0 save.

  **Verify.** Place, sit on, pick up a chair at home; refused elsewhere; save and load keep it;
  unit test of the world round trip; old saves load. Done: chair (sit), bed (sleep), chest (open,
  refused while full), no room, not a home, German texts, save + load. Rugs needed one more
  (agreed) addition: `PropDef.flat` = drawn under everyone (game and editor), judged by its picture
  when placed, things stand on it; the library's rugs and runners have it.

- [x] Village interiors and the tavern's upper floor — `feature/village-interiors`

  **Goal.** Every house of the village has an interior; the tavern's guest rooms are upstairs.

  **Design.** New maps `farmhouse` (Hal and Beth), `crier_cottage` (Oswin, Lotte, Finnick),
  `finn_cottage` (Finn), `stone_house` (empty: the player's later) and `tavern_upper` (landing,
  two guest rooms with the inn beds, Greta's room), built from a captured interior. Stairs props
  (`stairs_up`, `stairs_down`, drawn) with portals; the inn beds and Greta's bed move upstairs.
  The residents' night stops lead to their beds inside. Dagny (the night guard) lived in the stone
  house by day: she now rents a room upstairs in the tavern, so the stone house is free.
  Added on request: doors (`PropDef.door`, agreed engine addition: E opens / closes, locks with
  key / lockpick / relock, characters on routes open the doors they may and close them behind
  them, `is_locked`); the upstairs rooms have locked doors and Greta hands out a room key.

  **Engine.** None (content).

  **Touch points.** example maps and schedules, props / sheets (stairs), starter library.

  **Verify.** Enter every house from the village and back; the stairs both ways; residents and
  Greta in bed at night (`schedule=`); renting a room and sleeping upstairs; screenshots.

- [ ] Player housing and the elder's third quest — `feature/player-housing`

  **Goal.** The player can make the empty stone house their home and furnish it.

  **Design.** *A Home in Emberbrook* (quest designer, after *An Offering for the Shrine*): Halvar's
  old house; Brom has the key, the junk inside is cleared (quest objects, a script), Elder Rowan
  takes 100 coins; the reward makes it the player's home (`own_house()`) with a chair to start.
  Furniture items (chair, small table, bookshelf, flower pot, small rug, bench, chest, bed) are
  crafted at the sawhorse and some sold by Tilda; all of it in the starter library.

  **Engine.** None (content on *Furniture and a player home*).

  **Verify.** The quest end to end; `homes=`; furnishing the house; screenshots.

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented). Packaging work: needs the user's go-ahead.
- The web game downloads models on demand. It fetches `data.pack` whole and uncompressed before
  it starts, and in the example the 3D models are 29 of its 46 MB (0.18.2). The models could
  instead be separate files, fetched on map enter (the player's model and those of the map's
  characters) and when a character first needs one; `data.pack` would keep everything else.
  This touches the web file layer (`sys.rs` web, `web/rustpg2d.js`), `release::export_web` and
  model loading (`render/model.rs`, which parses every model at start today; natively that costs
  only 0.22 s, so desktop stays as it is).
- Visible helmets, hats, backpacks and quivers (moved from 0.20.0): item models on the head and
  back bones of the 3D characters (`HoldStyle::Head` / `Back`, `Gear` with head and back items,
  placed in `render/model.rs` like held items); needs new Meshy item models (about 33 credits each,
  ask first). The full design block is in git history (ROADMAP.md, 0.20.0).
- Dungeons generated from structures (a theme for a later release): a dungeon map kind that stitches
  captured rooms and corridors (`structure.rs`) into a new layout per visit, with enemies,
  chests and loot from tables.
- Mounts: riding a horse (two models in one pose: the rider seated on the mount's back bone),
  faster on roads.
- Player housing: items that place furniture props in a house the player owns, kept in the save.
- New Game+: start again from a finished save, keeping level and items.
- Seasons (moved from 0.20.0): a year of N days, tile variants per season (autumn / winter tiles
  without repainting), crops by season, snow in winter, a *Season is* condition and `season()`.
  The full design block is in git history (ROADMAP.md, 0.20.0).
