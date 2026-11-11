# 10. Art and sound

## Sizes and layouts

- **Tiles** are always 32 × 32 pixels. Tilesets are images of tiles in a grid.
- **Character figures** (sprite sheets of kind *Character figure*) are usually 32 × 48 pixels per
  frame (64 × 96 on 2× sheets) and show the whole character: clothes, hair and what it carries
  painted in. A figure has 4 columns and 16 rows, four facings (down, left, right, up) per
  animation: **walk** (rows 1–4, 4 frames; the first column is the standing pose), **idle** (rows
  5–8, 2 frames: breathing), **attack** (rows 9–12, 4 frames: wind-up, swing, follow-through,
  recover) and **hurt** (rows 13–16, 1 frame). A sheet that stops earlier shows the standing frame
  for the missing animations ([Characters](06-characters.md#how-characters-look)).
- **Item icons** are 32 × 32. **Props** use one frame, or a block of several frames for big props.
- **Portraits** can be any size (64 × 64 works well).
- The starter library's characters (people, animals, monsters) are **2× sheets**: 64 × 96 image
  pixels per frame for people, drawn in the 32 × 48 place ([High-resolution
  sprites](#high-resolution-sprites)).
- Keep images at most 2048 pixels wide and high; the editor shrinks bigger ones for display.

### High-resolution sprites

A sprite sheet can carry more detail than its place in the world: its **Scale** (*Database ›
Sprite sheets*) says how many image pixels make one world pixel. At **2×** a character frame is
drawn 64 × 96 pixels in the image and fills the same 32 × 48 place as a 1× character, with twice
the detail; item icons are 64 × 64, and so on (3× and 4× work the same way). Positions, hitboxes
and tiles stay in world pixels, and tilesets are always 1×.

The game draws the world magnified (the project's *Scale* times the player's *Zoom*), so a 2× sheet
shows all its detail at a magnification of 2 or more; at less it is sampled down. The frame size
on the form is in image pixels; the form shows the size in the world below it. The pixel editor
works on the image's own pixels, and the sheet and frame pickers show 2× frames at the same size
as 1× ones.

Positions of entities are their **feet** (bottom centre), which is how characters stand on tiles
and sort in front of and behind each other.

### Colour features

*Database › Sprite sheets › Colour features* (figures only) marks the parts of a figure that can take
other colours:

1. Add a feature: **skin**, **hair**, **clothing**, **eyes** (each comes with matching colours to
   offer) or **Other** (any name).
2. Click the image's colours (most used first) that belong to the part: they become the feature's
   **shades**, framed. Usually a part has 3–10 shades, dark to light.
3. Edit the **colours offered** (remove one by clicking it, add any colour, or add the *Skin tones*,
   *Hair* or *Cloth* presets). The character creator offers exactly these; in the character editor
   any colour can be chosen.
4. The preview row shows the front of the figure in every offered colour.

A shade that the part shares with another part (brown hair and brown boots) would be recoloured in
both places. Give one of them a slightly different shade first: in the sprite editor *Replace
colour* within a frame or row, or paint the part in a placeholder ramp (the magenta / cyan shades of
the reference ramps work well), then pick those shades. The form warns when a shade belongs to two
features. The example's figures were prepared this way: their hair shades in the head area are one
step off the browns of their belts and boots.

## The pixel editor

![The pixel editor](../images/editor-pixel.webp)

The **Pixel editor** tab, *Edit pixels* on a sprite sheet or tileset, and *Edit art* in the item,
prop and character forms open it.

| Tool | Key |
|---|---|
| **Select frame** (the default; clicks only select the frame under the cursor) | S |
| Pencil, eraser, fill, line, rectangle, colour picker | B, E, G, L, R, I |

- The left and right mouse buttons draw with two colours (X swaps them). The palette shows a default
  set and the colours used in the image.
- Mouse wheel zooms, middle-drag pans; the pixel grid and the frame grid can be shown (character
  sheets label their rows *Down / Left / Right / Up*).
- **Frames**: click one to select it, **Ctrl+C** copies it, **Ctrl+X** cuts it (copies and clears),
  **Ctrl+V** replaces the selected frame with the copy. Buttons do the same, plus *Paste mirrored*,
  *Flip* and *Clear*. *Canvas … Resize* adds frames.
- A live preview shows the selected frame and its row as an animation.

**Animating character figures**: a figure sheet opens with its 16 rows labelled and an
**animation panel** instead of the row preview:

- pick the **animation** (*Walk* 4 frames, *Idle* 2, *Attack* 4, *Hurt* 1) and the **facing**; it
  plays at the chosen speed, *Pause* / *Next frame* step through it, and the **frame strip** below
  selects a frame of it;
- **Onion skin** shows the animation's previous frame (red) and, with *and the next*, the next
  frame (green) faintly over the selected one, to draw the next pose in line;
- **Duplicate →** copies the frame into the next column; the arrows (or **Shift+arrow keys**) move
  the frame's pixels by one, e.g. to line up the feet;
- **Copy row** / **Paste row** / **Clear row** work on the selected frame's row;
- **Mirror Left → Right** makes the animation's Right row the mirrored Left row (*all animations*
  for every animation);
- **Fill idle & hurt** gives empty idle and hurt frames the standing walk frame of their facing,
  so a walk-only sheet becomes a complete figure to refine;
- **Replace colour** turns the right colour (pick it with the picker's right click) into the left
  colour in the frame, its row or the sheet: e.g. a hair shade one step off the boots' shade, so a
  colour feature can tell them apart.

Every operation is one undo step.
- **Ctrl+S** saves the PNG and updates it everywhere; Ctrl+Z / Ctrl+Y undo and redo.

## New assets

**New asset…** (*Assets* menu, the Database, or the Asset Library window) is a wizard for a **prop**
(size in tiles), an **item**, a **tileset**, a **sprite sheet** or a **structure**. Draw a new image,
start from a copy of an existing one, or reuse a frame of an existing sheet. New images open in the
pixel editor, and the definitions get sensible hitboxes and speeds. Characters are made with **New
character…**: from an existing figure, a new figure sheet to draw (blank or a copy), an import or
AI generation ([Characters › New characters](06-characters.md#new-characters)).

In the forms, **+ New blank icon** (items) and **+ New blank frame for this state** (props) find an
empty frame on the sheet, or add a row when it's full, and open it for drawing.

## Generate art with Meshy

*Assets › Generate art…* draws new art with your own [Meshy](https://www.meshy.ai) account. Enter
your API key once in *Edit › Settings… › Meshy* (it is kept for your user only, apart from
projects, and never shown to AI agents); *Test key* shows your credits. The settings also pick
the model (the cheapest, *Nano Banana 2 Lite*, costs 3 credits a picture), a **budget per
session** (Generate art stops when the session has spent it) and *Ask before spending*.

In the window, choose what to draw, give it an id and describe it:

- **Character**: a size (person, small person, big, huge, small animal, large animal, flying
  creature) and an attack (none, sword slash, overhead blow, spear thrust, bow shot, club smash,
  spell cast, pounce). It is drawn in your project's style: Meshy gets the walk cycles of two of
  your figures (the player character's and a playable one; else the starter library's hero) as
  references, draws the character's walk in all four directions, then, with that as the
  character's reference, its attack. The editor turns the pictures into a figure sheet with walk,
  idle (breathing), attack and hurt rows. 1 picture, 2 with an attack. *Add to project* creates
  the character with a person's speed and hitbox; mark its colour features afterwards.
- **Item icon**: 32 × 32, in the style of your *items* sheet's icons.
- **Prop**: 1 to 4 tiles wide and high, standing on its bottom edge.
- **Portrait**: head and shoulders for dialogs, 128 × 128 at 2×; *For character* makes it that
  character's dialog portrait when added.
- **Painted image**: a picture for *Show image* (up to 1024 pixels).

*Draw* shows the cost first; the job's pictures then take about a minute each. The result is
previewed (characters walk, idle, attack and flinch); **Redo walk** / **Redo attack** / **Redo
picture** draws one picture again if Meshy got something wrong (a missing staff, a wrong colour),
and **Add to project** writes the image and its sprite sheet, plus a character (characters), an
item (icons) or a prop (props) with that id when there is none yet. **Edit in sprite editor** then
opens the new sheet to touch it up. The status bar tells when a
job is ready and what it cost. Agents can do all of this from a chat ([AI agents](13-ai-agents.md#drawing-art)).

The scripts the example's art was made with are in `scripts/art/meshy/` (for batches of
characters).

## Importing

*File › Import…* brings existing art in without redrawing it. A report says what was imported and
what was left out; nothing existing is overwritten (files and ids get a free name).

**Tiled maps** (`.tmx` / `.tmj`, orthogonal, not infinite; embedded or external tilesets; CSV,
base64 or compressed tile data):

- every tileset becomes a tileset of the project, repacked into plain 32 px tiles (other sizes are
  scaled); tiles with a `solid` property block, tile animations become animated tiles, and Wang sets
  become terrains;
- tile layers become layers (names containing "over" or "above", or a boolean property `above`, draw
  above characters); a layer named `collision` marks solid cells;
- rectangles of object layers become trigger zones named like the object, and points named
  `spawn:NAME` spawn points.

Flips, rotations, image layers and tile objects are dropped and counted.

**LPC character sheets** (a Universal LPC Spritesheet with 64 × 64 frames, or a 4-row LPC walk
cycle) become character figures: the walk (frames 0, 2, 4, 6), idle (the standing frame), and on
a full sheet the attack (the slash rows) and hurt (the falling row) in R(ust)PG2D's layout.
Spellcast, thrust and shoot are left out. *A woman* marks the figure for female players.

**Aseprite sprite sheets** (the JSON of *File › Export Sprite Sheet*, next to its PNG): when the
tags are named after the character animations (`walk_down`, `walk_left`, `idle_right`,
`attack_up`, `hurt_down`…, or `idle` / `hurt` for every facing), the export becomes a character
figure: each tag's frames are spread over its row (walk and attack 4, idle 2, hurt 1). Otherwise the
frames are packed into a grid and the tags become the sheet's **animations**, which the *Effect*
and *Fly* actions offer and scripts play with `effect(sheet, x, y, #{ tag: "explode" })`.

For LPC and Aseprite imports the dialog also asks for the **Scale** of the new sheet (2× for art
drawn at twice the resolution; see [High-resolution sprites](#high-resolution-sprites)).

## Sounds and music

*Database › Sounds* lists the sound files with a volume and *Play* / *Stop* buttons; *Browse…* copies
a file into `sounds/`.

- **Formats**: WAV and **Ogg Vorbis** (`.ogg`; keeps music small). Opus, FLAC and MP3 aren't
  supported; a file in another format is reported by name when the project loads.
- Sounds are mixed at **44.1 kHz**, so that rate plays best.
- **Music loops without a gap.** Each map can have its music (map settings), and the *Play music*
  action switches it. The engine prepares music so the loop point is seamless; a track whose length
  is a multiple of 4096 samples at 44.1 kHz plays exactly as it is.
- **Game sounds** (*Database › Project*) are the sounds the game plays by itself: pickups, attacks,
  hits, dialogs, doors. The rain has its own built-in sound.
- Players set the volume of music, effects and ambience in *Options*.

## Emotion icons

Emotions are animations of a sprite sheet of the kind **Emotions** (*Database › Sprite sheets ›
Kind*). The starter library's `emotions` sheet has 32 × 32 frames, three per emotion in one row
(a speech bubble with a symbol), and an animation per emotion under *Animations* (name, first and
last frame, speed). To add your own, draw frames on that sheet or on a new *Emotions* sheet and add
an animation named after the emotion: *Show emotion* and `emote()` find it by that name.

## Parallax images

Sky, abyss, mist and cloud-shadow layers are sprite sheets drawn whole, set in a map's settings
([Maps › Parallax layers](03-maps.md#parallax-layers)). Images that tile seamlessly look best.
