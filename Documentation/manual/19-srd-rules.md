# 19. The SRD 5.1 rules

R(ust)PG2D plays by the **System Reference Document 5.1** (SRD 5.1) by Wizards of the Coast LLC,
released under the Creative Commons Attribution 4.0 licence. This chapter sums up how the engine
uses it, what is simplified while combat is still real-time, and what the licence asks of your
game. The details are in the chapters the sections link to.

## How the rules work in the engine

| Rule | In the engine | More |
|---|---|---|
| **Ability scores** | Six scores (1-30) with modifiers; the character creator sets them by standard array, point buy or 4d6 | [Gameplay › Rules](09-gameplay.md#rules-srd-51) |
| **Proficiency** | The bonus by level (+2 to +6) on skills, tools and saving throws the hero is proficient in | [Gameplay › Rules](09-gameplay.md#rules-srd-51) |
| **Checks and saving throws** | A d20 + modifier (+ proficiency) against a DC, with advantage and disadvantage; events, scripts, locks and crafting roll them; every roll goes into the journal's *Rolls* page | [Events](07-events.md#actions), [Scripting](08-scripting.md#rules-srd-51-dice-abilities-and-checks) |
| **Races and backgrounds** | The SRD's nine races with their subraces, traits and choices, and the Acolyte | [Characters › Races](06-characters.md#races) |
| **Levels and hit points** | The SRD's XP table up to level 20; HP from a d10 hit die (average or rolled) + CON | [Gameplay › Experience and levels](09-gameplay.md#experience-and-levels) |
| **Armor class** | 10 + DEX + the armor's defense | [Gameplay › Rules](09-gameplay.md#rules-srd-51) |
| **Conditions** | The 15 conditions with exhaustion's six levels; saving throws keep them off; damage or healing per round of 6 seconds | [Gameplay › Conditions and damage](09-gameplay.md#conditions-and-damage) |
| **Damage types** | 13 types; resistance halves, immunity stops, vulnerability doubles | [Gameplay › Conditions and damage](09-gameplay.md#conditions-and-damage) |
| **Rests** | Short rests spend hit dice; long rests (a night's sleep) restore HP, hit dice and lower exhaustion | [Gameplay › Rests](09-gameplay.md#rests) |
| **Stat blocks** | Any character can have one, from the SRD's 35 NPC templates (commoner, guard, bandit, veteran…) or by hand | [Characters › Stat blocks](06-characters.md#stat-blocks) |

The SRD's own words (conditions, damage types, languages, alignments, abilities and skills) are
built into the engine; the editor shows them as hover texts.

## What is simplified (until 0.29)

Combat is real-time until the turn-based combat of 0.29:

- **Attacks** don't roll against armor class yet. Weapons deal their *Damage* minus the target's
  *Defense*, after its resistances.
- **Characters** fight with *Max HP*, *Damage* and *Defense*, not their stat block's hit points.
  Their stat block's resistances, condition immunities and saving throws already count.
- **Disadvantage on attack rolls** (poisoned, prone, restrained…) waits for attack rolls.
- **The hit die** is a d10 for every hero until classes arrive (0.30). There are no spell slots yet,
  and no breath weapons or other class and race features beyond what the traits describe.

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
