# 1. Getting started

## Installing

Download the editor for your system from the
[releases page](https://github.com/Shadowrom2020/rustpg2d/releases). Every download contains the
editor, the game runners for all platforms (so you can export games for every system from any of
them) and the example project.

| System | File | How to start it |
|---|---|---|
| **Windows** 10 / 11, 64-bit | `rustpg2d-X.Y.Z-windows-x86_64.zip` | Unzip it anywhere and start `rustpg2d-editor.exe`. The program isn't code-signed, so the first start shows *Windows protected your PC*: choose *More info*, then *Run anyway*. |
| **macOS** 11 or newer (Apple Silicon and Intel) | `rustpg2d-X.Y.Z-macos.zip` | Unzip it, drag *R(ust)PG2D* into *Applications* and open it. The app is signed ad hoc but not notarized by Apple: the first start needs *System Settings › Privacy & Security › Open Anyway*, or `xattr -dr com.apple.quarantine "/Applications/R(ust)PG2D.app"` in Terminal. Shortcuts this manual writes with *Ctrl* use *Cmd* on a Mac; the editor shows them that way. |
| **Linux** x86_64 with glibc 2.34 or newer | `rustpg2d-X.Y.Z-x86_64.AppImage` | `chmod +x` the file and start it. Without FUSE, start it with `--appimage-extract-and-run`. |
| | `rustpg2d-X.Y.Z.flatpak` | `flatpak install --user rustpg2d-X.Y.Z.flatpak`, then start *R(ust)PG2D* from the app menu or with `flatpak run io.github.Shadowrom2020.rustpg2d`. |

R(ust)PG2D needs a graphics driver with OpenGL 3. In a virtual machine, turn on 3D acceleration.
To build it yourself instead, see *Building from source* in the [README](../../README.md).

### Where your things are kept

| | Linux | Windows | macOS |
|---|---|---|---|
| Editor settings, recent projects | `~/.config/rustpg2d` | `%APPDATA%\rustpg2d` | `~/Library/Application Support/rustpg2d` |
| Asset library | `~/.local/share/rustpg2d/library` | `%LOCALAPPDATA%\rustpg2d\library` | `~/Library/Application Support/rustpg2d` |
| Your projects | wherever you create them | | |

The asset library folder can be moved by setting the environment variable `RUSTPG2D_LIBRARY`.

## The welcome screen

Started without a project, the editor shows its welcome screen:

- **New project…** starts the new-project wizard (below).
- **Open project…** opens a project folder.
- **Recent projects** reopens one with a click.
- **Example project** copies the bundled example game to `~/rustpg2d/example_project`
  (Windows: `%USERPROFILE%\rustpg2d\example_project`) and opens it. Your copy is yours to change;
  choosing *Example project* again later opens it.

You can also start the editor with a project folder as its argument:
`rustpg2d-editor path/to/project`.

## The example project: Emberbrook

The example is a complete small game, built with the same editor you have, and the best way to see
how things are done. Open it, press **F6** to play from the start, and then look at how it's made:
select the villagers to see their schedules, open *Database › Quests* and *Database › Scripts*, and
switch to the *Events* view to see what each map's triggers do.

What you'll find in it:

- **Emberbrook**, an 80 × 60 village that lives by the clock: a market square with three stalls
  whose sellers come and go, the *Copper Kettle* tavern with rooms for paying guests, a bakery, a
  smithy, an apothecary, a chapel with a bell tower and a graveyard, a windmill and a mill, fields,
  an orchard, animals and two watchtowers you can climb. About twenty villagers follow daily
  routines and go to bed at night.
- **Whisperwood**, a forest with slimes, goblin archers, a bandit camp and a shrine at its heart;
  an **old mine** with ore veins, a pitch-dark treasure room and a dwarf trader; and **The Wilds**,
  a generated 1024 × 1024 map with fifty enemy spawners (slimes, goblin archers, wolf packs, bat
  swarms), an orc camp with its chief, a troll guarding a bridge and an elven herbalist who
  wanders between four spots during the day.
- **Six quests**: *The Strange Lights* (the main quest from Elder Rowan), *Pest Control* (Tilda's
  slime hunt), *The Lost Cat*, *Bread for the Festival*, *The Herbalist's Remedy* and *Rumours at
  the Tavern*.
- Skills, spells (among them a scripted Fireball), crafting, cooking, blacksmithing, mining,
  farming, fishing, lockpicking, a companion, ten achievements, and a full German translation.

Every asset of the example is also in your asset library, so you can use its art, characters and
structures in your own games.

## A new project

**File › New project…** (or *New project…* on the welcome screen) opens the wizard:

| Setting | What it does |
|---|---|
| **Game name** | The game's name, shown in its window title. The project folder name follows it (e.g. `knight-tales`) and can be changed. |
| **Where** | The folder the project is created in (*Browse…*). It must be empty. |
| **Player character's sex** | *Player chooses*, *Always male* or *Always female*. |
| **Character creator** | Whether players create their character (name and look) when a new game starts, and the default name. |
| **Starter assets** | Copies a starter set from your asset library: a grass tileset, the hero and heroine figures (with skin, hair and clothing colours), a coin, an apple, a potion, a sword, a key, a chest, a sign, a barrel, a well and a boulder. |
| **First map** | Name and size of the first map. It is filled with grass and has a start point in the middle. |
| **Pixel scale** | How large the game draws its pixels (1.5 means each pixel of art is 1.5 screen pixels). Tiles are always 32 × 32 pixels of art. |
| **Inventory slots** | How many stacks the player can carry. |

The project opens right away, ready to play with **F5**. Everything can be changed later in
*Database › Project*.

## Your first ten minutes

A small walk through the most important tools. Each step links to the chapter with the details.

1. **Paint the ground.** Pick a tile in the *Tileset* palette on the left and paint with the left
   mouse button. Choose **Terrain** (A) in the ribbon and pick *Dirt path* to paint a path whose
   edges blend into the grass by themselves. [Maps](03-maps.md)
2. **Place a house.** Choose **Structures** (H), pick a house from the palette (*Add from library…*
   brings more), and click on the map. [Maps › Structures](03-maps.md#structures)
3. **Add a villager.** Choose **Entities** (N), switch the palette to *Character*, pick a villager
   and click on the map. Right-click to stop placing. [Objects and zones](04-objects-and-zones.md)
4. **Make it talk.** With the **Select** tool (V), click the villager. In the inspector on the
   right, add a trigger: *On Interact*, action *Dialog*, text `Welcome, {player}!`.
   [Events](07-events.md)
5. **Play.** Press **F5** to play from where the camera looks, walk to the villager and press E.
   Close the game window to return to the editor.
6. **Give the player something.** Add a second action to the trigger: *Give item*, `coin`, 10, and
   tick *once* so it happens only the first time.
7. **Save.** **Ctrl+S** saves the map and the definitions.

From here, read [The editor](02-the-editor.md) for the whole window, then whatever your game needs.

## Older projects

Projects made with early versions of R(ust)PG2D used 16-pixel tiles. When the editor opens one, it first makes a full
backup (`<project>-backup-16px`), then converts it to 32-pixel tiles: images that are smaller copies
of library art are replaced by the originals, other images are enlarged without loss, and frame
sizes, hitboxes, speeds, positions and save games are scaled. The game refuses unconverted projects
with a hint to open them in the editor first.
