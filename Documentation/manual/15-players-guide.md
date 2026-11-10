# 15. The player's guide

How a game made with R(ust)PG2D is played. Use it to write your game's own instructions, or point your
players here.

## Keyboard and mouse

These are the default keys; players change them in *Options › Controls*.

| Key | Action |
|---|---|
| Arrows / WASD (Shift = run) | Move |
| E / Space / Enter | Interact, talk, advance a dialog |
| X | Attack in the facing direction |
| Q | Switch weapon (bare hands, then the weapons you own) |
| 1–8, or a click on a quick bar slot | Use that item or spell |
| C / I | The character window: character sheet and inventory |
| K / L | The character window's *Skills* and *Spells* tabs (if the game has them) |
| Tab | Next tab of the open window |
| J | Journal: *Quests*, *Bestiary*, *Dialogs* |
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

## Gamepad

Any controller the system knows works next to the keyboard and mouse (Xbox, PlayStation, Switch Pro,
generic ones); the left stick and the d-pad move and navigate.

| Button (Xbox names) | Action |
|---|---|
| Left stick / d-pad | Move; navigate menus, windows, the map |
| A | Interact, talk, advance, choose, use, buy / sell |
| B | Close a window, go back |
| X | Attack (while aiming a spell: cast straight ahead) |
| Y | Character window |
| RB / LB | Next / previous tab; on the map: zoom; while playing, LB opens companion orders |
| LT (hold) | Run; buy / sell ten |
| RT | Switch weapon |
| Start | Pause menu |
| Select | Map |
| Right stick click | Journal |
| Left stick click | Hide or show the interface |

## Menus

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
| Master, Music, Effects, Ambience volume | In 10 % steps (ambience = sounds like the rain) |
| Text speed | Slow, Normal, Fast, Instant |
| Shake & flash | The screen shake and red flash when hurt, and shakes from explosions |
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
