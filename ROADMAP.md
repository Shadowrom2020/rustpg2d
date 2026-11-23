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
4. **Content:** one content release, right after the rules (moved before *Reach* on 2026-11-20).
5. **Reach:** editor comfort, then a way in for new creators, then the platforms (*Everywhere*
   moved to just before 1.0.0 on 2026-11-23, the user's decision).
6. **1.0.0:** the formats freeze.

Each version has a theme, and items may move between versions.

Almost every item below changes the engine. The user agreed to the direction with this roadmap,
and each item's *Engine* line names the addition. When an item starts, confirm its details with
the user in a short plan before changing Rust code (CLAUDE.md, *Content first*). Build whatever
can be content as content: the library and example parts of each item.

## 0.35.0 — A New View

Scripts without code, and the move from top-down to an isometric 2.5D view in the tradition of
the western RPGs (Baldur's Gate, Planescape: Torment).

- [x] Visual scripts — `feature/visual-scripts-model`, `feature/visual-scripts-canvas`,
  `feature/visual-scripts-groups`

  **Goal.** Creators who don't write code build script functions from nodes: function nodes with
  input and output connectors wired together, turned into Rhai automatically. Node groups become
  new functions. (The user's request, 2026-11-23; answers in the session: nodes to Rhai only, its
  own mode tab, merge each part after CI.)

  **Design.**
  - **The asset:** a *visual script* (`defs/visual_scripts.ron`) is one script function: its id
    (the Rhai function's name), a label, parameters and an optional return value, the nodes (kind,
    position, the values typed into unconnected inputs) and the links between their pins.
  - **Pins:** flow pins (white: what runs next) and value pins typed *number*, *yes / no*, *text*,
    *list*, *options* or *any*; text pins that name something (an item, a sound, a map…) offer the
    project's ids as the API table's argument kinds do.
  - **Nodes:**
    - every script API function (the API table, grouped by the manual's sections): functions without
      a result are steps on the flow; queries (`has_item`, `flag`, `player_hp`) are value nodes
      without flow pins; the few with a result that also act (`check`, `roll`, `give_item`,
      `take_money`…) are steps with a value output;
    - flow: *Start* (the parameters), *Return*, *If*, *Sequence*, *Repeat* (with the count), *For
      each* (a list);
    - values: a typed value, maths, comparisons, and / or / not, joining texts, local variables
      (set / get);
    - calls of other visual scripts (groups) and of text script functions by name;
    - comments.
  - **Rhai:** every visual script becomes a `fn` in `scripts/visual_scripts.rhai`, written whenever
    the definitions are saved (marked generated: edit the nodes). Value nodes are written into the
    expressions that use them, a value used twice or made by a step goes into a `let`. The game
    runs it like any script: triggers, hooks, menus, spells and plugins call visual scripts by name.
    Nodes only turn into Rhai, not back; *Convert to text script* moves a function into a text
    script for good.
  - **Checks:** missing inputs, loops in values, types that don't fit, a name taken by the API or
    another function: shown on the nodes and in *Problems*; such a script is written as a stub
    that does nothing (with a warning in the log).
  - **The editor:** a *Nodes* mode tab: the visual scripts on the left; the canvas (pan, zoom,
    drag nodes, drag from a pin to another to link, right-click or Space for the node palette with
    search, Delete, undo, copy / paste); the selected node's inputs and the generated Rhai on the
    right.
  - **Groups:** select nodes, *Group into a function*: a new visual script with the cut links as
    its parameters and (at most one) result, replaced by a call node; *Open* goes into it.
  - **Renames and the library:** ids typed into node inputs are references (renames rewrite them,
    library export carries them), a visual script travels as an asset with the scripts it calls.

  **Engine.** `visual.rs` (model, node catalogue, code generation, checks), the defs file and its
  plumbing (project, release, library, refactor, agent server kinds), return types and the
  "acts" mark in the API table (a test checks them against Rhai's own signatures). The game's
  runtime is unchanged.

  **Example.** A few library functions rebuilt as visual scripts (e.g. a fountain that heals
  once, an ability check that opens a door), used in the example project.

  **Progress.** Part 1 (`feature/visual-scripts-model`): the asset, the code generation, the
  checks, renames and library, return types in the API table; agents write visual scripts with
  `upsert_def` (kind `visual_scripts`). Part 2 (`feature/visual-scripts-canvas`): the *Nodes* tab (list, canvas, palette, typed
  values, problems, the Rhai beside it, *Convert to text script*), Problems and quick search,
  manual chapter 20. Part 3 (`feature/visual-scripts-groups`): *Group into a function*
  (`visual::group`: parameters from the links coming in, one flow in and out, at most one result),
  *Open*, copy / paste / duplicate; the example's `ansel_blessing` and `rat_defeated` rebuilt as
  visual scripts (and in the starter library); visual scripts travel through the library as assets
  (a trigger running one brings it, it brings the text functions it calls).

  **Verify.** Unit tests of the code generation (every node kind, nesting, groups) that compile
  the result with Rhai; editor UI test of the canvas basics; the example's visual scripts run in
  autoplay; editor screenshots of the Nodes tab.

- [ ] Isometric view — `feature/iso-view`, `feature/iso-editor`, `feature/iso-props`,
  `feature/iso-walls`

  **Goal.** The game and the editor show the world isometric instead of top-down: diamond tiles,
  upright characters, buildings and furniture seen from the south-east and above. (The user's
  request, 2026-11-23; answers in the session: isometric **replaces** top-down, diamond tiles,
  buildings and furniture as rendered 3D models, a look study first.)

  **The look study** (2026-11-23, a throwaway copy in the scratchpad, no repository change):
  - Turning the game's `Camera2D` 45° and squashing it to half height projects everything drawn
    flat: the painted ground, flat decorations, rugs, bridges, the fight's grid and areas, the
    light map, and mouse picking (through the camera's inverse).
  - Upright things (characters, trees, lanterns, stalls) drawn unturned at their projected feet
    and sorted by `x + y` look right; round things (trees, rocks, barrels) work as pictures.
  - Characters only need the model camera at 30° (was 20°) and turned 45°.
  - The 11 example buildings as voxel houses on their props' footprints, rendered from the same
    angle, sit right in the village and hide what is behind them.
  - Found: interior wall pieces and counters become staircases; HP bars and other overlays are
    squashed; one sort point per building fails beside its sides; keys move along the map's
    axes; characters look small at zoom 1.

  **Design.**
  - **The world doesn't change:** square tiles of 32 px, maps, collision, pathfinding, the
    fight grid, zones, events, scripts and saves stay as they are. Only drawing and input change.
  - **The projection:** world (x, y) to screen (x − y, (x + y) / 2), so a tile is a 64 × 32
    diamond at zoom 1; characters and models are rendered at the same scale. Flat things
    (ground, flat decorations, rugs, grid, areas, lights, weather on the ground, fog of war) go
    through the projected camera; upright things are drawn at their projected feet in one
    batch (positions transformed on the CPU, not a camera per object as in the study).
  - **Depth order:** characters, projectiles and small props by `x + y` of their feet; props
    with a footprint (buildings, counters, beds) by their footprint box against the others (the
    usual isometric box order), so a character beside a building's side is drawn right.
  - **Upright overlays:** HP bars, names, speech bubbles, quest markers, damage numbers, the
    aim cursor's labels stand at the projected point above the head.
  - **Controls:** keys and the stick move relative to the screen (up = up the screen, the map's
    north-west), with eight facings drawn (the saved `Facing` stays four values; the renderer
    already turns walkers in eighths). Click to walk is unchanged.
  - **Characters:** the model camera at 30° and turned 45°; the atlas caches the same way.
  - **Props as 3D models:** `PropDef.model` (a static `.glb`) is rendered by the character
    renderer (cel shading, palette, outline) from the isometric angle and cached in the atlas;
    each state can name its own model. A placed prop turns in four steps (a decoration's
    `rotation` in the map's `Meta`, `flip` read as a quarter turn on old maps). Props without a
    model keep drawing their picture upright, so old projects still show everything.
  - **Walls and interiors:** wall sets (`PropDef.pieces` by neighbour mask) get one 3D segment
    per piece; walls south and east of the player, within a few tiles, are cut down to a low
    stub (or shown see-through, an option), so rooms read like the classic isometric interiors.
  - **The editor:** the map canvas uses the same projection for drawing, painting, placing,
    zones (diamond outlines), selection, regions, structures and the schedule editor; a
    *View › Top-down* toggle keeps a flat overview for painting large areas. The world map
    screen and the minimap stay top-down overviews.
  - **Map edges:** filler props (`black_void`, `dungeon_void`) are flat, so a room's floor ends
    in a straight diamond edge (upright, they made the zig-zag seen in the study); the map's
    border fades into the background over about a tile, as the old games' areas did.
  - **Old projects:** nothing is converted on disk; their square-cornered pictures stand upright
    until a model replaces them. Library assets keep their ids, so *Update asset library* brings
    the new models; *Problems* lists props shown as pictures that look square-cornered (their
    footprint is wider than a tile).

  **Engine.** `render/mod.rs` (the projection, upright batch, depth order), `runtime/mod.rs`
  `draw` and `camera`, overlays (`barks.rs`, `markers.rs`, `quests.rs`, `turns/ui.rs`,
  `juice.rs`), `runtime/input.rs` (screen-relative moves), `render/model.rs` (elevation, the
  turn, static prop models, four rotations), `decor.rs` + `map/io.rs` (`rotation` in `Meta`),
  `defs.rs` (`PropDef.model`, per-state models), the editor's `canvas.rs` and tools. Autoplay
  `status` prints `view=`; the map files' format is unchanged.

  **Verify.** Unit tests of the projection (world ↔ screen round trip, picking) and of the
  depth order (a character beside a building's side, behind it, in front of it); autoplay on a
  scratch copy: walking with keys in all eight directions, a fight with areas, a night with
  lights, an interior with cut walls, an old project with pictures only; editor scripts painting,
  placing and selecting on the projected canvas; screenshots of each.

- [ ] Isometric art with Meshy — `feature/iso-art-pilot`, `feature/iso-art`

  **Goal.** The starter library's and the example's ground materials, buildings, props and
  nature for the isometric view, made with Meshy where it is strong. (The user's request,
  2026-11-23.)

  **The look** (the user's choice after the pilot, 2026-11-23): painted and more realistic, still a
  bit retro, like the Infinity Engine games. Buildings and props are pre-rendered with light: a
  sky and a high warm sun from the north-west, soft shadows cast on the ground and kept in the
  picture, at twice the world's pixel density. Ground materials are painted textures; trees,
  rocks and plants are painted pictures at the same angle and light; the kit's figures get
  smoother shapes and painted textures (item *Painted figures* below).

  **The pilot** (304 credits): the red cottage and the chapel, a market stall, well, anvil,
  tavern table, bed and cart, and grass, cobblestone, wood floor and water. Concepts drawn from
  a voxel blockout (angle, footprint, door side) and the old picture (materials) keep shape and
  door; Meshy's models fit automatically (squared to the grid, the side facing south chosen by
  comparing the outline with the concept). Learned: props sized by guessed footprints come out
  small (the well), so props get a real size in metres against a 1.75 m character; Meshy's
  textures carry their own light, so renders add light gently (no studio shading on top); the
  outline can't tell a symmetric house's front from its back, so the colours are compared too;
  the grass needs another try.

  **What Meshy makes, and what not** (the library's 247 props and 46 materials, counted
  2026-11-23):

  | Kind | Count (example) | Route | Credits each |
  |---|---|---|---|
  | Buildings | 11 (11) | concept picture (2 tries) + image-to-3D | 4 + 4 + 30 = 38 |
  | Props with corners: stalls, furniture, workshops, carts, wells, statues, signs… | 70 (67) | concept picture + image-to-3D | 4 + 30 = 34 |
  | Ground materials | 46 (30) | painted texture from the current one, made seamless, up to 256 px | 6 |
  | Modular pieces: walls, roofs, fences, doors, railings, stairs | 74 (48) | the figure kit, by script (they must line up on the grid exactly, Meshy's models don't) | 0 |
  | Round or thin things: trees, rocks, plants, barrels, lamps, signposts | 82 (56) | painted pictures: image-to-image from the current picture, at the isometric angle and light | 4 to 6 |
  | Variants (chairs, stools and beds per facing, trapped / locked chests) | 10 | turns and states of one model | 0 |

  **Budget.** About 418 (buildings) + 2,380 (props) + 410 (nature) + 276 (materials) = 3,484
  credits, plus about 15 % for redraws: 4,000. The account has 5,476 after the pilot
  (2026-11-23). Spending always asks first: every batch's cost is shown and confirmed.

  **The pipeline** (`scripts/art/meshy/`, beside the character scripts; key and work folder as
  there):
  - `iso_cast.py`: each asset's id, kind, footprint (from the prop's hitbox), height class and
    description; the current top-down picture is its reference.
  - **Concept:** image-to-image (Nano Banana 2.1) with the current picture and the style
    references (the approved pilot building, a kit figure rendered isometric): "isometric
    three-quarter view from the south-east, 30° from above, medieval fantasy, painted
    textures, one object centred on plain white, no ground, no shadow"; buildings with the
    door in the middle of the front (south) wall, where the map's portal is.
  - **Model:** image-to-3D (`should_texture`, `should_remesh`; 3,000 triangles for props,
    8,000 for buildings, under the engine's 32k vertices per piece).
  - **Fit** (Blender headless): squared to the grid, the side facing south chosen by outline and
    colours against the concept, scaled to its size in metres (buildings: to their footprint),
    standing on 0, the origin at the front edge's middle (the decoration's point), compacted.
  - **Render** (Blender Cycles, headless): the isometric camera, sky and sun, a ground that only
    catches shadows, at 2 pixels per world pixel; the picture's anchor (where the decoration's
    point lies) is written beside it. Props that turn get four pictures. The engine draws them
    like any picture with an anchor, so the game needs no lighting of its own for them.
  - **Materials:** painted, made seamless (cross-faded with a half-turned copy), 64 to 256 px so
    the repeat isn't visible.
  - **Nature:** image-to-image from the current picture with the rendered props as style
    references (angle, light, shadow), background removed, anchored at the foot.
  - `gen.py`'s state file keeps every task id: a rerun never pays twice.
  - **Apply** through the editor's agent server (`write_asset`, `upsert_def` with the model,
    `library_send {replace: true}`), the starter history regenerated.
  - **Look check:** `model_sheet` renders each batch isometric in four turns as a contact sheet;
    a model that doesn't fit is redrawn once, then built by the kit instead.
  - **Order:** the pilot, then the example's buildings and props, then its materials, then the
    library-only assets.

  **Editor.** *Assets › Generate art…* gets a *Prop / building* kind with the same steps
  (concept, model, fit), so creators make their own props the way the library's were made.

  **Example.** All its buildings, props and materials replaced under the same ids (maps keep
  their placements; buildings' footprints are their hitboxes, so collision and portals stay);
  interiors rebuilt with kit walls.

  **Verify.** Every model passes `model_check`; the example's maps screenshot isometric
  (village, forest, the tavern, the mine) and played through in autoplay; *Update asset
  library* on a scratch copy of an older library brings the new models.

- [ ] Painted figures — `feature/painted-figures`

  **Goal.** Characters that match the painted, lit world while keeping what the kit gives:
  one shared skeleton and clips, swappable hair, beards, outfits and worn armour, the creator's
  choices. (The user's choice, 2026-11-23: kit figures painted, not Meshy characters and not the
  voxel look.)

  **Design.**
  - **Shapes:** the kit builds bodies and pieces as smooth low-poly meshes (lofted limbs, torso
    and head shapes on the same joints) instead of voxel blocks; builds, slots and piece names
    stay, so definitions, saves and the creator keep working.
  - **Textures:** each body and piece gets UVs from the kit and a painted texture from Meshy's
    retexture with the original UVs kept (a style prompt plus the pilot's renders as style
    reference), so the texture fits the skinned mesh the kit wrote. Colour features keep working:
    the recolour maps the texture's shades of a feature onto the chosen colour.
  - **Light:** the character renderer lights figures like the pre-rendered props (sky, sun from
    the north-west, soft shading, a soft shadow), at the isometric camera (30°, turned 45°).
  - **Monsters:** the kit's creatures get the same treatment (smooth shapes, painted textures).

  **Engine.** `render/model.rs` (textured lighting instead of the cel step, the shadow), the kit
  scripts (`kitlib.py` smooth meshes and UVs, `figure.py`, `creatures.py`), a retexture step in
  `scripts/art/meshy/`.

  **Verify.** Kit tests (manifest, vertex limits); `model_sheet` contact sheets of every build
  and piece; autoplay: the creator with hair and outfit choices, equipped armour shown, a fight;
  screenshots next to the painted props.

## 0.36.0 — Learning

A way in for new creators: tutorials and small games to learn from (moved from 0.35.0 on
2026-11-23, the user's decision).

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

## 0.37.0 — Everywhere

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

- **Between adventures** (moved from 0.32.0 on 2026-11-20, for later consideration): the SRD's
  *Between Adventures* chapter and the remaining character rules. Its design as planned:
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
    on the sheet and the creator, readable by conditions and scripts (a bond's quest); the hero's
    alignment on the sheet.
  - **Feats:** the feat choice instead of an ability score improvement exists (Grappler, the SRD's
    one); more feats would be the project's own.
  - **Multiclassing** by the SRD's rules (prerequisites, proficiencies, spell slots;
    `ClassDef.multiclass` already holds the prerequisites).
  - **Lore data:** the SRD's pantheons (a cleric's deity) and the planes, as reference data for
    texts and choices.
  - **Engine:** the downtime window, lifestyle costs per day, inspiration on `WorldState`, the
    personality fields of backgrounds and the hero. **Example:** the inn charges by lifestyle;
    Brom teaches smith's tools over downtime; the elder grants inspiration for a kind deed.
    **Verify:** autoplay spends downtime training a tool and crafting an item (days and gold in
    `status`), gains and spends inspiration on a check (`rolls=` with advantage).
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
- **iOS export:** needs a Mac and an Apple developer account; after Android (0.37).
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
  faster on roads; with them the SRD's **mounted combat** (moved from 0.32.0 on 2026-11-20:
  mounting and dismounting, controlled and independent mounts, falling off, the lance's two hands
  unless mounted).
- **New Game+:** start again from a finished save, keeping level and items.
- **Seasons** (moved from 0.20.0):
  - a year of N days;
  - tile variants per season (autumn / winter tiles without repainting);
  - crops by season and snow in winter;
  - a *Season is* condition and `season()`;
  - the full design block is in git history (ROADMAP.md, 0.20.0).
