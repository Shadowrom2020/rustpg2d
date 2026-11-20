//! The SRD 5.1 rules (System Reference Document 5.1 by Wizards of the Coast LLC, CC BY 4.0):
//! dice, ability scores, skills, proficiency, ability checks and saving throws. Pure logic; the
//! player's scores and proficiencies are in [`crate::state::WorldState`], which rolls with them.

pub mod abilities;
pub mod check;
pub mod combat;
pub mod encounter;
pub mod equipment;
pub mod magic_items;
pub mod conditions;
pub mod damage;
pub mod dice;
pub mod rules;
pub mod statblock;
pub mod survival;

pub use abilities::{Abilities, Ability};
pub use check::{RollRecord, Spec};
pub use conditions::{ActiveCondition, Condition, ConditionSpec};
pub use damage::{DamageType, Defenses, HitKind};
pub use dice::{Expr, Mode};
pub use rules::{ATTRIBUTION, ATTRIBUTION_FILE};
pub use statblock::{npc_template, npc_templates, DamagePart, FeatureKind, HitRider, Size, Speeds, StatAttack, StatBlock, StatFeature, StatSpell, StatSpellcasting};

/// The rules' conversion version of projects made or converted by this editor
/// (`ProjectConfig::rules_version`): 1 = the skill tree became SRD proficiencies (0.28); 2 = the
/// status effects became conditions, and those without one (haste, strength, weakness,
/// shield) were taken out (0.28); 3 = spells became the SRD's (slots instead of mana: their
/// old effects as spell parts, 0.30); 4 = weapons and armor became the SRD's (categories,
/// properties, the armor table: 0.31).
pub const RULES_VERSION: u32 = 4;

/// A lock or crafting difficulty of the old skill levels (0.27 and older: 1 = easy … 9) as an
/// SRD DC: 10, 12, 15, 18, 20, 22, 25, 28, 30. 0 stays 0.
pub fn old_level_dc(level: u32) -> u32 {
    const DC: [u32; 9] = [10, 12, 15, 18, 20, 22, 25, 28, 30];
    match level {
        0 => 0,
        l => DC[(l as usize - 1).min(8)],
    }
}

/// Settings of the rules (`project.ron` `rules`): what a new game's hero starts with until the
/// character creator sets it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RulesSettings {
    /// Ability scores of a new hero, STR to CHA.
    pub abilities: Abilities,
    /// Proficiencies of a new hero: skills (`perception`), tools (`thieves-tools`), saving throws
    /// (`save:str`).
    pub proficiencies: Vec<String>,
    /// A short line on screen for every check and saving throw ("Perception 14 vs DC 12: success").
    pub show_checks: bool,
    /// Food and water count: days without food beyond 3 + CON, or less than a day's water, give
    /// exhaustion (the SRD's *Food and Water*).
    pub food_water: bool,
    /// Walking more than eight hours between long rests calls for CON saves (the SRD's *Forced
    /// March*).
    pub forced_march: bool,
    /// How carrying is limited: the plain rule (STR × 15 lb) or the variant encumbrance.
    pub encumbrance: survival::Encumbrance,
    /// What the hero needs in the bag to make camp (a long rest anywhere): an item id or SRD id;
    /// "" = nothing.
    #[serde(default = "default_camp_item")]
    pub camp_item: String,
}

fn default_camp_item() -> String {
    "bedroll".into()
}

impl Default for RulesSettings {
    fn default() -> Self {
        RulesSettings { abilities: Abilities::STANDARD, proficiencies: Vec::new(), show_checks: true, food_water: false, forced_march: false, encumbrance: survival::Encumbrance::Plain, camp_item: default_camp_item() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_levels_become_dcs() {
        assert_eq!([0, 1, 2, 3, 5, 9, 12].map(old_level_dc), [0, 10, 12, 15, 20, 30, 30]);
    }

    #[test]
    fn old_lock_levels_become_dcs() {
        let l: crate::defs::LockSpec = ron::from_str("(key: \"\", pick_difficulty: 2)").unwrap();
        assert_eq!(l.pick_dc, 12);
        let l: crate::defs::LockSpec = ron::from_str("(key: \"k\", pick_dc: 14, relock: true)").unwrap();
        assert_eq!((l.pick_dc, l.relock, l.consume_key), (14, true, true));
        let text = ron::to_string(&l).unwrap();
        assert!(text.contains("pick_dc:14") && !text.contains("pick_difficulty"), "{text}");
        let l: crate::defs::LockSpec = ron::from_str("(key: \"k\")").unwrap();
        assert_eq!(l, crate::defs::LockSpec { key: "k".into(), ..Default::default() });
    }

    #[test]
    fn rules_settings_default_in_old_projects() {
        let r: RulesSettings = ron::from_str("()").unwrap();
        assert_eq!(r, RulesSettings::default());
        let r: RulesSettings = ron::from_str("(abilities: (8, 14, 13, 12, 10, 15), proficiencies: [\"stealth\"])").unwrap();
        assert_eq!(r.abilities.score(Ability::Cha), 15);
        assert!(r.show_checks);
    }
}
