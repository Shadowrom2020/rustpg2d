# 17. Testing your game

Besides playing it, a game can be **driven by a script**: the game runner presses keys, clicks,
waits, takes screenshots and prints its state. That is how the editor's AI agents play-test
(`play_test`, [chapter 13](13-ai-agents.md)), and how you can check that a quest still works after
a change without playing it through by hand. From a terminal:

```sh
RUSTPG2D_AUTOPLAY="wait 1; talkto tilda; wait 3; status; shot /tmp/tilda.png; quit" \
  rustpg2d-game path/to/project --no-title --map shop --spawn entrance
```

`--no-title` skips the title screen; `--map` / `--spawn` (or `--pos X,Y` in pixels) start
somewhere specific; `--dev` adds the developer console and live script reloading
(play-tests from the editor use it). A crafted save in `saves/quick.ron` loaded with `press f9`
starts in a precise situation (loading never fires *Map enter* triggers).

## How a script runs

The game runner plays a script when it is started with `RUSTPG2D_AUTOPLAY="cmd; cmd; …"` —
the editor's play-tests from agents (`play_test`) and the project's own test runs use it. Commands
are separated by `;`, words by spaces; they run in order, each `wait` and `hold` taking its time,
everything else one frame. Output goes to stdout: `[autoplay] …` lines from `status`, the game's
own `[game]` / `[events]` messages, and `error:` lines.

## Commands

| Command | Does |
|---|---|
| `wait SECS` | Waits (dialog text types out over ~2.5 s; wait before reading it). |
| `press KEY[+KEY]` | Presses keys for one frame: letters, `enter`, `escape`, `space`, `tab`, `up` `down` `left` `right`, `f1`…`f12`, `shift+g` (Shift held). `e` interacts / confirms (first press completes a dialog page's text, the second closes it), `escape` opens the pause menu, `c`/`i` the character window, `k` skills, `l` spells, `m` the map, `h` hides the HUD, `x` attacks, `f5` quick-saves, `f9` quick-loads, `q` swaps weapons. |
| `hold KEY SECS` | Holds keys (movement: `hold right 0.5` walks about one tile per 0.2 s). |
| `pad BUTTON[+BUTTON]` / `padhold BUTTON SECS` | Gamepad: `a b x y lb rb lt rt select start ls rs up down left right`. |
| `click X Y` / `rclick X Y` | Left / right click on tile (X, Y) of the map; right-click walks there. |
| `clickenemy` | Clicks the nearest enemy. |
| `type TEXT` | Types text (the character creator's name field). |
| `hud window\|map` | Clicks the HUD's character-window or map button. |
| `wintab character\|skills\|spells` | A tab of the open character window. |
| `slot N` / `slotdrag A B` / `slotdrag player N out` | Inventory slots: use / move / drop. |
| `shop buy\|sell N` | Row N of the open shop. |
| `opt ROW prev\|next\|row` | The Options page's arrows. |
| `minigame hit\|miss\|bite` | Press E at the sweet spot / far from it / when a fish bites. |
| `craft K` | The Make button of recipe row K in the open crafting window. |
| `mapbtn close\|zoomin\|zoomout\|center` | Buttons of the map screen. |
| `drag X1 Y1 X2 Y2` / `wheel DY` | Mouse drag / wheel in screen pixels (the map screen). |
| `console LINE` | A developer console line (the game runs with `--dev` in play-tests): `call FN`, `give ITEM N`, `take ITEM N`, `flag NAME on\|off`, `var NAME V`, `settime H`, `teleport MAP SPAWN`, `hp N`, `xp N`, `reload`, or any Rhai expression. |
| `settime H` / `settime +H` | Sets the clock / lets H hours pass (days pass, props regrow). |
| `skillpoint ID` / `learn ID` / `cast ID` / `call FN` | Spend a skill point, learn / cast a spell, call a script function. |
| `spawners` | Prints the spawner zones' state. |
| `close` | The open window's close button. |
| `talkto NAME` | Right-clicks the character named NAME where it stands now: the player walks there and talks (characters that wander). |
| `status` | Prints one `[autoplay]` line with the game state (below). |
| `shot NAME` | A PNG screenshot (play_test returns it as an image). |
| `quit` | Ends the run (play_test appends it). |

A fresh game starts with the title screen unless `--no-title` (play_test's default): then it
begins on the chosen map and spawn, with the map's *Map enter* triggers and cut-scenes
(`cutscene=` in `status` while one plays; `press e` advances its dialogs).

## The `status` line

`[autoplay] dev=on|off fps=N frame_ms=avg/worst update_ms=N draw_ms=N astar=searches/nodes … menu=title|pause|…|- autosave=none|pending|saved cutscene=NAME|- ui_hidden=… window=closed|… npcs=[…] worldmap={…} skills={points=N skills={..} spells={..} mana=N/M …} xp=N level=N effects=[…] ai=[…] bestiary=[…] dialog_log=N pad=… attacking=… time=HH:MM light=… weather=kind:intensity … companions=[…] player="Name|Sex|body" creator=… quests=[id:stage …] achievements=[…] minigame=… climbing=true|false props=[def:state@x,y] crops=[…] lang=CODE map=NAME tile=(X,Y) hp=Some(N) vars={…} inv={item: count …} flags={…} music=… dialog=true|false busy=true|false enemies=N sounds=[…] foes=[name:hp/max@x,y …] device=Keys|Pad pad_style=xbox|playstation|nintendo attacks=[name:windup|swing|lunge|recover …]`

The useful fields for checking content: `map` and `tile` (where the player is), `dialog` (a
dialog is open), `cutscene`, `quests` (quest id → stage), `inv`, `flags`, `vars`, `hp`,
`props` (props not in their first state), `npcs` (characters on the map and their gear),
`enemies`, `menu` (`defeated` after a death), `autosave`, `schedule` (scheduled characters:
name:stop-time[>map](walking|inside|elsewhere)[:sit|:lie]@x,y), `sounds` (the last six sound
effects started, also when the game is muted: a bell or an animal's call), `climbing` (the
player is on a ladder tile), `foes` (the hostile characters on the map with their HP and tile:
whether a shot hit), `device` (the device used last: hints show keys or pad buttons; `pad a`
switches to the pad, `press` or a click back), `pad_style` (how buttons are drawn) and `attacks`
(hostile characters winding up, swinging or lunging, recovering).

For performance checks: `fps`, `frame_ms` (average / worst frame), `update_ms` and `draw_ms`
(average time of the two halves of a frame) and `astar` (path searches and the nodes they
expanded), all since the previous `status`, so `wait 10; status` measures ten seconds. With
`RUSTPG2D_PERF=1` the game also logs the same as a `[perf] … entities=N` line every second.

## Tips for play-tests

- Park the mouse first (`drag 2 2 2 2`), or the real pointer over the window moves the menu
  selection.
- Talk to a character: stand next to them facing them (`hold right 0.3`), `press e`, `wait 2.5`,
  `press e` (completes the text), `wait 0.5`, `press e` (closes), then `status`.
- A `Choice`: `press down` / `press up` select, `press e` confirms.
- Start at a precise tile with `pos` (play_test) instead of walking there.
- Keep runs short (`timeout_secs`): the game window is on the person's screen meanwhile.
