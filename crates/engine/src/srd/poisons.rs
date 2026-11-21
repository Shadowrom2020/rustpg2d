//! The SRD's *Poisons*: contact, ingested, inhaled and injury poisons with their Constitution
//! save, damage and conditions. Items carry them (`ItemDef::poison`); the runtime coats weapons
//! with injury and contact poisons and exposes creatures to them (`runtime/poisons.rs`).

use serde::{Deserialize, Serialize};

use super::{Condition, ConditionSpec};

/// How a poison gets into a creature.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoisonKind {
    /// Smeared on an object or a weapon: touching it exposes (it coats a weapon).
    Contact,
    /// Swallowed: food or drink.
    Ingested,
    /// A powder or gas, breathed in.
    Inhaled,
    /// On a weapon or a piece of ammunition: a wound it deals exposes (it coats a weapon).
    #[default]
    Injury,
}

impl PoisonKind {
    pub const ALL: [PoisonKind; 4] = [PoisonKind::Contact, PoisonKind::Ingested, PoisonKind::Inhaled, PoisonKind::Injury];

    pub fn label(self) -> &'static str {
        match self {
            PoisonKind::Contact => "Contact",
            PoisonKind::Ingested => "Ingested",
            PoisonKind::Inhaled => "Inhaled",
            PoisonKind::Injury => "Injury",
        }
    }

    /// Whether it coats a weapon or ammunition when used.
    pub fn coats(self) -> bool {
        matches!(self, PoisonKind::Contact | PoisonKind::Injury)
    }
}

/// A poison: a Constitution save against its DC, its damage and its conditions on a failure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PoisonDef {
    /// Which of the SRD's poisons it started from ("" = made by hand; for the editor).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub preset: String,
    pub kind: PoisonKind,
    /// The Constitution save's DC.
    pub dc: i32,
    /// Poison damage on a failed save ("" = none).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub damage: String,
    /// Half the damage on a successful save (else none).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub half: bool,
    /// The conditions a failed save gives (poisoned for an hour, unconscious while poisoned).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ConditionSpec>,
    /// More conditions when the save fails by 5 or more (Drow Poison: unconscious).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub worse: Vec<ConditionSpec>,
    /// How long a coating stays potent, in minutes (0 = until a hit delivers it).
    #[serde(skip_serializing_if = "is_zero")]
    pub minutes: f32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl Default for PoisonDef {
    fn default() -> Self {
        PoisonDef { preset: String::new(), kind: PoisonKind::Injury, dc: 10, damage: String::new(), half: false, effects: Vec::new(), worse: Vec::new(), minutes: 0.0 }
    }
}

/// The SRD's poisons: id (the SRD's item index), name, price in gold pieces.
pub const PRESETS: [(&str, &str, u32); 16] = [
    ("poison-basic-vial", "Poison, basic (vial)", 100),
    ("assassins-blood", "Assassin's Blood", 150),
    ("burnt-othur-fumes", "Burnt Othur Fumes", 500),
    ("crawler-mucus", "Crawler Mucus", 200),
    ("drow-poison", "Drow Poison", 200),
    ("essence-of-ether", "Essence of Ether", 300),
    ("malice", "Malice", 250),
    ("midnight-tears", "Midnight Tears", 1500),
    ("oil-of-taggit", "Oil of Taggit", 400),
    ("pale-tincture", "Pale Tincture", 250),
    ("purple-worm-poison", "Purple Worm Poison", 2000),
    ("serpent-venom", "Serpent Venom", 200),
    ("torpor", "Torpor", 600),
    ("truth-serum", "Truth Serum", 150),
    ("wyvern-poison", "Wyvern Poison", 1200),
    ("potion-of-poison", "Potion of Poison", 0),
];

const HOUR: f32 = 3600.0;

impl PoisonDef {
    /// One of the SRD's poisons (`PRESETS`), `None` for an unknown id. Repeated saves are a fixed
    /// duration here: Crawler Mucus paralyzes for 2 rounds, Burnt Othur Fumes burns for 3 rounds,
    /// Pale Tincture is one dose; Midnight Tears acts at once; Torpor lasts 14 hours (4d6's
    /// average). The unconscious wake when hurt (`runtime/poisons.rs`).
    pub fn preset(id: &str) -> Option<PoisonDef> {
        use PoisonKind::*;
        let poisoned = |s: f32| ConditionSpec::new(Condition::Poisoned, s);
        let base = PoisonDef { preset: id.into(), ..Default::default() };
        Some(match id {
            "poison-basic-vial" => PoisonDef { kind: Injury, dc: 10, damage: "1d4".into(), minutes: 1.0, ..base },
            "assassins-blood" => PoisonDef { kind: Ingested, dc: 10, damage: "1d12".into(), half: true, effects: vec![poisoned(24.0 * HOUR)], ..base },
            "burnt-othur-fumes" => {
                let burn = ConditionSpec { condition: None, seconds: 3.0 * super::conditions::ROUND, level: 0, save: None, ongoing: Some(super::conditions::Ongoing { dice: "1d6".into(), damage_type: Some(super::DamageType::Poison) }), buff: None };
                PoisonDef { kind: Inhaled, dc: 13, damage: "3d6".into(), effects: vec![burn], ..base }
            }
            "crawler-mucus" => PoisonDef { kind: Contact, dc: 13, effects: vec![poisoned(2.0 * super::conditions::ROUND), ConditionSpec::new(Condition::Paralyzed, 2.0 * super::conditions::ROUND)], ..base },
            "drow-poison" => PoisonDef { kind: Injury, dc: 13, effects: vec![poisoned(HOUR)], worse: vec![ConditionSpec::new(Condition::Unconscious, HOUR)], ..base },
            "essence-of-ether" => PoisonDef { kind: Inhaled, dc: 15, effects: vec![poisoned(8.0 * HOUR), ConditionSpec::new(Condition::Unconscious, 8.0 * HOUR)], ..base },
            "malice" => PoisonDef { kind: Inhaled, dc: 15, effects: vec![poisoned(HOUR), ConditionSpec::new(Condition::Blinded, HOUR)], ..base },
            "midnight-tears" => PoisonDef { kind: Ingested, dc: 17, damage: "9d6".into(), half: true, ..base },
            "oil-of-taggit" => PoisonDef { kind: Contact, dc: 13, effects: vec![poisoned(24.0 * HOUR), ConditionSpec::new(Condition::Unconscious, 24.0 * HOUR)], ..base },
            "pale-tincture" => PoisonDef { kind: Ingested, dc: 16, damage: "1d6".into(), effects: vec![poisoned(24.0 * HOUR)], ..base },
            "purple-worm-poison" => PoisonDef { kind: Injury, dc: 19, damage: "12d6".into(), half: true, ..base },
            "serpent-venom" => PoisonDef { kind: Injury, dc: 11, damage: "3d6".into(), half: true, ..base },
            "torpor" => PoisonDef { kind: Ingested, dc: 15, effects: vec![poisoned(14.0 * HOUR), ConditionSpec::new(Condition::Incapacitated, 14.0 * HOUR)], ..base },
            "truth-serum" => PoisonDef { kind: Ingested, dc: 11, effects: vec![poisoned(HOUR)], ..base },
            "wyvern-poison" => PoisonDef { kind: Injury, dc: 15, damage: "7d6".into(), half: true, ..base },
            "potion-of-poison" => PoisonDef { kind: Ingested, dc: 13, damage: "3d6".into(), half: true, effects: vec![poisoned(60.0)], ..base },
            _ => return None,
        })
    }

    /// What a save does: the damage dealt of `rolled` (all on a failure, half or none on a
    /// success) and the conditions given (`effects` on a failure, `worse` too when it failed by
    /// 5 or more).
    pub fn outcome(&self, rolled: i32, saved: bool, missed_by: i64) -> (i32, Vec<ConditionSpec>) {
        let dmg = match (saved, self.half) {
            (false, _) => rolled,
            (true, true) => rolled / 2,
            (true, false) => 0,
        };
        let mut given = Vec::new();
        if !saved {
            given.extend(self.effects.iter().cloned());
            if missed_by >= 5 {
                given.extend(self.worse.iter().cloned());
            }
        }
        (dmg, given)
    }

    /// "CON DC 13, 3d6 poison (half), poisoned 1 h" (the editor's line, status).
    pub fn summary(&self) -> String {
        let mut s = format!("CON DC {}", self.dc);
        if !self.damage.is_empty() {
            s.push_str(&format!(", {} poison{}", self.damage, if self.half { " (half)" } else { "" }));
        }
        for e in &self.effects {
            s.push_str(&format!(", {}", e.summary()));
        }
        s
    }
}

/// A weapon or ammunition coated with a poison (`WorldState::coating`): one dose, delivered by
/// the next hit with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Coating {
    /// The poison's item.
    pub poison: String,
    /// The coated weapon's or ammunition's item.
    pub on: String,
    /// Seconds until it dries, counted like conditions (`None` = it lasts until delivered).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<f32>,
}

impl Coating {
    /// `dt` seconds pass; false when it dried.
    pub fn tick(&mut self, dt: f32) -> bool {
        match &mut self.left {
            Some(l) => {
                *l -= dt;
                *l > 0.0
            }
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisons_by_the_srd() {
        for (id, _, _) in PRESETS {
            assert!(PoisonDef::preset(id).is_some(), "{id}");
        }
        let wyvern = PoisonDef::preset("wyvern-poison").unwrap();
        assert_eq!((wyvern.kind, wyvern.dc, wyvern.damage.as_str(), wyvern.half), (PoisonKind::Injury, 15, "7d6", true));
        let basic = PoisonDef::preset("poison-basic-vial").unwrap();
        assert_eq!((basic.dc, basic.damage.as_str(), basic.minutes), (10, "1d4", 1.0));
        assert!(basic.kind.coats() && !PoisonDef::preset("malice").unwrap().kind.coats());
        assert_eq!(PoisonDef::preset("snake-oil"), None);
        // A failed save: all the damage and the conditions; by 5 or more the worse ones too.
        let drow = PoisonDef::preset("drow-poison").unwrap();
        assert_eq!(drow.outcome(0, false, 2).1.len(), 1);
        assert_eq!(drow.outcome(0, false, 5).1.iter().filter_map(|c| c.condition).collect::<Vec<_>>(), [Condition::Poisoned, Condition::Unconscious]);
        assert_eq!(drow.outcome(0, true, 0).1, vec![]);
        assert_eq!([wyvern.outcome(25, false, 1).0, wyvern.outcome(25, true, 0).0, basic.outcome(3, true, 0).0], [25, 12, 0]);
        // Files keep only what differs.
        let text = ron::to_string(&drow).unwrap();
        assert!(!text.contains("minutes") && !text.contains("half"), "{text}");
        assert_eq!(ron::from_str::<PoisonDef>(&text).unwrap(), drow);
        assert_eq!(ron::from_str::<PoisonDef>("()").unwrap(), PoisonDef::default());
        // A coating dries after its minutes; one without lasts until delivered.
        let mut c = Coating { poison: "poison-basic-vial".into(), on: "dagger".into(), left: Some(60.0) };
        assert!(c.tick(54.0) && !c.tick(6.0));
        let mut c = Coating { left: None, ..c };
        assert!(c.tick(1e6));
    }
}
