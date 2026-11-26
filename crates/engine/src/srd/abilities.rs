//! The six ability scores, their modifiers, the proficiency bonus, the 18 SRD skills and the
//! SRD's tools (the ids are the SRD data's index names: `sleight-of-hand`, `thieves-tools`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// The SRD's weapon and armor proficiencies (categories and single weapons): id, name.
pub const GEAR: [(&str, &str); 56] = [
    ("light-armor", "Light Armor"),
    ("medium-armor", "Medium Armor"),
    ("heavy-armor", "Heavy Armor"),
    ("all-armor", "All armor"),
    ("padded-armor", "Padded Armor"),
    ("leather-armor", "Leather Armor"),
    ("studded-leather-armor", "Studded Leather Armor"),
    ("hide-armor", "Hide Armor"),
    ("chain-shirt", "Chain Shirt"),
    ("scale-mail", "Scale Mail"),
    ("breastplate", "Breastplate"),
    ("half-plate-armor", "Half Plate Armor"),
    ("ring-mail", "Ring Mail"),
    ("chain-mail", "Chain Mail"),
    ("splint-armor", "Splint Armor"),
    ("plate-armor", "Plate Armor"),
    ("shields", "Shields"),
    ("simple-weapons", "Simple Weapons"),
    ("martial-weapons", "Martial Weapons"),
    ("clubs", "Clubs"),
    ("daggers", "Daggers"),
    ("greatclubs", "Greatclubs"),
    ("handaxes", "Handaxes"),
    ("javelins", "Javelins"),
    ("light-hammers", "Light hammers"),
    ("maces", "Maces"),
    ("quarterstaffs", "Quarterstaffs"),
    ("sickles", "Sickles"),
    ("spears", "Spears"),
    ("crossbows-light", "Crossbows, light"),
    ("darts", "Darts"),
    ("shortbows", "Shortbows"),
    ("slings", "Slings"),
    ("battleaxes", "Battleaxes"),
    ("flails", "Flails"),
    ("glaives", "Glaives"),
    ("greataxes", "Greataxes"),
    ("greatswords", "Greatswords"),
    ("halberds", "Halberds"),
    ("lances", "Lances"),
    ("longswords", "Longswords"),
    ("mauls", "Mauls"),
    ("morningstars", "Morningstars"),
    ("pikes", "Pikes"),
    ("rapiers", "Rapiers"),
    ("scimitars", "Scimitars"),
    ("shortswords", "Shortswords"),
    ("tridents", "Tridents"),
    ("war-picks", "War picks"),
    ("warhammers", "Warhammers"),
    ("whips", "Whips"),
    ("blowguns", "Blowguns"),
    ("hand-crossbows", "Hand crossbows"),
    ("crossbows-heavy", "Crossbows, heavy"),
    ("longbows", "Longbows"),
    ("nets", "Nets"),
];

/// A tool by id or name (any case; "Thieves' Tools" = "thieves-tools").
pub fn tool(id: &str) -> Option<(&'static str, &'static str, Ability)> {
    let k = norm(id);
    TOOLS.iter().copied().find(|(i, n, _)| *i == k || norm(n) == k)
}

/// A weapon or armor proficiency by id or name ("Light hammers" = "light-hammers").
pub fn gear(id: &str) -> Option<(&'static str, &'static str)> {
    let k = norm(id);
    GEAR.iter().copied().find(|(i, n)| *i == k || norm(n) == k)
}

/// Whether an id is a proficiency the SRD knows: a skill, a tool, a weapon or armor
/// proficiency, or a saving throw (`save:dex`).
pub fn is_known_proficiency(id: &str) -> bool {
    let id = id.trim();
    match id.strip_prefix("save:") {
        Some(a) => Ability::parse(a).is_some(),
        None => skill(id).is_some() || tool(id).is_some() || gear(id).is_some(),
    }
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
    if let Some((id, _)) = gear(t) {
        return id.to_string();
    }
    t.to_ascii_lowercase()
}

/// A proficiency's display name: "Stealth", "Thieves' Tools", "DEX saves", else the id.
pub fn proficiency_name(id: &str) -> String {
    if let Some(a) = id.strip_prefix("save:").and_then(Ability::parse) {
        return format!("{} saves", a.short());
    }
    skill(id).or_else(|| tool(id)).map(|(_, n, _)| n.to_string()).or_else(|| gear(id).map(|(_, n)| n.to_string())).unwrap_or_else(|| id.to_string())
}

/// How the character creator sets the base ability scores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum AbilityMethod {
    /// 15, 14, 13, 12, 10, 8, swapped between the abilities.
    Standard,
    /// 27 points, every score 8 to 15.
    PointBuy,
    /// 4d6, the lowest die dropped, six times; swapped between the abilities.
    Roll,
}

impl AbilityMethod {
    pub const ALL: [AbilityMethod; 3] = [AbilityMethod::Standard, AbilityMethod::PointBuy, AbilityMethod::Roll];
    pub fn label(self) -> &'static str {
        match self {
            AbilityMethod::Standard => "Standard array",
            AbilityMethod::PointBuy => "Point buy",
            AbilityMethod::Roll => "Roll 4d6",
        }
    }
}

/// Points a point-buy budget has.
pub const POINT_BUY_BUDGET: i32 = 27;

/// What a score costs in point buy (8 = 0 … 13 = 5, 14 = 7, 15 = 9); `None` outside 8-15.
pub fn point_buy_cost(score: i32) -> Option<i32> {
    match score {
        8..=13 => Some(score - 8),
        14 => Some(7),
        15 => Some(9),
        _ => None,
    }
}

/// The points six scores cost (scores outside 8-15 count as impossible: `None`).
pub fn point_buy_spent(scores: &Abilities) -> Option<i32> {
    scores.0.iter().map(|s| point_buy_cost(*s)).sum()
}

/// One score rolled the SRD way: 4d6, the lowest die dropped (the seeded dice).
pub fn roll_score() -> i32 {
    let mut d: Vec<u32> = (0..4).map(|_| super::dice::die(6)).collect();
    d.sort_unstable();
    d[1..].iter().sum::<u32>() as i32
}

/// Assigning a pool of six values (the standard array, six rolls) to the abilities: ability `i`
/// takes the next lower (`step` < 0) or higher value of the pool, and the ability that had that
/// value takes `i`'s. Equal values count as one step.
pub fn swap_assigned(scores: &mut Abilities, i: usize, step: i32) {
    let mut values: Vec<i32> = scores.0.to_vec();
    values.sort_unstable();
    values.dedup();
    let cur = scores.0[i];
    let Some(k) = values.iter().position(|v| *v == cur) else { return };
    let target = values[(k as i32 + step).rem_euclid(values.len() as i32) as usize];
    if let Some(j) = scores.0.iter().position(|v| *v == target) {
        scores.0.swap(i, j);
    }
}

/// Every proficiency id: the 18 skills, the tools, the six saving throws (editor suggestions).
pub fn proficiency_choices() -> Vec<String> {
    SKILLS.iter().map(|s| s.0.to_string()).chain(TOOLS.iter().map(|t| t.0.to_string())).chain(Ability::ALL.iter().map(|a| format!("save:{}", a.id()))).chain(GEAR.iter().map(|g| g.0.to_string())).collect()
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
    fn point_buy_and_swaps() {
        assert_eq!([8, 9, 13, 14, 15].map(point_buy_cost), [Some(0), Some(1), Some(5), Some(7), Some(9)]);
        assert_eq!(point_buy_cost(16), None);
        assert_eq!(point_buy_cost(7), None);
        // 15 15 15 8 8 8 = 27 points; the standard array costs 27 too.
        assert_eq!(point_buy_spent(&Abilities([15, 15, 15, 8, 8, 8])), Some(27));
        assert_eq!(point_buy_spent(&Abilities::STANDARD), Some(27));
        assert_eq!(point_buy_spent(&Abilities([16, 8, 8, 8, 8, 8])), None);
        let mut a = Abilities::STANDARD;
        swap_assigned(&mut a, 5, 1); // CHA 8 takes 10, WIS takes 8
        assert_eq!(a, Abilities([15, 14, 13, 12, 8, 10]));
        swap_assigned(&mut a, 0, 1); // STR 15 wraps around to the lowest, 8
        assert_eq!(a, Abilities([8, 14, 13, 12, 15, 10]));
        super::super::dice::seed(9);
        for _ in 0..200 {
            assert!((3..=18).contains(&roll_score()));
        }
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
        assert_eq!(proficiency_id("Light hammers"), "light-hammers");
        assert_eq!(proficiency_name("crossbows-light"), "Crossbows, light");
        assert!(is_known_proficiency("battleaxes") && is_known_proficiency("save:wis") && is_known_proficiency("thieves-tools"));
        assert!(!is_known_proficiency("swordsmanship") && !is_known_proficiency("save:luck"));
    }
}
