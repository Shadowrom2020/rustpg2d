//! The SRD's *Objects* (statistics for objects): armor class by material, hit points by size
//! (fragile or resilient), damage thresholds, immunity to poison and psychic damage. Props carry
//! them as `PropDef::object`; the runtime breaks and forces them (`runtime/objects.rs`).

use serde::{Deserialize, Serialize};

use super::DamageType;

/// What an object is made of: its armor class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Material {
    /// Cloth, paper, rope: AC 11.
    Cloth,
    /// Crystal, glass, ice: AC 13.
    Glass,
    /// Wood, bone: AC 15.
    #[default]
    Wood,
    /// Stone: AC 17.
    Stone,
    /// Iron, steel: AC 19.
    Iron,
    /// Mithral: AC 21.
    Mithral,
    /// Adamantine: AC 23.
    Adamantine,
}

impl Material {
    pub const ALL: [Material; 7] = [Material::Cloth, Material::Glass, Material::Wood, Material::Stone, Material::Iron, Material::Mithral, Material::Adamantine];

    /// The SRD's armor class of the material.
    pub fn ac(self) -> i32 {
        match self {
            Material::Cloth => 11,
            Material::Glass => 13,
            Material::Wood => 15,
            Material::Stone => 17,
            Material::Iron => 19,
            Material::Mithral => 21,
            Material::Adamantine => 23,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Material::Cloth => "Cloth, paper, rope",
            Material::Glass => "Crystal, glass, ice",
            Material::Wood => "Wood, bone",
            Material::Stone => "Stone",
            Material::Iron => "Iron, steel",
            Material::Mithral => "Mithral",
            Material::Adamantine => "Adamantine",
        }
    }
}

/// How big an object is: its hit points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ObjectSize {
    /// A bottle, a lock.
    Tiny,
    /// A chest, a lute.
    Small,
    /// A barrel, a chandelier, a door.
    #[default]
    Medium,
    /// A cart, a 10-ft-by-10-ft window.
    Large,
}

impl ObjectSize {
    pub const ALL: [ObjectSize; 4] = [ObjectSize::Tiny, ObjectSize::Small, ObjectSize::Medium, ObjectSize::Large];

    /// The SRD's average hit points: fragile or resilient.
    pub fn hp(self, resilient: bool) -> i32 {
        match (self, resilient) {
            (ObjectSize::Tiny, false) => 2,
            (ObjectSize::Tiny, true) => 5,
            (ObjectSize::Small, false) => 3,
            (ObjectSize::Small, true) => 10,
            (ObjectSize::Medium, false) => 4,
            (ObjectSize::Medium, true) => 18,
            (ObjectSize::Large, false) => 5,
            (ObjectSize::Large, true) => 27,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ObjectSize::Tiny => "Tiny",
            ObjectSize::Small => "Small",
            ObjectSize::Medium => "Medium",
            ObjectSize::Large => "Large",
        }
    }
}

/// What breaking an object does.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Broken {
    /// It is gone (a crate, a barricade).
    #[default]
    Disappear,
    /// It switches to one of its states (a door's open one, a broken chest).
    ToState { state: String },
}

/// A prop's statistics as an object (`PropDef::object`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ObjectStats {
    pub material: Material,
    pub size: ObjectSize,
    /// Resilient (a door, a chest) rather than fragile (a vase): more hit points.
    pub resilient: bool,
    /// Armor class (0 = the material's).
    #[serde(skip_serializing_if = "is_zero")]
    pub ac: i32,
    /// Hit points (0 = by size).
    #[serde(skip_serializing_if = "is_zero")]
    pub hp: i32,
    /// Damage below this does nothing (a damage threshold).
    #[serde(skip_serializing_if = "is_zero")]
    pub threshold: i32,
    /// A Strength (Athletics) check to force it open when locked (0 = it can't be forced).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub force_dc: u32,
    pub broken: Broken,
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl ObjectStats {
    /// Its armor class.
    pub fn armor_class(&self) -> i32 {
        if self.ac > 0 {
            self.ac
        } else {
            self.material.ac()
        }
    }

    /// Its hit points when whole.
    pub fn max_hp(&self) -> i32 {
        if self.hp > 0 {
            self.hp
        } else {
            self.size.hp(self.resilient)
        }
    }

    /// The damage a hit of `amount` (of `kind`) does to it: nothing of poison or psychic,
    /// nothing below its threshold.
    pub fn damage_taken(&self, amount: i32, kind: Option<DamageType>) -> i32 {
        if matches!(kind, Some(DamageType::Poison | DamageType::Psychic)) || amount < self.threshold {
            0
        } else {
            amount.max(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_by_the_srd() {
        assert_eq!(Material::ALL.map(Material::ac), [11, 13, 15, 17, 19, 21, 23]);
        assert_eq!(ObjectSize::ALL.map(|s| (s.hp(false), s.hp(true))), [(2, 5), (3, 10), (4, 18), (5, 27)]);
        let door = ObjectStats { resilient: true, threshold: 5, ..Default::default() };
        assert_eq!((door.armor_class(), door.max_hp()), (15, 18));
        assert_eq!(door.damage_taken(4, Some(DamageType::Slashing)), 0, "below the threshold");
        assert_eq!(door.damage_taken(7, Some(DamageType::Bludgeoning)), 7);
        assert_eq!(door.damage_taken(30, Some(DamageType::Poison)), 0);
        assert_eq!(door.damage_taken(30, Some(DamageType::Psychic)), 0);
        let iron = ObjectStats { material: Material::Iron, ac: 0, hp: 40, ..Default::default() };
        assert_eq!((iron.armor_class(), iron.max_hp()), (19, 40));
        // Files keep only what differs.
        let text = ron::to_string(&door).unwrap();
        assert!(!text.contains("ac:") && !text.contains("force_dc"), "{text}");
        assert_eq!(ron::from_str::<ObjectStats>(&text).unwrap(), door);
        assert_eq!(ron::from_str::<ObjectStats>("()").unwrap(), ObjectStats::default());
    }
}
