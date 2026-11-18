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

The order weighs how many creators each step unlocks against its size:

1. **Text and looks:** whether a game can be made for a market at all.
2. **Commands and plugins:** the event commands and plugins creators expect.
3. **The SRD 5.1 ruleset:** the rules of the tabletop's open reference document, with turn-based
   combat, classes and spells, instead of yet another JRPG maker (the user's decision, 2026-11-16).
4. **Reach:** platforms, editor comfort and a way in for new creators.
5. **Content:** one content release.
6. **1.0.0:** the formats freeze.

Each version has a theme, and items may move between versions.

Almost every item below changes the engine. The user agreed to the direction with this roadmap,
and each item's *Engine* line names the addition. When an item starts, confirm its details with
the user in a short plan before changing Rust code (CLAUDE.md, *Content first*). Build whatever
can be content as content: the library and example parts of each item.

## 0.27.0 — Plugins

Systems become add-ons: scripts can change the engine's outcomes, and plugins package them with
their settings.

- [x] Script hooks — `feature/script-hooks`

  **Goal.** Scripts change what the engine does: damage, healing, item use, levels, prices,
  saving.

  **Design.**
  - **Registration:** the project declares hooks in `project.ron` `hooks`; a plugin declares them
    in its `plugin.ron` (hook name, function, priority).
  - **Calls:** hooks run in priority order. Each gets a map (`#{attacker, target, amount, element,
    …}`) and returns a changed map, or `()` to keep it.
  - **Hooks:** `damage`, `heal`, `item_use`, `xp`, `level_up`, `buy`, `sell`, `craft`,
    `map_enter`, `new_game`, `save`, `load`, `menu_open`, `effect_apply`, and with the SRD rules
    (0.28 to 0.30) `roll`, `ability_check`, `saving_throw`, `attack_roll`, `initiative`,
    `turn_start` / `turn_end`, `combat_start` / `combat_end`, `rest` and `spell_cast`.
  - **Cost:** only registered hooks are called, under a Rhai operation limit.

  **Engine.** `Script::call_hook` returning a value (today's `call` / `call_args` return actions),
  the call sites, `08-scripting.md`.

  **Decided while building.**
  - Registration is `ProjectConfig.hooks` ({hook, function, priority}) edited in *Database ›
    Project › Script hooks*; plugins add theirs in the next item. Lower priorities run first.
  - The maps: `damage` {attacker ("player" or ""), target, target_def, amount (after defense), crit},
    `heal` {target, amount}, `item_use` {item, cancel}, `xp` {amount}, `level_up` {level, hp,
    mana, points}, `buy` / `sell` {item, price, merchant} (-1 = not interested), `craft` {recipe,
    count, cancel}, `map_enter` {map, from}, `new_game` {}, `save` / `load` {slot}, `menu_open`
    {menu, cancel}, `effect_apply` {target, effect, strength, seconds, cancel}. No `element`
    (the engine has no elements yet). Poison ticks aren't hits.
  - Hooks may change the world and queue actions; the queued actions run after the current ones.
    The operation limit per hook call is 200 000 (2 million for ordinary calls).
  - Shop prices go through the hooks when the window opens and after each trade (a cache), since
    they are drawn every frame.
  - The SRD hooks come with their rules (0.28 to 0.30).
  - Example: friends (five hearts) sell 10 % cheaper and pay 10 % more (`buy` / `sell`).

  **Verify (done).** Autoplay on a scratch copy: a damage-doubling hook (hit numbers 4 instead of
  2, Rook 10 → 8 from a 1-point area), a hook keeping the player unhurt, the journal kept shut,
  XP × 10, poison lengthened to 30 s, the friends' prices (bread 6 → 5 at Jorun's); ten damage
  hooks in a slime fight: `update_ms` 0.1 as without hooks (60 hook calls).

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

## The SRD 5.1 ruleset (0.28.0 to 0.31.0)

The user's decision (2026-11-16): R(ust)PG2D's rules follow the **System Reference Document 5.1**
(SRD 5.1) by Wizards of the Coast, as closely as the engine allows, instead of growing into another
JRPG maker. Things that apply to all four releases:

- **The ruleset replaces the engine's own.** Real-time combat, the skill tree, mana, the
  progression curves, race stat changes and status effects go. The SRD's ability scores, skills,
  proficiency, spell slots, conditions and turn-based combat come instead.
  - Exploration stays real-time; a fight switches to turn-based combat.
  - Systems the SRD doesn't cover stay and use SRD checks: crafting, gathering, farming,
    fishing, lockpicking, sneaking, housing, factions, friendship, schedules. For example, crafting
    becomes a tool check, lockpicking a Dexterity (Thieves' Tools) check, sneaking Dexterity
    (Stealth) against passive Perception.
- **Data:**
  - **Source:** the SRD data comes from [5e-bits/5e-database](https://github.com/5e-bits/5e-database)
    (`src/2014/en/5e-SRD-*.json`, the data behind the 5e-srd-api; MIT for the code). The
    [5e-srd-api](https://github.com/5e-bits/5e-srd-api) is the reference for how the parts
    connect.
  - **Import tool:** `xtask srd-import` (or an editor example) turns the JSON into definitions for
    the starter library, so the data stays reproducible. It is not hand-typed.
  - **Pinned:** the data is pinned to a commit; updating it is a deliberate step with a diff.
- **Licence:** the SRD 5.1 text and data are used under **CC BY 4.0**.
  - Every game that uses SRD content carries the attribution: "This work includes material taken
    from the System Reference Document 5.1 ("SRD 5.1") by Wizards of the Coast LLC and available
    at https://dnd.wizards.com/resources/systems-reference-document. The SRD 5.1 is licensed under
    the Creative Commons Attribution 4.0 International License available at
    https://creativecommons.org/licenses/by/4.0/legalcode."
  - **Where:** a project setting *Uses SRD 5.1 content* (on in the example and for projects made
    from the starter library) makes exports write it as `SRD-5.1-attribution.txt` and show it on
    the title screen's credits.
  - **Names:** R(ust)PG2D says "uses the SRD 5.1 ruleset". It never says Dungeons & Dragons or
    D&D, which are trademarks and not part of the SRD, and it uses no non-SRD monsters or settings.
- **Tiles and feet:** one tile (32 px) is a 5-foot square. Speeds, ranges and areas convert at
  that scale (speed 30 ft = 6 tiles per turn, a 20-ft radius = 4 tiles).
- **Compatibility:** these releases break compatibility, which the user agreed to.
  - The editor converts what it can on open, with a backup. Characters get stat blocks from their
    old values (HP to hit dice, damage to an attack, defense to AC), races map to SRD races,
    items keep their ids and become SRD equipment where one fits.
  - Saved games of earlier versions don't load: a *Breaking:* line in each release.
  - Script functions of the old systems (`skill`, `mana`, `learn_spell`, …) are removed or remapped,
    as listed in `08-scripting.md`.
- **The example:** Emberbrook moves to the rules release by release.
  - Villagers become SRD commoners and guards; its enemies become SRD monsters (wolves, bandits,
    skeletons, a ghost, the mine's boss).
  - The hero creates a level-1 character, and its quests give XP by the SRD table.
  - The example's own spells (Heal, Light, Spark, Fireball) become Cure Wounds, Light, Fire Bolt
    and Fireball.

## 0.28.0 — The Rules

The core of the SRD: dice, abilities, checks, races, conditions, rests and the character
sheet.

- [ ] Dice, abilities and checks — `feature/srd-core`

  **Goal.** Every roll in the game is an SRD roll.

  **Design.**
  - **Dice:** dice expressions (`1d20+5`, `2d6+STR`, `8d6`) are rolled by one module with a
    seedable random source (autoplay tests can fix the seed). Advantage and disadvantage roll two
    d20s; a natural 20 and a natural 1 are reported.
  - **Abilities:** the six scores (STR, DEX, CON, INT, WIS, CHA) with their modifiers, the
    proficiency bonus by level, and the 18 SRD skills with their abilities. Saving throws and
    passive scores.
  - **Events:** conditions *Ability check* and *Saving throw* (ability or skill, DC, advantage),
    whose result is remembered for the next actions (`check_passed`), and an action *Roll*.
  - **Scripts:** `roll("2d6")`, `check("stealth", 15)`, `save("dex", 14)`, `modifier("str")`,
    `proficiency()`.
  - **Roll log:** a log in the journal shows every roll with its dice, modifiers and result,
    and the HUD shows a short line for checks (`Perception 14 vs DC 12: success`).
  - **Replaces:** the engine's skill tree and skill points (`skills.rs` skill effects), the
    *Skill level* condition and `skill()` (remapped to proficiency).

  **Engine.** A new `srd/` module (dice, abilities, skills, checks), conditions and actions
  (appended), script functions, the journal's roll log, the conversion of old skills.

  **Example.**
  - Lockpicking becomes a DEX (Thieves' Tools) check.
  - The shrine's riddle becomes an INT (Religion) check.
  - Tilda's haggling becomes CHA (Persuasion).

  **Verify.** Unit tests: dice parser and statistics, modifiers, proficiency by level. Autoplay with
  a fixed seed: a check passes and one fails (`status` prints `rolls=`), and the roll log appears in
  screenshots.

- [ ] Races and the character creator — `feature/srd-races`

  **Goal.** The player creates an SRD character: race, ability scores, background, skills.

  **Design.**
  - **Races:** the nine SRD races with their SRD subraces: Dwarf (Hill), Elf (High), Halfling
    (Lightfoot), Human, Dragonborn, Gnome (Rock), Half-Elf, Half-Orc, Tiefling. Each has its
    traits: ability bonuses, speed, size, darkvision, resistances, languages, proficiencies,
    racial spells. `RaceDef` becomes the SRD race with its bodies per sex kept.
  - **Ability scores:** standard array, point buy (27 points) or 4d6-drop-lowest, picked in the
    creator.
  - **Background:** the SRD's one background, Acolyte, plus creator-made backgrounds (skill and
    tool proficiencies, languages, equipment, a feature as text).
  - **Class:** the creator's class row arrives with 0.30. Until then everyone is a level-1
    commoner with a fighter's hit die.
  - **Darkvision:** the night and interior lighting use it (a character with darkvision sees
    dim areas brighter).
  - **Models:** races map to the library's models. Goblins, orcs and trolls stop being playable
    races (in the SRD they are monsters); half-orcs use the orc models.

  **Engine.** `races.rs` rewritten for SRD races, the creator (`creator.rs`) rows for ability
  scores and background, darkvision in `lights.rs`, the race import, the conversion of old
  `RaceDef`s.

  **Example.** The creator offers the nine races; Emberbrook's elves and dwarves get SRD traits.

  **Verify.** Autoplay creates a Hill Dwarf with point buy (`status` prints `sheet=`). Unit tests:
  point-buy costs, racial bonuses. Screenshots of the creator.

- [ ] The character sheet — `feature/srd-sheet`

  **Goal.** The character window shows an SRD character sheet.

  **Design.**
  - **The page:** abilities with modifiers, saving throws, skills (proficient ones marked), AC,
    initiative, speed in feet, HP and hit dice, proficiency bonus, passive Perception, languages,
    proficiencies, features and traits.
  - **Levels:** XP by the SRD table (300, 900, 2700…, level 20 at 355,000). Level-up raises HP
    (rolled or average, a project setting), the proficiency bonus and the class features of 0.30.
  - **Who has a sheet:** every character, as an SRD stat block (abilities, AC, HP, speed, senses,
    proficiencies). For NPCs it comes from a template (commoner, guard, bandit, veteran…, from the
    SRD's monster list) or is filled in.
  - **Editor:** the Characters form gets a stat block section with the SRD's layout.

  **Engine.** `progression.rs` replaced, the character window (`charwin.rs`, `inventory_ui.rs`),
  `CharacterDef` stat block fields (appended; old fields converted), the editor form.

  **Verify.** Screenshots of the sheet at levels 1 and 5; XP thresholds (unit test); conversion of
  the example's characters (a compat test with a 0.27 project).

- [ ] Conditions, damage types and rests — `feature/srd-conditions`

  **Goal.** The SRD's conditions and damage rules replace status effects.

  **Design.**
  - **Conditions:** the 15 conditions (blinded, charmed, deafened, frightened, grappled,
    incapacitated, invisible, paralyzed, petrified, poisoned, prone, restrained, stunned,
    unconscious, and exhaustion with its six levels), each with its rules: advantage,
    disadvantage, auto-fail, speed 0. Icons sit over heads and in the sheet.
  - **Damage:** the 13 damage types; resistance, immunity and vulnerability on races, monsters,
    items and conditions.
  - **Rests:** a short rest spends hit dice. A long rest (1 hour of game time or sleeping in a
    bed, which exists) restores HP, half the hit dice, spell slots and features, and lowers
    exhaustion.
  - **Events:** conditions and actions *Apply condition* / *Remove condition* (the effect actions
    remapped), *Short rest* / *Long rest*.

  **Engine.** `effects.rs` reworked into conditions, damage in `combat.rs`, the rest actions and
  beds (`furniture.rs` sleep), script functions.

  **Verify.** Unit tests of each condition's rule flags. Autoplay: poisoned gives disadvantage on a
  check; a long rest restores HP and hit dice; exhaustion levels are shown.

- [ ] SRD data import and attribution — `feature/srd-data`

  **Goal.** The SRD's content comes into the starter library from the source data, with its
  licence.

  **Design.**
  - **The import:** `xtask srd-import <5e-database checkout>` writes skills, races, subraces,
    traits, conditions, damage types, languages, alignments and backgrounds now. Classes, spells,
    equipment and monsters follow with 0.30 and 0.31. It writes into the starter library's
    `defs/` and keeps the SRD's index names as ids (`hill-dwarf`), so a re-run updates instead of
    duplicating.
  - **Attribution:** the *Uses SRD 5.1 content* project setting; exports write the attribution
    file; the title screen's credits show it; the starter library carries it.
  - **Docs:** the manual gets an *SRD 5.1 rules* chapter (how rolls, checks and conditions work in
    the engine, what's simplified) and the licence note.

  **Engine.** The xtask command, the project setting, export of the attribution file.

  **Verify.** The import is idempotent (a second run changes nothing); an export contains the
  attribution file; the problems list warns about SRD assets in a project without the setting.

## 0.29.0 — Turn by Turn

SRD combat: initiative, actions, movement on the grid, attacks against AC, death saves.

- [ ] Turn-based combat — `feature/srd-combat`

  **Goal.** Fights follow the SRD's combat rules in turns, on the map where they happen.

  **Design.**
  - **Starting:** a fight starts when hostiles notice the party (their passive Perception against
    Stealth; unaware creatures are surprised in the first round), or when the player attacks.
    Everyone near rolls initiative (d20 + DEX) and a turn order bar appears. Exploration resumes
    when no hostile is left fighting.
  - **A turn:** movement up to the speed (tiles of 5 ft, with an overlay of where it reaches;
    difficult terrain from materials and decorations costs double), one action, one bonus action
    and one reaction per round.
  - **Actions:** Attack, Cast a Spell (0.30), Dash, Disengage, Dodge, Help, Hide, Ready, Search,
    Use an Object, plus grapple and shove.
  - **Attacks:** an attack roll d20 + modifiers against AC; damage dice by weapon. Critical hits
    double the dice. Ranged attacks get disadvantage in melee and at long range. Cover comes from
    props and decorations (half, three-quarters). Opportunity attacks when leaving reach;
    two-weapon fighting.
  - **0 HP:** the player and party members fall unconscious and make death saving throws
    (stabilize, Medicine check, healing). Monsters die. All party members down means *defeated*
    (exists).
  - **The party:** the player plays every party member's turn, as in tactical RPGs. A party member
    can be left to its AI (a toggle per member).
  - **Enemies:** they act by their stat block (multiattack, special actions with recharge) with
    a simple tactical AI: focus, keep range, flee at low HP by their morale.
  - **UI:** turn order bar, action bar (actions, bonus actions, spells, items), movement and
    reach overlays, target highlighting with the hit chance, a combat log of every roll. A tile
    cursor drives everything for keyboard, mouse and gamepad.
  - **Removed:** real-time combat (`attacks.rs`, `defense.rs`, blocking, dodging, wind-ups,
    battle stance, real-time AI hunting / fleeing, hit-stop). Battle music and the damage numbers
    stay.

  **Engine.** Large:
  - a new `runtime/turns/`: state machine, initiative, action economy, grid movement on the
    occupancy grid, line of sight and cover, AI;
  - the removal of the real-time combat modules;
  - the combat UI;
  - autoplay commands (`turn move X Y`, `turn attack NAME`, `turn action dodge`, `turn end`);
  - `status` `combat=round:who:hp…`.

  **Example.** Emberbrook's fights (wolves in Whisperwood, bandits, the mine's skeletons and boss)
  are SRD encounters; the guards fight beside the player.

  **Verify.**
  - **Autoplay with fixed seeds:** a fight won, one lost with death saves, an opportunity attack,
    cover, a surprise round. Screenshots of each UI part.
  - **Unit tests:** attack maths, crits, death saves.
  - **Perf:** a 10-creature fight stays at 60 fps (`update_ms`).

- [ ] Monsters and encounters — `feature/srd-monsters`

  **Goal.** SRD monsters as ready-made characters, and encounters sized for the party.

  **Design.**
  - **Monsters:** the SRD monster list imported (`xtask srd-import`) as character definitions:
    stat block, actions, traits, CR, XP. Each is mapped to a library model, or left without one (a
    placeholder: the mannequin tinted) until 0.35 brings models.
  - **Encounters:** encounter zones (`ZoneKind::Encounter`, appended) hold a monster list or an
    XP budget by the SRD's encounter difficulty (easy, medium, hard, deadly for the party's
    levels), and spawn their group once or on a timer (spawners exist).
  - **Rewards:** XP by CR, split among the party.

  **Engine.** The monster import, the zone kind, the encounter budget maths (unit-tested), the
  editor's zone form.

  **Example.** The forest and the mine use encounter zones.

  **Verify.** Unit tests of XP budgets. Autoplay: an encounter zone spawns a group within budget
  (`status`), and the fight runs.

## 0.30.0 — Classes and Magic

The twelve SRD classes with their subclass, spellcasting and the spell list.

- [ ] Classes and levelling up — `feature/srd-classes`

  **Goal.** The player and party members have SRD classes from level 1 to 20.

  **Design.**
  - **The classes:** the 12 SRD classes (barbarian, bard, cleric, druid, fighter, monk, paladin,
    ranger, rogue, sorcerer, warlock, wizard), each with the SRD's subclass (Berserker, Lore, Life,
    Land, Champion, Open Hand, Devotion, Hunter, Thief, Draconic Bloodline, Fiend, Evocation).
  - **Features:** hit die, proficiencies, starting equipment and features by level from the import.
  - **Features that act:** implemented as engine abilities (Rage, Sneak Attack, Action Surge,
    Second Wind, Ki, Lay on Hands, Wild Shape later…). Features that are only text are shown on the
    sheet.
  - **Levelling up:** a window to choose the subclass, ability score improvements or the feat
    (Grappler is the SRD's one), new spells and cantrips, and HP.
  - **Multiclassing** by the SRD's rules (prerequisites, proficiencies, spell slots) comes last
    and may move to the backlog.

  **Engine.** Class data (import), the level-up window, the feature runtime (a feature vocabulary
  plus Rhai for the rest), the creator's class row.

  **Example.** The hero picks a class; Dagny (fighter) and Finn (ranger) join as party members.

  **Verify.** Autoplay levels a fighter, a rogue and a wizard from 1 to 5 (choices scripted);
  `status` prints `sheet=`; each acting feature in a fight; screenshots of the level-up window.

- [ ] Spellcasting and the SRD spells — `feature/srd-spells`

  **Goal.** Spells work as in the SRD.

  **Design.**
  - **Casting:** spell slots by class level (pact magic for warlocks), known or prepared spells,
    cantrips that scale with level, the spellcasting ability, spell attack bonus and save DC,
    components (a focus or component pouch), concentration (a CON save on damage, one at a time),
    rituals, casting at higher levels.
  - **The spells:** the SRD spell list (319 spells) imported with their texts. Each spell's
    mechanics come from a spell effect vocabulary:
    - attack roll or saving throw;
    - damage dice and type, half on a save;
    - conditions;
    - healing;
    - temporary HP;
    - buffs and debuffs (advantage, AC, speed);
    - areas (sphere, cone, cube, line, cylinder in tiles);
    - duration and concentration;
    - teleport, light, summon (later).

    Spells the vocabulary can't express get a Rhai script or are marked *story only* (usable
    through events, e.g. *Speak with Dead* in a quest).
  - **Outside combat:** *Light*, *Detect Magic*, *Knock* (opens locks), *Mage Hand* and the like.
  - **Replaces:** the engine's spells and mana (`skills.rs` spells, the *Spells* tab, `cast`,
    mana regeneration).

  **Engine.** The spellcasting runtime, the effect vocabulary, the area targeting UI (templates on
  the grid), the import, the Spells tab reworked into a spellbook, the removal of mana.

  **Example.** The example's spells become Cure Wounds, Light, Fire Bolt and Fireball; Ansel the
  priest is a cleric.

  **Verify.** Autoplay with fixed seeds for each effect kind (a save for half, an attack spell, an
  area, concentration broken by damage, an upcast). A count of how many of the 319 spells are fully
  mechanical, *script* or *story only* goes in the manual.

## 0.31.0 — Gear and Treasure

SRD equipment, magic items and money.

- [ ] SRD equipment — `feature/srd-equipment`

  **Goal.** Weapons, armour and gear follow the SRD.

  **Design.**
  - **Weapons:** simple / martial, damage dice and type, and the properties ammunition, finesse,
    heavy, light, loading, range, reach, thrown, two-handed, versatile.
  - **Armour:** light / medium / heavy with the AC formulas, STR requirements and Stealth
    disadvantage; shields.
  - **Other gear:** tools and kits (used by checks), adventuring gear, packs, mounts and vehicles
    as items (riding them is in the backlog).
  - **Money:** cp, sp, ep, gp, pp, with prices in the SRD's coins; the example's coin becomes gp.
  - **Proficiency:** attacking with a weapon the character isn't proficient with drops the
    proficiency bonus; armour without proficiency gives disadvantage and no spellcasting.
  - **Items keep their look:** 3D models, icons and held pictures stay; the item form gets the SRD
    fields.

  **Engine.** `ItemDef` SRD fields (appended; old weapon / armour fields converted), the import,
  shop prices in coins, the item form.

  **Verify.** Unit tests of the AC formulas and properties; autoplay buys, equips and attacks with a
  versatile weapon two-handed.

- [ ] Magic items — `feature/srd-magic-items`

  **Goal.** The SRD magic items.

  **Design.**
  - **Data:** rarity, attunement (three items at most), charges and recharge, and effects
    through the spell effect vocabulary and conditions (a +1 weapon, a Ring of Protection, a
    Potion of Healing that rolls 2d4+2).
  - **Import:** the SRD magic item list.
  - **Library:** items without art get a generic icon per category until the library has art.

  **Engine.** Attunement and charges on items, the import, the item form.

  **Verify.** Autoplay: attune, use charges, recharge at dawn; a +1 weapon's maths in a fight.

## 0.32.0 — Everywhere

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

## 0.33.0 — Studio Comfort

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

- [ ] Animation editor — `feature/animations`

  **Goal.** Animations for spells, attacks, class features and items, timed with sound and flashes.

  **Design.**
  - **The asset:** a new `defs/animations.ron`. An `AnimationDef` has:
    - timed cels: frames of a sprite sheet with position, scale, rotation, opacity and blend;
    - particle emitters: count, speed, spread, lifetime, colour over life, gravity;
    - flashes of the target or the screen;
    - sounds at frames.
  - **Uses:** spells, class features, items, *Play effect* (an animation where a sheet is
    accepted) and the attacks of turn-based combat (0.29).
  - **Editor:** a timeline and a preview on a target dummy.
  - **Library:** starter animations: slash, fire, ice, heal, thunder, buff.

  **Engine.** The defs file, the player in `runtime/fx.rs`, the editor.

  **Verify.** Each starter animation in autoplay screenshots mid-play; `status` prints `fx=`.

## 0.34.0 — Learning

A way in for new creators: tutorials and small games to learn from.

- [ ] Tutorials — `feature/tutorials`

  **Goal.** A learning path.

  **Design.**
  - **Step-by-step manual chapters** with screenshots: "Your first quest", "Your first dungeon"
    and "Your first encounter" (an SRD fight: monsters, an encounter zone, the rewards).
  - **"Coming from another maker":** the terms side by side:
    - switches → flags;
    - self switches → prop states;
    - common events → common events;
    - plugins → plugins;
    - parallel processes → parallel common events;
    - a JRPG battle system → SRD turn-based combat.
  - **README:** links to them.

  **Engine.** None (docs). The screenshots are taken with editor and autoplay scripts.

  **Verify.** Follow each tutorial on a fresh project and fix every step that doesn't match.

- [ ] Sample projects — `feature/sample-projects`

  **Goal.** Small games to learn from, besides Emberbrook.

  **Design.**
  - **The samples:**
    - an SRD adventure for a party of four level-1 characters: a village, a dungeon and its boss;
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

## 0.35.0 — A Bigger Library

A content release for the starter library. Generating 3D models costs Meshy credits: ask first.

- [ ] Material and decoration sets — `feature/biome-sets`

  **Goal.** More worlds than a green valley.

  **Design.** Snow, desert, dungeon, cave and swamp: materials, props, scatter sets and
  structures for each. The example gets a cave level in the mine with them.

  **Engine.** None (content).

  **Verify.** A screenshot of each set on a test map; the example's cave in autoplay.

- [ ] Models for the SRD monsters — `feature/bestiary`

  **Goal.** The SRD monsters imported in 0.29 get 3D models with walk, idle, attack and hurt
  animations.

  **Design.** By how often games use them: wolves, bats, rats, spiders, skeletons, zombies,
  goblins, orcs, ogres, giants, dragons (wyrmlings first), elementals, animals. Each one costs
  Meshy credits (ask); models are shared where the SRD has variants (a wolf and a dire wolf
  scaled).

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
- **iOS export:** needs a Mac and an Apple developer account; after Android (0.32).
- **RPG Maker MV / MZ importer** (moved from 0.32, 2026-11-16: R(ust)PG2D follows the SRD rules,
  not the JRPG model): maps and art through the tile converter, the database mapped onto SRD
  definitions where it fits, events onto triggers; the RTP's files skipped (licensed for RPG
  Maker only). The full design block is in git history (ROADMAP.md, 0.32.0).
- **SRD multiclassing** if it doesn't fit 0.30, and **Wild Shape**, **summons** and the other SRD
  features that need a creature of their own.
- **Riding mounts and vehicles** with the SRD's rules (mounted combat).
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
