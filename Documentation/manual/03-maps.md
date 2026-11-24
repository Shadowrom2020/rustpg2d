# 3. Maps

A map is one area of the game world: a village, a house's inside, a forest, a dungeon level, up to
1024 × 1024 tiles of 32 × 32 pixels large. Its ground is **painted** with materials (grass, a road,
a pond), trees, rocks, walls and buildings stand on it as **decorations**, and entities (props,
characters, items) and zones are placed on it. The tile grid remains for what needs whole squares:
collision overrides, route planning, zones and positions in events and scripts.

## Creating and switching maps

- **Map › New map…** (or *+ New map…* in the map dropdown) asks for a name and a size. Map names may
  use letters, digits, `_` and `-`.
- The **map dropdown** in the top bar switches maps; **Ctrl+M** opens a searchable switcher;
  **Ctrl+PgUp / Ctrl+PgDn** step through the maps.
- With nothing selected, the inspector shows the map's settings: its **Title** (the name players
  see on the map screen, at its exits and in the save list; empty = the map's name with a capital,
  e.g. *Tavern*), **Rename map…** (every reference in the project follows), **Resize…**, the **Background** colour shown outside the map and where
  no ground is painted, and the map's **Music** (a looping sound; empty keeps whatever is playing). **Battle
  music**: *Project's* (*Database › Project › Game sounds › Battle music*), *None* (the map's music
  plays on in fights) or *Own track*.

## Painted ground

The map's **painted ground** is a grid of 8 × 8 pixel cells (four per tile side), each holding a **material** from [*Database › Materials*](05-database.md#materials): grass,
dirt, sand, water, cobblestone, snow… The game draws it with a shader that shapes where two
materials meet, so painted roads, shores and meadows have natural, uneven edges instead of tile
edges:

- **Crisp** materials (dirt, water, cobblestone) get a hard edge in steps of the art's pixels,
  made ragged by noise (*Raggedness*, *Edge pixel*);
- **Soft** materials (sand, snow, forest floor) fade into their neighbour;
- **Straight** materials (wooden floor, stone tiles, carpet, rug) follow the 8 px cells exactly,
  with straight lines and square corners; their rim is a border (the gold edge of the carpet);
- the material with the higher **priority** is drawn over the other where they meet (water over
  sand over grass), and a **rim** draws a fringe just outside a material's edge (the sandy band
  round the starter library's water);
- **moving** materials flow (*Flow*: the texture drifts, a river) or play **animation frames**
  (*Animation*: the example's rippling water, the starter library's lava, swamp water and
  waterfall pool), in the editor as in the game.

Painted ground **collides** by its cells: water stops walking exactly at its drawn edge (shots fly
over it: *Low*), a *Solid* material stops walking, shots and sight. Route planning (click-to-move,
characters) counts a tile as blocked when half its cells are. Collision painted on the
[collision layer](#collision) wins over the ground (*Walkable* frees a tile, *Solid* blocks it).
Footsteps play the material's sound.

The starter library brings materials for grass, meadow, forest floor, dirt, farmland, sand, snow,
cobblestone, flagstones, stone tiles, wooden floor, carpet, rug, water, earth, cave floor and the
void, the animated **lava**, **swamp water** and **waterfall pool**, **mud**, natural **cave rock**,
**sandstone tiles**, and those its converted tile structures needed. A map uses at most 64 materials.

### The Ground tool

**Ground** (P) is the first tool of the ribbon's *Paint* group. Its panel shows the project's
materials as cards with their names (the map's own first; a search box filters them): click one,
then paint on the map:

| Mode | Use |
|---|---|
| **Brush** | Click or drag a brush: **Round**, or **Square** for straight edges |
| **Line** | Drag from one end to the other: a road or a stream as wide as the brush |
| **Rectangle** | Drag from one corner to the other: a filled rectangle (a floor, a carpet, a field) |
| **Fill** | Click: the connected area of the same material (or of no ground) |

*Size* is the brush's diameter (a line's width) in tiles; **[** and **]** change it by a quarter
tile. **Snap to tiles** (on by default) puts rectangle corners, the square brush and line ends on
the 32 px tile grid; off, they follow the 8 px cells. A rectangular carpet: *Carpet*, *Rectangle*,
drag over the floor. **Shift** erases (no ground: the background or the tiles show), **right-click** takes the
material under the cursor. Each stroke is one undo step. While the tool is active, decorations
drawn over everyone are see-through.

AI agents paint the ground with `paint_ground` (circles, strokes, polygons) and `fill_ground`
([chapter 13](13-ai-agents.md)); scripts of the editor with `ground`, `gpaint`, `gline`, `gfill`
([Testing](17-testing.md)).

## Decorations

Trees, rocks, flowers, fences, walls and buildings are placed as **decorations**: props drawn as
pictures anywhere on the map (pixel positions, not tiles). A decoration has its prop's picture,
hitbox, solidity (*Low* included), light and *Ladder* flag, but no states, triggers or interaction,
so a map can hold tens of thousands of them (a 1024 × 1024 forest of 67 000 trees runs at full
speed). A prop that does something (a door, a chest, a tree to chop) is placed with the
[Entities tool](04-objects-and-zones.md) instead.

Each decoration lies on a **layer**: *Flat* (on the ground, under everyone: rugs, puddles),
*Sorted* (in front of or behind characters by its feet: trees, rocks, walls, houses) or *Above*
(over everyone: treetops, roofs, arches); choosing a prop that *lies flat on the ground* picks
*Flat*. A prop with an **animation** (*Database › Props*:
frames of its sheet and a speed) plays it as a decoration too: torches, flags, water wheels.

**Decorations** (O) in the ribbon's *Objects* group has four modes:

| Mode | Use |
|---|---|
| **Place** | Click: one of the chosen prop, on its grid when it snaps (*Snap to*) |
| **Line** | Drag a straight line of a prop on its grid: a wall or a fence. A **wall set** (a prop with *Wall pieces*) swaps in the piece each spot needs by its neighbours (corners, ends, crossings), also where a new line meets an old one |
| **Scatter** | Click or drag a brush: a **scatter set**'s props at random, keeping their spacing (*Forest*, *Rocks*, *Wild plants*; [Database › Scatter sets](05-database.md#scatter-sets)) |
| **Erase** | Click or drag a brush: removes the decorations under it |

*Mirror at random* flips placed props half of the time; props with a 3D model or pictures from
four sides **turn**: **R** (or *Turn* in the left panel) turns the next ones a quarter, their
hitbox with them; **[** and **]** change the brush size;
right-click takes the prop of the decoration under the cursor. Each click or stroke is one undo
step. With *View › Collision* on, solid decorations show their hitboxes (red; yellow: low) and the
painted ground its blocked cells.

AI agents place them with `place_objects`, `scatter` and `erase_objects` ([chapter 13](13-ai-agents.md)).

## Maps from older versions

Until 0.20, maps were grids of tiles from tilesets, painted on layers (*Ground*, *Detail*,
*Overhead*) with brush, rectangle, fill and auto-tiling terrain tools. The editor converts such a
project **once, when it opens it**, after copying it beside itself (`<name>-backup-tiles`, a
project file as `<name>-backup-tiles.r2dproject`):

- the **ground layer** becomes painted ground: every kind of tile a material whose texture is the
  tile (so the map looks as before; a project's materials with the same picture are used instead;
  an animated tile's frames become the material's animation frames),
  every **terrain** a material of its inside tile; a terrain tile's edge tiles paint its cells by
  their neighbours, so shores and paths keep their shape, now in 8 px cells;
- the other layers become **decorations** (*Detail* = *Flat*, *Overhead* = *Above*) of props made
  for the tiles (`terrain_tile_12`: a sheet over the tileset's image, 32 px frames; solid, low,
  ladder, light and animation carried over; give them real names with *Rename…* in
  *Database › Props*, which updates every map and structure, as the example and the starter
  library did: `hedge`, `berry_bush`, `wooden_door`…);
- **tile structures** become structures of ground cells and decorations; the tilesets go.

Nothing that could be walked on is blocked afterwards; entities, zones, triggers and save games
are unchanged. Your asset library converts the same way the first time it opens (backup beside it),
and old `.r2dpack` packages convert while they are imported. Converted maps look like the tile maps
did; repaint them with the Ground and Decorations tools to get natural edges and free placement.

## Collision

What blocks movement comes from the painted ground (*Solid* and *Low* materials, by their cells)
and from solid decorations, props and characters (by their hitboxes). The **Collision** tool (C)
overrides it per tile: **left-click** paints *force solid*, **right-click** *force walkable* (a gap
in a wall of decorations, a bridge over painted water), **Shift+left** goes back to automatic. The
*Collision* button in the ribbon's View group shows it all as an overlay.

### Low and high obstacles

Solid things come in two heights. **High** ones (walls, tree trunks, statues, market stalls) stop
walking, arrows, bolts and the sight of enemies. **Low** ones (fences, rocks, mushrooms, water,
tables, barrels, chairs) stop only walking: the player and the archers in the game shoot over them,
and enemies see across them. Which materials are low is set per material (*Database › Materials ›
Walking: Low*), which props are low per prop (*Database › Props › Low*, also for their
decorations); everything solid is high unless it is marked low. In the collision overlay low cells
and hitboxes are yellow, high ones red, and a tile painted *force solid* with the Collision tool
counts as high.

## Copying, cutting and pasting regions

With the **Select** tool, drag on empty space (or **Shift+drag** anywhere, also over entities) to
select a rectangle of tiles; a dashed outline shows it.

- **Ctrl+C** copies it: the painted ground, the decorations and entities standing in it and the
  collision overrides (zones are not copied).
- **Ctrl+X** copies it and empties it; **Del** only empties it.
- **Ctrl+V** arms the copy: it follows the mouse as a transparent preview, and every click pastes it
  as one undo step, until Esc or right-click. A pasted copy replaces everything in its rectangle,
  so trees don't stand in a pasted house. Pasted entities keep their triggers; named ones get a
  free name (`guard` becomes `guard_2`).

The clipboard stays when you open another map, so regions move between maps. A selected entity can
be copied on its own too. The region's inspector has the same commands plus **Save as structure…**.

## Structures

Structures are ready-made pieces placed with one click: houses, wall and fence pieces, rooms,
ponds, groves, a plaza, a market row, and a piece of each landscape: a *Frozen pond* and a
*Snowy grove*, an *Oasis* and a *Desert ruin*, a *Cave chamber* and a *Crystal grotto*, a *Swamp
pool* and a dungeon's *Collapsed hall*. A structure paints ground, places decorations, sets collision
overrides (e.g. a walkable door) and places its entities (a shop's sign and barrel, a room's chest).
Tiles it marks *clear* lose the map's decorations first, so no tree stands in a room.

**Placing**: choose **Structures** (H). The palette groups them by category with thumbnails (search
box, *Add from library…*). Pick one: a transparent preview follows the mouse, a click places it,
right-click or Esc stops. Each placement is one undo step. Placed structures are ordinary ground,
decorations and entities.

**Capturing your own**: *New from map area…* in the Structures palette (or *Save as structure…* on
a selected region): drag a rectangle on the map, then give it an id, a name and a category. Untick
*Include the ground* for things that should stand on any ground. Its ground, the decorations and
entities standing in it and its collision overrides become the structure; with the ground, placing
it clears the area's decorations first.

**The structure editor** (*Database › Structures*; new ones with *New asset… › Structure*, as an
empty grid or a copy of another one) has three columns:

- **Left**: the tools (*Ground*: paint a material on tiles, erase = the map keeps its ground, remove
  the map's ground; *Decorations*: place one (on the chosen layer), clear the map's decorations;
  *Collision*: solid, walkable, automatic, keep the map's; *Objects*: place a prop as an entity).
  Below: the size (*Resize*) and undo / redo.
- **Middle**: the grid. Click or drag to paint; right-click picks a tile's material (or removes the
  decorations or props standing there).
- **Right**: the materials, the prop gallery or the collision legend.

Finer ground (a shore, a winding path) is easier to paint on a map and capture.

The example has 35 structures (pieces of nature and the buildings of the village); the starter
library adds town houses, a brick shop, a watchtower, city walls, fences, hedges, a plaza, a market,
a canal, a campsite and dungeon rooms, corridors and a prison cell. The example's buildings are
single pictures (props such as *Stone house*, *Thatched cottage*, *Timber-framed hall*, *Chapel*,
with an invisible *Window glow* light behind each lit window), its trees the tree props; the
starter library's other structures still show their former tiles as decorations.

## Map settings

Select nothing (Esc) and the inspector shows the map's settings.

### Lighting

| Setting | Effect |
|---|---|
| **Always bright** | The default: no darkness. |
| **Outdoor (day & night)** | Bright by day, darker at night by the map's *Night darkness*, with warm dusk and dawn. The game shows the time of day. |
| **Dark (dungeon, cave)** | Constant darkness; at 100 % only light sources show anything. |

Light comes from props (per state, so a lamp can be lit or off; as decorations too), characters, items (a lantern in the hand, a torch on the ground), any placed entity, and
the player's equipped light. Each light has a radius in tiles, a colour, a strength and optionally a
flicker. **View › Lighting preview** (the bulb in the ribbon, with a time slider for outdoor maps)
shows darkness and lights in the editor.

![The lighting preview at 21:30](../images/editor-lighting.webp)

The clock itself is set in *Database › Project* (day length, the hour new games start at); see
[Gameplay › Time of day](09-gameplay.md#time-of-day).

### Weather

*Clear*, *Rain*, *Snow* or *Fog*, with a *Strength*. Rain falls in streaks with splashes, darkens
the map a little and plays a rain sound (players set its loudness with *Options › Sound › Ambience
volume*); snow drifts down; fog drifts over the map in soft clouds. Particles are drawn over the lit
scene, dimmer at night, with lamps showing through. **View › Weather preview** (the cloud in the
ribbon) shows it in the editor. The *Set weather* action and the scripts `set_weather` /
`set_map_weather` change it during the game; the change is kept in save games.

In the example, an eerie fog hangs over Whisperwood until the shrine is lit, and it rains in the
Wilds.

### Parallax layers

Images that move at their own speed: **behind the map** (a sky or an abyss, seen where no ground
is painted and past the map's edge) or **over the map** (cloud shadows, mist). Each layer
has a sprite sheet (drawn whole) and:

| Setting | Meaning |
|---|---|
| **Follows** x / y | 100 % moves with the ground, 0 % is fixed to the screen, in between looks far away |
| **Drift** | Pixels per second, for clouds that move by themselves |
| **Offset**, **Repeat** x / y, **Size**, **Opacity** | Placement, tiling, scale and transparency |

The arrows change the drawing order. **View › Parallax layers** previews them. In the example,
cloud shadows drift over the Wilds and dark mist moves far below the mine's chasm.

### Map screen and fog of war

The in-game map screen (M) shows the whole map. Tick **Fog of war** and it only shows what the
player has explored: about six tiles around everywhere they have walked, with soft-edged fog over
the rest, and exits only once found. Explored areas are kept in the save game.

### Edge transitions and map triggers

*Edge transitions* let the player walk off a map's edge into the next map, and *map triggers* react
to the player arriving, waking up or the clock reaching an hour. See
[Objects and zones › Map transitions](04-objects-and-zones.md#map-transitions) and
[Events](07-events.md).

## Ladders

A prop with the **Ladder** flag (*Database › Props*), placed as a decoration, can be climbed by the
player: only up and down move, slower, and the hero is seen from behind and can't attack. Ladders
aren't solid, but other characters keep off them. A ladder's top can lead onto a platform or to
another map through a portal. The example's two watchtowers at the east gate are built this way
(roof and front rail over everyone, the lookout deck walkable, the braces solid; the library has them as the
structure *Lookout Tower*).

## Climbing, jumping and falling

Maps are flat, so height is built from three pieces (the SRD's movement rules):

- **Climbable walls and ropes:** a *Ladder* prop with an **Athletics DC** (*Database › Props*) is a
  wall the hero has to climb: stepping onto it rolls Athletics against the DC (on screen and in the
  roll log). A success climbs it like a ladder, at the ladder's slower speed; a failure finds no hold, and
  the hero can try again two seconds later. DC 0 is a plain ladder. The tiles under it need to be
  walkable (paint them *Walkable* in the collision layer when the wall is solid rock). The
  library has the **Rough rock wall** (DC 12) and the **Climbing rope** (no check).
- **Drops:** a *Drop* zone ([Objects and zones › Zones](04-objects-and-zones.md#zones)) over a
  pit or along a ledge's edge. Paint it dark (the *Abyss* material) and its tiles *Walkable* in the
  collision layer, so the player can step in. Walking in is a fall of the zone's height: 1d6
  bludgeoning per 10 ft (at most 20d6), prone for a moment, then the landing. That is a spawn on
  this map or another (a lower chamber, the level below), or with none the tile past the zone.
  Characters keep off drops, and the click-to-walk path goes round them.
- **Jumps:** running (Shift) at a drop, or walking 10 ft up to it, the hero leaps it when it is no
  wider than their Strength score in feet (half from standing): a hop over to the first free tile
  past it, or one beside it. A wider gap is a fall. A **click** (left or right) on the far side of a
  drop the hero can't walk round plans the jump too: the path runs three tiles straight at the gap
  and over it, then on to the clicked spot; a gap too wide for the hero is "Can't get there."
- **In fights** nobody steps into a drop, but walkers long-jump it as part of their move (the same
  distances, a run-up of two tiles walked straight at it this turn), and a fight ends when no foe
  can get at the party any more ([Gameplay › Fights](09-gameplay.md)).
- **Fliers** (a stat block with a fly speed: bats, birds, dragons) cross drops, water and other low
  obstacles in fights; walls and tall props still stop them.

The example's mine has both: the shaft in Grumbold's lair drops 20 ft into a small chamber whose
rough rock wall leads back up, and a 10-ft crack splits the corridor to the rats' nest (run to
jump it).
