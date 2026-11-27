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
5. **Reach:** editor comfort, then the editor rewritten on a new foundation (*The Studio*, the
   user's request of 2026-11-26), then a way in for new creators, then the platforms
   (*Everywhere* moved to just before 1.0.0 on 2026-11-23, the user's decision).
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

- [x] Isometric view — `feature/iso-view`, `feature/iso-editor`, `feature/iso-props`,
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

  **Progress.** Part 1 (`feature/iso-view`): the game's projection (`render/iso.rs`: ground camera,
  upright cameras at the feet, depth order with footprints), overlays standing or placed above the
  feet, screen picking of entities, keys / stick / fight cursor / photo camera along the screen,
  figures at 30° turned to the diamonds, flat filler props, the border fade, `status view=`.
  *Decided while building:* each standing thing is drawn through its own upright camera (one draw
  call each) instead of a CPU-transformed batch: their drawing code keeps its world coordinates.
  Part 2 (`feature/iso-editor`): the editor's canvas through the same view (tools, zones, previews
  and the grid on the diamonds; entities, decorations, ghosts and outlines upright in depth order;
  picking on the screen; arrow keys and dragging along the screen), *View › Top-down* (I).
  Part 3 (`feature/iso-props`): `PropDef.model` / `PropState.model` drawn through the figure
  renderer (`Assets::draw_prop`, origin at the hitbox's middle, frames grown for wide objects),
  `PropDef.turns` (pictures from four sides), turns by the placed prop's `facing` and, for
  decorations, the two high bits of `Decor.state` (`Decor::turn`; *decided while building*: the
  map format and older readers stay as they are), hitboxes turned, R in the editor.
  Part 4 (`feature/iso-walls`): `PropDef.block` draws walls and counters as boxes on their hitbox
  with their picture on the faces (`iso::draw_block`), ordered by the whole footprint, cut to a stub
  in front of the hero (`iso::hides`); the library's and the example's walls and the counter are
  blocks. *Decided while building:* blocks instead of a 3D model per wall piece (no art needed, the
  existing wall pictures become the faces; kit-made wall pieces can still come with the art); the
  cut-away is always a stub (no see-through option yet).

  **Verify.** Unit tests of the projection (world ↔ screen round trip, picking) and of the
  depth order (a character beside a building's side, behind it, in front of it); autoplay on a
  scratch copy: walking with keys in all eight directions, a fight with areas, a night with
  lights, an interior with cut walls, an old project with pictures only; editor scripts painting,
  placing and selecting on the projected canvas; screenshots of each.

- [x] Isometric art with Meshy — `feature/iso-art-pilot`, `feature/iso-art`

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

  **Changed 2026-11-24 (the user's choice).** Ordinary buildings are no Meshy models: they are
  built from parts in the editor (cheaper, and creators make their own; the chapel too, see below).
  One-offs (the windmill, the bell tower, the watchtower), props with corners, nature and the ground stay with
  Meshy (about 3,000 credits); fences, walls, railings and stairs are kit pieces. The figures'
  retexture waits until the account is topped up.

  **Progress.** Part 1 (`feature/buildings`): `PropDef.building` (`BuildingShape`: walls on the
  hitbox, gable / hip / shed / flat roofs, tiled pictures, doors, windows, chimneys) made into a
  still 3D model by `building.rs` and drawn like a prop's model; the prop form's *Building* section
  (plan with clicks, live preview, *Copy the look of…*); the example's and the library's ten houses
  and the chapel as buildings with the existing wall and roof pictures (the pilot's Meshy chapel was
  tried: its texture smudges the door and the stained glass; big models keep 512 px textures). *Decided while building:* one generated model per building prop instead of
  tile pieces, so maps, hitboxes and portals stay; the editor can't run Blender, so the engine
  builds the mesh. The example's interiors got thin back walls (library `*_thin` / `*_thin_side`
  props; the old two rows of wall blocks became black filler) and the *Front Door* became a thin
  block that covers a building's own door (the user's feedback, 2026-11-24). Part 2
  (`feature/cover`): ground shadows of still 3D models, the see-through cover (props in front of the
  hero, companions and nearby foes fade); part 3 (`feature/fences`): fences and hedges as thin
  blocks along their line (*decided while building:* blocks with the existing pictures instead of
  kit 3D pieces: they line up exactly and need no art). Part 4 (`feature/iso-art-batch1`, the
  user's choice of a smaller first batch, 2026-11-24): the windmill, the bell tower, three stalls,
  the notice board, the statue and the bench (280 credits: 10 concepts at 4, 8 models at 30; balance
  5,146), plus the pilot's well, anvil and tavern table; the scripts and results are kept in
  `scripts/art/meshy/work/iso_batch1/` (git-ignored). *Learned:* concepts whose front faces east
  (the bench, the notice board) need a quarter turn after fitting; hitboxes stay, the model's front
  edge goes to the feet. The user decides how to go on: "do 1 and 2" (the rest of the props, then
  nature and ground). Part 5 (`feature/iso-art-props`): 40 more props and 6 state models (1,564
  credits; balance 3,582); turned copies for chairs, stools, beds, the chest variants and the east
  gate; flames stay pictures. *Learned:* the outline can't tell a chest's front from its back, so
  every model is checked by eye against its concept (four needed a manual turn). Part 6
  (`feature/iso-art-nature`): 72 painted pictures and 18 state pictures (368 credits; balance 3,214),
  cut out, scaled to the old picture's height, the foot at the frame's bottom middle. The pictures' shadows came later (`feature/picture-shadows`: drawn at run time, the picture
  sheared along the sun, rather than baked: no anchor field needed). Part 7 (`feature/iso-art-ground`): 42 painted ground textures
  (172 credits incl. a second grass; balance 3,042) from the old tile repeated 4 x 4 as the
  reference, so every stone and plank keeps its size; 128 px, seamless by cross-fading with a
  shifted copy. Later parts (2026-11-24/25): ground shadows of 3D props and the see-through cover (#325),
  fences and hedges (#326), the east gate, wall torches and the mine (#331, #332), fences, posts, soil
  plots and crops as models (#333), structures edited and drawn as in the game (#334), obsolete 2D art
  and tools removed (#335-#337), pictures rendered from models (#339), the Build tool (#340, #341),
  pictures casting shadows (#342). Part 8 (`feature/iso-art-finish`, 252 credits; balance 2,686): the
  last top-down art painted (fire and light props per state, doors, cave mouth, dungeon bits), painted
  textures for every wall block and the buildings' walls, roofs, doors and windows, a livelier grass,
  rock cliffs; the pilot's cart and market stall as models; the library's town house, brick shop,
  watchtower, lookout tower, dungeon rooms, prison cell, fences and hedge rebuilt; tile-era pieces
  retired. Part 9 (`feature/art-prop-kind`): *Generate art* gets the *Prop / building 3D model*
  kind (concept, image-to-3D, stood on the ground and sized to the hitbox in the editor: no Blender
  needed). Done 2026-11-25. Afterwards (`feature/iso-weak-spots`, 16 credits; balance 2,670): plaster
  textures repainted, railing and prison bars as kit-built 3D pieces, rock cliffs as a rocky 3D tile,
  picture shadows starting under the painted thing (props seemed to float).

- [x] Painted figures — `feature/painted-figures`

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

  **Progress.** Part 1 (`feature/painted-figures`, no credits): the kit's shapes smoothed by Blender
  after the voxel build (`scripts/art/kit/smooth.py`: welded, remeshed, Laplacian-smoothed; hair,
  beards and hats together with a plain head of their size; a fallback to gentle smoothing, then to
  voxels, where a thin part would melt; colours by the nearest voxel face, weights carried over),
  applied to the library and the example; soft lighting in the model shader, a softer outline, a
  shadow under figures. *Decided while building:* smoothing the voxel sculpts instead of lofting new
  shapes in `kitlib.py`, so every build, piece and creature stays as it is designed. Next: the
  painted textures (Meshy retexture with the original UVs: needs the kit to write UVs, and credits).
  Part 2, the painted textures, was tried and dropped (2026-11-26, the user's decision; 20 credits,
  balance 2,634): the human head and the commoner's clothes got UVs (Blender smart project) and a
  Meshy retexture with the original UVs (10 credits each). At the game's size (about 70 px) the gain
  was small: Meshy's painting added folds, a collar and a belt but moved the colours (the blue tunic
  went dark teal, the face darker), and the kit's own colours with only Meshy's light and shade
  looked almost like the plain figures. The item ends with part 1.

  **Verify.** Kit tests (manifest, vertex limits); `model_sheet` contact sheets of every build
  and piece; autoplay: the creator with hair and outfit choices, equipped armour shown, a fight;
  screenshots next to the painted props.

## 0.36.0 — The Studio

The editor rewritten from the ground up, on a new foundation, to be the best possible place to
make a game in (the user's request, 2026-11-26: "the editor works, but is not intuitive and the UI
lacks convenience"). Everything old goes; libraries, architecture and compatibility may change.

**Decided with the user (2026-11-26).**
- **Foundation:** the editor is an eframe application (the current egui) drawing with wgpu. The
  engine's renderer leaves macroquad, so that the game draws inside the editor (map views,
  previews, a play panel) and the editor is no longer held at egui 0.31 by `egui-macroquad`.
- **Older projects:** the new editor opens projects saved by 0.35, through one conversion. Every
  converter for older versions is removed. 0.35.0 is the bridge: an older project is opened and
  saved there first. 0.35.0 final is cut from `master` before this release's branch merges.
- **Sharing:** one pack format, and online sources the editor browses and installs from.
- **One long-lived branch,** `feature/studio`, merged into `master` once at the end (an agreed
  exception to AGENTS.md's short-lived branches).
  - A draft pull request into `master` is opened with the first push, so CI runs on every push.
  - `master` takes only bugfixes meanwhile; merge `master` into the branch after each.
  - The parts below are worked in order, each on this branch. A part is done when the branch
    builds, passes clippy and the tests, and its *Verify* is done; mark it then.
  - The old editor does not survive part 1 (it needs macroquad). Until the new one can do the
    work, look things up in the old code on `master` (`git show master:crates/editor/src/…`) and
    edit example content with a 0.35 editor in a `master` worktree, merged in as above.

**What every screen is judged by.**
1. **One workspace, no modes.** Maps, assets, scripts and settings are documents in tabs, side by
   side; panels are arranged freely and remembered.
2. **Found in two clicks, without searching.** Everything has one obvious place in the Project
   browser. Searching (Ctrl+P) is the shortcut, not the only way.
3. **Every reference is a link.** Wherever an id is shown it has a picture, opens on a click, and
   its picker can create what is missing in place.
4. **Every operation is a command:** it has a name, can be undone, can get a shortcut, and is
   reachable from the command palette, the editor script and the agent server alike.
5. **Work is never lost:** one undo history, a working copy saved as you go, recovery after a
   crash, *Undo* on a toast instead of "Are you sure?".
6. **Help is where the creator is:** every field explains itself, every empty place says what to
   do next, every problem is shown where it is and offers its fix.
7. **Seen as the game shows it:** every preview is drawn by the game's own renderer, and the game
   plays inside the editor.
8. **Old concepts leave the UI:** nothing is offered that the isometric, 3D-figure, SRD engine no
   longer uses.

Read the current documentation of eframe / egui, wgpu, winit and the audio library when part 1
starts and pin the versions then (looked up on 2026-11-26, APIs not read: egui / eframe 0.37,
wgpu 30, winit 0.31 beta, kira 0.12). wgpu's version is the one `egui-wgpu` uses.

- [~] Part 1: The engine on wgpu — `feature/studio`

  **Goal.** The engine draws, plays sound and reads input through a layer of its own instead of
  macroquad: the game looks and plays as in 0.35, and can be drawn into any number of views inside
  another application.

  **Design.**
  - **A new crate, `crates/gfx`:** the window and its events (winit), drawing (wgpu), text
    (fontdue, the rasteriser macroquad used, so glyphs keep their pixels), sound (kira; `lewton`
    still decodes music for the gapless loops of `audio.rs`), keys, mouse, touch and gamepads
    (`gilrs`; the browser's Gamepad API on the web), and files (`sys.rs`).
  - **Port by a look-alike API.** The runtime's calls keep their names and meaning
    (`draw_rectangle`, `draw_line`, `draw_circle`, `draw_triangle`, `draw_texture_ex`, `draw_mesh`,
    `set_camera`, `render_target`, `Texture2D`, `Image`, `Color`, `Rect`, `Vec2`, `KeyCode`,
    `get_time`, `screen_width`…), offered by the engine over a *current frame* (what is being drawn
    into, with which input). The runtime's files change their imports, not their logic. Tidying the
    API afterwards is not part of this item.
  - **The game is handed its world:** `Game` takes its input and its render target from the
    caller (`screen_width` is the target's width), so several games and views can live in one
    process, and none needs a window. The game binary is a winit loop around it; autoplay's input
    injection is the same path.
  - **Shaders in WGSL:** the painted ground (`render/ground.rs`), the figures' cel shader and
    outline (`render/model.rs`), the light map (`render/lighting.rs`), picture blend modes
    (`runtime/pictures.rs`), the screen effects (`runtime/juice.rs`) and the one in
    `runtime/mod.rs`. CPU skinning and the atlas pages of `render/model.rs` stay as they are.
  - **Web:** wasm-bindgen with wgpu's WebGL2 backend (WebGPU where the browser has it); the web
    side of `sys.rs` moves to web-sys (storage, the fetch of `data.pack`, the query string),
    replacing `web/mq_js_bundle.js` and `web/rustpg2d.js`; `xtask` builds the web runner (the
    bindgen step as a library, no tool to install). Rhai gets its default features back there.
  - **No software rendering for players:** a software adapter (llvmpipe, lavapipe, WARP) gets the
    existing message box, as today; `RUSTPG2D_ALLOW_SOFTWARE=1` lets tests and CI through.
  - **A device from outside:** `gfx` can be given the device and queue eframe created, and can
    make its own without a window (tests, the headless agent server's thumbnails).
  - **Screenshots:** autoplay `shot`, photo mode and save thumbnails read back from the target
    (no more vertical flip).
  - **Gone:** macroquad, miniquad, egui-macroquad, the `image` 0.24 pin and macroquad's own png.

  **Progress.** `crates/gfx` (device, the draw list, textures and render targets, materials in
  WGSL, cameras, shapes, text, input, the window loop on winit, sound on kira); the engine and
  the game on it (imports and the seven shaders; nothing else of the 101 files changed); the
  comparison set (`scripts/studio/shots.sh`, `compare.py`, `RUSTPG2D_FIXED_STEP`); the web game
  (wasm-bindgen, WebGL 2, `cargo xtask-web`, the browser's services in `gfx::web`). Still open:
  a real Windows machine, somebody listening to the sound. (`builds.yml` and `xtask dist` know the
  web pair; they are run again when there is an editor to package.)

  **Decided while building.**
  - **Draw calls wait for the frame's end.** macroquad sent them to the GPU at every camera
    change; the isometric view changes the camera per upright object. Each draw call keeps the
    projection it was made under, and one render pass is made per run of draw calls into the
    same picture. Clearing is an entry of the list.
  - **Everything is drawn into a picture** (`gfx::Screen`), which the window copies to its
    surface: the same path for the game, the editor's views and tests, and screenshots are a
    read-back. All pictures are `Rgba8Unorm` (no sRGB conversion, as OpenGL drew).
  - **OpenGL's conventions are kept where the engine counts on them:** render targets are drawn
    upside down (the engine's cameras and texture coordinates expect it), clip space depth is
    converted from -1..1, and the screen is drawn 1/128 pixel lower so that lines exactly between
    two pixel rows land on the row OpenGL chose (the UI's box outlines). `get_screen_data` still
    comes bottom row first and `export_png` still turns it over (so the note above about the
    flip is not done: tidying that API is left for later).
  - **macroquad's own code is reused** for shapes, text, the glyph atlas, cameras, colours and
    rectangles (MIT OR Apache-2.0; `crates/gfx/LICENSE-macroquad-MIT`), so layout and glyph
    pixels are the same. `quad-rand` stays as `gfx::rand`.
  - **Material shaders** get their declarations (uniform block, textures, vertex input) written
    in front by `load_material` from the declared uniforms, so a shader and its uniform layout
    can't disagree.
  - **X11 windows on Linux where there is an X server** (also under Wayland, through XWayland),
    as macroquad's were; `RUSTPG2D_WAYLAND=1` asks for a Wayland window. Measured on GNOME
    Wayland with the NVIDIA driver: a Wayland window waiting for the display misses frames (55 to
    59 a second with 40 ms hitches), and a covered one gets none at all, so a scripted run with
    the window behind others would stand still. Frames are drawn when the events are through;
    presenting waits for the display.
  - **The web game draws with WebGL 2 only,** not WebGPU: reading a picture back at once
    (screenshots, save thumbnails, photo mode) is a `readPixels` on wgpu's WebGL context
    (`Texture2D::get_texture_data`); wgpu's own read-back needs a turn of the browser's event
    loop, which the engine's callers don't give. WebGPU would need those callers to wait.
  - **32-bit indices into one vertex list:** WebGL 2 can't add a draw call's first vertex
    itself. Meshes still come with 16-bit indices (65,535 vertices each).
  - **The web runner is a pair** (`rustpg2d-game.wasm` + `rustpg2d-game.js`, written into
    `target/web` by `cargo xtask-web`: wasm-bindgen's second step as a library); the export
    copies both. The JavaScript files of `crates/engine/web` are gone: storage, the clock, the
    query string, gamepads, downloads and fetching are Rust (`gfx::web`). On the web the sound
    device opens with the player's first key or click (browsers allow it only then); loops
    started before (the music) start with it.
  - **The old editor** is out of the workspace (`exclude` in `Cargo.toml`) and stays in the tree
    to read until its parts are rebuilt.

  **Engine.** This part is the engine: `crates/gfx`, every `use macroquad` (101 of the engine's
  201 files, 82 of them in `runtime/`), `crates/game/src/main.rs`, `audio.rs`, `sys.rs`, `web/`,
  `release.rs` (the web export's files), `desktop.rs`, `glibc_compat.rs` checks, `xtask`
  (`dist`, the web build), the CI apt lines and the Flatpak permissions (ROADMAP step 11).

  **Verify.**
  - A fixed set of autoplay screenshots (title, village by day and night, a dungeon, a fight, the
    character window, a dialog, the shop, the map screen, weather) taken with 0.35 and with the new
    renderer, compared picture by picture; differences explained or fixed.
    *Done 2026-11-26 on Linux (Vulkan, NVIDIA): 18 pictures, both runners stepping a sixtieth
    of a second per frame (0.35 with a patched macroquad in a scratch worktree). The UI screens
    are identical; world scenes differ in 0.0-0.5 % of pixels, single pixels along rotated
    edges. No dialog or shop picture in the set yet.*
  - `status` frame times on the forest map, old against new (`RUSTPG2D_PERF=1`).
    *Done 2026-11-26 (debug builds, RTX 4060): both hold 60 frames a second with the worst
    frame near 20 ms. The CPU's share of a forest frame went from 4.7 ms (drawing, with a flush
    per camera change) to 0.8 ms drawing + 1.5 ms sending 1,377 draw calls in 6 passes; without
    vsync (`RUSTPG2D_VSYNC=0`) the forest runs at about 600 frames a second, the house at 4,000.
    `status` prints `gpu=CALLS/PASSES:MS`.*
  - `scripts/ci/smoke.sh` (the game half) passes; sound heard: music loop, a fade, footsteps.
    *The game half passes (2026-11-26); the editor's checks are skipped while there is no
    editor. Sound: the game opens its stream at the system's sound server and `status` shows the
    village music as a whole loop; nobody has listened yet.*
  - Linux (AppImage on Debian 12, glibc ≤ 2.34), Windows under Wine (try this first: Wine's
    graphics were fragile on this machine), the test Mac, the browser (headless Brave, saves kept
    across runs).
    *The browser: done 2026-11-26 (headless Brave on SwiftShader, the example exported with
    `cargo run -p rustpg2d-engine --example export_web`): the title screen draws as on the
    desktop, `status` and `shot` come out in the console, a changed option is still there in a
    second run with the same profile. Not checked: sound in a browser, a real graphics card
    (SwiftShader ran at 8 to 30 frames a second), the download size (the runner is 17 MB before
    compression, macroquad's was smaller).
    Linux: done 2026-11-26 in a Debian 12 container without a graphics card (release build):
    the game needs glibc 2.34 as before (five more math functions in `glibc_compat.rs`), refuses
    to start on the software renderer with a plain message, and runs on it with
    `RUSTPG2D_ALLOW_SOFTWARE=1`. Not checked: the packaged AppImage (there is no editor to
    package yet).
    Windows: done 2026-11-26 under Wine (MinGW build from a container): the game draws through
    Vulkan on the real graphics card, without the software-OpenGL workaround Wine needed
    before; its picture of the house matches Linux's. Not checked: a real Windows machine
    (DirectX 12).
    macOS: done 2026-11-26 on the test Mac (M2, macOS 27.2, Metal; a debug build made there):
    the game holds 60 frames a second in the house and the village, its picture of the house
    differs from Linux's in 0.2 % of pixels, and the new editor window shows the game in its
    panel. Handing a village frame to Metal takes about 5 ms there (0.3 ms with Vulkan on
    Linux): worth a look before big maps. Not checked: an exported `.app` (the export needs the
    editor), sound.*
  - Tests render without a window: one golden picture each for ground, a figure, text.
    *Done 2026-11-26 as one picture: `crates/engine/tests/headless.rs` loads the example's
    house, plays 90 frames without a window and compares the screen with
    `tests/golden/house.png` (ground, figures, text, the UI's skin). Skipped without a graphics
    adapter unless `RUSTPG2D_GPU_TESTS=1`; CI runs it on Vulkan's software renderer.*

- [~] Part 2: The editor core — `feature/studio`

  **Goal.** Everything the editor knows and does, without any UI: one place that the windows, the
  editor script and the agent server all work through.

  **Design.**
  - **A new crate, `crates/editor-core`** (no egui). The **workspace**: the project (settings,
    definitions), the open maps (loaded when first needed), the library, what is selected in each
    document.
  - **Commands.** Every change to the project is a command the workspace runs: *paint ground*,
    *place a prop*, *set a field of a definition*, *rename an id*, *install a pack*. A command has
    an id, a title, parameters, and leaves an undo record.
    - One history for everything, replacing the map's (`doc.rs`) and the definitions'
      (`db_undo.rs`). Undo in a document undoes that document's last change; a history panel
      shows all of it. Typing in a field stays one step (the coalescing of `db_undo.rs`).
    - Definition changes are recorded as patches (`mcp/patch.rs`), map changes as the edits of
      `doc.rs`.
    - The **command registry** lists them all: the palette, shortcuts, the script and the agent
      tools are generated from it or checked against it.
  - **Questions it answers:** search (`search.rs`), usages and renames (`refactor.rs`), the
    problems of the project, kept current as assets change instead of on request (`problems.rs`),
    unused assets (`unused.rs`).
  - **Services:** the project file and its working copy (`project_file.rs`), files changed on disk
    (`mcp/watcher.rs`), imports (models, pictures, sounds, fonts), thumbnails (rendered off screen
    by the game's renderer, cached by content), background jobs (art, packing, downloads).
  - **Three ways in, one behaviour:** the UI (part 3), the editor script
    (`RUSTPG2D_EDITOR_SCRIPT`) and the agent server (`mcp/`, moved here) issue the same commands.
    The agent server also runs without a window: `rustpg2d-editor --headless PROJECT --mcp PORT`.
  - **Editor data in the project:** `editor/` in the project file holds what only the editor
    needs (folders, tags, notes, favourites, layouts, where an asset came from). Exports leave it
    out.

  **Progress.** `crates/editor-core`, from the UI-free files of the 0.35 editor:
  - the workspace (`workspace.rs`: a project folder or file, several open maps each with its
    history, saving, the status line, the selection and view of the map worked on; with or
    without a graphics adapter), map documents (`doc.rs`), the definitions' undo (`history.rs`),
    renames (`rename.rs`), the project file, the editor's settings and recent list, repository
    mode, the starter library (`starter.rs`);
  - reference lists (`refs.rs`), search and usages (`search.rs`), the problems list
    (`problems.rs`), map operations tools and agents share (`mapops.rs`), 3D models
    (`models.rs`), the asset library's transfers (`library.rs`), Generate art's jobs (`art/`);
  - **the agent server** (`mcp/`, all 71 tools): it runs without a window, `rustpg2d-studio
    --headless PROJECT --mcp PORT`. The smoke client passes against it (reads, painting with
    undo, saving, pictures of sheets, writing art, definitions, placing, capturing a structure,
    sending to the library, a play-test); only `screenshot_map` answers that it needs a window.
    `scripts/ci/smoke.sh` and CI run it.
  Tests on a scratch copy of the example, without a window: a change undone and saved leaves
  every file byte for byte, two maps stay open with their changes, a rename reaches maps and
  scripts and undoes; the 0.35 tests of the copied files (40 in all, among them "the manual
  names every agent tool").
  - **one history** (`timeline.rs`): the definitions' steps and each open map's in the order
    they were made, each with a name; *Undo* takes back the latest wherever it was, a document
    can undo its own latest change. Agents' writes are entries named after the tool.
  - **the command registry** (`commands.rs`): `Workspace::run("edit.rename", …)` /
    `run_line("map.open house")`; the first thirteen commands (save, undo, redo, rename, open
    and close a map, search, usages, the project check, reload, the agent server).
  Still open: more commands as the parts bring their features; pictures without a graphics
  adapter (`--headless`: map screenshots are answered in the window since part 6); the editor script running command
  lines; imports (Tiled, Aseprite: part 11 decides); the cleanup of unused files; structures'
  capture and the clipboard.

  **Decided while building.**
  - **Changes are observed, not funnelled:** forms and tools change the definitions directly
    and the history turns the difference into a step (as 0.35's did), so a widget needn't
    know about commands. Named commands wrap that for the palette, scripts and agents.
  - **Projects older than 0.35 are refused** with a message naming 0.35 already here (the
    converters themselves leave in part 11).
  - **The UI-free files of the 0.35 editor are copied over and adapted,** not rewritten: their
    tests come along. What they kept of the window is plain state on the workspace that the
    window fills in (`looking_at`, `asking`, the open script's text).
  - **Screenshots are answered by whoever draws** (`mcp_answer_shots` with a callback); the old
    way (frame the canvas, grab the window two frames later) is gone.

  **Engine.** None beyond part 1. The crate takes over the logic of the old `app.rs`, `doc.rs`,
  `db_undo.rs`, `maps.rs`, `multisel.rs`, `clipboard.rs`, `structures.rs`, `search.rs`,
  `problems.rs`, `cleanup.rs`, `project_file.rs`, `files.rs`, `import/`, `art/` (jobs) and `mcp/`.

  **Verify.** Tests without a window: every command applied to a copy of the example and undone
  leaves the files byte for byte as they were; `mcp_smoke` against the headless server; the test
  that `13-ai-agents.md` names every agent tool passes with the 0.35 catalogue.

- [~] Part 3: The shell — `feature/studio`

  **Goal.** The window creators work in: one workspace where everything is a tab, with the ways to
  get anywhere and do anything.

  **Design.**
  - **Layout:** the Project browser on the left, documents in the middle (tabs, split side by
    side), the Inspector on the right, Problems / Output / Help at the bottom, a status bar (save
    state, background jobs, problem counts). Docking with `egui_dock` or `egui_tiles` (choose when
    starting; it must tear panels off into windows). Presets (*Mapping*, *Writing*, *Data*) and
    the creator's own layout, remembered per project.
  - **Several windows:** any panel or document can be dragged out into an OS window of its own
    (a map on one screen, an item on the other). The asset library is a panel of this program,
    not a second process.
  - **Home** (no project open): recent projects with a picture of their title screen, *New
    project* (blank, the example, samples), *Open*, *Learn*, what is new in this version, the
    state of the starter pack.
  - **Overview** (the first document of a project): the game's card, a **checklist to a playable
    game** worked out from the project (a start map, a hero, a title screen, no problems, an
    export), the documents last worked on, a notes list.
  - **Getting around:** back / forward (Alt+Left / Right), breadcrumbs, pinned tabs, favourites
    (a star on any asset), *Go to anything* (Ctrl+P: maps, assets, script functions, characters on
    maps, dialog lines, settings, with previews), the **command palette** (Ctrl+Shift+P), a
    shortcut sheet with rebinding (from the backlog).
  - **Drag and drop:** files dropped on the window are imported by kind (pictures, `.glb`,
    sounds, fonts, packs); assets are dragged from the browser onto maps and into fields.
  - **Look:** a theme of its own (light and dark, a UI scale setting), one set of parts used
    everywhere: toolbars, property rows, cards, id chips, empty states, toasts with *Undo*.
  - **Settings** (the editor's own): appearance, shortcuts, saving, the agent server, the Meshy
    account, pack sources.

  **Progress.** `crates/studio` (binary `rustpg2d-studio` until it replaces the 0.35 editor), an
  eframe window (egui 0.37) on the engine's wgpu device, over `editor-core`'s workspace:
  - `shell.rs`: panels and documents as tabs that can be arranged (egui_dock): the Project
    browser on the left (the fixed groups with their kinds and counts, a filter), documents in
    the middle, Problems and History below; a menu bar made from the command list; the status
    bar (status line, saved or not, problem counts, frames a second);
  - the command palette (Ctrl+Shift+P; a command that takes words gets them typed after its id)
    and *Go to anything* (Ctrl+P, the core's search); Ctrl+S, Ctrl+Z, Ctrl+Y;
  - documents so far: the Overview with the checklist to a playable game, a definition (its
    usages as links, and its file entry as text until part 4's forms), a map (its facts until
    part 6's view), and **Play** (`play.rs`: the game in a tab, a `gfx::Screen` of the tab's
    size shown as an egui texture, the tab's keys and pointer as the game's input, 60 frames a
    second);
  - the agent server runs in the window too (calls between frames);
  - `RUSTPG2D_STUDIO_SCRIPT`: `wait`, `status` (`tabs=`, `focus=`, `history=`, `problems=`,
    `palette=`), `shot` of the whole window, `tab NAME`, `palette TEXT` / `goto TEXT` / `pick N`,
    `key`, `pause`, `resume`, `quit`, and any command line of the editor (`map.open village`).
  - **The home screen** (the Overview while no project is open, and its *Another project*
    section otherwise): *A new game* (its name, where it is kept: `~/rustpg2d/<name>` as a
    project file) makes a project and opens it: with *the basics from the asset library*
    unless that is unticked (ground to paint with, scatter sets, a wall and a fence, a hero
    who is the player's character, a few items and props, grass on the first map:
    `Workspace::start_with_basics`, the 0.35 wizard's starter assets), so Play shows a hero
    on grass at once; *Open a project* by where it is; the
    projects worked on lately; the bundled example in a packaged R(ust)PG2D. Another project
    opens in the same window (`Shell::switch_project`); it is refused while something isn't
    saved. *Close this project* leads back to the start. Where a new game is kept, a project
    file and a project's folder are also picked in the system's file dialog (*Browse…*, *A
    folder…*; a project picked opens at once), and so is the folder a game is published
    into. Started without a project (and not in a checkout with its example), the editor
    shows the home screen and nothing else.
  - **File dialogs** (`studio/src/pick.rs`, the `rfd` crate through the desktop portal on
    Linux, as the 0.35 editor's): a dialog runs beside the editor, which keeps drawing; who
    asked takes the answer by a key in a later frame; one dialog at a time (the buttons
    wait meanwhile); on macOS the dialog is modal. A scripted run and a test never open one:
    `pickanswer PATH` (`pick::answer_next`) says what the next answers.
  Still open: of the home screen pictures of the projects; layouts saved and presets, tearing tabs off into windows, back / forward,
  favourites, the shortcut sheet with rebinding, the theme (light, UI scale, icons), toasts,
  the editor's settings, `egui_kittest` tests; of a new game what the 0.35 wizard also
  asked: the hero's sex, whether players make their own character, the first map's size.

  **Decided while building.**
  - The new editor grows in `crates/studio` (the old one keeps `crates/editor`, with the
    starter library's files, until the switch-over), over `editor-core` when that exists.
  - Its window is X11 where there is an X server, like the game's (`RUSTPG2D_WAYLAND=1`).

  **Engine.** None. New: `crates/studio` (the eframe application over `editor-core`), the dock,
  the theme, `ui.rs`' icon helpers carried over.

  **Verify.** `egui_kittest`: open the example, open three documents, split, tear one off, restart
  and find the layout again; Ctrl+P and the palette reach a map, an item and a command.
  Screenshots of Home, Overview and a mixed layout in both themes.
  - The home screen, in the window: the project closed (the start with *A new game*, *Open a
    project* and the scratch game under *Worked on lately*); a new project made in a scratch
    folder (project.ron, defs, a start map, a script) and opened; its start map in a tab, where
    the Build tool says that an empty project has no building to build like yet; the example
    opened in the same window and the new project again.
  - A new game's basics: `a_new_project_starts_with_the_basics_of_the_library` (made, filled
    and read again from its files: called what was typed, the hero the player's character
    with a figure and hit points, every one of the 25 basics, grass on the first map, what
    they need with them). In the window: a new project, Play: the hero on grass with 12 hit
    points, walking. (Before, an empty project played a yellow box with none: found when a
    merchant in one wouldn't answer.)

- [~] Part 4: The Project browser and asset editors — `feature/studio`

  **Goal.** The Database, restructured: every kind of thing has an obvious place, lists show what
  they hold, and an asset's editor shows the asset.

  **Design.**
  - **Fixed groups** at the top (so a thing is always in the same place), the creator's folders
    and tags inside them:
    - **Game:** Overview and the settings pages (part 5).
    - **World:** maps, the World view (part 6), structures, ground materials, scatter sets.
    - **Objects:** props, by what they are (furniture, nature, buildings, containers, doors,
      lights, crops…).
    - **Characters:** characters (people, creatures), races, classes, backgrounds, factions.
    - **Items:** items by category (`item_category.rs`), recipes.
    - **Rules:** spells, diseases.
    - **Story:** quests, cut-scenes, common events, achievements, scripts, visual scripts, flags
      and variables (part 7).
    - **Art & sound:** 3D models, pictures (sprite sheets), animations, sounds and music, fonts,
      UI skins.
    - **Languages** and **Plugins.**
  - **Lists:** as a list, a grid of thumbnails or a table (`db_table.rs`); sorted and filtered by
    category, tag, *has problems*, *unused*, *from pack…*; several selected at once for tagging,
    moving, duplicating, sending to the library, deleting.
  - **An asset is a document:** a header (picture, name, id with rename, tags, where it came
    from, how often it is used), its sections, a live preview beside them (the prop as the game
    draws it with its states, the item in a figure's hand, the character moving, a spell's area),
    and *Used by* at the end.
  - **The field schema.** One table per kind of definition describes every field: its label, its
    help text, how it is edited (text, a line players read, a number with its range and unit, a
    choice, a reference to a kind, a dice expression, a colour, a list…), its section, whether it
    is basic or advanced, its default. From that one table come:
    - the form, the table view's columns and bulk editing;
    - *Find a setting* (a field by its name, across all forms and settings pages);
    - the mark on fields changed from their default, and *Reset*;
    - problems shown at the field;
    - the field descriptions agents get, and the manual's reference tables (a test compares).

    A test fails when a serde field of a definition is missing from its table. What a table can't
    express (triggers, stat blocks, building shapes, class levels, schedules) stays a hand-made
    editor plugged into it.
  - **One picker** for every reference: search, thumbnails, the last used, filters, **+ New…**
    (made from a template, edited in a side panel, back to the field) and **From library…**
    (fetched with what it needs).
  - **New…** (Ctrl+N): pick the kind, then where it starts from (blank, a copy, the library, a
    file, Meshy), with a preview.

  **Progress.** Every definition of every kind is edited in a form made from the field schema.
  - **The schema** (`editor-core/schema.rs`) has two sources. The *shape* comes from the engine's
    own types: with the feature `schema` every type that is written to a file derives a JSON
    Schema (`engine/schema.rs`; `cargo run -p rustpg2d-engine --features schema --example schema
    -- item` prints one), which gives a definition's fields in their order, what each holds, an
    enum's variants, the value a file may leave out, and the field's doc comment as its help.
    What only an editor knows is in three small tables there: `LABELS` (where the field's name
    doesn't say it), `SECTIONS` (the sections of each kind's form, `~` = for advanced use) and
    `REFERENCES` (the fields that hold an id, with the kinds they may name).
  - **The form** (`studio/form.rs`): sections that fold; a label and help (on hover) for every
    field; a choice for an enum, with the variant's fields under it and what fits kept when the
    kind changes; a picker for an id (what it names, missing ones in red, a search, *none*,
    *+ New…*, *Open*); a row for fixed-size values; nothing-or-something as *+ Add* / *Remove*;
    lists with add (of an enum: from the searchable list of its variants), remove, move; named
    entries; colours. Fields a file leaves out are shown with their default, so a form shows
    everything a definition can have; what still holds its default is dimmed, so what was set
    stands out, and a label's menu (right-click) has *Back to the default*. *Show all fields*
    adds the advanced ones. Every control is named by its label (AccessKit).
  - `editor-core`: `def_value` / `set_def_value` (a definition as data and back, refused with
    the reason when it isn't one), `new_def`, `duplicate_def`, `delete_def` (refused while
    something uses it), the commands `defs.new` (Ctrl+N), `defs.duplicate`, `defs.delete`. What
    is typed but isn't valid yet stays in the form with the reason. Every change is a history
    entry. A header with Rename, Duplicate, Delete, Find usages and *Used by* as links.
  - **Pictures** (`studio/thumbs.rs`): egui shows the engine's own textures, nothing is drawn
    or copied a second time: an item's or spell's icon and a prop's picture are their cells of
    their sheet, a character, model or race is the pose the game's model atlas renders (the part
    the figure covers: `Cell::drawn`, new in the engine), a material its texture. They are in
    the browser's rows, in an asset's header, in the pickers' lists, and beside a sprite's
    fields, where *Pick the frame…* shows the sheet's frames to click. Only what is on screen is
    asked for, so a list of hundreds costs what it shows.
  - **Everything of a kind** in a tab (*Show all* in the browser, script `tab kind:item`): cards
    with pictures or a list, with a filter and *+ New*.
  - **Categories told from what a thing is** (`editor_core::defs::categories`): items by the
    SRD's categories (`item_category.rs`), props by what they do (walls and buildings, doors,
    containers, furniture to use, lights, gathering and farming, traps, things that react,
    scenery), characters by creature type, spells by level, models (playable, figures, pieces,
    rig templates), structures by their category, sheets by kind. The browser lists a kind
    under them; a kind's tab filters by one.
  - **The creator's own order** (`editor-core/meta.rs`, `editor/meta.ron` in the project; the
    game never reads it, exports leave it out): a folder (`Village/People`), tags, notes and a
    favourite mark per definition or map. In an asset's header (*Favourite*, the folder, the
    tags with *+ Tag*, *Notes*); in the browser (*Favourites* first, a kind's folders before
    its categories, the filter also finds tags and folders); in a kind's tab (tags to filter
    by, folder and tags in the list). Commands `meta.tag`, `meta.untag`, `meta.folder`,
    `meta.favorite`, `meta.notes`. Written at once, no unsaved state; every change is a step of
    the one history; a rename takes what was noted along, a copy starts with folder and tags.
  - **Several at once:** in a kind's tab Ctrl+click chooses cards (a box per row in the list);
    the chosen are tagged, untagged, moved to a folder, made favourites or deleted together
    (one history step for the notes; a deletion is refused per asset while it is used).
  - ***New…*** (Ctrl+N, every *+ New*, a picker's *+ New…*; `studio/gallery.rs`): the kinds as
    the browser groups them, each with a sentence on what it is (`defs::about`); a name, from
    which the id is made (`defs::id_from_name`, numbered when taken); it starts blank or as a
    copy of another, picked from a list with pictures. `Workspace::make_def` and the command
    `defs.make KIND [from:ID] NAME` do the same without the window. One history step.
  - **What is wrong, and what nothing uses:** after the project check, an asset with a problem
    has a mark in the browser and on its card, its header says what is wrong, and a kind's tab
    shows *with problems* alone. *unused* (sheets, models, sounds, materials, fonts: what the
    clean-up of unused files looks at) shows what no definition, map, setting or script names.
  - **A kind as a table:** a row per definition, a column per field that fits into a cell (the
    schema says which; *Columns* picks, the first section's by default), cells changed in
    place with the forms' controls, a click on a column's head sorts by it. A change in a
    chosen row is made in every chosen row, as one history step.
  - **Names in use** (`schema::names`, table `NAMES`): where a flag, a variable, a script
    function, a faction, a prop's state, a target, a crafting station or an emotion is typed,
    a button beside the field lists the ones the project uses (`Workspace::refs`); a new one
    is typed.
  - The browser's fixed groups with a *+ New* per kind (part 3's shell); while something is
    typed into its filter, what fits is shown open and the rest is out of the way.
  - Script: `field KIND ID /POINTER VALUE` changes a field as the form does; `click X Y`,
    `hover X Y`, `wheel DY`, `type TEXT`, `press KEY` are real input in the window (eframe's
    input hook), `size W H`, `allfields on|off`.
  Still open: the hand-made editors (triggers as an event sheet in part 7; stat blocks,
  buildings, class levels), a state picked among the states of the prop in question, tool
  kinds and piece styles, units and ranges on numbers, problems at the field, live previews (an
  item in a hand; a prop with its states and a moving character are part 8's workshops, as
  are the pictures of structures),
  problems at the very field, *unused* for items, props and characters (nothing
  places or names them), folders as a tree with drag and drop,
  where an asset came from (a pack: part 9), *New…* from a file or Meshy (part 8; from the
  library is there), *From library…* in pickers (part 9), the schema as
  the agents' field descriptions and the manual's reference tables.

  **Decided while building.**
  - *The shape is derived, not written.* A hand-written table per kind would repeat ~300 types
    and go stale with every engine change. `schemars` reads the same serde attributes the files
    are written with, so the form can't disagree with the format; without the feature the engine is
    as before (the game never calls it).
  - *The reference table is checked against the engine's renames.* The test
    `references_agree_with_renames` makes up values with everything in them (every field, every
    variant, every text an id), renames each kind through them with `refactor::rename_in_db` /
    `rename_in_map`, and fails unless `REFERENCES` lists exactly the texts that changed, with
    the lines it should have. A new id field in the engine can't be forgotten in the form.
  - *Found on the way:* fifteen fields name a definition but aren't followed by renames (a
    quest in *Quest* actions and conditions, an achievement, spells in *Learn spell* and class
    features, a subrace's race, a route's sound, the camp item, a schedule's map…): the table
    `NOT_FOLLOWED`. They get pickers; renaming those ids still leaves them behind, as in 0.35.
    The schema walk (`schema::texts`) can replace the hand-written rename and usage passes;
    that belongs to part 11's clean-up, with a test of its own.
  - *Pictures are the engine's textures, not copies.* egui gets the sheet's texture and the
    part that is the sprite, or the model atlas's page and the pose's part. One exception: egui
    blends colours already multiplied by their alpha, the engine keeps them apart, so a sheet
    shown in the UI gets a multiplied copy the first time (else soft edges get bright fringes).
  - *Categories are derived first, tags come on top.* A project that was never sorted by hand
    is sorted already; the creator's folders and tags refine it.
  - *Notes aren't "unsaved changes".* `editor/meta.ron` is written when it changes, like the
    editor's own settings: tagging thirty props mustn't ask to save the project. Undo still
    covers it (a step keeps what was noted before). What was noted about something deleted is
    kept until the file is next written, so undoing the deletion finds it again.
  - *Help is the doc comment for now.* It was written for developers; where it reads badly for
    creators it gets a line in the tables (or a better comment).

  **Engine.** The schema tables (in `editor-core`, beside the definitions' use) and
  `editor/meta.ron` (folders, tags, notes, origin per asset). Definition formats unchanged.

  **Verify.** The schema coverage test. `egui_kittest`: for every kind, change a field, save,
  reload (the old `ui_tests.rs` cases carried over); a picker's *+ New…*; bulk tagging. Screenshots
  of the browser, a grid, a table and three asset documents.
  - Done so far: `every_field_has_its_section` (all 24 kinds), `references_agree_with_renames`;
    `form.rs`'s UI tests through AccessKit (sections and advanced fields, a choice picked from
    its list, an id picked by searching and opened, a field put back to its default from its
    label's menu; egui_kittest also fails any control without a name).
  - A scripted run in the window on a copy of the example, with real clicks: the Iron Sword's
    *Used on* opened and *Ally* clicked, *Teaches* opened, "fire" typed, *Fire Bolt* clicked: two
    history entries, saved, `items.ron` has `use_on: Ally` and `teaches: "fire-bolt"`, two undos
    take them back. Hovering *Stack size* shows its help; the wheel scrolls the form.
  - Pictures, looked at in the window: the cards of all items, props, characters (people and
    creatures each filling their card) and materials; the browser filtered by "sword" with its
    icons; the Elder's header and portrait; the Iron Sword's *Pick the frame…* with the chosen
    frame marked; the model picker's list with figures. `tests/headless.rs` checks `Cell::drawn`
    on the hero's pose. Categories: the browser's Props and Items under their categories, the
    Props tab with *Containers* clicked (5 of 187); test
    `definitions_are_put_into_categories_by_what_they_are`. Names in use: the Goblin Archer's
    *Faction* lists the project's eight, one clicked is a history entry; test
    `made_up_names_are_known_where_they_are_typed`.
  - The creator's order: `tests/workspace.rs` (`tags_and_folders_are_written_follow_renames_and_undo`:
    commands, the file, a copy, a rename, five undos, a redo) and `meta.rs`'s tests. In the
    window: the Potion's header with its folder, tag and notes; *Favourites* in the browser;
    three items chosen with Ctrl+click, *Tag…*, "quest" typed, Enter: one step *Tag 3 items*,
    three entries in `editor/meta.ron`, undone and redone.
  - Problems and unused, in the window: the Iron Sword taught a spell that doesn't exist:
    *with problems (1)* shows it alone with a red mark, its header says "refers to spell
    "no_such_spell", which doesn't exist"; a sheet made and named by nothing is under *unused
    (1)*. The script's `field` now reaches fields a file leaves out and refuses a field the
    kind doesn't have (`Node::at`).
  - The table, in the window: the items with Name, Description, Stack size and Value; sorted by
    Value downwards (Ring of Protection 4000 first); Gate Key and Bronze Key ticked, 77 typed
    into one's Value: both have 77, one step *Change value of 2 items*, one undo.
  - *New…*: `a_new_definition_is_made_from_its_name` (blank, numbered id, a copy, a quest's
    title, undo). In the window: *Item*, "Moon Blade" typed, *as a copy of another item*, the
    Hunting Bow clicked (its picture beside the list), *Create*: the tab of `moon_blade` opens,
    one history step, and after saving `items.ron` has it with the bow's values.

- [~] Part 5: Game settings and rules — `feature/studio`

  **Goal.** The settings of the game and its rules have a home of their own instead of one long
  form among the Database's tabs.

  **Design.**
  - **Pages** of the *Game* group, each a document with previews where there is something to see:
    - **Game:** name, icon, window size, where it starts, credits.
    - **Hero & party:** the player's character, the character creator (what it offers, the
      starting kit), the bag's size, what happens to a companion at 0 hit points.
    - **Rules:** the SRD options (scores, proficiencies, shown checks, encumbrance, food and
      water, reactions), magic, experience and levels; the SRD attribution's state.
    - **World:** the length of a day and the starting hour, factions (as a table of standings),
      money, map titles.
    - **Look & feel:** fonts by role, the UI skin, dialog portraits, emotions, facings.
    - **Title & menus:** the title screen with a live preview, the pause menu, the action bar.
    - **Sound:** the game's sounds, battle music.
    - **Languages:** the languages offered, and a **translation table** (every text beside its
      translations, missing ones first) next to the template export.
    - **Scripts & hooks:** the hooks; plugins and their parameters.
    - **Publishing:** the export as a page: targets, ids, icons, remembered; a check before
      exporting (problems, missing translations, unused files, attribution).
  - They are forms of the field schema, so *Find a setting* reaches them.

  **Progress.** The pages exist, as forms of the field schema.
  - `schema::PAGES`: nine pages (Game · Hero & party · Rules · World · Look & feel · Title &
    menus · Sound · Languages · Scripts & plugins), each the fields of the project's settings
    it shows, with labels of their own; a test keeps every setting on exactly one page.
  - The *Game* group of the Project browser lists them under the Overview; the browser's
    filter finds a page by a setting's name. A page is a tab (`settings:rules` for scripts)
    with the same controls as a definition's form: pickers for the start map, the player's
    character, the skin, the sounds; defaults dimmed; *Show all fields*.
  - `Workspace::config_value` / `set_config_value` (refused with the reason when it isn't
    valid settings); a change is a step of the history (*Change the game's settings*). Script:
    `setting /POINTER VALUE`.
  - **Publishing** (`studio/src/publish.rs` over `editor-core/src/export.rs` and
    `package.rs`; the *Game* group's last entry, tab `publish`): the game's name, the folder
    it goes into (`releases` beside the project), Linux / Windows / macOS / Web, a zip of
    each, one `data.pack` or loose files, on Linux an AppImage and a Flatpak bundle (app id,
    runtime), the icon. Under it the pre-flight: what stands in the way (a runner that is
    missing or was built from another engine than the editor, a project without maps, a bad
    app id) in red, what is worth knowing (the project not checked yet, a debug runner, a
    glibc too new for older distributions) in yellow, and what will be written where with
    the folder the saves go to. *Export* is there once nothing stands in the way.
  Still open: what a plain form doesn't give. The factions as a table of standings, the title
  screen's live preview, the translation table, the hooks with their script functions and the
  plugins' parameters by their kinds, the game's icon picked from a picture (it is a path on
  the Publishing page), the spawn point picked among the start map's, and *Find a setting*
  across forms and pages. Of Publishing: a folder picker (the folder is typed), the export
  on a thread with its progress (the window waits while the game is written).

  **Engine.** None: `ProjectConfig` keeps its shape; the pages group its fields.

  **Verify.** `egui_kittest`: a change on each page saved and reloaded. Screenshots of Rules,
  Title & menus and Publishing. An export from the page runs.
  - Done so far: `every_setting_is_on_one_page`; `settings_change_as_data_are_saved_and_undone`
    (a setting of seven pages changed, an invalid one refused with the reason, saved, read by a
    second workspace, one undo). In the window: the Rules and World pages looked at;
    `setting /day_minutes 30` and `/rules/show_checks false` are two history steps, in
    `project.ron` after saving, undone twice; `/nonsense` is refused.
  - Publishing: `the_preflight_says_what_stands_in_the_way_of_an_export` (nothing ticked and
    no name are stops and nothing would be written; this computer's platform with where the
    game and its saves go; a missing runner said, and nothing written; a bad Flatpak app
    id), the packaging glue's three tests, `publish::ui_tests` (the page filled in from the
    project, a second platform ticked, the app id following the name, no name stopping it).
    In the window: a release runner older than the editor refused with the reason (now a
    stop of the pre-flight); with a current one the example exported for Linux as
    *Emberbrook* (707 files, 99 MB: the program, `data.pack`, the licenses, a zip), and the
    exported game started on the village. Not run here: Windows, macOS, web, AppImage and
    Flatpak from the new page (the engine's and the packaging crate's code is 0.35's).

- [~] Part 6: The map workspace — `feature/studio`

  **Goal.** Building the world is direct: fewer tools that do more, everything on a map listed
  and reachable, and the game playing in the editor.

  **Design.**
  - **Maps are documents:** several open, side by side, each with its view and selection.
  - **Tools, regrouped:**
    - **Select** (V): click, box, move, turn, duplicate with handles on the canvas; align.
    - **Terrain** (P): ground materials with brush, line, rectangle, fill.
    - **Place** (N): one palette for props, characters, items, structures and scatter sets, also
      fed by dragging from the Project browser. A prop is placed as scenery; *Make interactive*
      in the Inspector turns it into an object with states and triggers, and back. Creators no
      longer choose between "Entities" and "Decorations" before placing (the map still stores
      decorations and entities).
    - **Build** (B): walls, rooms, houses, roofs, doors and windows, as today.
    - **Zones** (Z): every kind of zone, and map transitions as part of it.
    - **Collision** (C): the overlay edited by hand.
  - **Outliner:** everything on the map in a list (characters, objects, zones, lights), with
    search, hide, lock, rename; layers switched on and off (ground, decorations, objects, zones,
    collision, light, fog).
  - **Right-click anything:** *Edit definition*, *Find usages*, *Play from here*, *Save as
    structure*, *Replace with…*.
  - **Inspector:** what the placed thing changes against its definition is marked and can be
    reset; its events in the editor of part 7.
  - **Map settings** as a panel (light, weather, music, parallax, fog, edges); time of day and
    weather previews on the canvas's own bar.
  - **The World view:** every map as a card with its picture, the transitions as arrows between
    them; drag from one map to another to link them; maps nothing leads to are marked.
  - **Play inside the editor:** a Play panel runs the game in this process, on the same renderer:
    from the start, from here, from a test setup (`testplay.rs`); pause, step, speed.
    - While it runs, a **State** panel shows flags, variables, quests, the party and the bag, and
      changes them.
    - Clicking a character or prop in the running game opens its definition or its place on the
      map.
    - Scripts reload while it runs; the console is a panel; script errors link to their line.
    - *Play in its own window* (the real runner) stays for the last check.
  - The top-down view stays as a view option unless the user removes it in part 11.

  **Progress.** A map is a tab with its view, tools, Outliner and Inspector.
  - **The view** (`studio/src/mapview.rs` over `scene.rs`): the map drawn by the engine's
    renderer into a picture of its own, as the game shows it (painted ground, scenery,
    everyone standing in depth order, buildings, shadows), with the darkness and lights of an
    hour, the weather, the grid, zones and blocked tiles as options, and *From above*. The
    wheel zooms around the pointer; the middle button, Space with the left one, or the arrow
    keys move it; *Fit* (Home) shows the whole map. Each open map keeps its own camera,
    selection and tool; maps side by side each draw their own picture.
  - **Select** (V): click, Shift / Ctrl+click for several, a box dragged over empty ground;
    drag to move (one thing snaps as it does when placed, several move by tiles), R turns,
    Ctrl+D duplicates, Del deletes; a click on what is selected goes on to the thing behind
    it; a double click opens the definition (a portal: its other map); the right button has
    the same as a menu. A zone is picked by a click, so a drag inside a big one still draws
    a box; the selected zone has corners to drag.
  - **Terrain** (P): the materials with their pictures; brush (round or square), line,
    rectangle and fill, *On tiles* or on the ground's cells; Shift erases, the right button
    picks the material under the pointer, [ and ] size the brush. A stroke is one step named
    by its material (*Paint Cobblestone*).
  - **Place** (N): characters, props, items, structures and scatter sets in one searched list
    with pictures; the pick follows the pointer as a ghost and is put down by clicks (Shift:
    off the grid, R turns it first). **A prop that only stands there is placed as scenery; one
    that does something** (events, states, a container, a lock, something to gather…) **as an
    object**, and the list says which. Scenery with a grid of its own (walls, fences) is
    dragged out as a row; a scatter set is brushed on; a structure shows its footprint.
    A character, prop, item or structure can also be dragged out of the Project browser
    onto a map.
  - **Scenery is a thing like any other:** selected by a click (then dragged: a house isn't
    moved by a drag meant for something else), boxed, moved, turned, duplicated, deleted; the
    Inspector has its layer, mirroring and shown state, and ***Make interactive*** turns it
    into an object on the same spot (***Make scenery*** back, while the object has nothing of
    its own). Big things (a house, a wall) are hit by their shape, not by their picture's
    frame. The Outliner lists scenery by what it is (*Young Oak x 311*: a click selects them
    all); *Picks* says what the Select tool takes.
  - **Zones** (Z): every kind the engine has, listed from the field schema with its help; a
    drag makes one, named after its kind.
  - **Collision** (C): left blocks, Shift gives a tile back to the ground's own rule, the
    right button opens it.
  - **Outliner:** the map's characters, objects, items and zones, searched; a click selects
    and brings into view.
  - **Inspector:** the selected thing's form from the field schema, with the sections that
    mean something for what it is (a prop has no schedule, a character no lock); what it
    doesn't set itself is dimmed like a default; *Edit definition*, *Turn*, *Duplicate*,
    *Delete*. Zones the same, with their place in tiles.
  - **The core** (`editor-core/src/mapedit.rs`): placing, moving, turning, duplicating,
    deleting, zones, scenery, the ground, scatter strokes, structures and the Inspector's
    data, each one step of the history with a name (*Place Town Guard*, *Move elder*, *Add a
    spawn zone*, *Scatter Forest*). `snap_map` / `commit_map` make one step of whatever
    changed between them, objects and scenery alike.
  - **Agents see maps again:** `screenshot_map` is answered from the same drawing, without
    the map's tab being shown.
  - **Layers:** the painted ground, scenery, and characters, objects and items are each
    switched off and on (*Layers*); what is hidden can't be picked either.
  - **Save as structure:** several things selected (a house with its fence, boxed) are
    saved with the ground under them as a structure by the name typed (`tiles_of`,
    `save_as_structure`); the menu's *Find usages* shows where a thing's definition is
    placed and named.
  - **A character's day on the map:** the selected character's patrol route (numbered
    waypoints, arrows the way it walks) and daily schedule (dashed, each stop with its
    hour) are drawn.
  - **Map settings:** with nothing selected the Inspector is the map's own form (light and
    weather, music, what lies beyond its edges, its events), from the field schema
    (`MapSettings`, new in the engine: what a map is besides its ground and what stands on
    it).
  - **Transitions are drawn on both maps:** a Transition zone drawn on one map waits for its
    other side (the bar says so); the next one drawn on another map makes the two each
    other's (`Workspace::link_transitions`, a step of each map called *Link house and
    village*). *Link…* on a transition's Inspector makes an existing one wait.
  - **The World view** (`studio/src/worldview.rs`, the *World* group's first entry, tab
    `world`): every map as a card with its picture (a pixel a tile), in columns by how many
    maps lie between it and the start map; portals, transitions and edges as arrows from
    where they start to where they arrive; a way that leads nowhere and a map nothing leads
    to are marked and say why; a click opens the map, cards can be dragged
    (`Workspace::survey_world`).
  - **Play inside the editor** (`studio/src/play.rs`): *Play from here* in the map's menu
    starts the game in the Play tab with the hero on that tile (everything is saved first);
    F5 from the title screen, F6 on the map being worked on. The tab's bar: *Restart*,
    *Pause*, *Step* (a sixtieth of a second), the clock at 1/4 to 4 times, *Inspect* (a
    click on a character, a prop or an item in the running game opens what it is; Alt+click
    at any time), *State*, *Own window* (the real runner, started the same way).
  - **The State panel:** the running game's map, day and hour, flags, variables, quests,
    party and bag, searched and changed while it runs (a flag ticked off, a variable
    dragged, a quest's stage, an item given).
  - **The test setup** (`editor-core/src/testsetup.rs`): *Keep as the test setup* takes what
    the running game holds (the hero's level and class, the bag, flags, variables, quests,
    the clock, the party); *Play from here with the test setup* loads a game in progress
    with it on the tile, so the map's *Map enter* events (an intro) don't run. Kept per
    project in the editor's settings.
  - **The Build tool** (B; `editor-core/src/build.rs`, the panel in `studio/src/buildtool.rs`):
    *Walls* painted along the tile edges (thin walls and fences) or tile by tile (thick
    ones), round corners, Shift for a straight line; a *Room* dragged as its floor, with
    its back walls or all four and a doorway, a floor material and a roof if wanted; a
    *House* dragged as its footprint: a new building prop in the look of a chosen one with
    one to three storeys, its door in the front, windows along the walls, and a wing of the
    house it is dragged against (one roof over both); a *Roof* alone as a rectangle, or over
    tiles painted by hand or found inside walls with a click, any shape, cut into joined
    parts; *Doors*: a door, a window or a gap clicked into a wall piece or a house's wall.
    The right button takes a wall piece, a door or a window away again. Each is one step:
    Ctrl+Z takes a house back with the prop that was made for it.
  - **Copy, cut and paste** (Ctrl+C, Ctrl+X, Ctrl+V, the map's menu): what is selected
    (characters, objects, items, scenery, zones) is put down under the pointer, on the same
    map or another one, with new numbers and names that are free there
    (`Workspace::copy_objects`, `paste_objects`).
  - **Replace with…** (Inspector): another definition of its kind in a thing's place, where
    it stands, with its name and events; several selected at once. **In a row / In a
    column** put several things in line with the first one selected.
  - **A placed prop's state** is picked among its definition's states.
  - **A character's day on the map:** *Add waypoints* makes the map's clicks add to its
    patrol route (the right button takes the last one away); waypoints and the stops of its
    daily schedule are dragged where they are drawn.
  - **The World view's cards lie where they were left** (noted in `editor/meta.ron` with
    the creator's folders and tags; *Tidy* forgets the places).
  Still open: a structure's ghost (its footprint is shown); hiding and locking single
  things in the Outliner; schedule stops added by clicking the map (they are added in the
  Inspector, then dragged); a region of tiles copied with its ground (*Save as structure*
  does that by name). Of playing: the test setup's own form (it is made by playing), the
  console and script errors as panels (part 7), a click in the game leading to the thing's
  place on its map (it opens its definition).

  **Decided while building.**
  - **The world is drawn by the engine, the editor's marks by both.** What lies on the ground
    (zones, the grid, blocked tiles, frames) is drawn in the same pass with the ground's
    camera; names and handles are egui's, so they stay sharp at any zoom.
  - **A map's view state lives with its tab**, not in the workspace: two tabs of two maps
    each have a camera, a tool and a selection. The map pressed on becomes the one being
    worked on (commands and agents act on it).
  - **The tools' and the view's switches are different words** (*Zones* the tool, *Zone
    areas* the switch): the UI test that clicks them by name found the two *Zones* buttons.
  - **What a prop is decides how it is placed**, not which tool was picked first:
    `Workspace::prop_acts`. The creator can still turn either into the other.
  - **Scenery has no number**, so a selected piece is told by what it is (`Selection::Scenery`
    holds the decoration); moving it changes the selection with it.
  - **The game in the tab reads the project's files**, like the runner: playing saves first.
    Its clock is the tab's: `Game::step` (new) moves the game on by hand, several times a
    frame to fast-forward; `Game::thing_under_pointer` (new) says what a click would open.
  - **A test setup is made by playing**, not by filling in a form first: play to where the
    test starts, keep the state.
  - **A value that may be left out is one control with a way to say "none"**, also when it
    is plain text or a number: the schema the forms read writes `Option<String>` like every
    other optional thing (`tidy_schema`). The map's fight music showed it: it was a bare
    text field, it is a sound picker now, and the rename check found that renames follow it.

  - **Changes one action makes in two places are one step** (`Change::joined` in the
    timeline): a built house is a new prop and a thing on the map, two linked transitions
    are a change of each map. Undo and redo take them together; the History panel still
    lists both.
  - **Building models follow their definitions** (`Workspace::sync_buildings`): a shape
    changed in a form, or brought back by undo, is built again where it is looked at. The
    0.35 editor did that from each place that changed one.
  - **Undoing a change to pictures reads those pictures**, not every asset: a built house
    comes and goes with the picture rendered of it, and reading everything took 1.8 s on
    the example (measured). Materials and sounds still read everything.
  - **Maps are linked where the ways are**, by drawing the two transition zones; linking by
    dragging one card of the World view onto another was dropped: it could not say where on
    either map the way is.
  - **Arranging the World view is no step of the history:** a card's place is written at
    once and changes nothing of the game.
  - **A room of thick walls stands around the floor that was dragged**, and its roof reaches
    over the walls. In 0.35 thick walls stood on the floor's own north and west tiles and
    one tile past its corner. Thin walls stand on the floor's edges as before.
  - **A door in a wall closes the room** for *Inside walls*; a doorway left open doesn't
    (the room's own *With a roof* doesn't need the walls closed).
  - **What was built is saved as a structure with the Select tool** (a box around it, *Save
    as structure*): the Build tool's own *Built here* area of 0.35 is gone.
  - **A roof alone is drawn after what stands under it** (agreed 2026-11-27): in 0.35 and in
    the game, wall pieces in the southern and eastern rows under a roof were drawn over its
    eaves, and someone in that corner of the room over the roof, because things with
    overlapping footprints went by their feet. `iso::Footprint` has a `base` now (how high
    its underside is: a building without walls rests at its *height*), and what is raised
    comes after what stands under it. With the hero inside, the roof is cut away as before.

  **Engine.** `Game` embeddable (part 1). The canvas draws through the engine's renderer into a
  texture, as the old `canvas.rs` drew on the screen. Added for the Play tab: `Game::step`
  (devices and one update, what `frame` does before drawing) and `Game::thing_under_pointer`.
  For the Build tool: `Assets::load_sheet` (one picture read again, a built house's, instead
  of every asset); `iso::Footprint::raised` and `PropDef::raised_by` (the draw order of roofs).

  **Verify.** Editor script: each tool on a scratch copy (paint, place, turn, link two maps),
  `status` fields as today; the Play panel started from a tile, a flag set in the State panel
  changes a dialog; `egui_kittest` snapshot of the canvas with the Outliner. Frame times of the
  canvas on the largest map.
  - Done so far: `things_are_placed_changed_and_removed_on_a_map_step_by_step` (the core:
    placing on the grid, refusals, the Inspector's round trip, a zone cut to the map,
    copies, six named steps undone to a clean map); `egui_kittest` on the tab without a
    graphics adapter (`mapview::ui_tests`: the Outliner selects, the form shows a
    character's sections and not a prop's, a typed name is a step, *Delete* and undo; the
    tools' lists arm what is clicked). In the window on a copy of the example, by real
    clicks at tiles (`mapclick`, `mapdrag`): a guard placed, dragged two tiles, turned,
    duplicated, a spawn zone drawn, collision painted with both buttons, each undone by its
    name; the village by night with its lamps, from above, the house fitted. The village
    (80 x 60 tiles, 130 things, 475 pieces of scenery) draws in 0.4 to 1.9 ms a frame (CPU).
    The agents' smoke client passes with `screenshot_map` answered (725 x 459 px).
  - `scenery_ground_scatter_and_structures_are_steps_of_a_map` (a rock as scenery, made
    interactive and back, refused while it has a name; moved scenery told by its new place;
    a rectangle of cobblestone on tiles; two scatter dabs as one step; a structure; all
    undone to the map as it was). `mapview::ui_tests`: the Terrain list picks a material;
    the Outliner's *Young Oak x 311* selects them all, *Make interactive* and *Make scenery*
    are two named steps. In the window: a cobblestone stroke, a pond as a rectangle on
    tiles, a field as a line, a fill; an oak, a row of fence, a forest brushed on, a small
    cottage, a chest (placed as an object); a tree picked by a click and dragged three
    tiles; twelve steps undone by their names.
  - Playing, in the window: started on a village tile (the intro runs, as in a new game);
    `shrine_lit` set and `mira_talks` given a value through the State's functions, both in
    the game's own status, the achievement that waits for the flag unlocked at once; kept
    as the test setup (`L1:items0:flags2:vars1…`) and there again in the next game, which
    starts without the intro; paused, the game's clock stands at 08:08 through a second and
    a half and one step, runs eight game minutes in two seconds at 4x and none to speak of
    at 1/4; the elder answers E; an *Inspect* click on a market stall opens the stall's
    definition. `a_crafted_save_has_the_setup_and_finds_the_party` (and a running game's
    state gives the setup back). Not shown: a line of dialog changing with a flag set in
    the panel (the scripted hero did not get to face the walking child whose line it is).
  - `the_world_is_surveyed_and_two_maps_are_linked` (the example: 19 maps all reached from
    the village, no way that leads nowhere; two unlinked transitions are two problems,
    linked they are a pair, as two steps named *Link house and village*; the map being
    worked on stays). `references_agree_with_renames` covers the map's settings (music,
    fight music, an edge's map, a parallax layer's sheet). In the window: a transition
    drawn in the house and one in the village link themselves; the World view shows 19 maps
    and 38 ways; the village's settings in the Inspector; the elder's day (07:00 by the
    well, 13:00 on the bench) drawn on the square. `mapview::ui_tests`: 311 oaks selected
    and saved as *Oak grove*, undone by its name. A Town Guard dragged from the Project
    browser's row onto the village stands where it was let go.
  - `things_are_copied_to_another_map_replaced_and_put_in_a_row`,
    `a_characters_route_and_day_are_changed_on_the_map`,
    `a_maps_card_place_is_noted_with_the_creators_order`; `mapview::ui_tests`: two
    characters put in a row, copied and pasted, one replaced by a guard picked from the
    searched list, all undone. In the window: the elder copied in the village and pasted
    there and in the house (undone by its name); four clicks add waypoints 5 to 8 to the
    guard's route, the right button takes the last away, one is dragged, Esc ends; the
    chest's Inspector offers its states; a card of the World view dragged lies there after
    a restart. Not done by real keys: Ctrl+C / Ctrl+V (the script calls what they call).
  - The roof's order: `a_roof_alone_is_drawn_after_what_stands_under_it` (wall pieces in
    its southern and eastern rows and someone inside near that corner are covered; what
    stands in front of it is still in front). In the window: a room of timber walls with
    its roof, no wall top over the eaves; played with the hero inside, the roof and the
    front walls are cut away around him.
  - The Build tool: `walls_rooms_houses_and_roofs_are_built_as_one_step_each` (a thin wall
    round a corner, a piece taken away, a room of thick walls with a floor on a free site,
    its inside found once the doorway is walled up, a window piece and a door put in and
    taken out, a two-storey house undone and redone as one, a wing joined to it, an L of
    roof in two joined parts, a roof as high as the walls under it; all undone to the map and
    the definitions as they were), the 0.35 tool's eight unit tests (`build::tests`), and
    `mapview::ui_tests` (the panel's lists pick a wall and a look, marked tiles get their
    roof with *Build roof*, one undo takes it and its props away). In the window by real
    drags and clicks at tiles (`mapbuild`, `mapdrag`, `mapclick … right`): a stone wall, a
    room of plaster panels closed by four drags, a door and a window clicked into it, its
    12 tiles found by a click inside and roofed, the door taken out again; rooms of stone
    and timber walls with a floor and a roof; a painted L of roof built with Enter; a
    two-storey cottage; a stone house with a wing over one roof and a window clicked into
    its wall. Not shown: a built house played in the game from the editor (it is the same
    prop data 0.35's tool made).
- [~] Part 7: Events, conversations and scripts — `feature/studio`

  **Goal.** What happens in the game is as easy to write and to read as it is to place a prop.

  **Design.**
  - **The event sheet** replaces the trigger editor (`widgets.rs`): *When* / *If* / *Do* as rows
    with a one-line summary each, opened in place; drag to reorder, fold, select several, copy
    between characters and maps; the action palette searched and grouped as `07-events.md`;
    *Try it* plays from here with the trigger fired.
  - **Conversations:** runs of *Say* and *Choice* actions shown as a conversation: who speaks
    (with the portrait), the branches of a choice side by side, conditions on them; written like
    a script, line after line. Beside it the game's own dialog box drawing the selected line
    (fonts, skin, message codes). Stored as the actions they are.
  - **Flags and variables:** a page listing every flag, variable and text variable with where it
    is set and where it is read (the API table's collectors and `refactor.rs`), a note for each,
    and rename.
  - **Scripts:** files in tabs, the functions of a file as an outline, completion and
    documentation from the API table, errors at the line, *Go to* for ids, a function run in the
    Play panel.
  - **Documents of their own:** the map's Events graph (`graph.rs`) and visual scripts
    (`nodes.rs`) on one node canvas, the cut-scene timeline, the animation editor, the quest
    designer.

  **Progress.** The event sheet (`studio/src/sheet.rs` over `editor-core/src/events.rs`): every
  list of triggers, conditions and actions in every form is shown by it (a placed character's
  Inspector, a zone, the map's own events, a definition, the branches of an *If*).
  - **An event is a line**, *When the player uses it or talks to it, if the flag met_elder is
    not set · 3 actions*, opened as **When** (the event and what it names), **If** (its
    conditions; none: *always*) and **Do** (its actions), with *Only once* under them.
  - **A condition or an action is a line that says what it does** (`events::action_summary`,
    `condition_summary`, `trigger_summary`): *Elder: "Welcome, traveller."*, *Give 2 x Healing
    Potion*, *It changes to the state lit*, *If the flag … is not set: 2 actions, else 1*,
    definitions by their names. Kinds without words of their own show their name and plain
    values. A click opens the row in place with its fields (one row of a list at a time).
  - **Rows are dragged** to another place in their list; a row's menu moves it up and down,
    duplicates, copies, cuts, pastes above and deletes; *Paste* beside *+ Add…* puts the
    copied row at the end of any list of its kind: another event, another character,
    another map. Whole events are copied the same way.
  - **+ Add an action…** is a searched palette in the manual's groups (Talking, Game state,
    Entities and the world, Cut-scenes and effects, Rules, Magic), each kind with the engine's
    own comment as its help and starting as the engine's template (a count of 1, not 0).
    **+ Add an event…** offers the events that reach that owner, the likeliest first, as
    sentences. Lists of events are open in a form also while they are empty.
  - **Scripts in tabs** (`studio/src/scriptedit.rs` over `editor-core/src/scripts.rs`; the
    Project browser's *Story › Scripts*, with *+ New script*): a file with its colours and
    line numbers, its functions as an outline, suggestions while typing (the script API with
    its documentation, the project's own functions, and inside a string argument the ids it
    can name: items by id and name, maps, a map's spawn points, flags, states…), the
    signature of the call being written with the current parameter marked, brackets and
    quotes closed, new lines indented, Ctrl+click on a function to go to it (also into
    another file). The text is checked a moment after typing stops: ids that aren't there,
    functions named in strings that no script defines, what the compiler says, each
    underlined and listed with its line. Scripts are saved with everything (Ctrl+S, before
    playing); a problem of the project check that is in a script opens its tab at the
    place; a file changed outside (a rename, an agent, another program) is taken, or asked
    about when the tab has changes of its own.
  - **Conversations are written in the sheet**, line after line: a *Dialog* row is who
    speaks (typed, or picked from who has spoken here and the names that fill themselves in;
    a character of that name shows its face) and what is said, changed where it stands;
    *+ Line* adds the next one, spoken by the other of two people who take turns. A
    *Choice* is a line that *asks*, with its answers under it and under each answer what it
    leads to (lines, any actions, another question): the *If* behind the question that
    asks for that answer, made by *+ then…* and shown there instead of as a row of its own.
    Taking an answer away takes what it leads to with it and moves the later answers up.
  - **Try it** on an event of a placed thing, a zone or the map (the Inspector): the game
    starts in the Play tab with the hero beside the thing (in the zone) and the event raised
    a moment later, with the test setup's flags and things, so what the event does is seen
    at once; when its conditions don't hold in that game, the bar says that nothing ran.
  - **The cut-scene timeline** (`studio/src/timeline.rs` over `editor-core/src/cutscene.rs`;
    at the top of a cut-scene's tab): a row per track with its cues as boxes on a time axis
    (waiting ones blue, the others green, close ones stacked in lanes); a click or a drag on
    the axis moves the playhead; a cue is picked with a click, moved in time by dragging and
    changed under the rows (when, whether it waits, its action's fields); a track's name
    adds cues at the playhead from what that kind of track offers. *Show its map* puts the
    map's tab above the timeline, looking at the moment: each actor where it is then (a
    ring with its name, thin while walking) and the camera's frame. *Play from here* plays
    the cut-scene in the Play tab from the playhead.
  - **Flags and variables** (`studio/src/namespage.rs` over `editor-core/src/names.rs`;
    *Story › Flags and variables*, tab `names`): every flag, variable and text variable with
    how often it is set and read; one that is only ever set or only ever read is marked
    (mostly a name typed two ways); a note for each, kept in `editor/meta.ron`. The one
    opened lists every place it stands (a definition, a placed thing, a zone, a map's own
    events, a script's line), each a link there, and *Rename everywhere* follows it through
    definitions, maps and scripts as one step, undone by renaming back.
  - **Visual scripts on a canvas** (`studio/src/nodesview.rs` over
    `editor-core/src/visualedit.rs`; a visual script's tab): its name, parameters and result
    as a form on the left, its nodes in the middle, and on the right what the selected node
    does, what is wrong (a click shows the node) and the Rhai the game runs. The background
    dragged moves the view, Ctrl+wheel zooms; a node is moved by its title (every selected
    one with it), linked by dragging from a pin to another, and a link let go in empty space
    opens the palette with the nodes that take it. Right-click or Space opens the palette at
    the pointer: searched, in the manual's sections (flow, values, maths, the project's
    visual scripts and script functions, the script API). An input without a link is typed
    into the node (a number, a tick, a text; an id from what the project has); Delete takes
    the selected nodes away, Ctrl+C / Ctrl+V / Ctrl+D copy them with the links between
    them, *Group into a function* (Ctrl+G) makes them a visual script of their own, called
    in their place; a node that calls a visual script opens it with a double click.
  Still open: rows dragged from one list into another and several rows selected at once
  (copy and paste does it one by one); *Try it* for a definition's events (they have no
  place to be tried from: a placed one of them is); of conversations: the game's own dialog
  box drawing the line being written (fonts, skin, message codes), answers side by side; of
  flags and variables: `{name}` placeholders in texts aren't counted as reading one; of scripts: a
  function run in the Play tab, *Find in scripts*; of visual scripts: a box drawn around
  nodes to select them, *Fit all*; the
  documents still to come: the map's Events graph (on the visual scripts' canvas), the
  animation editor, the quest designer.

  **Decided while building.**
  - **The sheet reads the values as the files hold them** (JSON, as the forms do), so it is
    one piece of code for every place a list of events can be, and a kind the summaries
    don't know yet still gets a line.
  - **The palette offers what the engine's templates offer**: *Give skill points* and *End
    cut-scene* are not in them (old data keeps them and shows them).
  - **The sheet's clipboard is the editor's own**, kept while it runs; it holds one row of
    each kind (an event, a condition, an action).
  - **A conversation is not a view of its own**: it is how the sheet shows talk, so a line,
    a question and what follows them are written where every other action is, in every
    place events are edited. It stays the actions it was: a branch is an *If* that asks for
    one answer, and an *If* that asks for more is a row like any other.
  - **A visual script is a tab like any definition**, not a mode with its own list as in
    0.35: the Project browser's *Story › Visual scripts* lists them, *New…* makes one, and
    its form (name, parameters, what it gives back) stands beside its canvas.
  - **A drag is one step of the history** (`shell::hold_step`): a node moved over many
    frames, or a cue on the timeline, is undone with one Ctrl+Z, the way typing on in one
    field is one step.
  - **The canvas's thinking is in the core** (what the palette offers and for which link,
    linking with its type checks, adding, copying, grouping): tested without a window, and
    what the script's words and the canvas both call.
  - **A cut-scene's timeline is part of its tab, and its map is a tab beside it**, not a
    window over the map view as in 0.35: the map's tab shows a cut-scene's moment for as
    long as the timeline is in view, so the two are arranged like any other two tabs.
  - **The camera's frame is drawn as the screen's**: in the game's view a rectangle on the
    ground is a diamond on the screen, so the frame is the game's window around the
    camera's middle at the size this view shows the world.
  - **A field is a flag or a variable because the field schema says so** (`schema::NAMES`,
    the table the forms' suggestions use), in scripts because the script API's argument
    kinds say so: the page, the survey and the rename are one walk over the values as the
    files hold them, so a new action that names a flag is found once that table knows it.
    An action sets, a condition reads.
  - **Renaming onto a name that exists is refused**: the two would become one, and renaming
    back could not part them again.
  - **A rename is written at once** (definitions, map files, scripts with a `.bak` of each
    changed file), as the rename of an id is.
  - **A script's text has its own undo** (the text field's, Ctrl+Z while writing): it is not
    a step of the project's history, as in 0.35. The tab keeps what was typed until it is
    saved or the editor closes.
  - **The script editor's thinking is in the core** (where the cursor is, what to suggest,
    what typing adds, how a script is painted, what is wrong in a text): tested without a
    window, and there for agents.

  - **Try it raises the event, it doesn't run one trigger's actions**: every trigger of the
    thing that waits for that event runs if its conditions hold, as in play. It starts from
    the test setup (a game in progress), so a map's intro doesn't play first.

  **Engine.** `ScriptHost::compile_check` is no longer for tests only (a script is checked
  before it is saved). `Game::raise(event, owner)` (with `EventOwner`): raises the event a
  trigger waits for as if it had just happened to a thing, a zone or the map, for *Try it*. Maybe an *off* mark on an action (a row switched off while testing),
  asked for when it is built.

  **Verify.** `egui_kittest`: build a trigger with a condition and three actions, reorder, copy to
  another character, undo; write a conversation with a choice and play it; rename a flag from its
  page (script and triggers follow). Screenshots.
  - Done so far: `events::tests` (every action the engine offers is in exactly one group and
    has a line, every event and condition has one, fifteen lines word for word);
    `sheet::ui_tests` (an event built from the lists: a condition, three actions from the
    grouped palette, a row closed and moved from its menu, duplicated, the event copied to
    a prop's list and removed; rows keep their place when one is moved, pasted or removed);
    `an_event_added_in_the_inspector_is_a_step_of_the_map` (a guard gets *When it is
    defeated* and *Give experience* from the searched palette; undone, he is as he was). In
    the window: the campfire's four events read as sentences, one opened with its *If* and
    three *Do* rows, an action opened in place, the palette searched for "stat"; the guard's
    events in the Inspector and the list of events to add.
  - The timeline: `cutscene::tests` (0.35's: cues close together stack in lanes; the preview
    walks actors and pans the camera); `timeline::ui_tests` (a cue picked by its box, its
    *Waits* changed, a track added for the guard, a cue for him from the track's menu at
    the playhead, deleted, *Play from here* asked for at 2.5 s). In the window on the
    example's intro (5 tracks, 17 cues): at 7.6 s the hero at 3.5, 30 and the guard at 7, 30
    walking, the camera at 0, 30, shown on the village above the timeline; a *Face* cue
    added and undone by its name; played from 7.6 s, the Play tab shows the two meeting and
    the hero's line.
  - *Try it*: `an_event_of_a_placed_thing_is_tried_from_beside_it` (the Inspector's button
    asks for the game on a free tile beside the guard with his event). In the window: the
    guard's event tried from the map; the Play tab opens with the hero west of him and his
    line "Halt! Oh, it's you, Alex…" in the game's dialog box.
  - Conversations: `a_choices_answers_lead_to_their_branches` (branches found per answer,
    a second question in the same variable keeping its own, where a new branch goes, an
    answer taken away with its branch and the later ones moved up, speakers taking turns);
    `a_conversation_is_written_line_by_line_with_a_question_and_its_answers` (three lines
    typed, the third the first speaker's again, a question with two answers, *+ then…*
    making the branch behind it with a variable made up, a line written into the branch).
    In the window: a conversation of two lines, a question with two answers and what each
    leads to, read as written; put on the square's signpost and tried from the map, it
    plays in the game: the line, the question, and after the first answer "Take this
    potion." with the speaker's portrait.
  - Flags and variables: `names::tests` (names in a script by what each call does with
    them; a flag renamed where it is a flag, a text of the same letters left alone);
    `flags_and_variables_are_surveyed_and_renamed_everywhere` (the example: 55 names
    surveyed in 0.09 s, every flag the pickers offer among them; `shrine_lit` renamed in
    its 9 places, the event of the same name a script raises kept, a fresh look at the
    files agreeing; a note following a second rename; all undone, the script as it was);
    `namespage::ui_tests` (searched, opened, a place followed to the house's chest, a note
    typed, renamed from its row). In the window: the page with 24 flags, 27 variables, 4
    text variables, 13 marked; a rename and its undo by the script's words.
  - Scripts: `scripts::tests` (the 0.35 editor's tests of the cursor's context and of the
    suggestions, how a script is painted, brackets closed and lines indented, a suggestion
    taken); `scriptedit::ui_tests` (a script's functions as its outline, an item that isn't
    there said with its line, the text put right and saved, the project knowing its new
    functions, a compile error at its line; a file changed outside taken, or asked about
    and kept). In the window: the example's `main.rhai` (50 functions), `give_item("po`
    typed after a line: six items offered by id and name, the signature with *item* marked,
    then *count*; Enter takes the first; Ctrl+S writes the file.
  - Visual scripts: `visualedit::tests` (the palette in the manual's sections, links that
    replace what an input had and refuse wrong types, nodes added, linked, copied and found
    for a dragged link); `nodesview::ui_tests` (a node added from the searched palette at
    the pointer, linked by a drag from pin to pin, its item typed in, the Rhai saying so;
    moved by its title as one step; Delete taking it and its link but never Start; undone).
    In the window on the example's *Brother Ansel's blessing* (16 nodes, 15 links): read on
    the canvas with its Rhai beside it; a node moved (one step), `give_item` added from the
    palette and linked from Start by a drag, a link let go in empty space offering only what
    takes it, Delete and undo; three nodes grouped into a function of their own (the Rhai
    calls it), saved: `scripts/visual_scripts.rhai` holds both and the game starts with them.

- [~] Part 8: Workshops — `feature/studio`

  **Goal.** Making a prop, a structure or a character is done on the thing itself, not in number
  fields.

  **Design.**
  - **The prop workshop:** the prop large, as the game draws it. On it: the hitbox dragged by its
    corners, the anchor, the spot it is used from, the light's place and reach. The states as a
    strip, each with its preview.
    - **Starts from:** a model file dropped in, a picture, measurements (a building: ridge,
      overhang and openings dragged on the preview), a copy, Meshy.
    - **What it is:** presets fill the fields and triggers of a *Container*, *Door with a lock*,
      *Light*, *Seat*, *Bed*, *Crop*, *Gathering spot*, *Sign*, *Trap*. Presets are templates in
      the library (content), so more can be added and shared.
  - **Structures:** select on a map, *Save as structure*; a structure opens as a document on the
    map canvas (as 0.35 does); its thumbnail renders itself; houses of the Build tool save the
    same way.
  - **The character workshop:** one document for look (model, pieces, colours), rules (stat block
    or class), day (schedule, route), talk, shop, loot; presets *Villager*, *Merchant*, *Guard*,
    *Companion*, *Monster* (from the library's SRD monsters).
  - **Items, spells, quests:** templates in *New…* (*Weapon*, *Potion*, *Key*, *Scroll of…*).
  - **Import helpers:** a model's check (size, rig, clips, what is missing and its fix), a
    sound's loop heard, a picture's frames cut.
  - **Pictures:** the pixel editor as a document (icons, seamless material textures).
  - **Generate art:** a panel with the jobs, opened from each workshop's *Generate…*.

  **Engine.** None planned; the presets are library content (*Content route*). Added for the
  editor: `audio::load_one(path)` (one sound file decoded with its length, to be listened to
  without loading every sound of the project).

  **Progress.**
  - **The prop workshop** (`studio/src/propshop.rs`; at the top of a prop's tab, above its
    form): the prop large on a few tiles, drawn by the game's renderer as a placed one is
    (its picture or model, its shadow, a building's walls and roof). On the picture: the
    hitbox as its outline on the ground (red when that state is in the way, green when not)
    with its corners to drag, the spot it is used from as a ring with the way its user
    looks, and the light's reach as a circle with a point to drag. Its states are a strip of
    pictures above; the one picked is the one shown, *+ State* adds one that starts as a
    copy of the first. *Turn* shows a prop that turns a quarter further, *In the dark* shows
    it at night, lit by its own light, from far enough away to see the whole reach. Drags
    go in steps of four pixels (a quarter of a tile for the light), Shift pixel by pixel.
    The light's middle (a dot in a ring) is dragged to where it sits on the ground, and the
    foot of a prop drawn as a picture (a cross) to where the picture stands on its hitbox,
    a pixel of the picture at a time; *Light / Picture back to its usual place* undo that.
  - **The character workshop, the look** (`studio/src/charshop.rs` over
    `editor-core/src/looks.rs`; at the top of a character's tab): the figure large as the
    game draws it, with what it wears and holds, turning or seen from one side, walking,
    standing, striking, hurt. Beside it the look as things to pick: the model from cards of
    the project's figures (searched; another model's colours and pieces replace the old
    one's), each slot the model can wear a piece in with the pieces that fit (*the model's
    own*, *None*, a piece), and each colour feature of the model and its pieces as swatches
    of the colours it offers, *As drawn*, any other colour, and *Random colours*. Under
    them what it wears and holds: a box per slot with the item's picture (weapon, left hand,
    head, body, clothing, backpack; *More slots* for a tool, ammunition and what is worn
    besides); a click offers the items that go there as cards, searched, and *Nothing*; the
    figure wears and holds it at once.
  - **A new thing can start as an asset of the library** (*New…*, the third way beside
    *blank* and *a copy*; `Workspace::make_from_library`, command `defs.make KIND library:ID
    NAME`): the library's assets of that kind are listed, searched; the one picked comes
    into the project with what it needs (pictures, sounds, models, the script functions its
    events call) under the name typed, with its own id, as one step. Without a name it is
    added as it is. So the library's chest, door, lamp, bed, crop or merchant is the
    template a new one is made from, and a template of one's own is an asset sent to the
    library. The one picked is shown beside the list as the library has it: a prop's
    picture, an item's icon, a character's figure.
  - **A sound is heard and a sheet is cut where they are defined** (`studio/src/artshop.rs`):
    a sound's tab starts with *Play*, *Loop* (to listen for the seam) and *Stop*, its length
    and file, a bar while it plays, and what is wrong with a file that can't be played. A
    sprite sheet's tab starts with its picture under the grid its frames are cut by, their
    numbers on them: *across* and *down* cut it again (the frame's size follows; pixels
    that belong to no frame are said), a click picks a frame, its animations are listed and
    the one picked plays beside the picture; *+ Animation from this frame* adds one to the
    end of its row.
  - **A new thing starts from a file** (`editor-core/src/fromfile.rs`; *New…*, the fourth
    way; command `defs.make KIND file:PATH NAME`): a file dropped on the editor's window
    opens *New…* with it, named after the file. A 3D model (`.glb`) becomes a prop that
    shows it: stood on the ground in the middle of its footprint, one tile wide and as deep
    as the model, its pictures for lists drawn from it. A picture (`.png`) becomes a prop
    standing on its lower edge with the sprite sheet that holds it (a large picture is
    shown smaller, through the sheet's scale; its hitbox starts as wide as the foot of what
    is painted: a tree's trunk, not its crown), or a sprite sheet alone. A sound (`.ogg`,
    `.wav`) becomes a sound. The file is copied into the project; the new thing's tab
    opens, a prop's with its workshop. *Browse…* picks the file in the system's file dialog
    (`studio/src/pick.rs`) instead; a new thing without a name yet is named after it.
  - **Generate art** (`studio/src/artpage.rs` over `editor-core/src/art/`; the page *Generate
    art* of the Game group, tab `art`): what Meshy is asked for with the creator's own
    account: an item's icon, a portrait, a painted picture, a 3D character, a creature, an
    item's or a prop's 3D model, more clips for a model. What a kind is made for is picked
    (the item, the prop, the model, the character a portrait is of), not typed; the kind's
    options (attack, a woman, extra animations) follow it. The button says what the job
    costs and asks a second time (*Ask a second time before spending*); what stands in the
    way is said before it is tried (no key, the session's budget, what the request lacks).
    The account (key, balance, the model that draws, the budget) folds away once a key is
    there. Jobs are listed with their steps' progress, the result's picture, *Again* for a
    step with its cost, and *Add to project*, which writes the files, makes the definitions
    and opens what it became. *Generate art…* in a prop's, item's, character's or rigged
    model's tab opens the page with that thing filled in.
  - **The pixel editor** (`studio/src/pixeledit.rs` over `editor-core/src/pixels.rs`; *Paint*
    under a sprite sheet's picture): the frame picked in the picture, large, on a chequered
    ground, painted pixel by pixel: pencil, eraser, fill, line, rectangle, colour picker (keys
    B E G L R I), the first colour on the left button and the second on the right, swapped
    with X; the picture's own colours as swatches, then a few to start from. What is drawn
    stays inside the frame. The frame as a whole is copied, pasted into another, mirrored,
    emptied and moved a pixel at a time. The picture has its own undo (the buttons, or
    Ctrl+Z with the pointer over it), as a script's text has; *Save picture*, or saving
    everything, writes the file, and the sheet shows the new picture everywhere at once.
    A ground material's tab has the same editor for its texture, **across its seams**: the
    texture is shown with part of itself again on every side (a little darker), a stroke
    that leaves on one side comes in on the other, a fill goes on through the edges, and
    *Left / Up / Down / Right* move the seam into the middle; an animated material's frames
    are picked there. Saved, the ground of every open map shows it at once
    (`Assets::load_material`). The picture is shown as large as fits, or closer with *+*
    (it scrolls then).
  - **The model workshop** (`studio/src/modelshop.rs` over `editor-core/src/modelcheck.rs`;
    at the top of a 3D model's tab): the model as the game draws it, turning or from one
    side, playing whichever of its animations is clicked (a piece is shown on a figure it
    fits). Beside it what the check found (`Models::check`: its size against what one draw
    takes, its palette, colour features that match no colour, a missing clip for one of the
    four animations every figure plays, a piece's bones a body lacks), the problems first,
    the rest folded under *What it is*. The fixes are at hand: each animation's clip is
    picked from the clips its files hold (with their lengths) instead of typed, and how
    fast it is drawn; *Guess from the clips' names* maps every animation without a clip
    ("Walking" for walk, "Hit_Reaction" for hurt); *+ Animation* adds one of the kinds the
    game knows (work, rest, weapon stances, sneaking) with its likeliest clip; a model
    without a skeleton is offered the rig templates (four legs, a bird, a flyer…: its
    animations come with the skeleton) and *Turn it* until it faces the front. A `.glb`
    also becomes a 3D model for characters (*New… › 3D model › from a file*, or dropped):
    copied in, made smaller, its animations guessed, its tab open on what is missing.
  - **A structure is worked on as a map** (`editor-core/src/structdoc.rs`; a structure's
    tab): its tab is the structure itself as a small map of its size, with every tool of a
    map: paint its ground, place and move things, build walls and roofs on it, set blocked
    tiles; the Outliner lists what it holds and the Inspector changes what is selected.
    With nothing selected the Inspector has its name, its category and its width and depth
    in tiles (smaller cuts off what stood on the tiles that go). Every finished change is a
    step of the editor's history under the name of what was done (*Place Chest*, *Paint
    Dirt*); its picture, and the structure the Place tool puts down, follow at once. A new
    structure (*New… › Structure*) starts as a few empty tiles to build on. Around its own
    tiles (outlined) its map has a tile of room for what hangs over its edge: the example's
    garden has a fence a pixel off its western side, which stands there and can be moved;
    what is put there is part of the structure, ground painted and tiles blocked there are
    not (they go again, and the editor says why). The Zones tool isn't offered: a structure
    holds no zones.
  - **Structures have pictures** (`thumbs.rs`): a structure is placed on a map of its own
    and drawn as a map is (its ground, what lies and stands on it), once and again when it
    changed, two a frame: its card in *Structures*, in the Place tool's palette and at the
    top of its tab. (Saving a selection of a map as a structure and placing one are part 6.)
  Still open: a figure sitting or lying at the use spot instead of the ring, a
  building's ridge, overhang and openings dragged on the picture, how large a prop's model
  is shown as a handle; starting from measurements and Meshy, an item's model from a file, pictures
  other than PNG; of a model's workshop the joints of a fitted skeleton moved by hand, clips
  borrowed from another model, more clip files picked, an attack's hit point, colour
  features marked on the model; a sound's volume and loop points set by ear; of structures: a house of the Build
  tool saved as one from the tool itself; of the character workshop: rules,
  day, talk, shop and loot as parts of one document (they are sections of the form below),
  *Start from…* another character; presets that are more than
  one asset (a *Merchant* with a shop and a schedule to fill in), pictures in the rows of the
  library's list in *New…* and on the Library tab (the one picked has one); of the pixel editor an area selected and
  moved, a frame added to a sheet, the wheel zooming at the pointer, a material's texture
  seen on a patch of ground while it is painted; of Generate art: *from Meshy* as a way to
  start in *New…*, a figure sheet picked to model from, a job's 3D result turned in a
  preview before it is added.

  **Decided while building.**
  - **The workshop is the top of the prop's tab, not a tab of its own**: the picture and the
    form show the same prop, so a number typed below moves the outline above and the other
    way round, and there is one place a prop is made in.
  - **The stage is a map of its own with the prop placed on it**, drawn by the map view's
    scene code: what the workshop shows is what a map shows, also for kinds of props that
    get their own way of being drawn later.
  - **The camera holds still while something is dragged** and fits again when it is let go:
    a view that zooms out as the hitbox or the light grows moves the picture under the
    pointer (the first version did: a light dragged from 3 tiles ran away to 16).
  - **The figure is as large as it stands tall, on its feet**: a pose's frame is roomy and
    grows with what is held, so the preview scales by the height of the figure standing
    still and puts its feet on one spot; a spear or a swung arm reaches as far as the box. A
    wolf is longer than tall, and its head reaches further from its feet than its tail: the
    preview also fits twice the further reach of the figure standing still, front and side.
  - **The definition is what is true of a structure, its map is a way to change it**: a
    finished change on the map is taken into the definition as a step of the definitions'
    history, and an undo, a form or an agent changing the definition builds the map again.
    The map's own undo history is not part of the editor's. (The first plan, the map as the
    history and the definition following silently, can't work: the definitions' history
    keeps whole definitions, so an older step undone would have put back a structure from
    before changes it never saw.)
  - **A structure's map has no file and is no map of the project**: it is not saved as one,
    not listed, not part of the world view, not played from (*Play from here* and *Try it*
    are not offered there); it is open for as long as its tab is.
  - **What was done on the map is a step of its own**, whatever has the keyboard: the
    definitions' history joins changes made while one field has it (typing), which made a
    chest placed and a path painted right after one step. Only typing into a field of
    something placed there joins the step before.
  - **A structure's map is a tile larger on every side than the structure**: what a
    structure holds may lie outside its tiles (placing one puts it there too), and a map of
    exactly its size could neither show nor hold that. So a structure's own tiles start at
    1, 1 on its map (`structdoc::MARGIN`, `own_tiles`), and everything standing on the map
    is the structure's, at places counted from its own corner. What lies further out than
    that tile is kept as it is, unseen.
  - **A seamless texture answers for its copies**: a pixel asked for beside the frame is the
    one it comes back in at (`PixelDoc::wrapped`), so strokes, fills, the preview of a line
    and the pointer's mark work on the copies shown around it without knowing of them. A
    small texture is shown with half of itself around, one over 64 pixels with a quarter:
    it needs the room for itself.
  - **Generate art is a page, not a window over the others**: jobs take minutes, and the
    page with its list stays where it is while a prop or a map is worked on; a workshop's
    *Generate art…* leads there and *Add to project* leads back to the thing.
  - **What a job made is written at once** (its files and the definitions, as in 0.35): it
    cost credits, and a crash before the next save shouldn't lose it. It is a step of the
    history all the same.
  - **A picture's undo is its own**, like a script's text: the definitions' history keeps
    definitions, not files, and a picture isn't in the project until it is saved. Ctrl+Z is
    the picture's while the pointer is over it (`pixeledit::has_pointer`), the project's
    elsewhere; *unsaved changes* counts painted pictures, and saving everything writes them.
  - **One frame at a time, and nothing leaves it**: a sheet is frames side by side, and a
    stroke that ran into the next frame would paint a second icon by accident. The frame is
    the one picked in the sheet's picture above, so there is one place a frame is picked.
  - **A model's check is the engine's, its fixes are the editor's**: what a model needs is
    asked of the loaded model (`Models::check`, the agents' `model_check`), so the workshop
    and an agent read the same lines; the workshop adds the ways out. A model changed there
    is taken by the loaded one as it is (`Models::set_def`); only another skeleton reads the
    project's art again.
  - **A dropped file opens *New…*, it doesn't make something at once**: what a file should
    become is a guess (a picture is a prop most of the time, a sheet sometimes), and the
    thing needs a name; the guess and the file's name are filled in, so Enter is enough.
  - **A prop from a 3D model is a change like the others** (`Workspace::show_prop_model`):
    its model's file and the pictures drawn from it are in the project's folder at once, its
    definitions are saved when the project is, and undone they are gone (the files stay, for
    *Clean up unused files*). The first version saved the definitions at once, as the 0.35
    editor did for generated models: that also saved whatever else waited to be saved.
  - **The library asset picked in *New…* is looked at as a small project of its own**
    (`Workspace::library_look`: it and what it needs, read from the library's folder when it
    is picked; `Shell::lib_look` with pictures of its own): its picture is drawn as a
    project's is, a figure included, without loading the library's 300 models for a list.
    The rows of the list have no pictures for the same reason.
  - **Named like the library files it, a new thing from the library is that asset**: *Hero*
    from the library's `hero` is `hero`, not a `hero_2` beside nothing.
  - **Presets are the library's assets, not a list of their own**: *New… › as one of the
    asset library* makes a prop that is a container, a door with a lock, a light or a seat
    by starting from one that is, with its events, sounds and script functions. What was
    planned as presets per kind (*Container*, *Door*, *Villager*, *Potion*…) is then content
    of the starter library, which anyone can add to, instead of code in the editor.
  - **The library's own asset doesn't stay beside the new one**: only what was asked for
    is in the project afterwards (the new one and what it needs); one the project had
    already is left as it is, and the new one is a copy of the project's.
  - **Two fields the engine gained for the workshop** (agreed 2026-11-27): `LightSpec.offset`
    (where a light sits on the ground, pixels east and south of what carries it; not set =
    the rule it had: north of a prop by two thirds of its picture's height) and
    `PropDef.picture_offset` (pixels right and down that a prop's picture is moved from
    standing with its lower edge's middle on the prop's place; its shadow and its clicked
    area move with it). Both are optional, not written when unset, and files of before
    load as they were. Neither turns with a prop turned a quarter, as its use spot doesn't.
    In the isometric view the old rule puts a lamp's light beside the lamp (north on the
    ground is up and to the right on the screen): with its own place the glow lies around
    the lamp's head.
  - **A corner drags the width on both sides**: the hitbox stays centred on where the prop
    stands, as the engine has it; the southern corners change the width only.

  **Verify.** Editor script and `egui_kittest`: a prop from a `.glb` to a placed, opened chest
  without typing a number; a house saved as a structure and placed on another map; a merchant
  from the preset trades in the Play panel. Screenshots of each workshop.
  - Done so far: `propshop::ui_tests` (the chair's north-east corner dragged east: wider in
    steps of four, as deep as before, one step named *Change the prop Chair*; its use spot
    dragged south, a step of its own; *+ State* adds a copy of the first and shows it; all
    undone, the chair as it was; the campfire's reach dragged in the dark grows by what the
    drag is on the ground, the camera not zooming out under it). In the window: the chest
    with its outline and shadow, the chair with its use spot, the campfire lit in the dark
    with its light and reach; the chest's corner dragged (28 to 64 pixels wide, 16 deep as
    before) and undone, the chair's spot dragged and the chair turned, the campfire's reach
    dragged from 3 to 5 tiles and undone; the cottage as a building with its roof, turned.
  - The three paths above, end to end:
    - *A prop from a `.glb` to a placed, opened chest*: in the window `Sea_chest.glb`
      dropped, Enter, a container with two potions given to it (by the script's `field`
      word: in the form it is *+ Add* at *Container* and the items picked), placed on the
      village, played from the test setup beside it: E opens the *Sea chest* with its potions.
    - *A house saved as a structure and placed on another map*:
      `a_house_of_the_build_tool_is_saved_as_a_structure_and_placed_on_another_map` (a 5 x 4
      house built on an empty map, selected, saved as *Farmhouse*, placed on another map:
      the same building stands there; opened as a map of its own it is as saved).
    - *A merchant from the library trades in the Play panel*: in the window a new, empty
      project; *Hero* and *Trader Joan* (the library's outfitter) made from the library, the
      hero named on *Hero & party*, the trader placed on the map, played: walking into her
      and E opens the *Outfitter* shop with its stock and prices (her items and their
      pictures came along).
  - The known limits of the steps above, seen to: `structdoc` tests (the garden's fence a
    pixel off its edge is on its map; a chest put on the tiles around a structure's own is
    part of it, ground painted there goes again without a step), the structure's tab without
    a Zones tool (`a_structure_is_worked_on_as_a_map…`); in the window the garden with all
    four sides of its fence, a barrel put beside it, undone. File dialogs: the gallery's
    *Browse…* answered by `pick::answer_next` in `the_new_gallery_starts_from_a_dropped_file`;
    in the window the home screen's *A folder…* answered with the scratch project opens it.
    **No real dialog was opened**: a scripted run never opens one, so whether the desktop
    portal shows it on this machine is unseen. The library's owlbear and anvil shown in
    *New…* when picked; a wolf's side inside the character preview; a prop from a `.glb`
    not saved until the project is, undone and redone (`fromfile` test).
  - The pixel editor: `pixels` tests (a stroke that runs out of the frame sets only its
    pixels, the frame beside it untouched; rectangle, fill, eraser; undone step by step back
    to the file's picture; a frame copied, pasted, mirrored, moved, recoloured, saved and
    read again), `a_frame_is_painted_with_the_pointer_and_saved` (*Paint*, a colour of the
    palette, a pencil stroke dragged along a row: one step; the second frame picked and
    filled; undo, redo, *Save picture*: the file holds it). In the window: the apple of the
    example's item icons painted with the script's pointer (a stroke dragged, the ground
    filled), saved with everything, the file changed and the sheet's picture with it.
  - A material's texture across its seams: `a_seamless_texture_is_painted_across_its_edges`
    (a stroke out over the right edge comes back in on the left of the same frame, the
    animation's other frame untouched; a line down and one across leave one area, which a
    fill reaches all of; moved half a frame, what leaves comes in), `a_materials_texture_is_
    painted_across_its_seam` (the second frame picked, a stroke dragged onto the copy beside
    it, *Save picture*: the file holds both ends). In the window: a red stroke over the right
    edge of the example's grass (128 x 128), continuous across the seam in the copies;
    saved with everything, the village's ground shows it at once, in every repeat.
  - Generate art, **without Meshy** (`RUSTPG2D_MESHY_FAKE`; no credits were spent and no
    real request was sent): `a_workshop_fills_the_form_and_a_finished_job_is_added_to_the_
    project` (opened for a prop: its kind and the prop, *Draw it: 33 credits*; for a
    character: a portrait of it; a finished icon job listed with its result, *Add to
    project*: the sheet and the item are there, as a step, and the item's tab is asked for;
    the test never clicks *Draw it*). In the window with the fake folder: an icon drawn,
    shown and added as an item; *Generate art…* clicked in the Tavern Sign's tab, the page
    with the prop and its cost, drawn (a picture and a model), added: the prop shows the
    model. **Not seen: a real job** (the key field, the balance, Meshy's own progress).
  - The light's place and the picture's foot: engine tests (`a_light_sits_where_its_spec_
    says_else_by_the_rule`, `defaults_match_the_file_format`: unset they are not written and
    old files read), `a_lights_place_and_a_pictures_foot_are_dragged_and_put_back` (the
    street lamp's picture moved by ten pixels of its own for a drag of ten at the zoom shown,
    its light's middle dragged in steps of four with its reach point following, each a
    step; both put back with their buttons: the lamp as it was). In the window: the lamp in
    the dark with its light dragged from beside it onto its head, its picture dragged off
    its hitbox and undone.
  - The model workshop: `modelcheck` tests (the hero lacks nothing; walking without a clip
    is said and guessed back from the clips' names; a clip picked by hand, an animation of
    its own added and removed; a creature without its rig template lacks all four animations
    and has them again with it, turned a quarter), `modelshop::ui_tests` (the missing
    animation said, its clip picked from the list, *Guess from the clips' names*; the rig
    templates offered to a model without a skeleton). In the window, end to end: the wolf's
    file as `Moor_hound.glb` made a 3D model (four things to see to, no skeleton), the
    four-legged template picked (nothing missing, it walks in the preview), a new character
    given that model, placed on the village and seen standing there in the Play tab.
  - Sounds and sheets: `a_sheet_is_cut_into_frames_and_gets_an_animation` (a picture of
    96 x 64 as one frame cut 3 x 2: frames of 32 x 32, one step; the fourth frame clicked,
    an animation from it to its row's end; a cut that leaves a pixel over says so). In the
    window: the emotions sheet with its 3 x 14 grid, a frame picked, its 14 animations
    listed; the items sheet cut 4 x 4 and undone; the creak's tab: 0.30 s, played, looped
    and stopped by the script's words (sound was switched off for the run: not heard).
  - From a file: `fromfile::tests` (files told apart by their names; a picture four tiles
    wide: a prop on a sheet at scale 2 with the copied file, one step, undone to what was;
    a tree standing on its trunk's width; the same picture as a sheet alone; a `.glb`: a
    prop with its model file and a depth from the model; a sound; what can't be, refused);
    `the_new_gallery_starts_from_a_dropped_file` (a picture dropped is a prop to be, named
    after its file; as a sound it is refused with what it can become, as a sheet made; a
    file that isn't there said). In the window: `Smiths_anvil.glb` dropped, Enter: the
    prop's tab with the anvil on its stage (32 x 28 hitbox), its picture in the header,
    placed on the village; a painted tree's picture dropped: the prop with its shadow.
  - A structure as a map: `structdoc::tests` (every structure of the example goes through a
    map and back with the same things at the same places, also the garden's fence outside
    its tiles; opened and looked at, nothing changes and nothing is a step; a chest placed
    is in the definition as one step *of the definitions*, undone and redone with its map
    following, three placed in a row three steps, saved with the definitions and without a
    map file; a new name leaves the map, another size rebuilds it, deleted it closes);
    `a_structure_is_worked_on_as_a_map_with_its_facts_in_the_inspector` (its facts instead
    of a map's settings, its name typed as one step, a piece of scenery deleted from map
    and definition, undone). In the window: the garden as a map; a chest placed and a dirt
    path painted (two steps, each undone), saved (18 pieces of scenery still, no map file),
    placed on the village with its chest; a new *Hut* sized 6 x 5, painted and furnished.
  - Structures: in the window the example's 37 structures as cards with their pictures
    (cottages with their roofs, walls and fences, furnished rooms on their floors, the
    fishing dock on its water, the lookout tower).
  - New from the library: `a_new_one_starts_as_a_library_asset_under_its_own_name` (the
    library's owlbear as *Grumpy*: its own id, the model it is drawn with in the project,
    no owlbear beside it, one step, undone to exactly what was; without a name added as it
    is; a copy of the chest the project has; what isn't there refused);
    `the_new_gallery_offers_the_library_and_asks_for_one_of_it` (the third way with how
    many there are, what the project has marked, searched, picked, named, asked for). In
    the window: *New character* offering 378 of the library; `defs.make character
    library:owlbear Grumpy` opening Grumpy's tab with the owlbear's figure, and its undo.
  - The character's look: `looks::tests` (a figure's slots by name with the pieces that fit
    it, a piece bringing its colour features once each; another model dropping the old
    one's colours and pieces; random colours being offered ones); `charshop::ui_tests` (the
    guard: no headwear from the slot's list, his clothing in an offered colour and as drawn
    again, seen from the left striking; his weapon's slot offering the weapons, another one
    picked as a step *Change what Town Guard wears*, taken off, undone; another model from
    the searched cards; all undone); `slots_offer_the_items_that_fit_them`.
    In the window: the guard with his spear, five slots and two colour features; without
    headwear (his helmet stays: he wears one as equipment, which takes the slot) and in
    green, striking seen from the left; the cards of the project's figures.

- [~] Part 9: Library and sharing — `feature/studio`

  **Goal.** What a creator makes can be given to others, and what others made can be found,
  installed and kept up to date, from inside the editor.

  **Design.**
  - **One pack format** replaces asset packages (`.r2dpack`), the starter library file (`.r2dlib`)
    and plugin files (`.r2dplugin`). A pack has a manifest (id, title, author, version, licence
    with its attribution text, description, tags, the engine versions it fits, preview pictures,
    the packs it needs) and holds assets with their files and script functions, plugins, and
    templates. Packs and plugin files made with 0.35 can still be installed.
  - **The library** is the installed packs and the creator's own assets. Its panel shows packs as
    cards (preview, version, *update available*) and assets with filters (kind, tag, pack,
    licence), large previews (a model turning, a sound playing), and *Add to project* naming what
    comes along.
  - **Updates:** a project remembers which pack and version each asset came from. The editor
    offers what changed per asset: added, updated, edited by you, retired (`starter_update.rs`,
    for every pack).
  - **The starter library is a pack** (`rustpg2d.starter`), installed and updated the same way;
    `starter_pack.rs`, `starter_ui.rs`, `library_update.rs` and `starter_history.ron` fold into it.
  - **Sources:** a source is a web address with an index file: the packs it offers, each with its
    manifest, download address, size and SHA-256. The editor lists sources (the project's own one
    with the starter pack is preset), browses and searches them, installs and updates, and checks
    every download's SHA-256. No accounts, no server of ours: any static web space works (a GitHub
    repository's pages or releases).
  - **Before installing:** a pack with scripts or plugins says so and lists the functions.
  - **Share…** on any selection (assets, a structure, a plugin): the pack is put together with
    what the selection needs (`library::deps`, the script functions dialog), the manifest filled
    from what was entered last time, previews rendered, and written as a file together with its
    index entry. *Export source folder* writes an index and the packs for a folder to upload.
    A pack file dropped on the editor is installed.
  - **Same id twice:** the same content is skipped; different content asks: replace, keep, or
    install as a copy under a new id (renamed inside the pack).
  - **Licences reach the game:** the packs a game uses are listed with their attribution on the
    Publishing page and offered for the credits.

  **Engine.** `packs.rs` out of `library.rs`, `starter_pack.rs`, `plugins.rs` (pack / unpack) and
  `starter_update.rs`; the origin of assets in `editor/meta.ron`; `xtask` writes the starter pack
  and the official index; HTTPS with `ureq` as `art/meshy.rs` and the starter download do.

  **Progress.** A first step, over the library as it is (no new format yet): the **Library
  tab** (`studio/src/librarypage.rs`; the *Game* group's *Asset library*, tab `library`)
  lists what the library holds by kind with how many of each, searched, with what the
  project has already marked (a click opens the project's one); what is ticked is added to
  the project with what it needs, as one step (`Workspace::library_to_project`). A
  definition's tab has *Send to library* (`project_to_library`).
  - **The pack format** (`engine/src/packs.rs`): a pack file (`.r2dpack`, a zip) is
    `pack.ron` (`PackManifest`: format, id, title, author, version, licence, attribution,
    description, tags, the oldest engine it fits, the packs it needs, its previews),
    `assets/manifest.ron` with `assets/files/…` (the definitions, their files and script
    functions, as 0.35's asset packages held them: `Bundle::manifest_text` /
    `from_manifest`), `plugins/<id>/…`, `previews/…` and an optional `history.ron`.
    `Pack::from_bytes` tells the file kinds apart by what is inside and reads the three of
    0.35 as packs with a made-up manifest (`legacy`): an asset package, a plugin file
    (`plugin.<id>`), the starter library file (`rustpg2d.starter`, with its plugins and
    history). A pack of a newer format or for a newer engine is refused with the reason;
    `Pack::code` lists what in it runs (script functions, plugins). A source's index
    (`index.ron`, `SourceIndex`): each pack's manifest, file, size, SHA-256 and what it
    holds; `IndexEntry::check` refuses a download that isn't the file named; `newest` offers
    the newest usable version of each pack.
  - **Packs in the editor's core** (`editor-core/src/packs.rs`, commands `packs.*`):
    `pack_preview` says what a file is and what installing it would do (what the library
    doesn't have, has as it is, has under the same id but different; what in it runs; the
    packs it needs that aren't there; the version installed); `install_pack` puts its
    assets, files and script functions into the library with the same id *kept*,
    *replaced* or brought in as a *copy* under a new id, its plugins into the library's
    `plugins/`, its previews under `packs/<id>/`, and records it in the library's
    `packs.ron` (manifest, the assets it brought under the ids they have there, where it
    came from); a pack that needs one that isn't installed is refused. `share_pack` writes
    a selection of the project with what it needs as `<id>-<version>.r2dpack` into a folder
    and lists it in the `index.ron` there, so the folder is a source as it is or uploaded.
    `read_index` / `download` read a source by web address or folder; a download is checked
    against the index's size and SHA-256. Commands: `packs.share FOLDER TITLE VERSION
    kind:id… [author:… licence:… plugin:…]`, `packs.look FILE`, `packs.install FILE
    [keep|replace|copy]`, `packs.source ADDRESS`, `packs.get ADDRESS ID [keep|replace|copy]`,
    `packs.list`.
  Still open: packs, sources and *Share…* in the window (they are commands so far), updates
  per asset (which ones the creator changed), licences on the Publishing page, the starter
  library installed as a pack, the old writers (`.r2dlib`, `.r2dplugin`) folded away; and
  of the tab: pictures of the library's assets, tags and filters, dragging onto a map, the
  starter library's installing and updating from the window.

  **Verify.** A source served from the scratchpad (`python3 -m http.server`): browse, install,
  a version 2 offered and applied, a changed asset kept, a wrong SHA-256 refused. A shared pack
  installed into a fresh library and used in a new project. The starter pack installed through
  the same path. A 0.35 `.r2dpack` and `.r2dplugin` installed.
  - The core, in tests (`editor-core` `packs`): the example's chest and guard shared (the
    guard's model and its file come along), the folder read as a source, the pack looked at
    (all new), installed into an empty library, listed, read again from the library's
    files; a version 1.1 with a renamed chest: differing, kept, replaced, the first version
    brought in as a copy under a new id; a pack that needs a missing one refused; a changed
    file refused; a pack forgotten.
  - **Over HTTP with the real editor** (`python3 -m http.server` on the shared folder, a new
    empty library, a new project): `packs.source` lists *Market things 1.0* as not
    installed, `packs.get` installs its 29 assets, a *Watchman* and a *Strongbox* are made
    from the library's guard and chest in the new project; version 1.1 is offered
    (*installed: 1.0*), looked at (1 differing), applied with the library's chest kept,
    then with it replaced; with a byte of the served file changed the download is refused
    for its SHA-256.
  - The format: `packs` tests in the engine (a pack with an item, its sheet and file, a
    script function, a plugin and a preview written and read again as it was, its manifest
    alone read without unpacking; bad ids and versions, a newer format, a newer engine and
    a missing file refused; a 0.35 asset package and plugin file read as packs and written
    again as packs of now; the starter library's `.r2dlib` read as `rustpg2d.starter` with
    its 100+ props, plugins and history; an index with two versions offers 1.10 before 1.0,
    names the file's address, and refuses another file and a changed byte).
  - Done so far: `librarypage::ui_tests` (the owlbear searched among 2,683 assets, ticked
    among the characters, added with its model as one step, marked as in the project and
    opened from there, undone). In the window: the same on a scratch library, the owlbear's
    tab with its picture afterwards. From nothing to play: a new project, a guard, the stone
    house and grass from the library (30 assets with what they need), the ground painted, a
    house built, the guard placed, *Play here*: the game shows them (and a placeholder for
    the hero: an empty project has none; the Overview's checklist now leads to where each
    missing step is done).

- [ ] Part 10: Help and guidance — `feature/studio`

  **Goal.** A new creator gets from an empty project to a playable game without leaving the
  editor to find out how.

  **Design.**
  - **The manual inside:** a Help panel shows the Creator's Manual (shipped in the editor, so it
    works offline), searched, and opened by F1 at the chapter for whatever has the focus.
  - **Fields explain themselves:** the schema's help text on hover and, longer, in the Help panel.
  - **Empty places say what to do next:** an empty list, map or page names what belongs there and
    offers the action.
  - **Problems with fixes:** the list is always current and grouped; an entry goes to its place,
    and where it can, offers the fix (*Create the missing item*, *Pick another*, *Remove the
    reference*). Fields with a problem are marked.
  - **Guided tours:** a tour is a list of steps kept as data. Each step points at the control to
    use and is ticked when the command behind it ran. The first three: *Your first map*, *A
    character who talks*, *A quest*. The Learning release writes its tutorials as tours.
  - **Agents:** the agent server as today, with a button that copies the set-up for an agent.
  - **What's new** after an update.

  **Engine.** None. Tours and the manual's anchors are data in the editor crate.

  **Verify.** Each tour followed on a fresh project by `egui_kittest`, ending with a play-test
  that starts. F1 from five places opens the right chapter. A broken reference fixed from the
  Problems panel.

- [ ] Part 11: Old things leave — `feature/studio`

  **Goal.** Nothing remains that only older versions needed, in the code or in the UI.

  **Design.**
  - **Removed without asking** (the user's decision on older projects): the 16 px migration
    (`migrate.rs`), the tile converter (`convert_tiles.rs`, `old_tiles.rs`, `io::decode_tiles`,
    the v1 / v2 map readers and `EntityV1`), the 0.27 skill tree conversion (`srd_convert.rs`),
    the rust2d folder adoption (`legacy.rs`), the upgrade steps of definitions (`SpellDef::upgrade`,
    `ItemDef::upgrade`, rules versions below the current one, 0.27's status effects), the legacy
    `Appearance.body`, `PropDef.pieces`, the conversion of project folders on open, the fixtures of
    versions before 0.35 and their tests.
  - **Listed to the user before removal** (their rule of 2026-11-24 for tools): the Tiled import,
    the Aseprite import, pictures held in the hand without a model, what is left of sprite-sheet
    figures and props, the top-down view, Meshy's 2D kinds.
  - **One conversion, 0.35 → 0.36,** run by the editor on open with a backup copy beside the
    project: everything this release reshapes (editor data, pack origins, whatever the parts
    above recorded under *Decided while building*). The game and the editor refuse older projects
    with a message that names 0.35.
  - **Save games** of 0.35 load where the fields allow it; it is not promised (**Breaking**).
  - **Fixture:** the example project as of the 0.35.0 tag, for the conversion's test.

  **Engine.** The removals above; `convert_035.rs`.

  **Verify.** The 0.35 example converts and plays through the smoke run; a 0.34 project is
  refused with the message; `cargo test --workspace` without the old fixtures.

- [ ] Part 12: Tests, documentation, switch-over — `feature/studio`

  **Goal.** The new editor replaces the old one completely, and the rules for working on it are
  true again.

  **Design.**
  - **The parity table:** every feature of the 0.35 editor (its menus, the 126 editor script
    commands, the agent tools, the manual's editor chapters) with its place in the new editor or
    *removed* and why. Made when part 3 starts, kept in this item, ticked as parts land. The
    branch merges when no row is open.
  - **The editor script** runs commands from the registry and can click: controls are found by
    their label through the accessibility tree, in the live program. `status` keeps the fields
    the smoke run and `17-testing.md` use.
  - **UI tests:** `egui_kittest` over the whole program, with snapshot pictures of the main
    screens. The patched-egui workaround of CLAUDE.md is gone.
  - **Agent server:** the 0.35 catalogue plus tools for packs and sources; `13-ai-agents.md`.
  - **CI and packages:** the apt lines (Vulkan for the Xvfb runs), `smoke.sh`, the web build, the
    release builds, `xtask dist`, Dependabot's ignore list (the egui pins are gone; egui's crates
    move together as a group).
  - **Documentation:** the manual's editor chapters rewritten and every editor screenshot taken
    again; `architecture.md`; CLAUDE.md's workspace map, coding guide and verification sections;
    AGENTS.md; the README; the CHANGELOG with its **Breaking** lines.
  - **The merge:** the draft pull request is made ready and merged; 0.36.0 is cut when the user
    asks.

  **Engine.** None.

  **Verify.** The parity table has no open row. The example built from an empty project by an
  agent through the server reaches a play-test. `scripts/ci/smoke.sh` whole. The packages of every
  platform start (the release pull request's *Packages* check, run by hand on the branch before
  the merge).

## 0.37.0 — Learning

A way in for new creators: tutorials and small games to learn from (moved from 0.35.0 on
2026-11-23, and behind *The Studio* on 2026-11-26, so that they teach the new editor).

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
  - **Guided tours:** each tutorial is also a tour inside the editor (0.36, *Help and guidance*:
    steps kept as data, each pointing at its control and ticked when done), opened from Home ›
    *Learn* and from its chapter.
  - **README:** links to them.

  **Engine.** None (docs and tour data). The screenshots are taken with editor and autoplay
  scripts.

  **Verify.** Follow each tutorial on a fresh project and fix every step that doesn't match; each
  tour run by the editor's UI tests.

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
  index of community packs: a pack source (0.36, *Library and sharing*) kept in a repository,
  preset in the editor beside the official one, with the rules for getting a pack listed.

  **Engine.** None. These are the user's settings and decisions.

  **Verify.** None.

## 0.38.0 — Everywhere

Touch screens, phones, Steam and signed downloads. The engine lines below name the platform layer
of 0.36 (`crates/gfx`: winit, wgpu), which replaced macroquad.

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

  **Engine.** `runtime/input.rs` (the touch events of `crates/gfx`), `mouse.rs`, UI hit tests;
  autoplay `touch X Y`, `touchhold`, `swipe`.

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

  **Engine.** `sys.rs` (web), the web files of the export (`web/`), `release::export_web`, model
  loading in `render/model.rs`.

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
  - **Runner:** the game on Android through winit's `android-activity` and wgpu (Vulkan, GLES
    for older devices), packaged by `xtask` in CI; check the tools of the day when it starts.
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
    0.36 and later through the migration chain (0.36 removed every older converter and opens 0.35
    projects through one conversion). Confirm with the user.
  - **Fixtures:** 1.0.0 files of every format.

  **Engine.** `migrate.rs`, the version fields, the compat fixtures and tests.

  **Verify.** Every fixture of every earlier version opens (or is refused with a clear message,
  as decided). A 0.36.0 project migrates and plays.

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
- **Gamepad remapping in the editor's play panel** (the shortcut sheet went into 0.36, *The
  shell*).
- **An assistant inside the editor** (left out of 0.36): a chat panel that works on the project
  through the agent tools. Needs a decision on whose account and key it uses; the agent server
  for outside agents exists.
- **Linked structures** (left out of 0.36): placed copies of a structure that follow when the
  structure changes. Structures are stamps today; this needs the map to remember what was placed.
- **Skinning on the GPU** (left out of 0.36): figures are skinned on the CPU and cached in atlas
  pages; with wgpu the skinning could move into a shader.
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
