//! Ability checks and saving throws: what is rolled (`"stealth"`, `"dex"`, `"dex:thieves-tools"`,
//! a saving throw `"dex"`), the modifier from the character's scores and proficiencies, and the
//! record that goes into the roll log.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::abilities::{self, Abilities, Ability};
use super::dice::{Mode, Modifiers, D20};

/// What a character brings to a roll: scores, proficiencies, level, and what its class adds.
pub struct Roller<'a> {
    pub abilities: &'a Abilities,
    pub proficiencies: &'a BTreeSet<String>,
    pub level: u32,
    pub training: Option<&'a Training>,
}

/// What class features add to checks (`classes::ClassTraits`): expertise doubles the proficiency
/// bonus, half proficiency (Jack of All Trades: every ability; Remarkable Athlete: STR, DEX, CON)
/// adds half of it to checks without proficiency, Reliable Talent makes a d20 below 10 a 10 on
/// proficient checks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Training {
    pub expertise: BTreeSet<String>,
    pub half: Vec<Ability>,
    pub half_all: bool,
    pub reliable: bool,
    /// Saving throws with advantage (Danger Sense).
    pub save_advantage: Vec<Ability>,
    /// Armor the hero isn't proficient with: disadvantage on STR and DEX checks and saves.
    pub armor_untrained: bool,
    /// Armor with Stealth disadvantage worn.
    pub stealth_disadvantage: bool,
}

impl Modifiers for Roller<'_> {
    fn ability_mod(&self, a: Ability) -> i64 {
        self.abilities.modifier(a) as i64
    }
    fn proficiency(&self) -> i64 {
        abilities::proficiency_bonus(self.level) as i64
    }
}

/// A check or save, resolved: which ability, whether proficiency counts, its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub ability: Ability,
    /// The proficiency that adds the bonus ("" = none applies).
    pub proficiency: String,
    pub save: bool,
    /// "Persuasion", "DEX (Thieves' Tools)", "WIS save".
    pub label: String,
    /// The short id for status lines: "persuasion", "dex:thieves-tools", "save:wis".
    pub id: String,
}

impl Spec {
    /// Reads what to roll: a skill (`stealth`), an ability (`dex`), an ability with a tool or skill
    /// (`dex:thieves-tools`, `int:smiths-tools`), a tool alone (its usual ability); with `save`
    /// an ability's saving throw. `None` = nothing known by that name.
    pub fn parse(what: &str, save: bool) -> Option<Spec> {
        let w = what.trim();
        if save {
            let a = Ability::parse(w.strip_prefix("save:").unwrap_or(w))?;
            return Some(Spec { ability: a, proficiency: format!("save:{}", a.id()), save, label: format!("{} save", a.short()), id: format!("save:{}", a.id()) });
        }
        if let Some((a, p)) = w.split_once(':') {
            let a = Ability::parse(a)?;
            let id = abilities::proficiency_id(p);
            if id.is_empty() {
                return None;
            }
            let name = abilities::proficiency_name(&id);
            return Some(Spec { ability: a, label: format!("{} ({name})", a.short()), id: format!("{}:{id}", a.id()), proficiency: id, save });
        }
        if let Some(a) = Ability::parse(w) {
            return Some(Spec { ability: a, proficiency: String::new(), save, label: a.name().to_string(), id: a.id().to_string() });
        }
        if let Some((id, name, a)) = abilities::skill(w) {
            return Some(Spec { ability: a, proficiency: id.into(), save, label: name.into(), id: id.into() });
        }
        if let Some((id, name, a)) = abilities::tool(w) {
            return Some(Spec { ability: a, proficiency: id.into(), save, label: format!("{} ({name})", a.short()), id: format!("{}:{id}", a.id()) });
        }
        None
    }

    /// The bonus the roller adds: the ability modifier, plus proficiency when they have it
    /// (twice with expertise; half without it for a half-proficient ability check).
    pub fn modifier(&self, r: &Roller) -> i64 {
        let prof = self.proficient(r);
        let t = r.training;
        let bonus = if prof {
            if t.is_some_and(|t| t.expertise.contains(&self.proficiency)) { 2 * r.proficiency() } else { r.proficiency() }
        } else if !self.save && t.is_some_and(|t| t.half_all || t.half.contains(&self.ability)) {
            r.proficiency() / 2
        } else {
            0
        };
        r.ability_mod(self.ability) + bonus
    }

    fn proficient(&self, r: &Roller) -> bool {
        !self.proficiency.is_empty() && r.proficiencies.contains(&self.proficiency)
    }

    /// Rolls it against `dc`.
    pub fn roll(&self, r: &Roller, dc: i32, mode: Mode) -> RollRecord {
        // (Danger Sense: advantage on the save.)
        let mode = if self.save && r.training.is_some_and(|t| t.save_advantage.contains(&self.ability)) { mode.combine(Mode::Advantage) } else { mode };
        let mut d = D20::roll(mode);
        // Reliable Talent: a proficient check's d20 is at least 10.
        if !self.save && self.proficient(r) && r.training.is_some_and(|t| t.reliable) && d.kept < 10 {
            d.kept = 10;
        }
        let modifier = self.modifier(r);
        let total = d.kept as i64 + modifier;
        RollRecord {
            label: self.label.clone(),
            id: self.id.clone(),
            dice: format!("d20 ({})", d.text()),
            modifier,
            total,
            dc,
            success: Some(total >= dc as i64),
            natural: if d.kept == 20 || d.kept == 1 { d.kept as u8 } else { 0 },
            mode,
        }
    }
}

/// One line of the roll log (saved with the game, the last [`LOG_LEN`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RollRecord {
    /// "Persuasion", "2d6+STR".
    pub label: String,
    /// "persuasion", "2d6" (status lines).
    #[serde(default)]
    pub id: String,
    /// "d20 (14/6)", "2d6 (3, 5)".
    pub dice: String,
    pub modifier: i64,
    pub total: i64,
    /// The DC (0 = a plain roll).
    #[serde(default)]
    pub dc: i32,
    /// Checks and saves: whether it succeeded.
    #[serde(default)]
    pub success: Option<bool>,
    /// 20 or 1 when the d20 showed it, else 0.
    #[serde(default)]
    pub natural: u8,
    #[serde(default)]
    pub mode: Mode,
}

/// How many rolls the log keeps.
pub const LOG_LEN: usize = 50;

impl RollRecord {
    /// A plain roll of a dice expression.
    pub fn plain(label: &str, rolled: &super::dice::Rolled) -> RollRecord {
        let dice = if rolled.dice.is_empty() { String::new() } else { format!("({})", rolled.dice.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")) };
        let id = label.split(['+', '-']).next().unwrap_or(label).trim().to_ascii_lowercase();
        RollRecord { label: label.to_string(), id, dice, modifier: rolled.modifier, total: rolled.total, dc: 0, success: None, natural: 0, mode: Mode::Normal }
    }

    /// `persuasion:16/12:ok`, `persuasion:5/12:fail`, `2d6:7` (status lines).
    pub fn status(&self) -> String {
        match self.success {
            Some(ok) => format!("{}:{}/{}:{}{}", self.id, self.total, self.dc, if ok { "ok" } else { "fail" }, if self.natural > 0 { format!(":nat{}", self.natural) } else { String::new() }),
            None => format!("{}:{}", self.id, self.total),
        }
    }

    /// How much it beat the DC by (negative = failed by).
    pub fn margin(&self) -> i64 {
        self.total - self.dc as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srd::dice::seed;

    fn hero() -> (Abilities, BTreeSet<String>) {
        let profs: BTreeSet<String> = ["persuasion", "thieves-tools", "save:con"].iter().map(|s| s.to_string()).collect();
        (Abilities([8, 14, 13, 12, 10, 15]), profs)
    }

    #[test]
    fn specs_are_read() {
        let s = Spec::parse("Persuasion", false).unwrap();
        assert_eq!((s.ability, s.proficiency.as_str(), s.id.as_str()), (Ability::Cha, "persuasion", "persuasion"));
        let s = Spec::parse("dex:Thieves' Tools", false).unwrap();
        assert_eq!((s.ability, s.label.as_str(), s.id.as_str()), (Ability::Dex, "DEX (Thieves' Tools)", "dex:thieves-tools"));
        let s = Spec::parse("thieves-tools", false).unwrap();
        assert_eq!(s.ability, Ability::Dex);
        let s = Spec::parse("con", true).unwrap();
        assert_eq!((s.label.as_str(), s.proficiency.as_str()), ("CON save", "save:con"));
        assert_eq!(Spec::parse("wis", false).unwrap().proficiency, "");
        assert!(Spec::parse("cooking", false).is_none());
        assert!(Spec::parse("stealth", true).is_none());
    }

    #[test]
    fn modifiers_add_proficiency_only_when_proficient() {
        let (a, p) = hero();
        let lvl1 = Roller { abilities: &a, proficiencies: &p, level: 1, training: None };
        let lvl5 = Roller { abilities: &a, proficiencies: &p, level: 5, training: None };
        assert_eq!(Spec::parse("persuasion", false).unwrap().modifier(&lvl1), 2 + 2);
        assert_eq!(Spec::parse("persuasion", false).unwrap().modifier(&lvl5), 2 + 3);
        assert_eq!(Spec::parse("deception", false).unwrap().modifier(&lvl1), 2);
        assert_eq!(Spec::parse("dex:thieves-tools", false).unwrap().modifier(&lvl1), 2 + 2);
        assert_eq!(Spec::parse("str", false).unwrap().modifier(&lvl1), -1);
        assert_eq!(Spec::parse("con", true).unwrap().modifier(&lvl1), 1 + 2);
        assert_eq!(Spec::parse("dex", true).unwrap().modifier(&lvl1), 2);
    }

    #[test]
    fn rolls_meet_the_dc_and_report_naturals() {
        let (a, p) = hero();
        let r = Roller { abilities: &a, proficiencies: &p, level: 1, training: None };
        let s = Spec::parse("persuasion", false).unwrap();
        seed(3);
        let mut seen = (false, false);
        for _ in 0..400 {
            let rec = s.roll(&r, 12, Mode::Normal);
            assert_eq!(rec.success, Some(rec.total >= 12));
            assert_eq!(rec.modifier, 4);
            if rec.natural == 20 {
                seen.0 = true;
                assert_eq!(rec.total, 24);
            }
            if rec.natural == 1 {
                seen.1 = true;
                assert_eq!(rec.status(), "persuasion:5/12:fail:nat1");
            }
        }
        assert!(seen.0 && seen.1);
    }
}
