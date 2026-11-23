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
| **Encounter** | Spawns a group of monsters, listed or picked for the party's level ([below](#encounters)). |
| **Map transition** | One end of a two-way link between maps (below). |
| **Fishing spot** | Water where the player can fish ([Gameplay › Fishing](09-gameplay.md#fishing)). |
| **Drop** | A pit or a ledge's edge, its **height** in feet: the player walking in falls (1d6 bludgeoning per 10 ft, at most 20d6, and prone for a moment) and **lands on** the nearest spawn of that name, on this map or another (*Lands on map* empty = this one), or with no spawn on the first open tile past the zone. Running at it, or walking 10 ft up to it, jumps it when it is no wider than the hero's Strength score in feet (half without a run-up). Characters keep off it; fliers cross it in fights ([Maps › Climbing, jumping and falling](03-maps.md#climbing-jumping-and-falling)). |
| **Trap** | A hidden trap of the SRD (since 0.32): a pressure plate, a pit's cover, a tripwire. **Preset** fills the fields from the SRD's sample traps (collapsing roof, falling net, fire-breathing statue, hidden pit, poison darts, poison needle, rolling sphere); change anything after. **Found by** Perception or Investigation against its **DC** (0 = it can't be found): the hero notices it within 2 tiles by their passive score, or by a *Search*. **Disarm** DC: DEX with Thieves' Tools, E standing in or beside the found trap (0 = it can't be disarmed). When it springs: an **Attack** roll against the hero's armor class (0 = none) or a **Saving throw**, the **damage** and its type (half on a successful save when *half on a success* is ticked), **Then (after a hit) a save** with its own damage (the darts' poison), a **condition**, a **Fall** (1d6 per 10 ft and prone) and the **Text** shown. **Armed again after** hours, or it is sprung for good. Walking in springs an armed trap, found or not; companions and other characters don't, and nothing springs in a fight. Its triggers also react to *Trap sprung* and *Trap disarmed* ([Gameplay › Traps](09-gameplay.md#traps)). |
| **Ambience** | A looping sound heard here: full **volume** inside the zone, fading to nothing **fades over** that many tiles around it (0 = only inside). Birds over a meadow (a zone over the whole map), a river along its banks, a tavern's murmur over the room. Zones with the same sound play it once, at the loudest; different sounds mix, and they fade in and out as the player walks. Players set their loudness with *Options › Sound › Ambience volume* ([Art and sound › Soundscapes](10-art-and-sound.md#soundscapes-and-footsteps)). |

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

## Encounters

An **Encounter** zone spawns a group of monsters when the map is entered, on random walkable tiles
inside it (never right next to the player), and respawns them like a spawner (*Never*, *Each time
the map is entered*, *After a delay*). It lists **monsters** with counts and has a **difficulty**:

- **Fixed group:** the listed monsters, as many as listed (2 wolves and a giant rat).
- **Easy, Medium, Hard, Deadly:** the list is a pool, and the counts are at most how many of each.
  The group is picked within the party's **XP budget**: the most expensive monster that fits
  first, then others at random until nothing more fits (at least one, at most eight). The pick is
  the same for a map and zone every time.

The budget is the engine's own (the SRD 5.1 has no encounter tables): every party member (the hero
and the companions on the map) at the hero's level L is worth the experience of one monster of
challenge **L/8** (easy), **L/4** (medium), **L/2** (hard) or **L** (deadly), from the SRD's
challenge → XP table:

| Hero's level | Easy | Medium | Hard | Deadly |
|---|---|---|---|---|
| 1 | 25 XP | 50 XP | 100 XP | 200 XP |
| 4 | 100 XP | 200 XP | 450 XP | 1,100 XP |
| 8 | 200 XP | 450 XP | 1,100 XP | 3,900 XP |

A companion doubles it, two triple it. A monster is worth its *Experience* (the SRD's XP for its
challenge, from its stat block); defeating it gives that XP to the hero. The SRD's monsters are in the
asset library ([Characters › Monsters](06-characters.md#monsters)): *Add from library* brings them
into a project, then name them in the zone. An encounter's *Loot of spawned characters* works like a
spawner's. `status` prints `encounters=` (each zone with its difficulty, budget and group).

In the example every group of monsters is an encounter, so a lone level-1 hero meets few and a
party or a higher level more: a Medium one of wolves and giant rats waits in Whisperwood's
south-east (*wolf_hollow*: a wolf for a lone level-1 hero, more with the guard along), the rat den
(*rat_den*: two giant rats at level 1, four at level 2) and the goblin camp are Medium too, and
the mine has Easy ones of giant rats and bats. The Wilds' fifty groups work the same way.

## Loot

Characters that can be defeated have a **loot table** (*Database › Characters › Loot*): each line is
an item, a drop chance in percent and an amount (min–max); every line rolls on its own. A placed
character can *Customize loot*, and a spawner can override it for everything it spawns. What drops
lands on the ground where the character fell and stays there, also across map changes and in save
games, until picked up. Characters also drop what they wear and carry.

In the example, the rat den's giant rats drop coins and apples, goblin archers arrows and coins, and the forest's
goblin camp has its own richer table.
