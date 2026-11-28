//! Crafting recipes (`defs/recipes.ron`): ingredients from the player's items become a result
//! item at a crafting station (a name such as "alchemy" or "anvil"; the *Open crafting* action
//! / `open_crafting(station)` opens the window with that station's recipes).

use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;
use crate::state::WorldState;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RecipeDef {
    pub id: String,
    /// Item made.
    pub result: String,
    /// How many are made at once.
    #[serde(default = "one")]
    pub count: u32,
    /// Items used up.
    #[serde(default)]
    pub ingredients: Vec<ItemStack>,
    /// Where it is made ("" = the recipes of `open_crafting("")`, e.g. a crafting kit).
    #[serde(default)]
    pub station: String,
    /// The proficiency the recipe is about: a tool (`smiths-tools`) or an SRD skill; "" = none.
    /// Crafting rolls a check with it for the quality steps (Intelligence with a tool, the
    /// skill's ability with a skill).
    #[serde(default)]
    pub skill: String,
    /// More than 0: only a hero proficient in `skill` can make it (0.27 and older: the skill
    /// level needed).
    #[serde(default)]
    pub level: u32,
    /// Tool kinds needed (`ItemDef::tool`, e.g. "hammer"), not used up: in the Tool slot, held
    /// or in the inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// Better results for better rolls (a *Fine Iron Sword* is its own item): crafting rolls the
    /// `skill` check, and the highest step whose DC the roll meets replaces `result`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality: Vec<QualityStep>,
    /// The state the station's prop must be in (e.g. "lit" for a cooking fire); "" = any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub station_state: String,
}

/// A crafting check of `dc` or more makes `result` instead (`RecipeDef::quality`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct QualityStep {
    #[serde(default)]
    pub dc: u32,
    pub result: String,
}

/// What crafting made.
#[derive(Clone, Debug, PartialEq)]
pub struct Crafted {
    /// The item made (a quality step's when the roll met its DC).
    pub result: String,
    /// How many didn't fit into the inventory (lost: callers check `fits` first).
    pub lost: u32,
    /// The quality check, when the recipe has quality steps.
    pub roll: Option<crate::srd::RollRecord>,
}

/// Item id -> tool kind (`Database::tool_kinds`), to tell which tools the player has.
pub type ToolKinds = std::collections::HashMap<String, String>;

/// Whether the player has a tool of `kind`: worn / held, or in the inventory.
pub fn has_tool(world: &WorldState, kinds: &ToolKinds, kind: &str) -> bool {
    let is = |id: &str| kinds.get(id).is_some_and(|k| k.eq_ignore_ascii_case(kind));
    world.equipment.stacks().any(|s| is(&s.item)) || world.inventory.item_ids().iter().any(|id| is(id))
}

fn one() -> u32 {
    1
}

/// Why a recipe can't be made right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    /// Ingredients short: (item, have, need).
    Items(Vec<(String, u32, u32)>),
    /// The hero isn't proficient in the recipe's skill or tool.
    Proficiency(String),
    /// Tools the player doesn't have (tool kinds).
    Tools(Vec<String>),
    /// The station isn't in the state the recipe needs (e.g. the fire isn't "lit").
    StationState(String),
}

impl RecipeDef {
    pub fn new(id: &str, result: &str, count: u32, ingredients: Vec<ItemStack>, station: &str) -> Self {
        RecipeDef { id: id.into(), result: result.into(), count, ingredients, station: station.into(), skill: String::new(), level: 0, tools: Vec::new(), quality: Vec::new(), station_state: String::new() }
    }

    /// The item a crafting check of `total` makes: the best quality step it meets, else `result`.
    pub fn result_for_total(&self, total: i64) -> &str {
        self.quality.iter().filter(|q| q.dc as i64 <= total && !q.result.is_empty()).max_by_key(|q| q.dc).map_or(self.result.as_str(), |q| q.result.as_str())
    }

    /// The check crafting rolls for the quality steps: `int:<tool>`, an SRD skill as it is, `int`
    /// without a skill.
    pub fn check_what(&self) -> String {
        let s = self.skill.trim();
        if s.is_empty() {
            "int".into()
        } else if crate::srd::abilities::skill(s).is_some() || s.contains(':') {
            s.into()
        } else {
            format!("int:{s}")
        }
    }

    /// Whether the hero needs a proficiency they lack.
    pub fn needs_proficiency(&self, world: &WorldState) -> bool {
        self.level > 0 && !self.skill.trim().is_empty() && !world.is_proficient(&self.skill)
    }

    /// What is missing to make it once (None = it can be made).
    pub fn missing(&self, world: &WorldState, kinds: &ToolKinds) -> Option<Missing> {
        if self.needs_proficiency(world) {
            return Some(Missing::Proficiency(self.skill.clone()));
        }
        let tools: Vec<String> = self.tools.iter().filter(|t| !t.trim().is_empty() && !has_tool(world, kinds, t)).cloned().collect();
        if !tools.is_empty() {
            return Some(Missing::Tools(tools));
        }
        let short: Vec<(String, u32, u32)> = self
            .ingredients
            .iter()
            .filter(|s| !s.item.is_empty())
            .map(|s| (s.item.clone(), world.item_count(&s.item), s.count))
            .filter(|(_, have, need)| have < need)
            .collect();
        (!short.is_empty()).then_some(Missing::Items(short))
    }

    /// Makes it once: takes the ingredients, rolls the quality check (recipes with quality
    /// steps; a failed roll still makes `result`), gives the item. Or what is missing.
    pub fn craft(&self, world: &mut WorldState, kinds: &ToolKinds) -> Result<Crafted, Missing> {
        if let Some(m) = self.missing(world, kinds) {
            return Err(m);
        }
        let roll = if self.quality.iter().any(|q| !q.result.is_empty()) {
            let dc = self.quality.iter().map(|q| q.dc).min().unwrap_or(10) as i32;
            world.check(&self.check_what(), dc, crate::srd::Mode::Normal, false)
        } else {
            None
        };
        let result = roll.as_ref().map_or(self.result.as_str(), |r| self.result_for_total(r.total)).to_string();
        for s in self.ingredients.iter().filter(|s| !s.item.is_empty()) {
            world.take_item(&s.item, s.count);
        }
        let lost = world.give_item(&result, self.count.max(1));
        Ok(Crafted { result, lost, roll })
    }

    /// Whether the result would fit into the inventory once the ingredients are used up.
    pub fn fits(&self, world: &WorldState) -> bool {
        let mut w = WorldState { inventory: world.inventory.clone(), equipment: world.equipment.clone(), ..Default::default() };
        for s in self.ingredients.iter().filter(|s| !s.item.is_empty()) {
            w.take_item(&s.item, s.count);
        }
        w.give_item(&self.result, self.count.max(1)) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crafting_uses_ingredients() {
        let r = RecipeDef::new("potion", "potion", 2, vec![ItemStack::new("herb", 2), ItemStack::new("apple", 1)], "alchemy");
        let mut w = WorldState::default();
        w.give_item("herb", 3);
        assert_eq!(r.missing(&w, &ToolKinds::new()), Some(Missing::Items(vec![("apple".into(), 0, 1)])));
        assert!(r.craft(&mut w, &ToolKinds::new()).is_err());
        assert_eq!(w.item_count("herb"), 3, "nothing used when it fails");
        w.give_item("apple", 1);
        assert!(r.fits(&w));
        assert_eq!(r.craft(&mut w, &ToolKinds::new()).map(|c| (c.lost, c.roll.is_none())), Ok((0, true)));
        assert_eq!((w.item_count("herb"), w.item_count("apple"), w.item_count("potion")), (1, 0, 2));
        let skilled = RecipeDef { skill: "alchemists-supplies".into(), level: 1, ..r };
        assert_eq!(skilled.missing(&w, &ToolKinds::new()), Some(Missing::Proficiency("alchemists-supplies".into())));
        w.set_proficient("Alchemist's Supplies", true);
        assert_eq!(skilled.missing(&w, &ToolKinds::new()), Some(Missing::Items(vec![("herb".into(), 1, 2), ("apple".into(), 0, 1)])));
    }

    #[test]
    fn tools_are_needed_not_used() {
        let r = RecipeDef { tools: vec!["hammer".into()], ..RecipeDef::new("blade", "sword", 1, vec![ItemStack::new("ingot", 1)], "anvil") };
        let kinds: ToolKinds = [("smith_hammer".to_string(), "hammer".to_string())].into_iter().collect();
        let mut w = WorldState::default();
        w.give_item("ingot", 2);
        assert_eq!(r.missing(&w, &kinds), Some(Missing::Tools(vec!["hammer".into()])));
        w.give_item("smith_hammer", 1);
        assert_eq!(r.craft(&mut w, &kinds).map(|c| c.lost), Ok(0));
        assert_eq!((w.item_count("smith_hammer"), w.item_count("ingot")), (1, 1), "the tool stays");
        // Equipped counts too.
        w.take_item("smith_hammer", 1);
        *w.equipment.slot_mut(crate::defs::EquipSlot::Tool) = Some(ItemStack::new("smith_hammer", 1));
        assert!(has_tool(&w, &kinds, "Hammer"));
    }

    #[test]
    fn quality_steps_and_upgrades() {
        let mut r = RecipeDef { skill: "smiths-tools".into(), level: 0, ..RecipeDef::new("sword", "iron_sword", 1, vec![ItemStack::new("ingot", 2)], "anvil") };
        r.quality = vec![QualityStep { dc: 12, result: "fine_sword".into() }, QualityStep { dc: 18, result: "master_sword".into() }];
        assert_eq!((r.result_for_total(5), r.result_for_total(12), r.result_for_total(17), r.result_for_total(25)), ("iron_sword", "fine_sword", "fine_sword", "master_sword"));
        assert_eq!(r.check_what(), "int:smiths-tools");
        let mut w = WorldState::default();
        w.give_item("ingot", 6);
        crate::srd::dice::seed(11);
        let c = r.craft(&mut w, &ToolKinds::new()).unwrap();
        let roll = c.roll.expect("quality steps roll");
        assert_eq!(c.result, r.result_for_total(roll.total));
        assert_eq!(w.item_count(&c.result), 1);
        assert_eq!(w.rolls.len(), 1, "the roll is in the log");
        // An equipped item can be an ingredient (upgrading it).
        let up = RecipeDef::new("up", "fine_sword", 1, vec![ItemStack::new("sword", 1), ItemStack::new("ingot", 1)], "anvil");
        *w.equipment.slot_mut(crate::defs::EquipSlot::Weapon) = Some(ItemStack::new("sword", 1));
        assert_eq!(up.missing(&w, &ToolKinds::new()), None);
        assert_eq!(up.craft(&mut w, &ToolKinds::new()).map(|c| c.lost), Ok(0));
        assert_eq!(w.equipment.get(crate::defs::EquipSlot::Weapon), None);
    }

    #[test]
    fn minimal_recipe_in_ron() {
        let r: RecipeDef = ron::from_str("(id: \"a\", result: \"b\")").unwrap();
        assert_eq!((r.count, r.ingredients.len(), r.station.as_str(), r.level), (1, 0, "", 0));
    }

    #[test]
    fn quality_steps_read_their_dc() {
        let q: QualityStep = ron::from_str("(dc: 17, result: \"fine\")").unwrap();
        assert_eq!(q.dc, 17);
        assert!(ron::to_string(&q).unwrap().contains("dc:17"));
    }
}
