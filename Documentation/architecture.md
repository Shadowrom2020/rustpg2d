# R(ust)PG2D architecture

How the engine, its platform layer, the game runner and the editor fit together: the technology,
the data model, how a frame is drawn, the runtime, the editor and its agent server, the file
formats and their compatibility rules, the verification tooling, and the caveats worth knowing.
Written for people (and agents) changing the code; the [Creator's Manual](manual/README.md) is the
user manual, [CLAUDE.md](../CLAUDE.md) the working rules and the module-by-module map.

This describes the branch of *0.36.0 — The Studio* ([ROADMAP.md](../ROADMAP.md); what is left is
in [STUDIO_PLAN.md](../STUDIO_PLAN.md)): the engine draws, plays sound and reads input through a
crate of its own (`crates/gfx`), and the editor is rewritten as `crates/studio` over
`crates/editor-core`. The 0.35 editor (`crates/editor`, macroquad and egui 0.31) is out of the
workspace and kept only to read until it is deleted; this document describes it only where
something of it is still in use.

## 1. Technology

| Crate | Lines (0.36 branch) | Role |
|---|---|---|
| `crates/gfx` | ~5 300 | the platform layer: device, draw list, textures and screens, WGSL materials, cameras, shapes, text, input, the window loop, sound |
| `crates/engine` | ~94 000 | library shared by game and editor: definitions, maps, save games, the runtime (`runtime/`), rendering, scripting, library bundles and packs, exports |
| `crates/game` | ~700 | the game binary: command line, window, the `RUSTPG2D_AUTOPLAY` runner |
| `crates/editor-core` | ~31 000 | the editor without a UI: the workspace, commands, the one undo history, the field schema, the agent (MCP) server |
| `crates/studio` | ~23 000 | the editor (binary `rustpg2d-studio`): the eframe window, its tabs, forms, map view and the game in a tab |
| `crates/packaging` | ~500 | AppImage and Flatpak bundles from an `AppSpec` |
| `crates/xtask` | ~800 | `cargo xtask check-version / check-branch / release-notes`, `cargo xtask-dist`, `cargo xtask-web` |
| `crates/editor` | ~51 000 | the 0.35 editor, excluded from the workspace (`exclude` in `Cargo.toml`), not compiled |

Rust 2021, the toolchain pinned in `rust-toolchain.toml` (`rust-version` 1.99), clippy with
`-D warnings`. The stack, as `Cargo.toml` and `Cargo.lock` have it:

- **Platform layer** (`crates/gfx`): **wgpu 30** (feature `webgl`: WebGL 2 in browsers; the
  version egui-wgpu 0.37 uses, so the editor can hand its device to the engine), **winit 0.30**,
  **fontdue 0.9** (the rasteriser macroquad used, so glyphs keep their pixels), **glam 0.30**
  (`scalar-math`: the vertex layout needs unaligned `Vec4`), **bytemuck**, **image 0.25** (PNG and
  JPEG decoding), **quad-rand 0.2** (re-exported as `gfx::rand`: the same numbers for the same seed
  as macroquad's `rand`), **kira 0.12** without its default features (cpal, ogg, vorbis, wav, pcm:
  the realtime thread priority would want D-Bus), **pollster** natively; on the web
  **wasm-bindgen 0.2** (0.2.130 in `Cargo.lock`), wasm-bindgen-futures, js-sys, web-sys and
  **glow 0.17** (reading textures back through WebGL). Shapes, text with its glyph atlas, cameras,
  colours and rectangles are macroquad's own code (MIT OR Apache-2.0,
  `crates/gfx/LICENSE-macroquad-MIT`), and the drawing API keeps macroquad 0.4's names: the engine
  imports `gfx::prelude::*` where it imported macroquad's prelude.
- **Engine:** **rhai 1.27** (the scripting language, `script.rs`); **serde + ron 0.12** for every
  text format (project, definitions, map objects, saves, settings, translations); **postcard +
  lz4_flex** for the binary part of map files; **png 0.18** (`imageio.rs`) for pictures the engine
  and editor decode and write themselves (textures are decoded by gfx's `image`); **lewton 0.9**
  decodes Ogg Vorbis music so loops can be gapless; **gltf 1** (3D models), **ttf-parser 0.25**
  (which characters a project font has), **libm** (the math functions newer glibc re-versioned,
  `glibc_compat.rs`, so release binaries built on a rolling distribution still need only glibc
  2.34); natively **gilrs 0.11** (gamepads; needs libudev on Linux) and **zip 9**; with the feature
  `schema` **schemars 1** and **serde_json** (the editor's forms).
- **Editor core:** **rmcp 3** + **tokio** + **axum 0.8** + **schemars** + **serde_json** (the agent
  server), **ureq 3** with rustls (the starter library's download, Generate art with Meshy),
  **rustpg2d-packaging**.
- **Editor window** (`crates/studio`): **eframe 0.37** (egui 0.37, its wgpu renderer, X11 and
  Wayland), **egui_dock 0.22** (panels and tabs), **rfd 0.18** (file dialogs through the desktop
  portal on Linux), **winit 0.30** (only to ask eframe's event loop for X11). Tests:
  **egui_kittest 0.37**, and an rmcp client for `examples/mcp_smoke.rs`.
- **xtask:** **wasm-bindgen-cli-support =0.2.130** (feature `web`; must be the version of the
  `wasm-bindgen` crate the game is built with), **zip** and the packaging crate (feature `dist`).
  **packaging:** **backhand 0.25** (the AppImage's squashfs).
- Targets: Linux (AppImage, Flatpak), Windows (MinGW cross-build), macOS (universal app, signed
  ad hoc), the game also for `wasm32-unknown-unknown` (`sys.rs` abstracts files and the clock,
  `gfx::web` the browser).

Build profiles (`Cargo.toml`): `dev` compiles our crates at opt-level 1 and dependencies at 2, so
debug builds play at full speed; CI's test jobs override both to 0. `release` uses thin LTO, one
codegen unit and strips symbols; it does not abort on panic, because gfx's `audio.rs` catches a
panic of the sound device.

## 2. Data model

**Project** = a folder: `project.ron` (`ProjectConfig`: name, player, inventory size, magic, the
SRD `rules` and `rules_version`, languages, …), `defs/*.ron` (seventeen lists, `project::DEFS`), `maps/*.map`, `sprites/`, `materials/`, `tilesets/` (pictures of converted tiles),
`sounds/`, `scripts/*.rhai`, `lang/<code>.ron`, the editor's own `editor/meta.ron` (§5) and the
ignored `saves/`. `Project::load` reads it
all into `Database` (`defs.rs`), one `Vec` per kind, referenced by string ids everywhere
(`refactor.rs` knows every reference for renames and library exports: `refs_of_*`).

**Maps** (`map/`): `Map` = size (up to 1024×1024 tiles of 32 px), the painted ground, decorations,
a collision grid (per-tile overrides), entities, zones, edge links, triggers and settings (music,
lighting, fog, weather, parallax). The **painted ground** (`ground.rs`): `Map.ground`, chunked in
32×32-tile chunks, one byte per
8 px cell = a slot in `Map.materials` plus one (0 = none), a uniform chunk stored as one value;
`MaterialDef`s (`defs/materials.ron`) say how a material looks and walks. `render/ground.rs` draws
it every frame with a WGSL shader kept to what WebGL 2 can do (per chunk a 130 × 130 cell texture
with a border of the neighbours' cells, the materials' textures in one atlas, no texture arrays,
the per-material table in a 128 × 4 texture; edges from bilinear
coverage of the cell centres, warped by noise and snapped to the art's pixels when crisp). The
runtime collides hitboxes with the cells and plans routes on tiles blocked by half their cells
(`MaterialTable`). **Decorations** (`decor.rs`) are `Map.decor`: a prop slot (`Map.decor_defs`), feet
position in pixels, flip, state and layer (the state's two high bits are a quarter turn, `Decor::turn`,
for props that turn: a 3D model, `PropDef.model`;
drawn by `Assets::draw_prop`, which game and editor share, a model standing on its hitbox's middle; placed
props turn by their `facing`, and `PropDef::hitbox_facing` / `DecorLook::rect_at` turn their hitboxes;
props with `PropDef.block` are drawn as boxes in window pixels by `iso::draw_block`, ordered by their
whole footprint and cut to `iso::STUB` when `iso::hides` says they stand in front of the hero); the
runtime indexes them by chunk (`DecorIndex`), draws the
Sorted ones in the depth-sorted list with characters, and marks the solid ones once per map in the
occupancy grid's base (`Occupancy::keep_as_base`, `OCC_DECOR` for the collision's broad phase;
tiles painted *walkable* are taken out again). The file (`map/io.rs`) is `MAGIC` + version + lz4 of
a postcard `Core3` (name, size, collision, a RON string `Meta` with everything else, the materials,
the ground and the decorations).

**Tile projects** (0.20 and older) had tilesets (`defs/tilesets.ron`) and maps of tile layers
(format v2, v1 all postcard). They are still read (`old_tiles.rs`, `io::decode_tiles`), but only
for `convert_tiles.rs`, which turns them into painted projects once: the 0.35 editor ran it when it
opened such a project (after a backup copy), `library::open_library` runs it on an old library, and
bundles read from old `.r2dpack`s go through it in a temporary folder. The new editor refuses every
project older than 0.35 instead, naming 0.35 (`Workspace::new` in editor-core). The ground layer becomes ground cells (a
terrain tile's slot mask says which of its 4 × 4 cells are the terrain), the other layers
decorations of props made per tile over a sheet of the tileset's image; materials are shared by
their texture's pixels. Everything else (`decode_with`, the game) sees a tile map without its
tiles and warns. `migrate.rs` (16 px projects) runs before it and writes v2 maps (`save_tiles`).

**Save games** (`state.rs`): `WorldState` as RON in `saves/<slot>.ron` (plus `autosave`): flags,
variables, inventory, equipment, quests, shops, containers, per-map entity states, dropped items,
the player's homes and placed furniture, fired once-triggers, companions, explored fog, the clock,
the player's look. Everything the player
can change lives here; nothing else is persisted.

**Settings** (`settings.rs`): per user and game, `~/.config/rustpg2d/<game>/settings.ron`
(`RUSTPG2D_SETTINGS_DIR` overrides). Achievements unlocked go next to it.

**Asset library** (`library.rs`): a project folder without maps at
`~/.local/share/rustpg2d/library` (`RUSTPG2D_LIBRARY` overrides). The editor's starter set is the
committed folder `crates/editor/assets/starter_library/`. Releases publish it as one file,
`rustpg2d-starter-library-X.Y.Z.r2dlib` (`starter_pack.rs`: a zip of `starter.ron` = version and
`FORMAT`, `history.ron` and `library/…`; written by `xtask dist`), which the editor installs beside
the library (`starter/`, `crates/editor-core/src/starter.rs`: downloaded from the release and checked
against its `SHA256SUMS`, or loaded from a file; only a pack of the editor's own `FORMAT` is read).
Repository mode and development builds read the folder itself. The starter set is applied to a library when its `starter.stamp` (rules version + hash of the files) differs: new ids
are added, images the new set only extends are replaced, and `starter.pending` is left for an editor to offer *Update asset library*
(`starter_update.rs`): it compares the library with the starter set using
`crates/editor/assets/starter_history.ron` (fingerprints of every definition, file and script
function any starter set shipped, written by `cargo run -p rustpg2d-editor-core --example
starter_history` from git; a test keeps it current), replaces untouched assets, asks about edited
and retired ones, and backs the library folder up first.

**Packs** (`packs.rs`, 0.36) are the one file format shared work travels in: a `.r2dpack` is a zip
of `pack.ron` (`PackManifest`: id, title, author, version, licence, what it needs), `assets/` (a
`Bundle`: definitions, their files and script functions), `plugins/<id>/`, `previews/` and an
optional `history.ron`; `Pack::from_bytes` also reads 0.35's asset packages, plugin files
(`.r2dplugin`) and the starter library file as packs (`PackManifest::legacy`). A source (a web
address or a folder) offers packs through a `SourceIndex` with each pack's size and SHA-256. The
editor's side is editor-core's `packs.rs`: installing into the library (record `packs.ron`, same
ids kept, replaced or copied), updating a pack asset by asset over `starter_update::plan` /
`apply` (the starter library is the pack `rustpg2d.starter`), sharing a selection, reading sources.

**Visual scripts** (`visual.rs`, `defs/visual_scripts.ron`): a `VisualScript` is one Rhai function
as nodes and links between their pins (flow and typed values). The node catalogue comes from the
script API table: its return types (`-> Kind` / `=> Kind` for functions that also act, checked
against Rhai's own signatures with the `metadata` feature in tests) decide whether a call is a step
on the flow or a value node. `function_code` walks the flow from *Start*, writes value nodes into
the expressions that use them and a step's used result into a `let`, and checks types, missing
functions, circles and results used before their step (a broken function is written as a stub).
`Project::save_defs` writes `scripts/visual_scripts.rhai`; the game only ever runs that Rhai. Ids
typed into inputs are references by the API argument kinds (renames, library export). In the
editor a visual script is a tab like any definition: editor-core's `visualedit.rs` holds what the
canvas does (the palette from the API table grouped by the manual's sections, read from
`08-scripting.md` at build time, with a test that every function has a section; linking with type
checks, adding, copying, grouping into a function), and `studio/src/nodesview.rs` draws it on an
`egui::Scene`.

**Unused assets** (`unused.rs`): models, sheets, sounds, materials and fonts count as used when their
id appears as a string in the other definition lists (each `defs/` list serialized as RON), the
project config, a map's `Meta` (plus its materials and decoration props) or, as a substring, in a
script or plugin file; used assets add their own strings until nothing changes (rig templates,
footstep sounds), and a few are used by rule (playable models, pieces fitting a used body, the
mannequin, emotions sheets). Reading every list as text keeps new fields covered without a
reference walker (a test fails when a `DEFS` list has no text). `release::game_files` leaves the
unused out of exports; editor-core's `cleanup.rs` deletes them after a backup (the *Clean up
unused files* window, `studio/src/cleanupwin.rs`).

**Exported games** (`release.rs`) carry the project's files as `data.pack` (`pack.rs`), or loose in
a `data` folder when the export says so. The pack is a header, a postcard index (path,
offset, length) and the files, XOR-scrambled with a position-keyed SplitMix64 stream (against
casual eyes, not protection). Natively `release::bundled_root` opens it, reads only the index and
mounts it in `sys` as the `data` folder: `sys::read` / `exists` / `files_in` serve paths under it
from the pack (reading a file by seeking) and fall back to the disk, so the rest of the engine
doesn't know whether a game is packed. The web game downloads the same format whole
(`release::read_web_pack`) into `sys`'s in-memory files (§3, *The web*).

**The SRD 5.1 rules** (`srd/`): `dice.rs` parses and rolls dice expressions (`2d6+STR`) with a
seedable thread-local xorshift generator (`seed`, `RUSTPG2D_SEED`; automated runs start at 1),
`abilities.rs` has the six abilities, modifiers, the proficiency bonus by level, the 18 skills and
the SRD's tools by their SRD index ids, `check.rs` resolves what a check rolls (`Spec`: a skill, an
ability, `ability:tool`, a saving throw) and the `RollRecord` of the roll log. The hero's
`abilities`, `proficiencies` and `rolls` live in `WorldState`, which rolls with them
(`WorldState::check` / `roll_dice`), so the runner, scripts and the game share one path; the
game shows new rolls by `WorldState::roll_count` (`runtime/rolls.rs`). Locks (`LockSpec::pick_dc`)
and recipe quality steps (`QualityStep::dc`) read the old skill levels through `serde(from)`
reprs. Races carry their SRD data (`races.rs`: a subrace's `parent`, `RaceDef::merged` /
`Database::race_full`, `build_hero` turns the creator's picks into scores, proficiencies, languages
and resistances) and backgrounds are a definition kind of their own (`backgrounds.rs`); editor-core's
`srd_import.rs` (run by its `srd_import` example) writes both from a pinned checkout of
5e-database; the creator (`runtime/creator.rs`) has a second page for scores, choices and the
background; darkvision lifts the ambient light (`render::lighting::darkvision`). `srd_convert.rs` turns a 0.27 project's skill tree into proficiencies once (by
`ProjectConfig::rules_version`), renaming skill ids with the refactor machinery (`RefKind::Skill`
is no definition any more: it names a proficiency).

Levels and the hero's hit points are SRD too (`progression.rs`): the XP table, `max_hp(level, CON,
rolls, per_level)`. `WorldState::hp_rolls` keeps what each level's hit die gave (missing = the
average, so saves of 0.27 load) and the runtime derives the max HP from them and the current CON
modifier (`Game::player_max_hp`) instead of storing it; `hit_dice` counts the dice left (`None` =
the level). Stat blocks (`srd/statblock.rs`) are data on `CharacterDef::stats` / `EntityInstance::stats`
(the placed one wins, `Game::stat_block`); the SRD's NPC templates are `srd/npcs.ron`, built into
the engine (`include_str!`, parsed once) and written by `srd_import`. Fights fight with them
(below); the script host gets them as a list per script call (`set_stat_blocks`, `npc_stat`).

Diseases (`diseases.rs`, `defs/diseases.ron`, `RefKind::Disease`) are data too: `DiseaseDef`
says what the symptoms do and how the course runs over long rests; the hero's are
`WorldState::diseases` (`ActiveDisease`: onset, current DC, stage, hit points lost), run by
`runtime/diseases.rs` (onsets each frame, courses in `long_rest_effects`, fits in both damage
paths). A monster gives one through its stat block rider's `disease`.

Classes (`classes.rs`, `defs/classes.ron`, `RefKind::Class`) are data: `ClassDef` with its levels
(features, flattened numbers), subclasses and `ClassFeature`s whose `FeatureEffect`s the engine
acts on (the importer's mapping table decides which) and whose `FeatureChoice`s the level-up window
asks for. `ClassTraits::of(class, level, HeroClass)` adds the effects up for a level and the picks
made; the runtime asks it whenever a number depends on the class (`runtime/classes.rs`:
`class_armor_class`, `class_speed_ft`, `who_traits` for fighters, `refresh_training` for the
`srd::check::Training` that checks read: expertise, half proficiency, reliable talent, save
advantage). The hero's `HeroClass` is `WorldState::class` and their hit die replaces the d10
(`progression::max_hp_die`). Party members carry theirs on `CharacterDef::class` / `level` and,
once they joined, on the saved `Companion` (xp, level, hp rolls); `member_block` builds their
stat block from class, level and gear, which fights use instead of the definition's. Level-ups with
choices wait in `WorldState::levelups` (`player` or a member's home `map:uid`, since a companion's
runtime uid changes from map to map) for `runtime/levelup.rs`'s window, which opens only when no
fight, event or dialog runs.

The features that act are effects too: `Resource` (a pool of uses: a count, a class number, an
ability modifier, per level; back after a short or long rest), `Act` (a class action with its cost,
resource and `ActEffect`) and riders (Sneak Attack, Stunning Strike, Uncanny Dodge…), collected by
`ClassTraits` as `resources`, `acts` and `riders`. `runtime/turns/features.rs` runs them: the uses
spent live in `WorldState::spent` / `Companion::spent` (the most comes from the traits each time,
so a level-up raises it), what lasts within a fight in the `Fighter`'s `ClassFx` (rage, frenzy,
reckless, an inspiration die, Sacred Weapon, temporary hit points; not saved), once-per-turn marks
in `Combat::once`. The fight's code calls in at fixed points: `make_attack` (buffs, inspiration,
rider damage, the target's reaction, on-hit saves), `combat_damage` (rage's resistance, temporary
HP, Relentless Rage, then `after_damage`: Dark One's Blessing, Retaliation), `who_check`
(rage's advantage, inspiration and Indomitable on a failed save), `begin_turn` (rage upkeep,
Survivor) and the AI's `ai_class_step`.

Spells (`skills.rs`) are the SRD's since 0.30: a `SpellDef` carries the SRD's fields and its
mechanics as `SpellPart`s; a spell of 0.29 (mana, one `SpellEffect`) is upgraded when the defs are
read (`SpellDef::upgrade` in `project::parse_defs`), and `srd::RULES_VERSION` 3 has
`srd_convert::convert_project` write that conversion once with a backup (the 0.35 editor did it on
open, libraries do it on open; the new editor refuses a project of an older rules version). A class's `Spellcasting` says how it has its spells; slots
come from its level table (`spellcasting.spell_slots_level_N`), spent ones live in
`WorldState::slots_spent` / `Companion::slots_spent` (rests reset them), known spells in
`WorldState::spells` / `Companion::spells`, prepared ones in `prepared` (empty = the first ones it
may prepare). `runtime/spells.rs` answers who casts what (`cast_options`: spell and slot pairs),
concentration (the hero's in `WorldState::concentration`, party members' in `Skills`), scrolls and
casting outside fights; `runtime/turns/casting.rs` resolves a cast in or out of a fight
(`resolve_spell`: attack or save, then the parts per target). What a spell leaves on a creature is an
`ActiveCondition` with the spell as its `source` (`spell@caster`), either a condition or a
`Buff`, so concentration ending removes it everywhere and the conditions' ticking ends it; the fight
code reads buffs in `who_ac` / `armor_class`, `who_speed`, `make_attack`, `who_check` and
`combat_damage`.

Equipment is the SRD's since 0.31: `ItemDef` has `srd` (the SRD index it is), `cost_cp`, `weight`
and `coin_cp`; `Weapon` a `WeaponCategory`, `WeaponProperty`s, versatile dice and a thrown range;
`Armor` an `ArmorCategory` (Clothing gives no AC), `base_ac`, `dex_max`, `str_min` and Stealth
disadvantage. Items of 0.30 are upgraded when read (`ItemDef::upgrade`), `RULES_VERSION` 4 writes
it once in the same way. `srd/equipment.rs` holds the pure rules (`armor_class`, `armor_proficient`,
`weapon_proficient`, `change`, `format_cp`, the `FOCUS` list); `runtime/equipment.rs` applies them to
the hero (AC, training, the STR minimum through `hero_speed_ft`, `Training::armor_untrained` and
`stealth_disadvantage` that `WorldState::check` reads) and `turns/mod.rs` `weapon_spec` builds the
attack (finesse, versatile, reach, thrown, heavy, loading). Money is coins: `WorldState::coins` (the
project's coin ids by worth, filled when the world is prepared, not saved) lets `money_cp`, `pay_cp`
(smallest coins first, change back, refused when the change wouldn't fit) and `give_cp` work in
copper pieces; a shop whose currency has a `coin_cp` prices in cp (`ShopSpec::price`), others keep
counting their one currency item.

Magic items (0.31) are `ItemDef::magic` (`srd/magic_items.rs` `MagicItem`). Their passive effects are
`FeatureEffect`s, the class features' vocabulary: `runtime/magic_items.rs` collects the items in effect
(equipped in any slot but the tool, attuned when they need it) and `hero_traits` / `who_traits` fold
them into the `ClassTraits` with `add_effects`, so everything that reads traits (AC formulas,
resistances, speed, evasion, riders) sees them; a hero without a class gets traits only when an item
has effects. Flat bonuses (`BonusTo`) are read at the sites that add up a number (`armor_class`,
`weapon_spec`, `member_block`, `spell_dc`, `Training::save_bonus` in `Spec::modifier`). Ability scores
of items and potions raise `WorldState::abilities` in place once a frame (`refresh_item_abilities`);
`ability_base` holds the hero's own while they are raised, so code that changes scores goes through
`WorldState::base_abilities_mut`. Charges are per stack: `ItemStack::spent` (never merged; kept by
`Inventory::add_stack`, `take_from_slot`, transfers and `DroppedItem::spent`); `check_dawn` compares
the dawns passed with `WorldState::last_dawn`. Using an item builds a throw-away `SpellDef` from its
`use_parts` and resolves it like a spell; a spell cast from an item sets `Game::casting_item`
(`spell_dc` reads its DC) and is cast like a scroll (the action, no slot), the charges spent after it.

Conditions (`srd/conditions.rs`) replaced the status effects of 0.27: `ConditionSpec` (what weapons,
touches, actions and scripts apply: a condition, seconds, exhaustion levels, a saving throw,
damage or healing per round of 6 s) and `ActiveCondition` (the hero's in `WorldState::effects`, a
character's in `RtEntity::effects`); both read 0.27's `{kind, strength, seconds}` through `serde(from)`
reprs, and `Condition`'s `#[serde(other)] Any` catches old effect names that have no condition.
`rules(list)` sums up what a list does (disadvantage, failing saves, speed, acting, lying down…);
`WorldState::check` applies the hero's, `runtime/effects.rs` ticks them and the runtime reads
`rules` where it moves, attacks, notices and draws. `srd/damage.rs` has the 13 damage types and
`Defenses::apply`; hits carry their type (`combat::Blow::damage_type`, projectiles,
`hurt_player_by`) and meet the hero's defenses (`player_defenses`: race, armor, petrified) or a
stat block's. Rests are `runtime/rest.rs` (the short rest's window, `long_rest_effects`, which a
night's sleep in `furniture.rs` runs). `srd_convert` (RULES_VERSION 2) takes out what 0.27's haste,
strength, weakness and shield became (`drop_lost_effects`). `srd/rules.rs` builds in `rules.ron` (the SRD's texts and
its race / background ids, written by `srd_import` like `npcs.ron`); `srd::ATTRIBUTION` goes into
every export (`release.rs`, the packages) and, with `ProjectConfig::srd`, the Credits page and
`srd_attribution()`.

**Translations** (`lang.rs`): `lang/<code>.ron` with `texts` (content: dialogs, names, quest texts,
collected by `content_texts`) and `ui` (runtime strings; English is the key). The game swaps the
`Database` for a translated copy (`translate_db`).

## 3. The platform layer (`crates/gfx`)

The engine was written against macroquad 0.4. On this branch it draws, plays sound and reads input
through `crates/gfx`, which offers the same names and behaviour (`draw_rectangle`,
`draw_texture_ex`, `draw_mesh`, `set_camera`, `render_target`, `Texture2D`, `Image`, `Material`,
`is_key_pressed`, `get_time`, `screen_width`…): the runtime's files changed their imports and the
shaders, not their logic. What differs is underneath.

**The context** (`lib.rs`). Everything the drawing, input and time functions work on is one
`Context` in a thread-local: the device (`gpu.rs`: device, queue, a nearest and a linear sampler,
the bind group layouts), the draw list (`batch.rs`), what "the screen" is right now, the current
`Input`, the camera stack, the fonts, the clock and the window's wishes (fullscreen, cursor, size,
quit). It is set up by `gfx::init(device, queue, w, h, conf)` with a device made elsewhere (the
editor passes eframe's), by `gfx::init_headless(w, h)` with a device of its own and no window
(tests, `model_sheet`), or by the window. The device asks only for what WebGL 2 offers
(`Limits::downlevel_webgl2_defaults`), so every backend qualifies. A software adapter (llvmpipe,
lavapipe, WARP) is refused with a message unless `RUSTPG2D_ALLOW_SOFTWARE=1` (tests, CI);
`RUSTPG2D_GFX_INFO=1` prints the adapter, its backend and the surface's formats.

**A frame.** `begin_frame` moves the clock on, empties the camera stack, resets the draw list and
puts a clear to black at its start; every drawing function ends in `Batch::geometry`, which adds
triangles to the list; `end_frame` sends the list to the GPU (`flush`) and ends the input's frame
(what was pressed or released this frame is over). The list is a port of macroquad's `quad_gl.rs`
in what starts a new draw call (another texture, material, mode, clip, viewport or target), with
two differences: a draw call keeps the projection and model matrix it was made under, so changing
the camera sends nothing to the GPU (macroquad flushed at every camera change, and the isometric
view changes it for every upright thing), and clearing is an entry of the list. A material's draw
call keeps a copy of its uniform block and textures as they were. `flush` writes all vertices and
32-bit indices into one vertex and one index buffer (WebGL 2 can't add a draw call's first vertex
itself; meshes still come with 16-bit indices, 65,535 vertices each), one 256-byte uniform block per
draw call bound at a moving offset, then makes **one render pass per run of commands into the same
picture** (a clear at the start of a run becomes the pass's load operation) and submits one command
encoder. Anything that reads a texture back flushes first. `frame_stats()` tells the last flush's
draw calls, passes, vertices and CPU milliseconds; the game's `status` prints them as
`gpu=CALLS/PASSES:MS`.

**OpenGL's conventions are kept** where the engine counts on them. Projections are made in OpenGL's
clip space (y up, depth −1..1) and the flush converts them (depth to 0..1). Render targets are drawn
upside down (`TargetInner::flip`), because the engine's cameras and texture coordinates expect it;
viewports count from the bottom left and clips from the top left, as macroquad's did. The screen
is drawn 1/128 pixel lower, so that an edge exactly through pixel centres lands on the row OpenGL
chose (the UI's half-pixel box outlines). Every picture is `Rgba8Unorm` without sRGB conversion,
and the window's surface is drawn through a format without sRGB encoding, so colours arrive as
OpenGL drew them.

**Screens.** Everything is drawn into a picture, a `Screen` (`texture.rs`: a render target with a
depth buffer that is not flipped, and the scale between its pixels and the points `screen_width()`
answers in). The window copies the context's screen onto its surface after each frame (one
triangle, nearest filtering). `gfx::with_screen(screen, input, f)` flushes, swaps the given screen
and input in as the current ones, runs `f`, flushes again and swaps them back: what `f` draws lands
in that picture, `screen_width()` is its width, and the key and mouse functions answer from that
input. That is how the editor draws a running game, its map views and its previews (§5), and why
several screens, and several games, can live in one process. `Screen::texture().view()` is the
wgpu view another renderer shows. Reading back: `Texture2D::get_texture_data` copies into a mapped
buffer natively (it waits for the GPU) and calls WebGL's `readPixels` through wgpu's GL context on
the web (§ *The web*). `get_screen_data` still returns the screen bottom row first, as macroquad's
did, and `Image::export_png` still turns it over: autoplay `shot`, photo mode and save thumbnails
work unchanged.

**The window** (`window.rs`). A program is a future whose loop ends each turn with
`next_frame().await`, as macroquad programs are. `gfx::window::run(conf, main)` opens a winit
window, makes the device for its surface, and polls the future once per frame between
`begin_frame` and `end_frame`; then it applies the program's wishes and presents the screen.
Natively a frame is drawn whenever the events are through (`about_to_wait` with
`ControlFlow::Poll`) and presenting waits for the display (`Conf::vsync`; the runner turns it off
for `RUSTPG2D_VSYNC=0`). On Linux the window is X11 where there is an X server, also under Wayland
through XWayland, as macroquad's were: a Wayland compositor hands a covered window no frames, and a
scripted run behind other windows would stand still. `RUSTPG2D_WAYLAND=1` asks for Wayland. With
`prevent_quit()` the close button only sets `is_quit_requested()` (the game then asks whether to
save). `run_headless(w, h, main)` polls such a future without a window, as fast as frames come.

**Materials** (`material.rs`). Shaders are WGSL. `load_material` writes the declarations in front
of a shader's source from what the material declares (the uniform block with `Projection`,
`Model`, `_Time` and the material's own uniforms in order; group 1 the draw call's texture and
sampler; group 2 each of the material's textures; the vertex input with position, texture
coordinates, colour and a free `normal`), so a shader and its uniform layout can't disagree; a
shader holds only its output struct and `vs_main` / `fs_main`. Pipelines are made per material,
draw mode and depth, and dropped with their material. The engine's shaders: the painted ground
(`render/ground.rs`), the figures' cel shader with its depth-only and shadow variants
(`render/model.rs`), a picture's shape in white (`runtime/juice.rs`); the light map's multiply
(`render/lighting.rs`) and additive pictures (`runtime/pictures.rs`) are blend states over
`DEFAULT_SHADER`.

**Text** (`text.rs`, `text/atlas.rs`) is macroquad's code over fontdue: glyphs are rasterised per
size into an atlas texture, laid out as macroquad laid them out. The built-in font is ProggyClean
(`crates/gfx/src/ProggyClean.ttf`, Latin-1 only); `load_ttf_font_from_bytes` loads a project's
fonts. The engine's text layer (`runtime/text.rs`, `fonts.rs`) sits on top of it (§8).

**Sound** (`audio.rs`) is kira's mixer behind macroquad's mixer's API, which the engine's
`audio.rs` was written for: a `Sound` is decoded whole (`StaticSoundData`: WAV, Ogg Vorbis),
`play_sound` starts it once or looped (kira's loop region) at a volume (turned into decibels), and
`stop_sound` / `set_sound_volume` reach every playing of it. There is no pitch and no pan. The
device opens with the first sound loaded; one that can't be opened, or panics while opening, means
silence. On the web the device opens with the player's first key, click or touch
(`user_gesture`: browsers allow sound only then), and loops asked for before (the music) start
with it. The engine's side is unchanged (§8, *Audio*).

**Input** (`input.rs`). An `Input` is fed with events (keys named by what they type in the
keyboard's layout, as miniquad's key codes were; typed characters; mouse buttons, position and
wheel; touches) and read through the module's functions; positions are fed in pixels and read in
points, and `end_frame` forgets what went down or up this frame. The window feeds the context's
own; an application that embeds the engine feeds one of its own and makes it current with
`with_screen`. Gamepads are not in gfx: the engine polls gilrs once a frame
(`runtime/input.rs`, `poll_pad`), on the web the browser's Gamepad API through `gfx::web`.
Autoplay's injected keys and mouse are also the engine's (`runtime/input.rs`: `inject`,
`inject_mouse`, `inject_text`), above gfx.

**Time** (`time.rs`): the clock moves at `begin_frame`; `get_time`, `get_frame_time`, `get_fps`.
`set_fixed_step` makes every frame take the same time whatever the real clock says
(`RUSTPG2D_FIXED_STEP` in the runner, the headless test), so two runs draw the same frames.

**The web.** The game is built for `wasm32-unknown-unknown`; `cargo xtask-web` runs
wasm-bindgen's second step as a library (xtask's `web` feature) and writes the pair
`target/web/rustpg2d-game.wasm` + `rustpg2d-game.js`, which the web export copies beside
`index.html` (`crates/engine/web`) and `data.pack`. In the browser winit draws on the page's canvas,
the device is asked for in the background with wgpu's GL backend only (WebGL 2, not WebGPU: reading
a picture back at once for screenshots, save thumbnails and photo mode is a `readPixels` on wgpu's
WebGL context, while wgpu's own read-back needs a turn of the browser's event loop that the engine's
callers don't give), and a frame is drawn per `RedrawRequested`. `gfx::web` is the browser's side:
the clock, the console, `localStorage` (keys per page folder), the query string, the language, the
gamepad, downloads and fetching files. The runner fetches `data.pack`, mounts its files as `data/`
in `sys`'s in-memory table (`sys::web::mount`, which adds the files written in earlier visits from
`localStorage`), decodes the sounds before the game starts (`audio::preload`, with a loading
screen) and takes an autoplay script from `?autoplay=`. Rhai is built there without its default
features (`std`, `no_time`).

## 4. The runtime (`crates/engine/src/runtime/`)

`Game` (`mod.rs`) owns the project (with the translated `Database` and the original), `Assets`,
`Audio`, `world: WorldState` (what gets saved), `loaded: Loaded` (the current map's runtime state:
map, per-tile `solid` and `ladder` flags, the `entities`, tile lights), a cache of the last four
maps, the player's `Body`, the `ActionRunner` and the UI windows' states. A `Game` has no window of
its own: it draws on whatever is the screen and reads whatever is the input (§3), so the runner,
the editor's Play tab and the headless test all run the same `Game`.

`RtEntity` is a flat struct per character, prop or dropped item on the current map: kind,
definition id, uid, `Body` (feet position, facing, hitbox, speed, animation timer), combat, patrol,
schedule, companion, visitor, AI and status-effect state. **Indices are valid within a frame only;
anything that lives longer stores a uid.** Uid ranges: placed entities are small numbers, spawned
characters start at `1 << 30`, companions at `0x5000_0000`, visitors (scheduled characters from
other maps) at `0x5800_0000`, furniture the player placed at `0x5C00_0000`, dropped items at
`0x6000_0000`. `persist(i)` writes an entity's
state back into `world.entities[map]`.

### Frame

The runner (`crates/game/src/main.rs`) is a future run by `gfx::window::run`: each turn it applies
the autoplay script's step, calls `Game::frame` and ends with `next_frame().await`. `Game::frame` is
`step(get_frame_time())` then `draw()`; `step(real)` polls the gamepad and the devices and runs
`update(dt)` with `dt` = the frame time capped at 50 ms (variable step). A host that shows the game
itself calls `step` (several times a frame to fast-forward, with part of the time for slow motion)
and `draw` on its own. `update` runs, in order:

1. summoned creatures that vanished last frame leave the entity list (`RtEntity::gone`: nothing
   holds an entity's index at this point, as it may deep inside a fight's damage), the occupancy
   grid, screen shake, thumbnail capture, the script-side player mirror;
2. modal screens that pause the world and return early: console, character creator, minigame,
   sleeping, "heal whom?", orders menu, pause menu, world map;
3. play time, achievements, the clock (`advance_hours`, crops regrow), *Time reached* triggers;
4. debug toggle, quick save / quick load;
5. the fade state machine (`finish_transition` switches the map at full black);
6. pending events, autosave, game over;
7. UI windows take input first, then the mouse (`mouse.rs`: click-to-move with A*, right-click
   interact, clickable UI);
8. the action runner (taken out with `mem::take`; actions queued meanwhile are appended);
9. `frozen` = runner busy, fading or modal;
10. `update_turns` while a fight is on, `update_player`, fog reveal, `update_entities`,
    `update_combat` (timers; outside fights the Attack key), projectiles, spawners, effects and
    weather, gathering.

`update_entities` handles every character by priority: scripted `move_target`, a fighter's
turn (it only walks its `move_target`), item use, knockback, speed effects, companion, chase AI,
schedule, activity, patrol, then a wander fallback. The chase AI (`ai.rs`) notices: a hostile
character that notices the player or a companion calls `start_combat`.

**Turn-based fights** (`runtime/turns/`, SRD 5.1) live in `Game.fight: Option<Box<Combat>>`, which
is never saved (`write_save` refuses during a fight, the autosave waits; `enter_map` ends one).
`start_combat` gathers the fighters within 12 tiles (`Fighter`: `Who::Player | Who::Npc(uid)`, the
side, initiative, death saves, Dodge / hidden / reaction / readied / grapple state), snaps them to
tile middles, sorts them by d20 + DEX; later hostiles `join_fight` at the end of the order. Each
turn (`begin_turn`: conditions tick a round, death saves, standing up from prone) has a `Turn`
economy; `update_turns` walks the current fighter along its path a tile per move target
(opportunity and readied attacks on each step), runs the AI (`ai.rs`, one step per pause) or the
player's input (`ui.rs`). Attacks are `AttackSpec`s (the hero's weapon, a stat block's attacks, a
derived block for characters without one) rolled by `srd/combat.rs` (pure: `odds`, `attack`,
`crit_dice`, `Cover::of_lines`, `DeathSaves`); `combat_damage` applies them through the target's
defenses and the `damage` hook, and `strike_entity` / `hurt_player_by` route every other damage
(scripts, spells, areas) there while a fight is on. Party members at 0 HP fall (unconscious, death
saves), others die (`defeat`). `check_over` ends the fight when no enemy fights on, or loses it
when nobody of the party stands and those down are stable. Movement reach is a Dijkstra over
tiles (`reachable`: foes block, allies are passed, diagonals past free corners, difficult materials
cost double), cover counts lines from the attacker's tile middle to the target tile's corners
through walls, high obstacles and creatures.

**Spawners and encounters** (`runtime/spawner.rs`) share `SpawnerRt`: a zone, a group (a
character id per member), respawn timers. A spawner's group is its `count` of one character; an
encounter's (`ZoneKind::Encounter`) is its list, or with a difficulty a group `srd/encounter.rs`
picks from it within the party's XP budget (`budget`: the hero's level and the companions on the
map; `pick` seeded by `seed_of(map, zone)`, so a map shows the same group every visit), computed
when the map is entered (`setup_spawners`). `missing_member` decides which member a respawn
brings back. The SRD's monsters are no engine data: `srd_import` writes them as starter library
characters (`import_monsters`), and `RulesText::monsters` lists their ids for `srd_content`.
Targets are `Foe::Player`, `Foe::Companion(uid)` and `Foe::Npc(uid)`: factions
(`factions.rs`, `ProjectConfig::factions`) become a `factions::Table` (faction index per
`RtEntity::faction`, hostility as a bit matrix) when the game starts, so `npc_foes` / `foes` are
lookups. `RtEntity::hostile` stays the one flag "hostile to the player" every system reads; it is
computed by `hostile_to_player` (the *Hostile* box and the player's standing, `WorldState::standing`)
and recomputed by `refresh_hostility` when a standing or the player's race changes. A character's
faction id (`Game::faction_id`) is what *Set faction* gave it (`RtEntity::faction_set`, saved as
`EntityState::faction`), else the placed character's own (`EntityInstance::faction`), else its
definition's or race's (`Database::faction_of`); the player's is `WorldState::player_faction`, else
their race's. `set_faction` recomputes the one character (`apply_faction`), or all of them for
the player. Projectiles
remember whether their shooter was hostile to the player and its faction, so they hit only foes.
Walking characters share `walk_to_tile` (`patrol.rs`): an A* path (`path.rs`, budget per
waypoint), one tile at a time through `try_move`, with re-planning when blocked and giving up
after a few seconds. The player moves through `try_move` too: solid tiles, solid entities and the
player block; ladders block characters and are climbed by the player (vertical only, slower).

The view is isometric (`render/iso.rs`): the world stays a square grid of 32 px tiles (maps,
collision, pathfinding, the fight grid and saves know nothing of the view), and a world point
lands at (x − y, (x + y) / 2) on the screen, a tile being a 64 × 32 diamond at zoom 1. `Game::view`
(an `iso::View`: target, zoom, window size; kept for the frame in `frame_view`, so mouse picking
between frames sees what is on the screen) gives two kinds of camera:

- the ground camera (`Game::camera`): a `Camera2D` turned 45° and squashed to half height. All
  that lies flat is drawn through it in world coordinates (ground, flat decorations and flat props,
  the fight's tiles and areas, the light map, the mouse's tile), and its inverse is the mouse
  picking of the ground;
- upright cameras (`View::upright(feet)`, `Game::upright(feet, ..)`): unturned, at √2 × the zoom,
  with `feet` where the ground camera shows them. Everything that stands (characters, props,
  sorted decorations, projectiles, and the overlays drawn at offsets from feet: health bars,
  emotes, padlocks, effects) is drawn through the camera of its own feet, so its drawing code keeps
  its world coordinates. Screen-space overlays (speech bubbles, quest markers, damage numbers)
  take `View::above(feet, height)`.

Changing the camera per upright thing is cheap because gfx keeps each draw call's projection
instead of flushing (§3). Depth: `iso::order` sorts by `x + y` of the feet; props whose footprint (`PropDef::hitbox` at the
feet) is wider than about a tile are ordered box to box against the rest (the one wholly west or
north of the other is farther), so a character beside a building's side is drawn right. Entities
are picked on the screen (`entity_screen_rect`: the click rect standing at the feet). Keys and the
stick move along the screen (`iso::screen_dir_to_world`: up = the map's north-west); click to walk
is unchanged. Figures are rendered 30° from above and turned `ISO_TURN` (−45°) against the world.
The editor's map view (`studio/src/scene.rs`) uses the same `iso::View` (`Look::iso`, `None` while
the tab's *From above* switch is on): `Look::ground` is its ground camera, so marks drawn in world
coordinates and the pointer's `Look::to_world` need nothing else; `Scene::standing(feet, ..)` wraps
what stands in its upright camera, and `Scene::entities_at` picks entities on the screen.

`draw` runs: the view, parallax layers behind, the painted ground, flat decorations, the map
border's fade, fish shadows, flat props, figures' blob shadows, the contact shadows of 3D props (`Assets::draw_prop_contact`: `iso::contact_shadow` on the model's footprint, `Models::ground_rect`) and the ground shadows of props drawn as pictures (`Assets::draw_prop_shadow`: the picture sheared along the sun as `model::SHADOW_SHEAR` from a quarter of its width behind its feet (the middle of what is painted), dark and half clear, unless `PropDef::shadow` is off), then entities, the player, sorted decorations and what
flies in depth order (each through its upright camera), combat / the fight's tiles / gathering /
activity / FX / mouse overlays, decorations above everyone, parallax above, the light pass when it is dark or the weather tints, weather, the save
thumbnail, the debug overlay, the fade, HUD, windows, dialog, menus, console.

The windows share their building blocks in `runtime/ui.rs`: `ScrollList` (selection, scrolling,
double-click; the trade, crafting and menu lists), `slot_grid` with `DragState` (the item slots of
the inventory and companion windows, one drag-and-drop state machine), `draw_item_detail` (the
item footer), dialogs and toasts. A new window composes these instead of copying a loop.

### Maps, events, scripts

Map changes: `transition(map, arrival)` starts a fade; `finish_transition` → `take_map` (cache or
disk) → `enter_map`: the furniture the player placed there joins the map's entities (as placed
props, so containers, locks and states find it by uid), the old map's patrols are persisted, `solid` / `shot` come from the collision
overrides and the ground (`MaterialTable::apply_to_tiles`), ladders and lights from decorations, entities are built (placed ones with their saved state, companions,
visitors, dropped items), scheduled characters are placed where their day says, the arrival spot
is made free, spawners and regrowth run, music starts, *Map enter* fires. `load_game` replaces the
world and re-enters the save's map the same way, without *Map enter* (`adopt_world` brings an old
save up to date; `StartOptions.load` = `--load FILE` does the same in `Game::new`, for the editor's
*Play from here with the test setup*, whose save editor-core's `testsetup.rs` writes:
`write_test_save`, `testplay.ron` in the editor's settings folder).

Events: `fire(event, source)` collects matching triggers (definition, instance, companion and
visitor triggers), checks `once` keys in `world.fired` and the conditions, and pushes the actions
to the `ActionRunner` (`events/runner.rs`). The runner handles waits, `If`, variables and items
itself and sends the rest to `Game::execute`, which answers Continue, Block (a dialog, a walk) or
Insert (more actions). Scripts (`script.rs`) run in a Rhai engine limited to 2 million operations
and 64 call levels; the `WorldState` is moved into the script context and back around each call.
Queued script functions (`say`, `walk_to`, …) become actions on the runner; immediate ones read and
change state directly.
Script hooks (`hooks.rs`, `ProjectConfig::hooks`) turn this around: the engine calls them.
`Game::hook(name, map)` (`runtime/hooks.rs`) runs the functions of a hook in priority order through
`ScriptHost::call_hook` (200 000 operations each), passing each the map the one before returned, and
pushes the actions they queue onto the runner (kept even while the runner runs: `update` appends the
stand-in runner's queue). Call sites check `hooked(name)` before building a map, so unregistered
hooks cost nothing. Shop prices are drawn every frame, so `buy` / `sell` run when the trade window
opens and after each trade, into a price cache (`ShopUi::hooked`) that `shop_rows` reads.

Plugins (`plugins.rs`) are folders `plugins/<id>/` beside the project's own content. The editor
never merges them: its Scripts & plugins page (`studio/src/pluginspage.rs` over editor-core's
`plugins.rs`) adds, removes, orders and configures them (`ProjectConfig::plugins`). The game merges the
enabled ones when it starts (`plugins::apply` at the top of `Game::new`): their hooks join the
project's, their menu entries the title and pause menus, their definitions the database where the
project has no definition of that id (file paths made relative to the project), and their script
folders are compiled after the project's (`ScriptHost::set_extra_dirs`, also on hot reload);
`plugin_param` reads the parameters. Exports add the enabled folders' files; libraries get the
starter's plugins in `open_library` (`plugins::sync_from_files`, by version).

Quests described in the quest editor (`QuestDef::giver`, `offer`, `steps`, `rewards`) are played
by `runtime/quests.rs` as ordinary actions: talking to a character (`interact_entity`, after its
own *Interact* triggers) pushes a Talk step's lines plus a *Quest* advance / complete, or the offer
(dialogs, a *Choice*, an *If* with *Quest* start; built by `QuestDef::offer_actions`), or the
giver's line; Conditions steps are checked every 0.25 s while the runner is idle; Reach steps on a
zone *Enter* or an interaction with the stage's target. Every quest change goes through
`quest_changed` (`runtime/journal.rs`), whose `Step::Insert` runs `QuestDef::follow_up` right after
the *Quest* action: the old step's done actions, the new step's start actions, the rewards.

### Rendering and assets

`render/ground.rs` draws the ground (above), sprites draw at their feet (`Assets::draw_sprite`;
decorations by chunk, `DecorIndex::near`). Characters are 3D models (below); figure sheets of 0.17
load as plain sprites. `render/lighting.rs` renders the lights into a
half-resolution target (ambient colour, one additive glow per light in view) and multiplies it
over the scene. `Assets::load` loads every sheet and material texture up front (materials' average
colours for overviews); a missing image becomes a checkerboard.
A sheet's `SheetTex` cuts frames in image pixels (`frame_w`, `src_rect`) but every draw function
draws them at `world_size()` = image size / `SheetDef::scale`, so high-resolution (2x) sheets
take the same place as 1x ones; code that needs a sprite's size in the world uses `world_size()`
or `sprite_size()`, never `frame_w` (the editor's thumbnails take a frame's part of the texture in
image pixels, `SheetTex::region`).
What a character does is an `Anim` (`render/character.rs`: walk, idle, attack, hurt, an extra clip);
`paperdoll.rs` picks it (`player_anim`, `npc_anim`: attack phase, `RtEntity::hurt`, moving,
breathing) and `Game::look` / `Models::look_of` say which model draws it: the look's model, else the
model named like its legacy `body` (a 0.17 figure's id), else the mannequin (built into the engine,
`models/mannequin*.glb`, unless the project has its own). Held items (`hands.rs`): an item's
`HeldSprite` (picture, hold point, orientation mark) is laid onto a hand's `Grip` by `lay` (mirrored
across the axis when it points left; carried props upright), two-handed things on `both_hands`;
`held_with` turns a character's equipment (or the tool of its activity, or a carried prop) into
`HeldLayer`s at given grips. An item with a 3D model (`ItemModel`) is drawn as that mesh instead: `render/model.rs`
builds each hand's frame (grip point, thumb-side grip axis, forearm direction) and
`model::hold_rotation` turns the item's handle direction and front onto it by its `HoldStyle`
(*Grip*: grip axis and forearm; *Upright*: up and the facing; *Hang*: down and the facing; *Strap*:
up and the facing turned outward). `model::estimate_hold` guesses grip, handle and front from the
mesh along its `main_axes` (the tightest rectangle in its main plane), with slice widths from
triangles clipped to each slice. A pose's frame is the model's standard frame grown by whatever
sticks out of it (a spear, a swung arm; in steps of 8 px): `Cell::base` is the standard frame inside
the cell, so map drawing (anchored at the feet) is unchanged and UI boxes (`draw_in`, `draw_part`,
the editor's previews) show the standard frame at their size, held items reaching past it.
3D characters (`model.rs`, `render/model.rs`): a character is drawn from the rigged glTF of its
`ModelDef`. `Model::load` keeps the skinned mesh, the node tree, the
skin's joints with their inverse bind matrices and the clips (`add_clips` adds those of other files,
matched by node name); `pose` samples a clip (stepped at the clip's fps by the caller) into world
matrices and removes the root's horizontal motion; `skin` deforms the mesh on the CPU (the draw
list uploads its vertices at every flush anyway). `render::model::Models` renders a pose
(`PoseReq`: look, `Gear`, animation, time, facing) once: the mesh turned to the facing plus the
clip's alignment (`facing_of`: square to the hips and shoulders), into a scratch render target with
depth through an orthographic `Camera3D` 30° above (busts: 20°), turned `ISO_TURN`, with the cel shader (WGSL: light bands, a
colour step; `Vertex::normal` carries the normals); held items are quads at the hand bones (the
grip axis is the hand bone's thumb-side axis, found in the rest pose) laid out by `hands::held_with`;
then the picture is copied with a 1-pixel dark outline into an atlas page (1024², shelf-packed, the
oldest of 8 pages reused) and its `Cell` cached by (model, colours, gear, animation, frame, facing).
At most 24 new poses render per frame; `push_camera_state` lets this happen in the middle of the
world pass (the draw list records each call's target, so the pose's render pass and the world's
alternate). The world, windows and portraits draw cells like sprites (`draw`, `draw_in`,
`draw_part`, `draw_rotated`); busts are their own 64 x 64 close-ups of the head bone. A model is one
mesh drawn in one call: gfx takes up to 65,535 vertices per mesh, and models are checked against
the engine's own `DRAW_VERTICES` / `DRAW_INDICES` (32,768 / 98,304, the sizes 0.35's windows asked
macroquad for; `oversize` warns above them). Model textures are reduced to 32 colours when loaded (`model::quantize`), so colour
features pick exact shades of it (`recolor.rs`: each shade by its brightness relative to the
feature's middle shade); models without textures get a palette texel per
material; models 96 px and taller (big props) keep 512 px and 48 colours (`model::BIG_MODEL`).
**Buildings** (`building.rs`) are props with a `BuildingShape`: `building::build` makes a still
`Model` (`Model::still`: one joint, no clips; units are tiles, x east, y up, z south, the origin in
the footprint's middle) from the hitbox, walls, roof planes, door and window quads and chimney
boxes, each face cut along its picture's size (`clip`, Sutherland-Hodgman) so every piece maps
onto one cell of a packed texture atlas; `Models::load` registers it as `@building:<prop>`
(`PropDef::model_at` names it), and the editor builds one again when its definition changes
(editor-core's `Workspace::sync_buildings` calls `Assets::rebuild_building`, pictures from the
loaded sheets, which rebuilds the buildings joined to it too). A shape without `walls` is a roof alone. **Joined buildings** (`BuildingShape.joins`: the
other prop and its footprint's offset) stay separate props, so each keeps its own hitbox for
collision and depth order; `building::build_joined` runs a roof whose ridge points into the other
building on until it meets that one's roof (`roof_ends`), no wider than the other's footprint, and
puts the other's shape into `Model::occluder`: triangles the pose renderer draws into the depth
buffer only, before the model. Whichever of the two sprites is drawn later is then transparent
where the other is nearer the camera, so the pair composes like one model in any order; their
ground shadows are depth-tested against the same geometry, the faces inside the other building
(`Model::shadowless`, the last indices) cast none, and joined buildings skip the dark outline
(it would draw a seam across shared walls). Items' 3D models (`ItemDef.model`) load with the character models and are drawn in the
pose's scene, turned so their handle lies on the hand's grip axis. Unrigged models with a rig
template (`ModelDef.rig`) are fitted when they load (`Model::fit_rig`): the mesh is turned, the
template's joints are stretched from the template mesh's bounds over the model's (per axis) plus
the hand-moved offsets, joint translations are solved down the tree keeping the template's
rotations (so its clips mean the same), each vertex is bound to the four nearest bones
(1 / distance⁴) and the clips' translations are rescaled to the height. `Loaded` keeps the
unfitted mesh and the template, so a changed rig (the model workshop's fixes, an agent's) refits
without reading files (`Models::set_def`).

**Figures from pieces.** A model with `ModelDef.part` is a piece (hair, an outfit, a helmet): it is
never a figure of its own, it is worn. `parts_of` decides what a figure shows, per slot: its own
pieces (`ModelDef.parts`), replaced by the look's choices (`Appearance.parts`: a piece's id, a
style, or none) and then by worn items whose `ItemDef.wear` names a style (`Gear.worn`); a piece's
`hides` removes other slots. In `render`, the figure's pose is computed once; each piece is skinned
with those node transforms through `Model::bind_to` (its joints as the figure's nodes of the same
names, cached per figure) and drawn in the same pass with its own palette texture, recoloured by
the same feature names (a piece takes the figure's feature default before its own). The pose key
includes the pieces. A figure whose file is only its head sets `ModelDef.stand`, the whole
figure's height, which the frame, the scale and the hand grip use. A model with more vertices or
triangles than one draw takes is listed in `Models.warnings` (`oversize`). The starter library's
playable races are built this way by `scripts/art/kit` (the figure kit: voxel blocks on the
hero's skeleton, one shared clip file fitted to each build by `shared_clips`). The kit's creatures
are unrigged sculpts with a rig fit (`ModelDef.rig`: a template and where its joints go).
`Models::portrait_facing` says which way a still picture shows a look best (the side of a model
much longer than wide; the 0.35 editor's thumbnails used it, the new editor's figures face the
viewer).

The editor shows rendered poses without copying them: `studio/src/thumbs.rs` registers the atlas
page's wgpu texture with egui-wgpu (`register_native_texture`) and paints the pose's part of it
(§5, *Pictures of assets*).

## 5. The editor (`crates/editor-core`, `crates/studio`)

**Three layers.** The engine holds the data, the game and the renderer. `crates/editor-core` is
everything the editor knows and does without a UI (it does not depend on egui): the workspace, the
commands, the one history, the field schema, search, problems, renames, the project file, packs,
the agent server. `crates/studio` is the window (binary `rustpg2d-studio` until it replaces the old
editor's name): an eframe application whose tabs, forms and map views work on editor-core's
`Workspace`. Three ways in reach the same workspace: the window, the editor script
(`RUSTPG2D_STUDIO_SCRIPT`) and the agent server.

**The workspace** (`workspace.rs`) is an open project with its definitions, the map being worked on
(`doc`, a `MapDoc` with its edits) and the other open maps with their histories (`parked`), the
structures open as maps (`struct_docs`, `structdoc.rs`: a structure worked on as a map of its own,
`@structure:ID`, a margin larger on every side), the selection, camera and view options of the
map worked on, the asset library's state, the problems, search and usages, Generate art's jobs,
the agent server, the file watcher, and what the window tells agents (`looking_at`, `asking`).
`Workspace::open_path(path, Gpu)` opens a project file or folder (below). `Gpu::Yes` loads the
textures and models (`Assets`), which needs gfx set up on the thread; `Gpu::No` (the headless
agent server, tests) loads no pictures and draws nothing. Projects older than 0.35 (16 px tiles,
tile maps, an older rules version) are refused with a message naming 0.35.

**Commands** (`commands.rs`). Every operation that can be done by name is one `Command` of the
list `COMMANDS`: an id (`group.name`: `file.save_all`, `edit.rename`, `defs.make`, `map.new`,
`packs.install`, `agent.start`…), a title, a default shortcut, the words it takes, one sentence
of help, and whether it changes the project. `Workspace::run(id, args)` / `run_line("map.open
house")` run one; its answer (or what kept it from running) becomes the status line, and what it
changed is one history entry under its title. The window's menu bar and command palette are made
from the list, and the editor script runs any line that isn't one of its own words as a command.
The window takes over a few that open something of its own (`assets.cleanup`, `file.close_project`,
`map.graph`: `Shell::run`). Agent tools are a list of their own (`mcp/tools.rs`): they describe
themselves to a model and take structured parameters, and change the project through the same
workspace.

**One history** (`timeline.rs`). Every undoable change is a `Change` in the order it was made: where
(the definitions and settings, or a map by name), a label ("Rename item", "Paint ground"), and
whether it belongs to the change before it (`joined`: a built house is a new prop and a thing on a
map, two linked transitions a change of each map; they are undone together). *Undo* takes back the
latest change wherever it was; a document can undo its own latest. Underneath are two kinds of
steps: the definitions' (`history.rs`: `Workspace::history_frame` compares the definitions and
settings with its last look and turns a difference into a step that keeps only the lists that
changed, or the one definition; typing into the same field within 1.5 s is one step; renames undo
by renaming back, since they rewrite maps and scripts on disk) and each map's (`doc.rs`: `Edit`s of
collision, ground cells, decorations, objects, the whole map, or a group). **Changes are observed,
not funnelled:** forms, tools and agents change things directly and `history_frame` notices the new
steps; `begin(label)` names what comes next (commands do it with their title, agents with
`Agent: TOOL`). A drag over many frames is one step (`shell::hold_step`). A script's text has the
text field's own undo, not the project's; the creator's notes (`meta.rs`, `editor/meta.ron`:
folders, tags, notes, favourites) are written at once but are steps of the history.

**The field schema and the forms.** With the feature `schema` every serde type of the engine derives
`schemars::JsonSchema` (`#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]`); types with
a hand-written format get theirs in `engine/src/schema.rs`. The schema is the data *as written*:
fields in order, what each holds, enum variants, defaults, and each field's doc comment, which is
the form's help text. editor-core's `schema.rs` reads it (`of_kind`, `project`, `entity`, `zone`,
`trigger`, `map_settings`; `tidy_schema` makes an `Option` one control with a way to say "none")
and adds what only the editor knows, in tables: `LABELS`, `SECTIONS` (test
`every_field_has_its_section`), `REFERENCES` and `NOT_FOLLOWED` (the fields that name another
definition; test `references_agree_with_renames` renames every kind through made-up values and
fails unless the table lists exactly the texts that changed), `FILES` (fields naming a file of the
project), `NAMES` (flags, variables, script functions) and `PAGES` (the settings' pages; test
`every_setting_is_on_one_page`). A form (`studio/src/form.rs`) edits a definition's JSON value
(`Workspace::def_value`), choosing the control by the field's shape (a choice for an enum, a picker
for an id, lists with add, remove and move, groups that fold); the workspace turns the value back
into the definition and refuses what isn't one (`set_def_value`), and `history_frame` makes the
step. Parts with an editor of their own are plugged in: the event sheet (`sheet.rs`) for every
list of triggers, the workshops at the top of an asset's tab (props, items, characters, models,
animations, sheets, sounds, the pixel editor), each over a part of editor-core that is tested
without a window (`events.rs`, `pixels.rs`, `modelcheck.rs`, `looks.rs`, `visualedit.rs`,
`scripts.rs`…). Scripts are read by one lexer (`script_refs::lex` in the engine): editor-core's
`scripts.rs` (highlighting, suggestions, the problems of a text) and the reference finder work on
its tokens.

**One device for the engine and egui.** The window is eframe with its wgpu renderer
(`eframe::Renderer::Wgpu`); `Studio::new` hands eframe's device and queue to `gfx::init`, so every
texture and screen the engine makes is a wgpu texture egui-wgpu can show by registering its view
(`register_native_texture`). Each egui frame (`Studio::ui`) begins and ends a gfx frame around the
UI: `gfx::begin_frame`, the script's next commands, the agents' calls (`mcp_drain`), the shell's
UI (which draws the engine's views as it lays them out, each inside `gfx::with_screen`, whose
flushes submit the engine's work before eframe paints), the agents' screenshots, `gfx::end_frame`,
and a request for the next frame (the game and the agents move on by themselves). The window is
X11 where there is an X server, as the game's (`RUSTPG2D_WAYLAND=1`).

**The shell** (`shell.rs`): panels and documents as tabs that can be arranged (egui_dock): the
Project browser, documents in the middle, Problems and History below, a menu bar made from the
command list, the status bar; the command palette (Ctrl+Shift+P) and *Go to anything* (Ctrl+P, the
core's search). A `Tab` is the browser, the Overview (the home screen while no project is open),
Play, a settings page, everything of a kind, a definition, a map, the World view, a map's events
graph, a script, Flags and variables, Publishing, the Library, Generate art, Help or AI agents. The
asset library is a tab of this program (`librarypage.rs`, packs in `packspage.rs`), not a second
process. File dialogs (`pick.rs`, rfd) run beside the window, which keeps drawing; the answer is
taken by a key in a later frame.

**Maps in the editor.** A map's tab is a `MapView` (`mapview.rs`: its camera, tool and selection;
two tabs of two maps each have their own); the map pressed on becomes the workspace's `doc`, the one
commands and agents act on (`Workspace::open_map`). The view draws the map with the engine's
renderer into a `gfx::Screen` of the tab's size in pixels, inside `with_screen` (`scene.rs`: the
ground, flat things, then everything standing in depth order, isometric or from straight above),
and egui shows that texture. What lies on the ground (zones, the grid, blocked tiles, frames) is
drawn by the engine in the same pass with the ground camera; names and handles are egui's, painted
over the picture, so they stay sharp at any zoom. Above `OVERVIEW_THRESHOLD` visible tiles the map
is drawn as its `Overview`. What changes a map is editor-core's: `mapedit.rs` (placing, moving,
painting, zones, a new map, a resize; a step each), `build.rs` (the Build tool: walls, rooms, houses,
roofs, openings), `structdoc.rs`. Without a graphics adapter nothing is drawn. `MapView::shot` draws
a rectangle of tiles off screen for an agent's `screenshot_map`.

**The game in a tab** (`play.rs`). A `PlayView` holds a `Game`, its own `Screen` and its own
`gfx::input::Input`; the game is made inside `with_screen`, because it sizes things by its screen.
The game reads the project's files, like the runner, so starting it saves everything first. Each
frame the screen is resized to the tab (in physical pixels, at egui's pixels per point), the tab's
egui events become the game's input (keys and text while the pointer is over the tab, releases
always, buttons, wheel, the pointer in pixels), and the game runs inside `with_screen`: at normal
speed `Game::frame`, otherwise `Game::step` as often as the speed asks (none while paused, one for
a single step) and then `Game::draw`. Alt+click, or the inspect switch, asks
`Game::thing_under_pointer` instead of clicking. The script's `key` and `keyhold` feed the same
`Input`. *Own window* starts the real runner beside the editor with the same start (`--dev`,
`--map`, `--pos`, `--load` for a test setup). Cut-scenes play from the timeline's playhead
(`StartOptions::cutscene_at`).

**Pictures of assets** (`thumbs.rs`). Nothing is drawn again for the UI: egui shows the engine's
own textures with the part that is the asset's: a sheet's frame, the page of the model atlas a
pose was rendered into (`Thumbs::figure`, `pose`), a material's texture. One exception: egui
blends colours already multiplied by their alpha and the engine keeps them apart, so a sheet shown
in the UI gets a multiplied copy the first time (soft edges would get bright fringes). Textures not
shown for 600 frames are given back to egui-wgpu. Structures are drawn as small maps of their own
into screens; the prop workshop's stage is a map drawn by the map view's scene code; the animation
stage (`animshop.rs`) paints egui meshes over the sheets' textures from the engine's
`animations.rs`, which works cels and particles out from the time alone.

**Project files** (`project_file.rs`): a project is one `<name>.r2dproject` in the `data.pack`
format. The editor never works inside it: `ProjectFile::open` unpacks it into a working copy
(`<data>/rustpg2d/work/<stem>-<hash of the path>`, or `<RUSTPG2D_EDITOR_CONFIG>/work` in tests) and
`Workspace::open_path` opens that folder as `project.root`, so the forms, the agent server,
play-tests and exports all keep working on plain files. A `.r2dwork` record in the working copy
holds the file's size and mtime and a fingerprint of the copy's files (paths, sizes, mtimes) at the
last unpack or pack; it decides at open whether the copy can be reused (also recovering work saved
after the last pack) or must be unpacked afresh (an unpacked copy of another state is set aside,
never deleted). Every two seconds (and after a save) the fingerprint is compared and, when it
differs, a thread packs the copy into a temporary file and renames it over the project file; the
record notes the temporary file as `pending` before the rename (a rename keeps size and mtime), so
an editor stopped between the two steps still recognises the file. A project file whose size or
mtime changed behind the editor's back isn't overwritten: the work goes to
`<name> (changed N).r2dproject`. Closing a project and quitting write it at once
(`Workspace::finish_project_file`, also from eframe's `on_exit`). Project folders are converted on
open (`<folder>.r2dproject` beside them); `RUSTPG2D_FOLDER_PROJECTS=1` opens them as they are,
which the smoke script's agent part relies on. **Repository mode** (`repo_mode.rs`): an editor
whose current folder is a checkout of the repository sets `RUSTPG2D_LIBRARY` (unless set) to the
starter library, so library writes land in `crates/editor/assets/starter_library/`, and opens
`example_project/` as a folder, not converted; the starter source folder never gets the starter
set applied (`starter::open_library`), which would bring back deleted assets. `RUSTPG2D_REPO=0`
turns it off. Started without a project, the window opens `example_project` when the current
folder has one, else the home screen.

**The agent server** (`mcp/`) is an MCP server (rmcp, Streamable HTTP on `127.0.0.1:7743/mcp` by
default) on a tokio thread of its own (`server.rs`). Tool calls travel to the main thread as
`AgentRequest`s and are executed against the workspace between frames (`Workspace::mcp_drain`: up
to 40 calls or 8 ms per frame), so a tool sees the editor's state like a user action; each call is
one history entry named after the tool, a tool that panics answers with an error and leaves the
editor running, and every write saves the project (`mcp_after_write`). Screenshots are answered by
whoever draws (`mcp_answer_shots` with a callback): in the window `MapView::shot` renders the tiles
asked for; a picture of the whole window is not taken yet. `play_test` (`play.rs`) plans the run on
the main thread, then runs the sibling `rustpg2d-game` binary with an autoplay script on the agent
thread and returns its status lines and screenshots; it warns when the runner's `--engine-build`
differs from the editor's. A file watcher (`watcher.rs`) compares the project's files once a second
(mtime and size) and reloads what changed on disk when the editor has no unsaved edits to it,
else asks. The 71 tools are declared once in `tools.rs` (`mcp_tools!`: description, annotations,
parameters and the workspace function each one runs); the `Call` enum, the tool names and the
dispatch are generated from that block. **Without a window**, `rustpg2d-studio --headless PROJECT
[--mcp PORT] [--seconds N]` opens the project with `Gpu::No`, starts the server and loops: drain the
calls, answer screenshots with an error that names `read_map` instead, check the files and the
project file once a second. Play-tests still start the game.

**The editor script** (`RUSTPG2D_STUDIO_SCRIPT`, `main.rs`) drives the real window. Its input words
(`click X Y [ctrl|right]`, `hover`, `drag`, `wheel`, `type`, `press`) become egui events that join
the window's own input (`raw_input_hook`), a batch a frame, so they go through the same widgets a
person uses; `mapclick` / `mapdrag` / `maphover` do it by tiles of the map view. Other words call
the window's functions (`tab`, `palette`, `goto`, `field KIND ID /POINTER VALUE` = a form's field,
`setting`, the workshops' and pages' words), `shot` saves the whole window
(`ViewportCommand::Screenshot`), `status` prints `[status]` and one line per part in view (`[game]`,
`[mapview]`, `[script]`…), `quit` ends without asking, `closewindow` is the close button, and any
other line is a command (`map.open village`, `edit.undo`). CLAUDE.md lists the words.

**The 0.35 editor** (`crates/editor`: macroquad and egui 0.31 through egui-macroquad) is out of the
workspace and is not built; it stays in the tree to read until its last parts are rebuilt. Its
UI-free files were copied into editor-core and adapted, with their tests.

## 6. Formats and compatibility rules

Projects, maps, definitions and saves of older versions must keep loading (AGENTS.md), with the
exception agreed for this release: the new editor opens projects saved by 0.35 and refuses older
ones with a message naming 0.35 (ROADMAP.md, 0.36.0). What the rules mean per format:

- **RON structs** (definitions, map `Meta`, saves, settings, project): new fields get
  `#[serde(default)]` (or `default = "fn"`); never rename a field without an alias; variants of
  enums stored by name (`Action`, `Cond`, `TriggerOn`, `ZoneKind`, `OnUnlock` in v2 maps and in
  definitions) may be added anywhere by format, but are **appended** by convention because v1
  maps stored them by index. A new serde type also derives `schemars::JsonSchema` under the
  feature `schema`, or the editor's forms can't show it.
- **postcard** (map `Core`, `TileLayer`, `Chunk`, `Ground`, `GroundChunk`): positional and without defaults. **These
  structs are frozen**: a new layer or chunk property goes into the RON `Meta`, or the format
  version is bumped with a reader for the old one.
- **Game packs** (`data.pack`, and project files `.r2dproject`): the 8-byte magic `R2DPAK1\0` carries the version; a game refuses
  a pack with another magic, which is fine because a pack is only ever read by the runner it was
  exported with (the export checks `ENGINE_BUILD_TAG`). Project files are read by later editors:
  the format (and the keystream, pinned by a test) must not change without a new magic and a
  reader for the old one.
- **Packs** (`.r2dpack`, `packs.rs`): `packs::FORMAT` is the version of the pack format itself; a
  reader refuses packs of a newer format instead of half-reading them. 0.35's asset packages,
  plugin files and starter library files are still read as packs.
- **Saves**: `WorldState` fields all default; unknown fields are ignored, so a newer save loads in
  an older game minus what it does not know. `SAVE_VERSION` (2 since 0.28) lets loading migrate:
  a save below 2 gets the project's starting SRD scores and proficiencies.
- **Libraries**: existing libraries only gain new ids from a changed starter set on their own
  (images that are pure extensions are replaced); changed assets arrive through *Update asset
  library*. Bump `STARTER_RULES` when the way starter sets are applied changes, and regenerate
  `starter_history.ron` whenever the starter set changes (the test says so).
- **Script API**: functions are never removed or change meaning within a major version; a new one
  is registered in `script.rs` and described once in the `API` table of `script_api.rs` (parameter
  kinds: `Arg::Id` for ids, `Arg::Line` for a line players read, `Arg::Text` for other words),
  which drives the editor's completion, renames, the script check, translation templates and the
  flag / variable collectors; the manual's chapter 8 (`Documentation/manual/08-scripting.md`) must describe it too (a test checks).

## 7. Verification tooling

| Tool | What it drives | Where it runs |
|---|---|---|
| `RUSTPG2D_AUTOPLAY="…"` | the game: key presses, clicks, saves, `status` (a line with every runtime state field), screenshots; in a browser `?autoplay=` | every feature is verified with it; `Documentation/manual/17-testing.md` |
| `RUSTPG2D_STUDIO_SCRIPT="…"` | the editor's real window: clicks, typing and keys through egui's input, the map view by tiles, forms' fields, pages and workshops, any editor command, `status`, `shot` | the same; CLAUDE.md lists the words |
| `crates/studio/examples/mcp_smoke.rs` | a real MCP client against the agent server (`PORT`, or `PORT headless` against `--headless`): tools, resources, prompts, painting with undo, `write_asset`, `upsert_def`, `capture_structure`, `library_send`, `play_test` | the smoke script; by hand with a scratch library |
| `cargo test --workspace` | unit tests: formats, library merges, scripting, translations, starter-library consistency, the field schema's tables; `crates/engine/tests/compat.rs` loads files of earlier versions (`tests/fixtures/`); `crates/engine/tests/headless.rs` plays the example's house for 90 frames without a window and compares the screen with `tests/golden/house.png` (`RUSTPG2D_BLESS=1` writes it anew; skipped without a graphics adapter unless `RUSTPG2D_GPU_TESTS=1`); `crates/editor-core/tests/workspace.rs` works on a scratch copy of the example without a window (a change undone and saved leaves every file byte for byte, one history over maps and definitions, renames, building, placing); the studio's `ui_tests` modules click and type through egui's AccessKit tree with `egui_kittest` on workspaces opened `Gpu::No` (every control needs a name) | CI on every pull request, with Vulkan's software renderer (`RUSTPG2D_GPU_TESTS=1`, `RUSTPG2D_ALLOW_SOFTWARE=1`) |
| `scripts/ci/smoke.sh` | the game on a scratch copy of the example project (title, a map, the clock, an old save, quick save, a map change with its autosave, a turn-based fight), the editor's agent server without a window driven by the MCP smoke client, and the editor in a window opening a map (the folder becoming a project file) and quitting cleanly | CI's *Runtime smoke* job (Xvfb) on every pull request; locally with a display |
| `scripts/studio/shots.sh` + `compare.py` | the renderer comparison set: the same autoplay runs with two runners (`RUSTPG2D_FIXED_STEP`: every frame a sixtieth of a second), compared picture by picture | by hand (0.35 against the new renderer) |
| CI (`.github/workflows/ci.yml`) | *Branch policy*, *Build, lint, test* on every PR; *Version*, the release builds (`builds.yml`: every platform at once, the web pair through `xtask web`), *Packages* (assembles AppImage/Flatpak/Windows/macOS, an Xvfb smoke run of game and editor), *Windows*, *macOS*, *macOS app* on release PRs (`release/X.Y.Z`); a merge into master with the tested tree only reports | GitHub Actions |
| Release workflow | on a version without its tag (a merged release PR): publishes the packages of the merged PR's CI run (builds them only when they are gone), tags | pushes to master |

## 8. Caveats and performance characteristics

Known properties of the current implementation (see the CHANGELOG for what each release changed):

- The ground is one quad per visible chunk with the ground shader; decorations near the view are
  drawn one quad each, and consecutive quads of the same texture share a draw call (with 0.35's
  renderer, 125 000 decorations on the 1024 × 1024 Wilds drew in under half a millisecond of CPU).
  Above `OVERVIEW_THRESHOLD` visible tiles (60,000) the editor draws a map as its `Overview`, one
  pixel per tile (the ground's colour, the colour of the decorations standing on the tile,
  refreshed when they change). Entities outside the view (with a margin for tall sprites) are
  skipped.
- Text uses gfx's built-in font (ProggyClean, Latin-1 glyphs only) unless the project has fonts, measured
  per draw. The runtime's `draw_text` / `measure_text` (`runtime/text.rs`) look every string up in
  the language's runtime texts first (`lang::tr_ui`, a hash lookup; English copies nothing);
  `draw_plain` / `measure_plain` draw content that is translated already. With project fonts
  (`ProjectConfig.fonts`, `defs/fonts.ron`) a string is split into runs: each character goes to the
  first font of the current role's chain (`text::role`: ui, dialog, hud, title; then the
  fallbacks; then the built-in font) whose character map (`fonts::Coverage`, read with ttf-parser
  once at start) has it, and each run is drawn with `draw_text_ex`. Pixel fonts are drawn at whole
  multiples of their grid (`FontDef::draw_size`) with Nearest filtering. Without project fonts the
  calls go straight to gfx. Wrapping is `text::wrap` / `wrap_counted` (lines and
  the characters each uses, for text that types out): pieces split at spaces and, in Chinese and
  Japanese, between characters except before closing / after opening punctuation (`breaks_anywhere`,
  kinsoku sets), measured with `measure_plain`; `render::wrap_text` calls it.
- Dialog message codes (`dialog_codes.rs`) are kept in the text through translation and
  `expand_text` (which leaves `{x:…}` alone). The dialog box (`runtime/ui.rs`, `Dialog::new`)
  replaces each colour / icon / pause / speed code with a private-use character (U+F0000 + its
  index), so the codes' places survive `text::wrap_with` (a code measures nothing, an icon a
  square) and the typing counts them; drawing switches colour at them and records icon spots that
  `Game` fills with item icons. `{name:…}` and `{at:…}` are applied when the box opens; a leading
  `{at:…}` is split off before the translation lookup (`split_box_at`), and `lang::content_texts`
  leaves it out the same way.
- The title screen and the pause menu (`menus.rs`: `ProjectConfig.title`, `.menu`, `.credits`) are
  built by `runtime/menu.rs::rows()` from their entry lists; an empty list is the built-in one
  (`TitleScreen::DEFAULT`, `PauseMenu::DEFAULT`), so old projects keep their menus. A custom entry
  is `Row::Custom(function)`: the pause menu closes and queues `Action::RunScript`; the title screen
  stays open and steps the action runner once at once (menus pause the game, so instant actions run
  and waiting ones stay queued). The title's map is loaded behind it at start and on *Quit to main
  menu* (`title_map`, `Game::title_scene`: camera tile, held hour, music); a picture background and
  the logo are sheets drawn in `draw_menu`.
- Picture layers (`pictures.rs`): `WorldState.pictures` by layer, each with its look and a move
  in progress (`PictureMove`: from, to, elapsed, easing), advanced in `update` and drawn in
  `runtime/pictures.rs` after the world, its lighting and weather (screen pictures by
  `script_ui::Anchor`, map pictures through the camera), before the fades and the HUD; additive
  blending uses a material made once.
- Script UI (`script_ui.rs`): HUD elements live in `WorldState.hud` (by id, saved), set by the
  script functions directly on the world the script holds; `runtime/script_hud.rs` draws them every
  frame with the HUD (texts through `expand_text`, bars reading `script_ui::Source`: a variable, a
  number, `hp:NAME` / `max_hp:NAME` of a character on the map). `window_open` / `window_close`
  write `WorldState.window_request` (`#[serde(skip)]`); the game takes it after scripts ran (once
  per frame, and right after a window button's function) into `Game.script_window`, a modal panel
  whose update runs before the menus. A button pushes `Action::RunScript` and steps the action
  runner once, so a function that reopens the window refreshes it at once.
- Audio (`audio.rs`) works over gfx's sound functions (§3), which know only volume and looping, as
  macroquad's mixer did: *Play sound*'s pitch and pan are rendered into stereo variants
  (`pitch_pan`) cached by id, pitch and pan; music is prepared for gapless loops by `loop_ready`
  (resampled to 44.1 kHz and stretched to whole mixer buffers of `MIX_BUFFER_FRAMES`), a
  preparation made for quad-snd's gap at the loop point that still runs on kira; *Play music*'s
  crossfade keeps the old track playing in `music_out` while both gains move; a music effect sets
  `effect` and `duck` (the music's extra gain) in `tick_music`, which `update_battle_music` calls
  once a frame. The project's `sfx.level_up` / `sfx.victory` are played as music effects
  (`progression.rs`, `battle.rs`: `Battle.won` counts the player's defeats in a fight).
- The camera (`Game::camera`) looks at `camera_focus()`: the player, or a `CamMove` of the
  *Camera* action (`runtime/camera.rs`: from the focus it had, eased toward a point or an entity,
  with a zoom multiplying the view scale). `update_camera` sends it back to the player once the
  main runner is idle, no cut-scene plays and the move arrived. *Scroll text*
  (`runtime/scroll_text.rs`) is a modal overlay updated before the menus, so it also rolls over
  the title screen.
- Common events (`common_events.rs`, `defs/common_events.ron`): *Call common event* returns
  `Step::Insert` of the event's actions with the caller's `ActionCtx`. `runtime/common.rs` runs
  after the main runner each frame: parallel events keep one `ActionRunner` each in
  `Game.common_runs` (dropped when the flag is cleared, cleared on load); an autorun event is
  pushed into the main runner while it is idle and its flag is set, so it freezes the player like
  any running event.
- Animations (`animations.rs`, `defs/animations.ron`): an `AnimationDef`'s cels, emitters,
  flashes and sounds are pure functions of the time (`Cel::pose`, `Emitter::particles`: each
  particle's start, direction and speed hash from its number), so the editor's animation stage
  (`studio/src/animshop.rs`, egui meshes over the sheets' textures) and the game draw the same. `runtime/animations.rs`
  keeps `Game.anims` (id, point, target, age; flashes and sounds started as the age passes them,
  dropped after `length`) and `tint_flashes` (a target's figure drawn again through the hit-flash
  shader in the flash's colour). `spawn_effect` plays an animation when the sheet id names one
  (*Play effect*, `effect()`, spell impacts); `AttackSpec.anim` (from `Weapon.hit_anim` /
  `StatAttack.anim`) plays in `land_attack`, `ItemDef.use_anim` in `use_item_now` /
  `use_item_on`, a class `Act`'s `anim` in `use_class_act`. A field that names a sheet or an
  animation reports both kinds as references; `Database::refs_of` keeps the one that exists.
- Cut-scene timelines (`cutscenes.rs`, `defs/cutscenes.ron`): a `CutsceneDef` holds tracks of
  `Cue`s (time, wait, an `Action`). *Play cutscene* looks the id up first and plays
  `CutsceneDef::actions(from)`: the cues sorted by time with *Wait* actions between them, cues
  that don't wait wrapped in `Action::Parallel`, which starts a side `ActionRunner`
  (`Game.side_runs`, updated after the main runner in `runtime/common.rs`; cleared on load). A
  start time (`--cutscene-at`, `Game.cutscene_from`) turns the earlier cues into their end state
  (`catch_up`: walks become *Place*, moves take 0 s, dialogs and sounds drop out). In the editor,
  editor-core's `cutscene.rs` works out where the actors are and what the camera frames at a moment
  (walks at the walker's speed, pans over their seconds; a dialog takes no time), the map's tab
  shows it while the timeline (`studio/src/timeline.rs`) is in view, and *Play from here* plays the
  cut-scene in the Play tab from the playhead.
- UI skins (`skin.rs`: `SkinDef` in `defs/skins.ron`, `ProjectConfig.skin` = the game's) are drawn
  by `runtime/skin.rs`, the one place window chrome comes from: `panel`, `dialog_box`, `name_box`,
  `hud_frame`, `button_picture` / `button_fill`, `cursor`, `toast_box`, `bar` (HP, mana, XP) and
  `text(kind, builtin)` for window texts. Every function takes the colour its place always used and
  returns it unchanged unless the skin sets that part, so the built-in look stays pixel for pixel
  (high contrast always draws the built-in look: `cur()` returns the empty skin then). `install`
  runs once at game start; it resolves the skin's pictures to textures (sheet frames) and hands its
  colour names to `dialog_codes::set_skin_colors`. Pictures are nine-slices
  (`render::draw_nine_slice`, rectangle maths in `skin::nine_parts` / `repeats`). `skin::draw_preview`
  draws a sample of a skin (the 0.35 editor's skin form showed it; the new editor doesn't call it
  yet). World drawing (markers, damage numbers, speech bubbles) is not skinned.
- The hot definition lookups (`db.item(id)`, `prop`, `character`, `sheet`) go through
  an id index that verifies its hits and falls back to a scan, so an index left stale by an
  in-place edit is slower, never wrong; `Database::reindex` refreshes it.
- Characters are 3D models drawn as pictures (`render/model.rs`):
  - Every model of the project is read and parsed when the assets load (`Models::load`), before the
    title screen; the example's models are about 34 MB of GLB. Start-up time and memory grow with
    the cast (in 0.18.2 the example's models took 0.22 s and 38 MB natively, 0.45 s in a browser).
    The web game downloads `data.pack` whole and uncompressed before it starts: the models are 29
    of the example's 46 MB.
  - A pose (model, colours, gear, animation frame at the clip's fps, facing and `turn`) is skinned on the CPU
    and rendered off screen once, then kept in an atlas of 1024 x 1024 pages (4 MB each), as many
    as `ProjectConfig::model_atlas_mb` allows (8 to 128 MB, default 32 = 8 pages). At most
    `RENDERS_PER_FRAME` (24) poses render per frame; over that, a character shows another cached
    frame of the same model and facing meanwhile.
  - Walkers on a diagonal are drawn turned an eighth from their four-way `Facing` (`PoseReq::turn`,
    from `Body::heading`, the last frame's step; `ProjectConfig::eight_facings`): their walk poses
    double, the facing itself (interaction, maps, saves) stays four-way.
  - Every pose drawn stamps its page; when all pages are full, the page drawn from longest ago is
    cleared and its poses render again when needed. In 0.18.2 the example's village filled 8
    pages after about four minutes of walking; even with 2 pages (8 MB) a frame stayed at 16.7 ms.
  - Recoloured model textures (one per model and colour set, `Loaded::textures`) are never evicted:
    bounded by the looks actually drawn. `status` prints `models=` poses / pages / most pages /
    renders this frame / renders in all / pages cleared; `RUSTPG2D_MODEL_DEBUG=1` shows the atlas.
- Pathfinding (`path.rs`) runs on the tile grid with a node budget per search and tells "no
  route" from "out of budget"; walkers (`patrol.rs::walk_to_tile`, shared by patrols, schedules,
  companions and chasers walking home) re-plan once per blockage and give up after four seconds.
  Planning and the broad phase of collision read the per-frame occupancy grid
  (`runtime/occupancy.rs`); code that moves an entity by more than a tile within a frame calls
  `occupy(i)`.
- Two collision grids per map: `Loaded::solid` (walking) and `Loaded::shot` (shots and sight:
  tiles with half their cells of a *Solid* material, not *Low*; a tile painted *force solid*
  counts). Movement tests the cells under a hitbox exactly (`tile_solid_cells`).
  Solid props that aren't `low` add `OCC_HIGH` to the occupancy grid; `Game::blocks_shot` reads
  both and is what projectiles and `line_clear` (aiming, enemy and companion line of fire, enemy
  sight) use.
- Pixel ↔ tile conversions follow `grid.rs` (feet one pixel above the tile's bottom edge; props an
  even number of tiles wide stand on a tile edge).
- `status` prints frame timing (`fps=`, `frame_ms=`, `update_ms=`, `draw_ms=`), the last flush's
  draw calls, passes and CPU time (`gpu=`) and the number of path searches (`astar=`) since the
  previous `status`; `RUSTPG2D_PERF=1` logs them every second.
- Files are written whole or not at all (`sys::write`: temporary file + rename). A definition
  entry or a map object that can't be read is left out with a log line, and the editor backs the
  file up before it could save without it.
- A graphics adapter is needed: players on a software renderer get a message, not a slow game
  (`RUSTPG2D_ALLOW_SOFTWARE=1` lets tests through). The editor's forms and pages are tested
  without one (`egui_kittest` on `Gpu::No` workspaces: no textures, sounds or models); what draws
  (the map view, the game in a tab, the workshops' pictures) is tested through editor script runs
  in the real window and the headless golden picture. The game, the editor's agent server and the
  editor's window run in CI (`scripts/ci/smoke.sh`).
- A game binary older than the engine cannot read new map data. The editor's own Play tab runs the
  game inside the editor and can't differ; the agent server's `play_test` compares
  `rustpg2d-game --engine-build` with its own fingerprint (the engine sources plus the versions of
  the crates that define the file formats) and warns. Every runner also carries the fingerprint as
  `ENGINE_BUILD_TAG` in its bytes, so the exports check runners they can't run (web, other
  platforms) and refuse one built from another engine.
