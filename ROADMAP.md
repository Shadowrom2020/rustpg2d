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

## 0.21.0 — Weapons in Hand

Fighting should look like the weapon in the hand: a sword slashes, an axe comes down overhead, a
spear thrusts, a bow draws and looses, a crossbow aims and fires, a shield comes up to block, and a
character standing ready holds its weapon like one. Each item says which animations it uses; each
model maps those animation names to its clips. Each *Engine* line is a proposal: agree it with the
maintainer before changing Rust code (CLAUDE.md, *Content first*).

- [x] Weapon animations — `feature/weapon-anims`

  **Goal.** Attack, block and idle animations follow the equipped weapon (and the shield in the
  left hand) instead of one attack clip per model for every weapon. A creator picks the animations
  on the item; models carry clips for the standard names; characters without a fitting clip keep
  today's behaviour.

  **Design.**
  - *Animation names, not clips, on items.* `ItemDef` gets `anims` (serde default, left out of the
    file when empty): `attack`, `idle`, `block` (and `walk`, optional), each an animation name.
    A model maps names to its clips as it already does (`ModelDef.clips`: `anim` → `clip`, like
    `sit`, `fish`, `sneak`), so one item works with every model that has those names, and each
    model keeps its own retargeted clips.
  - *A standard vocabulary* the editor offers (free names stay possible): `attack_1h` (sword, mace:
    a one-handed slash), `attack_2h` (axe, hammer, two-handed sword: overhead), `attack_thrust`
    (spear, dagger), `attack_bow`, `attack_crossbow`, `attack_cast` (staff, wand; spells),
    `attack_unarmed` (fists); `idle_1h`, `idle_2h`, `idle_bow`, `idle_spear`; `block_shield`,
    `block_weapon` (a parry with the weapon), `block_2h`.
  - *Which animation plays* (`paperdoll.rs`, `attacks.rs`, `defense.rs`): attacking, the weapon's
    `attack`, else the model's `attack`; bare hands `attack_unarmed`, else `attack`. Standing, the
    weapon's `idle`, else the left-hand item's (a shield's), else `idle`; walking, the weapon's
    `walk` if it has one, else `walk`. Blocking (the player's raised shield), the shield's `block`,
    else the weapon's, else today's pose. Every step falls back to what the model has, so models
    without the new clips look as they do now. NPCs use their own equipment the same way.
  - *Timing.* Clips land their blow at different moments. `ClipMap` gets an optional `hit` (0..1,
    where in the clip the blow lands; default 0.42 = today's swing point), and the attack plays
    through the clip continuously, wind-up up to `hit` while the enemy winds up and the swing after
    it, instead of today's four fixed points (`anim_req`). Ranged attacks release the shot at `hit`.
  - *Defaults from the weapon* (the item form's *Estimate*, and items without `anims`): ranged →
    `attack_bow` (an id or name containing "crossbow" → `attack_crossbow`); two-handed with a reach
    over 1.4 tiles → `attack_thrust`; two-handed → `attack_2h`; other melee → `attack_1h`; a shield
    (left-hand item held with *Strap*) → `block_shield`. An item without `anims` uses these at run
    time, so existing projects get the new animations as soon as their models have the clips.
  - *Getting the clips.* Meshy's animation library has them (catalogue: `GET
    /openapi/v1/animations/library`, 678 actions): one-handed *Right-hand Sword Slash* (219),
    *Left Slash* (97), *Charged Slash* (242); two-handed *Heavy Hammer Swing* (128), *Charged Axe
    Chop* (237); *Thrust Slash* (240); *Archery Shot* (224–227); spell casts (125–137); blocks
    *Block1–10* (138–146), *Sword Parry* (147), *Two-handed Parry* (149); punches (191–214); idles
    *Combat Idle* (89), *Axe Stance* (85), *Crouch Look Around Bow* (258); *Spear Walk* (560). No
    crossbow clip: an archery or *Run and Shoot* (98) clip held level, or a hold pose. Final picks
    after looking at the previews (`preview_url` GIFs) with the user.
  - *Clips without buying them per model.* All Meshy humanoid rigs share the same bone names, so a
    clip bought once on a reference model can be retargeted onto every other model: rotation keys
    times inverse(source rest) times target rest, the target's bone offsets (the method that fitted
    the sneak clips in 0.20.0). The editor gets *Copy clips from another model…* (Database › 3D
    models, a script command and an agent tool) that writes the retargeted clip file and maps the
    names. Body types far from the reference (troll, goblins, chibi vs realistic) are checked in
    renders and get their own purchase when the copy looks wrong (re-rig from the uploaded model,
    5 credits, then 3 per clip). *Generate art* gains *Add clips to a model* (library actions by
    name, re-rigging when the model's rig task expired) so this needs no hand-written scripts.

  **Engine.** Agreed (all of it, Generate art included): `ItemDef.anims` (`ItemAnims`), `ClipMap.hit`, the
  animation choice by equipment and the continuous attack playback in the runtime (`paperdoll.rs`,
  `attacks.rs`, `combat.rs`, `defense.rs`, `render/model.rs` `anim_req`), clip retargeting in
  `model.rs` (a function shared by the editor's copy tool and the art pipeline), the editor's item
  form section and model tools, `status` `anims=` naming the clip that plays.

  **Decided while building.**
  - *The player's blows and shots stay instant* (damage on the key press, as before): the strike
    (`Anim::Strike`, `PlayerCombat.strike` for the weapon's cooldown, 0.3 to 0.6 s) hurries through
    the wind-up and lands the clip's hit point a quarter into it (`render/model.rs` `strike_time`),
    then plays the rest. Enemies wind up to the hit point (`Anim::Windup`) and swing from it
    (`Anim::Swing`, with a short follow-through); a raised shield plays the block clip up to its hit
    point in 0.2 s and holds it (`Anim::Block`). Blocking without a block clip shows idle.
  - *Copying clips writes no new files*: `ModelDef.shared_clips` lists another model's clip files,
    fitted to this model's bones when it loads (`Model::add_clips_retargeted`); a clip named like
    one the model has is left out, so its own come first. *Copy clips* (form), `modelshareclips`
    (script) and `share_model_clips` (agent tool) add the files and map the names
    (`Editor::share_clips`); animations a model maps already are kept unless replaced.
  - *Shields* are estimated from armor in the left hand with defense > 0 (lanterns and torches have
    none), not from the *Strap* hold style.
  - *A held item never points back into the forearm*: a clip's fist rolled over in a thrust made the
    spear point at the elbow; the grip axis is flipped then (`out_of_fist`).
  - *Clips bought*: ten on the hero (30 credits), nine kept: `attack_2h` (Heavy Hammer Swing, hit 0.84),
    `attack_thrust` (Thrust Slash, 0.29), `attack_bow` (Archery Shot, 0.6), `attack_cast` (0.55),
    `attack_unarmed` (Right Jab from Guard, 0.33), `idle_1h` (Combat Stance), `idle_2h` (Axe Stance),
    `block_shield` (Block1, 0.15),
    `block_weapon` (Sword Parry, 0.3). `idle_bow` (Crouch Look Around Bow) was dropped: a crouched
    watch made archers look as if they sneaked, so bows stand in the plain idle. `attack_1h`,
    `attack_crossbow`, `idle_spear` and `block_2h` weren't bought: models play their plain clips there (their own slash). Every other person of the
    example (34 models) and the starter library (16 more, the realistic figures included) shares
    them; renders of the heroine, elf, dwarf, goblin, troll, guard, villager and mannequin looked
    right, so no body type got its own purchase.
  - *The example's items keep no `anims`*: the estimates already give the spear its thrust, the bows
    their draw and the shield its block; the battle axe is one-handed in the example (its slash).
  - *Generate art › 3D model clips* (`ArtKind::Clips3d`, agent kind `clips`): always re-rigs the
    model's file (`model_url`, 5 credits) and adds the clips as a shared clip file, mapped by
    `CLIP_CHOICES` (weapon names and `sneak`; `craft`, `farm`, `chop`, `sleep` too).
  - *Problems* warns about an item naming an animation that no model maps, and about a model's
    animation whose clip none of its files has (also missing model and clip files).

  **Touch points.** `defs.rs` (`ItemDef`, `ClipMap`, `Default` / `fresh`), `render/model.rs`
  (`anim_req`, `PoseReq`), `render/character.rs` (`Anim`), `runtime/paperdoll.rs`
  (`player_anim`), `runtime/attacks.rs` (NPC wind-up, swing, recover), `runtime/combat.rs` (the
  player's swing and shot timing), `runtime/defense.rs` (blocking), `model.rs` (retarget), editor
  `database.rs` (item form *Animations*: name pickers, *Estimate*, the 3D preview playing them;
  model form clip names), `models_tab.rs` / `art/` (copy clips, add clips), `problems.rs` (an item
  naming an animation that a model wielding it lacks), agent tools, `refactor.rs` (none: names are
  not ids), manual 05 (items), 06 (models), 09 (combat), 13 (tools), CHANGELOG.

  **Example.** The example's weapons get their animations: swords → `attack_1h` / `idle_1h`;
  axes, hammer → `attack_2h` / `idle_2h`; spear → `attack_thrust` / `idle_spear`; bows → `attack_bow`
  / `idle_bow`; crossbow → `attack_crossbow`; the wooden shield → `block_shield`; bare hands →
  `attack_unarmed`. Clips for the playable models and the armed NPCs (guards, outlaws, bandit chief,
  orcs, goblins, troll): bought once on a reference model each body type needs, copied to the rest;
  the credits asked before each purchase. Starter library too.

  **Verify.** Model sheets of a model with each attack clip; in the game the player with sword,
  axe, spear, bow, crossbow and bare hands (shots of the swing at wind-up, blow and recovery;
  `anims=` names the clip), blocking with the shield, standing with each weapon; an NPC guard's
  spear thrust and an outlaw's slash timed with their wind-up (the red zone); a model without the
  new clips still attacks as before; a copied clip on a body far from the reference (troll); the
  item form (UI test: pick an animation, *Estimate*, save); old item files load (no `anims`).

- [x] Backpacks — `feature/backpacks`

  **Goal.** A *Back* equipment slot for a backpack that shows on the character's model and gives the
  player more inventory slots while worn: the example's small backpack +6, big +12, magic +24.

  **Design.**
  - *Slot and item.* `EquipSlot::Back` (appended) and `Equipment.back`; an item worn there is armor
    with `slot: Back` and `ItemDef.bag_slots` (serde default 0) = the extra slots.
  - *Inventory size* = `ProjectConfig.inventory_size` + the worn backpack's `bag_slots`, set on
    equip, unequip and load. Taking a backpack off (or swapping it for a smaller one) packs the
    items of the slots that go into free slots of the rest; when they don't fit it stays on
    ("Make room first").
  - *On the model.* `HoldStyle::Back`: the item's 3D model sits on the upper-back bone (`Spine02`,
    else the spine bone nearest the shoulders), against the back of the torso, turned with it.
    Characters of any kind can wear one (NPC equipment too).
  - *Example.* Three backpacks modelled in Blender (no Meshy credits) with rendered icons: the
    small and big ones in the village merchant's shop, the magic one in the bandit chief's loot.

  **Engine.** Agreed: the slot, `bag_slots`, the inventory size, the Back slot in the character
  window, `equip` / `unequip` "back", `HoldStyle::Back` in `render/model.rs`, the editor's item form.

  **Touch points.** `defs.rs` (`EquipSlot`, `ItemDef.bag_slots`, `HoldStyle`), `state.rs`
  (`Equipment`), `runtime/equipment.rs`, `inventory_ui.rs` (slot), `runtime/mod.rs` (size on load),
  `render/model.rs` (back placement), editor `database.rs` (item form), `builder.rs` (equipment),
  manual 05, 09, 15, CHANGELOG.

  **Verify.** Equip each backpack: the inventory grows by its slots, the model wears it in all
  facings and while walking and fighting; taking it off with full extra slots refuses, with room
  packs; save and load keep it; an NPC wearing one; the item form (UI test); old saves load.

- [x] Photo mode — `feature/photo-mode`

  **Goal.** Freeze the game, move the camera freely, no HUD, save a picture (from the backlog).

  **Design.** P (`Act::Photo`, rebindable) or the pause menu's *Photo mode* (gamepads) enters it
  (not on the title screen, in cut-scenes, dialogs or windows). Nothing updates; the camera moves
  (keys / stick / mouse drag), zooms 0.5× to 3× (wheel, + / −, LB / RB); H / Y hides the player,
  [ / ] / LT / RT show another hour's light (the clock comes back on leaving). E / Enter / Space / A
  grabs the world drawn so far (no hint bar) as a PNG; Esc / P / B leaves.

  **Decided while building.** Pictures go to the Pictures folder (`paths::pictures`: Linux reads
  `XDG_PICTURES_DIR` / `user-dirs.dirs`, also the home's inside a Flatpak) under the game's name,
  else a `photos` folder beside the saves; `RUSTPG2D_PHOTOS_DIR` overrides (tests). Exported
  Flatpaks get `--filesystem=xdg-pictures:create`. The web game downloads the picture
  (`rustpg2d_download` in web/rustpg2d.js, plugin version 3). Animated tiles and breathing go on
  (they follow the clock of the frame); characters, weather, effects and the game clock stand still.

  **Engine.** Agreed: `runtime/photo.rs`, `Act::Photo`, the camera and drawing hooks, the pause menu
  row, `sys::give_player`, `paths::pictures`, the web plugin's download.

  **Verify.** Scripted: P, move, zoom, hour, hide, picture (file checked: no hint, player hidden),
  leave (clock back), pause menu entry, pad B, the world frozen (clock and characters), German
  hint bar, unwritable Pictures folder (fallback), the web game in headless Brave (download call).

- [x] Accessibility — `feature/accessibility`

  **Goal.** Bigger text, captions for sound effects, a high-contrast HUD, hold instead of repeated
  presses (from the backlog).

  **Design.** Options › *Accessibility…*, a page of its own (`Settings`: `text_size`, `captions`,
  `high_contrast`, `hold_repeat`, off in older files). Text size scales what players read (dialog
  box and choices, toasts, speech bubbles, captions, the item details footer, which grows with it:
  `ui::detail_h`); the windows' fixed layouts stay. Captions: `SoundDef.caption` (translated with the
  content), `Audio` notes every sound started (`take_heard`), `runtime/captions.rs` shows them with
  the side for world sounds (`Game::sound_from`: hits, swings, defeats, locks, gathering, *Play
  sound* of an entity) and *Music: …* for a new track; at the top right while a window is open. High
  contrast: `ui::contrast()` in `panel()`, toasts, the dialog's choices, the HUD frame, clock, quick
  bar, speech bubbles, damage numbers. Hold to repeat: Attack held (weapon cooldown paces it), E held
  on a gatherable prop every 0.45 s, and so does the right mouse button held after a right-click on one
  (`mouse_hold_repeat`: the clicked target, once the player stands there); never in dialogs or windows.

  **Engine.** Agreed: the settings and their page, the text scale and contrast in the UI code, the
  caption field and `captions.rs`, hold to repeat in `combat.rs` / `update_player` / `mouse.rs`.

  **Verify.** Scripted: all four on (settings file), Larger text in a dialog, a speech bubble, the
  details footer (the character window still fits 720p), captions for swings (x6), music and hits,
  holding X (five swings in 2 s), holding E at an oak (stump, a log), the Accessibility page, the
  German texts; older settings files load.

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
