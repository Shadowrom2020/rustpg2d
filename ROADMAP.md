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

## 0.7.0 — Trades and town life

Theme: the player gathers, smiths, farms and cooks, and the townsfolk visibly live their days
across maps, using furniture and working at their trades. Suggested order: the gathering
groundwork with mining → blacksmithing; farming → cooking; then the townsfolk items, which show
the trades in their routines.

### [x] Mining and gathering — `feature/mining`

**Goal.** Ore veins the player breaks with a pickaxe, plus the groundwork the other trades use:
tools in recipes, gathering props and prop states that change over in-game days.

**Design.**
- *Tools* (partly there: `ItemDef.tool` names a tool kind and the Tool slot holds one):
  `RecipeDef.tools: Vec<String>` (tool kinds needed, not used up: equipped in the Tool slot or
  in the inventory); the crafting window lists a missing tool like a missing ingredient
  (`Missing::Tool`).
- *Days*: `WorldState.day: u32` (serde default 0), advanced when the hour wraps (also by
  sleeping, `pass_time`); `total_hours() = day * 24 + hour`. Autoplay `settime +H` advances
  across days.
- *Timed prop states*: `PropDef.regrow: Option<Regrow { from, to, hours }>`;
  `EntityState.changed_at: Option<f64>` (total hours, serde default) is set on every state change,
  and the state is moved on when the map is entered or the time passes it on the current map,
  so it also works while the player is elsewhere.
- *Gathering nodes*: `PropDef.gather: Option<Gather { tool, hits, loot, then_state, skill }>`:
  with the tool equipped (or in the bag), attacking or E on the prop counts a hit (a progress
  bar over the prop, chips as an effect); after `hits` the loot table rolls (`roll_loot`), the
  prop switches to `then_state` ("depleted"). `SkillEffect::Gather { tool, percent }` (appended)
  lowers the hits.
- *Mining*: copper, iron and silver veins (gem veins rarely) need a *pickaxe* (tool kind
  "pickaxe", also a weak weapon); depleted veins regrow after 3 in-game days.

**Decided while building.** `RecipeDef::missing` / `craft` take the item -> tool kind map
(`Database::tool_kinds`; scripts get it with `set_recipes`); `crafting::has_tool` checks
equipment and inventory. The pickaxe is a Tool-slot item only (an item is either a tool or a
weapon: `equip_slot`), so any weapon's swing counts as a hit while the pickaxe is carried.
Gathering lives in `runtime/gather.rs`: E (`interact_entity`) or a melee swing with no enemy in
it (`attack_at`) on a prop with `gather` that isn't in its `then_state`; hits are forgotten
after 4 s; hits needed = `hits × (1 − Gathering %)`, at least 1 (bonus capped at 80 %). Prop
state changes record `changed_at` (`RtEntity.changed_at`, saved in `EntityState`); a prop
already in a regrow `from` state without a time starts counting when first seen.
`WorldState::advance_hours` moves the clock across days (the running clock, sleeping, autoplay
`settime +H`); `Action::SetTime` still only sets the hour. `status()` gains `gather=`
(def:hits/needed), `props=` (props not in their first state) and `day=`.

**Touch points.** `crafting.rs`, `runtime/crafting_ui.rs`, `state.rs` (`day`, `changed_at`),
`defs.rs` (`Regrow`, `Gather`), `skills.rs`, `runtime/{mod,lights,furniture,combat}.rs`, new
`runtime/gather.rs`, editor `database.rs` (prop form), game `main.rs` (`settime +H`), README.

**Example.** Veins in the old mine's dark corridors, a pickaxe in the mine's entrance chest, a
*Mining* skill; ore sells at Tilda's.

**Example content added afterwards (user request: tools, stations and resources for every
crafting skill so far).** Alchemy: a *Mortar and Pestle* (tool kind "mortar", at Tilda's) needed
by every alchemy recipe, an *Alchemy* skill (Antidote 1, Swiftness 2; Swiftness needed Arcana 1
before), herb bushes and mushroom patches (gathered bare-handed in one hit, regrow after 24 h).
Mining: coal veins, a *smelter* by the mine entrance (`smelter` station: 2 ore + 1 coal → an
ingot, *Smith's Tongs* needed, silver at Mining 2; it glows), tongs in the miner's crate and at
Tilda's. Checked: a herb bush gives a herb and is back the next day; the smelter lists its three
recipes, with ore, coal and tongs it makes a copper ingot and the tongs stay; the alchemy table
marks every brew missing without the mortar and makes a potion with it (screenshots of both
crafting windows with the tools and skill levels). Then (user request) **the Workshop**: a map
north of the village (village `edges.north`, offset -11, through a gap in the border trees at x
27-28) gathering it all: supply chest (pickaxe, tongs, mortar, apples, coal), herb garden (three
herb bushes, three mushroom patches, the alchemy table), quarry (all eight veins, the smelter),
a bed and signs; plants regrow after 12 h so one night in the bed brings them back. Checked by
walking north from the village into it and back, taking the chest, picking, mining, smelting a
copper ingot, brewing a potion, and sleeping (07:00 next day, the bush back).

**Verify (done).** Autoplay on a scratch copy of the mine (slimes cleared): right-click on the
iron vein without a pickaxe → "You need a pickaxe for the iron vein."; with one, three hits →
`gather=["vein_iron:1/3"]`, `2/3`, then iron ore (and copper) and `props=["vein_iron:mined
out@10,7"]` (screenshots of the progress bar and the rubble); `settime +71` → still mined out on
day 3, `+2` more → regrown; Mining 2 → `1/2` and an X swing finishes it; F5 / F9 keeps the
change time (`changed_at` in the save), +73 h after loading → regrown. Editor screenshots of the
Gathering / Regrows sections, the Gathering skill effect and the recipe's Tools row. Unit tests:
old saves without `day` / `changed_at`, the clock across days, gathering props in RON, tools
needed but not used up.

### [x] Blacksmithing — `feature/blacksmithing` (follows *Mining*)

**Goal.** An anvil forges ingots into weapons, armor and tools (smelting ore into ingots exists
since *Mining and gathering*: the example's smelter by the mine, coal as fuel, smith's tongs).

**Design.**
- The *anvil* crafting station (a prop with `Open crafting`): ingots, leather, wood → weapons,
  armor, arrowheads, tools; recipes need a *hammer* and the smith's tongs via `RecipeDef.tools`.
  Smelting stays at the smelter (`smelter` station, already in the example).
- A *Smithing* skill unlocks better recipes (recipe `skill` / `level`). Quality: optional
  `RecipeDef.quality: Vec<(level, item)>` makes a better item id at higher skill (a *Fine Iron
  Sword* is its own definition, so items stay plain).
- Optional upgrade recipes: an equipped item as an ingredient (sword + 2 iron ingots → fine
  sword), using the existing ingredient check.
- Optional: a forge whose "lit" state is a light source and needs coal to light (the example's
  smelter glows all the time).

**Decided while building.** Quality is `RecipeDef.quality: Vec<QualityStep { level, result }>`
measured on the recipe's `skill` (`result_for(world)`: the highest step reached; `craft` and
`fits` use it, the crafting window shows the result and the next step as a hint, the editor a
*Better results* list). Upgrades needed nothing new: ingredients already come from the
inventory and then the equipment (`take_item`). No forge: smelting is the smelter's. The
example's anvil stands in the Workshop's quarry next to the smelter (no smithy in the village
yet: the blacksmith NPC comes with *NPC routines*).

**Touch points.** `crafting.rs` (`quality`), `runtime/crafting_ui.rs`, gen_example (smithy map
part, forge / anvil / ingot / coal art, recipes), README.

**Example.** A smithy next to Tilda's shop with a blacksmith NPC (by day at the anvil, see *NPC
routines*), forge and anvil, recipes for an iron sword, helmet, chainmail, arrows and the
pickaxe itself.

**Verify (done).** Autoplay in the Workshop: without hammer and tongs every anvil recipe is
missing; with them two iron ingots make an Iron Sword; at Smithing 2 the same recipe makes a
Fine Iron Sword, and the upgrade recipe turns the Short Sword and an ingot into one; screenshots
of the anvil window (tools, skill needs, "Smithing 2: Fine Iron Sword"), the anvil by the smelter
and the recipe's Better results in the editor. Unit test: quality steps and an equipped item as
ingredient.

### [x] Farming — `feature/farming` (follows *Mining*: days, timed states)

**Goal.** The player plants seeds in soil plots, crops grow over in-game days and are harvested.

**Design.**
- Crops: `CropDef { id, name, seed, stages: Vec<(SpriteRef, hours)>, harvest: loot }` in a new
  `defs/crops.ron` (`project.rs` `DEFS`, `release.rs`, `library.rs`), so one plot prop grows
  any crop; Database tab *Crops*.
- Plots: `PropDef.plot: bool` (serde default); using a seed item on a plot (the existing "use
  item on the prop in front" path) plants it; `EntityState.planted: Option<(crop, total_hours)>`
  (serde default) is saved; the plot draws the crop's stage sprite on top of itself. Rain on
  the map counts as watering (growth × 1.5 while it rains on the plot's map); a *watering can*
  tool does the same for a day. Ripe plots are harvested with E (a *hoe* optional: more yield),
  and return to empty.
- Growth is computed from total hours, so it continues while the player is away.

**Decided while building.** No `crops.ron` and no Crops tab: the seed item carries its crop
(`ItemDef.crop: Option<Crop { stages: Vec<CropStage { sprite, hours }>, harvest }>`; the
editor's item form has a *Seed* section, renames and library export follow its sprites and
harvest items), so one plot prop grows any seed. `PropDef.plot: bool`; `EntityState.planted:
Option<Planted { seed, at, bonus }>` (`RtEntity.planted`, saved by `persist`); growth =
`total_hours − at + bonus`, where rain on the current map adds 0.5 h per hour (`water_crops`,
called by the running clock, `advance_time` and sleeping; maps the player isn't on get no rain
bonus). `runtime/farm.rs`: E on a plot (`try_farm`, after gathering in `interact_entity`)
plants the first seed carried / tells the hours left / harvests; using a seed item plants it
on the plot in front (`plant_on_facing`). No watering can and no hoe (left out); the village
garden stays as it is (the field is in the Workshop). `status()` gains `crops=`
(seed:stage/stages|ripe@x,y).

**Touch points.** new `crops.rs` (def), `project.rs`, `release.rs`, `library.rs`, `refactor.rs`,
`state.rs`, `runtime/{mod,gather}.rs`, editor `database.rs` (Crops tab, prop *Plot*), gen_example
(plots, seeds, crop stages), README.

**Example.** The fenced garden behind Tilda's shop becomes six plots; wheat, carrot and pumpkin
seeds at Tilda's; crops sell to her and feed *Cooking*.

**Verify (done).** Autoplay in the Workshop: E on a plot without seeds plants nothing, with seeds
→ `wheat_seeds:1/3`; +9 h → 2/3, +16 h → ripe, E → 3 wheat and the plot empty; carrot seeds used
from the inventory plant the plot in front; F5 / F9 keeps the crop, still ripening on time; 18 h
dry → 2/3, 18 h in rain → ripe; all three crops planted and grown (screenshot of the field
growing and ripe). Editor: the item's Seed section (screenshot). Unit tests: stage timing,
old items / props / plot states.

### [x] Cooking — `feature/cooking` (follows *Farming*)

**Goal.** Raw food becomes cooked food that heals more and gives timed buffs.

**Design.**
- A cooking fire or stove is a crafting station; `RecipeDef.station_state: String` (serde
  default "") requires the station prop's state ("lit").
- Raw food (meat from animals' loot, fish from fishing, mushrooms, apples, crops) → cooked food
  whose *on use* heals and applies the existing effects (*Regen*, *Strength*, *Haste*).
- A *Cooking* skill unlocks recipes; `SkillEffect::FoodBonus { percent }` (appended) raises
  healing from items tagged `ItemDef.food: bool`.
- No hunger system (left out on purpose; a game can script one).

**Decided while building.** The station's prop is the *Open crafting* trigger's owner
(`open_crafting(station, title, owner)`; `CraftingUi.prop`); `recipe_missing` checks
`station_state` against that prop's state first (`Missing::StationState`, "Needs it lit"), so
a crafting window opened by a script without an owner can't make such recipes. Food: the
player's own *on use* triggers of a food item run through `fire_item_use`, which scales
positive *Heal* actions by the *Food* skills (`food_bonus`); companions eating food get no
bonus. The example's campfire starts "out", using coal on it (its UseItem trigger) lights it,
and `regrow` puts it out after 8 hours; no stove in the elder's house and no meat (there are no
animals yet). Dish icons live on a new `food.png` sheet: `items.png` reached 1952 px and egui
refuses textures over 2048 px, which crashed the editor (now any oversized image is shown
shrunk instead: `widgets::color_image`).

**Touch points.** `crafting.rs`, `runtime/crafting_ui.rs`, `skills.rs`, `runtime/combat.rs`
(heal bonus), `defs.rs` (`food`), gen_example, README.

**Example.** A campfire in the Wilds and the elder's stove; four recipes (roast fish, stew,
carrot soup, apple pie); raw meat from the Wilds' animals.

**Verify (done).** Autoplay in the Workshop kitchen: E on the cold campfire opens the window with
every dish "Needs it lit"; coal used on it → `props=["campfire:lit@17,7"]`, a trout → Grilled
Trout; +9 h → the fire is out again; Grilled Trout heals +4 at Cooking 0 and +6 at Cooking 2;
Pumpkin Stew heals and starts Regen (`effects=["regen:1:14.5"]`); at Cooking 2 stew and pie are
made (screenshots of both windows). Editor: the recipe's Station state field; the editor opens a
project with a 4096 px image. Unit test: the food bonus, old recipes / items.

### [x] Schedules across maps and with furniture — `feature/schedule-maps`

**Goal.** A scheduled character goes to other maps (home, the shop, the tavern) and uses
furniture there: sleeps in a bed, sits on a chair, stands at a counter.

**Design.**
- *Stops on other maps*: `ScheduleStop.map: String` (serde default, "" = its own map). The
  character walks to the portal / transition zone of its map that leads there (the nearest
  `ZoneKind::Portal` / `Transition` with that `target_map`) and leaves; on its home map it is
  `away` (as *inside* stops today). Where everyone is lives in the save game:
  `WorldState.npc_whereabouts: BTreeMap<String /* home map */, BTreeMap<u32, String /* map */>>`
  (serde default).
- *Visitors*: building a map's entities adds the characters whose current stop is on it (their
  instance comes from the home map through the map cache), with a runtime uid range of their own
  like `DROPPED_UID_BASE`; their triggers, look and items come along, state changes are saved
  under the home map's uid. Arriving while the player is there: they appear at the portal's
  arrival spawn and walk to the stop; leaving: they walk to the next portal and disappear. Maps
  the player isn't on aren't simulated (placed at the stop of the hour on entering, as today).
- *Furniture* (the player's `UseSpot` exists since 0.5.0): `ScheduleStop.prop: String` (an
  entity name on the stop's map; the tile is the spot in front of it). Arrived, the character
  snaps to the spot and takes the pose, drawn like the seated / lying player (`furniture.rs`
  `draw_seated_player` generalised to any character, with the prop's cover). A used prop is
  skipped by others and refused to the player ("Someone is using it").
- *Interaction*: talking to a sleeping character runs its triggers, with `Cond::Pose { target,
  pose }` (appended) for "Zzz…" lines, or a default "{name} is asleep." toast when no trigger
  matches. `status` `schedule=` gains the map and `pose`.

**Decided while building.** No `WorldState.npc_whereabouts`: where a scheduled character is
follows from its schedule and the hour (other maps aren't simulated), so nothing new is saved.
Visitors come from an index of every map's scheduled characters with stops elsewhere
(`Game::visitor_defs`, loaded once); `build_visitors` adds them to a map's entities with uids
from `VISITOR_UID_BASE` (0x5800_0000), `RtEntity.visitor` carries the home map / uid and the
placed instance's triggers (`collect`), visitors are never persisted. `Schedule` gains `home`
and `elsewhere`; a stop's map is `map_of(k)`. Leaving: the character walks to this map's
`Portal` / `Transition` zone whose target is the stop's map (else its home map's) and goes away
there; arriving while the player is there: it appears on the zone leading back to the previous
stop's map. Furniture: `ScheduleStop.prop` names a prop with a `use_spot`; arrival snaps the
character onto it (`RtEntity.npc_seat`, drawn by `draw_on_furniture`, shared with the player;
sorted at the prop's feet); blocked within 1.5 tiles of a furniture stop it uses it anyway (a
bed's box is solid); getting up uses `free_spot`. The player is refused furniture in use
("Someone is using it."), and E / a click on such furniture talks to its user (`user_of`).
`Cond::Pose { target, pose }` (appended; "" = on its feet) via `ActionHost::pose`. Stops in the
editor: two lines per stop (time, map, tile; facing, inside, furniture); tiles of stops on
other maps are typed; the map view draws only this map's stops. Editor script: `stop K map
NAME|- | prop NAME|- | tile X Y`. `status()` `schedule=` shows `HH:MM>map`, `(elsewhere)` and
`:sit` / `:lie`.

**Touch points.** `entity.rs` (`ScheduleStop`), `state.rs`, `runtime/{schedule,patrol,mod,
furniture,maps}.rs`, `events/mod.rs`, editor `app.rs` / `canvas.rs` (schedule editing: the stop's
map and prop, a stop on another map shown in a list), README.

**Example.** Tomas sleeps in the elder's sideways guest bed instead of vanishing at the door;
Tilda sits behind her counter by day; the bench on the village green in the afternoon.

**Verify (done).** Autoplay on a scratch copy with crafted saves: the village at 15:30 →
`tomas:15:00:sit` on the west bench (screenshot); at 20:29 he walks from the pond to the house
door (20,7) and is `(elsewhere)` by 20:42; the house at 22:00 → `tomas:20:30>house:lie`
(screenshot of him in the sideways bed under the quilt), E next to the bed (or a right-click on
it) → "Zzz… five more minutes…" with his portrait; the house at noon → elsewhere; in the house at
20:27 he comes in at the door and lies down by 20:30; at 06:57 he gets up and leaves through the
door; the village at 06:57 → he comes out of the elder's door at 07:00 and walks to his morning
stop; F5 at 22:00, F9 → asleep again. Editor: the two-line stop rows (screenshot), `stop 1 map
shop` / `tile`. Unit test: old stops, `map_of`, the RON round trip.

### [x] NPC routines: visible activities — `feature/activities` (follows the trades and *Schedules across maps*)

**Goal.** Characters are seen fishing, hoeing, hammering, chopping, sweeping or reading, without
producing anything.

**Design.**
- *Data*: `Activity { kind: ActivityKind, tool: String /* item id */, facing: Option<Facing> }`,
  `ActivityKind { Fish, Farm, Craft, Chop, Mine, Sweep, Read, Custom { sheet } }`; on a schedule
  stop (`ScheduleStop.activity`, serde default) and on a placed character doing it all day (in
  the map's RON `Meta`). Editor: an *Activity* combo (+ tool, facing) in the Daily schedule rows
  and the placed-character inspector.
- *Looks*: the character stands at its spot facing the activity with the tool in hand
  (`player_equipment_drawn` generalised to any character); a work cycle (the walk frames slowly
  in place) and per-activity effects drawn as pure functions of time like the fish shadows:
  *Fish* line and bobber (`rod_tip`) with now and then a splash; *Farm* soil specks; *Craft*
  sparks and the anvil sound nearby; *Chop* / *Mine* chips; *Sweep* dust; *Read* a book;
  *Custom* a creator's sheet. Sounds within a few tiles at the effects volume.
- *Behaviour*: talking pauses it (faces the player, then back to work); nothing is saved.

**Decided while building.** `Activity { kind, tool }` without its own facing (the stop's
facing, or how the character is placed: `RtEntity.act_facing`); no *Custom* kind (a creator's
own sheet) yet. `runtime/activities.rs`: `current_activity` (a reached stop's activity when the
schedule is on, else the placed character's; not on furniture, while away or for companions),
`activity_step` keeps it still in `update_entities` (also during the talk pause, so a wandering
villager doesn't walk off), `activity_equipment` puts the tool in the weapon hand,
`activity_frame` cycles the walk frames per stroke, `draw_activities` draws the effects after the
entities (facing up, the anvil's sparks land beyond the head). Talking sets `act_pause` (5 s).
The anvil / axe / pickaxe stroke plays the hit sound within 4 tiles. `status()`:
`activities=["name:kind@x,y"]`. Example: the elder reads all day, Tomas fishes 17:30-20:30 at
(9,24), the Workshop's smith (anvil) and farmer (field) are new villagers placed there.

**Touch points.** `entity.rs`, `map/io.rs` (Meta), new `runtime/activities.rs`,
`runtime/{schedule,minigame,paperdoll}.rs`, editor `app.rs` / `widgets.rs`, gen_example, README.

**Example.** Tomas fishes at the pond in the afternoon, the blacksmith hammers at the anvil,
a farmer hoes the garden plots, the elder reads by the well.

**Verify (done).** Autoplay on a scratch copy: the village at 18:00 → `activities=["elder:read@26,15",
"tomas:fish@9,24"]`, a screenshot of Tomas with the rod, the line and the bobber on the pond and
one of the elder with an open book; right-click / E on Tomas → he leaves the list while talking
and is back after the pause; the Workshop → `smith:craft@26,11`, `farmer:farm@6,16` with
screenshots of the sparks on the anvil and the farmer's walk cycle by his plot. Editor: the
*Activity* section and the stops' *doing* choice (screenshot). Unit test: activities in RON, the
rod tip.

## 0.8.0 — Scenery and sound

Theme: maps gain smoother terrain edges and depth, and music loops without a gap.

### [ ] 47-tile corner auto-tiling — `feature/autotile-47`

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

**Touch points.** `autotile.rs`, editor `terrain.rs`, `database.rs`, `import/tiled.rs`,
gen_example (art and terrain), README.

**Example.** The starter sand path gets a blob set, composed from quarter tiles in the generator
(inside, edge, outer corner, inner corner), and the village's paths are retiled with it.

**Verify.** Unit tests: the 256 → 47 table (47 distinct, symmetric under rotation), `paint` on an
L-shape gives the inner corner; editor script `terrainpaint` on an L, screenshot; old terrains
without `blob` still paint (test).

### [ ] Parallax layers — `feature/parallax`

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

**Touch points.** `map/mod.rs`, `map/io.rs` (Meta), `render/{parallax (new),mod}.rs`,
`runtime/mod.rs` (draw order), editor `app.rs` (map properties), `canvas.rs`, `main.rs`
(`viewopt parallax`), `refactor.rs` / `library.rs` (image refs travel with maps), README.

**Example.** Cloud shadows drifting over the Wilds (above, alpha 0.15) and a dark mist behind
the mine, seen under empty tiles and beyond the map edge (behind, factor 0.3).

**Verify.** Autoplay screenshots at two player positions show the clouds moving slower than the
ground; a map without layers draws as before; editor screenshot with the preview; map round trip
test.

### [ ] Gapless music loops — `feature/gapless-music`

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

**Touch points.** `audio.rs`, `engine/Cargo.toml` (`lewton`), README (audio notes).

**Example.** The two example tracks loop cleanly (they are generated: also render them at whole
buffer lengths in `gen_example/music.rs`).

**Verify.** Unit tests: decoded and stretched lengths are whole buffers; the mixer replay finds
no gap for a music-length sound. Listening check by the user (unmuted run); underrun grep as for
the rain.

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
