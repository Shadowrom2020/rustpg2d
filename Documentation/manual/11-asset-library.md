# 11. The asset library

The **asset library** is your own collection of ground materials, scatter sets, sprite sheets,
sounds, items, props, characters, structures, spells and recipes, shared by all your projects. Build something
once, use it in every game.

## What's in it

The library is filled from the **starter asset library**, a download of its own for each version
([Getting started › The starter asset library](01-getting-started.md#the-starter-asset-library)):
when you install it, or the first time the editor opens the library with it installed. The
**starter set** holds all of the example
game's art (the village's buildings and some 60 props, its villagers, children and animals in both
sexes, goods, recipes, sounds and music, structures, its ground materials and scatter sets) plus the
**desert, mountain, city and dungeon** pieces of the former tilesets (materials such as oasis
water, hardpan, frozen lake, canal and plaza; walls, pillars and furniture as props), more characters (hero, elder, wizard
and sorceress, nomads, nobles, bandits, skeletons, elves, dwarves, orcs (and their chief), a troll, a dwarf
trader and an elven herbalist, a wolf, a bat and a ghost; the six races of the example; every humanoid in a male and a
female version), three **realistic** figures in another style than the example's chibi cast (*Hero
(realistic)*, *Elf (realistic)*, *Elf woman (realistic)*, drawn at 115 %) and more structures (town
houses, city walls, dungeon rooms). **Five landscapes** each have their props, ground, scatter sets
and structures: **snow** (snowy pines, snow-capped rocks, ice, frozen bushes, a snowman), the
**desert** (cacti, sandstone rocks and broken pillars, tumbleweed, a skull, dune grass, sandstone
tiles), **caves** (stalagmites, glowing mushrooms and blue crystals that give light, cave rock),
the **swamp** (reeds, lily pads, a dead tree, a fallen log, a mossy stump, bog mushrooms, mud) and
**dungeons** (rubble, broken pillars, a skull pile). The starter library
also includes the SRD's 159 spells that act in play (the example's with their icons; Fireball with its flying sprite
and explosion), a **Locked Chest** (DC 12 to
pick), and
fonts: **monogram** (the example's pixel font: Latin, Cyrillic, Greek) and **Fusion Pixel** in its
Japanese, Korean, Simplified and Traditional Chinese forms (each covers all of Chinese, Japanese and
Korean; the forms differ in how some characters are drawn), and three UI skins: **Wood** (the
example's), **Stone** and **Classic** (the built-in look, to copy) ([Definitions › UI
skins](05-database.md#ui-skins)).

When a newer starter library (installed with a newer editor) brings more assets, the library adds
the ones it doesn't have yet the next time it's opened. Changed assets (redrawn art, new animations,
better values) wait for you:

### Updating the library

The starter library is a **pack** like any other ([Packs](#packs)): the Library tab's *Packs and
sources* offers it while the library doesn't list it as one yet (*Take the one this editor has*, or
*Download it*: the file of this editor's version, checked against the release's checksums), and
afterwards a newer version of it like any newer pack. What a newer version would do is shown before
anything is done:

- **New**: assets your library doesn't have; they are added.
- **Updated**: newer versions of assets you never changed; they are replaced (definition,
  picture or sound, and the script functions they need).
- **Changed by you and in the pack**: assets you edited that the new version changed too. Your
  version stays unless you tick it.
- **No longer in the pack**: assets earlier versions had and this one dropped. Ticked ones are
  deleted; one that other assets in your library still use isn't offered.

Before anything is replaced the whole library folder is copied beside itself
(`<folder>.bak-…`); what was done names the copy. Your projects aren't touched by it: *In this
project* on the same side then lists the project's assets that are older than the library's and
updates them, as one step (what you changed in the project stays unless ticked).

How it tells your edits from older versions: the library keeps what each version of a pack shipped
(`packs/<id>/history.ron`; for the starter library the editor knows every version it ever shipped,
`crates/editor/assets/starter_history.ron`); an asset that matches one of them was never changed by
you.

The library lives in `~/.local/share/rustpg2d/library` (Windows: `%LOCALAPPDATA%\rustpg2d\library`;
macOS: `~/Library/Application Support/rustpg2d/library`). `RUSTPG2D_LIBRARY` moves it. The installed
starter asset library is the file `starter/starter-library.r2dlib` beside it.

## Taking assets into a project

The **Library tab** (*Asset library* under *Game* in the Project browser) shows what the library
holds, by kind with how many of each (*All*, *Characters*, *Items*…), searched, with pictures; what
the project has already is marked (a click opens the project's one). Tick what you want and **Add N
to the project**: they come with **everything they need** (their sprite sheets, ammunition, keys,
sounds, models and script functions), as one step of the history. Assets already in the project
with identical content are reused; an id or file name taken by something different gets a `_2`
suffix, and all references follow.

A single asset also comes in from *New…* (*as one of the asset library*), and a problem about
something missing offers *Take it from the library* when the library has it.

## Putting assets into the library

**Send to library** in a definition's header sends it (with everything it needs) to the library;
**Share…** writes it as a pack file instead ([Packs](#packs)). Library assets are always used as
last saved. To work on the library's own assets, open its folder as a project (*Open a project* ›
*A folder…*).

## Packs

A **pack** (`.r2dpack`) is the one way to share assets with other creators: a file with a title, a
version, who made it, a licence and a credit line, pictures of what it holds, and its assets,
script functions and plugins. The Library tab's **Packs and sources** side:

- **A pack file** (*From a file…*, or dropped on the window) or one from a source is **shown before
  anything is installed**: what it holds, what in it runs (script functions, plugins), how much is
  new to the library, there as it is, or there under the same id but different, and for those the
  choice (replaced by the pack's, kept, or brought in as copies). The packs it needs that aren't
  installed are named.
- **Installed packs** are listed with what they brought and where from.
- **Sources**: a web address or a folder of packs with its index (*Add source*); each pack it offers
  can be installed or updated (downloaded, checked against its size and SHA-256, then shown as
  above).
- **Share…** in a definition's header (or *Packs › Share as a pack* for several) writes them as a pack,
  with what they need, into a folder, and lists it in the folder's `index.ron`: the folder is a
  source as it is, or uploaded anywhere.

An asset taken from a pack remembers it, and the Publishing page lists the packs in the game with
their licences and credit lines, to put into the game's credits ([Publishing](14-publishing.md)).
Asset packages, plugin files and starter library files of 0.35 are read as packs too.

## Plugins

The library also keeps **plugins** (its `plugins/` folder): add-ons a project uses without copying
them in ([Plugins](18-plugins.md)). The *Scripts & plugins* page adds them to a project. The starter library brings four: a calendar,
hard mode, a minimap and damage-number styles ([The starter plugins](18-plugins.md#the-starter-plugins)).
A newer starter set updates the library's copies of them unless you changed one without raising
its version.

## The SRD 5.1 data

The starter library's races (with their subraces, traits and choices), the Acolyte background,
the twelve classes with their subclasses, levels, features and spellcasting ([Characters ›
Classes](06-characters.md#classes); the features that act get their effects from a table in the
import, the starting equipment is the SRD's: the first option of each choice, a focus for casters
that would start without one), the SRD's spells ([Gameplay ›
Spellcasting](09-gameplay.md#spellcasting): attacks, saves, damage and healing from the data,
conditions, boons, light and Knock from a table in the import; the import leaves out the 152
story-only spells (and their mentions in monsters, races, classes and magic items) and Comprehend Languages, Tongues, Detect Poison and Disease, Detect Evil and Good, Animal Friendship, Dream, Plane Shift and Control Water, so the library has the 159 that act in play; a spell the library
had keeps its icon and look, and one it made act stays as it is), the 334 monsters ([Characters ›
Monsters](06-characters.md#monsters); every damage part of an attack, and what a hit does read from its
text: 67 attacks with more damage of other types, 85 effects on a hit in 65 attacks, such as a save
against a condition or poison, or a grapple's escape DC; the 36 casters' spellcasting from the
structured data: caster level, ability, DC, attack bonus, slots and spells, innate ones with their
uses a day) and the 237 items of the equipment list with the five coins
([Gameplay › Equipment and money](09-gameplay.md#equipment-and-money): weapons with their category,
properties, dice and ranges, armor with its armor class, tools, gear, packs that unpack their
contents, mounts and vehicles as plain items; generic icons by kind on the `srd_gear` sheet; library
items that stand for an SRD item, such as the iron sword for the longsword or the chainmail for
chain mail, take its numbers and keep their name and look, and the coin becomes the gold piece), and the
magic items ([Gameplay › Magic items](09-gameplay.md#magic-items): rarity, attunement, charges and their
recharge from the SRD's text, a table in the import for the rules of about half of them, the others story
items; magic weapons and armor are a copy of the library's item for their base, with its look, others get
generic icons by kind on the `srd_magic` sheet; the SRD's generic +1 / +2 / +3 entries are the *Bonus* of any
item, and six ready-made ones come with it: +1 Longsword, Dagger, Shortbow, Arrows, Chain Mail and Shield,
as well as a Silvered Dagger and Silvered Arrows (the SRD's silvering);
spell scrolls are items that *teach* a spell; the library's potion and Elixir of Strength became the Potion of Healing and the Potion of Hill Giant Strength, keeping their ids, look and price) come from the **SRD 5.1** (by Wizards of the Coast LLC, CC BY 4.0), through the data of
[5e-bits/5e-database](https://github.com/5e-bits/5e-database), pinned to a commit. The import is a
tool of the repository; running it again updates what it wrote and keeps what the library added
(models, factions, sizes, colours; a character that already has a monster's id keeps its look,
behaviour, triggers and loot and takes the SRD's stat block, and a character with a monster's stat
block keeps its numbers and gains the SRD's size, type, speeds, senses and texts, and its attacks the
SRD attack's other damage and effects on a hit). Run from the
repository, it also gives the example's characters, classes and equipment the same data (the
example gets the items its classes start with, the coins and the magic items it shows) and writes the SRD's 35 NPC stat
blocks (commoner, guard, bandit, veteran…) into the engine (`crates/engine/src/srd/npcs.ron`): the
templates of the [stat blocks](06-characters.md#stat-blocks), and the SRD's own words for
conditions, damage types, languages, alignments, abilities and skills (`rules.ron`: the editor's
hover texts; tests keep the engine's lists equal to the SRD's). A second run writes the same files.
The starter library is marked *Uses SRD 5.1 content* and carries `SRD-5.1-ATTRIBUTION.txt`
([The SRD 5.1 rules](19-srd-rules.md#the-licence-what-your-game-needs)).

```sh
git clone https://github.com/5e-bits/5e-database /tmp/5e-database
git -C /tmp/5e-database checkout a6212beb4b278917c2cff41c2b04c4ef4f3e0d6f
cargo run -p rustpg2d-editor-core --example srd_import -- /tmp/5e-database [library or project folder]
```

## Scripted assets

Spells with a *Script* effect, and items, props and characters whose triggers run scripts,
need their script functions. *Send to library* and *Share…* take them along: the asset's function
and everything it calls or passes on (`aim("…")`, `fly(…, "…")`, `then_call("…")`), and the sprite
sheets, sounds and items the functions use by id. A pack says what in it runs before it is
installed.

In the library the functions live in `scripts/library.rhai`. Adding such an asset to a project
appends its functions to the project's `scripts/library.rhai`; functions the project already has are
kept as they are. Keep a feature's functions self-contained (helpers they call, no flags of one
particular story) so they work in every game. Story logic, such as quest steps and dialogs, belongs on
placed characters, zones and map triggers, so the definitions themselves stay reusable.
