//! The SRD's magic items (`ItemDef::magic`): rarity, attunement, a +N bonus, passive effects
//! while worn or held (the class feature vocabulary), charges that come back at dawn, spells cast
//! from the item, and what using it does (spell parts: a potion's healing, a buff). Pure data and
//! rules; the runtime is `runtime/magic_items.rs`.

use serde::{Deserialize, Serialize};

use crate::classes::FeatureEffect;
use crate::skills::SpellPart;

/// At most this many items are attuned at once.
pub const MAX_ATTUNED: usize = 3;

/// How rare a magic item is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    VeryRare,
    Legendary,
    Artifact,
}

impl Rarity {
    pub const ALL: [Rarity; 6] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::VeryRare, Rarity::Legendary, Rarity::Artifact];

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::VeryRare => "Very rare",
            Rarity::Legendary => "Legendary",
            Rarity::Artifact => "Artifact",
        }
    }

    /// By the SRD's name ("Very Rare", "very rare").
    pub fn parse(s: &str) -> Option<Rarity> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "common" => Rarity::Common,
            "uncommon" => Rarity::Uncommon,
            "rare" => Rarity::Rare,
            "very rare" | "veryrare" | "very-rare" => Rarity::VeryRare,
            "legendary" => Rarity::Legendary,
            "artifact" => Rarity::Artifact,
            _ => return None,
        })
    }

    /// A price in copper pieces for the rarity (the SRD's *Magic Item Rarity* table gives ranges:
    /// common 50-100 gp, uncommon 101-500, rare 501-5,000, very rare 5,001-50,000, legendary
    /// 50,001+; an artifact is priceless).
    pub fn price_cp(self) -> u32 {
        100 * match self {
            Rarity::Common => 100,
            Rarity::Uncommon => 400,
            Rarity::Rare => 4_000,
            Rarity::VeryRare => 40_000,
            Rarity::Legendary => 200_000,
            Rarity::Artifact => 0,
        }
    }
}

/// A spell cast from an item (a wand's, a staff's): it spends `charges` (more for a higher slot
/// when the item allows it: one more charge per level).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ItemSpell {
    /// The spell's id.
    pub spell: String,
    /// Charges a cast spends (0 = free: at will).
    pub charges: u32,
    /// The slot level it is cast at (0 = the spell's own level).
    #[serde(skip_serializing_if = "is_zero")]
    pub slot: u32,
    /// One more charge raises the slot level by one (Wand of Magic Missiles), up to this slot
    /// (0 = not).
    #[serde(skip_serializing_if = "is_zero")]
    pub upcast_to: u32,
    /// Its save DC and attack bonus come from the item (DC; the attack bonus is DC - 8); 0 = the
    /// user's own spellcasting.
    #[serde(skip_serializing_if = "is_zero")]
    pub dc: u32,
}

/// What makes an item magic.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct MagicItem {
    pub rarity: Rarity,
    /// It works only while attuned (at most [`MAX_ATTUNED`] items).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub attunement: bool,
    /// Who can attune to it: class ids, race ids or `spellcaster`; empty = anyone.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attune_by: Vec<String>,
    /// +N: a weapon's or ammunition's attack and damage rolls, armor's or a shield's AC.
    #[serde(skip_serializing_if = "is_zero_i")]
    pub bonus: i32,
    /// What it does while worn or held (and attuned when it needs it): the class features'
    /// vocabulary (AC and save bonuses, resistances, ability scores, speed…).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<FeatureEffect>,
    /// Its charges (0 = none).
    #[serde(skip_serializing_if = "is_zero")]
    pub charges: u32,
    /// The charges it regains at dawn: dice (`1d6+1`), `all`, or "" = none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub recharge: String,
    /// Spending its last charge rolls a d20: on a 1 it is destroyed.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub destroy_on_last: bool,
    /// Spells cast from it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spells: Vec<ItemSpell>,
    /// What using it does to its user (or whom it is used on): spell parts (a potion's `Heal`
    /// 2d4+2, a `Buff`, a `Condition`), for `use_seconds` (0 = instant parts only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub use_parts: Vec<SpellPart>,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub use_seconds: f32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_zero_i(n: &i32) -> bool {
    *n == 0
}

fn is_zero_f(n: &f32) -> bool {
    *n == 0.0
}

impl MagicItem {
    /// Whether it does nothing the rules know (a story item: its effect is up to scripts).
    pub fn story_only(&self) -> bool {
        self.bonus == 0 && self.effects.is_empty() && self.spells.is_empty() && self.use_parts.is_empty()
    }

    /// How many charges come back at dawn from `spent` used ones (`roll` rolls dice).
    pub fn recharged(&self, spent: u32, roll: impl FnOnce(&str) -> u32) -> u32 {
        let r = self.recharge.trim();
        if spent == 0 || r.is_empty() {
            return spent;
        }
        if r.eq_ignore_ascii_case("all") {
            return 0;
        }
        spent.saturating_sub(roll(r))
    }
}

/// The SRD's text of an item's first line ("Wand, uncommon (requires attunement by a
/// spellcaster)"): whether it needs attunement and by whom (`spellcaster`, class or race ids:
/// "by a cleric, druid, or paladin" = `cleric`, `druid`, `paladin`).
pub fn parse_attunement(line: &str) -> (bool, Vec<String>) {
    let l = line.to_ascii_lowercase();
    let Some(at) = l.find("requires attunement") else { return (false, Vec::new()) };
    let rest = &l[at + "requires attunement".len()..];
    let rest = rest.split(')').next().unwrap_or("");
    let Some(by) = rest.trim().strip_prefix("by ") else { return (true, Vec::new()) };
    let who: Vec<String> = by
        .replace(", or ", ",")
        .replace(" or ", ",")
        .split(',')
        .map(|w| w.trim().trim_start_matches("a ").trim_start_matches("an ").trim().replace(' ', "-"))
        .filter(|w| !w.is_empty())
        .collect();
    (true, who)
}

/// The SRD's text on charges: "has 7 charges" and "regains 1d6 + 1 expended charges daily at
/// dawn" (or "all expended charges"). (0, "") when it says nothing.
pub fn parse_charges(text: &str) -> (u32, String) {
    let l = text.to_ascii_lowercase();
    let words: Vec<&str> = l.split_whitespace().collect();
    let mut charges = 0;
    for w in words.windows(3) {
        if w[0] == "has" && (w[2].starts_with("charges") || w[2].starts_with("charge")) {
            if let Ok(n) = w[1].parse::<u32>() {
                charges = n;
                break;
            }
        }
    }
    if charges == 0 {
        return (0, String::new());
    }
    let recharge = match l.find("regains ") {
        Some(at) => {
            let rest = &l[at + "regains ".len()..];
            if rest.starts_with("all") {
                "all".to_string()
            } else {
                let dice: String = rest.split("expended").next().unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
                if crate::srd::Expr::parse(&dice).is_ok() { dice } else { String::new() }
            }
        }
        None => String::new(),
    };
    (charges, recharge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attunement_lines() {
        assert_eq!(parse_attunement("Ring, rare (requires attunement)"), (true, vec![]));
        assert_eq!(parse_attunement("Wand, uncommon (requires attunement by a spellcaster)"), (true, vec!["spellcaster".to_string()]));
        assert_eq!(parse_attunement("Staff, rare (requires attunement by a druid, sorcerer, warlock, or wizard)").1, vec!["druid", "sorcerer", "warlock", "wizard"]);
        assert_eq!(parse_attunement("Weapon (any sword), legendary (requires attunement by a paladin)").1, vec!["paladin"]);
        assert_eq!(parse_attunement("Potion, common"), (false, vec![]));
    }

    #[test]
    fn charge_texts() {
        assert_eq!(parse_charges("This wand has 7 charges. … The wand regains 1d6 + 1 expended charges daily at dawn."), (7, "1d6+1".into()));
        assert_eq!(parse_charges("The staff has 10 charges and regains 1d6 + 4 expended charges daily at dawn."), (10, "1d6+4".into()));
        assert_eq!(parse_charges("It has 3 charges, and it regains all expended charges daily at dawn."), (3, "all".into()));
        assert_eq!(parse_charges("You gain a +1 bonus to AC."), (0, String::new()));
    }

    #[test]
    fn dawn_recharge() {
        let m = MagicItem { charges: 7, recharge: "1d6+1".into(), ..Default::default() };
        assert_eq!(m.recharged(5, |_| 3), 2);
        assert_eq!(m.recharged(2, |_| 7), 0);
        assert_eq!(MagicItem { recharge: "all".into(), ..m.clone() }.recharged(6, |_| 0), 0);
        assert_eq!(MagicItem { recharge: String::new(), ..m }.recharged(4, |_| 9), 4);
    }

    #[test]
    fn prices_rise_with_rarity() {
        assert!(Rarity::ALL.windows(2).take(4).all(|w| w[0].price_cp() < w[1].price_cp()));
        assert_eq!(Rarity::parse("Very Rare"), Some(Rarity::VeryRare));
    }
}
