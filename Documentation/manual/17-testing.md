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
| `keydown KEY[+KEY]` / `keyup [KEY]` | Holds keys while the next commands run (`keydown f; wait 0.3; shot …; keyup`); `keyup` without a key lets go of all. |
| `pad BUTTON[+BUTTON]` / `padhold BUTTON SECS` | Gamepad: `a b x y lb rb lt rt select start ls rs up down left right`. |
| `click X Y` / `rclick X Y` | Left / right click on tile (X, Y) of the map; right-click walks there. |
| `rhold X Y SECS` | Right-click tile (X, Y) and keep the button down for SECS seconds (*Hold to repeat* on a tree or a rock). |
| `clickenemy` | Clicks the nearest enemy. |
| `type TEXT` | Types text (the character creator's name field). |
| `hud window\|map` | Clicks the HUD's character-window or map button. |
| `wintab character\|skills\|spells` | A tab of the open character window (also `graphics`, `sound`, `gameplay`, `accessibility` on the Options page, and the journal's tabs). |
| `slot N` / `slotdrag A B` / `slotdrag player N out` | Inventory slots: use / move / drop. |
| `shop buy\|sell N` | Row N of the open shop. |
| `opt ROW prev\|next\|row` | The Options page's arrows. |
| `minigame bite` | Press E when a fish bites (the fishing game; the lockpicking game of 0.27 and older became a DEX check). |
| `craft K` | The Make button of recipe row K in the open crafting window. |
| `mapbtn close\|zoomin\|zoomout\|center` | Buttons of the map screen. |
| `drag X1 Y1 X2 Y2` / `wheel DY` | Mouse drag / wheel in screen pixels (the map screen). |
| `console LINE` | A developer console line (the game runs with `--dev` in play-tests): `call FN`, `give ITEM N`, `take ITEM N`, `flag NAME on\|off`, `var NAME V`, `settime H`, `teleport MAP SPAWN`, `hp N`, `xp N`, `reload`, or any Rhai expression. |
| `settime H` / `settime +H` | Sets the clock / lets H hours pass (days pass, props regrow). |
| `seed N` / `learn ID` / `cast ID` / `call FN` | Make the SRD dice roll the same from here on (also `RUSTPG2D_SEED=N`; automated runs start with seed 1), learn / cast a spell, call a script function. |
| `crit always\|never\|chance` | The player's critical hits: forced on, forced off, or by the weapon's chance again. |
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

`[autoplay] dev=on|off fps=N frame_ms=avg/worst update_ms=N draw_ms=N astar=searches/nodes … menu=title|pause|options:TAB|…|- autosave=none|pending|saved cutscene=NAME|- ui_hidden=… battle=off|on:SECS[who in combat] music_fade=GAIN[>next][<old:GAIN] (`<old:GAIN` while a crossfade fades the old track out) music_effect=-|ID:SECS_LEFT@DUCK|-@DUCK (a fanfare playing and how loud the music is under it) captions=[text, <left, right> …] photo=off|on:ZOOMx:HHh[:hidden]@x,y [photo_last=PATH] window=closed|… npcs=[…] worldmap={…} skills={spells={..} mana=N/M …} xp=N level=N effects=[…] ai=[def:hunting|attack|circle|fleeing|regroup|returning@x,y …] bestiary=[…] dialog_log=N pad=… attacking=… time=HH:MM light=… weather=kind:intensity … companions=[…] player="Name|Sex|body" race=id(hp±N,speed±N%,def±N,sizeN)|- factions=playerfaction[faction:standing …]{name=faction[*] …}|- hooks=[hook:calls …]|- (how often each script hook ran its functions; `-` without hooks) plugins=[id …] (the plugins the game loaded, in order) creator="Name|race|Sex|figure colors{feature:RRGGBB …} rows Name/Race/…" quests=[id:stage …] achievements=[…] minigame=… climbing=true|false props=[def:state@x,y] crops=[…] doors=[name:open|closed[:locked]@x,y …] homes=[map …] furniture=[map:prop@x,y …] lang=CODE fonts=[role:font|builtin …] missing_glyphs=N (characters no font had, drawn as empty boxes) skin=default|ID[:contrast][!sheet:frame …] (the UI skin: `:contrast` while high contrast shows the built-in look instead, `!sheet:frame` per picture that wasn't found) menu_rows=[label …] (the open menu page's rows, the selected one starting with `>`) title_bg=map:NAME@X,Y|spawn:HOUR|-|picture:SHEET (the title screen's background) hud=[id:text:"…"|image:SHEET#FRAME|bar:VALUE/MAX|minimap:SIZExTILES@anchor+x,y …] (the HUD elements scripts set, plugins' as plugin/id) pictures=[layer:sheet@x,y:opacity[>] …] (picture layers; `>` while moving) screen=-|tint:r,g,b,a;flash:a;cover:a (screen tint, flash, fade) timers=[name:seconds[+shown] …] input=-|number:VAR=typed|text:VAR="typed" (the open Input number / Input text window; answer it with `type`, `press backspace`, `press enter`, `pad up` / `pad a`) texts={name: "value" …} (text variables) routes=[who:step/steps:summary[+repeat] …] (move routes) common=[parallel:ID autorun:ID …] (common events running by themselves: parallel ones with a running queue, autorun ones whose flag is set) camera=player|tile:X,Y|follow:NAME[@ZOOMx][>] (what the camera looks at; `>` while it moves) scroll=-|ROLLED/TOTAL (Scroll text: pixels rolled of the whole way; `keydown e` speeds it up, `press escape` skips it) script_window=ID["line", "[sheet]", "<Button>", …]|- (the window a script opened; the selected button starts with `>`) map=NAME tile=(X,Y) hp=Some(N) vars={…} inv={item: count …} bag=used/slots flags={…} music=… dialog=true|false dialog_text="…" (the open dialog's text without message codes) busy=true|false enemies=N sounds=[…] foes=[name:hp/max@x,y …] device=Keys|Pad pad_style=xbox|playstation|nintendo attacks=[name:windup|swing|lunge|recover …] block=on|parry|roll|off dodge=ready|N.N emotes=[who:emotion …] barks=[who:line …] ambience=[sound:loudness …] friends=[Name:points …] markers=[quest@x,y | quest>map@x,y …] (quest markers shown: at the target's tile, or at the exit to its map; empty when switched off) (variables friend_<name>, the journal's People page) (the ambience loops heard here, also when muted; footsteps show in sounds=) hitstop=SECS/COUNT numbers=[3 6! -2 …] anims=[player:walk|idle|attackN|strike|windup|swing|block|hurt name:… (characters within 8 tiles; `>attack_thrust` etc. when their weapon or shield chose a weapon animation; a diagonal walker adds its direction: walk@se, @sw, @nw, @ne)] held=[who:item@R|L|B[/back] who:item@back …] (what the player and characters within 8 tiles hold, in which hand; `@back` = a backpack worn, `@hip` = a one-handed weapon sheathed out of combat) models=poses/pages/most pages p/renders this frame r/renders in all/pages cleared e rolls=[persuasion:16/12:ok, dex:thieves-tools:9/12:fail:nat1, 2d6:7 …] abilities=STR15,DEX14,… prof=+2 profs=athletics,perception,…` (3D characters: `RUSTPG2D_MODEL_DEBUG=1` shows the first atlas page of rendered poses; the most pages come from *Character pose memory*) (`numbers=`: the last six damage numbers, `!` = critical, `-` = the player or a companion hurt; the text a `damage_number` hook gave when it changed it) (`rolls=`: the last six SRD rolls, oldest first: a check or saving throw as `what:total/DC:ok|fail` with `:nat20` / `:nat1` when the d20 showed it, a dice roll as `dice:total`; `abilities=` the hero's six scores, `prof=` the proficiency bonus, `profs=` the hero's proficiencies) (`factions=`: the player's faction and standing with every faction of the project, then in braces the characters on the map whose faction isn't their definition's (`*` = changed in play by *Set faction*), `-` without factions; `foes=` lists the characters hostile to the player)

The useful fields for checking content: `map` and `tile` (where the player is), `dialog` (a
dialog is open), `cutscene`, `quests` (quest id → stage), `inv`, `flags`, `vars`, `hp`,
`props` (props not in their first state), `npcs` (characters on the map and their gear),
`enemies`, `menu` (`defeated` after a death), `autosave`, `schedule` (scheduled characters:
name:stop-time[>map](walking|inside|elsewhere)[:sit|:lie]@x,y), `sounds` (the last six sound
effects started, also when the game is muted: a bell or an animal's call), `climbing` (the
player is on a ladder), `foes` (the hostile characters on the map with their HP and tile:
whether a shot hit), `device` (the device used last: hints show keys or pad buttons; `pad a`
switches to the pad, `press` or a click back), `pad_style` (how buttons are drawn) and `attacks`
(hostile characters winding up, swinging or lunging, recovering), `block` (the player's shield:
`parry` in its first 0.2 s, `roll` while dodging) and `dodge` (seconds until the next roll), `emotes` (emotions shown over heads now; `player:joy`), `ground` (the painted material under the
player's feet and how many materials the map uses: `sand/9`, `-/0` without painted ground).

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
