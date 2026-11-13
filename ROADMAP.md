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
stand, friendships grow with gifts, the year turns, armor shows on the 3D characters, signposts
connect the maps, and the dark hides a careful player. Build order as listed (the content-only
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

- [ ] Visible helmets, hats, backpacks and quivers — `feature/worn-items`

  **Goal.** Equipped head items and things carried on the back show on the 3D characters, as
  held items already do.

  **Design.** An item model gets two more places besides the hands: the *head* bone (helmets,
  hats, hoods) and the *back* (backpacks, quivers, a sword on the back). The item form's 3D
  model section chooses the place; the hold estimate gets a head and a back fit (size from the
  head bone's span, the back from the shoulders). Head items with a model replace the colour
  feature they cover (hair under a helmet) when the model says so. The bust (portraits, icons)
  shows head items.

  **Engine.** Proposed: `HoldStyle::Head` and `HoldStyle::Back` (appended), `Gear` with the head
  item and the back item, placing them in `render/model.rs` like held items.

  **Touch points.** `defs.rs` (`HoldStyle`, `ItemModel`), `model.rs` (estimates),
  `render/model.rs` (`Gear`, placing), `runtime/paperdoll.rs` / `equipment.rs`, editor
  `models_tab.rs` / `database.rs` (item form), manual 05 / 06.

  **Example.** Models for the iron helmet, a straw hat, a hood, a backpack and a quiver (Meshy,
  about 33 credits each: ask before spending); the guards wear helmets.

  **Verify.** Autoplay equipping a helmet (`held=` shows head / back), shots in four facings and
  walking, the bust with a helmet, characters without the bones (animals) ignore them.

- [ ] Seasons — `feature/seasons`

  **Goal.** The year turns: spring, summer, autumn and winter change how outdoor maps look,
  what grows and the weather.

  **Design.** A year of N days (project setting, default 28 per season), the season from the
  day count (`WorldState.day`). Tiles get season variants (a tileset maps tile -> autumn /
  winter tile, like animated tiles map to frames), so painted maps change without repainting;
  props can show a state per season (bare trees). Crops list the seasons they grow in (out of
  season they wither). The weather's chances per season (snow in winter). Conditions and a
  script function for the season, so schedules and dialogs can differ.

  **Engine.** Proposed: `ProjectConfig.season_days`, `TilesetDef.seasons` (tile variants),
  `Crop.seasons`, a *Season is* condition and `season()`, weather chances per season, `status`
  `season=`.

  **Touch points.** `project.rs`, `defs.rs`, `render/mod.rs` (`display_index` with the season),
  `runtime/farm.rs`, `runtime/weather.rs`, `events/mod.rs` (`Cond`, appended), `script.rs`,
  editor tileset form, manual 05 / 09.

  **Example.** Autumn and winter tiles for grass, trees and paths; crops by season; a snowy
  winter village.

  **Verify.** Autoplay with `settime +H` across seasons (`season=`, shots of the village in each),
  a crop withering out of season, snow only in winter.

- [ ] Stealth — `feature/stealth`

  **Goal.** A careful player can sneak: crouching makes them harder to notice, darkness and
  cover hide them, and a sneak attack hits harder.

  **Design.** A sneak key (and pad button) toggles crouching: slower walk, a crouch clip (or the
  walk lowered). Enemy noticing (`ai.rs`: sight range, half through walls) is scaled by sneaking
  (x0.5) and by the light at the player (the light map's brightness: dark = x0.4), never below
  a minimum near range. An attack on an enemy that hasn't noticed the player does double damage
  (a skill can raise it). `status` prints `sneak=` and per enemy whether it noticed.

  **Engine.** Proposed: sneak state and input, light level sampling at the player, the sight
  scale in `ai.rs`, the sneak attack bonus in `combat.rs`, a sneak clip in `ModelDef.clips`.

  **Touch points.** `runtime/input.rs`, `runtime/ai.rs`, `runtime/lights.rs` /
  `render/lighting.rs`, `runtime/combat.rs`, `settings.rs` (key binding), manual 09 / 15.

  **Example.** The bandit camp at night as a stealth approach; a *Sneaking* skill.

  **Verify.** Autoplay approaching an enemy standing / sneaking / sneaking at night (distance at
  which it notices), a sneak attack's damage (`numbers=`), the pad button.

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
- Quest markers: a quest stage names its target (character, prop or zone); an arrow at the
  screen's edge and a pin on the world map (`worldmap.rs`), switchable in the options.
- Swimming and boats: a *swim* tile flag (like ladders): slow, no weapons, a swim clip; a
  rowboat prop to cross lakes.
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
