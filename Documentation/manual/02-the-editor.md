# 2. The editor

![The editor with the village map open](../images/editor-map.webp)

## The window

From top to bottom:

- **The menu bar** groups commands by task: *File* (projects, save all, export, import, quit),
  *Edit* (undo, redo, copy, cut, paste), *Map* (new, open, save, fit), *Assets* (new asset, add from
  library, import a package, the Asset Library window, save and reload definitions), *View* (grid,
  overlays, previews) and *Play*.
- **The mode tabs** next to it switch the main area: **Map** (painting and placing),
  **Events** (the map's triggers as a graph, see [Events](07-events.md#the-events-graph)),
  **Nodes** (visual scripts: script functions built from nodes, see [Visual scripts](20-visual-scripts.md)),
  **Database** (all definitions, see [The Database](05-database.md)), **Pixel editor** (see
  [Art and sound](10-art-and-sound.md#the-pixel-editor)) and **Asset Library**, which opens the
  library in its own window (see [The asset library](11-asset-library.md)). The **map dropdown**
  shows the open map and switches to another one.
- **The ribbon** holds the tools as icon buttons in groups: *Edit* (undo, redo) · *Select* ·
  *Paint* (ground) · *Collision* · *Objects* (decorations, entities, zones, transitions,
  structures) · *View* (grid, collision overlay, zones, minimap, lighting preview, weather preview,
  parallax layers, top-down, fit) · *Play*. Hovering a button shows its
  name, its shortcut and a one-line how-to.
- **The left panel** shows the active tool's options (the materials, the decorations' props and
  scatter sets, the entity or structure palette).
- **The map** in the middle, with a **minimap** in the corner. It shows the map **isometric**, as
  the game does: the ground as diamonds, everything standing upright at its feet, and the tool
  previews, zones, selections and the grid on the diamonds. **View › Top-down** (I) shows it from
  straight above instead, e.g. to paint a large area; the game always shows it isometric.
- **The right panel** is the **inspector**: the selected entity's or zone's settings, or the map's
  own settings when nothing is selected.
- **The status bar** shows the tile under the mouse, the zoom, the map's size and counts, and the
  active tool's how-to.

## Tools and shortcuts

| Key | Tool |
|---|---|
| V | **Select** (the default): select, move and copy things |
| P | **Ground**: paint the ground with materials (brush, line, fill; [ and ] change the size) |
| C | **Collision**: make tiles solid or walkable |
| N | **Entities**: place props, characters and items |
| Z | **Zones**: draw spawn points, portals, trigger areas, spawners |
| T | **Transition**: link two maps with a two-way exit |
| H | **Structures**: place ready-made houses, walls, rooms |
| O | **Decorations**: trees, rocks, walls and fences placed freely; scatter brush, lines ([Maps › Decorations](03-maps.md#decorations)) |
| B | **Build**: walls along tile edges, rooms and houses dragged as rectangles ([Maps › Building walls, rooms and houses](03-maps.md#building-walls-rooms-and-houses)) |

| Input | Action |
|---|---|
| Mouse wheel, middle-drag, Space+drag, arrow keys | Zoom and pan (the arrow keys move along the screen) |
| I | Isometric / top-down view |
| R | Turn the selected prop, or the decoration about to be placed, a quarter (props with a 3D model or pictures from four sides) |
| Right-click (Ground, Decorations) | Pick the material or prop under the cursor |
| G / Home | Toggle the grid / fit the map into the view |
| Ctrl+Z / Ctrl+Y | Undo / redo (in the Database mode: definition changes) |
| Ctrl+S | Save the map and the definitions |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / cut the selected region or entity / paste |
| Ctrl+D / Del | Duplicate / delete the selected entity or zone; Del on a region clears it |
| Esc | Deselect; stop placing or pasting; otherwise back to the Select tool |
| Ctrl+P | Quick search: maps, definitions, script functions, named characters, zones, dialog lines ([Finding things](#finding-things)) |
| Ctrl+M | Go to a map (searchable) |
| Ctrl+PgUp / Ctrl+PgDn | Previous / next map |
| F5 | Play-test from the camera position (or from the selected spawn point) |
| F6 | Play-test from the project's start |

Every map remembers where you were looking when you switch away and back.

## Saving and undo

**Ctrl+S** saves the open map and the definitions; *File › Save all* saves everything. A dot next
to the map's name (•), *defs •* or *• edited* in the pixel editor marks unsaved changes; undoing
back to the saved state makes them clean again.

Every edit on a map is one undo step (Ctrl+Z, Ctrl+Y), including placing a structure, pasting a
region and changes made by an AI agent. Renaming a definition saves at once, because it rewrites
every file that refers to it.

**Definitions have their own undo.** In the Database mode Ctrl+Z / Ctrl+Y, the **Undo** / **Redo**
buttons in its toolbar and *Edit › Undo (definitions)* step through the changes to definitions and
project settings: form edits, *+ New*, *Duplicate*, *Delete*, characters made in the character
builder and definitions an AI agent wrote. What you type into one field is one step (until you
pause for a moment or move to another field). Undoing a **rename** renames back, in the maps and
scripts too. A text field you are typing in keeps its own Ctrl+Z for its text. The last 100 steps
are kept until you open another project or the definition files are changed by another program.

## Finding things

**Quick search** (**Ctrl+P**, or *Edit › Quick search…*) opens a search box at the top of the
editor. It finds maps, definitions of every kind (by id and name), script functions, the named
characters and objects and the zones of every map, and the lines of dialogs and choices in
triggers. Letters only need to come in order (`hpot` finds *health_potion*, `shrine awa` finds
`shrine_awakening()`); exact ids and names come first. Arrow keys and **Enter** (or a click) go
there: a map opens, a definition opens in the Database, a function opens in the script editor at
its line, a character or zone is selected and centred on its map, a dialog line selects whoever
says it. **Esc** closes the box.

**Find usages** lists every place a definition is used: the **Usages** button above the
Database list, or right-click a name in the list › *Find usages*. The *Usages* window shows the
placed copies and spawners on every map, the triggers that name it (on maps, zones, objects and
definitions), other definitions that use it (shops, loot, recipes, equipment), script lines and
the project settings; click one to go there, *Refresh* after changes. Use it before deleting or
changing a definition.

**Problems** (*Edit › Problems…*, or click the error and warning counts in the status bar) lists
everything broken in the project in one window:

- **errors** (something is missing or can't work): definitions that refer to ids that don't exist
  or whose image or sound file is missing, characters without a body; placed objects whose
  definition is gone, triggers and loot naming unknown ids or script functions that don't exist,
  portals and transitions to missing maps, spawns or zones, spawners of unknown characters;
  unknown ids and compile errors in scripts; entries a map or definitions file had to leave out
  when it was loaded (they are lost when the file is saved again);
- **warnings** (probably not what you meant): maps no portal, transition, map edge, trigger or
  script leads to from the start map, quests without stages, script functions defined twice,
  untranslated texts and leftover translations per language, and the project check of *Export
  game* (start spawn, player character, languages without a file).

The buttons at the top show or hide errors and warnings, the list next to them picks one area
(project, definitions, maps, scripts, translations). Click an entry to go there: a definition opens
in the Database, an object or zone is selected and centred on its map, a script opens at the line.
The list is checked again whenever something is saved (in the editor, by an AI agent, or a file
that changed on disk) and with *Refresh*.

*File › New project*, *Open project* and *Close project* first ask to save or discard unsaved map,
definition and pixel-editor changes; *Cancel* keeps everything as it is.

## Play-testing

**F5** starts the game from where the camera looks (or from the selected spawn point), **F6** from
the project's start. Play-tests skip the title screen and the character creator and use the
default player name. Close the game window to return to the editor.

### Test-play from here

![The right-click menu and the Test setup window](../images/editor-test-setup.webp)

**Right-click on the map** (with the Select tool) for a menu at that tile:

- **Play from here**: a new game on this tile.
- **Play from here with the test setup**: on this tile with what the test setup says, to test a
  late scene without playing up to it.
- **Test setup…** opens the window.

The **test setup** holds the hero's **level** and **class**, **items** (into the bag), **flags**,
**variables**, **quests** at a stage (or done), the **time** (hour and day; off = the project's
start hour) and the **party**: placed characters by name, on any map, who join as companions.
It is kept per project in the editor's settings (not in the project). The editor writes it as
a save game (in its own settings folder, not the project's `saves/`) and starts the game with
it, so the map's *Map enter* triggers don't run, as when a game is loaded.

Play-tests run in developer mode:

- **Scripts reload live**: saving a script in the editor reloads it in the running game within
  half a second. A script with an error keeps the previous version running and shows the error.
- **F4 opens the developer console**: give items, set flags and variables, jump in time, teleport,
  call script functions and run any line of script. See
  [Scripting › The developer console](08-scripting.md#trying-scripts-while-the-game-runs).
- **F3** shows the debug overlay (collision, zones, map edges, hitboxes).
- *Play-test in* (*Database › Project › Languages*) starts play-tests in another language.

Exported games never have the console.

## Files and folders

*File › New project…*, *Open project…* and *Open project folder…* use your desktop's file dialogs.
Fields that take an image or a sound have a *Browse…* button; a file from outside the project is
copied into the project's `materials/`, `sprites/` or `sounds/` folder.

A project is one file, `<name>.r2dproject` (see [Getting started › Projects are one
file](01-getting-started.md#projects-are-one-file)). *Database › Project* shows where it is and
where its working copy is: the files the editor works on while the project is open, laid out like
this. You rarely need to touch them directly; the editor writes them, and the project file follows
within two seconds.

```
project.ron           the game's name, start map and spawn, pixel scale, player character and settings
defs/                 the definitions: items, props, characters, materials, scatter sets, sprite
                      sheets, sounds, fonts, quests, spells, recipes, achievements,
                      structures (text files)
maps/<name>.map       the maps
scripts/*.rhai        the scripts (all files share one namespace)
sprites/, materials/  the images (tilesets/: pictures of tiles from before 0.21, used as sheets)
sounds/               WAV and Ogg Vorbis files
fonts/                TTF / OTF fonts and their licence texts
lang/<code>.ron       translations
saves/                save games of play-tests (never exported)
```

All text files are plain RON. To keep a project in version control as files, develop it as a
folder: with the environment variable `RUSTPG2D_FOLDER_PROJECTS=1` the editor opens project
folders as they are instead of converting them (that is how the example game is kept in
R(ust)PG2D's own repository).

**Repository mode** (for working on R(ust)PG2D itself): the editor started with a checkout of the
R(ust)PG2D repository as its current folder (e.g. `cargo run -p rustpg2d-editor` in the
repository) opens the repository's `example_project/` directly, as a folder, and uses the starter
library (`crates/editor/assets/starter_library/`) as its asset library: *Send to library*, the
Asset Library window and the agent server's `library_send` change the repository's files. A
**Repository** badge at the left of the status bar says so (its tooltip names the project and the
library). A project given on the command line opens instead of the example; `RUSTPG2D_LIBRARY`
still picks another library, and `RUSTPG2D_REPO=0` turns the mode off. The starter library isn't
filled from a starter set in this mode (it is the starter set), so assets deleted there stay
deleted; after changing it, run `cargo run -p rustpg2d-editor --example starter_history`. Releases
publish it as `rustpg2d-starter-library-X.Y.Z.r2dlib` (`cargo xtask-dist` writes it, or
`cargo run -p xtask --features dist -- starter-pack DIR` alone).

### Cleaning up unused files

Models, pictures and sounds you add and never use make the project bigger. **Assets › Clean up
unused files…** lists what the project has but doesn't use:

- **3D models, sprite sheets, sounds, ground materials and fonts** whose id nothing names: no map,
  character, item, prop, race, quest, cut-scene or other definition, no project setting, and no
  script or plugin file (an id that appears anywhere in a script's text counts as used, also inside
  a longer string). A used model's rig template and a used material's footstep sound count as
  used. So do the bodies the character creator offers, pieces (hair, outfits) that fit a used
  body, the mannequin and *Emotions* sheets.
- **Stray files** in `sprites/`, `models/`, `sounds/`, `materials/`, `fonts/` and `tilesets/` that
  no definition points to (pictures, models, sounds and fonts only: a folder's credits stay).

Each entry shows its size. Tick what should go and press **Delete ticked**: a definition goes with
its files, unless something used shares them. Everything deleted is copied first into a folder beside
the project, `<name>-backup-cleanup` (then `-2`, `-3`…), with the definition files as they were;
copy files back from there to undo.

When you **close the project or the editor**, the same window opens by itself if the project has
unused assets: *Delete ticked and close*, *Keep all and close*, or *Cancel* to stay. What you keep
(untick, or *Keep all*) is remembered for the project and not offered again when it closes; the
window from the Assets menu still lists it (as *kept before*). Rig templates (`rig_…` models) are
editor tools: they are never offered for deletion.

Exported games leave unused assets out whether or not you clean up (see
[Publishing](14-publishing.md#what-you-get)).

### Files changed outside the editor

The editor watches the project's files (in a project file's working copy). When something else
changes them (another program, an AI agent, a `git pull` in a folder project), it reloads them by itself if you have no unsaved edits to them; otherwise a
banner asks whether to *Reload* or *Keep mine*.

### Files the editor can only read in part

A definition entry or a map object that can't be read (a typo from editing a file by hand, or
something written by a newer version) is left out instead of stopping the whole project from
loading. A banner at the top says which entry it was, and the file is backed up next to itself
(`items.ron.bak`, `village.map.bak`) before the editor could save it without that entry. The game
logs the same and runs with the rest. Fix the entry in the backup or the original and reload.
