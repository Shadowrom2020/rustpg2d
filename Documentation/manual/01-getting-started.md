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

### The starter asset library

The ready-made assets (ground materials, props, characters with their 3D models, items, sounds,
music, structures and fonts: everything the example game is made of, and more) are a download of
their own, `rustpg2d-starter-library-X.Y.Z.r2dlib` on the same releases page, about 40 MB. The
editor installs it as a [pack](11-asset-library.md#packs): the Library tab's *Packs and sources*
offers **Download it** (it fetches the file of the editor's own version, checks it against the
release's `SHA256SUMS` and shows what it brings before installing) or **Take the one this editor
has** (one found beside the editor, or a file you downloaded, e.g. for a computer without
internet). The editor reads only a starter library made for its own version line: an editor
refuses one of another format and says which version it is for. Its assets go into your
[asset library](11-asset-library.md).

### Where your things are kept

| | Linux | Windows | macOS |
|---|---|---|---|
| Editor settings, recent projects | `~/.config/rustpg2d` | `%APPDATA%\rustpg2d` | `~/Library/Application Support/rustpg2d` |
| Asset library | `~/.local/share/rustpg2d/library` | `%LOCALAPPDATA%\rustpg2d\library` | `~/Library/Application Support/rustpg2d/library` |
| The installed starter asset library | `~/.local/share/rustpg2d/starter` | `%LOCALAPPDATA%\rustpg2d\starter` | `~/Library/Application Support/rustpg2d/starter` |
| Your projects | wherever you create them | | |

The asset library folder can be moved by setting the environment variable `RUSTPG2D_LIBRARY`; the
starter asset library then lives in a `starter` folder beside it.

## The home screen

Started without a project, the editor shows its home screen (with a project open, the same is the
**Another project** section at the end of the Overview):

- **A new game** makes a project (below).
- **Open a project** by where it is: a project file (`.r2dproject`, *Browse…*) or a project's
  folder (*A folder…*).
- **Worked on lately** reopens one with a click.
- **The example game** (in a packaged R(ust)PG2D) copies the bundled example game to
  `~/rustpg2d/example_project.r2dproject` (Windows: `%USERPROFILE%\rustpg2d\example_project.r2dproject`)
  and opens it. Your copy is yours to change; choosing it again later opens it.

You can also start the editor with a project file (or folder) as its argument.

## Projects are one file

A project is a single file, `<name>.r2dproject`: copy it, mail it or back it up like any document.
While it is open, the editor works on an unpacked copy of it (in `~/.local/share/rustpg2d/work`,
on Windows `%APPDATA%\rustpg2d\work`, on macOS `~/Library/Application Support/rustpg2d/work`) and
writes the project file again whenever something was saved there: within two seconds of a save,
and before the project closes or the editor quits. The file is replaced in one step, so a crash
never leaves half a file, and work saved shortly before a crash is recovered the next time the
project opens.

A project folder is turned into a project file when you open it: the editor writes
`<folder>.r2dproject` beside it and opens that. The folder itself is left as it was; delete it once
you no longer need it. Opening the folder again opens the project file made from it.

If the project file is changed by something else while it is open (another editor, a sync tool),
the editor doesn't overwrite it: it saves your work as `<name> (changed 1).r2dproject` beside it
and says so in the status line.

## The example project: Emberbrook

The example is a complete small game, built with the same editor you have, and the best way to see
how things are done. Open it, press **F5** to play it from its title screen, and then look at how
it's made: select the villagers on the village's map to see their schedules, open *Story › Quests*
and *Story › Scripts* in the Project browser, and a map's **Events graph** to see what its triggers
do.

What you'll find in it:

- **Emberbrook**, an 80 × 60 village that lives by the clock: a market square with three stalls
  whose sellers come and go, the *Copper Kettle* tavern with guest rooms up its stairs, a bakery, a
  smithy, an apothecary, a chapel with a bell tower and a graveyard, a windmill and a mill, fields,
  an orchard, animals and two watchtowers you can climb. Every house can be entered; about twenty
  villagers follow daily routines and go to bed in their own homes at night.
- **Whisperwood**, a forest with giant rats, goblin archers, a bandit camp and a shrine at its heart;
  an **old mine** with ore veins, a pitch-dark treasure room and a dwarf trader, and below it **The
  Deep Caves**, lit only by glowing mushrooms, with an underground lake and a giant wolf spider's lair;
  and **The Wilds**,
  a generated 1024 × 1024 map with fifty groups of enemies (giant rats, goblin archers, wolf packs, bat
  swarms), an orc camp with its chief, an ogre guarding a bridge and an elven herbalist who
  wanders between four spots during the day.
- **Nine quests**: *The Strange Lights* (the main quest from Elder Rowan), *Pest Control* (Tilda's
  rat hunt), *The Lost Cat*, *Bread for the Festival*, *The Herbalist's Remedy*, *Rumours at
  the Tavern*, *Friend of the Elves*, *An Offering for the Shrine* and *A Home in Emberbrook*, which
  makes Halvar's old stone house yours to furnish (furniture from the sawhorse and Tilda's shop).
- The SRD 5.1 rules (ability checks: a locked chest, the shrine's carvings, haggling with Tilda),
  spells (among them a scripted Fireball), crafting, cooking, blacksmithing, mining, farming,
  fishing, a companion, ten achievements, and a full German translation.

Every asset of the example is also in your asset library, so you can use its art, characters and
structures in your own games.

## A new project

**A new game** on the home screen (or under *Another project* on the Overview):

| Setting | What it does |
|---|---|
| **Its name** | The game's name, shown in its window title. The project file's name follows it (e.g. `knight-tales.r2dproject`). |
| **Kept in** | The folder the project file is made in (*Browse…*); `~/rustpg2d` at first. |
| **Start with the basics from the asset library** | Ground to paint with (grass, meadow, dirt, sand, water, cobblestone, flagstones, wooden floor), the *Forest* and *Rocks* scatter sets, a stone wall and a wooden fence, a hero who is the player's character, a few items (a coin, an apple, a potion, a sword, a key) and props (a chest, a sign, a barrel, a well, a boulder), and grass on the first map. Unticked, the project starts empty. |

**Make it** makes the project and opens it, ready to play with **F5**: a hero on grass. What the
0.35 wizard also asked (the player character's sex, whether players create their character, the
pixel scale, the inventory's size, the first map's size) are settings: the *Game* and *Hero &
party* pages, and the map's *Size* in its Inspector.

## Your first ten minutes

A small walk through the most important tools. Each step links to the chapter with the details.

1. **Paint the ground.** Open the first map (*World › Maps* in the Project browser). Choose
   **Terrain** (P) in the map's bar, pick *Dirt* in the panel on the left, switch to *Line* and
   drag across the grass: a path with natural edges. Choose **Place** (N), *Scatter sets*, the
   *Forest* set, and brush a wood beside it. [Maps](03-maps.md)
2. **Place a house.** With **Place**, pick *Structures* and a house (the Library tab brings more),
   and click on the map. [Maps › Structures](03-maps.md#structures)
3. **Add a villager.** With **Place**, pick *Characters* and a villager, and click on the map. Esc
   or the right button stops placing. [Objects and zones](04-objects-and-zones.md)
4. **Make it talk.** With the **Select** tool (V), click the villager. In the Inspector on the
   right, under *Events*: **+ Add an event…** › *When the player uses it or talks to it*, then
   **+ Add an action…** › *Dialog*, text `Welcome, {player}!`. [Events](07-events.md)
5. **Play.** Press **F6** to play on this map where the view looks (or *Try it* in the Inspector),
   walk to the villager and press E. The game runs in the Play tab.
6. **Give the player something.** Add a second action to the event: *Give item*, `coin`, 10, and
   tick *once* so it happens only the first time.
7. **Save.** **Ctrl+S** saves everything.

From here, read [The editor](02-the-editor.md) for the whole window, then whatever your game needs.

## Older projects

This editor opens projects of R(ust)PG2D 0.35 and later. A project of an earlier version is refused
with a message: open and save it with R(ust)PG2D 0.35 first, which converts it (16-pixel tiles,
tile maps, the skill tree of 0.27, each with a backup beside the project).
