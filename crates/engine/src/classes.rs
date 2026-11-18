//! Classes (`defs/classes.ron`, SRD 5.1): what the hero and party members are trained as. A class
//! gives the hit die, saving throws, armor / weapon / tool proficiencies, skill choices, starting
//! equipment and features by level (20 levels), and has subclasses with features of their own.
//! A feature is the SRD's text plus, where it acts, effects from [`FeatureEffect`] (the passive
//! ones change the numbers; `runtime/` reads them through [`ClassTraits`]). The hero's class is
//! `WorldState::class` ([`HeroClass`]); a party member's is `CharacterDef::class`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;
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
}

impl ClassTraits {
    /// The passive effects of a character of `level` in class `c`.
    pub fn of(c: &ClassDef, level: u32, hc: &HeroClass) -> ClassTraits {
        let mut t = ClassTraits { crit_from: 20, attacks: 1, ..Default::default() };
        t.features = c.feature_ids(level, &hc.subclass, &hc.picks);
        for id in &t.features {
            let Some(f) = c.feature(id) else { continue };
            match f.choice {
                Some(FeatureChoice::Expertise { .. }) => t.expertise.extend(hc.picks.get(id).cloned().unwrap_or_default()),
                Some(FeatureChoice::Skills { .. }) => t.proficiencies.extend(hc.picks.get(id).cloned().unwrap_or_default()),
                _ => {}
            }
            for e in &f.effects {
                match e {
                    FeatureEffect::ArmorClass { base, abilities, shield } => t.armor.push((*base, abilities.clone(), *shield)),
                    FeatureEffect::Speed { feet, unarmored: false } => t.speed_ft += feet,
                    FeatureEffect::Speed { feet, unarmored: true } => {
                        // (0 = the class's `unarmored_movement` number at the level: the monk's.)
                        let ft = if *feet == 0 { c.number(level, "unarmored_movement") as u32 } else { *feet };
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
                }
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
