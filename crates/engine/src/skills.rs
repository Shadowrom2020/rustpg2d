//! Skills and magic: definitions (`defs/skills.ron`, `defs/spells.ron`), the player's skill
//! levels, skill points, learned spells and mana (in the save game), and the built-in skill and
//! spell effects. Both systems are optional per project (`ProjectConfig::skills`); creators
//! extend them with the `Script` effects, which call Rhai functions.

use serde::{Deserialize, Serialize};

use crate::defs::SpriteRef;

/// Project settings for skills and magic (`project.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillSettings {
    /// Skills: levels bought with skill points (K opens the screen).
    pub enabled: bool,
    /// Magic: spells learned from items, cast with mana (needs `enabled`).
    pub magic: bool,
    /// Skill points a new game starts with.
    pub start_points: u32,
    /// Mana without any skill bonus.
    pub max_mana: u32,
    /// Mana regained per second.
    pub mana_regen: f32,
}

impl Default for SkillSettings {
    fn default() -> Self {
        SkillSettings { enabled: false, magic: false, start_points: 0, max_mana: 20, mana_regen: 0.5 }
    }
}

impl SkillSettings {
    pub fn magic_on(&self) -> bool {
        self.enabled && self.magic
    }
}

/// What a skill does per level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SkillEffect {
    /// Only for scripts and conditions.
    None,
    /// Melee attack damage (weapons and bare hands) +percent per level.
    MeleeDamage { percent: f32 },
    /// Ranged weapon damage (bows, crossbows) +percent per level.
    RangedDamage { percent: f32 },
    /// Spell damage (bolts, blasts) +percent per level.
    SpellDamage { percent: f32 },
    /// Defense +points per level.
    Defense { points: i32 },
    /// Movement speed +percent per level.
    Speed { percent: f32 },
    /// Opens locks up to a difficulty equal to the level (see `LockSpec::pick_difficulty`).
    Lockpick,
    /// Merchants: buying cheaper and selling better by percent per level.
    Haggle { percent: f32 },
    /// Maximum mana +points per level.
    Mana { points: u32 },
    /// Mana regained per second +amount per level.
    ManaRegen { per_second: f32 },
    /// Calls the Rhai function `function(level)` whenever the level changes (and once when a
    /// game starts or loads), so a script can apply its own effect.
    Script { function: String },
    /// Gathering with a tool kind ("pickaxe"; "" = any): percent fewer hits per level.
    Gather { tool: String, percent: f32 },
    /// Food (items marked food) heals percent more per level.
    FoodBonus { percent: f32 },
}

impl SkillEffect {
    pub const LABELS: [(&'static str, &'static str); 13] = [
        ("None", "No built-in effect (for scripts and conditions)"),
        ("Melee damage", "Melee weapons and bare hands: damage + % per level"),
        ("Ranged damage", "Bows and crossbows: damage + % per level"),
        ("Spell damage", "Bolt and blast spells: damage + % per level"),
        ("Defense", "Defense + points per level"),
        ("Speed", "Movement speed + % per level"),
        ("Lockpick", "Picks locks up to a difficulty of the skill level"),
        ("Haggle", "Buys cheaper and sells better by % per level"),
        ("Mana", "Maximum mana + points per level"),
        ("Mana regen", "Mana regained per second + per level"),
        ("Script", "Calls a script function with the level"),
        ("Gathering", "Gathering with a tool (mining with a pickaxe, ...): % fewer hits per level"),
        ("Food", "Food (items marked as food) heals % more per level"),
    ];
    pub fn index(&self) -> usize {
        match self {
            SkillEffect::None => 0,
            SkillEffect::MeleeDamage { .. } => 1,
            SkillEffect::RangedDamage { .. } => 2,
            SkillEffect::SpellDamage { .. } => 3,
            SkillEffect::Defense { .. } => 4,
            SkillEffect::Speed { .. } => 5,
            SkillEffect::Lockpick => 6,
            SkillEffect::Haggle { .. } => 7,
            SkillEffect::Mana { .. } => 8,
            SkillEffect::ManaRegen { .. } => 9,
            SkillEffect::Script { .. } => 10,
            SkillEffect::Gather { .. } => 11,
            SkillEffect::FoodBonus { .. } => 12,
        }
    }
    pub fn template(i: usize) -> SkillEffect {
        match i {
            1 => SkillEffect::MeleeDamage { percent: 10.0 },
            2 => SkillEffect::RangedDamage { percent: 10.0 },
            3 => SkillEffect::SpellDamage { percent: 10.0 },
            4 => SkillEffect::Defense { points: 1 },
            5 => SkillEffect::Speed { percent: 5.0 },
            6 => SkillEffect::Lockpick,
            7 => SkillEffect::Haggle { percent: 5.0 },
            8 => SkillEffect::Mana { points: 10 },
            9 => SkillEffect::ManaRegen { per_second: 0.25 },
            10 => SkillEffect::Script { function: String::new() },
            11 => SkillEffect::Gather { tool: String::new(), percent: 15.0 },
            12 => SkillEffect::FoodBonus { percent: 25.0 },
            _ => SkillEffect::None,
        }
    }
    pub fn label(&self) -> &'static str {
        Self::LABELS[self.index()].0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: SpriteRef,
    #[serde(default = "default_max_level")]
    pub max_level: u32,
    #[serde(default = "default_effect")]
    pub effect: SkillEffect,
}

fn default_max_level() -> u32 {
    5
}
fn default_effect() -> SkillEffect {
    SkillEffect::None
}

impl SkillDef {
    pub fn new(id: &str, name: &str, description: &str, icon: SpriteRef, max_level: u32, effect: SkillEffect) -> Self {
        SkillDef { id: id.into(), name: name.into(), description: description.into(), icon, max_level, effect }
    }
    /// A short description of the effect at `level` (skill screen).
    pub fn effect_text(&self, level: u32) -> String {
        let l = level as f32;
        match &self.effect {
            SkillEffect::None => String::new(),
            SkillEffect::MeleeDamage { percent } => format!("+{:.0}% melee damage", percent * l),
            SkillEffect::RangedDamage { percent } => format!("+{:.0}% ranged damage", percent * l),
            SkillEffect::SpellDamage { percent } => format!("+{:.0}% spell damage", percent * l),
            SkillEffect::Defense { points } => format!("+{} defense", points * level as i32),
            SkillEffect::Speed { percent } => format!("+{:.0}% speed", percent * l),
            SkillEffect::Lockpick => format!("picks locks up to difficulty {level}"),
            SkillEffect::Haggle { percent } => format!("{:.0}% better prices", percent * l),
            SkillEffect::Mana { points } => format!("+{} max mana", points * level),
            SkillEffect::ManaRegen { per_second } => format!("+{:.2} mana/s", per_second * l),
            SkillEffect::Script { function } => format!("{function}({level})"),
            SkillEffect::FoodBonus { percent } => format!("food heals {:.0}% more", percent * l),
            SkillEffect::Gather { tool, percent } => format!("{:.0}% fewer hits{}", (percent * l).min(80.0), if tool.is_empty() { String::new() } else { format!(" with a {tool}") }),
        }
    }
}

/// What a spell does when cast.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SpellEffect {
    /// Restores HP.
    Heal { amount: i32 },
    /// Shoots a projectile in the facing direction (like a ranged weapon without ammo).
    Bolt { damage: i32, projectile: SpriteRef, speed: f32, range: f32 },
    /// Damages everything within `radius` tiles around the player.
    Blast { damage: i32, radius: f32 },
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
            1 => SpellEffect::Bolt { damage: 2, projectile: SpriteRef::default(), speed: 300.0, range: 6.0 },
            2 => SpellEffect::Blast { damage: 3, radius: 2.0 },
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
    /// Needs this skill at `level` or more to be learned and cast ("" = no requirement).
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub level: u32,
    pub effect: SpellEffect,
}

impl SpellDef {
    pub fn new(id: &str, name: &str, description: &str, icon: SpriteRef, mana: u32, cooldown: f32, effect: SpellEffect) -> Self {
        SpellDef { id: id.into(), name: name.into(), description: description.into(), icon, mana, cooldown, skill: String::new(), level: 0, effect }
    }
    pub fn requires(mut self, skill: &str, level: u32) -> Self {
        self.skill = skill.into();
        self.level = level;
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
        for i in 0..SkillEffect::LABELS.len() {
            assert_eq!(SkillEffect::template(i).index(), i);
        }
        for i in 0..SpellEffect::LABELS.len() {
            assert_eq!(SpellEffect::template(i).index(), i);
        }
        let d = SkillDef::new("x", "X", "", SpriteRef::default(), 3, SkillEffect::MeleeDamage { percent: 10.0 });
        assert_eq!(d.effect_text(2), "+20% melee damage");
    }
}
