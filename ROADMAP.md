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
   pass. The example project (`example_project/`, edited with the editor or its AI agent
   server) shows the feature: add new entities instead of recreating existing ones, so uids
   stay stable.
6. **Commit, push and open pull requests only when the user asks.** When all items of a
   version are `[x]` (or the user decides to ship early), cut `release/X.Y.Z` as described in
   AGENTS.md and remove the version's section from this file (its design notes live on in the
   CHANGELOG and the code).
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
11. **Example content.** `example_project/` and the starter library
    (`crates/editor/assets/starter_library/`) are plain files: edit them with the editor or its AI
    agent server (CLAUDE.md, *Example content*), send new or changed example assets to the
    starter library (`library_send` with `replace` and `drop_script_triggers`, the library set to
    that folder) and commit both together; existing entities keep their uids; give new example
    characters their own ids; never touch `example_project/saves/`.
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
    the version's roadmap section removed from this file (the CHANGELOG is its record),
    `cargo xtask check-version release/X.Y.Z`, pull request into master, merge after the four
    checks, merge the back-merge pull request.

## 0.11.0 — A living village

The example becomes a game worth playing and the asset library grows with it: every terrain of
every starter tileset is a 47-tile set (inner corners everywhere), the library gains the props,
characters, animals, items, sounds, music and structures a fantasy village needs, the village is
rebuilt as **Emberbrook** (80 × 60: a river with bridges, a cobbled market square, tavern, bakery,
smithy, apothecary, chapel and graveyard, mill and farm, fisher's dock and pond, an east gate) with
enterable interiors, a daily life (every villager on a schedule, animals, children, a town crier,
lamps and lit windows at night, dawn fog, the noon bell) and four more quests. The Workshop test
map goes away; its stations and content move into the village. Four pull requests, in this order
(each builds on the one before): `feature/tilesets-47`, `feature/village-assets`,
`feature/village`, `feature/village-quests`; then `feature/example-as-files` retires the
generator that built all of it, and `feature/ladders` makes the gate's watchtowers climbable. Everything added to the example is also in the
starter library (`STARTER_RULES` 5 → 6; existing user libraries have to be regenerated to get the
new terrain art, see README). Save games made in the old village don't fit the new one (entity
uids change); the other maps only gain entities at the end.

- [x] Every terrain a 47-tile set — `feature/tilesets-47`

  **Goal.** All starter tilesets (`terrain`, and the library's desert, mountains, city, dungeon)
  carry full 47-tile blob sets for every terrain, so painted paths, water, cobbles and floors get
  inner corners everywhere; the 16-tile blocks disappear. New single tiles and four new terrains
  (Cobblestone, Farmland, Meadow, Carpet) give the village its materials.

  **Design.**
  - `terrain.png` becomes **16 columns (512 px)**: rows 0–3 the single tiles 0..63 (indices 0–34 keep
    their numbers; new 35..62: `COBBLE, TILLED, FLOWER_BED, CARPET, TILED_FLOOR, FENCE_V, FENCE_POST,
    GATE_PILLAR, HEDGE, DOCK, DOCK_EDGE, BRIDGE_RAIL_N, BRIDGE_RAIL_S, ROOF_SLATE(+_EDGE),
    ROOF_THATCH(+_EDGE), CHIMNEY, CHIMNEY_SLATE, STONE_HOUSE, STONE_WINDOW, STONE_DOOR, CHAPEL_WINDOW,
    COUNTER, GRAVE_MOUND, TIMBER_WALL_X, WINDOW_DARK, STRAW_FLOOR`), rows 4–5 reserved, then one 7 × 7
    blob block per terrain, two per 7-row band from row 6 (`blob_start(k) = (6 + 7*(k/2))*16 +
    (k%2)*7`): 14 terrains → 512 × 1760 px (under egui's 2048 px limit).
  - `TERRAINS` (`gen_example/art.rs`): 0 Dirt path · 1 Pond (solid; also the river) · 2 Sand ·
    3 Shore (animated, solid) · 4 Forest floor · 5 Snowfield · 6 Forest path · 7 Stream (animated,
    solid) · 8 Flagstones · 9 Rug · 10 Cobblestone (on grass, joins Dirt path / Flagstones) ·
    11 Farmland (tilled soil on grass) · 12 Meadow (flowers on grass) · 13 Carpet (on wood floor).
    `terrains()` fills each `Terrain.blob` with `fill_blob_from_block(blob_start(k), 16)`, sets
    `blob[46]` to the ordinary inside tile where there is one, and derives the 16-tile fallback
    `tiles[k] = blob[BLOB_SLOT[k | 0xF0]]`, `tiles[15] = inside` (used when *Corners* is off).
    `terrain_anims()` keeps the water cycle and animates the Shore / Stream inside tiles;
    `terrain_solid()` = `SOLID` + all 47 tiles of Pond, Shore and Stream. Remove `block_start`,
    `BLOB_TERRAINS`, `edge_block`; `EdgeArt` entries for the four new terrains.
  - The library extras (`art/extra.rs::add_terrains`) lay one blob block per 7-row band below the
    base rows of desert / mountains / city / dungeon (base rows unchanged, so `structures.rs`
    indices stay); `main.rs::mine()` uses `Terrain::inside()` instead of `tile_for(15)`.
  - `B::building(x, y, w, Roof::{Red, Slate, Thatch}, Wall::{Timber, TimberX, Stone}) -> door`
    (chimney at `x+w-2`, a lit window tile, walkable door) prepares the village build.
  - `library.rs`: `STARTER_RULES = 6` (the 16-column sheet is not an extension of the old one, so
    `add_missing` keeps the old art; the user regenerates the library). Delete the stale
    `example_project/tilesets/desert.png`.

  **Touch points.** `crates/engine/examples/gen_example/art.rs` (`mod t`, `TERRAINS`, `terrains`,
  `terrain_anims`, `terrain_solid`, `terrain_edges`, `blob_block`), `art/extra.rs`
  (`add_terrains`, tileset sizes), `main.rs` (`B`, `mine`), `library.rs` (`STARTER_RULES`),
  README (terrains, library note), CHANGELOG.

  **Example.** Every example map is regenerated with the new sets (no visible change but the
  inner corners); `example_project/tilesets/terrain.png` 512 × 1760.

  **Verify.** Regenerate; editor screenshots of every map before / after; `tool terrain; terrain
  Cobblestone; terrainpaint 5 5; terrainpaint 6 5; terrainpaint 6 6; shot` shows an inner corner;
  `terrainedit 10` shows the filled template; `terrainblob terrain "Dirt path" off` then painting
  gives the 16-tile fallback; `rustpg2d-editor --library` thumbnails of the tilesets;
  `cargo run -p rustpg2d-editor --example mcp_smoke` still passes.

- [x] Village assets: props, characters, animals, items, sounds, music, structures — `feature/village-assets`

  **Goal.** The props, people, animals, goods and structures of a fantasy village, in the example
  project's definitions and the starter library, with German texts.

  **Design.**
  - Sheets: `sprites/village.png` (new, `art/village.rs`), `props2.png` moved from the library
    extras into the example (minus the duplicates campfire / anvil / bench; `write_starter_pack`
    filters extras' props by id and asserts unique ids per kind), `sprites/items2.png` (new icons:
    `items.png` is full), `portraits.png` grows to 11 (baker, innkeeper, herbalist, priest, miller,
    blacksmith, fisher), animal bodies `chicken`, `dog`, `cat`, `cat_ginger`, `sheep`, `cow` (32 × 32
    frames, `art/animals.rs`, not playable), children `child_boy` / `child_girl` (pre-drawn 32 × 48
    like skeleton / ghost), held sheets `held_broom`, `held_hoe`, `held_hammer`, `held_pickaxe`,
    7 new + 5 promoted outfits (`outfits()` → 20).
  - Props (ids): `market_counter_produce|fish|bread`, `awning_red|green|blue`, `flour_sacks`, `keg`,
    `keg_rack`, `tavern_table`, `stool` (+ `_up|_left|_right` use-spot variants), `bread_shelf`,
    `dough_table`, `herb_rack`, `potion_shelf`, `bellows`, `oven` (cooking station "Bread Oven",
    lit / out), `hearth` (cooking station "Kitchen Hearth"), `grindstone`, `tool_rack`, `forge`
    (smelter station "Forge"), `fireplace`, `weapon_rack`, `cauldron`, `chicken_coop`, `apple_tree`
    (gather apples, regrows daily), `trough`, `hay_bale`, `beehive` (honey), `boat`, `broken_cart`,
    `scarecrow`, `notice_board`, `fish_rack`, `fish_barrel`, `laundry`, `watchtower`, `bell_tower`,
    `altar`, `candle_stand`, `bell_rope` (rings `bell`), `grave_cross`, `pew` (sit), `brazier`,
    `flower_pot`, `flower_box`, `woodpile`, `millstone`, `cat_basket`, `sign_tavern|bakery|smithy|
    apothecary`, `moonpetal` (opens 20–5 h only: `If TimeBetween`), `feverleaf_bush`,
    `bitterroot_plant`, `windmill`.
  - Characters (own ids; shops on the definition): `baker`, `innkeeper`, `herbalist`, `priest`,
    `miller`, `fisher`, `town_crier`, `child_boy`, `child_girl`; promoted from the extras with
    their looks: `farmer`, `farmer_f`, `blacksmith`, `villager_f`, `guard_f`; animals `chicken`,
    `dog`, `cat`, `cat_ginger`, `sheep`, `cow` (`max_hp 0`, Wander, a sound + one line on E).
  - Items: `flour`, `egg`, `cheese`, `milk`, `honey`, `moonpetal`, `feverleaf`, `bitterroot`,
    `remedy`, `locket`, `festival_bread`, `ale`, `broom` (tool), `hoe` (tool). Recipes
    `bake_festival_bread` (cooking, lit station; flour 2 + egg + honey), `brew_remedy` (alchemy,
    mortar; moonpetal + feverleaf + bitterroot). Sounds `bell`, `cluck`, `bark`, `meow`, `baa`,
    `moo`; music `music_tavern` ("The Copper Kettle", `music.rs`). Structures: `tavern_front`,
    `bakery_front`, `smithy_stone`, `chapel`, `farmhouse_thatch`, `mill`, `market_row`,
    `animal_pen`, `graveyard`, `dock`, `orchard`, `gate_towers`, `room_tavern`, `room_smithy`,
    `room_bakery`, `room_chapel`.

  **Decided while building.** A market stall is one 3x3 prop (awning, the seller's place, the
  counter) instead of separate counters and awnings: `stall_produce`, `stall_fish`,
  `stall_bread`. The village sheet is packed from one list (`art/village.rs::SPRITES`; defs look
  sprites up by name). The definitions live in a new `gen_example/village_db.rs`. A millstone
  station grinds flour (`grind_flour`, station "mill"). The oven, hearth, fireplace, forge and
  brazier start lit and are relit with coal. The festival loaf and the remedy aren't sold (quest
  items). The library's male merchant / wizard / nomad / bandit outfit sheets became
  `outfit_<id>_m…` (their female cut collided with the female versions' outfits);
  `write_starter_pack` now asserts one id per kind. Portraits are drawn from a `Bust` table.

  **Touch points.** `gen_example/art.rs` (+ new `art/village.rs`, `art/animals.rs`), `art/extra.rs`
  (remove the promoted assets), `main.rs` (`database()`, `write_starter_pack`), `structures.rs`,
  `music.rs`, `lang_de.rs` (every new text), `render/character.rs` (`Held` variants if the held
  sheets need them), README (asset list), CHANGELOG.

  **Example.** Definitions only (the maps use them in the next item); the library window shows
  every new asset with a thumbnail.

  **Verify.** `rustpg2d-editor --library` with `libfilter prop|character|structure` + `shot`;
  a scratch map with `stplace` of every structure; `chartemplate baker` etc. in the builder;
  `sndplay bell`; a test map with `music_tavern`; autoplay equipping broom / hoe shows the held
  sprites; the starter pack rebuilt without duplicate ids; the lang test passes.

- [x] Emberbrook: the village, its interiors and daily life — `feature/village`

  **Goal.** The village map rebuilt as a realistic fantasy village with enterable key buildings,
  villagers who live by the clock, animals, children, a town crier and a day / night atmosphere;
  the Workshop map removed and its content integrated.

  **Design.**
  - 80 × 60 map. River down x ≈ 18 with two bridges (main street y 29–31, lower lane y 46–47);
    cobbled market square x 26–42 y 15–28 (well, notice board, statue, lamps, benches, three market
    stalls: produce, fish, flour); tavern, bakery north of the square, smithy with a flagstone yard
    north-east, elder's house, Tilda's shop, crier's home and barracks facades east; apothecary with
    a herb garden, chapel with bell tower, fenced graveyard (moonpetals), Mira's home, fisher's hut
    with dock, boat and pond (fishing spot) south; orchard with apple trees and a beehive; west of
    the river the windmill, mill house, farmland (12 plots, scarecrow, seed sack), farmhouse,
    chicken coop and a fenced pen (sheep, cow). Stone wall and gate towers east; the east edge
    leads to the forest (`offset` −6 / +6), no north edge. Spawn `start` at the square.
  - Interiors via a shared `interior(name, w, h, floor, wall)` helper: `tavern` (16 × 12, counter,
    kegs, fireplace, hearth, pantry chest, tables and stools, two inn beds usable after paying
    Greta 5 coins = flag `inn_room_paid`, cleared by the map's Sleep trigger), `bakery` (oven,
    counter, shelves, a cat), `smithy` (forge, anvil, grindstone, racks, supply chest), `apothecary`
    (alchemy table, cauldron, racks, chest), `chapel` (altar, candles, pews, bell rope), `home_mira`
    (three beds, cat basket), `mill` (millstone, sacks; Biscuit the cat, inactive until the quest).
    Each interior has a portal back to `<name>_door` in the village; every scheduled NPC has a named
    bed.
  - Villagers with schedules (`ScheduleStop`, activities with tools): Elder Rowan, Mira, Pip (sick
    in bed; a healthy twin inactive), Ida, Tomas, the guard (patrol), watchwoman Dagny (night
    rounds), Greta (innkeeper), Bram (baker), Brom (blacksmith), Wren (herbalist), Brother Ansel
    (priest), Jorun (miller), Hal and Beth (farmers), Old Finn (fisher), crier Oswin (`crier_talk()`
    rumours), children Lotte and Finnick, Rufus the dog, chickens, sheep, a cow, a cat. Dialog
    lines gated by `TimeBetween` / `Pose`; shops open after the dialog; notice board = an `If`
    chain over the quest states.
  - Atmosphere: lit windows, street lamps, the glowing tavern sign, braziers at the gate; **new
    `TriggerOn::Hour { hour }`** (appended variant, Map owner; `GameEvent::Hour` fires when the
    clock crosses the hour, also after `set_hour` / sleep) drives dawn fog (5:30 on, 8:30 off), the
    morning cluck and the noon bell. Fallback if the trigger is cut: MapEnter / Sleep + `TimeBetween`.
  - Workshop removal: `workshop()` and `example_project/maps/workshop.map` go; stations and content
    move (supply chest → smithy / apothecary chests, herb garden → apothecary, smelter → the smithy
    forge, anvil → smithy, plots + seeds → farmland, campfire + pantry → tavern hearth / bakery oven,
    beds → inn and homes, smith / farmer → Brom, Hal, Beth). Forest: feverleaf bushes and the raided
    flour cart with two inactive outlaws (quest hook); mine: bones + the `locket` pickup; house: the
    bed named `elder_bed` (append only).

  **Decided while building.** The village lives in a new `gen_example/village_map.rs`
  (`village()` and the seven interiors; buildings are placed from `structures::building_def` /
  `chapel_def`). Market stalls are the 3x3 stall props. Villagers live on the village map and
  visit the interiors (the tavern's Greta and Mira's sick brother live in theirs); facade
  houses (farmhouse, crier's home, barracks, fisher's hut) are `inside` stops. The bell is one
  sound (no `Wait`: actions would hold the player); the dawn mist is also set on Map enter by
  the hour (an interior visit can't miss the 8:30 change). The raided cart in the forest and the
  locket in the mine move to the quests item, which uses them. Engine fixes found here: walkers
  that start overlapping can separate, a blocked walker re-plans around people; `status()`
  prints `sounds=`.

  **Touch points.** `gen_example/main.rs` (`village`, new `interior`, the map list, `forest`,
  `mine`, `house`, `database()` schedules), `events/mod.rs` + `runner.rs` + `runtime/mod.rs`
  (`TriggerOn::Hour`), `editor/src/widgets.rs` + `graph.rs` (the new trigger), `lang_de.rs`,
  `Documentation/events.md`, README (village tour, Workshop paragraphs rewritten, saves note),
  CHANGELOG.

  **Example.** The village itself; `intro_cutscene` keeps its texts (the player arrives on the
  west road, the guard approaches).

  **Verify.** Autoplay `--map village --spawn start`: day / night screenshots; `settime 5.6` →
  `weather=fog`, `settime 8.6` → clear, `settime 11.95; wait 4` → the bell; `settime 7/12/15/18/21/2`
  → `schedule=` rows of every villager at the right stop / map and `activities=` (craft, farm, fish,
  sweep); enter every interior and come back; talk to every NPC and animal; pay Greta and sleep in
  an inn bed; buy at every shop; the new-game intro; the forest edge both ways; `mcp_smoke` passes;
  the Hour trigger in the editor's event picker and `events_reference`.

- [x] Four village quests — `feature/village-quests`

  **Goal.** Quests that use the new village: *The Lost Cat* (Mira; clues from Bram, Finn, Jorun →
  Biscuit in the mill → Mira), *Bread for the Festival* (Bram; flour from the raided cart in the
  forest → bake a Festival Loaf at the oven → Bram), *The Herbalist's Remedy* (Ida; feverleaf,
  bitterroot and a night-only moonpetal → brew at Wren's table → cure Pip), *Rumours at the Tavern*
  (Greta; buy a round → Brom's story → Halvar's locket in the mine → Brom). `strange_lights` and
  `pest_control` stay.

  **Design.** Quest ids `lost_cat` (30 xp), `festival_bread` (60), `herbal_remedy` (70),
  `tavern_rumours` (90), each with three stages and `{var}` progress texts where useful (`cat_clues`);
  triggers on the NPCs ordered NotStarted → stages → Completed; script functions `cat_clue()` and
  `crier_talk()`; achievements `cat_person`, `festival_baker`, `village_healer`, `old_debts`
  (`Quest Completed` conditions, icons from the item sheets). `shrine_touch`, `mira_talk`,
  `slime_defeated`, `intro_cutscene` stay as `Documentation/scripting.md` quotes them. German for
  every new text. README: six quests, village tour, interiors, schedules, the Hour trigger,
  terrains, three music tracks; CHANGELOG complete; then `release/0.11.0`.

  **Decided while building.** The quests live in `gen_example/village_quests.rs`: their
  triggers are put in front of the villagers' own (by name, on the maps village_map.rs builds),
  plus the forest's raided cart with two inactive outlaws (a Map enter trigger wakes them while
  the flour is wanted) and Halvar's bones and locket in the mine. The festival, remedy and
  rumour quests have two stages (baking / brewing and handing over are one stage: crafting fires
  no event). Rewards come on completion (no later "come back" talk). Greta's menu gained
  *Heard any rumours?*; Bram's everyday lines wait until his quest was offered. Achievements
  unlock from `Quest Completed` conditions. Engine: talking across counter props (stalls), the
  autoplay command `talkto NAME`.

  **Touch points.** `gen_example/main.rs` (`database()` quests / achievements, NPC triggers,
  `SCRIPT`), `lang_de.rs`, README, CHANGELOG, this file.

  **Example.** The four quests, playable start to end.

  **Verify.** One autoplay per quest from start to `quests=` completed (`status` at every stage,
  `achievements=` grows; the moonpetal refuses by day and gives at `settime 21`; the oven needs
  `lit`); `--lang de` screenshots; `langtemplate de` reports nothing missing;
  `cargo xtask check-version release/0.11.0`.

- [x] The example and the starter library as files — `feature/example-as-files`

  **Goal.** No more generator: `example_project/` and the editor's starter asset library are
  plain, committed files, changed through the editor and its AI agent server, which makes every
  content change a small diff instead of Rust code plus two regenerated outputs.

  **Design.**
  - The starter library becomes a library folder, `crates/editor/assets/starter_library/`
    (unpacked once from the last `starter.r2dpack`; identical assets, files and script
    functions). `crates/editor/build.rs` embeds its files (`library_ui::STARTER`);
    `Bundle::from_files` reads them (`project::parse_defs` shared with `Project::load`) and
    `open_library` takes them, stamped with a hash of the files (stable across zip / ron versions;
    `STARTER_RULES` stays 6, the new stamp reruns `add_missing` once and adds nothing).
  - `crates/engine/examples/gen_example/` and the `vorbis_rs` dev-dependency are removed.
  - Checks the generator made become tests: `lang.rs` `example_translation_has_no_unused_entries`;
    the editor's `starter_library_is_consistent` (ids once per kind, dependencies, script
    functions) and `starter_library_has_every_example_asset` (every example asset in the library,
    identical, triggers running story scripts left out).
  - Three AI agent server tools for what only the generator could do: `write_asset` (PNG / WAV /
    Ogg into the project, decoded first, art reloaded), `capture_structure` (a map region becomes
    a structure) and `library_send` (project assets to the library; `replace` updates existing ids
    in place — `Bundle::replace_into` — and `drop_script_triggers` leaves story triggers behind).

  **Touch points.** `engine/src/{library,project,lang,hash}.rs`, `editor/build.rs`,
  `editor/src/library_ui.rs`, `editor/src/mcp/{tools,exec,exec_defs,exec_maps}.rs`,
  `editor/examples/mcp_smoke.rs`, `Documentation/mcp.md`, README, CLAUDE.md, AGENTS.md, the PR
  template, CHANGELOG, this file.

  **Verify.** The new pack-free starter equals the old pack (all kinds, 147 files, 4 functions);
  a fresh library, an old one (gains the new ids) and an up-to-date one (unchanged) open in
  `--library`; the new-project wizard copies its starter assets; `mcp_smoke` passes with the new
  tools against a scratch copy and a scratch library.

- [x] Ladders: climbable lookout towers — `feature/ladders`

  **Goal.** Ladder tiles the player climbs (only up and down, slower, seen from behind), used
  for the village's two watchtowers at the east gate: climb up to the lookout deck under the
  roof. The north tower no longer overlaps the barracks. (Swimming, planned together with
  ladders before, was dropped.)

  **Design.**
  - `TilesetDef.ladder: Vec<u32>` (`#[serde(default)]`, not written when empty), like `solid`;
    `TilesetTex.ladder`, `Assets::compute_ladder` → `Loaded.ladder` (layers below the
    characters). A ladder tile isn't solid; the player's feet on one = climbing: horizontal
    input is ignored, speed × 0.6 (`CLIMB_SPEED`), facing up, drawn towards the ladder's middle,
    no attacks. `status()` `climbing=`.
  - Characters keep off ladders: `rect_blocked_by` blocks them for movers that ignore
    themselves (NPCs), and their path finding, spawning and companions' spots use
    `tile_blocks_npc`.
  - Editor: *Database → Tilesets* gets *Pick ladder tiles in the grid* (green ladder icon on
    ladder tiles); the AI agent server's `tileset_legend` lists them, `def_template` notes them.
  - The tower is 2 × 5 tiles of `terrain.png` row 4 (64–79): roof (Overhead) over a solid tile;
    the lookout (back posts and rail, solid; front corner posts on Overhead); the deck
    (walkable, front rail on Overhead so it covers the legs of whoever stands there); legs with
    the ladder in the left column (ladder tiles 76, 78) and braces on the right (solid). A new
    structure `lookout_tower` places it.
  - Starter updates (`STARTER_RULES` 6 → 7): an image of the same size that only adds art in
    empty cells is an upgrade too (`extends`), and an upgraded tileset's new tiles bring their
    solid / ladder / light / animation flags (`new_tiles`), so existing libraries get the tower.

  **Touch points.** `engine/src/{defs,library}.rs`, `render/mod.rs`, `runtime/{mod,combat,
  patrol,ai,companions,spawner,mouse}.rs`, `editor/src/{database,mcp/exec_maps,mcp/exec_defs}.rs`,
  `example_project/` (terrain.png, tilesets.ron, structures.ron, village map), the starter
  library, README, CHANGELOG, CLAUDE.md, `Documentation/autoplay.md`.

  **Example.** The barracks gets its east wall column back; tile towers at x 74–75, rows 23–27
  and 33–37 replace the two `watchtower` props; a woodpile and a blocked cell guard the south
  deck's open side.

  **Verify.** Autoplay from a save at the ladder foot: `hold up` → `climbing=true`, `hold left`
  doesn't move, `press x` doesn't attack, on up to the deck (`climbing=false`), along it, the
  braces block going down on the right, back down the ladder to the road; the same on the south
  tower; screenshots of climbing and standing on the deck. Engine tests: ladder RON, filled-cell
  upgrades with their flags.

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented).
- Gamepad button glyphs in the HUD and hints (Steam Deck); rebinding gamepad buttons.
- Companions learn spells from scrolls and books given to them and cast heal / bolt spells by
  the rules of *Companion inventory* (`Companion.spells`, a mana pool from the definition).
- Activities that produce something: the fishing NPC sells fresh trout at a market stall the
  next morning (a trigger at the end of an *NPC routines* activity).
