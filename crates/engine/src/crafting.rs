//! Crafting recipes (`defs/recipes.ron`): ingredients from the player's items become a result
//! item at a crafting station (a name such as "alchemy" or "anvil"; the *Open crafting* action
//! / `open_crafting(station)` opens the window with that station's recipes).

use serde::{Deserialize, Serialize};

use crate::inventory::ItemStack;
use crate::state::WorldState;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Skill needed (id, "" = none) and its level.
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub level: u32,
    /// Tool kinds needed (`ItemDef::tool`, e.g. "hammer"), not used up: in the Tool slot, held
    /// or in the inventory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// Better results at higher levels of `skill` (a *Fine Iron Sword* is its own item): the
    /// highest step the player's level reaches replaces `result`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality: Vec<QualityStep>,
    /// The state the station's prop must be in (e.g. "lit" for a cooking fire); "" = any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub station_state: String,
}

/// From skill level `level` on, a recipe makes `result` instead (`RecipeDef::quality`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityStep {
    pub level: u32,
    pub result: String,
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
    /// Skill level too low: (skill, have, need).
    Skill(String, u32, u32),
    /// Tools the player doesn't have (tool kinds).
    Tools(Vec<String>),
    /// The station isn't in the state the recipe needs (e.g. the fire isn't "lit").
    StationState(String),
}

impl RecipeDef {
    pub fn new(id: &str, result: &str, count: u32, ingredients: Vec<ItemStack>, station: &str) -> Self {
        RecipeDef { id: id.into(), result: result.into(), count, ingredients, station: station.into(), skill: String::new(), level: 0, tools: Vec::new(), quality: Vec::new(), station_state: String::new() }
    }

    /// The item it makes for the player now: the best quality step their skill reaches.
    pub fn result_for(&self, world: &WorldState) -> &str {
        if self.skill.is_empty() {
            return &self.result;
        }
        let have = world.skill_level(&self.skill);
        self.quality.iter().filter(|q| q.level <= have && !q.result.is_empty()).max_by_key(|q| q.level).map_or(self.result.as_str(), |q| q.result.as_str())
    }

    /// What is missing to make it once (None = it can be made).
    pub fn missing(&self, world: &WorldState, kinds: &ToolKinds) -> Option<Missing> {
        if !self.skill.is_empty() && world.skill_level(&self.skill) < self.level {
            return Some(Missing::Skill(self.skill.clone(), world.skill_level(&self.skill), self.level));
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

    /// Makes it once: takes the ingredients, gives the result. Returns how many of the result
    /// didn't fit into the inventory (they are lost: callers check `fits` first), or what is
    /// missing.
    pub fn craft(&self, world: &mut WorldState, kinds: &ToolKinds) -> Result<u32, Missing> {
        if let Some(m) = self.missing(world, kinds) {
            return Err(m);
        }
        let result = self.result_for(world).to_string();
        for s in self.ingredients.iter().filter(|s| !s.item.is_empty()) {
            world.take_item(&s.item, s.count);
        }
        Ok(world.give_item(&result, self.count.max(1)))
    }

    /// Whether the result would fit into the inventory once the ingredients are used up.
    pub fn fits(&self, world: &WorldState) -> bool {
        let mut w = WorldState { inventory: world.inventory.clone(), equipment: world.equipment.clone(), ..Default::default() };
        for s in self.ingredients.iter().filter(|s| !s.item.is_empty()) {
            w.take_item(&s.item, s.count);
        }
        w.give_item(self.result_for(world), self.count.max(1)) == 0
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
        assert_eq!(r.craft(&mut w, &ToolKinds::new()), Ok(0));
        assert_eq!((w.item_count("herb"), w.item_count("apple"), w.item_count("potion")), (1, 0, 2));
        let skilled = RecipeDef { skill: "alchemy".into(), level: 2, ..r };
        assert_eq!(skilled.missing(&w, &ToolKinds::new()), Some(Missing::Skill("alchemy".into(), 0, 2)));
    }

    #[test]
    fn tools_are_needed_not_used() {
        let r = RecipeDef { tools: vec!["hammer".into()], ..RecipeDef::new("blade", "sword", 1, vec![ItemStack::new("ingot", 1)], "anvil") };
        let kinds: ToolKinds = [("smith_hammer".to_string(), "hammer".to_string())].into_iter().collect();
        let mut w = WorldState::default();
        w.give_item("ingot", 2);
        assert_eq!(r.missing(&w, &kinds), Some(Missing::Tools(vec!["hammer".into()])));
        w.give_item("smith_hammer", 1);
        assert_eq!(r.craft(&mut w, &kinds), Ok(0));
        assert_eq!((w.item_count("smith_hammer"), w.item_count("ingot")), (1, 1), "the tool stays");
        // Equipped counts too.
        w.take_item("smith_hammer", 1);
        *w.equipment.slot_mut(crate::defs::EquipSlot::Tool) = Some(ItemStack::new("smith_hammer", 1));
        assert!(has_tool(&w, &kinds, "Hammer"));
    }

    #[test]
    fn quality_steps_and_upgrades() {
        let mut r = RecipeDef { skill: "smithing".into(), level: 0, ..RecipeDef::new("sword", "iron_sword", 1, vec![ItemStack::new("ingot", 2)], "anvil") };
        r.quality = vec![QualityStep { level: 2, result: "fine_sword".into() }, QualityStep { level: 4, result: "master_sword".into() }];
        let mut w = WorldState::default();
        w.give_item("ingot", 6);
        assert_eq!(r.result_for(&w), "iron_sword");
        w.skills.insert("smithing".into(), 3);
        assert_eq!(r.result_for(&w), "fine_sword");
        assert_eq!(r.craft(&mut w, &ToolKinds::new()), Ok(0));
        assert_eq!((w.item_count("fine_sword"), w.item_count("iron_sword")), (1, 0));
        // An equipped item can be an ingredient (upgrading it).
        let up = RecipeDef::new("up", "fine_sword", 1, vec![ItemStack::new("sword", 1), ItemStack::new("ingot", 1)], "anvil");
        *w.equipment.slot_mut(crate::defs::EquipSlot::Weapon) = Some(ItemStack::new("sword", 1));
        assert_eq!(up.missing(&w, &ToolKinds::new()), None);
        assert_eq!(up.craft(&mut w, &ToolKinds::new()), Ok(0));
        assert_eq!(w.equipment.get(crate::defs::EquipSlot::Weapon), None);
    }

    #[test]
    fn minimal_recipe_in_ron() {
        let r: RecipeDef = ron::from_str("(id: \"a\", result: \"b\")").unwrap();
        assert_eq!((r.count, r.ingredients.len(), r.station.as_str(), r.level), (1, 0, "", 0));
    }
}
