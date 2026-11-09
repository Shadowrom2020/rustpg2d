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

## 0.12.0 — Quality and reliability

A pass over the implementation instead of new content: the runtime's hot paths, everything that
could lose or refuse data, automated checks of the running game, and the duplicated code that lets
copies drift apart. The recap of the architecture lives in `Documentation/architecture.md`. One
pull request (`feature/quality-and-reliability`) with one commit per stage, so the history stays
bisectable; the version number (PATCH or MINOR) is decided when the release is cut.

- [x] Quality and reliability pass — `feature/quality-and-reliability`

  **Goal.** Fewer wasted frames, no data lost or refused because of one bad entry, a game that
  runs in CI, and one implementation of each rule.

  **Design.** Twenty-five stages in four groups. *Small fixes and measurement:* walkers re-plan once
  per blockage and companions pause between failed searches; save/load edge cases (quick-load
  during a fade, `resuming`, once-keys of runtime uids, f32 phase seeds); a `Perf` sampler
  (`fps=`, `frame_ms=`, `astar=` in `status`). *Reliability:* atomic `sys::write` (tmp + rename),
  map geometry and lz4 size validation, tolerant defs / map-object loading with a `.bak` and a
  banner in the editor, MCP `catch_unwind` + reply timeout, a Windows start-up error box, the
  release profile and a pinned toolchain. *Tests and CI:* old-format fixtures (0.9 defs and maps,
  a frozen v1 map, crafted 0.10 saves) under `crates/engine/tests/fixtures`, a *Runtime smoke* job
  with Xvfb and autoplay plus `mcp_smoke`. *Performance and reuse:* `Default` + `fresh()` for the
  definition types, one kind enum (`RefKind`) with a `Def` trait and `with_def_kind!`, `grid.rs`
  for pixel ↔ tile conversions, a verified id index in `Database`, a per-frame occupancy grid for
  planning and collision, faster A*, fewer per-frame allocations in drawing, `walk_home` on
  `walk_to_tile`, `Action::walk` / `Project::for_each_trigger`, the script API table in the
  engine, shared UI widgets (`ScrollList`, slot grids), small utilities (base64, `hour_text`,
  path safety, RON writer) in one place each.

  **Decided while building.** Done in twenty-five staged commits on the one branch: the twenty
  planned stages, then the five pieces first set aside (a shared `ScrollList` for the shop,
  crafting and menu lists; one slot grid and drag state for the inventory and companion windows,
  which gave the companion window its stack counts back; the `mcp_tools!` macro that declares the
  54 agent tools once; one Rhai lexer, `script_refs::lex`, under the highlighter, the reference
  finder and the function lists; and `Project::for_each_trigger` with `TriggerSite` for the
  project check, the translation template and the editor's name collectors). The runtime smoke
  job is advisory until it has been green on a few pull requests. `walk_home` now shares
  `walk_to_tile`, so a character's `home` moves only when it arrives somewhere, not with every
  step. Verified end to end besides the per-stage checks: a chaser lured past its leash walks home
  (`ai=` goes hunting → returning → gone), and the Windows game shows its start-up error box
  (cross-built in a container, run under Wine).

  **Touch points.** `runtime/` (mod, patrol, ai, companions, mouse, lights, combat, npc_use),
  `path.rs`, `defs.rs`, `refactor.rs`, `project.rs`, `sys.rs`, `map/io.rs`, `events/mod.rs`,
  `script_refs.rs`; editor `mcp/`, `app.rs`, `database.rs`, `wizard.rs`, `main.rs`; `ci.yml`;
  `Documentation/architecture.md`, README, CLAUDE.md, CHANGELOG.

  **Verify.** Per stage: build, clippy, tests and the scripted check named in the plan (autoplay
  `status` fields, screenshots, `mcp_smoke`); before / after `fps=` and `astar=` for the village
  at noon and at night, a villager blocked in a doorway and a companion below a watchtower; the
  user's saves, the fixtures and a copy of the user's library keep loading.

## Backlog (unscheduled ideas)

- Code signing and notarization of the Windows and macOS games (today: Windows unsigned, macOS
  ad-hoc signed; documented).
- Gamepad button glyphs in the HUD and hints (Steam Deck); rebinding gamepad buttons.
- Companions learn spells from scrolls and books given to them and cast heal / bolt spells by
  the rules of *Companion inventory* (`Companion.spells`, a mana pool from the definition).
- Activities that produce something: the fishing NPC sells fresh trout at a market stall the
  next morning (a trigger at the end of an *NPC routines* activity).
