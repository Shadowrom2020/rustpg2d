# 19. The SRD 5.1 rules

R(ust)PG2D plays by the **System Reference Document 5.1** (SRD 5.1) by Wizards of the Coast LLC,
released under the Creative Commons Attribution 4.0 licence. This chapter sums up how the engine
uses it, what is still simplified, and what the licence asks of your game. The details are in the chapters the sections link to.

## How the rules work in the engine

| Rule | In the engine | More |
|---|---|---|
| **Ability scores** | Six scores (1-30) with modifiers; the character creator sets them by standard array, point buy or 4d6 | [Gameplay › Rules](09-gameplay.md#rules-srd-51) |
| **Proficiency** | The bonus by level (+2 to +6) on skills, tools and saving throws the hero is proficient in | [Gameplay › Rules](09-gameplay.md#rules-srd-51) |
| **Checks and saving throws** | A d20 + modifier (+ proficiency) against a DC, with advantage and disadvantage; events, scripts, locks and crafting roll them; every roll goes into the journal's *Rolls* page | [Events](07-events.md#actions), [Scripting](08-scripting.md#rules-srd-51-dice-abilities-and-checks) |
| **Races and backgrounds** | The SRD's nine races with their subraces, traits and choices, and the Acolyte | [Characters › Races](06-characters.md#races) |
| **Levels and hit points** | The SRD's XP table up to level 20, split among the classed party; HP from the class's hit die (average or rolled) + CON | [Gameplay › Experience and levels](09-gameplay.md#experience-and-levels) |
| **Classes** | The twelve classes with their subclass, proficiencies, starting equipment and features to level 20; the level-up window's choices (subclass, ability score improvement or the Grappler feat, expertise, fighting styles); the features that work by themselves act, class actions (Rage, Second Wind, Action Surge, Flurry of Blows, Lay on Hands, Turn Undead…) in the fight's action bar with uses back after rests, riders and reactions (Sneak Attack, Stunning Strike, Uncanny Dodge…); party members have classes and level too and use their class actions | [Gameplay › Classes and levels](09-gameplay.md#classes-and-levels) |
| **Armor class** | 10 + DEX + the armor's defense | [Gameplay › Rules](09-gameplay.md#rules-srd-51) |
| **Conditions** | The 15 conditions with exhaustion's six levels; saving throws keep them off; damage or healing per round of 6 seconds | [Gameplay › Conditions and damage](09-gameplay.md#conditions-and-damage) |
| **Damage types** | 13 types; resistance halves, immunity stops, vulnerability doubles | [Gameplay › Conditions and damage](09-gameplay.md#conditions-and-damage) |
| **Rests** | Short rests spend hit dice; long rests (a night's sleep) restore HP, hit dice and lower exhaustion | [Gameplay › Rests](09-gameplay.md#rests) |
| **Monsters** | The 334 SRD monsters as library characters with their stat blocks; encounter zones size groups for the party | [Characters › Monsters](06-characters.md#monsters), [Encounters](04-objects-and-zones.md#encounters) |
| **Stat blocks** | Any character can have one, from the SRD's 35 NPC templates (commoner, guard, bandit, veteran…) or by hand, with its attacks and Multiattack | [Characters › Stat blocks](06-characters.md#stat-blocks) |
| **Combat** | Turn-based on the map's tiles (5 ft): initiative, surprise, movement with difficult terrain, the action economy and the SRD's actions, attack rolls against AC with advantage and disadvantage, critical hits, cover, opportunity attacks, two-weapon fighting, grapple and shove, death saving throws | [Gameplay › Health and combat](09-gameplay.md#health-and-combat) |

The SRD's own words (conditions, damage types, languages, alignments, abilities and skills) are
built into the engine; the editor shows them as hover texts.

## What is simplified

- **Classes:** the domain and oath spells (always prepared) and Wild Shape (shapes and summons,
  0.32) are text. Reactions (Uncanny Dodge, Deflect Missiles, Retaliation, Indomitable) are taken
  as soon as they fit, without asking (a reaction prompt is 0.32). *Stunning Strike* spends its ki
  on the first melee hit of a turn, *Open Hand Technique* always knocks prone, a turned undead is
  frightened (and so runs), *Lay on Hands* heals all a creature lacks (as far as the pool goes).
  Party members' AI doesn't use *Frenzy* (its exhaustion). Features that need
  other systems (Divine Smite, Favored Enemy's tracking, Natural Explorer, Mindless Rage, Persistent
  Rage, Stroke of Luck, Cutting Words, Quivering Palm…) are text. Multiclassing is 0.32 (the classes keep
  its prerequisites). The only feat is the SRD's Grappler. Heroes without a class (projects without
  classes) have a d10 hit die. In a
  fight spells are cast from the action bar ([Spellcasting](09-gameplay.md#spellcasting)).
- **Spells:** 113 of the 319 act by their parts; the other 206 are story only (their text, cast
  by events and scripts). Material components need a component pouch or any focus (the SRD ties each
  focus to its classes); costly materials aren't used up. Characters other than party members don't cast (monsters' spellcasting and the
  AI's casting are 0.32). Reaction spells (Shield) are cast on the caster's own turn. Areas hit
  everyone on their tiles (cover on DEX saves is 0.32); Magic Missile's darts strike one target;
  Sleep, Color Spray and the other spells of hit point pools are story only; Power Word Stun ignores
  the target's hit points; the rituals of the SRD are all story spells. A cantrip on a scroll is cast,
  not learned.
- **Equipment:** a thrown weapon stays in the hand (the throw is an attack, nothing is picked up
  again); a net and a lance are plain weapons (their special rules are text); helmets give no armor
  class; mounts and vehicles are items (riding is later), and so are trade goods; weight is recorded
  but there is no carrying limit or encumbrance yet (0.32); paying takes the smallest coins first,
  and change comes in gold, silver and copper. Characters without a stat block add their armor's
  armor class above 10 (a shield its bonus) to theirs.
- **Monsters:** their attacks act; special actions with recharge, legendary actions and traits such
  as Pack Tactics or Regeneration are texts until 0.32. Large and bigger creatures take one tile.
  Characters without a stat block fight with one made from their old values (AC 10 + *Defense*,
  their *Max HP*, +2 to hit with their weapon or *Damage*).
- **Encounter budgets** are the engine's own formula: the SRD 5.1 has no encounter tables.
- **Difficult terrain** comes from materials only (not yet from decorations); weapons with the
  *Reach* property reach 10 ft.
- **No breath weapons** or other class and race features beyond what the traits describe.

## The licence: what your game needs

The SRD 5.1 is free to use under **CC BY 4.0**: a game that uses its material must credit it.

- **Exports** always contain `SRD-5.1-ATTRIBUTION.txt` beside the engine's licence, because the
  game runner's rules include SRD text.
- **The credits:** *Database › Project › Uses SRD 5.1 content* (on in the example, in the starter
  library and in new projects that start from it) ends the *Credits* page with the attribution.
  `srd_attribution()` returns it for credits a script rolls; the example's ending credits show it.
- **Edit › Problems** warns when a project has SRD races, backgrounds or stat blocks but the
  setting is off.
- **Trademarks:** the SRD's licence doesn't cover Wizards of the Coast's trademarks. Don't call
  your game a "Dungeons & Dragons" or "D&D" game; "uses the SRD 5.1" is fine.

The attribution, as the licence asks for it:

> This work includes material taken from the System Reference Document 5.1 ("SRD 5.1") by Wizards
> of the Coast LLC and available at https://dnd.wizards.com/resources/systems-reference-document.
> The SRD 5.1 is licensed under the Creative Commons Attribution 4.0 International License
> available at https://creativecommons.org/licenses/by/4.0/legalcode.

The SRD's data was taken from [5e-bits/5e-database](https://github.com/5e-bits/5e-database) (MIT
License) by the repository's import tool ([The asset library › The SRD 5.1
data](11-asset-library.md#the-srd-51-data)).
