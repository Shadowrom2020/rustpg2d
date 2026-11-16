# R(ust)PG2D compared with commercial RPG makers

Where R(ust)PG2D 0.23.0 stands against the tools people buy to make RPGs, what it does better, what
it lacks, and what would bring it on par. The facts about other tools describe their usual
out-of-the-box state (many gaps there are closed by paid or community plugins); prices and details
change, so check them before quoting.

**Compared with**

- **RPG Maker MZ** (2020) and **MV** (2015): the classic tile RPG makers (JavaScript runtime,
  HTML5 export). "RPG Maker MX" doesn't exist; MZ is the current classic version.
- **RPG Maker Unite** (2024): the same idea built on Unity.
- **Wolf RPG Editor**: free, Japanese, a very open database and event system.
- **RPG Developer Bakin** (2023): 3D, with Lua / C# scripting.
- General engines used for RPGs: **GDevelop** (events, no code) and **Godot** (with RPG add-ons).

## At a glance

| Area | RPG Maker MZ / MV | R(ust)PG2D 0.23.0 |
|---|---|---|
| Genre focus | Classic JRPG: turn-based battles, a party | Action RPG / life sim: real-time combat, a living village |
| Maps | 48 px tiles, autotiles, layers, region IDs | Painted ground with blended edges, free decorations, structures, maps up to 1024 × 1024 |
| Movement | 4 / 8 directions on a grid | Free pixel movement, A* pathfinding |
| Battles | Front- or side-view turn-based; troops, battle events, Effekseer animations | Real-time melee and ranged combat, blocking, dodging, AI that hunts and flees, companions |
| Party | Up to 4 (or more) actors with classes, levels and gear each | One hero plus companions with their own gear and orders |
| Database | Actors, classes, skills, items, weapons, armour, enemies, troops, states, animations, tilesets, common events, system, types, terms | Items, props, characters, skills, spells, recipes, quests, races, factions, achievements, materials, models, scatter sets |
| Events | About 100 event commands, self switches, move routes, common events | About 75 actions, triggers and conditions, plus Rhai scripting with completion and checks |
| Extending | Huge JavaScript plugin ecosystem (battle systems, menus, HUD) | Rhai scripts for game logic; no way to change UI or systems without Rust |
| Art | Large built-in RTP, character generator, paid DLC packs | Starter library, 3D characters drawn as pixel art, AI art generation with Meshy |
| Text and fonts | TTF fonts, Unicode, Japanese and Chinese; message codes for colour, icons and speed | One pixel font with Latin-1 only; placeholders `{player}` / `{var}` |
| Localization | None built in (plugins) | Built in: translation templates, a language menu, German shipped |
| Platforms | Windows, macOS, web; Android / iOS through the web build | Windows, macOS, Linux (AppImage, Flatpak), web |
| Store integration | Steam achievements and the overlay through plugins | None |
| Price / licence | Commercial licence; the runtime ships with games | Free, MIT, open source |
| Project files | JSON in folders | One project file, or RON / text files in git |

## Where R(ust)PG2D is already ahead

These are worth protecting and putting in front:

- **A living world out of the box.** Daily schedules across maps, villagers who sit, sleep, work at
  their spot and visit each other, day and night, weather, dynamic lights. In RPG Maker this is
  plugin work.
- **Systems that are plugins elsewhere:** crafting, cooking and blacksmithing, gathering and mining,
  farming, fishing, lockpicking, sneaking, companions with orders, factions, friendship, housing and
  furniture, achievements, photo mode.
- **The quest designer.** Giver, offer dialog, steps (talk, reach, conditions), rewards and quest
  objects in one form, with markers that follow the quest logic. RPG Maker has only switches and
  variables.
- **Large painted worlds.** Blended terrain, free decoration placement and structures. Maps up to
  1024 × 1024 run fast in Rust.
- **Tools for checking a project:** a Problems list, the Events graph, find usages, renames that
  update every reference, Database undo, live reload and a developer console.
- **Built-in localization and accessibility:** caption, text size, high contrast and hold-to-repeat
  options.
- **The AI agent server.** Assistants build maps, quests and scripts in the running editor and
  play-test them. No commercial maker has this.
- **Open source, free and Linux-native**, with text formats that work with git.

## Gaps, by importance

### 1. Text, fonts and languages (blocks whole markets)

Games are limited to Latin-1 in one pixel font. RPG Maker's biggest audiences (Japan, China, Korea,
Russia, Brazil) need full Unicode.

- **Project fonts:** TTF / OTF files, a pixel font and a smooth one, size per use (dialog, menus,
  HUD), with a fallback chain for CJK. The editor (egui) can render Unicode; the game runner needs
  a font atlas built from the project's fonts.
- **Message codes** in dialogs: `\c[red]…`, inline item and skill icons, a wait, the text speed, a
  name box, and the dialog box at the top, middle or bottom.
- Right-to-left text can come later.

### 2. Make the game's look and menus customizable

RPG Maker games look alike, but plugins can restyle everything. R(ust)PG2D's menus, HUD, title
screen and windows are drawn by the engine with fixed layouts.

- **A UI skin:** a window frame image (nine-slice), colours, cursor, button style and title screen
  background per project. That covers most "make it look like my game" requests without code.
- **A title screen and menu designer:** which entries, their order, a logo, music, and custom menu
  pages driven by scripts.
- **Script-drawn UI:** a small Rhai API for windows, text, images, bars and buttons, so creators can
  build their own HUD elements, minigames and menus.

### 3. Turn-based battles and a party (the JRPG expectation)

Most RPG Maker users come for front- or side-view turn-based battles with a party. R(ust)PG2D's
real-time combat is a strength, but many creators will look for the classic mode first.

- **Party members** as a step up from companions: up to four actors with their own levels, stats,
  skills and gear, swapped in a party menu.
- **A turn-based battle mode** (optional, per encounter): a battle screen with the map behind it,
  troops of enemies, an action menu (attack, skill, item, guard, flee), turn order by speed, battle
  events, and victory / defeat outcomes. Start with front view, which RPG Maker has as its default;
  side view comes later, using the 3D models' existing animations.
- **Classes and parameter curves:** stats per level from a curve, and a trait system (element
  rates, state resistance, stat boosts) shared by classes, gear and states.
- **Damage formulas** as expressions (`a.atk * 4 - b.def * 2`) with elements and critical hits.

### 4. Extensibility: a plugin model

RPG Maker's real moat is its plugin ecosystem. Rhai covers logic, but nobody can add a new system,
UI or battle style without forking the engine.

- **Script hooks** on engine events: damage dealt, an item used, a menu opened, a save written. A
  script can change the outcome.
- **Plugin packages:** a folder or `.r2dpack` with scripts, assets, parameters shown as a form in
  the editor, and a version. The asset library already moves bundles, so this is a step from there.
- **A stable script API with versioning**, so plugins keep working across releases.

### 5. Event commands creators expect

Most of RPG Maker's commands exist, but some everyday ones are missing or only reachable from
scripts:

- **Pictures:** several numbered layers, move / fade / rotate with easing, and tinting.
- **The screen:** tint, flash, fade out / in as actions (shake and dim exist).
- **Timers:** a countdown shown on screen that fires an event.
- **Input:** a number input and a name input outside the character creator.
- **Move routes:** a visual list of steps (move, turn, wait, jump, change speed or graphic) per
  character, instead of Patrol / Walk to tile alone.
- **Balloon icons** over heads exist as emotions; scrolling text (credits) and a map scroll for
  cut-scenes are missing.
- **A visual cut-scene timeline:** cut-scenes are script functions today; a track view (who moves
  where, what is said, camera) would make them approachable.
- **Common events:** reusable action lists picked in the editor, today only via script functions.

### 6. Content and assets

RPG Maker ships thousands of tiles, characters, faces, battlers, animations and music, and sells
more.

- **More starter content:** material sets for snow, desert, dungeon and cave, interior props, monsters
  and animals with animations, portraits, more music and sound effects.
- **A battle / skill animation editor** (particles or sprite sequences with timing, sound and
  flash): today effects are single sprite sheets.
- **Generator presets:** RPG Maker's character generator has hairstyles and outfits. The colour
  features cover colours; swappable parts (hair, clothing models) would cover shapes.
- An asset marketplace or community library index (even a curated GitHub list).

### 7. Platforms and stores

- **Mobile:** touch controls (a virtual stick, tap to walk and talk, which already exists for the
  mouse), a portrait-safe UI, and Android / iOS exports. The web build is a path to mobile browsers
  first.
- **Steam:** achievements, cloud saves and rich presence through Steamworks. The engine's
  achievements already map one to one. Steam Deck already works through Linux.
- **Signing:** code-signed Windows builds and notarized macOS builds; unsigned downloads scare
  players (already in the backlog).
- **Consoles** are out of reach without publisher programmes; not a near-term goal.

### 8. Maturity and trust

- **A 1.0 with a stability promise:** projects, saves and the script API stay compatible across 1.x
  releases. 0.21 broke projects, and creators planning a commercial game need that promise.
- **A learning path:** video or step-by-step tutorials ("your first quest", "your first dungeon"),
  small sample projects besides Emberbrook, and an FAQ for people coming from RPG Maker (switches →
  flags, variables, common events → script functions, plugins → scripts).
- **An importer** for RPG Maker MV / MZ projects (maps → tiles converted by the existing tile
  converter, the database → items, characters and skills, events → triggers where they map). It
  would let people switch with their work.
- **A community place** (Discord or Discussions), a showcase of games, and a bug tracker template;
  some exist on GitHub.

## Smaller things RPG Maker users will look for

- Vehicles (a boat, a ship, an airship over the world map); mounts are already in the backlog.
- Random encounters by region (spawners exist; region-based encounter tables are the RPG Maker way).
- A world-map mode with a smaller character and a different scale.
- Music effects (a fanfare that pauses the background music), and pitch / pan per sound.
- A test-play from any map at any point with chosen party, items and switches (`--map` /
  `--spawn` and crafted saves exist, but not in the editor UI).
- Database bulk editing (a table view to change many items' prices at once).
- Gamepad remapping in the editor's preview, and a shortcut sheet.

## A suggested order

Ordered by how many creators each step unlocks, against its size:

1. **Fonts and Unicode** (TTF, CJK) with dialog message codes.
2. **UI skins and a title / menu designer.**
3. **Event command gaps:** picture layers with easing, screen tint / flash / fade, timers, number
   input, move routes.
4. **Script hooks and plugin packages** with a versioned API.
5. **Party members, classes with curves and traits.**
6. **An optional turn-based battle mode** (front view first).
7. **Steamworks** (achievements, cloud saves) and signed builds.
8. **Touch controls** and a mobile web build, then native mobile exports.
9. **A 1.0 stability promise,** tutorials and an RPG Maker importer.
10. **More content:** material sets, monsters, music, and a skill animation editor.

Steps 1 to 4 make R(ust)PG2D a match for RPG Maker for anyone making an action or life-sim RPG, which
is where it is already stronger. Steps 5 and 6 bring in the classic JRPG crowd. Steps 7 to 9 make
it a tool people can bet a commercial release on.
