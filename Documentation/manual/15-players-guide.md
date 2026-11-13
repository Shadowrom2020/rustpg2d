# 15. The player's guide

How a game made with R(ust)PG2D is played. Use it to write your game's own instructions, or point your
players here.

## Keyboard and mouse

These are the default keys; players change them in *Options › Controls*.

| Key | Action |
|---|---|
| Arrows / WASD (Shift = run) | Move |
| E / Enter | Interact, talk, advance a dialog |
| X | Attack in the facing direction |
| F (hold) | Block with a shield (see below) |
| Space | Dodge roll |
| Q | Switch weapon (bare hands, then the weapons you own) |
| 1–8, or a click on a quick bar slot | Use that item or spell |
| C / I | The character window: character sheet and inventory |
| K / L | The character window's *Skills* and *Spells* tabs (if the game has them) |
| Tab | Next tab of the open window |
| J | Journal: *Quests*, *Bestiary*, *Dialogs*, *People* (the villagers you made friends with, in hearts) |
| M | The map screen |
| G | Companion orders (with companions along) |
| H | Hide or show the interface |
| F5 / F9 | Quick save / quick load |
| Esc | Pause menu; closes windows |
| Left click | Walk to the tile, attack the enemy under the cursor, advance a dialog |
| Right click | Walk to the character, item or prop under the cursor and interact with it |

Characters behind a counter (one solid tile between, in a straight line) are talked to across it,
with E or a right-click.

### In the windows

- **Character window**: arrows or a click select; Enter, a double-click or a right-click uses or
  equips; drag items onto or off the equipment slots, onto the quick bar, or out of the window to
  drop them; G drops the selected stack (Shift+G one item).
- **Shop**: click *Buy* or *Sell* (or double-click a row, or Enter / E) to trade one, Shift for ten;
  Tab switches between the merchant's goods and your items.
- **Loot window**: click a stack to take it (right-click: one), click your own to store it; T takes
  everything; Tab switches side.
- **Crafting window**: *Make* (or Enter / E) makes one, Shift up to ten.
- **Map screen**: arrows, dragging or the wheel scroll; + / − zoom; Space centres on you.
- **Journal**: Tab or Left / Right switch tabs; the *Dialogs* tab scrolls with Up / Down, PgUp / PgDn,
  Home / End.

## Defending yourself

**Block**: hold F (gamepad: RB) with a shield in your left hand. You walk at half speed and can't
attack, but blows, arrows and lunges coming **from the front** do no damage, only push you back a
little. Raise the shield just before a blow lands (the last moment of the enemy's wind-up) to
**parry**: the attacker is stunned for a second. Without a shield, F only tells you that you need
one.

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
| Left stick click | Hide or show the interface |

**Hints follow your hands.** The help lines at the bottom of windows and menus name the keys while
you play with the keyboard and mouse, and show your controller's buttons as soon as you press one
(and back when you press a key or click). The pictures match the controller: coloured A B X Y for
Xbox pads and the Steam Deck, ✕ ○ □ △ for PlayStation, the Nintendo letters for Switch pads. If the
game guesses wrong, choose it in *Options › Button style* (Auto, Xbox, PlayStation, Nintendo).

**Rebinding buttons.** In *Options › Controls* every action shows its button next to its keys. Select
an action, press A (Enter on the keyboard) and then the button you want for it; X clears the
action's button, *Reset to defaults* brings back all keys and buttons. B and Start keep the old
buttons; moving stays on the stick and the d-pad. A button that does something else in the same
place is named under the list (*R2 is also Switch weapon*).

## Menus

- **Character creator** (after *New game*, when the game has one): type a name, then Up / Down
  choose a row and Left / Right change it: race (each with its own strengths, described under the
  picture), sex, body and colours (skin, hair, clothing…). Enter starts the adventure, Esc goes back.
- **Title screen**: *New game*, *Continue* (the latest save), *Load game*, *Options*, *Achievements*
  (if the game has them), *Exit*.
- **Pause menu** (Esc): *Resume*, *Save game* (a new named save or overwrite one), *Load game* (every
  save with a small picture of the world, the map, the play time and its age; Del deletes), *Options*,
  *Save and quit*, *Quit* (after a confirmation).
- Closing the window while playing asks *Save and exit*, *Exit without saving* or *Cancel*.

## Options

| Option | Choices |
|---|---|
| Fullscreen | On / off |
| Zoom | 75–250 % on top of the game's own pixel scale |
| Show FPS | On / off |
| Master, Music, Effects, Ambience volume | In 10 % steps (ambience = the rain and the sounds of places: birds, rivers, a tavern's murmur) |
| Text speed | Slow, Normal, Fast, Instant |
| Shake & flash | The screen shake and red flash when hurt, shakes from explosions, and the short pause when a blow lands |
| Damage numbers | On / off: the numbers that rise from hits |
| Language | The game's languages |
| Controls… | Every action with its keys: Enter, then a key, adds one (up to three per action); Del clears; *Reset to defaults* |

Options are saved per game and kept between sessions and save games: in `~/.config/rustpg2d/<game>/settings.ron` on Linux, `%APPDATA%\rustpg2d\<game>` on Windows, `~/Library/Application Support/rustpg2d/<game>` on macOS, and in the browser for web games. Achievements are kept there too, not in save games.

## Where saves are kept

| System | Save games |
|---|---|
| Linux | `~/.local/share/<game-name>/saves` (Flatpak: `~/.var/app/<app id>/data/<game-name>/saves`) |
| Windows | `%APPDATA%\<game-name>\saves` |
| macOS | `~/Library/Application Support/<game-name>/saves` |
| Web | The browser's storage for the site |
