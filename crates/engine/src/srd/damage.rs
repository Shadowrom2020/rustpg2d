//! The SRD's damage types and what resistance, immunity and vulnerability do to damage of a type.

use serde::{Deserialize, Serialize};

/// The 13 damage types of the SRD. Stored by name (definitions, maps).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// What made a hit: a plain weapon, a silvered one, or magic (a magic weapon or ammunition, a
/// spell, a monster's magical attack). Resistances "from nonmagical attacks (that aren't
/// silvered)" don't cover the latter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HitKind {
    #[default]
    Plain,
    Silvered,
    Magical,
}

/// Whether a list of resistances (or immunities, vulnerabilities) covers a damage type. Entries
/// are type ids (`fire`) or the SRD's words ("bludgeoning, piercing, and slashing from nonmagical
/// attacks": every type named counts); `all` covers every type. A plain hit.
pub fn covers(list: &[String], t: DamageType) -> bool {
    covers_hit(list, t, HitKind::Plain)
}

/// [`covers`] for a hit of a kind: an entry about nonmagical attacks doesn't cover a magical hit,
/// one about attacks "that aren't silvered" not a silvered one either ("that aren't adamantine"
/// stays: adamantine weapons aren't SRD items).
pub fn covers_hit(list: &[String], t: DamageType, hit: HitKind) -> bool {
    list.iter().any(|r| {
        let r = r.to_ascii_lowercase();
        if hit == HitKind::Magical && r.contains("nonmagical") {
            return false;
        }
        if hit == HitKind::Silvered && r.contains("silvered") {
            return false;
        }
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
        self.apply_hit(amount, t, HitKind::Plain)
    }

    /// [`Self::apply`] for a hit of a kind (a magic weapon gets through "nonmagical" resistances).
    pub fn apply_hit(&self, amount: i32, t: Option<DamageType>, hit: HitKind) -> i32 {
        let Some(t) = t else { return amount };
        if amount <= 0 {
            return amount;
        }
        if covers_hit(&self.immunities, t, hit) {
            return 0;
        }
        match (covers_hit(&self.resistances, t, hit), covers_hit(&self.vulnerabilities, t, hit)) {
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
        // A magic weapon gets through resistance to nonmagical attacks, a silvered one through
        // "that aren't silvered"; adamantine stays covered.
        assert_eq!(d.apply_hit(8, Some(DamageType::Slashing), HitKind::Magical), 8);
        assert_eq!(d.apply_hit(8, Some(DamageType::Slashing), HitKind::Silvered), 4, "silvering doesn't count here");
        let were = Defenses { resistances: vec!["bludgeoning, piercing, and slashing from nonmagical attacks that aren't silvered".into()], ..Default::default() };
        assert_eq!(were.apply_hit(8, Some(DamageType::Piercing), HitKind::Silvered), 8);
        assert_eq!(were.apply_hit(8, Some(DamageType::Piercing), HitKind::Plain), 4);
        let golem = Defenses { immunities: vec!["bludgeoning, piercing, and slashing from nonmagical attacks that aren't adamantine".into()], ..Default::default() };
        assert_eq!(golem.apply_hit(8, Some(DamageType::Piercing), HitKind::Silvered), 0);
        assert_eq!(golem.apply_hit(8, Some(DamageType::Piercing), HitKind::Magical), 8);
        assert_eq!(d.apply(7, None), 7, "untyped");
        assert_eq!(d.effect(Some(DamageType::Slashing)), Some("resisted"));
        assert!(covers(&["all".into()], DamageType::Thunder));
        assert!(!covers(&["firearms".into()], DamageType::Fire), "whole words");
    }
}
