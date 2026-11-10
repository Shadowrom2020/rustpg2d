# 10. Art and sound

## Sizes and layouts

- **Tiles** are always 32 × 32 pixels. Tilesets are images of tiles in a grid.
- **Characters** are usually 32 × 48 pixels per frame (64 × 96 on 2× sheets). A **character sheet**
  has 4 rows (facing down, left, right, up) × 4 walk frames; the first column is the standing pose.
  It may continue with 12 more rows of the same order: **idle** (rows 5–8, 2 frames: breathing),
  **attack** (rows 9–12, 4 frames: wind-up, swing, follow-through, recover) and **hurt** (rows 13–16,
  1 frame). The game plays them when they are there and shows the standing frame otherwise. Bodies,
  hair, beards, outfits, armor and held items all use this layout, so the layers line up. A body
  marked **Complete figure** has everything painted in and is drawn alone ([Characters](06-characters.md#how-characters-look)).
- **Item icons** are 32 × 32. **Props** use one frame, or a block of several frames for big props.
- **Portraits** can be any size (64 × 64 works well).
- The starter library's characters (people, animals, monsters and every layer they wear) are
  **2× sheets**: 64 × 96 image pixels per frame for people, drawn in the 32 × 48 place
  ([High-resolution sprites](#high-resolution-sprites)). Your own 1× layers still fit them.
- Keep images at most 2048 pixels wide and high; the editor shrinks bigger ones for display.

### High-resolution sprites

A sprite sheet can carry more detail than its place in the world: its **Scale** (*Database ›
Sprite sheets*) says how many image pixels make one world pixel. At **2×** a character frame is
drawn 64 × 96 pixels in the image and fills the same 32 × 48 place as a 1× character, with twice
the detail; item icons are 64 × 64, and so on (3× and 4× work the same way). Sheets of different
scales mix freely: a 2× body wears 1× hair and outfits, and they line up. Positions, hitboxes and
tiles stay in world pixels, and tilesets are always 1×.

The game draws the world magnified (the project's *Scale* times the player's *Zoom*), so a 2× sheet
shows all its detail at a magnification of 2 or more; at less it is sampled down. The frame size
on the form is in image pixels; the form shows the size in the world below it. The pixel editor
works on the image's own pixels, and the sheet and frame pickers show 2× frames at the same size
as 1× ones. A 2× body must still use the skin and hair reference shades below.

Positions of entities are their **feet** (bottom centre), which is how characters stand on tiles
and sort in front of and behind each other.

### Drawing layered characters

The starter bodies are drawn with reference shades that the game recolours: skin in **magenta**
shades (recoloured to the chosen skin colour) and hair in **cyan** shades (the chosen hair colour).
To draw your own body or hair style, start from a copy of a starter sheet (`body_male.png`,
`hair_short.png`) and keep those shades. Outfits and armor draw an outline only where they stick out
past the body. An outfit has a **female cut** (`outfit_X_f` next to `outfit_X`) for female bodies,
and held items follow the hand positions of the walk animation.

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
- **Ctrl+S** saves the PNG and updates it everywhere; Ctrl+Z / Ctrl+Y undo and redo.

## New assets

**New asset…** (*Assets* menu, the Database, or the Asset Library window) is a wizard for a **prop**
(size in tiles), an **item**, a **tileset**, a **sprite sheet** or a **structure**. Draw a new image,
start from a copy of an existing one, or reuse a frame of an existing sheet. New images open in the
pixel editor, and the definitions get sensible hitboxes and speeds. Characters are made with **New
character…** (the character builder).

In the forms, **+ New blank icon** (items) and **+ New blank frame for this state** (props) find an
empty frame on the sheet, or add a row when it's full, and open it for drawing. Characters have
**+ New sheet**: a blank 4 × 4 walking sheet or a copy of another character's.

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
cycle): the walk animation becomes a sheet in R(ust)PG2D's layout, as a body, hair, beard, outfit,
armor or held-item sheet (the dialog asks). Other animations are left out, and colours aren't
recoloured.

**Aseprite sprite sheets** (the JSON of *File › Export Sprite Sheet*, next to its PNG): the frames are
packed into a grid and the tags become the sheet's **animations**, which the *Effect* and *Fly*
actions offer and scripts play with `effect(sheet, x, y, #{ tag: "explode" })`.

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
