//! The SRD's *Traps*: what finds one (a passive or rolled Perception or Investigation against its
//! DC), what disarms it (DEX with Thieves' Tools), and what it does when it springs (an attack
//! roll or a saving throw, damage, a condition, a fall). Zones (`ZoneKind::Trap`) and props
//! (`PropDef::trap`) carry them; the runtime runs them (`runtime/traps.rs`).

use serde::{Deserialize, Serialize};

use super::{Ability, Condition, ConditionSpec, DamageType};

/// What finds a trap: looking (a tripwire, a pit's cover) or studying (a pressure plate, a lock).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Detect {
    #[default]
    Perception,
    Investigation,
}

impl Detect {
    pub fn skill(self) -> &'static str {
        match self {
            Detect::Perception => "perception",
            Detect::Investigation => "investigation",
        }
    }
}

/// A saving throw against a trap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrapSave {
    pub ability: Ability,
    pub dc: i32,
    /// Half the damage on a success (else none).
    #[serde(default)]
    pub half: bool,
}

/// When a sprung trap is armed again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Reset {
    /// Sprung for good.
    #[default]
    Once,
    /// Armed again after this many game hours.
    After { hours: f32 },
}

/// A trap (a zone's or a prop's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrapSpec {
    /// Which of the SRD's sample traps it started from ("" = made by hand; for the editor).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub preset: String,
    /// The DC that finds it (0 = it can't be found).
    pub detect_dc: i32,
    pub detect: Detect,
    /// The DEX (Thieves' Tools) DC that disarms it (0 = it can't be disarmed).
    pub disarm_dc: i32,
    /// An attack roll against the hero's AC with this bonus (0 = no attack: a save or nothing).
    #[serde(skip_serializing_if = "is_zero")]
    pub attack: i32,
    /// A saving throw.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<TrapSave>,
    /// Damage dice ("4d10"; "" = none) and its type.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub damage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<DamageType>,
    /// A second roll after a hit (the darts' and the needle's poison): a save, its damage, its type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub then_save: Option<TrapSave>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub then_damage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub then_type: Option<DamageType>,
    /// A condition on a hit or a failed save (a net's restrained, a needle's poisoned).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionSpec>,
    /// A fall of this many feet (a pit: 1d6 per 10 ft, prone).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fall_ft: u32,
    /// The line shown when it springs ("" = "A trap!").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    pub reset: Reset,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl Default for TrapSpec {
    fn default() -> Self {
        TrapSpec {
            preset: String::new(),
            detect_dc: 15,
            detect: Detect::Perception,
            disarm_dc: 15,
            attack: 0,
            save: None,
            damage: String::new(),
            damage_type: None,
            then_save: None,
            then_damage: String::new(),
            then_type: None,
            condition: None,
            fall_ft: 0,
            text: String::new(),
            reset: Reset::Once,
        }
    }
}

/// The SRD's sample traps: id and name.
pub const PRESETS: [(&str, &str); 7] = [
    ("collapsing_roof", "Collapsing roof"),
    ("falling_net", "Falling net"),
    ("fire_statue", "Fire-breathing statue"),
    ("hidden_pit", "Hidden pit"),
    ("poison_darts", "Poison darts"),
    ("poison_needle", "Poison needle"),
    ("rolling_sphere", "Rolling sphere"),
];

impl TrapSpec {
    /// One of the SRD's sample traps (`PRESETS`), `None` for an unknown id.
    pub fn preset(id: &str) -> Option<TrapSpec> {
        let save = |ability, dc, half| Some(TrapSave { ability, dc, half });
        let base = TrapSpec { preset: id.into(), ..Default::default() };
        Some(match id {
            "collapsing_roof" => TrapSpec { save: save(Ability::Dex, 15, true), damage: "4d10".into(), damage_type: Some(DamageType::Bludgeoning), text: "The roof collapses!".into(), ..base },
            "falling_net" => TrapSpec { detect_dc: 10, condition: Some(ConditionSpec::new(Condition::Restrained, 12.0)), text: "A net drops on you!".into(), ..base },
            "fire_statue" => TrapSpec { detect_dc: 15, detect: Detect::Investigation, disarm_dc: 13, save: save(Ability::Dex, 13, true), damage: "4d10".into(), damage_type: Some(DamageType::Fire), text: "Fire roars from the statue!".into(), ..base },
            "hidden_pit" => TrapSpec { fall_ft: 10, disarm_dc: 0, text: "The floor gives way!".into(), ..base },
            "poison_darts" => TrapSpec {
                detect: Detect::Investigation,
                attack: 8,
                damage: "1d4".into(),
                damage_type: Some(DamageType::Piercing),
                then_save: save(Ability::Con, 15, true),
                then_damage: "2d10".into(),
                then_type: Some(DamageType::Poison),
                text: "Darts shoot from the walls!".into(),
                ..base
            },
            "poison_needle" => TrapSpec {
                detect_dc: 20,
                detect: Detect::Investigation,
                damage: "1".into(),
                damage_type: Some(DamageType::Piercing),
                then_save: save(Ability::Con, 15, false),
                then_damage: "2d10".into(),
                then_type: Some(DamageType::Poison),
                condition: Some(ConditionSpec::new(Condition::Poisoned, 3600.0)),
                text: "A needle pricks your hand!".into(),
                ..base
            },
            "rolling_sphere" => TrapSpec { save: save(Ability::Dex, 15, true), damage: "10d10".into(), damage_type: Some(DamageType::Bludgeoning), text: "A stone sphere rolls at you!".into(), ..base },
            _ => return None,
        })
    }

    /// Damage after a save: all on a failure, half (rounded down) or none on a success.
    pub fn after_save(rolled: i32, saved: bool, half: bool) -> i32 {
        match (saved, half) {
            (false, _) => rolled,
            (true, true) => rolled / 2,
            (true, false) => 0,
        }
    }
}

/// A trap's state in a save game (`WorldState::traps`, by `map/uid`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrapState {
    /// The hero noticed it.
    pub found: bool,
    pub disarmed: bool,
    /// When it last sprang (game hours; None = never).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprung_at: Option<f64>,
    /// A failed disarming waits until then (game hours).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<f64>,
}

impl TrapState {
    /// Whether it springs now (not disarmed; not sprung, or reset since).
    pub fn armed(&self, reset: Reset, now: f64) -> bool {
        if self.disarmed {
            return false;
        }
        match (self.sprung_at, reset) {
            (None, _) => true,
            (Some(_), Reset::Once) => false,
            (Some(t), Reset::After { hours }) => now - t >= hours as f64,
        }
    }

    /// `traps=`'s word.
    pub fn word(&self, reset: Reset, now: f64) -> &'static str {
        if self.disarmed {
            "disarmed"
        } else if !self.armed(reset, now) {
            "sprung"
        } else if self.found {
            "found"
        } else {
            "armed"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traps_by_the_srd() {
        for (id, _) in PRESETS {
            assert!(TrapSpec::preset(id).is_some(), "{id}");
        }
        let darts = TrapSpec::preset("poison_darts").unwrap();
        assert_eq!((darts.attack, darts.damage.as_str(), darts.then_save.map(|s| s.dc)), (8, "1d4", Some(15)));
        let pit = TrapSpec::preset("hidden_pit").unwrap();
        assert_eq!((pit.fall_ft, pit.disarm_dc), (10, 0));
        assert_eq!(TrapSpec::preset("needle_in_a_haystack"), None);
        assert_eq!([TrapSpec::after_save(21, false, true), TrapSpec::after_save(21, true, true), TrapSpec::after_save(21, true, false)], [21, 10, 0]);
        // Armed, sprung, reset.
        let mut s = TrapState::default();
        assert!(s.armed(Reset::Once, 0.0));
        s.sprung_at = Some(10.0);
        assert!(!s.armed(Reset::Once, 100.0));
        assert!(!s.armed(Reset::After { hours: 8.0 }, 17.0) && s.armed(Reset::After { hours: 8.0 }, 18.0));
        s.disarmed = true;
        assert_eq!(s.word(Reset::After { hours: 8.0 }, 50.0), "disarmed");
        // Files keep only what differs; old files without traps load.
        let text = ron::to_string(&darts).unwrap();
        assert_eq!(ron::from_str::<TrapSpec>(&text).unwrap(), darts);
        assert_eq!(ron::from_str::<TrapState>("()").unwrap(), TrapState::default());
    }
}
