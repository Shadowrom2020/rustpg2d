//! Classes (`defs/classes.ron`, SRD 5.1): what the hero and party members are trained as. A class
//! gives the hit die, saving throws, armor / weapon / tool proficiencies, skill choices, starting
//! equipment and features by level (20 levels), and has subclasses with features of their own.
//! A feature is the SRD's text plus, where it acts, effects from [`FeatureEffect`] (the passive
//! ones change the numbers; `runtime/` reads them through [`ClassTraits`]). The hero's class is
//! `WorldState::class` ([`HeroClass`]); a party member's is `CharacterDef::class`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;
use crate::srd::conditions::Condition;
use crate::srd::Ability;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassDef {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The hit die's sides (6, 8, 10, 12).
    pub hit_die: u32,
    /// Saving throws it is proficient in (`str`, `con`).
    pub saves: Vec<String>,
    /// Armor, weapon and tool proficiencies (`light-armor`, `simple-weapons`, `thieves-tools`).
    pub proficiencies: Vec<String>,
    /// How many skills of `skill_options` the character picks.
    pub skill_choices: u32,
    pub skill_options: Vec<String>,
    /// Starting equipment (the library's items for the SRD's).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub equipment: Vec<ItemStack>,
    /// Multiclassing's prerequisites (0.32): an ability and the score it needs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub multiclass: Vec<(String, i32)>,
    /// The level the subclass is chosen at, and what the class calls it ("Primal Path").
    pub subclass_level: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subclass_title: String,
    /// Levels 1 to 20 (index 0 = level 1).
    pub levels: Vec<ClassLevel>,
    pub subclasses: Vec<SubclassDef>,
    /// Every feature of the class and its subclasses (and their options, such as the fighting
    /// styles), by id.
    pub features: Vec<ClassFeature>,
    /// How it casts spells (`None` = it doesn't); its slots and the cantrips and spells it knows
    /// are the level's numbers (`spellcasting.spell_slots_level_1`, `cantrips_known`, `spells_known`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spellcasting: Option<Spellcasting>,
}

/// A class's spellcasting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spellcasting {
    /// The spellcasting ability (`int` for a wizard, `wis` for a cleric, `cha` for a bard).
    pub ability: Ability,
    pub kind: CasterKind,
    /// Prepares half its level's worth (paladin; the ranger knows its spells).
    #[serde(default)]
    pub half: bool,
    /// Casts its ritual spells as rituals (without a slot, 10 minutes longer).
    #[serde(default)]
    pub ritual: bool,
}

/// How a class has its spells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CasterKind {
    /// Learns `spells_known` spells at its level-ups (bard, ranger, sorcerer).
    Known,
    /// Prepares from its whole list after a rest: the modifier + its level (cleric, druid, paladin).
    Prepared,
    /// Learns spells into a spellbook (6 at level 1, 2 a level, scrolls copied) and prepares from it (wizard).
    Spellbook,
    /// Knows its spells; its few slots, all of the highest level it has, come back after a short rest (warlock).
    Pact,
}

/// What a level of a class brings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassLevel {
    /// Feature ids gained at this level.
    pub features: Vec<String>,
    /// The class's numbers at this level (`rage_count`, `sneak_attack_dice`, `ki_points`,
    /// `martial_arts_die`…).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub numbers: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubclassDef {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// (level, feature ids) of the subclass.
    pub levels: Vec<(u32, Vec<String>)>,
}

/// A class feature: the SRD's words, and what of it acts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassFeature {
    pub id: String,
    pub name: String,
    pub level: u32,
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<FeatureEffect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<FeatureChoice>,
}

/// A choice a feature asks for when it is gained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FeatureChoice {
    /// Double the proficiency bonus of `count` skills (or thieves' tools) the character is
    /// proficient in.
    Expertise { count: u32 },
    /// `choose` of the listed features (the fighting styles).
    Options { choose: u32, options: Vec<String> },
    /// +2 to one ability or +1 to two (at most 20), or the feat.
    AbilityScores,
    /// `count` skills of the character's choice (the College of Lore's Bonus Proficiencies).
    Skills { count: u32 },
}

/// What a feature does. New variants are appended (the files store their names).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FeatureEffect {
    /// Armor class without armor: `base` + the modifiers of `abilities` (+ a shield when
    /// `shield`). The best formula counts.
    ArmorClass { base: i32, abilities: Vec<Ability>, shield: bool },
    /// More speed (feet), with `unarmored` only without armor or shield.
    Speed { feet: u32, unarmored: bool },
    /// A proficiency (skill, save `save:dex`, tool, armor, weapon).
    Proficient { what: String },
    /// Half the proficiency bonus on checks without proficiency (Jack of All Trades), only with
    /// `abilities` when listed (Remarkable Athlete: STR, DEX, CON).
    HalfProficiency { abilities: Vec<Ability> },
    /// A d20 below 10 counts as 10 on proficient checks.
    ReliableTalent,
    /// Critical hits from this natural roll (19, 18).
    CritRange { from: u32 },
    /// The Attack action makes this many attacks.
    ExtraAttacks { attacks: u32 },
    /// More damage dice on a melee critical hit.
    BrutalCritical { dice: u32 },
    /// A fighting style.
    FightingStyle { style: FightingStyle },
    /// A DEX save for half takes none on a success, half on a failure.
    Evasion,
    /// Advantage on saving throws of an ability (Danger Sense: DEX).
    SaveAdvantage { ability: Ability },
    /// Advantage on initiative.
    InitiativeAdvantage,
    Resistance { types: Vec<String> },
    ConditionImmunity { conditions: Vec<String> },
    /// Allies (and the character) within `feet` add the CHA modifier to saves; `immunity`: a
    /// condition they can't get (frightened).
    Aura { feet: u32, save_bonus: bool, immunity: String },
    /// An ability score up by `amount`, to at most `max`.
    AbilityBonus { ability: Ability, amount: i32, max: i32 },
    /// More HP per level.
    HpPerLevel { hp: i32 },
    /// Unarmed strikes and monk weapons use DEX if better and the martial arts die
    /// (`martial_arts.dice_value` of the level).
    MartialArts,
    /// Damage types that don't hurt it.
    Immunity { types: Vec<String> },
    /// A pool of uses (rages, ki, Lay on Hands' hit points): `count` + the class number `number`
    /// at the level + `ability`'s modifier (at least 1) + `per_level` × the level; back after a
    /// short or long rest. A later feature with the same id replaces it (Font of Inspiration).
    Resource {
        id: String,
        #[serde(default)]
        count: i32,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        number: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ability: Option<Ability>,
        #[serde(default)]
        per_level: i32,
        #[serde(default)]
        recharge: Recharge,
    },
    /// Something the character does in a fight (the action bar's *Class* entry): what it costs of
    /// the turn, the resource it spends one use of (empty = free to use), what it does.
    Act {
        id: String,
        name: String,
        #[serde(default)]
        cost: ActCost,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        resource: String,
        effect: ActEffect,
    },
    /// Sneak Attack: once per turn the class's `sneak_attack.dice_count` d6 more on a hit with a
    /// finesse or ranged weapon, with advantage or an ally next to the target.
    SneakAttack,
    /// Once per turn more damage on a weapon hit: the dice of the highest level reached (Divine
    /// Strike's 1d8, 2d8 at 14); `hurt_target`: only against a creature below its hit point maximum
    /// (Colossus Slayer).
    ExtraDamage {
        dice: Vec<(u32, String)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
        #[serde(default)]
        hurt_target: bool,
    },
    /// A hit makes the target save (`save` against 8 + proficiency + `ability`'s modifier) or get
    /// `condition` for `rounds`: with `resource` (one use, once per turn: Stunning Strike), with a
    /// melee attack only (`melee`), or only with Flurry of Blows' strikes (`flurry`: Open Hand Technique).
    OnHitSave {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        resource: String,
        save: Ability,
        ability: Ability,
        condition: Condition,
        rounds: u32,
        #[serde(default)]
        melee: bool,
        #[serde(default)]
        flurry: bool,
    },
    /// Bringing a foe to 0 HP gives temporary hit points: `ability`'s modifier + `per_level` × the
    /// level (Dark One's Blessing).
    OnKillTempHp { ability: Ability, per_level: i32 },
    /// While raging, dropping to 0 HP: a CON save against `dc` (+5 for each time since the last
    /// rest) to stay at 1 HP (Relentless Rage).
    DropTo1 { dc: i32 },
    /// At the start of its turn, at half its hit points or fewer (but above 0): `base` + `ability`'s
    /// modifier back (Survivor).
    StartOfTurnHeal { base: i32, ability: Ability },
    /// Its reaction halves an attack's damage (Uncanny Dodge).
    ReactionHalveDamage,
    /// Its reaction lowers a ranged weapon attack's damage by `dice` + `ability`'s modifier +
    /// `per_level` × the level (Deflect Missiles).
    ReactionReduceRanged { dice: String, ability: Ability, per_level: i32 },
    /// A failed saving throw is rolled again, spending a use of `resource` (Indomitable).
    RerollSave { resource: String },
    /// Its reaction: a melee attack on whoever next to it damaged it (Retaliation).
    ReactionAttack,
    /// Turn Undead destroys undead of this challenge rating or lower (Destroy Undead).
    DestroyUndead { cr: f32 },
    /// A flat bonus (magic items: a Ring of Protection's +1 to AC and saving throws).
    Bonus { to: BonusTo, amount: i32 },
    /// An ability score becomes at least `score` (a Belt of Giant Strength's 21).
    AbilityScore { ability: Ability, score: i32 },
    /// Advantage on checks of a skill (`stealth`; Boots of Elvenkind).
    CheckAdvantage { skill: String },
    /// Darkvision this far (feet; Goggles of Night 60).
    Darkvision { feet: u32 },
    /// A special sense this far (feet; 0 = as far as it sees): a Gem of Seeing's truesight.
    Sense { sense: crate::srd::conditions::Sense, feet: u32 },
    /// Lucky: a natural 1 on an attack roll, an ability check or a saving throw is rolled again.
    Lucky,
    /// Advantage on saving throws against these conditions (Brave: frightened; Fey Ancestry:
    /// charmed).
    SaveAdvantageAgainst { conditions: Vec<crate::srd::conditions::Condition> },
    /// Advantage on saving throws of these abilities against magic (Gnome Cunning: INT, WIS,
    /// CHA).
    MagicSaves { abilities: Vec<Ability> },
    /// Magic can't put it to sleep (Fey Ancestry).
    NoMagicSleep,
    /// It moves through the space of a creature larger than it (Halfling Nimbleness).
    Nimble,
    /// Dropped to 0 hit points but not killed outright: 1 hit point instead, once per its
    /// resource's rest (Relentless Endurance).
    Endurance { resource: String },
    /// Divine Smite: after a melee weapon hit, a spell slot for 2d8 more (1d8 a slot level above
    /// 1st, at most 5d8; 1d8 more against the creature types `extra_against`): the hero is asked,
    /// party members smite a critical hit.
    Smite {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        extra_against: Vec<String>,
    },
    /// `dice` more on every melee weapon hit (Improved Divine Smite's 1d8 radiant).
    MeleeDamage {
        dice: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
    /// Song of Rest: a short rest's hit dice heal 1d`die` more (the bard's or a bard in the party).
    SongOfRest { die: u32 },
    /// Escape the Horde: opportunity attacks against it have disadvantage.
    EscapeTheHorde,
    /// These spells are always prepared and don't count against the prepared ones (Domain and
    /// Oath Spells).
    AlwaysPrepared { spells: Vec<String> },
    /// Arcane / Natural Recovery: once per its resource's rest, a short rest gives back spent slots
    /// worth half the class level (rounded up), none of 6th level or higher.
    SlotRecovery { resource: String },
    /// Its spells' damage adds `ability`'s modifier: spell `spell` (Agonizing Blast: eldritch
    /// blast, CHA) or every spell of school `school` (Empowered Evocation: evocation, INT).
    SpellDamageBonus {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        spell: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        school: String,
        ability: Ability,
    },
    /// Sculpt Spells: its allies in its spells of school `school` with a save take nothing.
    Sculpt { school: String },
    /// Potent Cantrip: a cantrip's saved damage is halved instead of none.
    PotentCantrip,
}

/// What a flat bonus (`FeatureEffect::Bonus`) adds to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BonusTo {
    #[default]
    Ac,
    /// Every saving throw.
    Saves,
    /// Weapon attack rolls.
    Attack,
    /// Weapon damage rolls.
    Damage,
    SpellAttack,
    SpellDc,
}

impl BonusTo {
    pub const ALL: [BonusTo; 6] = [BonusTo::Ac, BonusTo::Saves, BonusTo::Attack, BonusTo::Damage, BonusTo::SpellAttack, BonusTo::SpellDc];

    pub fn label(self) -> &'static str {
        match self {
            BonusTo::Ac => "Armor class",
            BonusTo::Saves => "Saving throws",
            BonusTo::Attack => "Attack rolls",
            BonusTo::Damage => "Damage rolls",
            BonusTo::SpellAttack => "Spell attacks",
            BonusTo::SpellDc => "Spell save DC",
        }
    }
}

/// When a class resource's uses come back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recharge {
    #[default]
    LongRest,
    ShortRest,
}

/// What a class action costs of the turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActCost {
    #[default]
    Action,
    Bonus,
    Reaction,
    /// Nothing (Action Surge, Reckless Attack).
    Free,
}

impl ActCost {
    pub fn label(self) -> &'static str {
        match self {
            ActCost::Action => "Action",
            ActCost::Bonus => "Bonus",
            ActCost::Reaction => "Reaction",
            ActCost::Free => "Free",
        }
    }
}

/// What a class action does. New variants are appended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ActEffect {
    /// Heals the user: `dice` + `per_level` × the class level (Second Wind, Wholeness of Body).
    Heal {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        dice: String,
        #[serde(default)]
        per_level: i32,
    },
    /// Heals a creature it touches (itself too) from the resource's pool, as much as it lacks
    /// (Lay on Hands).
    HealPool,
    /// `per_level` × the level shared among the allies within `feet` (itself too), each up to half
    /// its hit points at most (Preserve Life).
    HealAllies { feet: u32, per_level: i32 },
    /// Another action this turn (Action Surge).
    ExtraAction,
    /// Rage: resistance to bludgeoning, piercing and slashing damage, the class's
    /// `rage_damage_bonus` on STR melee hits, advantage on STR checks and saves; 10 rounds, over
    /// early after a turn without attacking or being hurt. With Frenzy, a bonus attack each turn.
    Rage,
    /// `count` unarmed strikes on a foe within reach, after taking the Attack action (Flurry of
    /// Blows: 2, Martial Arts: 1).
    UnarmedStrikes { count: u32 },
    /// One melee weapon attack, only while raging; exhaustion when the rage ends (Frenzy).
    FrenzyAttack,
    /// Dash, Disengage, Dodge or Hide at the act's cost (Cunning Action, Step of the Wind,
    /// Patient Defense): `dash`, `disengage`, `dodge`, `hide`.
    Basic { act: String },
    /// An ally within `feet` gets a die (the class number `number`: its sides) to add to its next
    /// attack roll or saving throw that would fail (Bardic Inspiration).
    GiveDie { feet: u32, number: String },
    /// Creatures within `feet` (of the `types` when listed; `single`: one foe) save `save`
    /// against 8 + proficiency + `ability`'s modifier or get `condition` for `rounds`; `destroy`:
    /// undead up to Destroy Undead's challenge rating are destroyed instead (Turn Undead,
    /// Turn the Unholy, Intimidating Presence).
    AreaSave {
        feet: u32,
        save: Ability,
        ability: Ability,
        condition: Condition,
        rounds: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        types: Vec<String>,
        #[serde(default)]
        single: bool,
        #[serde(default)]
        destroy: bool,
    },
    /// Its weapon attacks add `ability`'s modifier (at least 1) for `rounds` (Sacred Weapon).
    WeaponBuff { ability: Ability, rounds: u32 },
    /// Advantage on its STR melee attacks this turn; attacks against it have advantage until its
    /// next turn (Reckless Attack).
    Reckless,
    /// Temporary hit points: `dice` + `per_level` × the level.
    TempHp {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        dice: String,
        #[serde(default)]
        per_level: i32,
    },
    /// A script function (arguments: the user's name and the target's tile x, y).
    Script { function: String },
    /// Wild Shape: the user takes the shape of a beast it knows (defeated, or seen on this map)
    /// of a challenge rating up to `max_cr`'s step for its class level (`(2, "1/4"), (4, "1/2"),
    /// (8, "1")`), without a flying speed below level `fly_from` and a swimming speed below
    /// `swim_from`, for `hours_per_level` × the level hours (at least 1). Used again in the shape
    /// (a bonus action), it leaves it.
    Shape {
        max_cr: Vec<(u32, String)>,
        #[serde(default)]
        fly_from: u32,
        #[serde(default)]
        swim_from: u32,
        #[serde(default)]
        hours_per_level: f32,
    },
    /// A dragonborn's Breath Weapon: `by_level` damage dice (by the user's level), the type,
    /// shape and save of its draconic ancestry (acid and lightning a 30 ft line and DEX, poison
    /// and cold a 15 ft cone and CON, fire a 15 ft cone and DEX), DC 8 + CON + proficiency.
    Breath { by_level: Vec<(u32, String)> },
    /// Casts spell `spell` at slot level `level` without a slot (Infernal Legacy's Darkness).
    Spell { spell: String, #[serde(default)] level: u32 },
    /// A boon on the user for `rounds` rounds (Divine Sense: sensing celestials, fiends and undead).
    Boon { buff: crate::srd::conditions::Buff, rounds: u32 },
    /// Flexible Casting with points of resource `points` (sorcery points): `create` a spell slot
    /// (the lowest spent one it can pay: 2, 3, 5, 6, 7 points for levels 1-5), or turn its lowest
    /// slot into points (its level).
    FlexibleCasting { points: String, create: bool },
    /// Quickened Spell: for `cost` points of `points` the next spell of an action this turn is
    /// cast with the bonus action.
    Quicken { points: String, cost: i32 },
}

/// Who a class action is used on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActTarget {
    Myself,
    /// An ally (itself too when `myself`) within `tiles`.
    Ally { tiles: i32, myself: bool },
    /// A foe within `tiles`.
    Foe { tiles: i32 },
}

impl ActEffect {
    pub fn target(&self) -> ActTarget {
        match self {
            ActEffect::HealPool => ActTarget::Ally { tiles: 1, myself: true },
            ActEffect::GiveDie { feet, .. } => ActTarget::Ally { tiles: (*feet / 5).max(1) as i32, myself: false },
            ActEffect::UnarmedStrikes { .. } | ActEffect::FrenzyAttack => ActTarget::Foe { tiles: 1 },
            ActEffect::AreaSave { feet, single: true, .. } => ActTarget::Foe { tiles: (*feet / 5).max(1) as i32 },
            ActEffect::Breath { .. } => ActTarget::Foe { tiles: 6 },
            ActEffect::Spell { .. } => ActTarget::Foe { tiles: 24 },
            _ => ActTarget::Myself,
        }
    }
}

/// A class action a character has (from [`FeatureEffect::Act`]) and the feature it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassAct {
    pub id: String,
    pub name: String,
    pub cost: ActCost,
    pub resource: String,
    pub effect: ActEffect,
    pub feature: String,
}

/// The SRD's fighting styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FightingStyle {
    /// +2 to ranged attack rolls.
    Archery,
    /// +1 AC while wearing armor.
    Defense,
    /// +2 damage with a one-handed melee weapon and no other weapon.
    Dueling,
    /// Reroll 1 and 2 on a two-handed weapon's damage dice.
    GreatWeaponFighting,
    /// A reaction: disadvantage on an attack against an ally next to you (with a shield).
    Protection,
    /// The off-hand attack adds the ability modifier.
    TwoWeaponFighting,
}

/// The hero's (or a party member's) class.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeroClass {
    pub class: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subclass: String,
    /// Choices made: feature id -> what (expertise skills, a fighting style's feature id,
    /// `ability:str` / `feat:grappler` for an ability score improvement).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub picks: BTreeMap<String, Vec<String>>,
}

impl ClassDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> ClassDef {
        ClassDef { id: id.into(), name: "New class".into(), hit_die: 8, subclass_level: 3, levels: (0..20).map(|_| ClassLevel::default()).collect(), ..Default::default() }
    }

    pub fn feature(&self, id: &str) -> Option<&ClassFeature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn subclass(&self, id: &str) -> Option<&SubclassDef> {
        self.subclasses.iter().find(|s| s.id == id)
    }

    /// A number of the class at `level` (`rage_count`), else 0.
    pub fn number(&self, level: u32, name: &str) -> i64 {
        self.levels.get(level.clamp(1, 20) as usize - 1).and_then(|l| l.numbers.get(name)).copied().unwrap_or(0)
    }

    /// The feature ids a character of `level` with subclass `subclass` has, in level order:
    /// the class's, the subclass's, and the options it picked (`picks`).
    pub fn feature_ids(&self, level: u32, subclass: &str, picks: &BTreeMap<String, Vec<String>>) -> Vec<String> {
        let mut out: Vec<(u32, String)> = Vec::new();
        for (k, l) in self.levels.iter().enumerate().take(level as usize) {
            out.extend(l.features.iter().map(|f| (k as u32 + 1, f.clone())));
        }
        if let Some(s) = self.subclass(subclass) {
            out.extend(s.levels.iter().filter(|(l, _)| *l <= level).flat_map(|(l, fs)| fs.iter().map(move |f| (*l, f.clone()))));
        }
        out.sort_by_key(|(l, _)| *l);
        let mut ids: Vec<String> = out.into_iter().map(|(_, f)| f).collect();
        // Chosen options (a fighting style) count as features of their own.
        let chosen: Vec<String> = ids.iter().filter_map(|f| picks.get(f)).flatten().filter(|p| self.feature(p).is_some()).cloned().collect();
        ids.extend(chosen);
        ids
    }

    /// The cantrips and spells reaching `level` adds to what a caster knows: new cantrips when
    /// `cantrips_known` grows; spells when a known caster's `spells_known` grows, a wizard's 6 at
    /// level 1 and 2 a level (its spellbook); none for a preparing caster.
    pub fn spell_gains(&self, level: u32) -> (u32, u32) {
        let Some(sc) = &self.spellcasting else { return (0, 0) };
        let n = |l: u32, k: &str| if l == 0 { 0 } else { self.number(l, k).max(0) as u32 };
        let cantrips = n(level, "spellcasting.cantrips_known").saturating_sub(n(level - 1, "spellcasting.cantrips_known"));
        let spells = match sc.kind {
            CasterKind::Known | CasterKind::Pact => n(level, "spellcasting.spells_known").saturating_sub(n(level - 1, "spellcasting.spells_known")),
            CasterKind::Spellbook => {
                if level == 1 {
                    6
                } else {
                    2
                }
            }
            CasterKind::Prepared => 0,
        };
        (cantrips, spells)
    }

    /// Whether `level` brings an ability score improvement.
    pub fn improvement_at(&self, level: u32) -> bool {
        self.levels.get(level.clamp(1, 20) as usize - 1).is_some_and(|l| l.features.iter().any(|f| self.feature(f).is_some_and(|d| d.choice == Some(FeatureChoice::AbilityScores))))
    }
}

/// What a character's class features add up to (the passive ones).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassTraits {
    /// Unarmored armor class formulas: (base, abilities, shield allowed).
    pub armor: Vec<(i32, Vec<Ability>, bool)>,
    pub speed_ft: u32,
    pub speed_unarmored_ft: u32,
    pub proficiencies: Vec<String>,
    /// Abilities with half proficiency on unproficient checks; `all_half` = every ability.
    pub half_prof: Vec<Ability>,
    pub all_half: bool,
    pub reliable: bool,
    /// Critical hits from (20 = only a 20).
    pub crit_from: u32,
    pub attacks: u32,
    pub brutal_dice: u32,
    pub styles: Vec<FightingStyle>,
    pub evasion: bool,
    pub save_advantage: Vec<Ability>,
    pub initiative_advantage: bool,
    pub resistances: Vec<String>,
    pub immunities: Vec<String>,
    pub condition_immunities: Vec<String>,
    /// Aura: (feet, saves get CHA, immunity).
    pub aura: Option<(u32, bool, String)>,
    pub ability_bonus: Vec<(Ability, i32, i32)>,
    pub hp_per_level: i32,
    pub martial_arts: bool,
    /// Skills (and tools) with expertise.
    pub expertise: Vec<String>,
    /// The feature ids it has.
    pub features: Vec<String>,
    /// Its class level.
    pub level: u32,
    /// Resources (by id, the last definition of an id counts) and class actions.
    pub resources: Vec<FeatureEffect>,
    pub acts: Vec<ClassAct>,
    /// The acting effects that aren't resources or actions (riders, reactions), in level order.
    pub riders: Vec<FeatureEffect>,
    /// Flat bonuses by what they add to (magic items).
    pub bonuses: Vec<(BonusTo, i32)>,
    /// Ability scores of at least (ability, score).
    pub ability_scores: Vec<(Ability, i32)>,
    /// Skills checked with advantage.
    pub check_advantage: Vec<String>,
    /// Darkvision in feet (0 = none).
    pub darkvision_ft: u32,
    /// Special senses in feet (blindsight, truesight…; 0 feet = as far as it sees).
    pub senses: Vec<(crate::srd::conditions::Sense, u32)>,
}

impl ClassTraits {
    /// The most uses of resource `id` (None = no such resource): `abilities` give the modifier.
    pub fn resource_max(&self, c: &ClassDef, id: &str, abilities: &crate::srd::Abilities) -> Option<i32> {
        self.resources.iter().find_map(|r| match r {
            FeatureEffect::Resource { id: rid, count, number, ability, per_level, .. } if rid == id => {
                let n = if number.is_empty() { 0 } else { c.number(self.level, number) as i32 };
                Some(count + n + ability.map_or(0, |a| abilities.modifier(a).max(1)) + per_level * self.level as i32)
            }
            _ => None,
        })
    }

    /// When resource `id` comes back.
    pub fn recharge(&self, id: &str) -> Recharge {
        self.resources.iter().find_map(|r| if let FeatureEffect::Resource { id: rid, recharge, .. } = r { (rid == id).then_some(*recharge) } else { None }).unwrap_or_default()
    }

    /// The resource ids it has.
    pub fn resource_ids(&self) -> Vec<String> {
        self.resources.iter().filter_map(|r| if let FeatureEffect::Resource { id, .. } = r { Some(id.clone()) } else { None }).collect()
    }

    /// The first rider matching `f`.
    pub fn rider<T>(&self, f: impl Fn(&FeatureEffect) -> Option<T>) -> Option<T> {
        self.riders.iter().find_map(f)
    }

    /// Traits of a character without class features (only crits on a 20, one attack).
    pub fn plain() -> ClassTraits {
        ClassTraits { crit_from: 20, attacks: 1, ..Default::default() }
    }

    /// The sum of bonuses to `to`.
    pub fn bonus(&self, to: BonusTo) -> i32 {
        self.bonuses.iter().filter(|b| b.0 == to).map(|b| b.1).sum()
    }

    /// Adds effects (a feature's, a magic item's): `c` = the class (numbers by level), `feature`
    /// = where class actions come from.
    pub fn add_effects(&mut self, effects: &[FeatureEffect], c: Option<&ClassDef>, feature: &str) {
        let t = self;
        let level = t.level;
        for e in effects {
            match e {
                FeatureEffect::ArmorClass { base, abilities, shield } => t.armor.push((*base, abilities.clone(), *shield)),
                FeatureEffect::Speed { feet, unarmored: false } => t.speed_ft += feet,
                FeatureEffect::Speed { feet, unarmored: true } => {
                    // (0 = the class's `unarmored_movement` number at the level: the monk's.)
                    let ft = if *feet == 0 { c.map_or(0, |c| c.number(level, "unarmored_movement") as u32) } else { *feet };
                    t.speed_unarmored_ft = t.speed_unarmored_ft.max(ft);
                }
                FeatureEffect::Proficient { what } => t.proficiencies.push(what.clone()),
                FeatureEffect::HalfProficiency { abilities } if abilities.is_empty() => t.all_half = true,
                FeatureEffect::HalfProficiency { abilities } => t.half_prof.extend(abilities.iter().copied()),
                FeatureEffect::ReliableTalent => t.reliable = true,
                FeatureEffect::CritRange { from } => t.crit_from = t.crit_from.min(*from),
                FeatureEffect::ExtraAttacks { attacks } => t.attacks = t.attacks.max(*attacks),
                FeatureEffect::BrutalCritical { dice } => t.brutal_dice = t.brutal_dice.max(*dice),
                FeatureEffect::FightingStyle { style } => t.styles.push(*style),
                FeatureEffect::Evasion => t.evasion = true,
                FeatureEffect::SaveAdvantage { ability } => t.save_advantage.push(*ability),
                FeatureEffect::InitiativeAdvantage => t.initiative_advantage = true,
                FeatureEffect::Resistance { types } => t.resistances.extend(types.iter().cloned()),
                FeatureEffect::ConditionImmunity { conditions } => t.condition_immunities.extend(conditions.iter().cloned()),
                FeatureEffect::Aura { feet, save_bonus, immunity } => {
                    let (f0, s0, i0) = t.aura.clone().unwrap_or((0, false, String::new()));
                    t.aura = Some((f0.max(*feet), s0 || *save_bonus, if immunity.is_empty() { i0 } else { immunity.clone() }));
                }
                FeatureEffect::AbilityBonus { ability, amount, max } => t.ability_bonus.push((*ability, *amount, *max)),
                FeatureEffect::HpPerLevel { hp } => t.hp_per_level += hp,
                FeatureEffect::MartialArts => t.martial_arts = true,
                FeatureEffect::Immunity { types } => t.immunities.extend(types.iter().cloned()),
                FeatureEffect::Resource { id: rid, .. } => {
                    t.resources.retain(|r| !matches!(r, FeatureEffect::Resource { id: x, .. } if x == rid));
                    t.resources.push(e.clone());
                }
                FeatureEffect::Act { id: aid, name, cost, resource, effect } => {
                    t.acts.retain(|a| a.id != *aid);
                    t.acts.push(ClassAct { id: aid.clone(), name: name.clone(), cost: *cost, resource: resource.clone(), effect: effect.clone(), feature: feature.to_string() });
                }
                FeatureEffect::Bonus { to, amount } => t.bonuses.push((*to, *amount)),
                FeatureEffect::AbilityScore { ability, score } => t.ability_scores.push((*ability, *score)),
                FeatureEffect::CheckAdvantage { skill } => t.check_advantage.push(skill.clone()),
                FeatureEffect::Darkvision { feet } => t.darkvision_ft = t.darkvision_ft.max(*feet),
                FeatureEffect::Sense { sense, feet } => t.senses.push((*sense, *feet)),
                other => t.riders.push(other.clone()),
            }
        }
    }

    /// The passive effects of a character of `level` in class `c`.
    pub fn of(c: &ClassDef, level: u32, hc: &HeroClass) -> ClassTraits {
        let mut t = ClassTraits { level, ..ClassTraits::plain() };
        t.features = c.feature_ids(level, &hc.subclass, &hc.picks);
        for id in &t.features.clone() {
            let Some(f) = c.feature(id) else { continue };
            match f.choice {
                Some(FeatureChoice::Expertise { .. }) => t.expertise.extend(hc.picks.get(id).cloned().unwrap_or_default()),
                Some(FeatureChoice::Skills { .. }) => t.proficiencies.extend(hc.picks.get(id).cloned().unwrap_or_default()),
                _ => {}
            }
            t.add_effects(&f.effects, Some(c), id);
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_effects_add_to_the_traits() {
        let mut t = ClassTraits::plain();
        t.add_effects(
            &[
                FeatureEffect::Bonus { to: BonusTo::Ac, amount: 1 },
                FeatureEffect::Bonus { to: BonusTo::Saves, amount: 1 },
                FeatureEffect::Bonus { to: BonusTo::Ac, amount: 2 },
                FeatureEffect::AbilityScore { ability: Ability::Str, score: 21 },
                FeatureEffect::CheckAdvantage { skill: "stealth".into() },
                FeatureEffect::Darkvision { feet: 60 },
                FeatureEffect::Resistance { types: vec!["fire".into()] },
            ],
            None,
            "",
        );
        assert_eq!((t.bonus(BonusTo::Ac), t.bonus(BonusTo::Saves), t.bonus(BonusTo::Attack)), (3, 1, 0));
        assert_eq!((t.ability_scores.clone(), t.check_advantage.clone(), t.darkvision_ft), (vec![(Ability::Str, 21)], vec!["stealth".to_string()], 60));
        assert_eq!((t.resistances.clone(), t.crit_from, t.attacks), (vec!["fire".to_string()], 20, 1));
    }

    fn fighter() -> ClassDef {
        let mut c = ClassDef::fresh("fighter");
        c.hit_die = 10;
        c.levels[0].features = vec!["fighting-style".into(), "second-wind".into()];
        c.levels[2].features = vec!["martial-archetype".into()];
        c.levels[3].features = vec!["asi-1".into()];
        c.levels[4].features = vec!["extra-attack".into()];
        c.features = vec![
            ClassFeature { id: "fighting-style".into(), name: "Fighting Style".into(), level: 1, choice: Some(FeatureChoice::Options { choose: 1, options: vec!["style-defense".into(), "style-archery".into()] }), ..Default::default() },
            ClassFeature { id: "style-defense".into(), name: "Defense".into(), effects: vec![FeatureEffect::FightingStyle { style: FightingStyle::Defense }], ..Default::default() },
            ClassFeature { id: "style-archery".into(), name: "Archery".into(), effects: vec![FeatureEffect::FightingStyle { style: FightingStyle::Archery }], ..Default::default() },
            ClassFeature { id: "asi-1".into(), level: 4, choice: Some(FeatureChoice::AbilityScores), ..Default::default() },
            ClassFeature { id: "extra-attack".into(), level: 5, effects: vec![FeatureEffect::ExtraAttacks { attacks: 2 }], ..Default::default() },
            ClassFeature { id: "improved-critical".into(), level: 3, effects: vec![FeatureEffect::CritRange { from: 19 }], ..Default::default() },
        ];
        c.subclasses = vec![SubclassDef { id: "champion".into(), name: "Champion".into(), levels: vec![(3, vec!["improved-critical".into()])], ..Default::default() }];
        c
    }

    #[test]
    fn features_and_their_effects_add_up_by_level() {
        let c = fighter();
        let picks: BTreeMap<String, Vec<String>> = [("fighting-style".to_string(), vec!["style-archery".to_string()])].into();
        let hc = HeroClass { class: "fighter".into(), subclass: "champion".into(), picks };
        let t1 = ClassTraits::of(&c, 1, &hc);
        assert_eq!((t1.attacks, t1.crit_from, t1.styles.clone()), (1, 20, vec![FightingStyle::Archery]));
        let t5 = ClassTraits::of(&c, 5, &hc);
        assert_eq!((t5.attacks, t5.crit_from), (2, 19));
        assert!(t5.features.contains(&"improved-critical".to_string()) && !t5.features.contains(&"style-defense".to_string()));
        assert!(c.improvement_at(4) && !c.improvement_at(5));
        // The files keep what was set.
        let back: ClassDef = ron::from_str(&ron::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn acting_features_collect_resources_actions_and_riders() {
        let mut c = ClassDef::fresh("bard");
        c.levels[0].features = vec!["inspiration".into()];
        c.levels[4].features = vec!["font".into(), "strike".into()];
        c.levels[2].numbers.insert("die".into(), 6);
        let pool = |recharge| FeatureEffect::Resource { id: "insp".into(), count: 0, number: String::new(), ability: Some(Ability::Cha), per_level: 0, recharge };
        c.features = vec![
            ClassFeature { id: "inspiration".into(), level: 1, effects: vec![pool(Recharge::LongRest), FeatureEffect::Act { id: "insp".into(), name: "Inspire".into(), cost: ActCost::Bonus, resource: "insp".into(), effect: ActEffect::GiveDie { feet: 60, number: "die".into() } }], ..Default::default() },
            // (Font of Inspiration: the same resource back after a short rest.)
            ClassFeature { id: "font".into(), level: 5, effects: vec![pool(Recharge::ShortRest)], ..Default::default() },
            ClassFeature { id: "strike".into(), level: 5, effects: vec![FeatureEffect::ExtraDamage { dice: vec![(5, "1d8".into()), (14, "2d8".into())], damage_type: None, hurt_target: true }], ..Default::default() },
        ];
        let hc = HeroClass { class: "bard".into(), ..Default::default() };
        let mut a = crate::srd::Abilities::default();
        let t1 = ClassTraits::of(&c, 1, &hc);
        assert_eq!((t1.acts.len(), t1.recharge("insp"), t1.resource_max(&c, "insp", &a)), (1, Recharge::LongRest, Some(1)), "at least 1 from a low CHA");
        assert_eq!(t1.acts[0].effect.target(), ActTarget::Ally { tiles: 12, myself: false });
        a.set(Ability::Cha, 16);
        let t5 = ClassTraits::of(&c, 5, &hc);
        assert_eq!((t5.resources.len(), t5.recharge("insp"), t5.resource_max(&c, "insp", &a)), (1, Recharge::ShortRest, Some(3)));
        assert_eq!(t5.riders.len(), 1);
        assert_eq!(t5.resource_max(&c, "ki", &a), None);
        // A pool by level (Lay on Hands: 5 per level).
        let loh = ClassTraits { level: 4, resources: vec![FeatureEffect::Resource { id: "loh".into(), count: 0, number: String::new(), ability: None, per_level: 5, recharge: Recharge::LongRest }], ..Default::default() };
        assert_eq!(loh.resource_max(&c, "loh", &a), Some(20));
        let back: ClassDef = ron::from_str(&ron::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn casters_gain_cantrips_and_spells_by_level() {
        let mut c = ClassDef::fresh("wizard");
        c.spellcasting = Some(Spellcasting { ability: Ability::Int, kind: CasterKind::Spellbook, half: false, ritual: true });
        for (l, n) in [(1, 3), (4, 4), (10, 5)] {
            for k in l..=20 {
                c.levels[k - 1].numbers.insert("spellcasting.cantrips_known".into(), n);
            }
        }
        assert_eq!((c.spell_gains(1), c.spell_gains(2), c.spell_gains(4)), ((3, 6), (0, 2), (1, 2)));
        // A known caster: the growth of spells_known; a preparing one learns no spells.
        c.spellcasting.as_mut().unwrap().kind = CasterKind::Known;
        c.levels[0].numbers.insert("spellcasting.spells_known".into(), 4);
        c.levels[1].numbers.insert("spellcasting.spells_known".into(), 5);
        assert_eq!((c.spell_gains(1), c.spell_gains(2)), ((3, 4), (0, 1)));
        c.spellcasting.as_mut().unwrap().kind = CasterKind::Prepared;
        assert_eq!(c.spell_gains(2), (0, 0));
        assert_eq!(ClassDef::fresh("fighter").spell_gains(1), (0, 0));
    }

    #[test]
    fn passive_features_of_other_classes() {
        // A barbarian: unarmored defense 10 + DEX + CON (shield allowed), resistance while raging
        // comes with PR 2; a monk's martial arts and unarmored movement by level.
        let mut c = ClassDef::fresh("barbarian");
        c.levels[0].features = vec!["unarmored-defense".into()];
        c.levels[1].features = vec!["danger-sense".into()];
        c.levels[4].features = vec!["fast-movement".into()];
        c.levels[19].features = vec!["primal-champion".into()];
        c.features = vec![
            ClassFeature { id: "unarmored-defense".into(), level: 1, effects: vec![FeatureEffect::ArmorClass { base: 10, abilities: vec![Ability::Dex, Ability::Con], shield: true }], ..Default::default() },
            ClassFeature { id: "danger-sense".into(), level: 2, effects: vec![FeatureEffect::SaveAdvantage { ability: Ability::Dex }], ..Default::default() },
            ClassFeature { id: "fast-movement".into(), level: 5, effects: vec![FeatureEffect::Speed { feet: 10, unarmored: false }], ..Default::default() },
            ClassFeature { id: "primal-champion".into(), level: 20, effects: vec![FeatureEffect::AbilityBonus { ability: Ability::Str, amount: 4, max: 24 }], ..Default::default() },
        ];
        let hc = HeroClass { class: "barbarian".into(), ..Default::default() };
        let t1 = ClassTraits::of(&c, 1, &hc);
        assert_eq!(t1.armor, vec![(10, vec![Ability::Dex, Ability::Con], true)]);
        assert!(t1.save_advantage.is_empty() && t1.speed_ft == 0);
        let t5 = ClassTraits::of(&c, 5, &hc);
        assert_eq!((t5.save_advantage.clone(), t5.speed_ft), (vec![Ability::Dex], 10));
        assert!(t5.ability_bonus.is_empty());
        assert_eq!(ClassTraits::of(&c, 20, &hc).ability_bonus, vec![(Ability::Str, 4, 24)]);
        // Old defs files without the new fields load with their defaults.
        let old: ClassDef = ron::from_str("(id: \"x\", name: \"X\")").unwrap();
        assert_eq!((old.hit_die, old.levels.len()), (ClassDef::default().hit_die, ClassDef::default().levels.len()));
    }
}
