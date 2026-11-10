# The R(ust)PG2D Creator's Manual

This manual is for everyone who makes games with R(ust)PG2D: what each part of the editor does,
how the game systems work, and how to put them together. It assumes no programming; the scripting
chapter introduces scripts for when you want them.

New here? Read [Getting started](01-getting-started.md) and [The editor](02-the-editor.md), then
open the example project and look at how Emberbrook is built while you read the chapters that
interest you.

## Contents

1. [Getting started](01-getting-started.md): install, the welcome screen, the example project, a new project, a first play-test
2. [The editor](02-the-editor.md): the window, tools and shortcuts, saving, play-testing, project files
3. [Maps](03-maps.md): tilesets and layers, painting, terrains, collision, structures, copy and paste, map settings
4. [Objects and zones](04-objects-and-zones.md): placing props, characters and items, the inspector, zones, map transitions, spawners
5. [The Database](05-database.md): items, props, tilesets, sprite sheets, sounds, quests, recipes, achievements, project settings
6. [Characters](06-characters.md): the character builder, the player and the character creator, behaviour, routines, merchants, companions, portraits
7. [Events](07-events.md): triggers, conditions and actions, cut-scenes, the Events graph
8. [Scripting](08-scripting.md): Rhai scripts, the script editor, live reload and the developer console
9. [Gameplay systems](09-gameplay.md): combat, levels, skills and magic, status effects, inventory, crafting, farming, fishing, lockpicking, furniture, time of day, achievements
10. [Art and sound](10-art-and-sound.md): sprite sheets, the pixel editor, new assets, importing from Tiled, LPC and Aseprite, sounds and music
11. [The asset library](11-asset-library.md): shared assets, the Asset Library window, packages, scripted assets
12. [Languages](12-languages.md): translating a game
13. [AI agents](13-ai-agents.md): letting an AI assistant work in the editor
14. [Publishing](14-publishing.md): exporting for Windows, macOS, Linux and the web, itch.io
15. [The player's guide](15-players-guide.md): controls, menus and options, as your players see them
16. [Troubleshooting](16-troubleshooting.md): common problems and their fixes

## References

These documents list everything in one place; the chapters link to them where it matters.

- [Scripting reference](../scripting.md): every script function, with recipes
- [Events reference](../events.md): every event, condition and action
- [AI agent server](../mcp.md): the tools AI assistants get
- [Autoplay](../autoplay.md): driving the game from a script, for testing
- [Architecture](../architecture.md): how R(ust)PG2D works inside

## Words used in this manual

| Word | Meaning |
|---|---|
| **Project** | A folder with a game: its settings, definitions, maps, scripts, images and sounds |
| **Map** | One area of the game world, a grid of 32 × 32 pixel tiles |
| **Tileset** | An image of tiles the maps are painted with |
| **Sprite sheet** | An image of frames for characters, props, item icons and effects |
| **Definition** | A kind of thing in the Database: an item, a prop, a character, a quest… |
| **Entity** | A definition placed on a map: a chest, a villager, a coin on the ground |
| **Zone** | A rectangle on a map: a spawn point, a portal, a trigger area, a spawner… |
| **Trigger** | *When* something happens, *if* conditions hold, *do* actions |
| **Asset library** | Your collection of assets shared by all projects |
| **Game runner** | The program that plays a game; exported games are a copy of it with the project's data |
