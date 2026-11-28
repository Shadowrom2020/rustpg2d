# 4. Objects and zones

## Objects: props, characters and items on a map

What stands on a map and does something is an **object**: a placed copy of a definition.

- **Props**: chests, doors and gates, lamps, furniture, crafting stations, farm plots, ore veins,
  trees to chop. (A prop that only stands there is placed as [scenery](03-maps.md#scenery).)
- **Characters**: villagers, merchants, enemies, animals.
- **Items**: things lying on the ground to be picked up. Pressing E on one picks it up.

### Placing

1. Choose **Place** (N).
2. In its panel on the left, pick what to place: a search box (name or id) and the kinds *All*,
   *Characters*, *Props*, *Items*, *Structures*, *Scatter sets*. The panel says what the picked
   prop will be: an object (it does something) or scenery. A game without any of a kind offers
   *New…*; the asset library has ready-made ones (*Asset library* under *Game* in the Project
   browser, or *New…* › *as one of the asset library*).
3. Click the map. Objects snap to the tile; hold **Shift** for free placement. Big props snap by
   their footprint (props an even number of tiles wide stand on a tile edge). **R** turns the next
   one a quarter.
4. **Right-click, Esc or *Stop* ends placing.**

The **Outliner** above the Inspector lists everything placed on the map (characters, objects,
items, scenery, zones; searched): click one to select it on the map.

### Selecting and moving

With the **Select** tool (V), click an object to select it and drag it to move it. Clicking again on
the same spot picks the next of the things stacked there. Esc lets go of the selection. **Ctrl+D**
duplicates it (a tile to the east), **Delete** deletes it, **R** turns it.

**Several at once:** **Shift+click** or **Ctrl+click** adds a thing or zone to the selection (or
takes it out again); a **drag on empty ground** selects everything inside the box (*Picks* in the
map's bar says what a box takes: characters, objects and items, scenery, zones). Dragging one of the
selected moves them all together; **Delete**, **Ctrl+D**, **Ctrl+C** / **Ctrl+X** / **Ctrl+V** act
on all of them. Each of these is one step of the history. The Inspector then offers *Duplicate*,
*Delete*, *In a row* / *In a column* (in line with the first one selected), **Replace with…**
(another definition of their kind, each where it stands, with its name and events) and *Save as
structure*. **Alt+drag** selects a region of tiles instead ([Maps › Copying, cutting and pasting
regions](03-maps.md#copying-cutting-and-pasting-regions)).

### The Inspector

The right panel shows the selected object: its name and what it is (*Character at tile 34, 22*),
**Edit definition** (opens what every one of them is), *Turn*, *Duplicate*, *Delete*, *Replace
with…* and, for a prop, *Make scenery*; for a prop with states its **State** when the map is first
entered (a gate that starts *open*). Then its form:

| Section | What it's for |
|---|---|
| **Placed here** | Its **Name** (like `guard` or `cellar_door`: actions and scripts address objects by this name, so give one to everything an event should talk to), **Facing**, **Active** (hidden ones can be shown later with the *Set active* action, e.g. a reward that appears), a character's **Faction**, a stack's count. |
| **Daily life** (characters) | A patrol route (*Add waypoints*: clicks on the map), a daily schedule, an activity ([Characters](06-characters.md)). |
| **Its own, instead of the definition's** | What this one has differently: a prop's container, lock, loot and light; a character's look, equipment, carried items, portrait, shop, loot, stat block and light. Left empty, the definition's counts. |
| **Events** | Its own events, *in addition to* its definition's, as an event sheet. See [Events](07-events.md). |

What happens to an object during the game (a prop's state, a door unlocked, a character defeated,
an object hidden) is kept across map changes and in save games.

## Zones

Zones are rectangles on a map, invisible in the game. Choose **Zones** (Z), pick the kind in its
panel, and drag a rectangle; drag a zone to move it, its corners to size it. The Inspector shows its
name, kind, place, events and (spawners) the loot of what it spawns:

| Zone | What it does |
|---|---|
| **Spawn** (a spawn point) | A named place where the player can arrive: the project's start, the target of a portal or a *Teleport* action. Usually one tile. |
| **Portal** | Entering it moves the player to a spawn point on a map (target map and spawn from dropdowns). Good for doors into houses. |
| **Trigger** (an area) | Runs its triggers when the player enters it, leaves it or presses E inside it. Good for "you feel watched" moments, quest steps and traps. |
| **Spawner** | Keeps a group of characters alive inside it (below). |
| **Encounter** | Spawns a group of monsters, listed or picked for the party's level ([below](#encounters)). |
| **Transition** | One end of a two-way link between maps (below). |
| **Fishing spot** | Water where the player can fish ([Gameplay › Fishing](09-gameplay.md#fishing)). |
| **Drop** | A pit or a ledge's edge, its **height** in feet: the player walking in falls (1d6 bludgeoning per 10 ft, at most 20d6, and prone for a moment) and **lands on** the nearest spawn of that name, on this map or another (*Lands on map* empty = this one), or with no spawn on the first open tile past the zone. Running at it, or walking 10 ft up to it, jumps it when it is no wider than the hero's Strength score in feet (half without a run-up). Characters keep off it; fliers cross it in fights ([Maps › Climbing, jumping and falling](03-maps.md#climbing-jumping-and-falling)). |
| **Trap** | A hidden trap of the SRD (since 0.32): a pressure plate, a pit's cover, a tripwire. **Preset** fills the fields from the SRD's sample traps (collapsing roof, falling net, fire-breathing statue, hidden pit, poison darts, poison needle, rolling sphere); change anything after. **Found by** Perception or Investigation against its **DC** (0 = it can't be found): the hero notices it within 2 tiles by their passive score, or by a *Search*. **Disarm** DC: DEX with Thieves' Tools, E standing in or beside the found trap (0 = it can't be disarmed). When it springs: an **Attack** roll against the hero's armor class (0 = none) or a **Saving throw**, the **damage** and its type (half on a successful save when *half on a success* is ticked), **Then (after a hit) a save** with its own damage (the darts' poison), a **condition**, a **Fall** (1d6 per 10 ft and prone) and the **Text** shown. **Armed again after** hours, or it is sprung for good. Walking in springs an armed trap, found or not; companions and other characters don't, and nothing springs in a fight. Its triggers also react to *Trap sprung* and *Trap disarmed* ([Gameplay › Traps](09-gameplay.md#traps)). |
| **Ambience** | A looping sound heard here: full **volume** inside the zone, fading to nothing **fades over** that many tiles around it (0 = only inside). Birds over a meadow (a zone over the whole map), a river along its banks, a tavern's murmur over the room. Zones with the same sound play it once, at the loudest; different sounds mix, and they fade in and out as the player walks. Players set their loudness with *Options › Sound › Ambience volume* ([Art and sound › Soundscapes](10-art-and-sound.md#soundscapes-and-footsteps)). |

*Zone areas* in the map's bar shows or hides zones on the map.

## Map transitions

There are three ways to connect maps:

**Map transition zones** are the easiest: a two-way exit, e.g. where a road leaves the map.

1. Choose **Zones** (Z) and the kind **Transition**.
2. Drag over the exit on this map. The map's bar says that it waits for its other side.
3. Open the other map and drag the other end there (where the road arrives).

The two are linked: each leads to the other. Entering one moves the player one tile inside the
other end (away from the map edge, so they don't bounce back). **Link…** in a transition's Inspector
starts the same for one that exists. The **World view** draws every way between maps and shows the
ones that lead nowhere in red.

**Portals** are one-way: they send the player to a named spawn point. Doors into buildings are
usually a portal outside and a portal inside, each leading to a spawn point next to the other.

**Edge transitions** (the map's settings, nothing selected: *Beyond the edges*): link a whole edge (north, south, east, west)
to another map. Walking off that edge moves the player onto the other map, keeping their position
along the edge plus an optional offset. Good for open worlds made of map pieces.

## Enemy spawners

A **Spawner** zone keeps a group of characters alive: choose the character, how many, and the
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
asset library ([Characters › Monsters](06-characters.md#monsters)): *New…* › Character › *as one of
the asset library* brings one into a project, then name it in the zone. An encounter's *Loot of spawned characters* works like a
spawner's. `status` prints `encounters=` (each zone with its difficulty, budget and group).

In the example every group of monsters is an encounter, so a lone level-1 hero meets few and a
party or a higher level more: a Medium one of wolves and giant rats waits in Whisperwood's
south-east (*wolf_hollow*: a wolf for a lone level-1 hero, more with the guard along), the rat den
(*rat_den*: two giant rats at level 1, four at level 2) and the goblin camp are Medium too, and
the mine has Easy ones of giant rats and bats. The Wilds' fifty groups work the same way.

## Loot

Characters that can be defeated have a **loot table** (*Loot* in the character's form): each line is
an item, a drop chance in percent and an amount (min–max); every line rolls on its own. A placed
character can have its own (*Its own, instead of the definition's*), and a spawner can override it
for everything it spawns. What drops
lands on the ground where the character fell and stays there, also across map changes and in save
games, until picked up. Characters also drop what they wear and carry.

In the example, the rat den's giant rats drop coins and apples, goblin archers arrows and coins, and the forest's
goblin camp has its own richer table.
