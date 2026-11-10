# 6. Characters

Villagers, merchants, guards, enemies, animals and the player are all **characters**: definitions
in *Database › Characters*, placed on maps with the Entities tool.

## How characters look

Every character is a **body** dressed in layers:

- the **body**: people in underwear (male, female, goblin, female goblin), or a whole creature for
  monsters and animals (slimes, skeletons, bats, chickens);
- **skin colour**, **hair style**, **hair colour** and, for men, a **beard**;
- **clothing**: an outfit, with a fitted female cut on female bodies;
- **armor** over it: a helmet, body armor;
- what's **in their hands**: a weapon, a shield, a lantern or a tool, drawn per facing and walk
  frame; a melee swing shows the weapon sweeping.

Everything a character wears is real equipment: it counts in combat, and an enemy drops it when
defeated.

**Complete figures**: a body sheet marked *Complete figure* is a whole character with clothes, hair
and what it holds painted in. Nothing is drawn over it (no outfit, armor, hair, beard or held
items) and it isn't recoloured; its equipment still counts, it just isn't shown. The example's
people, animals and monsters are complete figures (one sheet per character); the layered bodies,
hair styles and outfits stay in the asset library for characters you assemble yourself.

**Animations**: besides walking, a character sheet can have rows for **idle** (breathing while it
stands), **attack** (wind-up, swing, follow-through, recover) and **hurt** (when hit); see [Art and
sound › Character sheets](10-art-and-sound.md#sizes-and-layouts). The game plays them by itself:
standing characters breathe, enemies' wind-ups and swings and the player's swing show the attack
frames, a hit shows the hurt frame. A sheet without these rows (or a layer that lacks them) shows
its standing frame instead.

## The character builder

![The character builder](../images/editor-builder.webp)

*Database › Characters › Look & equipment* assembles a character visually. On the left is a
**preview** that walks, breathes, attacks or flinches (*Walk*, *Idle*, *Attack*, *Hurt*; facing
buttons, *turn around*, and the four standing directions); on the right rows of clickable **cards**:

- **Body** (sprite sheets of kind *Character body*), **Skin** swatches (presets or any colour),
  **Hair** style cards, **Hair colour** swatches, **Beard** cards;
- **Clothing** (items of kind *Clothing*, shown worn), **Armor & hands** (*Head*, *Body armor*,
  *Weapon*, *Left hand*) and **Carried items**.

**Randomize** rolls hair, colours, beard and clothing for the body. **Start from…** copies the look
and equipment of any character in the project or the asset library (a library character's sheets
and items are copied into the project first).

**New character…** (in the Characters tab) opens the builder as a window with an id and a name;
*Create* adds the character, then set its behaviour and combat values in the form.

**Placed characters with their own look**: select one on the map; *Look & equipment* in the
inspector shows what it wears. **Customize this one…** opens the builder for that one character only
(*Apply*), and *Use the definition's* drops the changes. A whole village can share one *villager*
definition while every villager looks different.

## The character form

| Section | Settings |
|---|---|
| **Character** | Name; **Description** (shown in the bestiary once the player has defeated one); speed in pixels per second; hitbox |
| **Portrait** | A drawn portrait for dialogs and **Moods** ([below](#dialog-portraits-and-moods)) |
| **Look & equipment** | The builder |
| **Behavior** | *Static*, *Wander* or *Chase player* (below) |
| **Combat** | **Max HP** (0 = can't be defeated), **Damage**, **Defense**, **Hostile**, **Attack wind-up** (seconds before a swing or lunge; 0 = standard), an effect **On touch** (poison, slow…), an optional ranged attack for hostile characters |
| **Experience** | XP the player gets for defeating it |
| **Loot** | What it drops ([Objects and zones › Loot](04-objects-and-zones.md#loot)) |
| **Merchant** | Makes it a trader ([Merchants](#merchants)) |
| **As companion** | *Can't be given gear*, for animals and creatures |
| **Magic (companion)** | Mana and mana per second: a companion with mana learns and casts *Heal* and *Bolt* spells |
| **Light** | Carries a light (a lantern); placed characters can carry their own instead |
| **Default triggers** | Triggers every copy runs ([Events](07-events.md)) |

## Behaviour

| Behaviour | What the character does |
|---|---|
| **Static** | Stands where it was placed (unless it has a patrol route, a schedule or an activity). |
| **Wander** | Walks around its home spot. |
| **Chase player** | Hunts the player and companions (below). |

**Chase player** characters notice the player within their **Sight** (tiles) in a clear line; behind
walls and trees only within half of it. They keep hunting for a few seconds after losing sight,
walking to where they last saw the player, and slide around obstacles. Per character:

- **Leash**: beyond that many tiles from home they give up and walk back, harmless on the way.
- **Flee below**: below that share of their HP they run away and stop attacking (group fighters
  run to an ally instead and come back after a short rest).
- **Keep distance**: shooters back off when the player comes closer.
- **Group** (*Fights as a group*, on by default): at most two group fighters close in on the
  player or a companion at a time, the others circle around and wait for their turn; they spread
  out instead of stacking. Turn it off for a lone brute that always attacks.
- **Alert** (tiles, default 6): on first seeing the player it alerts others of the same
  definition this close; 0 alerts nobody.

An *HP below %* trigger lets a character react when it's hurt, e.g. a boss calling for help; scripts
read any character's HP with `hp_of(target)`. In the example, the goblin archers keep four tiles
away, flee once wounded and give up ten tiles from their camp, and the *Bandit Chief* calls two
outlaws at half his HP.

**Making way**: a friendly character standing in the player's way (a villager in a doorway, a guard
in a gate) steps aside when the player keeps walking into it for a moment, so no character can shut
the player in.

## Patrol routes

A patrolling character walks a route of waypoints.

1. Select a placed character and choose *Patrol route › Add a patrol route*.
2. Click tiles on the map to add waypoints. Drag a waypoint to move it, right-click removes it,
   Esc or *Done editing* finishes.
3. Choose *Loop* or *Back and forth*, the *Pause* at each point and an optional *Speed*.

The route is drawn with numbered points while the character is selected. In the game the character
finds its way around obstacles, waits briefly when talked to, and continues where it was after map
changes and loading. The **Patrol** action (start / stop, or a new route) and the scripts
`patrol(target)`, `patrol(target, [[x, y], …])` and `stop_patrol(target)` control it. A one-point
route means "walk there and stay".

## Daily schedules

![A villager selected, with his daily schedule in the inspector](../images/editor-schedule.webp)

A schedule makes a character live by the clock.

1. Select a placed character and choose *Daily schedule › Add a schedule*.
2. Click tiles on the map to add the places it goes to during the day. Drag a stop to move it,
   right-click removes it, Esc or *Done editing* finishes.
3. Give each stop a **time** (from then on the character goes there; type `7:30` or drag in quarter
   hours), optionally a **direction** to face, and:
   - **inside**: the character goes indoors here (e.g. at a door) and is out of sight until the next
     stop;
   - **map**: a stop on another map; the character walks to this map's door leading there, leaves,
     and turns up on that map as a visitor;
   - **on**: a piece of furniture by name; the character sits on the chair or bench, or lies in the
     bed;
   - **doing**: an activity at the stop (below).

The stops are drawn as a dashed loop with their times while the character is selected. In the game
the character walks to the stop of the hour and stays there (wandering around it if its behaviour is
*Wander*). Entering a map puts scheduled characters where the hour wants them; maps the player
isn't on aren't simulated.

The **Schedule** action and the scripts `stop_schedule(target)` / `follow_schedule(target)` pause and
resume a schedule. A character that is **hidden** (placed with *Active* off, or switched off with
*Set active*) stays hidden whatever its schedule says and doesn't visit other maps; once *Set
active* shows it, it goes straight to the stop of the hour. That's how a character can join the
village's life later, like Pip after he is cured. The **Time of day** condition gives night-time lines, and the **Pose** condition
(sitting, lying, at furniture, on its feet) lets a character say different things in bed. A sleeper
without a matching line just shows "… is asleep."

In the example, about twenty villagers of Emberbrook follow schedules: Bram the baker kneads at
dawn, sells at his stall, has an ale in the tavern in the evening and goes to bed; Tomas rests on a
bench in the afternoon and sleeps in the elder's guest bed at night.

## Activities

Characters can be seen **doing** something: *fishing, hoeing, hammering, chopping wood, mining,
sweeping* or *reading*, at a schedule stop or all day where they're placed (inspector › *Activity*),
with an optional tool in the hand. They stay at their spot, work in place with small effects (the
fishing line and bobber, soil specks, sparks on the anvil, wood chips, an open book whose pages
turn), and turn to the player for a few seconds when talked to. Activities are only for the look:
nothing is produced.

## Merchants

Tick **Merchant** in the character form to make every copy a trader:

- an optional **shop title**;
- the **currency** item (default `coin`);
- the **sell rate**: what the merchant pays for the player's items, in % of the price (0 = buys
  nothing);
- whether **talking opens the shop** (otherwise use the *Open shop* action, e.g. after a dialog
  choice, or `open_shop(target)`);
- the **stock**: items with a quantity (or *unlimited*) and a price (0 = the item's *Value*).

A placed character can *Customize stock & prices* or become a merchant on its own (*Make this a
merchant*). In the game, talking to a merchant runs its *Interact* triggers (a greeting) and then
opens the trade window. Items the player sells join the stock, and every merchant's stock is kept
in save games. The *Haggle* skill effect improves prices.

![Tilda's shop](../images/game-shop.webp)

## Companions

The **Companion** action (or `follow(target)` in a script) makes a placed, friendly character follow
the player:

- Companions walk behind the player, find their way around obstacles, catch up after teleports and
  come along to every map.
- They fight hostile characters near the player with their weapon (melee or ranged) and are hunted
  and hurt like the player; they regain HP slowly when nothing hunts them.
- Talking to a companion runs its own triggers, e.g. a dialog with orders.
- **Orders** (G, gamepad LB, the *Companion order* action, `order(name, …)`): *Attack* (the default),
  *Defend* (only what hunts you or the companion), *Passive* (no fighting), *Stay here* (wait on the
  spot, back at the post after a fight) and *Follow me*.
- **Gear**: clicking a companion's row in the HUD, *Gear…* in the orders menu, or `open_companion`
  opens the companion window: its equipment, its bag and your inventory. Hand items over, equip
  them, take them back. Unless the definition says *Can't be given gear*.
- **Characters use what they carry**: companions, other friendly characters and enemies drink a
  healing potion when their health is critical, a cure when poisoned or burning, a buff in a fight.
  A bow you gave a companion shoots arrows from its bag.
- **Healing**: items and *Heal* spells set to *Use on: Player or companion* ask whom to heal; the
  *Heal character* action and `heal_npc(name, amount)` heal any character.
- **Magic**: a companion whose definition has mana (*Magic (companion)* in the character form)
  learns the spell of a scroll or spell book you hand it in the companion window (a scroll that
  is used up when read is used up; a book stays in its bag), or from the *Teach spell* action /
  `teach_spell`. It casts *Heal* on the most hurt of you, the companions near it and itself when
  someone is below half their HP (a knocked-down companion first: the heal gets it up), and *Bolt*
  spells at the enemy it fights. Mana regenerates; its HUD row shows a blue mana bar, and its window
  lists its mana and spells. Spells stay learned when it is dismissed. In the example the town
  guard has mana: give him Tilda's *Scroll of Healing* or the elder's spell book.
- **At 0 HP** a companion is **knocked down**: it lies where it fell, enemies leave it alone, and
  any heal gets it back up. Leaving the map with a downed companion asks whether to leave it behind
  (it then limps home). *Database › Project › Companions at 0 HP* can instead let it get up by
  itself after some seconds, or send it home at once.
- **Dismissed** (`stop_following`, or the action with *leaves*), a companion stays where it is on its
  own map or goes home from elsewhere.

Companions, their HP, items and orders are kept in save games. The player's attacks never hit
companions or other friendly characters. In the example, the town guard offers to come along once
you took on the elder's quest.

## Dialog portraits and moods

Dialog and choice boxes show the speaker's head and shoulders, drawn from their model as they're
dressed right now; nothing to set up. The speaker is found from the dialog's speaker name: `{player}`
is the hero, `{self}` the trigger's owner, otherwise a character on the map or any definition whose
name fits (letter case is ignored, and "Elder" fits "Elder Rowan"). Narration, signs and letters
have no portrait.

**Drawn portraits** replace the automatic one: *Portrait* in the character form (any frame size, e.g.
64 × 64; *New blank portrait* adds a frame on a `portraits` sheet and opens the pixel editor), a
placed character's own portrait in the inspector, and the player's (*Project › Player portrait*).

**Moods** are more portraits by name ("worried", "angry") in the character's *Moods* list. A single
line can use one (`say("Elder Rowan", "...", #{ mood: "worried" })`), or the **Set mood** action /
`mood(name, mood)` changes it until the event ends. *Database › Project › Dialog portraits* turns
portraits off.

![Elder Rowan's drawn portrait in a dialog](../images/game-dialog.webp)

## The player character and the character creator

The player is a character too: *Database › Project › Player character* names the definition, and
its look and equipment are what a new game starts with.

The **character creator** (*Database › Project › Character creator*, *Show the character creator…*)
lets players create their hero after *New game*: a **name** (with a default), **Male / Female** (or
fixed, if the project says *Always male* / *Always female*), a **body** among the bodies of that sex
marked *Playable* (e.g. human or goblin), and **skin colour, hair style, hair colour** and, for men, a
**beard**, from the lists you set (a list left empty has no row). A playable **complete figure** is a
pre-made hero: the creator names it after the character that wears it and hides the skin, hair
and beard rows for it. The example offers the hero, the heroine, a goblin and a goblin scout. An animated preview walks through
all four directions. Name, sex and look are kept in save games. Play-tests skip the creator and use
the default name.

**Addressing the player**: in dialog, choice and quest texts, `{player}` becomes the player's name
and `{sir|madam}` the first word for a male player and the second for a female one (any pair works:
`{brother|sister}`). The **Player sex** condition and the scripts `player_name()`, `player_sex()`,
`is_female()`, `is_male()` and `text("…")` do the same in logic.

## What characters carry

Characters have equipment and carried items too, set in the builder for the definition or for one
placed character. They use their equipped weapon and their armor's defense in combat, use the items
they carry, and drop everything when defeated. The *Equip item*, *Unequip slot*, *Give item to
character* and *Take item from character* actions and the scripts `equip`, `unequip`,
`give_item_to` and `take_item_from` change what characters wear and carry during the game; the
changes are saved.
