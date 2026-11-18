# 15. The player's guide

How a game made with R(ust)PG2D is played. Use it to write your game's own instructions, or point your
players here.

## Keyboard and mouse

These are the default keys; players change them in *Options › Gameplay › Controls*.

| Key | Action |
|---|---|
| Arrows / WASD (Shift = run) | Move |
| E / Enter | Interact, talk, advance a dialog |
| X | Attack in the facing direction |
| F (hold) | Block with a shield (see below) |
| Space | Dodge roll |
| Left Ctrl / V | Sneak on / off (see below) |
| Q | Switch weapon (bare hands, then the weapons you own) |
| 1–8, or a click on a quick bar slot | Use that item or spell |
| C / I | The character window: character sheet and inventory |
| K / L | The character window's *Skills* tab (your ability scores, saving throws and skills) and *Spells* tab (if the game has magic) |
| Tab | Next tab of the open window |
| J | Journal: *Quests*, *Bestiary*, *Dialogs*, *People* (the villagers you made friends with, in hearts), *Rolls* (your last checks and dice rolls) |
| M | The map screen |
| G | Companion orders (with companions along) |
| H | Hide or show the interface |
| P | Photo mode (see below) |
| F5 / F9 | Quick save / quick load |
| Esc | Pause menu; closes windows |
| Left click | Walk to the tile, attack the enemy under the cursor, advance a dialog |
| Right click | Walk to the character, item or prop under the cursor and interact with it |

Characters behind a counter (one solid tile between, in a straight line) are talked to across it,
with E or a right-click.

### In the windows

- **Character window**: arrows or a click select (on the character sheet the arrows move between
  the slots around the figure, right from the right column into the inventory); Enter, a
  double-click or a right-click uses or equips; drag items onto or off the equipment slots, onto the quick bar, or out of the window to
  drop them; G drops the selected stack (Shift+G one item). A **backpack** in the *Backpack* slot
  gives you more inventory slots; to take it off, first make room for what lies in its slots.
  **Furniture** (in a home of yours) is placed in front of you when used; E at it later asks
  whether to use it or pick it up again.
- **Shop**: click *Buy* or *Sell* (or double-click a row, or Enter / E) to trade one, Shift for ten;
  Tab switches between the merchant's goods and your items.
- **Loot window**: click a stack to take it (right-click: one), click your own to store it; T takes
  everything; Tab switches side.
- **Crafting window**: *Make* (or Enter / E) makes one, Shift up to ten.
- **Map screen**: arrows, dragging or the wheel scroll; + / − zoom; Space centres on you.
- **Journal**: Tab or Left / Right switch tabs; the *Dialogs* and *Rolls* tabs scroll with Up / Down,
  PgUp / PgDn, Home / End.

## Checks and locks

Games made with R(ust)PG2D follow the SRD 5.1 rules: when something is uncertain (persuading a
merchant, reading old carvings, picking a lock), the game rolls a d20, adds your ability modifier
and, if you are proficient in the skill or tool, your proficiency bonus, and compares the total
with a difficulty (DC). A line at the top of the screen says how it went ("Persuasion: 14 vs DC 13
- success"), and the journal's *Rolls* tab keeps the last fifty. Your scores and skills are on the
*Skills* tab (K); a dot marks the skills you are proficient in.

A **padlock** over a chest or door means it is locked. Without its key, press E to try to pick it
(a Dexterity check with Thieves' Tools). If it holds, give it ten minutes before you try again.

## Defending yourself

**Block**: hold F (gamepad: RB) with a shield in your left hand. You walk at half speed and can't
attack, but blows, arrows and lunges coming **from the front** do no damage, only push you back a
little. Raise the shield just before a blow lands (the last moment of the enemy's wind-up) to
**parry**: the attacker is stunned for a second. Without a shield, F only tells you that you need
one.

**Sneak**: Left Ctrl or V (gamepad: left stick click) switches sneaking on and off; *Sneaking*
shows under your hearts. You walk at half speed and quietly, and enemies notice you only within half
their sight, less still in the dark (at night, in dark places, away from torches and lamps; your own
lantern or light spell gives you away). One you bump into notices you anyway. A blow or an arrow
that hits an enemy who hasn't noticed you is a **sneak attack**: double damage, shown like a
critical hit. Fighting doesn't end sneaking; enemies who hunt you
lose track of you sooner while you sneak.

**Dodge**: Space (gamepad: B) rolls you about three tiles in the direction you are moving (or
facing). Nothing touches you while you roll, so you can roll away from a blow or straight through
it. Then you need a second before the next roll: the small blue dot after your hearts lights up
when you can roll again; a shield symbol next to it shows that your shield is raised.

## Gamepad

Any controller the system knows works next to the keyboard and mouse (Xbox, PlayStation, Switch Pro,
generic ones); the left stick and the d-pad move and navigate. In the browser version too: there
the first button press makes the pad known to the page.

| Button (Xbox names) | Action |
|---|---|
| Left stick / d-pad | Move; navigate menus, windows, the map |
| A | Interact, talk, advance, choose, use, buy / sell |
| B | Close a window, go back |
| X | Attack (while aiming a spell: cast straight ahead) |
| Y | Character window |
| RB / LB | Next / previous tab of an open window; on the map: zoom; while playing, RB (held) blocks with a shield and LB opens companion orders |
| B (while playing) | Dodge roll (while aiming a spell: cancel it) |
| LT (hold) | Run; buy / sell ten |
| RT | Switch weapon |
| Start | Pause menu |
| Select | Map |
| Right stick click | Journal |
| Left stick click | Sneak on / off (Hide interface has no button; H on the keyboard) |

Photo mode has no button of its own: choose *Photo mode* in the pause menu (Start).

**Hints follow your hands.** The help lines at the bottom of windows and menus (drawn a little
smaller, or in two lines, when a long translation doesn't fit the window) name the keys while
you play with the keyboard and mouse, and show your controller's buttons as soon as you press one
(and back when you press a key or click). The pictures match the controller: coloured A B X Y for
Xbox pads and the Steam Deck, ✕ ○ □ △ for PlayStation, the Nintendo letters for Switch pads. If the
game guesses wrong, choose it in *Options › Gameplay › Button style* (Auto, Xbox, PlayStation, Nintendo).

**Rebinding buttons.** In *Options › Gameplay › Controls* every action shows its button next to its keys. Select
an action, press A (Enter on the keyboard) and then the button you want for it; X clears the
action's button, *Reset to defaults* brings back all keys and buttons. B and Start keep the old
buttons; moving stays on the stick and the d-pad. A button that does something else in the same
place is named under the list (*R2 is also Switch weapon*).

## Menus

- **Character creator** (after *New game*, when the game has one): type a name, then Up / Down
  choose a row and Left / Right change it: race (its ability increases, speed and traits show
  under the picture), sex, body and colours (skin, hair, clothing…). Enter goes on to the second
  page: your ability scores (the standard array, point buy with 27 points, or 4d6 rolls; Left /
  Right swap or raise a score), what your race lets you choose (skills, a language, a dragon
  ancestor…) and your background. Enter starts the adventure, Esc goes back a page.
- **Title screen**: *New game*, *Continue* (the latest save), *Load game*, *Options*, *Achievements*
  (if the game has them), *Exit*. A game may order them differently, leave some out, add a
  *Credits* page (Up / Down or the wheel scroll it) and entries of its own.
- **Pause menu** (Esc): *Resume*, *Photo mode*, *Save game* (a new named save or overwrite one), *Load game* (every
  save with a small picture of the world, the map, the play time and its age; Del deletes), *Options*,
  *Save and quit*, *Quit* (after a confirmation); here too a game may change the entries.
- Closing the window while playing asks *Save and exit*, *Exit without saving* or *Cancel*.

## Options

The Options page has four tabs: Tab (or LB / RB on a gamepad) moves to the next one, a click picks
one. *Reset tab to defaults* at the bottom of each tab resets only that tab's options.

| Tab | Option | Choices |
|---|---|---|
| Graphics | Fullscreen | On / off |
| | Zoom | 75–250 % on top of the game's own pixel scale |
| | Shake & flash | The screen shake and red flash when hurt, shakes from explosions, and the short pause when a blow lands |
| | Show FPS | On / off |
| Sound | Master, Music, Effects, Ambience volume | In 10 % steps (ambience = the rain and the sounds of places: birds, rivers, a tavern's murmur) |
| Gameplay | Text speed | Slow, Normal, Fast, Instant |
| | Damage numbers | On / off: the numbers that rise from hits |
| | Quest markers | On / off: markers over quest targets, arrows at the screen's edge and pins on the map |
| | Language | The game's languages |
| | Button style | How gamepad buttons are shown: Auto, Xbox, PlayStation, Nintendo |
| | Controls… | Every action with its keys: Enter, then a key, adds one (up to three per action); Del clears; *Reset to defaults* |
| Accessibility | Text size, Captions, High contrast, Hold to repeat | **Text size** *Normal* / *Large* / *Larger* (125 %, 150 %: dialogs and choices, messages, speech bubbles, captions, the item details); **Captions** (what sounds and music are heard, written at the bottom left with `<` / `>` for the side a sound came from; at the top right while a window is open); **High contrast** (black panels with bright borders, plain white text, outlined damage numbers); **Hold to repeat** (holding Attack keeps attacking at the weapon's speed, holding E, or the right mouse button on a tree or a rock, keeps chopping or mining) |

Options are saved per game and kept between sessions and save games: in `~/.config/rustpg2d/<game>/settings.ron` on Linux, `%APPDATA%\rustpg2d\<game>` on Windows, `~/Library/Application Support/rustpg2d/<game>` on macOS, and in the browser for web games. Achievements are kept there too, not in save games.

## Photo mode

P, or *Photo mode* in the pause menu, stops the world (characters, weather, the clock) and hides
the interface; the camera is yours. A bar at the bottom names the controls (it isn't in the
pictures):

| Keys | Gamepad | |
|---|---|---|
| Arrows / WASD (Shift: faster), drag with the mouse | Left stick / d-pad | Move the camera over the map |
| Wheel, + / − | LB / RB | Zoom (0.5× to 3×) |
| [ / ] | LT / RT | Another hour's light (only for the pictures: the clock is back when you leave) |
| H | Y | Hide yourself |
| E / Enter / Space | A | Take a picture |
| Esc / P | B | Leave photo mode |

Pictures are PNG files at the window's size, named after the game and the moment
(`Emberbrook-2026-11-15_10-05-18.png`), in your Pictures folder: `~/Pictures/<game name>` on
Linux (or wherever `XDG_PICTURES_DIR` points, e.g. `~/Bilder`), `Pictures\<game name>` on Windows,
`~/Pictures/<game name>` on macOS; when that can't be written, a `photos` folder next to the save
games. The browser version offers each picture as a download.

## Where saves are kept

| System | Save games |
|---|---|
| Linux | `~/.local/share/<game-name>/saves` (Flatpak: `~/.var/app/<app id>/data/<game-name>/saves`) |
| Windows | `%APPDATA%\<game-name>\saves` |
| macOS | `~/Library/Application Support/<game-name>/saves` |
| Web | The browser's storage for the site |
