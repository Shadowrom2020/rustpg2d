//! Races (`defs/races.ron`): humans, goblins, elves, dwarves… Each race has its figures (per
//! sex; the creator offers them with their colour features), stat changes and proficiencies for a
//! player of that race, and a size (trolls are bigger: their bodies are drawn larger and get a
//! larger hitbox). Since 0.28 a race carries its SRD 5.1 data (imported from the 5e-database by
//! the editor's `srd_import` example): ability bonuses, speed, size category, darkvision,
//! languages, resistances, traits and the choices it offers; a subrace names its `parent` and
//! [`RaceDef::merged`] combines the two. The player picks a playable race in the creator
//! (`WorldState::player_race`); characters name theirs in `CharacterDef::race` (for conditions,
//! scripts and, later, factions). A project without races works as before.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::state::Sex;

/// A proficiency of a race (`skill`: a proficiency id; `level` from the skill tree of 0.27, 1 now).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RaceSkill {
    pub skill: String,
    pub level: u32,
}

/// A trait of a race (Darkvision, Dwarven Resilience…): shown in the creator and on the sheet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RaceTrait {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// What a racial choice picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChoiceKind {
    /// +1 to an ability (each pick a different one).
    #[default]
    Ability,
    Skill,
    Tool,
    Language,
    /// A draconic ancestry: `grants` names the damage type each option resists.
    Ancestry,
    /// A cantrip (kept as a choice until the SRD's spells arrive).
    Cantrip,
}

impl ChoiceKind {
    pub const ALL: [ChoiceKind; 6] = [ChoiceKind::Ability, ChoiceKind::Skill, ChoiceKind::Tool, ChoiceKind::Language, ChoiceKind::Ancestry, ChoiceKind::Cantrip];
    pub fn label(self) -> &'static str {
        match self {
            ChoiceKind::Ability => "+1",
            ChoiceKind::Skill => "Skill",
            ChoiceKind::Tool => "Tool",
            ChoiceKind::Language => "Language",
            ChoiceKind::Ancestry => "Ancestry",
            ChoiceKind::Cantrip => "Cantrip",
        }
    }
}

/// A choice a hero of the race makes in the creator (the half-elf's two +1s, the dragonborn's
/// ancestry, the high elf's cantrip…).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RaceChoice {
    /// Where the pick is kept (`WorldState::choices`), e.g. `draconic-ancestry`.
    pub id: String,
    /// What the creator calls it: "Draconic Ancestry".
    pub label: String,
    #[serde(default)]
    pub kind: ChoiceKind,
    /// How many to pick.
    #[serde(default = "one_u")]
    pub choose: u32,
    /// The ids to pick from: abilities (`str`), proficiencies, languages, trait ids, spell ids.
    #[serde(default)]
    pub options: Vec<String>,
    /// Option -> the damage type it gives resistance to (an ancestry).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub grants: std::collections::BTreeMap<String, String>,
    /// Option -> its display name ("Draconic Ancestry (Red)", "Fire Bolt"); else the id.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub names: std::collections::BTreeMap<String, String>,
}

impl RaceChoice {
    /// An option's display name.
    pub fn option_name(&self, id: &str) -> String {
        if let Some(n) = self.names.get(id) {
            return n.clone();
        }
        match self.kind {
            ChoiceKind::Ability => crate::srd::Ability::parse(id).map_or(id.to_string(), |a| a.name().to_string()),
            ChoiceKind::Skill | ChoiceKind::Tool => crate::srd::abilities::proficiency_name(id),
            _ => id.to_string(),
        }
    }
}

fn one_u() -> u32 {
    1
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
    /// A subrace's race (`hill-dwarf` -> `dwarf`; "" = none): it adds to that race ([`Self::merged`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parent: String,
    /// SRD ability score increases: `{"con": 2}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ability_bonuses: BTreeMap<String, i32>,
    /// Walking speed in feet (0 = 30, the SRD's usual): the player's speed is `walk_ft / 30`.
    #[serde(default, skip_serializing_if = "is_zero_u")]
    pub walk_ft: u32,
    /// The SRD size category: "Medium", "Small" ("" = Medium).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub size_category: String,
    /// Darkvision in feet (0 = none): the hero sees dim and dark places brighter.
    #[serde(default, skip_serializing_if = "is_zero_u")]
    pub darkvision_ft: u32,
    /// Extra hit points per level (a hill dwarf's Dwarven Toughness: 1).
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub hp_per_level: i32,
    /// Languages a hero of the race speaks (`common`, `dwarvish`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// Damage types the race resists (`poison`, `fire`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resistances: Vec<String>,
    /// Its traits (names and rules text).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<RaceTrait>,
    /// Choices a hero of the race makes in the creator.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<RaceChoice>,
    /// The race's default colours in the creator: colour feature -> RGB (a tiefling's red skin).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, [u8; 3]>,
}

fn is_zero_u(v: &u32) -> bool {
    *v == 0
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
            parent: String::new(),
            ability_bonuses: BTreeMap::new(),
            walk_ft: 0,
            size_category: String::new(),
            darkvision_ft: 0,
            hp_per_level: 0,
            languages: Vec::new(),
            resistances: Vec::new(),
            traits: Vec::new(),
            choices: Vec::new(),
            colors: BTreeMap::new(),
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

    /// Movement speed factor of a player of this race: its walking speed (30 ft = 1) and the
    /// speed percent of 0.27 and older.
    pub fn speed_factor(&self) -> f32 {
        let feet = if self.walk_ft == 0 { 1.0 } else { self.walk_ft as f32 / 30.0 };
        (feet * (1.0 + self.speed / 100.0)).max(0.1)
    }

    /// Walking speed in feet (30 when not set).
    pub fn walk_feet(&self) -> u32 {
        if self.walk_ft == 0 {
            30
        } else {
            self.walk_ft
        }
    }

    /// A subrace with its race (`parent`) folded in: bonuses add up, lists join (the subrace's
    /// entries after the race's, without repeats), looks, size, faction, speed, darkvision and
    /// size category come from the race where the subrace has none. A race without a parent (or
    /// a parent that doesn't exist) is returned as it is.
    pub fn merged(&self, parent: Option<&RaceDef>) -> RaceDef {
        let Some(p) = parent.filter(|p| !self.parent.is_empty() && p.id == self.parent) else { return self.clone() };
        let mut r = self.clone();
        for (a, b) in &p.ability_bonuses {
            *r.ability_bonuses.entry(a.clone()).or_insert(0) += b;
        }
        let join = |base: &[String], own: &[String]| -> Vec<String> {
            let mut v = base.to_vec();
            for x in own {
                if !v.contains(x) {
                    v.push(x.clone());
                }
            }
            v
        };
        r.languages = join(&p.languages, &self.languages);
        r.resistances = join(&p.resistances, &self.resistances);
        r.skills = p.skills.iter().chain(&self.skills).fold(Vec::new(), |mut v: Vec<RaceSkill>, s| {
            if !v.iter().any(|x| x.skill == s.skill) {
                v.push(s.clone());
            }
            v
        });
        r.traits = p.traits.iter().chain(&self.traits).cloned().collect();
        r.choices = p.choices.iter().chain(&self.choices).cloned().collect();
        if r.bodies_male.is_empty() && r.bodies_female.is_empty() {
            r.bodies_male = p.bodies_male.clone();
            r.bodies_female = p.bodies_female.clone();
        }
        if r.size == 1.0 {
            r.size = p.size;
        }
        if r.faction.is_empty() {
            r.faction = p.faction.clone();
        }
        if r.walk_ft == 0 {
            r.walk_ft = p.walk_ft;
        }
        if r.darkvision_ft == 0 {
            r.darkvision_ft = p.darkvision_ft;
        }
        if r.size_category.is_empty() {
            r.size_category = p.size_category.clone();
        }
        if r.description.is_empty() {
            r.description = p.description.clone();
        }
        for (k, v) in &p.colors {
            r.colors.entry(k.clone()).or_insert(*v);
        }
        r.max_hp += p.max_hp;
        r.hp_per_level += p.hp_per_level;
        r.defense += p.defense;
        r.speed += p.speed;
        r
    }

    /// "+2 CON, +1 WIS" (empty without bonuses).
    pub fn bonus_text(&self) -> String {
        crate::srd::Ability::ALL
            .iter()
            .filter_map(|a| self.ability_bonuses.get(a.id()).filter(|b| **b != 0).map(|b| format!("{} {}", crate::srd::abilities::signed(*b as i64), a.short())))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The race's level of `skill` (0 = none).
    pub fn skill_level(&self, skill: &str) -> u32 {
        self.skills.iter().filter(|s| s.skill == skill).map(|s| s.level).max().unwrap_or(0)
    }
}

/// What the creator (or a new game without it) makes of a race, base scores, choices and a
/// background.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hero {
    pub abilities: crate::srd::Abilities,
    pub proficiencies: std::collections::BTreeSet<String>,
    pub languages: std::collections::BTreeSet<String>,
    pub resistances: std::collections::BTreeSet<String>,
}

/// The key of the background's language picks in the choices (`WorldState::choices`).
pub const BACKGROUND_LANGUAGES: &str = "background-languages";

/// Builds the hero: `base` scores (else the project's) plus the race's bonuses and the chosen
/// +1s (racial increases stop at 20), the project's proficiencies with the race's, the chosen
/// skills and tools and the background's, the race's languages with the chosen ones, the race's
/// resistances with those of a chosen ancestry. `race` is the merged race ([`RaceDef::merged`]);
/// `choices` = choice id -> the picked options.
pub fn build_hero(race: Option<&RaceDef>, rules: &crate::srd::RulesSettings, base: Option<crate::srd::Abilities>, choices: &BTreeMap<String, Vec<String>>, background: Option<&crate::backgrounds::BackgroundDef>) -> Hero {
    use crate::srd::abilities::proficiency_id;
    let mut h = Hero { abilities: base.unwrap_or(rules.abilities), ..Default::default() };
    let raise = |a: &str, n: i32, abilities: &mut crate::srd::Abilities| {
        if let Some(a) = crate::srd::Ability::parse(a) {
            let s = abilities.score(a);
            abilities.set(a, (s + n).min(20.max(s)));
        }
    };
    h.proficiencies.extend(rules.proficiencies.iter().map(|p| proficiency_id(p)).filter(|p| !p.is_empty()));
    if let Some(r) = race {
        for (a, n) in &r.ability_bonuses {
            raise(a, *n, &mut h.abilities);
        }
        h.proficiencies.extend(r.skills.iter().filter(|s| s.level > 0).map(|s| proficiency_id(&s.skill)).filter(|p| !p.is_empty()));
        h.languages.extend(r.languages.iter().cloned());
        h.resistances.extend(r.resistances.iter().cloned());
        for c in &r.choices {
            let picked = choices.get(&c.id).map(|v| &v[..]).unwrap_or(&[]);
            for o in picked.iter().filter(|o| c.options.contains(o)).take(c.choose as usize) {
                match c.kind {
                    ChoiceKind::Ability => raise(o, 1, &mut h.abilities),
                    ChoiceKind::Skill | ChoiceKind::Tool => {
                        h.proficiencies.insert(proficiency_id(o));
                    }
                    ChoiceKind::Language => {
                        h.languages.insert(o.clone());
                    }
                    ChoiceKind::Ancestry => {
                        if let Some(t) = c.grants.get(o) {
                            h.resistances.insert(t.clone());
                        }
                    }
                    ChoiceKind::Cantrip => {}
                }
            }
        }
    }
    if let Some(b) = background {
        h.proficiencies.extend(b.proficiencies.iter().map(|p| proficiency_id(p)).filter(|p| !p.is_empty()));
        if let Some(langs) = choices.get(BACKGROUND_LANGUAGES) {
            h.languages.extend(langs.iter().take(b.language_choices as usize).cloned());
        }
    }
    h
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
    fn a_subrace_merges_with_its_race() {
        let dwarf: RaceDef = ron::from_str(r#"(id: "dwarf", name: "Dwarf", bodies_male: ["dwarf"], faction: "dwarves", ability_bonuses: {"con": 2}, walk_ft: 25, darkvision_ft: 60, languages: ["common", "dwarvish"], resistances: ["poison"], skills: [(skill: "battleaxes", level: 1)], traits: [(id: "darkvision", name: "Darkvision")])"#).unwrap();
        let hill: RaceDef = ron::from_str(r#"(id: "hill-dwarf", name: "Hill Dwarf", parent: "dwarf", playable: true, ability_bonuses: {"wis": 1}, traits: [(id: "dwarven-toughness", name: "Dwarven Toughness")])"#).unwrap();
        let m = hill.merged(Some(&dwarf));
        assert_eq!(m.ability_bonuses.get("con"), Some(&2));
        assert_eq!(m.ability_bonuses.get("wis"), Some(&1));
        assert_eq!((m.walk_ft, m.darkvision_ft, m.faction.as_str()), (25, 60, "dwarves"));
        assert_eq!(m.languages, vec!["common", "dwarvish"]);
        assert_eq!(m.resistances, vec!["poison"]);
        assert_eq!(m.bodies_male, vec!["dwarf"]);
        assert_eq!(m.traits.len(), 2);
        assert_eq!(m.bonus_text(), "+2 CON, +1 WIS");
        assert!((m.speed_factor() - 25.0 / 30.0).abs() < 1e-6);
        assert_eq!(dwarf.merged(None), dwarf, "a race without a parent stays as it is");
        assert_eq!(hill.merged(Some(&hill)).ability_bonuses.len(), 1, "only its own parent merges");
    }

    #[test]
    fn a_hero_gets_bonuses_choices_and_background() {
        let half_elf: RaceDef = ron::from_str(r#"(id: "half-elf", name: "Half-Elf", ability_bonuses: {"cha": 2}, languages: ["common", "elvish"], choices: [
            (id: "half-elf-abilities", label: "Ability Score Increase", kind: Ability, choose: 2, options: ["str", "dex", "con", "int", "wis"]),
            (id: "skill-versatility", label: "Skill Versatility", kind: Skill, choose: 2, options: ["stealth", "insight", "perception"]),
            (id: "half-elf-language", label: "Language", kind: Language, choose: 1, options: ["dwarvish", "orc"]),
        ])"#).unwrap();
        let rules = crate::srd::RulesSettings { proficiencies: vec!["athletics".into()], ..Default::default() };
        let mut choices = BTreeMap::new();
        choices.insert("half-elf-abilities".to_string(), vec!["dex".to_string(), "con".to_string(), "int".to_string()]);
        choices.insert("skill-versatility".to_string(), vec!["stealth".to_string(), "religion".to_string()]);
        choices.insert("half-elf-language".to_string(), vec!["orc".to_string()]);
        choices.insert(BACKGROUND_LANGUAGES.to_string(), vec!["giant".to_string(), "sylvan".to_string(), "abyssal".to_string()]);
        let acolyte: crate::backgrounds::BackgroundDef = ron::from_str(r#"(id: "acolyte", name: "Acolyte", proficiencies: ["insight", "religion"], language_choices: 2)"#).unwrap();
        let base = crate::srd::Abilities([8, 14, 13, 12, 10, 19]);
        let h = build_hero(Some(&half_elf), &rules, Some(base), &choices, Some(&acolyte));
        // +2 CHA stops at 20; +1 DEX and +1 CON, the third pick is one too many.
        assert_eq!(h.abilities, crate::srd::Abilities([8, 15, 14, 12, 10, 20]));
        let profs: Vec<&str> = h.proficiencies.iter().map(String::as_str).collect();
        assert_eq!(profs, ["athletics", "insight", "religion", "stealth"], "religion isn't one of the race's options, the background gives it");
        let langs: Vec<&str> = h.languages.iter().map(String::as_str).collect();
        assert_eq!(langs, ["common", "elvish", "giant", "orc", "sylvan"]);
        // A dragonborn's ancestry gives its resistance.
        let dragonborn: RaceDef = ron::from_str(r#"(id: "dragonborn", name: "Dragonborn", choices: [(id: "draconic-ancestry", label: "Draconic Ancestry", kind: Ancestry, options: ["red", "white"], grants: {"red": "fire", "white": "cold"})])"#).unwrap();
        let mut c = BTreeMap::new();
        c.insert("draconic-ancestry".to_string(), vec!["red".to_string()]);
        let h = build_hero(Some(&dragonborn), &rules, None, &c, None);
        assert_eq!(h.resistances.iter().collect::<Vec<_>>(), [&"fire".to_string()]);
        assert_eq!(h.abilities, rules.abilities, "the project's scores without a base");
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
        // A hill dwarf is a dwarf too.
        w.player_race = "hill-dwarf".into();
        w.player_race_parent = "dwarf".into();
        assert!(Condition::new(Cond::PlayerRace { race: "dwarf".into() }).eval(&w, &mut |_| false));
        assert!(Condition::new(Cond::PlayerRace { race: "hill-dwarf".into() }).eval(&w, &mut |_| false));
    }
}
