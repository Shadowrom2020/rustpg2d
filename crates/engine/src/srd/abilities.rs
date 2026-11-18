//! The six ability scores, their modifiers, the proficiency bonus, the 18 SRD skills and the
//! SRD's tools (the ids are the SRD data's index names: `sleight-of-hand`, `thieves-tools`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ability {
    Str,
    Dex,
    Con,
    Int,
    Wis,
    Cha,
}

impl Ability {
    pub const ALL: [Ability; 6] = [Ability::Str, Ability::Dex, Ability::Con, Ability::Int, Ability::Wis, Ability::Cha];

    /// "str" … "cha" (also the full names, any case).
    pub fn parse(s: &str) -> Option<Ability> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "str" | "strength" => Ability::Str,
            "dex" | "dexterity" => Ability::Dex,
            "con" | "constitution" => Ability::Con,
            "int" | "intelligence" => Ability::Int,
            "wis" | "wisdom" => Ability::Wis,
            "cha" | "charisma" => Ability::Cha,
            _ => return None,
        })
    }
    pub fn id(self) -> &'static str {
        ["str", "dex", "con", "int", "wis", "cha"][self as usize]
    }
    /// "STR"
    pub fn short(self) -> &'static str {
        ["STR", "DEX", "CON", "INT", "WIS", "CHA"][self as usize]
    }
    pub fn name(self) -> &'static str {
        ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"][self as usize]
    }
}

/// The modifier of an ability score: (score - 10) / 2, rounded down.
pub fn modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// The proficiency bonus at a character level: +2 at 1-4, +3 at 5-8 … +6 at 17-20.
pub fn proficiency_bonus(level: u32) -> i32 {
    2 + (level.clamp(1, 20) as i32 - 1) / 4
}

/// "+2" / "-1" / "+0"
pub fn signed(n: i64) -> String {
    if n < 0 {
        n.to_string()
    } else {
        format!("+{n}")
    }
}

/// The six scores, STR to CHA (10 each by default).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Abilities(pub [i32; 6]);

impl Default for Abilities {
    fn default() -> Self {
        Abilities([10; 6])
    }
}

impl Abilities {
    /// The SRD's standard array in ability order: 15 14 13 12 10 8.
    pub const STANDARD: Abilities = Abilities([15, 14, 13, 12, 10, 8]);
    pub fn score(&self, a: Ability) -> i32 {
        self.0[a as usize]
    }
    pub fn set(&mut self, a: Ability, score: i32) {
        self.0[a as usize] = score.clamp(1, 30);
    }
    pub fn modifier(&self, a: Ability) -> i32 {
        modifier(self.score(a))
    }
    /// `STR15,DEX14,…` (status lines).
    pub fn text(&self) -> String {
        Ability::ALL.iter().map(|a| format!("{}{}", a.short(), self.score(*a))).collect::<Vec<_>>().join(",")
    }
}

/// The 18 SRD skills: id, name, ability.
pub const SKILLS: [(&str, &str, Ability); 18] = [
    ("acrobatics", "Acrobatics", Ability::Dex),
    ("animal-handling", "Animal Handling", Ability::Wis),
    ("arcana", "Arcana", Ability::Int),
    ("athletics", "Athletics", Ability::Str),
    ("deception", "Deception", Ability::Cha),
    ("history", "History", Ability::Int),
    ("insight", "Insight", Ability::Wis),
    ("intimidation", "Intimidation", Ability::Cha),
    ("investigation", "Investigation", Ability::Int),
    ("medicine", "Medicine", Ability::Wis),
    ("nature", "Nature", Ability::Int),
    ("perception", "Perception", Ability::Wis),
    ("performance", "Performance", Ability::Cha),
    ("persuasion", "Persuasion", Ability::Cha),
    ("religion", "Religion", Ability::Int),
    ("sleight-of-hand", "Sleight of Hand", Ability::Dex),
    ("stealth", "Stealth", Ability::Dex),
    ("survival", "Survival", Ability::Wis),
];

/// A skill by id (also its name, any case, and with spaces / underscores for the dashes).
pub fn skill(id: &str) -> Option<(&'static str, &'static str, Ability)> {
    let k = norm(id);
    SKILLS.iter().copied().find(|(i, n, _)| *i == k || norm(n) == k)
}

/// The SRD's tools: id, name, the ability a check with them usually uses.
pub const TOOLS: [(&str, &str, Ability); 36] = [
    ("alchemists-supplies", "Alchemist's Supplies", Ability::Int),
    ("brewers-supplies", "Brewer's Supplies", Ability::Int),
    ("calligraphers-supplies", "Calligrapher's Supplies", Ability::Dex),
    ("carpenters-tools", "Carpenter's Tools", Ability::Str),
    ("cartographers-tools", "Cartographer's Tools", Ability::Int),
    ("cobblers-tools", "Cobbler's Tools", Ability::Dex),
    ("cooks-utensils", "Cook's Utensils", Ability::Wis),
    ("glassblowers-tools", "Glassblower's Tools", Ability::Dex),
    ("jewelers-tools", "Jeweler's Tools", Ability::Dex),
    ("leatherworkers-tools", "Leatherworker's Tools", Ability::Dex),
    ("masons-tools", "Mason's Tools", Ability::Str),
    ("painters-supplies", "Painter's Supplies", Ability::Dex),
    ("potters-tools", "Potter's Tools", Ability::Dex),
    ("smiths-tools", "Smith's Tools", Ability::Str),
    ("tinkers-tools", "Tinker's Tools", Ability::Dex),
    ("weavers-tools", "Weaver's Tools", Ability::Dex),
    ("woodcarvers-tools", "Woodcarver's Tools", Ability::Dex),
    ("disguise-kit", "Disguise Kit", Ability::Cha),
    ("forgery-kit", "Forgery Kit", Ability::Dex),
    ("herbalism-kit", "Herbalism Kit", Ability::Wis),
    ("navigators-tools", "Navigator's Tools", Ability::Wis),
    ("poisoners-kit", "Poisoner's Kit", Ability::Int),
    ("thieves-tools", "Thieves' Tools", Ability::Dex),
    ("dice-set", "Dice Set", Ability::Wis),
    ("playing-card-set", "Playing Card Set", Ability::Wis),
    ("bagpipes", "Bagpipes", Ability::Cha),
    ("drum", "Drum", Ability::Cha),
    ("dulcimer", "Dulcimer", Ability::Cha),
    ("flute", "Flute", Ability::Cha),
    ("lute", "Lute", Ability::Cha),
    ("lyre", "Lyre", Ability::Cha),
    ("horn", "Horn", Ability::Cha),
    ("pan-flute", "Pan Flute", Ability::Cha),
    ("shawm", "Shawm", Ability::Cha),
    ("viol", "Viol", Ability::Cha),
    ("land-vehicles", "Land Vehicles", Ability::Dex),
];

/// A tool by id or name (any case; "Thieves' Tools" = "thieves-tools").
pub fn tool(id: &str) -> Option<(&'static str, &'static str, Ability)> {
    let k = norm(id);
    TOOLS.iter().copied().find(|(i, n, _)| *i == k || norm(n) == k)
}

/// Lowercase, spaces and underscores as dashes, apostrophes dropped.
fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace(['\'', '’'], "").replace([' ', '_'], "-")
}

/// The id a proficiency is stored under: an SRD skill's or tool's id, `save:dex` for a saving
/// throw; anything else (a project's own tool) lowercased as it is.
pub fn proficiency_id(s: &str) -> String {
    let t = s.trim();
    if let Some(a) = t.strip_prefix("save:").and_then(Ability::parse) {
        return format!("save:{}", a.id());
    }
    if let Some((id, _, _)) = skill(t).or_else(|| tool(t)) {
        return id.to_string();
    }
    t.to_ascii_lowercase()
}

/// A proficiency's display name: "Stealth", "Thieves' Tools", "DEX saves", else the id.
pub fn proficiency_name(id: &str) -> String {
    if let Some(a) = id.strip_prefix("save:").and_then(Ability::parse) {
        return format!("{} saves", a.short());
    }
    skill(id).or_else(|| tool(id)).map_or_else(|| id.to_string(), |(_, n, _)| n.to_string())
}

/// Every proficiency id: the 18 skills, the tools, the six saving throws (editor suggestions).
pub fn proficiency_choices() -> Vec<String> {
    SKILLS.iter().map(|s| s.0.to_string()).chain(TOOLS.iter().map(|t| t.0.to_string())).chain(Ability::ALL.iter().map(|a| format!("save:{}", a.id()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_follow_the_table() {
        let want = [(1, -5), (2, -4), (3, -4), (8, -1), (9, -1), (10, 0), (11, 0), (12, 1), (15, 2), (18, 4), (20, 5), (29, 9), (30, 10)];
        for (score, m) in want {
            assert_eq!(modifier(score), m, "score {score}");
        }
        assert_eq!(Abilities::STANDARD.modifier(Ability::Str), 2);
        assert_eq!(Abilities::STANDARD.modifier(Ability::Cha), -1);
        assert_eq!(Abilities::default().text(), "STR10,DEX10,CON10,INT10,WIS10,CHA10");
    }

    #[test]
    fn proficiency_by_level() {
        let want = [2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6];
        for (i, b) in want.iter().enumerate() {
            assert_eq!(proficiency_bonus(i as u32 + 1), *b, "level {}", i + 1);
        }
        assert_eq!(proficiency_bonus(0), 2);
    }

    #[test]
    fn skills_and_tools_are_found_by_id_and_name() {
        assert_eq!(skill("Sleight of Hand").unwrap().0, "sleight-of-hand");
        assert_eq!(skill("animal_handling").unwrap().2, Ability::Wis);
        assert_eq!(tool("Thieves' Tools").unwrap().0, "thieves-tools");
        assert_eq!(proficiency_id("Smith's Tools"), "smiths-tools");
        assert_eq!(proficiency_id("save:Dexterity"), "save:dex");
        assert_eq!(proficiency_id("Fishing-Rod"), "fishing-rod");
        assert_eq!(proficiency_name("save:wis"), "WIS saves");
        assert_eq!(proficiency_name("cooks-utensils"), "Cook's Utensils");
        assert!(skill("cooking").is_none());
    }
}
