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

## 0.20.0 — A Living Village

Emberbrook should feel inhabited: villagers talk in passing, the world sounds like where you
stand, friendships grow with gifts, quests show where they lead, signposts connect the maps, and
exported games ship as one tidy pack. Build order as listed (the content-only
fast travel early; seasons and stealth last, they are the biggest). Each *Engine* line is a
proposal: agree it with the maintainer before changing Rust code (CLAUDE.md, *Content first*).

- [x] NPC barks — `feature/barks`

  **Goal.** Short lines in a speech bubble over a character's head that need no dialog box and
  don't stop play ("Fresh bread!", "Cold tonight..."), so the village talks while the player walks
  through it.

  **Design.** A bark is a line shown above a character for a few seconds (longer for longer
  lines), drawn like the emotion bubbles (`runtime/emotions.rs`), in the game font, wrapped, one per
  character at a time, fading out. Lines go through `expand_text` and translation like dialogs.
  Triggers decide when: schedule stops (a stop's arrival), the player coming near (a *Trigger*
  zone or a character's own proximity), the hour, the weather. A character can have a list of
  lines and say one at random, not the same one twice in a row. `status` prints `barks=`
  who:text while shown.

  **Engine.** Proposed: an `Action::Bark { target, lines, seconds }` (appended) and a script
  function `bark(who, text)`; drawing next to the emotion bubbles; `status` field; `de.ron` needs
  nothing (lines are content texts).

  **Decided while building.** Agreed as proposed (`Action::Bark { target, lines, seconds }`,
  `bark(who, text[, seconds])`). The bubble is drawn in screen space like the damage numbers
  (`runtime/barks.rs`; wrapped at about 5.5 tiles, above an emotion icon when one shows), not as a
  sprite. A character more than 12 tiles from the player says nothing, and a missing target is
  silent (characters away on their routines are normal), so zone and hour triggers need no
  checks. No proximity trigger was added: the example uses *Hour* map triggers (the crier at
  9:30, 11:00, 15:30, 17:30 while he stands in the square; Dagny at 22:00 and midnight), *Trigger*
  zones by daily stops (market, well, pier, the elder's spot), *Map enter* indoors (bakery, smithy,
  tavern) and a Script condition `is_raining()` for Tomas at 10:00. Verified: each of those in
  autoplay (`barks=`), the German lines (`--lang de`), the far and dry cases silent, the editor's
  action form (shot).

  **Touch points.** `events/mod.rs` (variant, label, template), `runtime/mod.rs` (execute),
  `runtime/emotions.rs` (drawing), `script.rs` + `script_api.rs` (`Arg::Line`), `widgets.rs`
  (action fields), manual 07 / 08.

  **Example.** Bark lines for the baker (dawn), the smith (at the anvil), the night watch, the
  tavern regulars, children; weather lines when it rains. German translations.

  **Verify.** Autoplay past villagers at their stops (`barks=`), a shot of two bubbles at once,
  a German play (`--lang de`), the lines in the translation template.

- [x] Soundscapes and footsteps — `feature/soundscapes`

  **Goal.** Places sound like what they are: birds in the meadow, the river near water, the
  tavern's murmur, wind in the Wilds, fading as the player walks between them; and steps sound
  like the ground (grass, stone, wood, snow).

  **Design.** An *Ambience* zone: a looping sound, its volume and a fade distance (louder towards
  the zone's middle); several zones mix; the existing *ambience* volume option applies (as the
  rain loop does). Footsteps: a terrain or a tileset's tiles name a step sound; the player (and
  nearby characters, quieter) play it on the walk clip's contact frames. `status` prints
  `ambience=` sound:volume and the last steps in `sounds=`.

  **Engine.** Proposed: `ZoneKind::Ambience { sound, volume, fade }` (appended); a `step` sound on
  `Terrain` and per tile on `TilesetDef` (serde default); step timing from the walk clip; looping
  and mixing through `audio.rs` (the rain loop's code).

  **Decided while building.** Agreed as proposed. Steps fall on every half cycle of the walk animation
  (`Models::cycle_seconds`, `Body::step_half`): two per cycle, in time with the drawn feet and
  quicker when running. (A first version stepped every half tile walked: about ten steps a
  second, "a machine gun" in play; the step sounds were also softened then, and each step's
  loudness varies by up to 20 %.) A terrain's step covers only its own
  tiles, not its base tile (often another ground, e.g. grass under a path); plain tiles (grass,
  floors) get `TilesetDef::steps`. Ambience loops use the music's gapless loops; the loudness
  follows the zones also when muted (for `status`). `KINDS` puts sounds before tilesets (a tileset
  now depends on sounds). The ten sounds are synthesized (`scripts/sounds/make_sounds.py`: loops
  as Ogg, steps as WAV). Verified: walking across the village's stream (`ambience=` river 0.11 to
  0.50 and back, steps dirt / wood / stone in `sounds=`), the tavern (murmur, wooden floor), the
  mine (drips, stone); the editor's zone inspector and footprints in the tileset grid (shots);
  tests for the fading, the step lookup and the pick toggling.

  **Touch points.** `map/mod.rs`, `map/io.rs` (Meta), `audio.rs`, `runtime/mod.rs` (zones, steps),
  `defs.rs` / `autotile.rs`, editor zone inspector and tileset / terrain forms, manual 03 / 10.

  **Example.** Meadow birds, the river along the stream, the tavern and the mine's drips; steps on
  grass, stone, wood and snow (new sounds, made like the existing ones).

  **Verify.** Autoplay walking from the meadow to the river (`ambience=` volumes rising and
  falling), steps on each ground (`sounds=`), muted runs still report them.

- [x] Fast travel by signposts — `feature/fast-travel`

  **Goal.** Signposts the player has found take them to the other signposts they found, for some
  time on the clock, so the larger world stays quick to cross.

  **Design.** Content only: a *Signpost* prop whose *Interact* trigger marks it found (a flag per
  signpost) and opens a choice of the found ones (with travel hours shown); choosing one sets
  the time forward and teleports to the signpost's spawn. One library script function builds
  the choice from the flags, so a creator places signposts and names them, nothing else.

  **Engine.** None (content): props, flags, `choice`, *Set time*, *Teleport*.

  **Decided while building.** Content only, as planned, with one change: scripts can't tell
  which entity they run on (`SELF` is only a uid), so instead of a flag per placed signpost the
  prop's own *Interact* trigger runs `signpost_menu()`, which finds the signpost by `current_map()`
  and `player_x()` / `player_y()` in the list `signposts()` (id, name in `text()` so it is
  translated, map, tile, arrival spawn, hours along the roads). Placed signposts need no triggers.
  *Set time* doesn't count days, so travel stops at night (21:00-5:00) and a trip may not end
  after midnight. Verified: village to Whisperwood (10:03 to 12:06, arriving at its spawn), to the
  Wilds (5 h), a first visit, at night, too late to arrive, German; shots of the choice.

  **Touch points.** `example_project/` (props, maps, script), the starter library
  (`library_send`), manual 09 (library content).

  **Example.** Signposts in the village square, Whisperwood, the mine and the Wilds.

  **Verify.** Autoplay: find two signposts, travel between them (`map=`, the clock moved), an
  unfound one isn't offered.

- [x] Relationships and gifts — `feature/friendship`

  **Goal.** Villagers like the player more for kindness and gifts: favourite and disliked
  gifts, friendship that opens new dialog lines and small rewards.

  **Design.** Mostly content: a friendship variable per character (0-10 hearts), raised by
  talking once a day and by gifts (*Use item* on a character already exists: liked +2, loved +3,
  disliked -1, once a day), dialog lines and rewards behind friendship conditions, one library
  script for the bookkeeping so a creator only lists likes and dislikes. The open question is
  where players see the hearts: in dialogs only (content), or a *People* page in the journal
  (engine).

  **Engine.** None for the core; proposed, optional: a journal *People* page that shows a
  character's variable as hearts (ask).

  **Decided while building.** Two engine additions, agreed: the journal *People* page (variables
  `friend_<name>` as ten hearts, `status` `friends=`) and three script functions, because content
  alone couldn't tell which item was given or to whom: `used_item()` (`ActionCtx::item`),
  `self_name()` and `is_consumable(item)` (the game takes a consumable item when a *Use item*
  trigger reacts, so the script must not take it twice and hands a second gift of the day back).
  Points 0 to 100: chat +2 a day, loved +15, liked +8, other +3, disliked -8, a reward at 50. The
  gift trigger's Script condition `friend_takes_gifts()` keeps items of characters without tastes.
  Helper functions don't see `SELF`, and `{self}` gives the definition's name ("Villager"), so
  replies name the speaker from `self_name()`. Verified: a loved gift with the reward, a second gift
  handed back, a disliked non-consumable gift taken (German), the People page (shot); a script test.

  **Touch points.** Example characters' triggers, a library script, `lang/de.ron`; for the
  optional page `runtime/journal.rs`.

  **Example.** Likes and dislikes for every villager, a heart-level reward each (a recipe, a
  discount, a gift back).

  **Verify.** Autoplay giving gifts (`vars=` friendship), the daily limit across `settime +24`,
  a dialog line that appears at a heart level.

- [x] Quest markers — `feature/quest-markers`

  **Goal.** Players see where the current step of a quest leads: a marker over the target in the
  world, an arrow at the screen's edge when it is off screen, a pin on the world map.

  **Design.** A quest stage may name its target: a map and the name of a placed character, prop
  or zone there (`QuestDef.targets`, one per stage, serde default; older quests load as they are).
  Every active quest whose current stage has a target shows a bobbing marker over the target when
  it is on screen, an arrow at the screen's edge pointing to it otherwise; a target on another map
  is shown at the portal or transition leading there (when the current map has one). The world map
  (M) pins the same spots. Players switch markers off in *Options*. `status` prints `markers=`.

  **Engine.** Proposed: `QuestDef.targets`, the markers in the HUD and on the world map, an option
  in `Settings`, the quest form's target fields.

  **Decided while building.** Agreed as proposed, for every active quest. `QuestDef.targets`
  (`QuestTarget { map, name }`, one per stage). A target on this map is a placed entity (marker over
  a character's head by its model's standing height, over a prop's top) or a zone (its middle); on
  another map, the nearest portal, transition or map edge leading there. Hidden during cut-scenes
  and on the title. The map screen's labels sit above the pins (exit labels are below exits).
  Example: three unnamed props got names (`gate_key_chest`, `raided_cart`, `halvar_locket`; saves
  go by uid). Verified: the elder, the house door, the east edge to the forest, the off-screen
  arrow, the map pins of two quests, the option off (shots, `markers=`); a UI test for the form.

  **Touch points.** `quest.rs`, `runtime/journal.rs` / a new `runtime/markers.rs`,
  `runtime/worldmap.rs`, `runtime/menu.rs` + `settings.rs`, editor `database.rs` (quest form),
  `refactor.rs` (map renames), manual 09 / 15.

  **Example.** Targets for the example's six quests' stages.

  **Verify.** Autoplay with a quest at a stage: the marker over its target (shot), the edge arrow
  when it is off screen, a target on another map shown at the door, the world map's pin, the
  option off; `markers=`.

- [x] Game files in one pack — `feature/game-pack`

  **Goal.** An exported game ships its files (definitions, maps, scripts, images, sounds, models,
  translations) as one file next to the program instead of a `data` folder with hundreds of
  files: tidier downloads, nothing to delete or edit by accident.

  **Design.** *Export game* writes `data.pack`: a header, an index (relative path, offset, length)
  and the files one after another, uncompressed (images, sounds, models and maps are compressed
  already). The game opens it at start, reads only the index, and reads a file from its offset
  when asked (no whole-pack load). Everything the game reads goes through `sys`, which serves the
  pack's files where it would read `data/…`; save games, settings and achievements are written
  to the player's folders as before. A game folder with a `data` folder (older exports, editor
  play-tests) keeps working. The web export uses the same format for its download. The export
  dialog can still write loose files (*Pack the game's files*, on by default) for creators who
  want to look inside.

  **Engine.** Proposed (ask before building): the pack format (`release.rs`), mounting it in
  `sys.rs` natively (read, exists, files in a folder), the runner finding `data.pack` next to
  itself (and in a macOS app's Resources), the window icon read through `sys`, export writing it
  (Linux / Windows / macOS folders and zips, AppImage, Flatpak, web).

  **Decided while building.** Agreed, with a scramble: index and files are XORed with a
  position-keyed SplitMix64 stream, so a text or image editor shows noise (documented as no
  protection). The format lives in its own `pack.rs` (`write`, `read_all` for the web, `Pack` =
  index in memory, files read by seek); the magic `R2DPAK1\0` carries the version.
  `release::bundled_root` prefers a `data` folder, else opens `data.pack` and mounts it in `sys`
  as that folder, so nothing else in the engine knows whether a game is packed; `sys::files_in`
  merges the pack's listing with the disk's. The web export's own pack format was replaced by this
  one. Editor scripts: the export commands take a last word `loose`. The AppImage and Flatpak take
  `data.pack` as one file. Verified: the example exported packed (one 45 MB `data.pack`) plays with its
  intro, models, a crafted save loaded and quick-saved, the forest map, German, footsteps and
  ambience; the AppImage, the Flatpak and the Windows export (Wine); a loose export; a packed export over a loose one (the old folder is
  replaced whole); the web export in headless Brave; unit tests for the format and the packed
  Linux and macOS exports (Resources/data.pack, listed in CodeResources).

  **Touch points.** `sys.rs`, `release.rs` (`export`, `export_web`, file list), `icon.rs`,
  `crates/game/src/main.rs` (finding the data), editor `export.rs` / `package.rs` (packages, the
  option), `xtask` dist if it packs the example, manual 14, architecture.

  **Verify.** Export the example for Linux, Windows (Wine) and the web: the folder holds the program,
  `data.pack`, README and license; the game plays (maps, sounds, models, translations, a save and a
  load); an AppImage and a macOS app (zip structure, signing) built from it; an export with loose
  files still runs; a unit test round-trips the pack.

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
- Dungeons generated from structures (a theme for 0.21.0): a dungeon map kind that stitches
  captured rooms and corridors (`structure.rs`) into a new layout per visit, with enemies,
  chests and loot from tables.
- Mounts: riding a horse (two models in one pose: the rider seated on the mount's back bone),
  faster on roads.
- Player housing: items that place furniture props in a house the player owns, kept in the save.
- Photo mode: freeze the game, move the camera freely, no HUD, save a picture.
- New Game+: start again from a finished save, keeping level and items.
- Accessibility: bigger text, captions for sound effects, a high-contrast HUD, hold instead of
  repeated presses.
- Seasons (moved from 0.20.0): a year of N days, tile variants per season (autumn / winter tiles
  without repainting), crops by season, snow in winter, a *Season is* condition and `season()`.
  The full design block is in git history (ROADMAP.md, 0.20.0).
- Stealth (moved from 0.20.0): a sneak key, enemies notice later when sneaking and in the dark
  (light level at the player in `ai.rs` sight), double damage on unaware enemies. Full design in
  git history (ROADMAP.md, 0.20.0).
