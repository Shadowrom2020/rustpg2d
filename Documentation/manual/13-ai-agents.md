# 13. AI agents

The editor has a built-in **AI agent server**. It lets an AI assistant such as Claude Code, Cursor or
Claude Desktop work in your open project: read and draw maps, place characters, write triggers,
quests and scripts, create art and sounds, translate, and play-test the result. You keep the editor
open and watch; every change appears at once and is one undo step (Ctrl+Z).

## Starting the server

*Assets › AI agent server…* opens its window. Start it there (*Start with the editor* remembers it).
It listens on `http://127.0.0.1:7743/mcp`, on your own computer only, and speaks the **Model Context
Protocol** (MCP) that AI assistants use for tools. The window lists every call the agent makes.

## Connecting an assistant

| Assistant | How |
|---|---|
| **Claude Code** | `claude mcp add --transport http rustpg2d http://127.0.0.1:7743/mcp` |
| **Cursor**, **VS Code** | Add `{"mcpServers": {"rustpg2d": {"url": "http://127.0.0.1:7743/mcp"}}}` to the MCP settings |
| **Claude Desktop** and other assistants that only start programs | `npx mcp-remote http://127.0.0.1:7743/mcp` as the server command |

Then ask in plain words, for example: "Add a fisherman to the village who sells trout in the morning
and fishes at the pond in the afternoon", or "Build a small cave map behind the waterfall with three
bats and a chest".

## What the agent can do

The server offers 54 tools: project settings; maps as text grids with legends, screenshots, tiles,
terrains, collision, entities, zones, triggers, map settings, structures and links between maps;
definitions with validation; writing art and sound files; scripts (read, write, check, the script
API); translations; play-tests that run the game with a script and return what happened and
screenshots; and the asset library (browse, copy into the project, send project assets to the
library). It also gives the agent the scripting, events, autoplay and server references and two prompts, *design_quest* and
*build_map*, with a plan for those tasks.

The editor watches the project's files: what an agent (or any other program) changes on disk is
reloaded when you have no unsaved edits to it; otherwise a banner asks *Reload* or *Keep mine*.

The [AI agent server](../mcp.md) document lists every tool and its conventions, for when you want to
know exactly what an agent can do.
