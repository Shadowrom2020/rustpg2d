//! SRD stat blocks: what a character is by the rules (abilities, armor class, hit points, speed,
//! senses, proficiencies, challenge). A character definition may have one
//! (`CharacterDef::stats`), a placed character its own (`EntityInstance::stats`). The SRD's NPC
//! stat blocks (commoner, guard, bandit, veteran…) are the templates: `npcs.ron`, written by the
//! editor's `srd_import` from the 5e-database and built into the engine.

use serde::{Deserialize, Serialize};

use super::Abilities;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatBlock {
    /// The template it came from (`guard`; "" = filled in by hand).
    pub template: String,
    /// The template's name as the SRD writes it ("Bandit Captain").
    pub name: String,
    pub abilities: Abilities,
    pub ac: i32,
    /// Its hit points (the average of `hit_dice`).
    pub hp: i32,
    /// "2d8+2".
    pub hit_dice: String,
    pub speed_ft: u32,
    pub darkvision_ft: u32,
    /// Skills and saving throws it is proficient in (`perception`, `save:dex`).
    pub proficiencies: Vec<String>,
    /// As the SRD writes them ("any one language (usually Common)").
    pub languages: String,
    /// Challenge rating: "0", "1/8", "2".
    pub cr: String,
    /// The experience it is worth.
    pub xp: u32,
}

impl Default for StatBlock {
    fn default() -> Self {
        StatBlock { template: String::new(), name: String::new(), abilities: Abilities::default(), ac: 10, hp: 4, hit_dice: "1d8".into(), speed_ft: 30, darkvision_ft: 0, proficiencies: Vec::new(), languages: String::new(), cr: "0".into(), xp: 10 }
    }
}

impl StatBlock {
    /// A field by name for scripts and status: str…cha, ac, hp, speed, darkvision, xp (`None` =
    /// no such field).
    pub fn field(&self, name: &str) -> Option<i64> {
        if let Some(a) = super::Ability::parse(name) {
            return Some(self.abilities.score(a) as i64);
        }
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "ac" | "armor_class" => self.ac as i64,
            "hp" | "hit_points" => self.hp as i64,
            "speed" => self.speed_ft as i64,
            "darkvision" => self.darkvision_ft as i64,
            "xp" => self.xp as i64,
            _ => return None,
        })
    }
}

const NPCS: &str = include_str!("npcs.ron");

/// The SRD's NPC stat blocks (the templates), by name order of the SRD.
pub fn npc_templates() -> &'static [StatBlock] {
    use std::sync::OnceLock;
    static T: OnceLock<Vec<StatBlock>> = OnceLock::new();
    T.get_or_init(|| ron::from_str(NPCS).unwrap_or_else(|e| {
        eprintln!("[rules] npcs.ron: {e}");
        Vec::new()
    }))
}

/// A template by id (`bandit-captain`).
pub fn npc_template(id: &str) -> Option<&'static StatBlock> {
    npc_templates().iter().find(|t| t.template == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_srd_npcs_are_built_in() {
        let t = npc_templates();
        assert_eq!(t.len(), 35, "{:?}", t.iter().map(|s| &s.template).collect::<Vec<_>>());
        let guard = npc_template("guard").unwrap();
        assert_eq!((guard.ac, guard.hp, guard.hit_dice.as_str(), guard.cr.as_str(), guard.xp), (16, 11, "2d8+2", "1/8", 25));
        assert!(guard.proficiencies.contains(&"perception".to_string()));
        assert_eq!(guard.field("ac"), Some(16));
        assert_eq!(guard.field("str"), Some(13));
        assert_eq!(guard.field("luck"), None);
        let commoner = npc_template("commoner").unwrap();
        assert_eq!((commoner.hp, commoner.xp), (4, 10));
        assert!(npc_template("werewolf-human").is_none(), "no lycanthropes");
        let s: StatBlock = ron::from_str("(template: \"x\")").unwrap();
        assert_eq!(s, StatBlock { template: "x".into(), ..Default::default() });
    }
}
