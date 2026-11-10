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
3. **Branch.** `git checkout develop && git pull --ff-only && git checkout -b feature/<item-slug>`
   (the slug is given in the item's heading). One item per branch; a release's items are
   separate pull requests into `develop`.
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
16. **Cutting a release.** `release/X.Y.Z` from develop: version in `Cargo.toml` (the build
    updates `Cargo.lock`), `## Unreleased` → `## X.Y.Z — date`, README *Current version*,
    the version's roadmap section removed from this file (the CHANGELOG is its record),
    `cargo xtask check-version release/X.Y.Z`, pull request into master, merge after the four
    checks, merge the back-merge pull request.

## 0.14.0 — Forest work, fair shots and gamepads

- [x] Low obstacles — `feature/low-obstacles`

  **Goal.** Low things (fences, crates, rocks, mushrooms, water, counters) stop walking but not
  arrows, bolts and thrown spells; tall things (walls, trees, statues) stop both. Today a shot
  stops at every solid *tile* (a solid mushroom tile blocks arrows) and flies through every
  *prop* (arrows pass through trees).

  **Design.** Two heights for everything solid: *high* (default) and *low*.
  - Tiles: a tileset marks low tiles like it marks ladders (`TilesetDef.low: Vec<u32>`); a low tile
    must also be solid to block walking (low without solid = nothing changes).
  - Props: `PropDef.low: bool` (also per state, `PropState.low: Option<bool>`, so an open gate
    can differ from a closed one). Solid props that are not low now block shots and sight
    like high tiles (this changes behaviour: arrows stop at trees and walls; mention it in the
    CHANGELOG). Characters never block shots (they are hit).
  - One grid for shots (`Loaded.shot_block`, built with `solid` and refreshed with the
    occupancy rebuild for props): projectiles (`update_projectiles`), `line_clear` (mouse aiming,
    enemy and companion line of fire, `Behavior::Chase` sight) read it; walking reads `solid`
    as before.
  - Players see the difference: none needed in the art; the manual explains the flag.

  **Decided while building.** The user chose opt-out props: every solid prop stops shots unless
  marked *Low* (as for tiles). No per-state override: a state's own *solid* already covers open
  gates (only solid things stop shots). A cell painted *force solid* counts as high. Existing
  asset libraries take the low marks for tilesets with the same image and props they haven't
  changed (`STARTER_RULES` 8). `status` gained `foes=` for the checks.

  **Engine.** Agreed: `TilesetDef.low`, `PropDef.low`, the shot grid (`Loaded.shot` from the
  tiles, `OCC_HIGH` in the occupancy grid for props, `Game::blocks_shot`).

  **Touch points.** `defs.rs` (`TilesetDef` + its hand-written `Default`, `PropDef` / `fresh`,
  `PropState`), `render/mod.rs` (tile flags next to `ladder`), `runtime/mod.rs` (`Loaded`,
  `tile_solid` → `tile_blocks_shot`, `rebuild_occupancy`), `combat.rs::update_projectiles`,
  `mouse.rs::line_clear`, `ai.rs` (sight), `companions.rs` / `companion_magic.rs` (line of fire),
  the editor's tileset form (*Pick low tiles in the grid*, like `tile_ladder_pick`) and prop form
  (*Low*), the collision view (low tiles drawn in another colour), `library.rs` tileset merge,
  agent server (`tileset` tools accept `low`).

  **Example.** Mark the low tiles of the example's tilesets (fences, rocks, mushrooms, water,
  bushes, counters, crates) and the low props (barrels, crates, fences, carts); find the
  mushroom the user reported. Starter library updated with the same marks.

  **Verify.** Unit tests: shot grid from tiles + props (low / high / state). Autoplay: a
  crafted save with the player behind a fence and a slime beyond it: `attack` with a bow →
  the slime's HP drops (`status`); the same across a wall → no hit (impact effect on the wall);
  an archer outlaw shoots over a fence; an arrow at a big oak stops. Editor: shot of the low-tile
  pick view and the collision view.

- [x] Woodcutting — `feature/woodcutting`

  **Goal.** Trees can be felled with an axe for logs; the stump grows back after a few days. A
  *Woodcutting* skill makes it faster.

  **Design.** All content, on the gathering system (`PropDef.gather`, `regrow`, skill effect
  `Gather`):
  - Items: *Woodcutter's Axe* (`woodcutter_axe`, `tool: "axe"`, held sprite `held_axe`, also a
    weak weapon), *Log* (`log`, stackable, sells cheaply), *Plank* (`plank`). New icons on a new
    sheet `materials.png` (`items.png` is full).
  - Props: `big_oak` and `big_pine` get `gather` (tool `axe`, 5 hits, 2–4 logs, a small chance of
    an apple / a pine cone) with a `stump` state (new stump art; solid and low: walking goes
    around it, shots fly over it) and `regrow` from `stump` after 96 hours. New smaller
    choppable trees `oak_tree` and `pine_tree` (1×2 tiles) for dense woods, same rules.
  - Skill *Woodcutting* (`woodcutting`, effect `Gather { tool: "axe", percent: 20 }`, 3 levels),
    like *Mining*.
  - Recipe *Plank* (2 logs → 3 planks) at a new `sawhorse` prop (station `carpentry`), and planks
    used by a couple of recipes (a *Wooden Shield* recipe at the anvil, a *Fishing Rod*).
  - Felling sound (`chop`), chips in wood colours (gather chips take the prop's colour already).

  **Decided while building.** The station is called `sawhorse` (a recipe's station must be a prop
  id). A stump is not solid (a prop's `low` is per definition, not per state, and a stump you walk
  over reads fine). The woodcutter's axe is a tool only (tools go into the tool slot, not the weapon
  slot), no pine cone, no felling sound (gathering always plays the hit sound and fires no event
  when a prop is used up). The forest's trees are tiles: six of them by the path were swapped for
  the new young-tree props, which have the same 1 x 2 footprint. Merchants buy anything already, so
  the miller needed no change. Planks also make a woodcutter's axe at the anvil.

  **Engine.** None (content).

  **Example.** The axe is sold by the blacksmith and one lies in the mill; choppable oaks and pines
  around the village edge, the forest and the wilds; a sawhorse at the mill; the miller buys
  logs. German texts. Everything into the starter library.

  **Verify.** Autoplay: buy the axe, `click` a tree with the axe equipped → `gather=big_oak:1/5`
  … → logs in the inventory, `props=big_oak:stump@x,y`; `settime +96` → back to `default`;
  with Woodcutting 2 fewer hits; craft planks at the sawhorse (`craft 0`). Shots of the
  chopping and the stump.

- [x] Cooking on a wood fire — `feature/wood-fires`

  **Goal.** Cooking fires (campfire, hearth, bread oven, fireplace) are lit with logs (firewood)
  instead of coal, and burn down after a while; the forge and the smelter keep coal.

  **Design.** All content: the props' *Use item* triggers take `log` instead of `coal` ("You
  stack the logs and the fire crackles to life."); a lit cooking fire turns back to unlit after
  4 in-game hours (`regrow` from `lit`), so firewood is a real resource; cooking recipes keep
  `station_state: "lit"`. The brazier and street lamps keep their current rules. Dialogs and
  hints that mention coal for cooking are rewritten (and their German lines).

  **Decided while building.** Only the campfire burns down (four hours instead of the eight it had):
  a regrow from a prop's default state counts from the moment the player first sees it, so a
  burn-down time on the hearth, oven and fireplace (which start lit) would put the tavern's hearth
  out four hours into every game; the villagers tend those, logs only relight them. A log on a
  burning fire isn't used ("The fire is burning well already."); a cold campfire tells the player to
  use a log instead of opening the cooking window. The example had no campfire on any map: a cold
  one with three logs now stands by the forest path. The baker's oven needed no schedule (it
  starts lit and stays lit).

  **Engine.** None (content).

  **Example.** A cold campfire with logs by the forest path; the tavern hearth, the bakery oven and
  Mira's fireplace start lit as before.

  **Verify.** Autoplay: `press e` at an unlit campfire with a log → `props=campfire:lit@…`,
  one log fewer; coal no longer lights it; cook a trout; `settime +5` → unlit again; the forge
  still takes coal.

- [ ] Editor: undo in the Database — `feature/db-undo`

  **Goal.** Ctrl+Z / Ctrl+Y (and the Edit menu) undo and redo changes to definitions (items,
  props, characters, quests, skills, spells, recipes, achievements, sheets, tilesets, sounds,
  project settings), like map edits.

  **Design.** A second undo stack for the Database: a snapshot of the changed definition list
  (`Database` is cloneable) taken when a form edit starts (first change after focus, or a
  button such as *+ New* / *Delete* / rename), merged while the same field keeps changing (typing
  is one step); renames are one step including the script / map references they rewrote. The
  shortcut acts on what has focus: the map canvas → map undo, the Database → Database undo. The
  agent server's `upsert_def` / `delete_def` create steps too.

  **Engine.** Editor code (needs the go-ahead): `doc.rs`-like stack in `database.rs`, hooks in
  `app.rs` menus / shortcuts.

  **Verify.** Editor script: `mode db; tab items potion`, change a field (script command), `undo`,
  `status` shows the old value; rename an item → undo restores the id and the references; MCP
  `upsert_def` then `undo`.

- [ ] Editor: multi-select on maps — `feature/map-multiselect`

  **Goal.** Select several objects on a map at once: Shift+click adds or removes one, dragging a
  box on empty ground with the Select tool selects all inside; move them together (drag or arrow
  keys), delete, copy / cut / paste them, and edit the fields they share in the inspector.

  **Design.** `Editor.selection` becomes a list (one entry = today's behaviour). Moving and
  deleting several is one undo step (`Edit::Group`). Copy uses the existing region clipboard's
  entity part. The inspector shows the shared fields (active, facing, name prefix) for a
  multi-selection, a list with the selected objects otherwise. Zones join the selection too.

  **Engine.** Editor code (needs the go-ahead): `app.rs`, `canvas.rs`, `clipboard.rs`, `doc.rs`.

  **Verify.** Editor script: `select` with shift, `boxselect X Y W H` (new command), `move DX DY`,
  `delete`, `undo`; `status` prints `sel=N`; shots of a box selection.

- [ ] Gamepad glyphs and rebinding — `feature/pad-glyphs`

  **Goal.** Players with a gamepad (Steam Deck, Xbox, PlayStation, Switch controllers) see the
  buttons of their pad wherever the game names a key, and can rebind the pad's buttons like the
  keyboard's keys.

  **Design.**
  - **Last device**: the game remembers whether keyboard / mouse or a pad was used last
    (`Input`), and every hint follows it at once.
  - **Glyphs**: small button pictures (A B X Y, LB RB LT RT, Start, Select, sticks, d-pad) in
    three styles: Xbox (also the Steam Deck), PlayStation, Nintendo; picked from the pad's
    name / vendor (gilrs), or chosen in *Options › Controls › Button style* (auto / Xbox /
    PlayStation / Nintendo). Drawn from an engine sheet (the game font is Latin-1 only, so no
    ✕ ○ □ △ text); keys keep their text labels (`key_label`).
  - **Where**: the interaction prompt, menus' bottom hints, the Options key list, the
    character window, crafting / shop / companion windows, the orders menu, minigames, the
    players' first hints.
  - **In content**: a text placeholder `{key:ACTION}` (`{key:interact}`, `{key:attack}`) in
    dialogs, choices, toasts and quest texts, expanded by `WorldState::expand_text` to the
    current key or pad button, so tutorials say "Press E" or show the A button. Translations
    keep the placeholder.
  - **Rebinding**: *Options › Controls* gets a *Gamepad* page like the keyboard one: an action,
    then press the button to use; conflicts shown as for keys; *Defaults* resets. Saved in
    `settings.ron` (`pad: BTreeMap<String, Vec<String>>`, serde default = today's layout).
    Movement stays on the stick and d-pad; Escape / Start stays fixed.

  **Engine.** Proposed (needs the go-ahead): `input.rs` (last device, pad bindings like
  `set_bindings`), `settings.rs` (`pad`), `menu.rs` (the page, button style), a glyph sheet in
  the engine (`include_bytes!`, like the window icons), the `{key:…}` placeholder in
  `state.rs::expand_text`, the hint texts in the runtime windows, German texts.

  **Example.** The example's first hints (the guard's "press E to talk", the crafting and
  fishing tutorials) use `{key:…}`.

  **Verify.** Autoplay: `pad a` then `status` shows `device=pad`, a shot of the prompt with the A
  glyph; `press e` → `device=keys`, the prompt says E; a dialog with `{key:interact}` both ways;
  rebinding (`opt` commands on the Gamepad page, `pad x` now confirms); the setting survives a
  restart (`settings.ron`); an old `settings.ron` without `pad` loads. The Steam Deck itself:
  only if the user tests it (say so in the report).

- [ ] Gamepads in the web game — `feature/web-pads`

  **Goal.** The browser version of a game can be played with a gamepad too (today only the
  desktop builds read pads, through gilrs).

  **Design.** The web plugin (`web/rustpg2d.js`) reads the browser's Gamepad API
  (`navigator.getGamepads()`, the *standard* mapping) once per frame and hands buttons and stick
  axes to the game, which feeds them into the same `Pad` state gilrs fills on the desktop, so
  bindings, glyphs (by the pad's id string) and the autoplay `pad` commands work the same.

  **Engine.** Proposed (needs the go-ahead): a few functions in `rustpg2d.js` and `sys.rs` /
  `input.rs` (`#[cfg(target_arch = "wasm32")]`).

  **Verify.** Headless Brave can't connect a pad: unit tests of the mapping, and the autoplay
  `pad` commands in the web export; a real pad only if the user tests it in a browser (say so).

## 0.15.0 — Combat and emotions

- [ ] Enemy attacks — `feature/enemy-attacks`

  **Goal.** Armed enemies attack instead of hurting by touch: they stop, wind up (a visible
  tell), swing their weapon in an arc or shoot, and recover; the player can see an attack
  coming and step out of it.

  **Design.**
  - A character with a melee weapon (its definition's or carried equipment, `npc_items`) uses
    it: wind-up 0.4 s (weapon raised, a short flash), the swing (the player's swing drawing and
    reach / arc / knockback of the weapon), recovery (the weapon's cooldown). Damage = the
    weapon's plus the character's `damage`. Unarmed monsters (slimes, bats) keep contact damage,
    with a short lunge tell before it.
  - `CharacterDef.attack: Option<AttackStyle>` overrides it (wind-up, recovery, `Touch` /
    `Strike` / `Charge`), serde default = decided by the weapon as above.
  - Companions use the same attack code (they swing like the player today); enemies attack
    companions as well as the player.

  **Engine.** Proposed (needs the go-ahead): attack states in `ai.rs` / `combat.rs`, the
  `attack` field, drawing the swing for characters (`paperdoll.rs`).

  **Verify.** Autoplay: an outlaw with a sword approaches, `status enemies=` shows
  `outlaw:windup` then `:swing`; stepping back during the wind-up → no damage; standing still →
  damage; slimes still hurt by touch. Shots of the tell and the swing.

- [ ] Block and dodge — `feature/block-dodge`

  **Goal.** The player can defend: hold *Block* with a shield to stop frontal hits, or *Dodge*
  (a short roll) through an attack.

  **Design.** Block (held while a shield is in the off hand; mouse button, key and pad
  trigger chosen against the existing bindings at the start): walks at half speed, frontal melee
  hits and arrows do no damage (knockback only); a block right as the swing lands (0.2 s)
  staggers the attacker for a second. Dodge (a key while moving, pad B; both rebindable on the keyboard and Gamepad pages of 0.14.0): a 3-tile roll with 0.3 s
  of invulnerability, 1 s cooldown. Both work for gamepads and in the Options key list;
  shown in the HUD (shield raised, a dodge cooldown pip).

  **Engine.** Proposed (needs the go-ahead): `combat.rs`, `input.rs`, `paperdoll.rs` (block pose,
  roll), `menu.rs` (keys); the manual's `15-players-guide.md`.

  **Verify.** Autoplay: `status` prints `block=on|off dodge=ready|N.N`; a blocked swing leaves the
  HP unchanged; a timed block → `outlaw:stagger`; a dodge through a swing → no damage.

- [ ] Emotions — `feature/emotions`

  **Goal.** Characters, NPCs, enemies and the player show what they feel with a small icon over
  their head: surprise (!), confusion (?), anger, joy, love, embarrassment, sadness, fear,
  sleepiness (Zzz), thinking (…), an idea, music, nervous sweat, annoyance. Creators add their own
  emotions with their own art and use them from events and scripts.

  **Design.**
  - **Emotion definitions** (`defs/emotions.ron`, Database tab *Emotions*): id, name, sprite
    (a sheet frame or an animation tag of the sheet, `SheetDef.tags`), optional sound, default
    duration (seconds; 0 = until cleared), a pop-in style (*pop*, *bounce*, *float up*, *shake*).
    The starter set (the 14 above) is drawn as speech-bubble icons on a new sheet `emotions.png`
    (2 or 3 frames each, animated) and lives in the starter library like any asset, so a
    creator can replace the art or add their own.
  - **Showing one**: action *Show emotion* (`target`, `emotion`, `seconds` 0 = its default,
    `wait`: the event goes on when it ends) and *Clear emotion*; scripts `emote(target, emotion)`,
    `emote(target, emotion, #{ seconds, wait })` (queued), `clear_emote(target)`; the dialog
    line options `say(..., #{ emote: "joy" })` and an *Emotion* field on *Dialog* / *Choice*
    actions show it over the speaker while the line is up. `target` as everywhere: `self`,
    `player`, a placed character's name. One emotion per character; a new one replaces it.
  - **Drawing**: above the head (the body sheet's frame height), following the character as it
    walks, drawn over the map and under the HUD (also in cut-scenes, where it matters most);
    not saved (an emotion lasting "until cleared" ends with the map change).
  - **Automatic emotions** (project setting *Emotions › Automatic*, each one an emotion id or
    none): an enemy notices the player (`surprise`), loses them (`confused`), flees (`fear`);
    a villager hit by the player (`anger`); a sleeping character (`sleepy`, while lying in bed);
    a companion who learns a spell or levels up (`joy`). Defaults point at the starter set;
    projects without those ids simply show nothing.
  - **Moods and emotions**: independent (moods change the dialog portrait), but *Set mood* can
    also show the emotion of the same name when the project setting *Show mood as emotion* is on.

  **Engine.** Proposed (needs the go-ahead). Content can't do it: `effect()` plays a one-shot
  animation at a fixed spot, it doesn't follow a walking character, can't stay up and has no
  hooks for "an enemy noticed the player". The addition: the definition type (new defs file:
  `project.rs` `DEFS`, `release.rs`, `library.rs` kind *emotion* + `refs_of_*`, renames),
  `Action::ShowEmotion` / `Action::ClearEmotion` appended, the `emote` field on *Dialog* /
  *Choice* (serde default), script functions (`script.rs`, `script_api.rs` with a new id kind
  *Emotion* for completion and renames), a new `runtime/emotions.rs` (state, drawing, the
  automatic hooks in `ai.rs`, `combat.rs`, `furniture.rs`, `companion_magic.rs`), the editor's
  Database tab with a live preview and the action fields, the agent server (`upsert_def` kind
  `emotions`).

  **Example.** The guard shows *surprise* when the player first arrives, Tilda *joy* when greeting,
  Pip *embarrassment* when thanking the player after the cure, the elder *thinking* before he
  explains the quest, outlaws *anger* when the fight starts (automatic *surprise* otherwise),
  sleeping villagers *Zzz* at night. German names for the emotions in `lang/de.ron`.

  **Verify.** Unit tests: an old Dialog action without the field loads; emotion lookup and
  rename. Autoplay: `status` prints `emotes=name:emotion(seconds left)@x,y`; `talkto tilda` →
  `tilda:joy`; an outlaw notices the player → `outlaw:surprise`; a scripted `emote("player",
  "idea", #{ wait: true })` blocks until it ends; at 23:00 a villager in bed shows `sleepy`.
  Shots of each starter emotion over a character (a test script cycling them), and the
  Database tab with the preview. A custom emotion added through the agent server
  (`write_asset` + `upsert_def`) shows up in play.

- [ ] Smarter enemies — `feature/combat-ai`

  **Goal.** Groups fight like groups: they spread around the player instead of stacking, take
  turns, call their friends, archers look for a clear shot, and the hurt ones fall back.

  **Design.**
  - Attack slots: at most 2 melee enemies attack the player (or a companion) at the same time;
    the others circle at 2–3 tiles and step in when a slot frees.
  - Separation: chasers steer away from each other (no stacking on one tile).
  - Alert: an enemy that notices the player alerts others of its kind (same definition, or the
    same spawner, later the same faction: 0.17) within 6 tiles; they show the automatic
    *surprise* emotion (previous item) as they turn towards the player.
  - Archers / casters: when `line_clear` fails (walls; low obstacles don't count) they move
    sideways to find a line; they lead a moving target a little.
  - Fleeing enemies run towards allies when there are any, and come back when healed.
  - All of it on by default for `Chase` (new fields of `Behavior::Chase` with serde defaults:
    `group: bool = true`, `alert: f32 = 6.0`); old projects get the smarter enemies too.

  **Engine.** Proposed (needs the go-ahead): `ai.rs` (slots, separation, alerting, line search),
  `combat.rs::enemy_shooting`.

  **Verify.** Autoplay with the forest's outlaw group: `status enemies=` shows states
  (`chase`, `circle`, `attack`, `flee`) and at most two `attack`; an archer behind a wall
  changes position until it shoots; `astar=` stays low (performance). Shots of a surround.

- [ ] Combat feel — `feature/combat-feel`

  **Goal.** Hits feel like hits.

  **Design.** Hit-stop (40 ms freeze on a landed hit), enemies flash white when hit, small
  damage numbers (Options toggle, on by default), health bars over enemies that were hurt
  (fade after 3 s), a heavier knockback curve, critical hits from the weapon skill (2× damage,
  bigger number). Everything respects *Shake & flash* off.

  **Engine.** Proposed (needs the go-ahead): `juice.rs`, `combat.rs`, `menu.rs` (option).

  **Verify.** Autoplay: `status` prints `hitstop=` and the last damage numbers; shots of a hit.

- [ ] Editor: quick search and usages — `feature/quick-search`

  **Goal.** Ctrl+P opens a search box that finds anything in the project: maps, definitions of
  every kind, script functions, named entities, zones, quests, texts; Enter jumps there. *Find
  usages* (context menu of an id, and in every Database form) lists where it is used, each
  entry clickable.

  **Design.** An index built from the project (refreshed on save), fuzzy matching, kind icons;
  usages from `refactor.rs` (`refs_of_*`, the same code that renames ids) plus the scripts
  (`script_refs.rs`).

  **Engine.** Editor code (needs the go-ahead): a new `search.rs`, hooks in `app.rs`.

  **Verify.** Editor script: `search potion` prints the hits, `searchpick 0` jumps (`status`
  shows the open tab / map and selection), `usages item potion` lists maps, shops, loot,
  scripts.

## 0.16.0 — A new look

- [ ] High-resolution sprites — `feature/hires-sprites`

  **Goal.** Sprite sheets can carry twice the detail: a sheet drawn at half its pixel size, so
  64×96 art fills the same 32×48 place as today's characters. Old 1× sheets keep working and
  mix with 2× ones (a 2× body with a 1× hat lines up).

  **Design.** `SheetDef.scale: u32` (serde default 1; 2 = two image pixels per world pixel).
  Every draw of a sheet (`render/mod.rs` draw functions, paperdoll layers, props, items, icons,
  effects, fly sprites, portraits, the editor's pickers and canvas, library thumbnails) divides
  the frame size by it; hitboxes, positions and tile sizes stay in world pixels. The game
  already draws the world at a whole-number zoom (`view_scale`, 2× or more), so 2× art shows
  its full detail; at zoom 1 it is sampled down. The pixel editor edits at the image's own
  resolution. Import (LPC, Aseprite) can set the scale.

  **Engine.** Proposed (needs the go-ahead): the field, the drawing, the editor views.

  **Verify.** Unit tests (draw size per scale); autoplay shots of a 2× test body next to 1×
  characters at zoom 2 and 3; editor shots of the sheet picker and the canvas; the web export.

- [ ] Character art in a new style — `feature/character-art`

  **Goal.** The characters look clearly better: new proportions, shading and animation at 2×
  resolution (64×96 frames) for every layer.

  **Design.** First a style sheet, agreed with the user from samples before the full set:
  palette, outline (dark, coloured, not black), light from the top left, 3.5 heads tall,
  4-frame walk + idle breathing (2 frames) + attack and hurt frames (used by 0.15's attacks).
  Then the full set, painted with the same skin (magenta) and hair (cyan) reference ramps so
  `recolor.rs` keeps working: bodies (human ♂/♀, goblin ♂/♀), hair, beards, every outfit
  (with its female cut), armor overlays, held items (with hand positions for the new frames),
  children. The old 1× sheets stay in the library for existing projects (new ids or a
  `_classic` suffix for the old ones; decided at the start, so existing projects don't change
  their look by surprise on a library update).

  **Engine.** None beyond the previous item (content), unless the extra animation rows need a
  sheet layout field: then propose it.

  **Verify.** Shots of every layer combination in the character builder and in play (walk,
  attack, sit, lie), the creator's previews, portraits; the starter-library test.

- [ ] Monsters and animals in the new style — `feature/creature-art`

  **Goal.** The slime, the village animals (chicken, dog, cats, sheep, cow) and the children in
  the new style at 2×, with attack and hurt frames for 0.15's combat; two new monsters for the
  wilds (a wolf, a bat) to give the combat AI more to work with.

  **Engine.** None (content).

  **Verify.** Shots in play and in the bestiary.

- [ ] Editor: project problems panel — `feature/problems-panel`

  **Goal.** One list of everything that is broken in the project: unknown ids in maps,
  definitions and scripts, portals to missing spawns, unreachable maps, script errors,
  missing translations, missing images or sounds, characters without a body. Each entry jumps
  to the place; the list refreshes on save.

  **Design.** Collects the checks that exist today (map validation on load, the script check
  with its unknown ids, the translation template's missing lines, `refs_of_*` for unknown ids)
  into one model the editor shows (a panel with filters: errors / warnings, per kind) and a new
  agent server tool `problems` returns.

  **Engine.** Editor code (needs the go-ahead): a new `problems.rs`.

  **Verify.** Editor script on a scratch copy with planted problems: `problems` prints them,
  `problempick K` jumps; shots.

## 0.17.0 — Peoples of the world

- [ ] Races — `feature/races`

  **Goal.** Races are a definition type: humans, goblins, elves, dwarves, orcs and trolls, each
  with its own bodies, looks, strengths and a faction. The player picks one in the character
  creator; NPCs and enemies belong to one.

  **Design.** `defs/races.ron`, `RaceDef`: id, name, description; bodies (male / female sheet;
  one for races without a choice); allowed hair and beards and the skin / hair colour ranges the
  creator offers; stat changes (max HP, speed in %, defense); starting skill levels (dwarves
  Mining 1 and Smithing 1, elves Archery 1, orcs Swordsmanship 1, trolls Toughness 2, goblins
  Scavenging 1); size (trolls 1.5× with bigger hitbox); a default faction; `playable`.
  `CharacterDef.race` (serde default "" = no race, the body decides as today);
  `WorldState.player_race`. Outfits and armor fit other body shapes by a *cut* like the female
  one: `SheetDef.cut` on bodies (`"dwarf"`, `"troll"`), and an overlay looks for
  `<overlay>_<cut>` (and `_<cut>_f`) before the plain one, so one item serves every race whose
  cut exists. Script / conditions: `player_race()`, a *Player race* condition.

  **Engine.** Proposed (needs the go-ahead): the definition type (+ Database tab, refs, library,
  release export), the creator page, the cut lookup, the condition and script function.

  **Verify.** Unit tests (defaults, an old save without a race, overlay lookup by cut). Autoplay:
  the creator with each race (`status creator=`), stats and skills applied, a dwarf wearing the
  tunic's dwarf cut; shots of all races side by side.

- [ ] Factions — `feature/factions`

  **Goal.** Who is hostile to whom follows from factions: orcs attack elves and humans on sight,
  wolves attack everyone, guards defend the village; the player's race and deeds change how
  factions treat them.

  **Design.** Project setting *Factions* (names) with an attitude table (hostile / neutral /
  friendly) between factions, and the player's standing per faction (`WorldState`, starting
  from their race's faction, changed by the *Change standing* action / `change_standing()`).
  A character is hostile to the player when its faction is hostile to the player's standing
  (`hostile: true` stays as an override for simple projects). Characters of hostile factions
  fight each other with 0.15's AI (attack slots, alerting); guards join fights near them.
  Condition *Faction standing*.

  **Engine.** Proposed (needs the go-ahead): faction settings and table, standing in the save,
  NPC-vs-NPC targeting in `ai.rs` (the companions' targeting generalised), action + condition +
  script functions.

  **Verify.** Autoplay: an orc patrol meets an elf traveller → `status enemies=` shows them
  fighting; the player as an orc walks past an orc camp unharmed; standing changes after a
  quest.

- [ ] Elves, dwarves, orcs and trolls — `feature/race-art`

  **Goal.** The new races in the example and the starter library, in the 2× style.

  **Design.** Bodies (♂/♀ for elves, dwarves, orcs; one troll body), their outfit / armor cuts
  where the human ones don't fit (dwarves, trolls), race hair and beards (dwarf braids, elf
  hair), portraits.

  **Engine.** None (content).

  **Example.** A dwarf merchant at the mine (ores and tools), an elven herbalist travelling the
  wilds (schedule), an orc camp in the wilds (enemies, a chief, loot), a troll guarding a bridge
  in the wilds (a mini-boss with *HP below* triggers), German texts; races and art to the starter
  library.

  **Verify.** Autoplay through each new place; shots; the starter-library test.

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented). Packaging work: needs the user's go-ahead.
