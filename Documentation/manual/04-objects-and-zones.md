# 4. Objects and zones

## Entities: props, characters and items on a map

Everything that isn't a tile is an **entity**: a placed copy of a definition from the Database.

- **Props**: chests, doors and gates, trees, lamps, furniture, crafting stations, farm plots, ore
  veins. Anything that stands around and may be used.
- **Characters**: villagers, merchants, enemies, animals.
- **Items**: things lying on the ground to be picked up. Pressing E on one picks it up.

### Placing

1. Choose **Entities** (N).
2. In the palette on the left, pick a definition. It has a search box (name or id), the tabs
   *All / Prop / Character / Item*, a *Recent* row of the last ones you used, and `[` / `]` (or
   PgUp / PgDn) to step to the previous / next definition. *Add from library…* brings more from your
   asset library.
3. Click empty space on the map. Entities snap to the tile; hold **Shift** for free placement. Big
   props snap by their footprint (props an even number of tiles wide stand on a tile edge), and a
   translucent ghost previews where they'll go.
4. **Right-click or Esc stops placing.** With nothing armed, clicks only select and move.

*On this map* below the palette lists the placed entities (filtered by the same search); click one
to select it and jump there.

### Selecting and moving

With the **Select** tool (V), click an entity to select it (it's found by its sprite or by the tile
it stands on) and drag it to move it. Clicking again on the same spot cycles through objects stacked
there. Empty space or a right-click deselects. **Ctrl+D** duplicates the selection, **Del** deletes
it, and *Place more like this* in the inspector arms its definition for placing.

**Several objects at once:** **Ctrl+click** adds an entity or zone to the selection (or takes it out
again), **Ctrl+drag** on empty space selects every entity standing in the box and every zone lying
completely inside it. Dragging one of the selected objects moves them all together by whole tiles
(the group stops at the map's edge), **Del** deletes them, **Ctrl+D** duplicates them one tile to
the right, **Ctrl+C** / **Ctrl+X** copy or cut the selected entities (zones aren't copied) for
**Ctrl+V**. Each of these is one undo step. The inspector then lists the selected objects (click one
to select only it) and switches *Active* for all of them. A plain drag on empty space still selects
a region of tiles ([Maps › Copying, cutting and pasting regions](03-maps.md#copying-cutting-and-pasting-regions)).

### The inspector

The right panel shows the selected entity's settings:

| Setting | What it's for |
|---|---|
| **Name** | An instance name like `guard` or `cellar_door`. Actions and scripts address entities by this name, so give one to everything a trigger should talk to. |
| **Definition** | What it is; it can be switched to another definition of the same kind. |
| **Position**, **Facing** | Where it stands (feet, in pixels) and which way it looks. |
| **Active / visible at start** | Hidden entities can be shown later with the *Set active* action, e.g. a reward that appears. |
| **State** (props) | The prop's starting state, e.g. a gate that starts *open*. |
| **Triggers** | This entity's own triggers. They run *in addition to* its definition's triggers. See [Events](07-events.md). |
| **Light** | A light of its own instead of its definition's. |

Many settings of a definition can be replaced for a single placed entity. The inspector offers them
where they apply:

- props: *Customize lock* (another key), dropping the definition's lock, *Make this lockable*;
  *Customize size & contents* of a container, or make it a container on its own
  ([Database › Props](05-database.md#props)).
- characters: *Customize this one…* for its own look, equipment and carried items; a portrait of
  its own; *Customize loot*; *Customize stock & prices* or *Make this a merchant*; a patrol route,
  a daily schedule or an activity ([Characters](06-characters.md)).

What happens to an entity during the game (a prop's state, a door unlocked, a character defeated,
an entity hidden) is kept across map changes and in save games.

## Zones

Zones are rectangles on a map, invisible in the game. Choose **Zones** (Z), drag a rectangle, and
choose its kind in the inspector:

| Zone | What it does |
|---|---|
| **Spawn point** | A named place where the player can arrive: the project's start, the target of a portal or a *Teleport* action. Usually one tile. |
| **Portal** | Entering it moves the player to a spawn point on a map (target map and spawn from dropdowns). Good for doors into houses. |
| **Trigger area** | Runs its triggers when the player enters it, leaves it or presses E inside it. Good for "you feel watched" moments, quest steps and traps. |
| **Enemy spawner** | Keeps a group of characters alive inside it (below). |
| **Map transition** | One end of a two-way link between maps (below). |
| **Fishing spot** | Water where the player can fish ([Gameplay › Fishing](09-gameplay.md#fishing)). |

The *Zones* button in the ribbon's View group shows or hides zones on the map. A selected portal has
**Go to <map>** (or double-click the portal) to jump to its target.

## Map transitions

There are three ways to connect maps:

**Map transition zones** are the easiest: a two-way exit, e.g. where a road leaves the map.

1. Choose **Transition** (T).
2. Drag over the exit on this map and pick the target map.
3. The target map opens: drag the other end there (where the road arrives).

Both zones are created and linked (`to_<map>`). Entering one moves the player one tile inside the
other end (away from the map edge, so they don't bounce back), and that end leads back. A transition
can also be drawn with the Zones tool and set up in the inspector: *Leads to map*, *Other end*,
*Draw the other end on …*, *Go to*, *Unlink* (both ends). The inspector says whether the link works
both ways; broken links are labelled red on the map.

**Portals** are one-way: they send the player to a named spawn point. Doors into buildings are
usually a portal outside and a portal inside, each leading to a spawn point next to the other.

**Edge transitions** (map settings, nothing selected): link a whole edge (north, south, east, west)
to another map. Walking off that edge moves the player onto the other map, keeping their position
along the edge plus an optional offset. Good for open worlds made of map pieces.

## Enemy spawners

An **Enemy spawner** zone keeps a group of characters alive: choose the character, how many, and the
respawn mode:

| Respawn | Behaviour |
|---|---|
| **Never** | Defeated characters stay dead, also in save games. |
| **Every time the map is entered** | The group is complete again whenever the player comes back. |
| **After N seconds** | Defeated characters return after a while. |

Characters appear on random walkable tiles inside the zone, never right next to the player. Their
*Defeated* triggers come from the character's definition. A spawner can have its own **loot table**
(*Loot of spawned characters*), which every character it spawns uses instead of its own.

The *Spawners on / off* action and the scripts `disable_spawners(map)` / `enable_spawners(map)` switch
spawners off and on (saved); a switched-off spawner's characters vanish. Characters placed directly
on a map (not by a spawner) stay dead once defeated.

## Loot

Characters that can be defeated have a **loot table** (*Database › Characters › Loot*): each line is
an item, a drop chance in percent and an amount (min–max); every line rolls on its own. A placed
character can *Customize loot*, and a spawner can override it for everything it spawns. What drops
lands on the ground where the character fell and stays there, also across map changes and in save
games, until picked up. Characters also drop what they wear and carry.

In the example, slimes drop coins and apples, goblin archers arrows and coins, and the forest's
goblin camp has its own richer table.
