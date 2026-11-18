//! Magic: spell definitions (`defs/spells.ron`), learned spells and mana (in the save game) and
//! the built-in spell effects, optional per project (`ProjectConfig::skills.magic`); creators
//! extend them with the `Script` effect, which calls a Rhai function. (The skill tree that lived
//! here until 0.27 became the SRD's skills and proficiencies: `srd`, `srd_convert`.)

use serde::{Deserialize, Serialize};

use crate::defs::SpriteRef;

/// Project settings for magic (`project.ron` `skills`; its old `enabled` and `start_points` of
/// the skill tree are ignored).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillSettings {
    /// Magic: spells learned from items, cast with mana.
    pub magic: bool,
    /// Mana at level 1.
    pub max_mana: u32,
    /// Mana regained per second.
    pub mana_regen: f32,
}

impl Default for SkillSettings {
    fn default() -> Self {
        SkillSettings { magic: false, max_mana: 20, mana_regen: 0.5 }
    }
}

impl SkillSettings {
    pub fn magic_on(&self) -> bool {
        self.magic
    }
}

/// What a spell does when cast.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SpellEffect {
    /// Restores HP.
    Heal { amount: i32 },
    /// Shoots a projectile in the facing direction (like a ranged weapon without ammo).
    Bolt {
        damage: i32,
        projectile: SpriteRef,
        speed: f32,
        range: f32,
        /// What it does (force, fire…; `None` = untyped).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
    /// Damages everything within `radius` tiles around the player.
    Blast {
        damage: i32,
        radius: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
    /// A light around the player for `seconds` (radius in tiles).
    Light { radius: f32, seconds: f32 },
    /// Calls the Rhai function (which can use the whole script API: say, heal, give_item,
    /// damage_around, player_light, ...).
    Script { function: String },
}

impl SpellEffect {
    pub const LABELS: [(&'static str, &'static str); 5] = [
        ("Heal", "Restores HP"),
        ("Bolt", "Shoots a projectile in the facing direction"),
        ("Blast", "Damages everything around the player"),
        ("Light", "Lights the player's surroundings for a while"),
        ("Script", "Runs a script function"),
    ];
    pub fn index(&self) -> usize {
        match self {
            SpellEffect::Heal { .. } => 0,
            SpellEffect::Bolt { .. } => 1,
            SpellEffect::Blast { .. } => 2,
            SpellEffect::Light { .. } => 3,
            SpellEffect::Script { .. } => 4,
        }
    }
    pub fn template(i: usize) -> SpellEffect {
        match i {
            1 => SpellEffect::Bolt { damage: 2, projectile: SpriteRef::default(), speed: 300.0, range: 6.0, damage_type: Some(crate::srd::DamageType::Force) },
            2 => SpellEffect::Blast { damage: 3, radius: 2.0, damage_type: Some(crate::srd::DamageType::Force) },
            3 => SpellEffect::Light { radius: 5.0, seconds: 30.0 },
            4 => SpellEffect::Script { function: String::new() },
            _ => SpellEffect::Heal { amount: 4 },
        }
    }
    pub fn label(&self) -> &'static str {
        Self::LABELS[self.index()].0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpellDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: SpriteRef,
    /// Mana spent per cast.
    #[serde(default)]
    pub mana: u32,
    /// Seconds before it can be cast again.
    #[serde(default)]
    pub cooldown: f32,
    /// Needs proficiency in this skill (an SRD skill such as `arcana`, or a tool) to be learned
    /// and cast ("" = no requirement).
    #[serde(default)]
    pub skill: String,
    /// Unused since the SRD rules (the skill tree's level); kept so old files keep their value.
    #[serde(default)]
    pub level: u32,
    pub effect: SpellEffect,
}

impl SpellDef {
    pub fn new(id: &str, name: &str, description: &str, icon: SpriteRef, mana: u32, cooldown: f32, effect: SpellEffect) -> Self {
        SpellDef { id: id.into(), name: name.into(), description: description.into(), icon, mana, cooldown, skill: String::new(), level: 0, effect }
    }
    pub fn requires(mut self, skill: &str) -> Self {
        self.skill = skill.into();
        self
    }
}

/// Quick bar entries that cast a spell: `"spell:<id>"`.
pub const SPELL_PREFIX: &str = "spell:";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_templates_round_trip() {
        for i in 0..SpellEffect::LABELS.len() {
            assert_eq!(SpellEffect::template(i).index(), i);
        }
    }

    #[test]
    fn old_skill_settings_still_load() {
        let s: SkillSettings = ron::from_str("(enabled: true, magic: true, start_points: 3, max_mana: 30)").unwrap();
        assert!(s.magic_on());
        assert_eq!(s.max_mana, 30);
    }
}
