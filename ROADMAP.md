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
