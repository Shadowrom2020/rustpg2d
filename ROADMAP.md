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

## The SRD 5.1 ruleset (0.28.0 to 0.32.0)

The user's decision (2026-11-16): R(ust)PG2D's rules follow the **System Reference Document 5.1**
(SRD 5.1) by Wizards of the Coast, as closely as the engine allows, instead of growing into another
JRPG maker. Things that apply to all five releases:

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
  - Saved games: a migration where it can be done (0.28's `SAVE_VERSION` 2 gives an old save's
    hero the project's starting scores); where it can't, they don't load, with a *Breaking:* line.
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

- [x] Dice, abilities and checks — `feature/srd-core`

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

  **Decided while building.**
  - **Checks are actions, not conditions:** conditions are evaluated for quest markers, choice
    visibility and editor previews, so rolling inside one would roll again and again. *Ability
    check* / *Saving throw* are one action with *On a success* / *On a failure* actions; the
    condition *Check passed* (by at least N) reads the last result. *Roll dice* rolls into a variable.
  - **What a check rolls** is written as a skill (`persuasion`), an ability (`dex`) or
    `ability:tool` (`dex:thieves-tools`); saving throws name the ability. Proficiencies are ids:
    the SRD's index names of skills and tools, `save:dex` for saving throws.
  - **The hero's data** (`abilities`, `proficiencies`, the last 50 `rolls`) lives in `WorldState`,
    which also rolls (`check`, `roll_dice`), so scripts, the action runner and the game share one
    path. The dice generator is thread-local and seedable (`seed N`, `RUSTPG2D_SEED`; autoplay
    starts at 1).
  - **Until the creator item**, *Database › Project › Rules* sets the new hero's scores (standard
    array by default) and proficiencies; races add theirs (`RaceDef.skills` holds proficiency ids).
  - **The skill tree is removed**, not remapped: skill definitions, points, effects and the Database
    tab. The K screen's *Skills* tab shows the SRD page instead (scores, saves, the 18 skills,
    tools); the full character sheet comes with `feature/srd-sheet`. Real-time combat, mana and
    spells stay (spells need a proficiency instead of a skill level).
  - **The lockpicking minigame is removed** (the user's decision): picking is a DEX (Thieves' Tools)
    check against `LockSpec::pick_dc` (old `pick_difficulty` levels convert to DCs on load); a failed
    lock waits ten game minutes. Fishing keeps its minigame.
  - **Crafting**: a recipe's `skill` is a proficiency (`level` > 0 = needed); quality steps have
    DCs and crafting rolls INT with the tool (or the skill's ability); a failed roll still makes the
    plain item. The example's recipes need no proficiency (nobody can gain one in play yet).
  - **Conversion** (`srd_convert.rs`, by `ProjectConfig::rules_version`): the editor and libraries
    convert once, with a backup; skill ids are renamed through the refactor machinery
    (`RefKind::Skill` = a proficiency id, no definition). Saves get `SAVE_VERSION` 2: older saves
    load with the project's starting scores (no break for saves after all).
  - **The example** had no shrine riddle and no haggling: the shrine's first look is a Religion check
    (DC 12: lore and 15 XP, instead of an elder's line), and Tilda's quest reward offers "Ask for a
    bit more" (Persuasion, DC 13: ten more coins). A **Locked Chest** (DC 12) stands by the river.
  - Found on the way: runtime templates failed when a value started with a multi-byte letter
    ("Überzeugen"); fixed in `lang.rs`.

  **Engine.** The `srd/` module (dice, abilities, skills, tools, checks), `srd_convert.rs`,
  `Action::Check` / `Action::Roll` and `Cond::CheckPassed` (appended), script functions (API 0.28),
  the journal's *Rolls* page and the HUD lines (`runtime/rolls.rs`), the SRD Skills tab, lock checks,
  crafting checks, `ProjectConfig::rules` / `rules_version`, `SAVE_VERSION` 2.

  **Example.**
  - Lockpicking is a DEX (Thieves' Tools) check: the Locked Chest by the river (DC 12).
  - The shrine's carvings are an INT (Religion) check.
  - Tilda's haggling is CHA (Persuasion).

  **Verify (done).** Unit tests: the dice parser and errors, fixed-seed statistics (d20, advantage,
  disadvantage means), modifiers 1-30, proficiency by level, check maths with proficiency and tools,
  old lock / quality levels to DCs, the skill-to-proficiency table, a 0.27 project converting
  (fixture `project-0.27.0`), the runner's check branches, the Rules form (UI test). Autoplay with
  fixed seeds on a scratch copy: the chest failed (7 vs 12), refused within ten minutes, picked
  after an hour (13 vs 12) and opened; the shrine's Religion check (18 vs 12: lore, +15 XP);
  Tilda's haggle passing (14 vs 13: +10 coins) and failing (4 vs 13); crafting a Fine Iron Sword
  (19 vs 12) and a plain one (3); screenshots of the check lines, the Rolls page and the Skills tab
  in English and German. Editor: the *Ability check* action form, *Project › Rules*, the example's
  conversion (9 skills mapped, 5 dropped) and the problems list (none).

- [x] Races and the character creator — `feature/srd-races`

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

  **Decided while building** (with the user: models reused, the import tool now, all racial
  choices).
  - **`RaceDef` grows** instead of being rewritten (old races keep loading as they are): `parent`
    (a subrace names its race; `RaceDef::merged` / `Database::race_full` fold them together),
    `ability_bonuses`, `walk_ft`, `size_category`, `darkvision_ft`, `languages`, `resistances`,
    `traits`, `choices` (kinds Ability +1, Skill, Tool, Language, Ancestry with the damage type it
    grants, Cantrip) and `colors` (the creator's default colours). Weapon and armor proficiencies
    (`battleaxes`) are proficiency ids too (`srd::abilities::GEAR`).
  - **The import tool** is the editor's `srd_import.rs`, run by `cargo run -p rustpg2d-editor
    --example srd_import -- <checkout> [folder]` (not xtask: it needs the engine's types), pinned
    to 5e-database `a6212beb`. It merges by id and keeps content fields; a base race with subraces
    becomes not playable (its subraces are). `srd-data` extends it.
  - **Backgrounds** are a definition kind (`defs/backgrounds.ron`, `RefKind::Background`, the
    Database tab), so they travel through the library. The Acolyte's equipment is the SRD's (0.31):
    the example gives it 15 coins; Farmhand and Wanderer are the example's own.
  - **The creator has a second page** (*Abilities and background*): method, scores, the race's
    choices, background and its languages; the project's `CharacterCreation::ability_methods`
    picks the methods. Its result goes through `races::build_hero`; racial increases stop at 20.
  - **Darkvision** lifts the ambient light towards a dim grey (`render::lighting::darkvision`),
    not the lights; speed is `walk_ft / 30`.
  - **Models**: halfling = hero / heroine at 0.75, gnome = dwarf models at 0.75, dragonborn and
    half-orc = the orc models (they have no colour features), half-elf = elf models, tiefling =
    hero / heroine with a red skin.
  - The HP and the class row stay for the sheet (item 3) and classes (0.30); weapon and armor
    proficiencies are listed on the sheet, not yet on the Skills tab.

  **Engine.** `races.rs` (SRD fields, merging, `build_hero`), `backgrounds.rs` (a new definition
  kind), the creator's second page, darkvision in `render/lighting.rs`, the speed factor, script
  functions, `sheet=`; the editor's `srd_import.rs`, race form, Backgrounds tab.

  **Example.** The creator offers the nine races (the subraces for dwarf, elf, halfling, gnome) and
  three backgrounds; Emberbrook's elves and dwarves get SRD traits through their races.

  **Verify (done).** Unit tests: point buy and the standard array swap, 4d6 range, subrace merging,
  `build_hero` (increases capped at 20, choices, background languages), the importer and its second
  run, backgrounds round trip, darkvision. UI test: a background added. Autoplay on a scratch copy:
  a Hill Dwarf with point buy (`sheet=` CON 17, WIS 14, dwarvish, poison, darkvision 60, 25 ft,
  Brewer's Supplies), a Half-Elf with its choices, a red Dragonborn with rolled scores (fire
  resistance), a Tiefling (red skin in the preview), the mine with and without darkvision, a
  dwarf walking 9 tiles where a human walks 11. Screenshots of both creator pages and of the
  editor's race form and Backgrounds tab.

- [x] The character sheet — `feature/srd-sheet`

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

  **Decided while building.**
  - **Stat blocks are data for now** (the maintainer's choice): `CharacterDef::stats` /
    `EntityInstance::stats` (`srd::StatBlock`: template, abilities, AC, HP, hit dice, speed,
    darkvision, proficiencies, languages, CR, XP). Real-time combat keeps `max_hp`, `damage`,
    `defense`; the turn-based combat of 0.29 takes the stat blocks. No conversion of the old fields.
  - **Templates** are the SRD's 35 humanoid NPCs (not the lycanthropes' forms), written by
    `srd_import` into `crates/engine/src/srd/npcs.ron` and built into the engine; the monsters come
    with `srd-monsters`. The agent server fills `stats: {template: "guard", …}` from the template.
  - **The hero's HP** are derived, not stored: `WorldState::hp_rolls` (what each level's die gave;
    missing = the average, so old saves load) + the current CON modifier + the race's new
    `hp_per_level` (hill dwarf) + its old `max_hp`. The hit die is a d10 for everyone until
    classes. Rolled HP go into the roll log. `hit_dice` is counted (spent with rests, item 4).
  - **AC** = 10 + DEX + the worn armor's defense until 0.31's armor.
  - **The HUD** shows a red HP bar with "hp / max" instead of hearts (the maintainer's choice); the
    skin's `hp` colours and frame dress it.
  - The **Skills tab became the Sheet** (K; `wintab sheet`, `skills` still works); features and
    traits (race traits, background feature) are selectable with their text at the bottom.
  - The importer clears the 0.27 example's HP / defense bonuses of the SRD races (they would count
    twice).
  - **XP rebalanced** in the example: quests 50-300 (the elder's Strange Lights 300; all ten give
    1,450), the shrine's lore 50, defeated characters their template's XP (bandit 25, goblin 50,
    orc 100, bandit chief 450, Rook the veteran 700), monsters on the same scale for now (wolf 50,
    bat and slime 10, trolls 700). The story's fixed fights and quests give about 4,500 XP: level 2
    after the first village quests, level 3 in Whisperwood, level 4 around the mine.

  **Verify (done).** Unit tests: the XP table at every threshold, HP by level (average, rolled, CON,
  at least 1, hill dwarf), the templates (35, guard AC 16 HP 11), the importer writing them, the
  scripts (`armor_class`, `hit_dice`, `npc_stat`), the 0.27 project's progression and 0.16
  characters loading (compat). UI test: a stat block from the Guard template, a proficiency
  toggled, saved, removed. Autoplay on a scratch copy: the sheet at levels 1 and 5 (an acolyte hill
  dwarf: HP 54 at level 5, Religion +3), the HP bar in the wood skin, rolled level-ups (seed 7:
  `rolls=1d10:3,1d10:1`, max HP 23 at level 3), `statblocks=` beside the guard. Screenshots of the
  sheet and the editor's stat block section.

- [x] Conditions, damage types and rests — `feature/srd-conditions`

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

  **Decided while building.**
  - **Effects replaced now** (the maintainer's choice): `srd/conditions.rs` (`Condition`,
    `ConditionSpec`, `ActiveCondition`, `rules`) took `effects.rs`'s place. Old data reads through
    reprs: poison = poisoned + poison per round (strength × 6), burn = fire per round, regen =
    healing per round, stun = stunned, slow = restrained; haste, strength, weakness and shield =
    nothing. The variants `ApplyEffect`, `CureEffect`, `HasEffect` and `WorldState::effects` keep
    their names (map and save compatibility); `Condition`'s `#[serde(other)] Any` reads old names
    without a condition. RULES_VERSION 2 converts projects once (with a backup) and takes out what
    became nothing, listing where.
  - **A spec** = condition (or none), seconds (0 = until removed), exhaustion levels, a saving throw
    that keeps it off, damage (a type) or healing (none) per round of 6 s, the first at once.
  - **In real time:** speed (0 / half), can't act, lying down (prone, unconscious), invisible
    (faint, unnoticed except close), blinded (the hero's circle; a character's 1-tile sight),
    charmed (no attacks on the hero / by the hero), frightened (flees), hits on paralyzed or
    unconscious are critical, exhaustion 4 halves max HP and 6 kills. Attack-roll disadvantage is a
    flag for 0.29. Checks and saves (`WorldState::check`) apply disadvantage and automatic failures.
    The Sheet shows conditions, speed and passive Perception (−5 with disadvantage) follow them.
  - **Damage types for everyone now** (the maintainer's choice): weapons, Bolt / Blast,
    area damage, ongoing damage; the hero's race and armor resistances, petrified = all; stat
    blocks got resistances, immunities, vulnerabilities and condition immunities (imported), used
    in real time. Resistance comes before the old defense.
  - **Rests:** the short rest is a window (R on the sheet, gamepad X) as the maintainer chose; any
    sleep of an hour or more is a long rest (the bed's full heal stayed). Spell slots and class
    features come with 0.30.
  - **Content:** the elixir of strength (STR 21 with a timer), swiftness potion (exhaustion,
    grapples) and apple pie were rebuilt as content; food heals per round; item texts say HP.

  **Verify (done).** Unit tests: each condition's rules, exhaustion's levels, ongoing rounds,
  applying again, the 0.27 reprs (specs, saved effects, old names), checks with poisoned / stunned /
  blinded, damage types and defenses, NPC saves (proficiency by CR), the converter taking out
  haste / strength, the scripts. UI test: a weapon's damage type and a touch condition. Autoplay
  on a scratch copy: poisoned + exhaustion 2 on the HUD and the Sheet, a short rest (1d10+CON = 7,
  +1 hour) and a long rest (exhaustion 2 → 1, poison over, hit dice back), prone hero and guard
  lying down, blindness, invisibility and a frightened guard, the slime's CON save and poison (a
  hill dwarf takes none of it), an armed swing knocking prone with a STR save. The example
  converted (its report lists the elixir, the swiftness potion and the apple pie). Screenshots of
  conditions, the rest window and the editor's condition fields.

- [x] SRD data import and attribution — `feature/srd-data`

  **Goal.** The SRD's content comes into the starter library from the source data, with its
  licence.

  **Design (as built).**
  - **The import** is the editor's `srd_import` example (`cargo run -p rustpg2d-editor --example
    srd_import -- <5e-database checkout> [folder]`), not an xtask command: xtask stays free of the
    engine, the importer needs its types. It writes races, subraces, traits, choices and
    backgrounds into a library's or project's `defs/`, and from the repository the NPC templates
    (`srd/npcs.ron`, with alignments) and `srd/rules.ron`: the SRD's texts of conditions, damage
    types, languages, alignments, abilities and skills and the ids of its races and backgrounds.
    It keeps the SRD's index names as ids (`hill-dwarf`); a second run changes nothing (tested
    byte for byte). Skills, conditions, damage types and languages stay engine constants: a test
    keeps them equal to the SRD's ids, names and abilities. Classes, spells, equipment and
    monsters follow with 0.30 and 0.31.
  - **Attribution:** `srd::ATTRIBUTION` (CC BY 4.0's wording, no trademarks, the data's source).
    Every export carries `SRD-5.1-ATTRIBUTION.txt` (the runner's rules include SRD text), and so
    do the AppImage / Flatpak packages, the repository and the starter library. The project setting
    *Uses SRD 5.1 content* (`ProjectConfig.srd`; on in the example, the library and new projects
    from it) ends the Credits page with it; `srd_attribution()` returns it for scripted credits
    (the example's `roll_credits` through a `{credits_srd}` text variable). *Problems* warns when
    a project has SRD races, backgrounds or template stat blocks and the setting is off.
  - **Docs:** the manual's chapter 19, *The SRD 5.1 rules*.

  **Verify (done).** Importer tests (the rules text, a second run identical, alignments), the
  constants equal to the SRD, the attribution files equal to the constant, the export test (the
  file in the folder and the zip), the package layout, a 0.27 project loading with the setting
  off, the Problems warning on a headless editor. The example's credits rolled on the title
  (screenshot), an export of a scratch copy.

## 0.29.0 — Turn by Turn

SRD combat: initiative, actions, movement on the grid, attacks against AC, death saves.

- [x] Turn-based combat — `feature/srd-combat`

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

  **As built.**
  - **Engine:** `runtime/turns/` (`mod.rs` the fight's state, start / join / end, turns, the walk
    with opportunity and readied attacks, attacks, damage, death saves, status; `actions.rs` the
    action economy and the SRD's actions with their checks; `ai.rs`; `ui.rs` input and drawing) on
    the pure rules of `srd/combat.rs` (odds, attack rolls, crits, cover, death saves, unit-tested).
    `Game.fight` is not saved: saving waits for the end. Walks use the bodies' move targets (5
    tiles a second). Characters without a stat block get one from their old values; a stat block
    without attacks gets one from the weapon / *Damage*. Damage from scripts, spells and areas
    goes through the fight's damage while one is on (0 HP = unconscious for the party).
  - **Kept:** battle stance and battle music now follow the fight; projectiles fly as the look of
    ranged attacks and spells; the real-time chase stays for noticing (it starts or joins fights)
    and for characters of hostile factions away from the player (they close in, no damage).
  - **Not yet:** difficult terrain from decorations, special actions with recharge, the AI-played
    companions' spells in fights, stabilizing with a Medicine check (healing gets them up). The
    example's fights are SRD-lethal for a lone level-1 hero; sizing them is *Monsters and
    encounters*'s.

  **Verify (done).** Unit tests of the odds, attack rolls, crits, cover and death saves; autoplay
  with fixed seeds on a scratch copy: a fight won (bats: XP, autosave, exploration resumes), one
  lost with death saves (stable, then the *defeated* menu), an opportunity attack, three-quarters
  cover in the log, a surprise round from sneaking, a companion's turn played and then handed to
  the AI, a late joiner (the orc chief), grapple / hide / help, keys, clicks and the action bar,
  saving refused in a fight; screenshots of the order bar, the action bar, the overlays, the hit
  chance and the log. Perf: a fight of four at `update_ms` 2.2.

- [x] Monsters and encounters — `feature/srd-monsters`

  **Goal.** SRD monsters as ready-made characters, and encounters sized for the party.

  **Design.**
  - **Monsters:** the SRD monster list imported (`xtask srd-import`) as character definitions:
    stat block, actions, traits, CR, XP. Each is mapped to a library model, or left without one (a
    placeholder: the mannequin tinted) until 0.36 brings models.
  - **Encounters:** encounter zones (`ZoneKind::Encounter`, appended) hold a monster list or an
    XP budget by the SRD's encounter difficulty (easy, medium, hard, deadly for the party's
    levels), and spawn their group once or on a timer (spawners exist).
  - **Rewards:** XP by CR, split among the party.

  **Engine.** The monster import, the zone kind, the encounter budget maths (unit-tested), the
  editor's zone form.

  **Example.** The forest and the mine use encounter zones.

  **Verify.** Unit tests of XP budgets. Autoplay: an encounter zone spawns a group within budget
  (`status`), and the fight runs.

  **As built.**
  - **Monsters as library characters** (your choice over built-in templates): `srd_import` writes
    the 334 into the starter library (`import_monsters`); ids are the SRD's indexes. A library
    model where one fits (wolf, bat, skeleton for undead, slime for oozes, the NPC models), else
    the `mannequin` model at the SRD size (`Appearance.size`). Characters that already had a
    monster's id took its stat block; characters with a monster's block (the templated NPCs) keep
    their numbers and gain the new fields. `StatBlock` gained size, type, speeds, senses and
    `features` (texts of traits / actions / reactions / legendary actions; they act in 0.32).
    `RulesText.monsters` makes them SRD content for the attribution.
  - **Encounters:** `ZoneKind::Encounter { monsters, difficulty, respawn }`; the budget is the
    engine's own (per party member at level L: one monster of CR L/8, L/4, L/2, L), because the
    SRD 5.1 has no encounter tables; `pick` is seeded per map and zone. Spawners and encounters
    share the runtime. XP goes to the hero alone until companions level (0.30).
  - **Not yet:** monsters' special actions, multi-tile creatures (0.32); fliers just use their
    fly speed on the grid.

  **Verify (done).** Unit tests of the budgets and the pick, the monster import (wolf, adult red
  dragon: size, speeds, attacks, Multiattack, legendary texts; a second run identical; a project
  only updated), `srd_content`. Autoplay on a scratch copy: the forest encounter picks a wolf (50
  XP) alone and four monsters (95 of 100 XP) with the guard along; the mine's easy one a giant rat;
  a fight with it gives 25 XP and a bestiary entry. The library window lists 385 characters at 60
  fps.

## 0.30.0 — Classes and Magic

The twelve SRD classes with their subclass, spellcasting and the spell list.

- [x] Classes and levelling up — `feature/srd-classes` (part 1), `feature/srd-class-actions` (part 2)

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

  **As built (part 1, `feature/srd-classes`).** Two stacked pull requests (your choice, the item
  being large): this one the data, the creator, levelling, the sheet, the party and the passive
  features; part 2 the features used as actions (resources, actions, riders, reactions, the AI).
  Your decisions: every feature that can act acts (subclass ones included), the full party levels
  (XP split among the classed members), starting equipment mapped to library items.
  - **Data:** `ClassDef` (`defs/classes.ron`, `RefKind::Class`, a library kind) with levels 1-20
    (features, numbers flattened from the SRD: `rage_count`, `spellcasting.spell_slots_level_1`…),
    subclasses, features with `FeatureEffect`s (passive vocabulary) and `FeatureChoice`s
    (options, expertise, ability scores, skills). The importer maps feature indexes to effects; the
    rest is text. `HeroClass` (class, subclass, picks) on `WorldState.class`, on
    `CharacterDef.class` + `level` and on saved `Companion`s (with xp, level, hp rolls).
  - **Levelling:** HP by the class's hit die (`max_hp_die`); pending level-ups
    (`WorldState.levelups`, party members by their home `map:uid`) open `runtime/levelup.rs`'s
    window when nothing else runs. ASI = two +1 rows or the Grappler feat (advantage on attacks
    against the creature grappled).
  - **The level-1 choices** are made in the level-up window right after the creator (not on the
    creator's page): the same window serves every level, and the creator's third page stays short
    (class and skills).
  - **Party:** `member_block` makes a classed companion's stat block (scores, HP, AC from gear or
    unarmored defense, one weapon attack, extra attacks, styles). Dagny and Finn join through
    their talk scripts (`set_class(name, class, level())`); Finn's stall opens from his talk.
  - **Events / scripts:** *Has class* (`Cond::Class`, also a subclass id), *Set class*
    (`Action::SetClass`), API 0.30 `class`, `subclass`, `class_level`, `has_feature`,
    `level_up_pending`, `set_class` (`Arg::Class`). `resource()` comes with part 2.

  **As built (part 2, `feature/srd-class-actions`).**
  - **Vocabulary** (`classes.rs`, appended `FeatureEffect`s): `Resource` (count + a class number +
    an ability modifier + per level; short or long rest; a later feature of the same id replaces it:
    Font of Inspiration), `Act` (id, name, cost Action / Bonus / Reaction / Free, resource,
    `ActEffect`: Heal, HealPool, HealAllies, ExtraAction, Rage, UnarmedStrikes, FrenzyAttack, Basic
    (dash / disengage / dodge / hide), GiveDie, AreaSave, WeaponBuff, Reckless, TempHp, Script), and
    the riders SneakAttack, ExtraDamage, OnHitSave, OnKillTempHp, DropTo1, StartOfTurnHeal,
    ReactionHalveDamage, ReactionReduceRanged, RerollSave, ReactionAttack, DestroyUndead.
    `ClassTraits` collects them (`resources`, `acts`, `riders`). The importer's `feature_effects`
    maps the SRD features to them.
  - **Runtime** (`runtime/turns/features.rs`): uses spent in `WorldState.spent` and
    `Companion.spent` (rests give them back), a fighter's `ClassFx` (rage rounds, frenzy, reckless,
    an inspiration die, Sacred Weapon, temporary HP; not saved), once-per-turn marks; the action bar's
    *Class* entry (`TurnAct::Class`, `Aim::Feature`), riders in `make_attack`, absorption and
    Relentless Rage in `combat_damage`, Indomitable / inspiration in `who_check`, Survivor and rage
    upkeep at the turn's start, the party AI's `ai_class_step`. Reactions are automatic (the prompt
    is 0.32).
  - **Scripts / testing:** `resource(id)`; autoplay `turn class ACT [NAME]`, `turn log [N]`;
    `status` `resources=`, `class_fx=`; the fight log shows each hit's damage.

- [x] Spellcasting and the SRD spells — `feature/srd-spells`

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

  **As built.** One pull request. Your decisions: mana removed with a conversion, scrolls by the
  strict SRD (only a class with the spell on its list uses one), the AI's casting stays 0.32.
  - **Data:** `SpellDef` gained the SRD fields (`level` reused, school, `SpellCast`, `SpellRange`,
    `Area` (`Shape`), components, ritual, concentration, `duration_rounds`, classes, `story_only`,
    `SpellVisual`) and `parts: Vec<SpellPart>` (Attack, Save, Damage / Heal / TempHp by level with
    `MOD`, Condition, Targets, Buff (`srd::conditions::Buff` on an `ActiveCondition`), Light,
    Unlock, Script). `SpellEffect::Parts` (appended, the default); old spells are upgraded when
    read (`SpellDef::upgrade`) and `RULES_VERSION` 3 converts projects with a backup.
    `ClassDef.spellcasting` (`Spellcasting`: ability, `CasterKind` Known / Prepared / Spellbook /
    Pact, half, ritual). `WorldState` `prepared`, `slots_spent`, `concentration`; `Companion`
    `spells`, `prepared`, `slots_spent`; `ActiveCondition` `source` and `buff`.
  - **Import:** the 319 spells (113 mechanical, 206 story only, no scripts); a table adds
    conditions, boons, light and Knock; damage without an attack or save is kept only for Magic
    Missile (Call Lightning and Flaming Sphere get their DEX save, Scorching Ray an attack).
  - **Runtime:** `runtime/spells.rs` (slots, known / prepared, concentration, scrolls, casting out
    of fights, the level-up picks), `runtime/turns/casting.rs` (targets, areas, `resolve_spell`,
    the CON save on damage, buffs in AC, speed, rolls and resistances); the Spells tab is the
    spellbook; mana and the companions' real-time magic are gone (`companion_magic.rs` keeps
    teaching under the class rule). Event action *Cast spell*; scripts `spell_slots`,
    `slots_left`, `restore_slots`, `prepare_spell`, `is_prepared`, `concentrating`, `cast_spell`
    (`mana` / `add_mana` deprecated); autoplay `turn cast`, `turn aim`.
  - **Example:** Cure Wounds, Light, Fire Bolt, Burning Hands, Magic Missile, Fireball (renamed from
    heal, spark, fire_burst); the scrolls and books follow the rule; Brother Ansel (the priest) is a
    Life cleric who heals a hurt hero once a day (`ansel_blessing`, *Cast spell* free).

## 0.31.0 — Gear and Treasure

SRD equipment, magic items and money.

- [x] SRD equipment — `feature/srd-equipment`

  **Goal.** Weapons, armour and gear follow the SRD.

  **Design.**
  - **Weapons:** simple / martial, damage dice and type, and the properties ammunition, finesse,
    heavy, light, loading, range, reach, thrown, two-handed, versatile.
  - **Armour:** light / medium / heavy with the AC formulas, STR requirements and Stealth
    disadvantage; shields.
  - **Other gear:** tools and kits (used by checks), adventuring gear, packs, mounts and vehicles
    as items (riding them: 0.32's mounted combat).
  - **Money:** cp, sp, ep, gp, pp, with prices in the SRD's coins; the example's coin becomes gp.
  - **Proficiency:** attacking with a weapon the character isn't proficient with drops the
    proficiency bonus; armour without proficiency gives disadvantage and no spellcasting.
  - **Items keep their look:** 3D models, icons and held pictures stay; the item form gets the SRD
    fields.

  **Engine.** `ItemDef` SRD fields (appended; old weapon / armour fields converted), the import,
  shop prices in coins, the item form.

  **Verify.** Unit tests of the AC formulas and properties; autoplay buys, equips and attacks with a
  versatile weapon two-handed.

  **As built.** One pull request. Your decisions: five coins with automatic change (the example's
  coin is the gold piece, prices rescaled), lockpicking requires thieves' tools, helmets are for
  looks only, all 237 items imported with generic icons.
  - **Data:** `ItemDef` `srd`, `cost_cp`, `weight`, `coin_cp`; `Weapon` `category`
    (`WeaponCategory`), `properties` (`WeaponProperty`), `versatile`, `thrown_ft`; `Armor`
    `category` (`ArmorCategory`: Clothing, Light, Medium, Heavy, Shield), `base_ac`, `dex_max`,
    `str_min`, `stealth_disadvantage`. `ItemDef::upgrade` in `parse_defs`, `RULES_VERSION` 4
    (`srd_convert` reports the equipment). `srd/equipment.rs`: the armor class, proficiency,
    change, prices as text, the focus list.
  - **Runtime:** `runtime/equipment.rs` (armor class, training, the STR minimum, `weapon=` /
    `armor=`), `runtime/money.rs` (the purse; `WorldState::money_cp` / `pay_cp` / `give_cp`, coins
    by worth in `WorldState.coins`), shops in purse mode (`ShopSpec::price`), weapon specs with
    finesse, versatile, reach, thrown, heavy and loading, two-weapon fighting with light weapons,
    Sneak Attack by finesse, untrained armor's disadvantage and no casting, material components,
    thieves' tools for locks, `Armor::ac_bonus` for characters without a stat block.
  - **Import:** the 237 items and five coins (`import_equipment`; packs unpack by *Give item*
    actions; `STANDS_FOR` maps library items to SRD indexes; generic icons on `srd_gear.png`, drawn
    by a script); class equipment through the same table, a focus for casters without one; the
    example gets the items its classes start with.
  - **Scripts / events:** `money`, `give_money`, `take_money`, `format_money`; *Has money*
    (`Cond::Money`), *Give money* / *Take money* (`Action::GiveMoney` / `TakeMoney`, appended; added
    after the first review to close the gap: events check *Has money*, then take it); console
    `money CP` / `pay CP`.
  - **Example:** see the CHANGELOG; a dagger, thieves' tools and a component pouch at Tilda's.

- [x] Magic items — `feature/srd-magic-items`

  **Goal.** The SRD magic items.

  **Design.**
  - **Data:** rarity, attunement (three items at most), charges and recharge, and effects
    through the spell effect vocabulary and conditions (a +1 weapon, a Ring of Protection, a
    Potion of Healing that rolls 2d4+2).
  - **Import:** the SRD magic item list.
  - **Library:** items without art get a generic icon per category until the library has art.

  **Engine.** Attunement and charges on items, the import, the item form; seven equipment slots
  for worn items; four new feature effects.

  **Decided while building.** Your decisions: seven new slots (neck, cloak, belt, feet, hands,
  two rings; not drawn), every item the rules vocabulary covers works (the rest are story items),
  a bonus field with six ready-made +1 items instead of the SRD's generic +N entries, attunement
  in the character window taking an hour. Passive effects reuse the class features' vocabulary
  (not the spell parts); using an item (potions) uses spell parts. A standing library item (the
  Red Potion) keeps its price. Attunement's key is *Take all*'s T (no new binding). Spell scrolls
  stay items that *teach* a spell.

  **As built.** One pull request.
  - **Data:** `srd/magic_items.rs` (`MagicItem`: rarity, attunement, `attune_by`, bonus, effects,
    charges, recharge, `destroy_on_last`, `spells: Vec<ItemSpell>`, `use_parts`, `use_seconds`;
    the SRD text parsers; `MAX_ATTUNED`), `ItemDef.magic`, `ItemStack.spent` (charges used: kept
    by inventory moves, chests and dropped piles, `DroppedItem.spent`), `EquipSlot` Neck, Cloak,
    Belt, Feet, Hands, Ring, Ring2 + `Equipment` fields, `FeatureEffect` Bonus (`BonusTo`),
    AbilityScore, CheckAdvantage, Darkvision (`ClassTraits::add_effects`, also used by `of`),
    `Buff::AbilityScore`, `WorldState.attuned` / `ability_base` (items raise `abilities`; writers
    use `base_abilities_mut`) / `last_dawn`, `Companion.attuned`, `Training` `check_advantage` /
    `save_bonus`, `Cond::Attuned` (appended).
  - **Runtime:** `runtime/magic_items.rs` (items in effect, `hero_traits` / `who_traits` with the
    items' effects, attunement, charges, dawn, use parts as a transient spell, spells from items:
    `item_casts`, the Cast window's rows, `Combat.item_cast`, `Game.casting_item` for an item's
    DC); bonuses in `weapon_spec`, `armor_class`, `member_block`, saves, spell DC / attack; the
    character window's worn row, details line (`magic_line`) and T key; the companion window's
    second column.
  - **Import:** `import_magic_items` (`magic_rules` = the hand table, 112 items with rules; 209
    story items; `MAGIC_STANDS_FOR`, `PLUS_ONE`, `EXAMPLE_MAGIC`), icons `srd_magic.png` (17
    frames, drawn by a script).
  - **Not built** (moved to the *Backlog*: *Magic items, the rest*).

  **Verify.** Autoplay: attune, use charges, recharge at dawn; a +1 weapon's maths in a fight.
  Checked on a scratch copy: three attunements (an hour each), the fourth refused; AC and saves
  with a Ring and a Cloak of Protection; CON 19 and max HP with an Amulet of Health; a Belt of
  Giant Strength only while attuned; potions of giant strength (STR 21 for an hour) and of
  healing; the +1 Longsword's +5 / 1d10+3; the wand's Magic Missile at slot 2 from the Cast
  window (7 to 5 charges); dawn recharging 1d6+1; screenshots of the character window, the
  Sheet, the Cast window and a companion's window; the editor's Magic item section (a screenshot
  and a UI test: a worn ring with attunement and charges, saved).

## 0.32.0 — SRD missing features

The SRD 5.1's rules that 0.28 to 0.31 leave out, collected so the ruleset is complete before the
reach releases. The list comes from the SRD's chapters (*Using Ability Scores*, *Adventuring*,
*Combat*, *Between Adventures*, *Gamemastering*, *Monsters*) compared with what the releases before
build, and from what the turn-based combat recorded as *Not yet*. Items may move into earlier
releases when they turn out small; anything still missing when a 0.28-0.31 item is built is added
here.

- [ ] Combat rules completed — `feature/srd-combat-rules`

  **Goal.** The combat chapter's remaining rules.

  **Design.**
  - **0 HP:** instant death from massive damage (what remains after 0 HP is at least the max HP);
    *knocking a creature out* (a melee hit that drops it to 0 can leave it unconscious and stable);
    **stabilizing** with a Medicine check (DC 10, an action) or a healer's kit, and *Spare the
    Dying*.
  - **Grappling completed:** the grappled creature's *escape* action (Athletics or Acrobatics
    against the grappler's Athletics), dragging or carrying a grappled creature at half speed, the
    size limit (at most one size larger) for grapple and shove.
  - **Creature size and space:** Large and bigger creatures take 2×2, 3×3, 4×4 tiles (movement,
    reach, cover, the overlays); squeezing into a smaller space; moving through a non-hostile
    creature's space is difficult terrain, through a hostile one's only when two sizes apart.
  - **Cover on DEX saves** (+2 / +5, like AC) and on area spells.
  - **Ready any action:** a readied spell (concentration on it), Dash, Disengage, a move; the
    trigger as the player chooses it.
  - **Seeing and being seen in fights:** lightly and heavily obscured areas (fog, darkness,
    foliage) from the lighting, the weather and decorations; unseen attackers and targets; hiding
    behind cover; the creature's senses (blindsight, tremorsense, truesight) against them.
  - **Difficult terrain from decorations** (undergrowth, rubble), not only from materials.
  - **Mounted combat** (mounting, controlled and independent mounts, falling off; riding needs the
    backlog's *Mounts*, built here when it hasn't been) and **underwater combat** (weapons with
    disadvantage, resistance to fire). Vehicles stay in the backlog.
  - **Conditions in full:** frightened can't move closer to the source of its fear, charmed can't
    attack the charmer, a grappler's speed 0 for the grappled, incapacitated loses concentration.
  - **The AI's spells:** characters and AI-played companions cast their spells in fights.
  - **The AI's other actions:** Help, Hide, Search, Grapple and Shove, and shooters that
    Disengage instead of shooting at disadvantage next to a foe (the AI only attacks, walks, dashes
    away, dodges and drinks potions now).
  - **Attacks in full:** several damage parts (a dragon's bite: piercing plus fire; the importer
    keeps only the first), riders on a hit (the wolf's bite: DC 11 STR save or prone), versatile
    damage with both hands free; opportunity attacks and Ready at a weapon's full reach (now only
    from the next tile).
  - **Surprise by the rules:** the hidden side's Stealth against the other side's passive
    Perception when a fight starts (now: whoever hadn't noticed the hero who struck first).
  - **XP for the party:** split among the party members that level, once companions have classes
    (0.30); now the hero gets it all.

  **Engine.** The fight's death and grapple rules, multi-tile creatures in `runtime/turns/`
  (reach, cover, pathing), obscurement from light / weather / decorations, the Ready choice,
  mounts.

  **Example.** The bridge troll (Large) holds its bridge; the bandits' camp at night is lightly
  obscured; Grumbold grapples.

  **Verify.** Unit tests of massive damage, the escape contest, multi-tile reach and cover;
  autoplay: a Large creature in a corridor, a stabilize by Medicine, an escape from a grapple, an
  attack from heavy obscurement; screenshots of a Large creature's tiles.

- [ ] Movement, senses and the environment — `feature/srd-environment`

  **Goal.** The *Adventuring* chapter's movement and environment rules, in and out of fights.

  **Design.**
  - **Movement modes:** climbing and swimming (half speed without a climb / swim speed, Athletics
    for hard ones), jumping (long and high jump from STR, Athletics to go farther), flying speeds
    (bats, birds, a *Fly* spell; hovering), crawling. Fliers cross water, pits and low obstacles
    (now they only walk at their flying speed on the grid), swimmers use water, burrowers the
    ground: the stat blocks' `speeds` already carry them.
  - **Falling:** 1d6 bludgeoning per 10 ft (at most 20d6), prone on landing.
  - **Breath and suffocation:** holding breath (1 + CON minutes), then CON rounds; underwater
    areas.
  - **Vision and light:** bright, dim light and darkness (dim light gives disadvantage on
    Perception by sight), darkvision exists; blindsight, tremorsense and truesight for creatures;
    the game shows what the hero can't see.
  - **Food and water:** days without food or water give exhaustion (an option: *Database ›
    Project › Rules › Food and water*); the example's food then counts.
  - **Travel:** forced march (CON saves past eight hours), travel pace (fast: −5 passive
    Perception, slow: can sneak).
  - **Carrying capacity and encumbrance:** STR × 15 lb (variant encumbrance as an option),
    pushing, dragging and lifting; items get a weight.

  **Engine.** Movement modes on the grid and while exploring (zones / materials for water and
  climbable walls), fall damage on ledges and pits, breath, light levels per tile, weights.

  **Example.** The river can be swum, the mine has a pit to fall into and a ledge to climb, the
  forest's clearing at night is dim light.

  **Verify.** Unit tests of jump distances, fall damage and carrying capacity; autoplay: swim the
  river, climb the ledge, fall into the pit (`status` HP and prone), a Perception check in dim
  light with disadvantage.

- [ ] Objects, traps, poisons, diseases and madness — `feature/srd-hazards`

  **Goal.** The SRD's hazards and the objects of the world by the rules.

  **Design.**
  - **Objects:** armor class and hit points by material and size (wood, stone, iron), damage
    thresholds, immunity to poison and psychic; props can be broken (a door, a chest, a barricade:
    a new *Breakable* part of the prop form) and forced open with a STR check.
  - **Traps:** the SRD's traps (collapsing roof, falling net, fire-breathing statue, pits, poison
    darts, poison needle, rolling sphere) as library props / zones: found by Perception or
    Investigation against their DC (passive Perception while walking), disarmed with Thieves'
    Tools, set off on a failure; saving throws and damage by severity.
  - **Poisons:** the SRD's poisons as items (contact, ingested, inhaled, injury), applied to a
    weapon or ammunition (an action), with their saves and conditions.
  - **Diseases:** cackle fever, sewer plague, sight rot: caught by a failed save, with their
    effects over long rests and their cures.
  - **Madness:** short-, long-term and indefinite madness from the SRD's tables, for events and
    monsters that cause it.

  **Engine.** Object stats on props, a trap kind (zone or prop) with detect / disarm / trigger,
  poison coatings on weapons, diseases as long-lasting conditions with a rest-based course.

  **Example.** The mine gets a pit trap and a dart trap, a goblin's poisoned arrows, a sewer rat
  that may carry sewer plague; the stone house's door can be broken.

  **Verify.** Unit tests of object damage thresholds and disease courses; autoplay: spot and
  disarm a trap (seeded), set one off, coat a weapon and poison a target, catch and cure a disease
  over long rests.

- [ ] Between adventures — `feature/srd-downtime`

  **Goal.** The *Between Adventures* chapter and the remaining character rules.

  **Design.**
  - **Lifestyle expenses** (wretched to aristocratic, per day) and services (inns, food, a
    messenger), from the SRD's prices.
  - **Downtime activities:** crafting by the SRD's rule (5 gp of progress a day, half the price in
    materials; the existing crafting stays for recipes), practicing a profession, recuperating
    (ends a disease or poison), researching, training (a new language or tool proficiency in 250
    days and 1 gp a day). A *Downtime* window spends days.
  - **Selling treasure:** gear at half price, art and gems at full (shops' sell rates).
  - **Inspiration:** granted by events and scripts, spent for advantage on one roll; the sheet
    shows it.
  - **Personality:** the background's personality traits, ideals, bonds and flaws (from the import)
    on the sheet and the creator, readable by conditions and scripts (a bond's quest).
  - **Feats:** the SRD's feat (Grappler) and the feat choice instead of an ability score
    improvement, if *Classes and levelling up* doesn't bring them.
  - **Multiclassing** by the SRD's rules (prerequisites, proficiencies, spell slots), if *Classes
    and levelling up* doesn't bring it (moved here from the backlog).
  - **Lore data:** the SRD's pantheons (a cleric's deity) and the planes, as reference data for
    texts and choices.

  **Engine.** The downtime window, lifestyle costs per day, inspiration on `WorldState`, the
  personality fields of backgrounds and the hero.

  **Example.** The inn charges by lifestyle; Brom teaches smith's tools over downtime; the elder
  grants inspiration for a kind deed.

  **Verify.** Autoplay: spend downtime training a tool and crafting an item (days and gold in
  `status`), gain and spend inspiration on a check (`rolls=` with advantage).

- [ ] Special abilities of monsters and classes — `feature/srd-special-abilities`

  **Goal.** What the stat block and spell vocabularies of 0.29 and 0.30 leave as text.

  **Design.**
  - **Monsters:** legendary actions and legendary resistance, recharge actions (*Recharge 5-6*),
    innate spellcasting, regeneration, pack tactics, swallow, multi-headed traits, damage
    transfer: the traits of the imported monsters that act, each as a runtime ability or a Rhai
    hook on the monster. They act from the `features` texts the import keeps on every stat block
    (0.29); save-based actions (a dragon's breath: a cone, a DEX save for half; a ghoul's
    paralyzing claws) become actions with an area, a save and their damage.
  - **Senses:** blindsight, tremorsense and truesight from the stat blocks' `senses` text, against
    hidden and invisible creatures and darkness.
  - **Shapes and summons** (moved here from the backlog): Wild Shape and *Polymorph* (a stat block
    taken over for a while),
    familiars (*Find Familiar*), conjured and summoned creatures as temporary party members.
  - **Reactions:** a reaction choice when one is possible (*Shield*, *Uncanny Dodge*, *Hellish
    Rebuke*, *Protection*), as a prompt in the player's turns and by rule for the AI.
  - **A count** in the manual of how many SRD monsters' traits act, and which stay text.

  **Engine.** A monster ability runtime (with Rhai for the rest), temporary stat block swaps, the
  reaction prompt.

  **Example.** The bridge troll regenerates (fire stops it); wolves use pack tactics; the
  priest's familiar.

  **Verify.** Autoplay: a recharge action, a legendary action between turns, a troll regenerating
  until burnt, a wild shape and back, a *Shield* reaction prompt.

## 0.33.0 — Everywhere

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

## 0.34.0 — Studio Comfort

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

## 0.35.0 — Learning

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

## 0.36.0 — A Bigger Library

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

- **Magic items, the rest** (left out of 0.31's magic items, for later consideration):
  - charges spent on a hit for more damage (a Staff of Striking's 1d6 force per charge);
  - items' reactions with charges (a Ring of Evasion: a failed DEX save succeeds, 3 charges; it gives
    plain Evasion now);
  - more damage against creature types (Dragon Slayer, Giant Slayer, Mace of Disruption, Holy
    Avenger: only their +N now; needs riders that check the target's type);
  - the fight AI casting from wands and staffs and drinking potions other than healing;
  - the story items' effects as engine features where several items share one (flying, invisibility
    at will, teleporting, summoning).
- **Right-to-left text** (Arabic, Hebrew): shaping and bidirectional runs in `runtime/text.rs`,
  after 0.24's fonts.
- **iOS export:** needs a Mac and an Apple developer account; after Android (0.33).
- **RPG Maker MV / MZ importer** (moved from 0.32, 2026-11-16: R(ust)PG2D follows the SRD rules,
  not the JRPG model): maps and art through the tile converter, the database mapped onto SRD
  definitions where it fits, events onto triggers; the RTP's files skipped (licensed for RPG
  Maker only). The full design block is in git history (ROADMAP.md, 0.32.0).
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
