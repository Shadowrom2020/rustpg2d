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
    `starter_history.ron` current (`cargo run -p rustpg2d-editor --example starter_history`; the
    tests fail otherwise), `cargo xtask check-version release/X.Y.Z`, pull request into master, merge after the four
    checks, merge the back-merge pull request.

## 0.17.0 — Peoples of the world

- [x] Update the asset library from the starter set — `feature/library-update`

  **Goal.** A menu entry brings a new version's starter assets into the creator's asset library:
  adds new ones, replaces updated ones, asks whether assets the starter dropped should be
  deleted, so nobody has to rebuild their library by hand.

  **Design.** *Assets › Update asset library…* (and *File › Update from the starter set…* in the
  Asset Library window, plus a banner when `open_library` left `starter.pending`). Untouched
  assets (definition and files match a version in `crates/editor/assets/starter_history.ron`)
  are replaced; assets the creator changed that the starter changed too are listed unticked
  (their version stays unless ticked); retired ids are offered for deletion (ticked when
  untouched and unused, refused while still used). The library folder is backed up to
  `<library>.bak-YYYYMMDD-HHMMSS` first; script functions the creator changed are kept.

  **Engine.** Agreed: `starter_update.rs` (history, plan, apply, backup), `STARTER_RULES` 9
  (`starter.pending`); editor `library_update.rs`, the history generator
  `crates/editor/examples/starter_history.rs` and the test `starter_history_covers_the_starter`.

- [x] Races — `feature/races`

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

  **Engine.** Agreed: `races.rs` (`RaceDef`: bodies per sex, looks, max HP / speed % / defense,
  starting skills, size, playable) + `defs/races.ron`, `RefKind::Race`, Database tab, renames,
  library; `CharacterDef.race`, `WorldState.player_race`, `SheetDef.cut`; the creator's Race row;
  `runtime/races.rs` (stats, starting skills, size of bodies listed by a race); the *Player race*
  condition and `player_race()`. The race's default faction comes with *Factions*. Done: human and
  goblin races in the example and the starter library.

  **Verify.** Unit tests (defaults, an old save without a race, overlay lookup by cut). Autoplay:
  the creator with each race (`status creator=`), stats and skills applied, a dwarf wearing the
  tunic's dwarf cut; shots of all races side by side.

- [x] Meshy art in the editor — `feature/meshy-art`

  **Goal.** Creators draw new art from the editor with their own Meshy account: they enter an API
  key in the editor's settings, describe a character (or an item icon, a prop, a portrait) and
  get a finished sheet in the project's style, without the Python scripts of
  `scripts/art/meshy/`. AI agents connected to the editor's agent server can do the same from a
  chat ("draw an orc shaman for the swamp camp").

  **Design.**
  - *Settings* (*Edit › Settings… › Meshy*, `EditorSettings`): the API key (stored apart from
    `editor.ron` in `<config>/meshy.key`, owner-only permissions; never written into projects,
    exports, logs or agent replies), the model (default `nano-banana-2-lite`, 3 credits a
    picture), a credit budget per session (default 60) and *Ask before spending*. A *Test key*
    button shows the balance.
  - *Two Meshy modes, one key*: **image-to-image** with the project's own sheets as style and
    layout references (characters: a walk grid, then an attack grid with the walk grid as the
    character reference, as the pipeline does), so new art matches what the project has;
    **text-to-image** (`POST /openapi/v1/text-to-image`, `remove_background`) for pictures without
    a reference (item icons, portraits, painted images for *Show image*). Tasks are polled (or
    followed over the SSE stream) and downloaded; failed tasks cost nothing.
  - *Converter in Rust* (editor `art/` module): the steps of `scripts/art/meshy/sprites.py`
    (figure extraction, size classes, side rows and flipping, faded frames, idle / hurt frames,
    palette, shadow) and icon / portrait cropping, producing a `Body` sheet (complete figure),
    an icon on a sheet or a portrait sheet.
  - *Art window* (*Assets › Generate art…*): kind (character with or without attack, item icon,
    prop, portrait, image), description, size class (person, small, big, huge, animal) and
    attack style, the cost before spending, progress per picture, a preview of the result
    (walk / attack / idle / hurt rows animated) with *Redo* per picture and *Add to project*
    (the sheet and, for characters, a character definition with the body).
  - *Agent server tools*: `art_settings` (key set?, model, budget left, balance; never the key),
    `generate_art {kind, id, description, size, attack, confirm_credits}` starts a job and returns
    its id and cost at once (the call refuses without `confirm_credits` equal to the cost, or past
    the budget), `art_jobs` (progress of every job; MCP progress notifications while a client
    waits), `art_result {job}` (the converted sheet as an image to look at, frame problems found),
    `art_apply {job, id}` (writes the asset and its definition like `write_asset` +
    `upsert_def`), `art_redo {job, picture}`. The editor shows every spend in its status bar and
    the Art window lists agent jobs too, so the creator sees what an agent draws.
  - *Chat*: the creator's chat is their AI assistant connected to the agent server (Claude
    Desktop / Code, Cursor, …): it describes, starts, watches, judges (`art_result` returns
    images) and applies, asking before it spends. An LLM chat inside the editor would need a
    second AI API and key; it is not part of this item.

  **Engine.** Editor code (agreed): settings (`EditorSettings` + `meshy.key`), the `art/` module
  (`meshy.rs` client, `mod.rs` jobs on worker threads, `convert.rs` the converter, `ui.rs` the
  windows, `agent.rs` the tools), `ureq` 3 with rustls (ring) as the HTTP client. No engine
  changes. As built: jobs are polled (no SSE stream, no MCP progress notifications: `art_jobs`
  shows progress); icons use image-to-image with the project's *items* icons as reference
  (text-to-image without one), props / portraits / images text-to-image.

  **Verify.** Unit tests of the converter on stored Meshy grids (no network in CI); with a key: a
  character and an item icon from the Art window and from an MCP client (cost refused without
  confirmation, budget respected, progress, result image, applied asset in play); the key never
  in project files, exports or replies.

- [x] Factions — `feature/factions`

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

  **Done.** As designed, with two refinements found while building the example: *Hostile*
  characters spare friends of their faction (standing 20 or more) instead of overriding the
  faction completely, so library monsters stay dangerous in projects without factions and an
  orc player is still welcome in the orc camp; a race without a faction leaves its players
  faction-less. Only humans, elves and dwarves are playable in the example; the villagers belong
  to the humans' faction. Faction ids are fixed once created (no
  rename). Library bundles don't carry the project's factions yet: library characters name
  theirs, which stay dormant until a project defines them.

- [x] Elves, dwarves, orcs and trolls — `feature/race-art`

  **Goal.** The new races in the example and the starter library, in the 2× style.

  **Design.** Bodies (♂/♀ for elves, dwarves, orcs; one troll body), their outfit / armor cuts
  where the human ones don't fit (dwarves, trolls), race hair and beards (dwarf braids, elf
  hair), portraits.

  **Engine.** None (content).

  **Done.** Drawn with Meshy (`scripts/art/meshy/`) as complete figures like 0.16's characters, so
  no outfit cuts or race hair were needed (the engine supports cuts for layered race bodies
  later); no portraits (dialogs show the head and shoulders). Elf, dwarf and orc bodies per sex,
  one troll body (3x art at race size 1.5), the orc chief, a dwarf trader, an elven herbalist.

  **Example.** A dwarf merchant at the mine (ores and tools), an elven herbalist travelling the
  wilds (schedule), an orc camp in the wilds (enemies, a chief, loot), a troll guarding a bridge
  in the wilds (a mini-boss with *HP below* triggers), German texts; races and art to the starter
  library.

  **Verify.** Autoplay through each new place; shots; the starter-library test.

- [x] Characters from real art — `feature/real-art-characters`

  **Goal.** Characters are complete figures: drawn, generated with Meshy or imported. The old
  layered system (bodies with skin / hair ramps, hair, beards, outfits, armor and held-item
  overlays, cuts) is removed. A creator marks **colour features** on a figure (skin, hair,
  clothing…) and the colours each may take; NPCs and the in-game character creator use them.

  **Design.** `SheetDef.features` (name, the art's shades, offered colours); `Appearance` = body
  + colour per feature; recolouring maps the picked shades onto the chosen colour by their
  brightness. Equipment counts in play but isn't drawn (the art shows it). The character editor:
  figure, colours, preview, equipment and carried items. *New character…* starts from AI
  generation, a new 16-row sheet to draw, an import (LPC with attack and hurt, Aseprite tags) or
  an existing figure. The creator: Race, Sex, Body and a colour row per feature.

  **Decided.** Features are picked from the art's own colours (no repainting); equipment isn't
  drawn on figures; this ships in 0.17.0. **Breaking:** save games, projects and libraries of
  earlier versions load, but layered characters keep only their body, worn / held overlays are
  gone and saved player looks keep only the body.

  **Engine.** Approved by the user: removal of the layered system, colour features, the
  creator's colour rows.

  **Verify.** Creator rows and the recoloured player in autoplay; NPC colours; the sheet form's
  colour features; New character through each source; old fixtures still load; *Update asset
  library* retires the layered assets.

  **Done.** As designed. The example's and the starter library's figures got their features by
  picking their palette colours; where hair shared shades with belts and boots, the head-area hair
  pixels were moved one step off (`+1` blue) so the parts separate. The male dwarf has only skin
  (his beard and sleeves share their reds). LPC imports bring attack and hurt along, Aseprite tags
  named after the animations make figures; *Update asset library* lets retired assets go that only
  replaced assets used (the layered bodies the races listed). 64 layered sheets left the example,
  107 the starter library.

- [x] Animation workflow in the sprite editor — `feature/real-art-characters`

  **Goal.** Drawing and fixing character animations without leaving the editor.

  **Design.** Figure sheets always open with the 16-row layout; an animation panel (walk / idle /
  attack / hurt per facing, play / pause / step, fps, frame strip, colour preview); onion skin;
  duplicate frame, copy / paste / clear / mirror rows (Left → Right), fill idle and hurt from
  the standing frames, nudge a frame by a pixel, replace a colour in a frame / row / sheet; all
  undoable. Agent tool `edit_frames`; editor script `px…` commands.

  **Engine.** None (editor).

  **Verify.** Editor scripts with screenshots of the panel and onion skin; row mirror checked
  pixel by pixel; undo; the agent tool through an MCP client.

  **Done.** As designed, on the same branch as *Characters from real art* (it builds on figures):
  the frame operations live in `crates/editor/src/frame_ops.rs` (unit-tested), shared by the
  sprite editor's buttons, the `px…` script commands and `edit_frames`. The colour preview stays
  in the sheet form's *Colour features* (a preview row per offered colour).

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented). Packaging work: needs the user's go-ahead.
