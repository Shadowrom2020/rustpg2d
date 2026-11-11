//! Races (`defs/races.ron`): humans, goblins, elves, dwarves… Each race has its figures (per
//! sex; the creator offers them with their colour features), stat changes and starting
//! skill levels for a player of that race, and a size (trolls are bigger: their bodies are drawn
//! larger and get a larger hitbox). The player picks a playable race in the creator
//! (`WorldState::player_race`); characters name theirs in `CharacterDef::race` (for conditions,
//! scripts and, later, factions). A project without races works as before.

use serde::{Deserialize, Serialize};

use crate::state::Sex;

/// A starting skill level of a race.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RaceSkill {
    pub skill: String,
    pub level: u32,
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

fn is_zero_i(v: &i32) -> bool {
    *v == 0
}

fn is_zero_f(v: &f32) -> bool {
    *v == 0.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RaceDef {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Offered in the character creator.
    #[serde(default)]
    pub playable: bool,
    /// Figure sheets of men / women of this race: the creator offers them (with their colour
    /// features), and a figure listed here gets the race's size.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies_male: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies_female: Vec<String>,
    /// A player of this race: extra max HP (negative: less), speed in percent, defense.
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub max_hp: i32,
    #[serde(default, skip_serializing_if = "is_zero_f")]
    pub speed: f32,
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub defense: i32,
    /// Skill levels a player of this race starts with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<RaceSkill>,
    /// The faction its characters (and a player of the race) belong to ("" = none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub faction: String,
    /// Size of its characters (1 = as drawn; trolls 1.5): sprite and hitbox.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub size: f32,
}

impl Default for RaceDef {
    fn default() -> Self {
        RaceDef {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            playable: false,
            bodies_male: Vec::new(),
            bodies_female: Vec::new(),
            max_hp: 0,
            speed: 0.0,
            defense: 0,
            skills: Vec::new(),
            faction: String::new(),
            size: 1.0,
        }
    }
}

impl RaceDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> RaceDef {
        RaceDef { id: id.to_string(), name: "New race".into(), playable: true, ..Default::default() }
    }

    /// The bodies of a character of `sex`.
    pub fn bodies(&self, sex: Sex) -> &[String] {
        if sex == Sex::Female {
            &self.bodies_female
        } else {
            &self.bodies_male
        }
    }

    /// Movement speed factor of a player of this race.
    pub fn speed_factor(&self) -> f32 {
        (1.0 + self.speed / 100.0).max(0.1)
    }

    /// The race's level of `skill` (0 = none).
    pub fn skill_level(&self, skill: &str) -> u32 {
        self.skills.iter().filter(|s| s.skill == skill).map(|s| s.level).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_race_needs_only_an_id_and_a_name() {
        let r: RaceDef = ron::from_str("(id: \"dwarf\", name: \"Dwarf\")").unwrap();
        assert_eq!(r, RaceDef { id: "dwarf".into(), name: "Dwarf".into(), ..Default::default() });
        assert_eq!(r.size, 1.0);
        assert_eq!(ron::to_string(&r).unwrap(), "(id:\"dwarf\",name:\"Dwarf\",playable:false)");
        let r: RaceDef = ron::from_str("(id: \"dwarf\", name: \"Dwarf\", speed: -10.0, skills: [(skill: \"mining\", level: 1)], size: 0.9)").unwrap();
        assert!((r.speed_factor() - 0.9).abs() < 1e-6);
        assert_eq!(r.skill_level("mining"), 1);
        assert_eq!(r.skill_level("cooking"), 0);
    }

    #[test]
    fn the_player_race_condition_compares_the_saved_race() {
        use crate::events::{Cond, Condition};
        let mut w = crate::state::WorldState::default();
        let goblin = Condition::new(Cond::PlayerRace { race: "goblin".into() });
        assert!(!goblin.eval(&w, &mut |_| false));
        w.player_race = "goblin".into();
        assert!(goblin.eval(&w, &mut |_| false));
        assert!(!Condition { negate: true, ..goblin }.eval(&w, &mut |_| false));
    }
}
