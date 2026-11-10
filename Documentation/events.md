# Events: triggers, conditions and actions

Everything that happens in a game outside scripts is a **trigger**: *on* an event, *if* its
conditions hold, run its actions. Triggers belong to an owner — a map, a zone, a placed
character / prop / item pickup, or a definition (a character's or prop's triggers apply to every
copy; an item's `on_use` triggers run when the player uses it). The editor edits them in the
inspector and the Database; agents send them as JSON to `set_triggers`, `add_trigger`,
`place_entity` / `update_entity` (`triggers`), `add_zone` (`triggers`) and `upsert_def`
(`triggers` / `on_use`). `events_reference` prints this list from the running editor.

A trigger:

```json
{"on": "Interact",
 "conditions": [{"cond": {"Quest": {"quest": "pest_control", "check": "NotStarted", "stage": 0}}, "negate": false}],
 "actions": [{"Dialog": {"speaker": "Elder", "text": "Slimes in the cellar again, {player}…"}},
             {"Choice": {"speaker": "Elder", "text": "Will you help?", "options": ["Yes", "Later"], "var": "help"}},
             {"If": {"conditions": [{"cond": {"Var": {"name": "help", "op": "Eq", "value": 0}}, "negate": false}],
                     "then": [{"Quest": {"quest": "pest_control", "op": "Start", "stage": 0}}], "otherwise": []}}],
 "once": false}
```

`once` runs the trigger a single time per save game. Texts may use `{player}` (the player's
name), `{sir|madam}` (by the player's sex) and `{var}` (a variable's value). Actions run in
order; `Dialog`, `Choice`, `Wait`, `WalkTo`, `Approach` and `Cutscene` pause until they finish.
Variables (`SetVar`, `Var`) are whole numbers, flags (`SetFlag`, `Flag`) booleans; `Choice` writes
the chosen option's index into its variable. `"target"` names `self` (the owner), `player` or a
placed entity's name.

## Events per owner

Owners accept only some events (`set_triggers` refuses the others):

| Owner | Events |
|---|---|
| map | `"MapEnter"`, `{"Custom":{"name":""}}`, `"Sleep"`, `{"Hour":{"hour":12.0}}` |
| zone | `"Enter"`, `"Interact"`, `"Leave"`, `{"Custom":{"name":""}}` |
| character (entity or def) | `"Interact"`, `{"UseItem":{"item":""}}`, `{"Custom":{"name":""}}`, `"Defeat"`, `{"HpBelow":{"percent":50}}` |
| prop (entity or def) | `"Interact"`, `{"UseItem":{"item":""}}`, `{"Custom":{"name":""}}`, `"Unlock"`, `"Lock"`, `"Sleep"` |
| item pickup (entity) | `"Interact"`, `{"UseItem":{"item":""}}`, `{"Custom":{"name":""}}` |
| item def (on use) | `{"UseItem":{"item":""}}` |

- `Interact`: the player presses E at the entity / inside the zone (zones also with E).
- `Enter` / `Leave`: the player walks into / out of a zone. `MapEnter`: the player arrives on the
  map (also after loading a save? — no: loading never fires it). `UseItem {item}`: the player
  uses that item on the owner (empty item = any). `Custom {name}`: an `Emit` action or the
  script's `emit(name)` with that name. `Defeat`: the character's hp reached 0. `HpBelow
  {percent}`: the character fell below that share of its hp. `Unlock` / `Lock`: the prop's lock
  changed. `Sleep`: the player woke up (in a bed, or the map's `Sleep` after sleeping anywhere on it).
  `Hour {hour}`: the clock reached that time of day (`12.0` = noon, `5.5` = 5:30) while the player
  is on the map, every day; also when time jumps forward past it (sleeping, `SetTime`), not when
  it is set back.

## Conditions

`{"cond": COND, "negate": false}`; every condition of a trigger must hold. Comparison `op`:
`Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`. Quest `check`: `NotStarted`, `Active`, `Completed`, `Failed`,
`AtStage`, `StageAtLeast` (with `stage`).

| Condition | JSON (example values) |
|---|---|
| Flag set | `{"Flag":{"name":""}}` |
| Variable | `{"Var":{"name":"","op":"Eq","value":0}}` |
| Has item | `{"HasItem":{"item":"","count":1}}` |
| Script | `{"Script":{"function":""}}` |
| Player HP | `{"PlayerHp":{"op":"Lt","value":3}}` |
| Prop state | `{"PropState":{"target":"self","state":""}}` |
| Quest | `{"Quest":{"quest":"","check":"Active","stage":0}}` |
| Player sex | `{"PlayerSex":{"sex":"Female"}}` |
| Time of day | `{"TimeBetween":{"from":20.0,"to":6.0}}` |
| Skill level | `{"Skill":{"skill":"","op":"Ge","level":1}}` |
| Knows spell | `{"KnowsSpell":{"spell":""}}` |
| Player level | `{"Level":{"op":"Ge","value":2}}` |
| Player has effect | `{"HasEffect":{"kind":"Poison"}}` |
| Defeated count | `{"Defeated":{"character":"","op":"Ge","count":10}}` |
| Pose (sitting, asleep) | `{"Pose":{"target":"self","pose":"lie"}}` |

## Actions

Quest `op`: `Start`, `Advance`, `SetStage`, `Complete`, `Fail`. Variable `op`: `Set`, `Add`.
Facing: `Down`, `Left`, `Right`, `Up`. Equipment slots: `Weapon`, `OffHand`, `Head`, `Body`,
`Hands`, `Feet`, `Ring`. Effects: `Poison`, `Regen`, `Haste`, `Slow`, `Strength`, `Weakness`
(see the scripting documentation).

| Action | JSON (example values) |
|---|---|
| Dialog | `{"Dialog":{"speaker":"","text":""}}` |
| Choice | `{"Choice":{"speaker":"","text":"","options":["Yes","No"],"var":"choice"}}` |
| Give item | `{"GiveItem":{"item":"","count":1}}` |
| Take item | `{"TakeItem":{"item":"","count":1}}` |
| Set flag | `{"SetFlag":{"name":"","value":true}}` |
| Set variable | `{"SetVar":{"name":"","op":"Set","value":0}}` |
| Teleport | `{"Teleport":{"map":"","spawn":""}}` |
| Set active | `{"SetActive":{"target":"self","active":false}}` |
| Set frame | `{"SetFrame":{"target":"self","frame":1}}` |
| Move | `{"Move":{"target":"self","dx":0,"dy":1}}` |
| Face | `{"Face":{"target":"self","facing":"Down"}}` |
| Wait | `{"Wait":{"seconds":0.5}}` |
| Emit event | `{"Emit":{"name":""}}` |
| Run script | `{"RunScript":{"function":""}}` |
| If | `{"If":{"conditions":[],"then":[],"otherwise":[]}}` |
| Play sound | `{"PlaySound":{"sound":""}}` |
| Play music | `{"PlayMusic":{"sound":""}}` |
| Heal player | `{"Heal":{"amount":1}}` |
| Set locked | `{"SetLocked":{"target":"self","locked":false}}` |
| Set prop state | `{"SetState":{"target":"self","state":""}}` |
| Open shop | `{"OpenShop":{"target":"self"}}` |
| Quest | `{"Quest":{"quest":"","op":"Start","stage":0}}` |
| Patrol | `{"Patrol":{"target":"self","active":true,"points":[]}}` |
| Set time of day | `{"SetTime":{"hour":8.0}}` |
| Give skill points | `{"GiveSkillPoints":{"amount":1}}` |
| Set skill level | `{"SetSkill":{"skill":"","level":1}}` |
| Learn spell | `{"LearnSpell":{"spell":""}}` |
| Add mana | `{"AddMana":{"amount":10}}` |
| Damage around player | `{"DamageAround":{"damage":2,"radius":2.0}}` |
| Light around player | `{"PlayerLight":{"radius":5.0,"seconds":30.0,"color":[190,210,255],"flicker":false}}` |
| Aim (then run script) | `{"Aim":{"function":""}}` |
| Fly sprite to point | `{"Fly":{"sprite":{"sheet":"","first":0,"last":0,"fps":12.0,"scale":1.0,"light":0.0,"glow":[255,160,70]},"x":0.0,"y":0.0,"speed":8.0,"function":""}}` |
| Play effect | `{"Effect":{"sprite":{"sheet":"","first":0,"last":0,"fps":12.0,"scale":1.0,"light":0.0,"glow":[255,160,70]},"x":0.0,"y":0.0}}` |
| Damage area | `{"DamageArea":{"x":0.0,"y":0.0,"radius":2.0,"damage":2}}` |
| Light at point | `{"LightAt":{"x":0.0,"y":0.0,"radius":3.0,"seconds":10.0,"color":[255,190,120],"flicker":true}}` |
| Heal area | `{"HealArea":{"x":0.0,"y":0.0,"radius":2.0,"amount":2}}` |
| Equip item | `{"Equip":{"target":"self","item":""}}` |
| Unequip slot | `{"Unequip":{"target":"self","slot":"Weapon"}}` |
| Give item to character | `{"GiveItemTo":{"target":"self","item":"","count":1}}` |
| Take item from character | `{"TakeItemFrom":{"target":"self","item":"","count":1}}` |
| Show / hide UI | `{"SetUiVisible":{"visible":false}}` |
| Play cutscene | `{"Cutscene":{"function":""}}` |
| Walk to tile | `{"WalkTo":{"target":"self","x":0,"y":0}}` |
| Walk up to character | `{"Approach":{"target":"self","to":"player"}}` |
| Place at tile | `{"Place":{"target":"player","x":0,"y":0}}` |
| Dim the scene | `{"Dim":{"amount":0.7,"seconds":1.0}}` |
| Spawners on / off | `{"SetSpawners":{"map":"","zone":"","active":false}}` |
| Defeat all enemies | `"DefeatEnemies"` |
| Autosave | `"Autosave"` |
| Give experience | `{"GiveXp":{"amount":10}}` |
| Apply effect | `{"ApplyEffect":{"target":"player","effect":{"kind":"Poison","strength":0.5,"seconds":6.0}}}` |
| Cure effect | `{"CureEffect":{"target":"player","kind":"Poison"}}` |
| Shake the screen | `{"Shake":{"seconds":0.5,"strength":1.5}}` |
| Set weather | `{"SetWeather":{"map":"","weather":{"kind":"Rain","intensity":0.6}}}` |
| Schedule | `{"Schedule":{"target":"self","active":false}}` |
| Companion | `{"Companion":{"target":"self","join":true}}` |
| Open crafting | `{"OpenCrafting":{"station":"","title":""}}` |
| Unlock achievement | `{"UnlockAchievement":{"id":""}}` |
| Lockpicking game | `{"Lockpick":{"difficulty":1,"var":"picked"}}` |
| Sleep | `{"Sleep":{"hours":8.0}}` |
| Heal character | `{"HealCharacter":{"target":"self","amount":2}}` |
| Companion order | `{"CompanionOrder":{"target":"self","order":"stay"}}` |
| Open companion window | `{"OpenCompanion":{"target":"self"}}` |
| Set mood | `{"Mood":{"target":"self","mood":"worried"}}` |
| Show image | `{"ShowImage":{"sheet":"tilda_full","wait":true,"scroll":true,"speed":40.0,"scale":1.0}}` |
| Hide image | `"HideImage"` |
| Shop stock | `{"ShopStock":{"target":"self","item":"trout","op":"Add","count":3,"price":0}}` (`op` `Add` / `Set`; `Set` with `count` 0 removes the line) |

## Patterns

- **A talking character**: `Interact` → `Dialog`s; gate different conversations with `Quest` /
  `Flag` conditions on several `Interact` triggers. Every trigger whose conditions hold runs (a
  definition's before the placed entity's own, each in list order), and all conditions are checked
  before any action runs, so give the triggers conditions that exclude each other.
- **A quest**: `upsert_def quests` (stages), the giver's `Interact` trigger starts it, zones /
  props / defeated enemies advance it (`Quest` with `Advance`), the giver's later trigger with
  `Quest {check: AtStage}` completes it and rewards (`GiveItem`, `GiveXp`).
- **A locked door**: a prop with `lock` ({key, on_unlock}) and `Unlock` trigger; the key an
  item somewhere (`place_entity` item) or bought (`shop`).
- **A picture on first meeting**: the character's `Interact` trigger with a `Flag` condition
  (negated), `SetFlag`, then `ShowImage` (`wait`: the event goes on once the player closes it;
  `scroll` for a tall picture) and `ShowImage` with `wait: false` for a portrait that stays
  behind the following `Dialog`s until the event ends.
- **A cut-scene**: `Cutscene {function}` runs a script function with `walk_to`, `say`, `place`
  while the HUD is hidden (see the scripting documentation).
- **Checks the engine can't express**: `Script {function}` conditions and `RunScript` actions
  call Rhai functions (`write_script`).
