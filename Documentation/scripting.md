# Scripting in R(ust)PG2D

R(ust)PG2D games are mostly built in the editor: maps, characters, items, triggers and their
actions. **Scripts** are for everything the editor's actions can't express on their own:
conversations that remember what was said, counters, quests with conditions, scripted skills
and spells, traps and other effects.

Scripts are written in [Rhai](https://rhai.rs/book/), a small scripting language with a syntax
close to Rust and JavaScript. This document covers how scripts fit into a game, the rules they
follow, and every function the engine provides.

## Contents

1. [A first script](#a-first-script)
2. [Where scripts live](#where-scripts-live)
3. [How scripts are run](#how-scripts-are-run)
4. [Immediate and queued functions](#immediate-and-queued-functions)
5. [Game state: flags and variables](#game-state-flags-and-variables)
6. [Targets and `SELF`](#targets-and-self)
7. [Positions, numbers and text](#positions-numbers-and-text)
8. [Writing scripts in the editor](#writing-scripts-in-the-editor)
9. [API reference](#api-reference)
10. [Recipes](#recipes)
11. [Limits, errors and debugging](#limits-errors-and-debugging)
12. [Rhai in five minutes](#rhai-in-five-minutes)

---

## A first script

A shrine that does nothing until the player has accepted a quest, and then lights up:

```rust
fn shrine_touch() {
    if !flag("quest_accepted") {
        say("It seems dormant.");
        return;
    }
    say("Ancient runes begin to glow!");
    set_flag("shrine_lit", true);
    set_state("self", "glowing");
    emit("shrine_lit");
}
```

To use it, give the shrine prop a trigger in the editor: **On Interact**, action **Run script**,
function `shrine_touch`. When the player presses E in front of the shrine, the game calls the
function.

## Where scripts live

- Scripts are the `*.rhai` files in the project's `scripts/` folder (in the example project:
  `example_project/scripts/main.rhai`). Edit them in the editor under **Database › Scripts**, or
  with any text editor.
- All files are loaded together when the game starts, as if they were one file. A function in
  one file can call a function in another, and every function name must be unique in the
  project. (If two files define the same name, the file that comes later alphabetically wins.)
- Put only **functions** (and comments) at the top level of a file. Statements outside functions
  don't keep values between calls; keep lasting state in [flags and variables](#game-state-flags-and-variables).
- Scripts are part of an exported game (*File › Export game*).
- **Asset library and packages**: a spell, skill or other asset that uses scripts can go into
  the asset library or a `.r2dpack` package together with its functions. The editor asks which
  functions to copy, with the ones the asset needs (and everything they call or pass on as a
  callback) ticked. A project that takes the asset gets the functions appended to
  `scripts/library.rhai`; functions it already has are kept. Keep a spell's functions
  self-contained (helpers they call, no project-specific flags) so they work in other games.

## How scripts are run

The game never runs a script on its own. Something in the game calls a script **function**:

| Caller | Set up in the editor | How the function is called |
|---|---|---|
| **Run script** action | Any trigger: on a map, zone, character, prop, item pickup or an item's *On use* | `name()`, `SELF` = the trigger's owner |
| **Script** condition | The *If all of* list of any trigger, or an *If* action | `name()`, must return `true` / `false` |
| Skill with the **Script** effect | *Database › Skills* | `name(level)` whenever the skill's level changes, and once when a game starts or is loaded |
| Spell with the **Script** effect | *Database › Spells* | `name()` when the spell is cast |
| `then_call("name")` | In a script | `name()` after the actions queued before it |
| `aim("name")` | In a script | `name(x, y)` with the target the player clicked |
| `fly(…, "name", …)` | In a script | `name(x, y)` when the flying sprite arrives |

Triggers fire on these events: **Interact** (the player presses E in front of an entity, or
right-clicks it), **Enter** / **Leave** (a zone), **Use item**, **Map enter**, **Custom event**
(sent by `emit`), **Defeat** (a character was defeated), **Unlock** / **Lock** (a prop's lock).
Loading a saved game does not fire *Map enter*.

A script function called by a trigger runs in the middle of that trigger: the actions the
function queues run **before** the trigger's remaining actions.

### Conditions

A function used as a **Script** condition decides whether a trigger runs. It must return a
`bool`, and the last expression of a function is its result:

```rust
fn has_enough_coins() {
    item_count("coin") >= 10
}
```

A condition should only look at the game, not change it. Actions it queues (`say`, `teleport`,
…) are thrown away.

## Immediate and queued functions

This is the most important rule of R(ust)PG2D scripting.

- **Immediate** functions read or change the game state at once, while the function runs:
  `flag`, `set_flag`, `get_var`, `set_var`, `add_var`, `item_count`, `has_item`, `give_item`,
  `take_item`, and all functions that only read something (`player_hp()`, `hour()`, `skill(…)`,
  …).
- **Queued** functions don't do anything while the function runs. They add an action to a queue,
  and the game performs the queue **after the function has returned, in order**: `say`,
  `choice`, `teleport`, `move_by`, `wait`, `heal`, `start_quest`, `play_sound`, `fly`, … (the
  [reference](#api-reference) marks every queued function).

Some queued actions **block** the queue until they are finished: a dialog waits for the player,
`choice` waits for an answer, `move_by` waits until the character arrived, `wait` waits, and
`teleport` waits for the screen fade. The game keeps running meanwhile (enemies still move), but
the queue only continues afterwards.

Because of this, the function itself never sees the result of a queued action:

```rust
fn ask() {
    choice("Buy a potion for 5 coins?", ["Yes", "No"], "buy");
    // Wrong here: the player hasn't answered yet, "buy" still has its old value.
    then_call("after_ask");          // runs once the answer is in
}

fn after_ask() {
    if get_var("buy") == 0 {         // 0 = the first option, "Yes"
        if item_count("coin") >= 5 {     // check first: take_item takes what there is
            take_item("coin", 5);
            give_item("potion", 1);
            say("Here you are.");
        } else {
            say("You can't afford it.");
        }
    }
}
```

`then_call("name")` queues a call of another function. Use it to continue after anything that
blocks: an answer, a dialog, a walk.

`queue_set_var(name, value)` is the queued twin of `set_var`: it changes a variable at its place
in the queue, e.g. after a dialog.

## Game state: flags and variables

Everything a game must remember lives in the **save game**. Scripts keep their state in two
kinds of named values, shared with the editor's actions and conditions:

- **Flags**: yes / no. `flag("met_elder")`, `set_flag("met_elder", true)`. Unset flags are
  `false`.
- **Variables**: whole numbers. `get_var("slimes_defeated")`, `set_var("gold_owed", 30)`,
  `add_var("slimes_defeated", 1)`. Unset variables are `0`.

Names are free text; pick clear ones (`pest_slimes`, `mira_talks`). They appear in the editor's
flag and variable suggestions as soon as a script uses them.

Rhai variables (`let x = …`) only live while a function runs. Anything that must survive until
the next call, or a save and load, goes into a flag or variable.

The rest of the game state (inventory, quests, skills, spells, mana, opened chests, defeated
enemies, …) is saved too, and scripts change it through the API functions.

## Targets and `SELF`

Functions that act on an entity take a **target**:

- `"self"` — the entity that owns the trigger which called the script (the shrine above);
- `"player"` — the player;
- any other text — the entity with that **name** on the current map (set the name in the
  editor's inspector, e.g. `mira`, `guard`).

`SELF` is a number: the owner's id (uid), or `-1` when there is no owner (spells, skills,
callbacks). `say(SELF, "…")` shows the owner's name as the speaker.

## Positions, numbers and text

- **Positions** are in **tiles**: `x` from the left, `y` from the top of the map. Fractions are
  allowed: `10.5` is the middle of tile 10. `player_x()` / `player_y()` are where the player's
  feet are; the player's chest is about `player_y() - 0.7`.
- **Radii** and **speeds** are in tiles (tiles per second), **times** in seconds.
- Positions, radii, speeds and times can be integers or decimals (`light_at(3, 4.5, 2, 10)`).
  Counts, amounts, hit points, levels, quest stages and tile offsets are **whole numbers**
  (`give_item("coin", 3)`, `heal(2)`, `move_by("mira", 2, 0)`). Rhai doesn't convert decimals
  to integers by itself; use `to_int()` if needed: `heal((max / 2.0).to_int())`.
- **Ids** (items, maps, spawn points, sounds, quests, skills, spells, sprite sheets, prop states)
  are strings exactly as in the editor's database: `give_item("potion", 1)`.
- **Text placeholders** work in `say`, `choice` and with `text(…)`:
  - `{player}` — the player's name;
  - `{sir|madam}` — the first word for a male player character, the second for a female one
    (`{his|her}`, `{hero|heroine}`, …);
  - `{name}` — the value of the variable `name`, e.g. `"You have defeated {slimes_defeated} slimes."`.
- **Translations**: the texts of `say(…)` and `choice(…)` are looked up in the game's language
  files (`lang/<code>.ron`, *Database › Project › Languages*) like dialogs in triggers, by the
  exact text before the placeholders are filled in. Write a text as **one string with
  placeholders**, `say("Mira", "Hi, {player}! You came {visits} times.")`, not glued together
  (`"Hi, " + player_name() + "!"`): a glued text only exists while the game runs, so the
  translation template can't list it and no translation matches it.

## Writing scripts in the editor

**Database › Scripts** lists the script files and their functions. Click a file to edit it, or a
function to jump to it. **+ New** creates a file, **Save & check** (Ctrl+S) saves and reports
syntax errors.

- **Completion**: while you type, the editor suggests engine functions (with their parameters),
  your own functions, keywords and `let` variables. Ctrl+Space shows all suggestions; ↑ ↓ choose,
  Tab or Enter accept, Esc closes.
- **Arguments**: inside the quotes of an id argument the editor offers exactly what fits:
  items for `give_item("`, maps and then that map's spawn points for `teleport(`, sprite sheets
  for `fly(` and `effect(`, script functions for `then_call(`, `aim(` and `fly`'s callback, quests,
  sounds, flags, variables and targets. Right after `(` or `,` the list opens at once and
  accepting inserts the id with its quotes.
- **Parameter hint**: the line above the editor shows the call around the cursor with the current
  parameter highlighted and a short description, for engine functions and your own functions.
- **Ctrl+click** a function name to jump to its definition, also in another file.
- **Problems**: ids that don't exist in the project (a misspelled item, a missing spawn point, an
  unknown sprite sheet) are listed under the file list and underlined in red.
- **Renames**: renaming an item, map, sound or sprite sheet in the editor also updates the ids in
  scripts, where they are passed directly to an API function.

### Trying scripts while the game runs

Play-tests started from the editor (F5 / F6, *Play as cutscene*) run the game with `--dev`:

- **Hot reload**: saving a script (Ctrl+S in the script editor, or any editor writing
  `scripts/*.rhai`) reloads all scripts within half a second; a toast says *Scripts reloaded*.
  Triggers, conditions and spells use the new functions from then on. If a file has an error,
  the game keeps running the previous scripts and shows the error (toast and console) until it
  is fixed.
- **Console** (F4): pauses the game and takes commands. `call FN` runs a script function,
  `give ITEM [N]` / `take ITEM [N]`, `flag NAME on|off`, `var NAME V`, `settime H`,
  `teleport MAP SPAWN`, `hp N`, `xp N`, `effects`, `status`, `reload`, `clear`, `help`. Any other
  line is run as Rhai with your functions available, and its value is printed:

  ```text
  > item_count("coin")
  12
  > set_flag("shrine_lit", true)
  > give_item("potion", 3); quest_stage("pest_control")
  1
  ```

  `print(...)` output and script errors appear in the console too. Up / Down bring back earlier
  lines, Esc clears the line (and closes the console when it is empty), F4 closes it. Actions
  that take time (dialogs, teleports, walking) run once the console is closed.

Exported games ignore `--dev`: players never get the console.

---

## API reference

*(queued)* marks functions that add an action to the queue (see
[Immediate and queued functions](#immediate-and-queued-functions)). All others act immediately
or only read something. Names in `code` are parameters; overloads are listed separately.

### Flags and variables

| Function | Description |
|---|---|
| `flag(name)` | Whether a flag is set (`false` if it was never set). |
| `set_flag(name, value)` | Sets (`true`) or clears (`false`) a flag. |
| `get_var(name)` | Value of a variable (`0` if unset). |
| `set_var(name, value)` | Sets a variable to a whole number. |
| `add_var(name, amount)` | Adds to a variable (negative subtracts). |
| `queue_set_var(name, value)` | Sets a variable when the queue reaches it, e.g. after a dialog. *(queued)* |

### Inventory

| Function | Description |
|---|---|
| `item_count(item)` | How many of an item the player carries. |
| `has_item(item)` | Whether the player carries at least one. |
| `give_item(item, count)` | Gives items to the player. Returns how many did **not** fit into the inventory (`0` = all fit). |
| `take_item(item, count)` | Takes up to `count` items from the player (also from the equipment slots). Returns how many were taken, which can be fewer: check `item_count` first when the player must pay. |

### Player

| Function | Description |
|---|---|
| `player_name()` | The name the player chose. |
| `player_sex()` | `"male"` or `"female"`. |
| `is_female()` / `is_male()` | The player character's sex. |
| `player_hp()` | Current hit points (2 per heart). |
| `player_max_hp()` | Maximum hit points. |
| `player_x()` / `player_y()` | Position in tiles (fractional; the feet). |
| `player_facing()` | `"up"`, `"down"`, `"left"` or `"right"`. |
| `heal(amount)` | Restores hit points; a negative amount hurts the player. *(queued)* |
| `heal_npc(name, amount)` | Restores a character's hit points (a companion, any character with HP; `"player"` = the player); a negative amount hurts it. *(queued)* |
| `current_map()` | Name of the current map. |

### Dialogs and flow

| Function | Description |
|---|---|
| `say(text)` | Shows a dialog box and waits for the player. *(queued, blocks)* |
| `say(speaker, text)` | Dialog with a speaker name. The dialog shows the speaker's portrait when the name fits a character: `"{player}"` (or the player's name) for the hero's own lines, a character on the map or a character definition otherwise (*Dialog portraits* in the README). *(queued, blocks)* |
| `say(entity, text)` | Dialog with an entity's name as speaker: `say(SELF, "…")`. *(queued, blocks)* |
| `say(speaker, text, options)` | The same with options: `#{ mood: "worried" }` shows the speaker's portrait for that mood (a *Moods* entry of its character definition) for this line, e.g. `say("Elder Rowan", "The lights are back...", #{ mood: "worried" });`. Also `say(SELF, text, options)`. *(queued, blocks)* |
| `mood(target, mood)` | A character's portrait shows that mood (its definition's mood portrait) from now on, until changed or the running event is over; `""` = its usual portrait. `target`: a character on the map, or a character definition's id or name. *(queued)* |
| `choice(text, options, var)` | Asks a question with answers `["Yes", "No"]`; the chosen index (0 = first) goes into variable `var`. *(queued, blocks)* |
| `wait(seconds)` | Pauses the queue. *(queued, blocks)* |
| `then_call(function)` | Calls another script function when the queue gets there. *(queued)* |
| `emit(event)` | Fires a custom event: triggers *On Custom event* with that name run before the rest of the queue. *(queued)* |
| `text(text)` | Returns the text with [placeholders](#positions-numbers-and-text) filled in. |
| `random(n)` | A random whole number from `0` to `n - 1`. |

### Maps and entities

| Function | Description |
|---|---|
| `teleport(map, spawn)` | Moves the player to a spawn point on a map (also the current one). *(queued, blocks)* |
| `set_active(target, active)` | Shows or hides an entity. Hidden entities don't collide or react. Saved. *(queued)* |
| `set_state(target, state)` | Switches a prop to one of its states (sprite, solid or not). Saved. *(queued)* |
| `set_frame(target, frame)` | Older way to change a prop's look (frame offset); prefer `set_state`. *(queued)* |
| `set_locked(target, locked)` | Locks or unlocks a prop without a key (unlocking applies its *on unlock* effect). *(queued)* |
| `move_by(target, dx, dy)` | Walks a character by whole tiles. *(queued, blocks until it arrives)* |
| `face(target, direction)` | Turns a character: `"up"`, `"down"`, `"left"`, `"right"`. *(queued)* |
| `patrol(target)` | Starts the character's own patrol route (set in the editor). *(queued)* |
| `patrol(target, points)` | Patrols the given waypoints `[[x, y], [x, y], …]` (whole tiles); one point = walk there and stay. *(queued)* |
| `stop_patrol(target)` | Stops a patrol: the character stands where it is. *(queued)* |
| `stop_schedule(target)` | The character stops following its daily schedule (set in the editor) and stays where it is. Saved. *(queued)* |
| `sleep(hours)` | The player sleeps (camping, an inn keeper's "Rest"): the screen fades, the clock moves on, HP and mana are restored, timed effects end, then *Woke up* triggers run. Refused with enemies nearby. *(queued, blocks)* |
| `is_sitting()` / `is_sleeping()` | Whether the player sits on furniture / lies in a bed (or the night is passing). |
| `lockpick(difficulty, var)` | Plays the lockpicking game (like a lock with that *Lockpicking* difficulty; the player's skill makes it easier) and puts 1 (picked) or 0 into the variable, e.g. `lockpick(2, "vault"); then_call("vault_result");`. *(queued, blocks until it is over)* |
| `unlock_achievement(id)` | Unlocks an achievement (*Database › Achievements*) for the player: a toast, kept across save games. *(queued)* |
| `achievement_unlocked(id)` | Whether the player has unlocked an achievement (in any game). |
| `open_crafting(station)` | Opens the crafting window with the recipes of a station (*Database › Recipes*), e.g. `open_crafting("alchemy")` on an alchemy table's trigger. *(queued)* |
| `open_crafting(station, title)` | The same with a window title, e.g. `open_crafting("anvil", "Smithy")`. *(queued)* |
| `can_craft(recipe)` | Whether the player has the ingredients (and the skill level) to make a recipe once. |
| `follow(target)` | The character joins the player as a companion: it follows from map to map and fights hostile characters near the player. Only placed, friendly characters can. Saved. *(queued)* |
| `stop_following(target)` | The companion leaves: on its own map it stays where it is, elsewhere it goes back to where it is placed. *(queued)* |
| `is_companion(name)` | Whether the character with that name follows the player. |
| `companion_count()` | How many companions follow the player. |
| `order(target, order)` | Tells a companion (or `"all"`) what to do, like the player's orders menu (G): `"attack"` (fight enemies near the player, the default), `"defend"` (only what hunts the player or the companion), `"passive"` (no fighting, back away from enemies), `"stay"` (wait where it is, fighting what comes close) or `"follow"`. Saved. *(queued)* |
| `order_of(name)` | How a companion fights: `"attack"`, `"defend"` or `"passive"` (`""` if no companion has that name). |
| `is_staying(name)` | Whether a companion was told to stay where it is. |
| `open_companion(target)` | Opens a companion's window — its gear and bag next to the player's inventory — once the running event is over, e.g. after a dialog's "Let me see your gear". *(queued)* |
| `follow_schedule(target)` | The character follows its daily schedule again: it heads for the stop of the current hour. *(queued)* |
| `open_shop(target)` | Opens a merchant's trade window. *(queued)* |
| `defeated_count(character)` | How many characters of a definition the player has defeated (the bestiary's count), e.g. `defeated_count("slime")`. |
| `hp_of(target)` | A character's HP right now: `"player"`, `"self"` (or `SELF`) or the name of a character on this map; `-1` if there is none (or it can't be hurt). |

A character's *HP below %* trigger runs when its HP falls below that share of its max HP and it
survives the hit, e.g. a boss calling for help at half its HP. It runs again if the character
is healed above the threshold and falls below it once more; tick *once* to prevent that.

```rust
fn ogre_hurt() {                      // the ogre's "HP below 30 %" trigger runs this
    say(SELF, "You will pay for that!");
    if hp_of(SELF) < 3 { apply_effect("self", "strength", 50, 20); }
}
```

### Cut-scenes

A cut-scene is an ordinary script function played with `cutscene("name")` or the trigger action
*Play cutscene*. While it plays the HUD is hidden, black bars slide in at the top and bottom,
and the player and the other characters wait (no input, no wandering or attacking) until the
function's queued actions are done: walks, dialogs, waits, sounds, … Afterwards the HUD is back
(also if the player had hidden it with H before, it stays hidden). A cut-scene started from
another one becomes part of it.

| Function | Description |
|---|---|
| `cutscene(function)` | Plays a script function as a cut-scene. *(queued)* |
| `walk_to(target, x, y)` | Walks a character (`"player"` too) onto tile `x`, `y`. *(queued, blocks until it arrives)* |
| `approach(target, other)` | Walks a character up to another one, stopping a tile before it (along the longer axis); then both face each other. *(queued, blocks)* |
| `place(target, x, y)` | Puts a character (`"player"` too) on tile `x`, `y` at once. Saved for NPCs. *(queued)* |
| `dim(amount, seconds)` | Darkens the scene to `amount` (0 = normal, 1 = deep night) over `seconds`, so that lights (`light_at`, glowing effects, lamps) show even by day. A cut-scene's end brightens it again; not saved. *(queued)* |

`move_by`, `face`, `say`, `choice`, `wait`, `teleport`, `play_sound`, `play_music` and every other
queued function work in cut-scenes as everywhere. Scripted walks go straight to their target
and ignore walls and characters, so a scene can't get stuck - plan the paths. Tile positions are
the editor's tile coordinates (shown in its status bar).

In the editor, the Scripts tab's *+ Cutscene* adds a template function, *Play as cutscene*
play-tests any function of the open script as a cut-scene on the open map, and the *Play
cutscene* action has a *Preview* button. A preview starts without the map's *Map enter*
triggers.

### Enemy spawners

| Function | Description |
|---|---|
| `disable_spawners(map)` | Switches off every enemy spawner of a map (`""` = the current one). The characters they spawned on the current map vanish; nothing spawns there any more. Saved. *(queued)* |
| `disable_spawners(map, zone)` | The same for one spawner zone, by its name. *(queued)* |
| `enable_spawners(map)` | Switches all of a map's spawners on again. *(queued)* |
| `enable_spawners(map, zone)` | Switches on a spawner zone that was switched off by name. *(queued)* |
| `defeat_enemies()` | Defeats every hostile character on the current map at once, like in combat: their loot drops and their *Defeated* triggers run (e.g. quest counters). *(queued)* |

### Equipment and carried items of characters

Every humanoid character has equipment (Clothing, Head, Body, Weapon, Left hand) and carried items.
What NPCs wear and hold is visible on layered models, counts in combat (an equipped weapon
replaces the definition's), and everything they wear and carry is dropped when they are defeated.
Changes are saved with the game.

| Function | Description |
|---|---|
| `equip(target, item)` | Puts an item into its slot (clothing, armor, weapon, left hand); what was worn there goes to the character's carried items. For `"player"` the item comes from the inventory (or is given). *(queued)* |
| `unequip(target, slot)` | Takes off what is worn in `"clothing"`, `"head"`, `"body"`, `"weapon"`, `"offhand"` or `"tool"` (into the carried items; the player: the inventory). *(queued)* |
| `give_item_to(target, item, count)` | Adds items to what a character carries (the player: the inventory). *(queued)* |
| `take_item_from(target, item, count)` | Takes items from what a character carries. *(queued)* |

```rust
fn guard_upgrade() {
    equip("guard", "axe");          // the spear goes to his carried items
    unequip("guard", "head");       // so does the helmet
    give_item_to("guard", "coin", 5);
}
```

### Interface

The player hides and shows the HUD (status frame, clock, quick bar and the buttons) with **H**.
Scripts can do the same, e.g. for a cut-scene. Dialogs, choices and windows the player opens
are still shown. Not saved: a loaded game always starts with the HUD visible.

| Function | Description |
|---|---|
| `hide_ui()` | Hides the HUD. *(queued)* |
| `show_ui()` | Shows it again. *(queued)* |
| `shake(seconds)` | Shakes the screen as hard as a hit on the player, fading out (earthquakes, explosions, a boss landing). *(queued)* |
| `shake(seconds, strength)` | The same with a strength: `1` = like a hit, `2` = twice as hard. *(queued)* |

### Saving

The game autosaves to the *Autosave* slot after every map change (once the new map's *Map
enter* triggers and cut-scenes are done), and a defeated player can continue from the newest
save. Scripts can add autosaves at important moments, e.g. after a boss fight or a quest step.

| Function | Description |
|---|---|
| `autosave()` | Saves the game to the *Autosave* slot as soon as the running event — its dialogs, waits or cut-scene — has finished, so a save never captures half an event. *(queued)* |

### Quests

| Function | Description |
|---|---|
| `start_quest(quest)` | Starts a quest. *(queued)* |
| `advance_quest(quest)` | Moves a quest to its next stage. *(queued)* |
| `set_quest_stage(quest, stage)` | Sets a quest's stage (0 = the first). *(queued)* |
| `complete_quest(quest)` | Completes a quest. *(queued)* |
| `fail_quest(quest)` | Fails a quest. *(queued)* |
| `quest_stage(quest)` | Current stage (`-1` = not started). |
| `quest_active(quest)` | Whether the quest is running. |
| `quest_done(quest)` | Whether the quest is completed. |

### Time of day

| Function | Description |
|---|---|
| `hour()` | Time of day in hours, `0` to `24` (`13.5` = half past one in the afternoon). |
| `is_night()` | Whether it is night (dark on outdoor maps). |
| `set_hour(hour)` | Sets the time of day, e.g. `set_hour(22)`. *(queued)* |

### Weather

Each map has its own weather (map settings: clear, rain, snow or fog, with a strength). Scripts
and the *Set weather* action change it; the change is kept in the save game, so the map keeps
that weather until it is set again. Weather names: `"none"`, `"rain"`, `"snow"`, `"fog"`, and
`"default"` = back to the map's own weather.

| Function | Description |
|---|---|
| `set_weather(kind)` | Weather of the current map at a medium strength (`0.6`). *(queued)* |
| `set_weather(kind, intensity)` | The same with a strength from `0` to `1`, e.g. `set_weather("rain", 0.8)`. *(queued)* |
| `set_map_weather(map, kind, intensity)` | Weather of another map, e.g. `set_map_weather("forest", "fog", 0.5)`. *(queued)* |
| `weather()` | The current map's weather: `"none"`, `"rain"`, `"snow"` or `"fog"`. |
| `weather_intensity()` | Its strength (`0` to `1`). |

```rust
fn storm_rolls_in() {
    if weather() != "rain" {
        say("Dark clouds gather over the hills.");
        set_weather("rain", 0.9);
    }
}

fn lift_the_curse() {
    set_weather("default");             // back to whatever the map has
    set_map_weather("forest", "none", 0);
}
```

### Sound

| Function | Description |
|---|---|
| `play_sound(sound)` | Plays a sound once. *(queued)* |
| `play_music(sound)` | Switches the looping music; `""` stops it. *(queued)* |

### Skills and magic

These only matter in projects with skills (and magic) enabled in *Database › Project*.

| Function | Description |
|---|---|
| `skill(skill)` | The player's level in a skill (`0` = not learned). |
| `skill_points()` | Unspent skill points. |
| `set_skill(skill, level)` | Sets a skill's level without spending points. *(queued)* |
| `give_skill_points(amount)` | Gives skill points (negative takes them away). *(queued)* |
| `knows_spell(spell)` | Whether the player has learned a spell. |
| `learn_spell(spell)` | Teaches a spell. *(queued)* |
| `mana()` | Current mana. |
| `add_mana(amount)` | Adds mana (negative drains it). *(queued)* |

### Experience and levels

These only matter in projects with *Experience and levels* enabled in *Database › Project*. Defeated
characters give the experience set in their definition (*Combat › Experience*), completed quests their
*Experience reward*; enough experience raises the level, which adds max HP, mana and skill points as set
in the project. *Defeat all enemies* / `defeat_enemies()` gives no experience.

| Function | Description |
|---|---|
| `xp()` | All the experience points the player has gained. |
| `level()` | The player's level (`1` = the first). |
| `give_xp(amount)` | Gives experience; enough of it raises the level, possibly several at once. Negative takes experience away, but the level never drops. *(queued)* |

```rust
fn trainer_talk() {
    if level() < 3 {
        say(SELF, "Come back when you have seen a few more fights.");
        return;
    }
    say(SELF, "You have grown strong. Take this lesson.");
    give_xp(50);
}
```

### Status effects

Timed effects on the player or a character. Weapons can give them on hit and hostile characters
on touch (set in the Database); these functions start and end them from scripts, like the
*Apply effect* / *Cure effect* actions. The player's effects are saved; a character's last until
the map is left. Applying an effect that is already running restarts it with the longer duration
and the stronger strength.

| Effect | `strength` means |
|---|---|
| `"poison"`, `"burn"` | HP lost per second (fractions add up; defense doesn't help) |
| `"regen"` | HP regained per second |
| `"slow"`, `"haste"` | percent slower / faster movement |
| `"strength"`, `"weakness"` | percent more / less damage dealt |
| `"shield"` | extra defense |
| `"stun"` | (unused) can't move or attack |

| Function | Description |
|---|---|
| `apply_effect(target, effect, strength, seconds)` | Starts an effect on `"player"`, `"self"` or a character's name. *(queued)* |
| `cure(target, effect)` | Ends one effect, e.g. `cure("player", "poison")`. *(queued)* |
| `cure(target)` | Ends all of the target's effects. *(queued)* |
| `has_effect(effect)` | Whether the player has the effect right now. |

```rust
fn swamp_gas() {
    if !has_effect("poison") {
        say("The air here burns in your throat.");
    }
    apply_effect("player", "poison", 1, 5);   // 1 HP per second for 5 seconds
}

fn holy_water() {
    cure("player");                              // every effect
    apply_effect("player", "regen", 0.5, 10);
}
```

### Spell effects: targeting, flying sprites, animations, damage

Use these to build spells (and traps, cut-scenes, …) in scripts.

| Function | Description |
|---|---|
| `aim(function)` | Starts magic targeting: the cursor turns into a crosshair, and the player's next click (on an enemy or any spot) calls `function(x, y)` with the target. X fires straight ahead (5 tiles), right-click or Esc cancels. *(queued)* |
| `fly(sheet, x, y, function)` | An animated sprite flies from the player to point `x, y`, turned in its direction, then calls `function(x, y)` (`""` = call nothing). *(queued)* |
| `fly(sheet, x, y, function, options)` | Like above with [options](#options-maps): `speed`, `frames` or `tag`, `fps`, `scale`, `light`, `color`. *(queued)* |
| `effect(sheet, x, y)` | Plays the frames of a sprite sheet once, centered on `x, y` (explosions, sparkles). *(queued)* |
| `effect(sheet, x, y, options)` | Like above with options: `frames` or `tag` (a named animation, e.g. `#{ tag: "explode" }`), `fps`, `scale`, `light`, `color`, `shake` (seconds the screen shakes as it starts). *(queued)* |
| `damage_area(x, y, radius, damage)` | Damages every character within `radius` tiles of `x, y`. Spell damage skills increase it. Never hurts the player. *(queued)* |
| `damage_around(damage, radius)` | The same around the player. *(queued)* |

**Spell costs**: when a spell's *Script* function calls `aim`, the spell's mana and cooldown are
spent only once the player has chosen a target; cancelling costs nothing. Without `aim`, they
are spent when the spell is cast.

A flying sprite doesn't collide with anything on its way: it acts only at its target.

### Light and healing

| Function | Description |
|---|---|
| `player_light(radius, seconds)` | A light around the player for a while. *(queued)* |
| `player_light(radius, seconds, options)` | With options `color` and `flicker`. *(queued)* |
| `light_at(x, y, radius, seconds)` | A light at a point for a while; it fades out during its last second. *(queued)* |
| `light_at(x, y, radius, seconds, options)` | With options `color` and `flicker`. *(queued)* |
| `heal_area(x, y, radius, amount)` | Heals the player and friendly (not hostile) characters within `radius` tiles of `x, y`. *(queued)* |

Lights only show where it is dark: on *Dark* maps and on *Outdoor* maps at night.

### Options maps

Some functions take an optional last argument, a Rhai object map `#{ key: value, … }`. Unknown
keys are ignored, missing keys use the default.

| Key | Used by | Meaning | Default |
|---|---|---|---|
| `speed` | `fly` | Tiles per second | `8` |
| `frames` | `fly`, `effect` | `[first, last]` frame of the sheet | all frames |
| `frame` | `fly`, `effect` | A single frame | — |
| `tag` | `fly`, `effect` | A named animation of the sheet (*Database › Sprite sheets › Animations*, e.g. the tags of an imported Aseprite sheet): its frames and speed replace `frames` / `fps` | — |
| `fps` | `fly`, `effect` | Frames per second (`fly` loops, `effect` plays once) | `12` |
| `scale` | `fly`, `effect` | Size factor (`2` = twice the frame size) | `1` |
| `light` | `fly`, `effect` | Radius of a glow in tiles (an effect's glow fades with it) | `0` (none) |
| `color` | `player_light`, `light_at`, `fly`, `effect` | `[red, green, blue]`, each `0`–`255` (for `fly` / `effect`: the glow's color) | cool white (player), warm white (`light_at`), fire orange (glows) |
| `flicker` | `player_light`, `light_at` | `true` = flickers like a flame | `false` |

```rust
fly("fireball", x, y, "fireball_hit", #{ speed: 9, fps: 14, light: 2.5 });
light_at(x, y, 3, 10, #{ color: [80, 255, 120], flicker: true });
```

### Constants

| Name | Description |
|---|---|
| `SELF` | Id (uid) of the entity that owns the calling trigger, `-1` if there is none. |

### Built into Rhai

Rhai's own functions work too, for example `print(…)` (writes to the game's console),
`to_string()`, `to_int()`, `to_float()`, `abs()`, `floor()`, `round()`, `len()` on strings and
arrays, `push` on arrays. See the [Rhai book](https://rhai.rs/book/).

---

## Recipes

### A character who remembers

```rust
fn mira_talk() {
    add_var("mira_talks", 1);
    let n = get_var("mira_talks");
    if n == 1 {
        say("Mira", "Hi! I'm Mira. " + player_name() + ", right?");
    } else if flag("shrine_lit") {
        say("Mira", "You lit the shrine? I want to be a {hero|heroine} like you!");
    } else {
        say("Mira", "Talked to me " + n + " times now. You must be bored!");
    }
}
```

### Counting defeated enemies for a quest

The slime character's **Defeat** trigger runs `slime_defeated`:

```rust
fn slime_defeated() {
    add_var("slimes_defeated", 1);
    if quest_active("pest_control") && quest_stage("pest_control") == 0 {
        add_var("pest_slimes", 1);
        if get_var("pest_slimes") >= 5 {
            set_quest_stage("pest_control", 1);   // "Go back to Tilda"
        }
    }
}
```

### A scripted skill

A skill with the **Script** effect and function `scavenging` (*Database › Skills*). The engine
calls it with the new level whenever the level changes, and once when a game starts or loads,
so the function can simply store the level:

```rust
fn scavenging(level) {
    set_var("scavenging", level);
}

// ... and use it elsewhere:
fn slime_defeated() {
    if get_var("scavenging") > 0 {
        give_item("coin", get_var("scavenging"));
    }
}
```

### A Fireball spell

A spell with the **Script** effect and function `cast_fireball` (*Database › Spells*), using the
sprite sheets `fireball` (4 frames, flying to the right) and `explosion` (8 frames):

```rust
fn cast_fireball() {
    aim("fireball_at");                // mana is spent once a target is chosen
}

fn fireball_at(x, y) {
    play_sound("fireball");
    fly("fireball", x, y, "fireball_hit", #{ speed: 9, fps: 14, light: 2.5 });
}

fn fireball_hit(x, y) {
    play_sound("explosion");
    effect("explosion", x, y, #{ fps: 14, scale: 2.4, light: 5, shake: 0.3 });
    damage_area(x, y, 2, 5);
    light_at(x, y, 2, 4, #{ color: [255, 120, 50], flicker: true });   // embers
}
```

### A healing spell with a visible effect

(`sparkle` stands for a sprite sheet of your own.)

```rust
fn cast_mending() {
    let x = player_x();
    let y = player_y();
    effect("sparkle", x, y - 0.7, #{ scale: 1.5, light: 3 });
    heal_area(x, y, 2, 4);             // the player and friendly characters nearby
}
```

### A trap that explodes when the player walks in

A zone with an **On Enter** trigger (*once*) running `trap`:

```rust
fn trap() {
    let x = player_x();
    let y = player_y();
    play_sound("explosion");
    effect("explosion", x, y, #{ scale: 2, light: 4 });
    heal(-3);                          // damage_area never hurts the player
}
```

### A short scripted scene

```rust
fn guard_steps_aside() {
    hide_ui();                      // a clean screen while it plays
    say(SELF, "Halt! ... Oh, it's you. Go ahead.");
    move_by("self", 1, 0);
    face("self", "left");
    then_call("gate_open");
}

fn gate_open() {
    set_state("gate", "open");
    play_sound("creak");
    show_ui();
}
```

### A cut-scene: arriving in town

The example project's intro (a *Map enter* trigger of the village, *once*, with the action *Play
cutscene* → `intro_cutscene`):

```rust
fn intro_cutscene() {
    place("player", 0, 30);         // at the west edge of the village
    face("player", "right");
    wait(0.8);
    walk_to("player", 5, 30);       // into town along the road
    wait(0.4);
    approach("guard", "player");    // the guard comes over
    say("Guard", "Halt, traveller! ... {player}? Thank the stars, you came!");
    say("Guard", "Please, talk to Elder Rowan at once - he is waiting by the well.");
}
```

### A cut-scene with light: the shrine awakens

The example's forest shrine (its *Interact* trigger runs `shrine_touch()`, which plays this the
first time): the clearing darkens so the lights show by day, the statues and the shrine light
up, two bursts of magic roll out, and the forest's spawners go quiet.

```rust
fn shrine_awakening() {
    face("player", "up");
    say("You lay your hand on the cold stone. It begins to hum under your fingers...");
    dim(0.8, 1.5);
    play_sound("chime");
    wait(1.6);
    let blue = #{ color: [120, 200, 255], flicker: true };
    light_at(47.5, 20.5, 1.5, 8, blue);         // a glow in the crystal
    wait(0.8);
    light_at(44.5, 20.2, 2.5, 7, blue);         // the statues' eyes
    light_at(50.5, 20.2, 2.5, 6.6, blue);
    wait(0.6);
    set_state("shrine", "glowing");
    effect("magic_burst", 47.5, 20.5, #{ scale: 1.5, fps: 14, light: 5, color: [140, 210, 255] });
    wait(0.5);
    effect("magic_burst", 47.5, 20.5, #{ scale: 3.5, fps: 9, light: 9, color: [170, 230, 255] });
    wait(1.0);
    say("Ancient runes flare up! A wave of light rolls out from the shrine.");
    disable_spawners("forest");
    set_flag("shrine_lit", true);
    dim(0, 2);
    wait(1.5);
}
```

### A fountain that heals once

```rust
fn fountain() {
    if flag("fountain_used") {
        say("The water is still.");
        return;
    }
    set_flag("fountain_used", true);
    light_at(player_x(), player_y() - 1, 3, 3, #{ color: [120, 200, 255] });
    heal(player_max_hp());
    say("You feel refreshed.");
}
```

---

## Limits, errors and debugging

- **Sandboxed**: scripts can't read or write files, open network connections or start programs.
  They only see the game through the functions above.
- **Operation limit**: a single call may run about two million operations. An endless loop is
  stopped with an error instead of freezing the game. Call depth is limited to 64 nested
  function calls.
- **Errors never crash the game**. A script error (a typo in a function name, a wrong argument
  type, calling a function that doesn't exist) is printed on the game's console as
  `[script] error: …` and the call is skipped; the game goes on.
- **Syntax errors** are reported by *Save & check* in the editor (and on the console when the
  game starts). A file with a syntax error is not loaded at all, so its functions are missing.
- **`print(…)`** writes `[script] …` to the console: start the game from a terminal to see it.
  A play-test started from the editor writes to the terminal the editor was started from.
- **Argument types matter**: a function expecting a whole number doesn't accept `2.5`, and a
  missing argument is an error. The parameter hint in the editor shows what a function expects.
- **Reserved words**: Rhai reserves some words for itself, among them `var`, `go`, `fn`, `let`,
  `const`, `this`, `in`, `match`, `static`. That's why variables are read with `get_var`, not `var`. Don't
  use reserved words as function or variable names.
- **Changes need a new game start**: the game loads scripts when it starts; restart the
  play-test after editing a script.

## Rhai in five minutes

```rust
// Comments start with // (or /* ... */).
fn greet(name) {                    // a function with a parameter
    let msg = "Hello, " + name;      // a local variable
    if name == "Mira" {
        msg += "!";
    } else if name.len() > 10 {
        msg += ", what a long name.";
    }
    msg                             // the last expression is the result
}

fn count_to_five() {
    let total = 0;
    for i in 1..=5 {                 // 1, 2, 3, 4, 5
        total += i;
    }
    while total > 10 { total -= 3; }
    total
}

fn lists_and_maps() {
    let items = ["coin", "potion"];  // an array
    items.push("apple");
    for it in items { print(it); }
    let opts = #{ speed: 9, light: 2.5 };   // an object map
    print(opts.speed);
    print(`Template strings: ${items.len()} items`);
}
```

- Numbers: `5` is an integer, `5.0` a decimal. `7 / 2` is `3`; `7.0 / 2.0` is `3.5`. Convert
  with `to_int()` / `to_float()`.
- Strings join with `+`, and numbers can be added to strings: `"Count: " + n`.
- Comparison `==`, `!=`, `<`, `>=`; logic `&&`, `||`, `!`.
- `return` leaves a function early; `return value;` returns a value.
- Functions can't see variables of other functions; pass values as parameters or store them in
  flags and variables.

The [Rhai book](https://rhai.rs/book/) explains the whole language.
