//! The SRD's damage types and what resistance, immunity and vulnerability do to damage of a type.

use serde::{Deserialize, Serialize};

/// The 13 damage types of the SRD. Stored by name (definitions, maps).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

impl DamageType {
    pub const ALL: [DamageType; 13] = [
        DamageType::Acid,
        DamageType::Bludgeoning,
        DamageType::Cold,
        DamageType::Fire,
        DamageType::Force,
        DamageType::Lightning,
        DamageType::Necrotic,
        DamageType::Piercing,
        DamageType::Poison,
        DamageType::Psychic,
        DamageType::Radiant,
        DamageType::Slashing,
        DamageType::Thunder,
    ];

    /// The SRD's index id (`fire`), as scripts, resistances and status lines name it.
    pub fn id(self) -> &'static str {
        match self {
            DamageType::Acid => "acid",
            DamageType::Bludgeoning => "bludgeoning",
            DamageType::Cold => "cold",
            DamageType::Fire => "fire",
            DamageType::Force => "force",
            DamageType::Lightning => "lightning",
            DamageType::Necrotic => "necrotic",
            DamageType::Piercing => "piercing",
            DamageType::Poison => "poison",
            DamageType::Psychic => "psychic",
            DamageType::Radiant => "radiant",
            DamageType::Slashing => "slashing",
            DamageType::Thunder => "thunder",
        }
    }

    /// "Fire".
    pub fn name(self) -> &'static str {
        match self {
            DamageType::Acid => "Acid",
            DamageType::Bludgeoning => "Bludgeoning",
            DamageType::Cold => "Cold",
            DamageType::Fire => "Fire",
            DamageType::Force => "Force",
            DamageType::Lightning => "Lightning",
            DamageType::Necrotic => "Necrotic",
            DamageType::Piercing => "Piercing",
            DamageType::Poison => "Poison",
            DamageType::Psychic => "Psychic",
            DamageType::Radiant => "Radiant",
            DamageType::Slashing => "Slashing",
            DamageType::Thunder => "Thunder",
        }
    }

    pub fn parse(s: &str) -> Option<DamageType> {
        let s = s.trim().to_ascii_lowercase();
        DamageType::ALL.into_iter().find(|t| t.id() == s)
    }
}

/// Whether a list of resistances (or immunities, vulnerabilities) covers a damage type. Entries
/// are type ids (`fire`) or the SRD's words ("bludgeoning, piercing, and slashing from nonmagical
/// attacks": every type named counts; magic weapons don't exist yet); `all` covers every type.
pub fn covers(list: &[String], t: DamageType) -> bool {
    list.iter().any(|r| {
        let r = r.to_ascii_lowercase();
        r == "all" || r.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w == t.id())
    })
}

/// What a creature does with damage of a type.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Defenses {
    pub resistances: Vec<String>,
    pub immunities: Vec<String>,
    pub vulnerabilities: Vec<String>,
}

impl Defenses {
    /// Damage of type `t` after the defenses (untyped damage is unchanged): immunity = 0,
    /// resistance = half (rounded down), vulnerability = double; resistance and vulnerability
    /// together cancel out.
    pub fn apply(&self, amount: i32, t: Option<DamageType>) -> i32 {
        let Some(t) = t else { return amount };
        if amount <= 0 {
            return amount;
        }
        if covers(&self.immunities, t) {
            return 0;
        }
        match (covers(&self.resistances, t), covers(&self.vulnerabilities, t)) {
            (true, false) => amount / 2,
            (false, true) => amount * 2,
            _ => amount,
        }
    }

    /// `resistant to fire` style word for the damage number (`None` = nothing special).
    pub fn effect(&self, t: Option<DamageType>) -> Option<&'static str> {
        let t = t?;
        if covers(&self.immunities, t) {
            return Some("immune");
        }
        match (covers(&self.resistances, t), covers(&self.vulnerabilities, t)) {
            (true, false) => Some("resisted"),
            (false, true) => Some("vulnerable"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_defenses() {
        for t in DamageType::ALL {
            assert_eq!(DamageType::parse(t.id()), Some(t));
        }
        assert_eq!(DamageType::parse(" Fire "), Some(DamageType::Fire));
        let d = Defenses { resistances: vec!["poison".into(), "bludgeoning, piercing, and slashing from nonmagical attacks".into()], immunities: vec!["psychic".into()], vulnerabilities: vec!["fire".into(), "poison".into()] };
        assert_eq!(d.apply(7, Some(DamageType::Slashing)), 3, "half, rounded down");
        assert_eq!(d.apply(1, Some(DamageType::Piercing)), 0);
        assert_eq!(d.apply(7, Some(DamageType::Psychic)), 0, "immune");
        assert_eq!(d.apply(7, Some(DamageType::Fire)), 14, "vulnerable");
        assert_eq!(d.apply(7, Some(DamageType::Poison)), 7, "both cancel out");
        assert_eq!(d.apply(7, Some(DamageType::Cold)), 7);
        assert_eq!(d.apply(7, None), 7, "untyped");
        assert_eq!(d.effect(Some(DamageType::Slashing)), Some("resisted"));
        assert!(covers(&["all".into()], DamageType::Thunder));
        assert!(!covers(&["firearms".into()], DamageType::Fire), "whole words");
    }
}
