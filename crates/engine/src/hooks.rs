//! Script hooks: script functions a project registers (`ProjectConfig::hooks`) to change what the
//! engine does. A hook gets a map describing what is about to happen (`#{target, amount, …}`) and
//! returns a changed map, or `()` to keep it; several functions on one hook run in priority order
//! (lower numbers first), each getting what the one before returned. Only hooks with functions are
//! called, under a Rhai operation limit ([`crate::script::HOOK_OPERATIONS`]).

use std::collections::BTreeMap;

use rhai::Dynamic;
use serde::{Deserialize, Serialize};

/// One registered hook: the engine calls `function` at `hook`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HookDef {
    pub hook: String,
    pub function: String,
    /// Lower runs first; equal ones in the order listed.
    #[serde(default)]
    pub priority: i32,
}

/// Every hook: name, the map it gets, what it does. The order is the editor's.
pub const HOOKS: &[(&str, &str, &str)] = &[
    ("damage", "attacker, target, target_def, amount, crit", "A hit is about to hurt the player or a character: `amount` is the damage after defense (set it; 0 = no damage). `attacker` is \"player\" for the player's own hits, else \"\"; `target` \"player\" or the character's name (`target_def` its definition)."),
    ("heal", "target, amount", "The player or a character is about to be healed by `amount` (set it; 0 = no healing)."),
    ("item_use", "item, cancel", "The player uses an item from the inventory: set `cancel` to true to stop it."),
    ("xp", "amount", "The player is about to gain (or lose) `amount` experience (set it)."),
    ("level_up", "level, hp, mana", "The player reaches `level`: what it gives (max HP is the project's; `hp` heals, `mana`), each can be set."),
    ("buy", "item, price, merchant", "The price of one `item` in a merchant's stock (set `price`); asked when the trade window opens and after each trade."),
    ("sell", "item, price, merchant", "What a merchant pays for one `item` (set `price`; -1 = not interested)."),
    ("craft", "recipe, count, cancel", "The player makes a recipe: `count` is how many results one make gives (set it), `cancel` stops it."),
    ("map_enter", "map, from", "The player arrived on `map` (from the map `from`, \"\" after loading or a new game). Return nothing; queue actions instead."),
    ("new_game", "", "A new game starts (before the start map is entered): give items, set flags."),
    ("save", "slot", "The game is about to be saved to `slot` (variables set here are saved with it)."),
    ("load", "slot", "A game was loaded from `slot`."),
    ("menu_open", "menu, cancel", "A menu is about to open: `menu` is \"pause\", \"character\", \"skills\", \"spells\", \"journal\" or \"map\"; `cancel` keeps it shut."),
    ("damage_number", "value, kind, text, color, size", "A damage number appears: `kind` is \"hit\", \"crit\" or \"hurt\" (the player's side hurt); set its `text`, `color` (a colour name or #rrggbb; \"\" = the kind's) or `size` (× the usual)."),
    ("effect_apply", "target, condition, seconds, cancel", "A condition is about to start on \"player\" or a character (after its saving throw failed): change `condition` (\"prone\"…) or `seconds`, or `cancel` it. (`effect` is the condition too, as until 0.27.)"),
];

/// Whether `name` is a hook the engine calls.
pub fn known(name: &str) -> bool {
    HOOKS.iter().any(|h| h.0 == name)
}

/// The registered functions per hook, in the order they run.
#[derive(Clone, Debug, Default)]
pub struct HookTable {
    by_hook: BTreeMap<String, Vec<String>>,
}

impl HookTable {
    pub fn new<'a>(defs: impl IntoIterator<Item = &'a HookDef>) -> Self {
        let mut list: Vec<&HookDef> = defs.into_iter().filter(|d| !d.function.is_empty()).collect();
        // (Stable: equal priorities keep their order.)
        list.sort_by_key(|d| d.priority);
        let mut by_hook: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for d in list {
            by_hook.entry(d.hook.clone()).or_default().push(d.function.clone());
        }
        HookTable { by_hook }
    }

    /// The functions of `hook` (empty: not registered, nothing is called).
    pub fn get(&self, hook: &str) -> &[String] {
        self.by_hook.get(hook).map_or(&[], |v| v.as_slice())
    }

    pub fn has(&self, hook: &str) -> bool {
        !self.get(hook).is_empty()
    }
}

/// A hook map from key / value pairs.
pub fn map<const N: usize>(pairs: [(&str, Dynamic); N]) -> rhai::Map {
    pairs.into_iter().map(|(k, v)| (k.into(), v)).collect()
}

/// A number of a hook map (whole numbers and decimals; `default` when missing or not a number).
pub fn num(m: &rhai::Map, key: &str, default: f64) -> f64 {
    m.get(key).and_then(|v| v.as_int().map(|i| i as f64).ok().or_else(|| v.as_float().ok())).unwrap_or(default)
}

/// A whole number of a hook map (rounded).
pub fn int(m: &rhai::Map, key: &str, default: i64) -> i64 {
    num(m, key, default as f64).round() as i64
}

/// A text of a hook map (`default` when missing or not a text).
pub fn text(m: &rhai::Map, key: &str, default: &str) -> String {
    m.get(key).and_then(|v| v.clone().into_string().ok()).unwrap_or_else(|| default.to_string())
}

/// An on / off value of a hook map (false when missing).
pub fn flag(m: &rhai::Map, key: &str) -> bool {
    m.get(key).and_then(|v| v.as_bool().ok()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_run_in_priority_order() {
        let d = |hook: &str, function: &str, priority| HookDef { hook: hook.into(), function: function.into(), priority };
        let t = HookTable::new(&[d("damage", "b", 5), d("damage", "a", 0), d("damage", "c", 5), d("heal", "h", 0), d("xp", "", 0)]);
        assert_eq!(t.get("damage"), ["a", "b", "c"]);
        assert_eq!(t.get("heal"), ["h"]);
        assert!(!t.has("xp") && !t.has("buy"));
        assert!(HOOKS.iter().all(|h| known(h.0)) && !known("nope"));
    }

    #[test]
    fn map_values_read_back() {
        let m = map([("amount", Dynamic::from(3_i64)), ("half", Dynamic::from(2.5_f64)), ("cancel", Dynamic::from(true))]);
        assert_eq!(int(&m, "amount", 0), 3);
        assert_eq!(int(&m, "half", 0), 3);
        assert_eq!(int(&m, "missing", 7), 7);
        assert!(flag(&m, "cancel") && !flag(&m, "amount"));
    }
}
