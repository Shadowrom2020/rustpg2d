//! Persistent game state: flags, variables, inventory and per-map entity changes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::entity::Facing;
use crate::defs::EquipSlot;
use crate::inventory::{Inventory, ItemStack};

/// Items worn / held by a character (the player's are not part of the inventory grid).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equipment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clothing: Option<ItemStack>,
    pub head: Option<ItemStack>,
    pub body: Option<ItemStack>,
    pub weapon: Option<ItemStack>,
    /// Left hand: shield or light.
    pub offhand: Option<ItemStack>,
    /// A tool (fishing rod, …), held while it is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<ItemStack>,
}

impl Equipment {
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemStack> {
        match slot {
            EquipSlot::Clothing => self.clothing.as_ref(),
            EquipSlot::Head => self.head.as_ref(),
            EquipSlot::Body => self.body.as_ref(),
            EquipSlot::Weapon => self.weapon.as_ref(),
            EquipSlot::Offhand => self.offhand.as_ref(),
            EquipSlot::Tool => self.tool.as_ref(),
        }
    }
    pub fn slot_mut(&mut self, slot: EquipSlot) -> &mut Option<ItemStack> {
        match slot {
            EquipSlot::Clothing => &mut self.clothing,
            EquipSlot::Head => &mut self.head,
            EquipSlot::Body => &mut self.body,
            EquipSlot::Weapon => &mut self.weapon,
            EquipSlot::Offhand => &mut self.offhand,
            EquipSlot::Tool => &mut self.tool,
        }
    }
    pub fn item(&self, slot: EquipSlot) -> Option<&str> {
        self.get(slot).map(|s| s.item.as_str())
    }
    pub fn is_empty(&self) -> bool {
        EquipSlot::ALL.iter().all(|s| self.get(*s).is_none())
    }
    /// Every equipped stack.
    pub fn stacks_mut(&mut self) -> impl Iterator<Item = &mut ItemStack> {
        [&mut self.clothing, &mut self.head, &mut self.body, &mut self.weapon, &mut self.offhand].into_iter().filter_map(|s| s.as_mut())
    }
    pub fn stacks(&self) -> impl Iterator<Item = &ItemStack> {
        EquipSlot::ALL.into_iter().filter_map(|s| self.get(s))
    }
    pub fn count(&self, item: &str) -> u32 {
        EquipSlot::ALL.iter().filter_map(|s| self.get(*s)).filter(|s| s.item == item).map(|s| s.count).sum()
    }
}

/// Runtime changes to a placed entity, persisted across map changes and saves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub active: bool,
    pub frame: u32,
    pub x: f32,
    pub y: f32,
    pub facing: Facing,
    /// Remaining stack size of a partly picked-up item.
    #[serde(default)]
    pub count: Option<u32>,
    /// Its lock has been opened.
    #[serde(default)]
    pub unlocked: bool,
    /// Current state of a prop (None in saves from before named states: `frame` is used).
    #[serde(default)]
    pub state: Option<String>,
    /// Where a patrolling character is on its route.
    #[serde(default)]
    pub patrol: Option<PatrolState>,
    /// A character with a schedule stopped following it (Schedule action / script).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub schedule_off: bool,
    /// When its prop state last changed (`WorldState::total_hours`; regrowing states).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_at: Option<f64>,
    /// What grows on a farm plot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planted: Option<Planted>,
}

/// A seed planted on a farm plot (`EntityState::planted`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Planted {
    /// The seed item (its `crop`).
    pub seed: String,
    /// When it was planted (`WorldState::total_hours`).
    pub at: f64,
    /// Extra growing hours from rain.
    #[serde(default)]
    pub bonus: f64,
}

/// Progress of a patrolling character (saved, so a loaded game continues the route).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatrolState {
    /// Waypoint it is heading for.
    pub index: u32,
    /// Walking the route backwards (back-and-forth routes).
    #[serde(default)]
    pub back: bool,
    /// Stopped by an action / script.
    #[serde(default)]
    pub stopped: bool,
    /// A route given by an action / script instead of the placed character's own.
    #[serde(default)]
    pub route: Option<crate::entity::PatrolRoute>,
}

/// The save game layout this version writes (`WorldState::version`). Saves of older versions
/// carry none (0); every field they lack has a default. The number only gives a future
/// migration something to branch on.
pub const SAVE_VERSION: u32 = 1;

fn save_version<S: serde::Serializer>(_: &u32, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u32(SAVE_VERSION)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldState {
    /// [`SAVE_VERSION`] of the program that wrote the save (0: before 0.12).
    #[serde(default, serialize_with = "save_version")]
    pub version: u32,
    pub flags: BTreeSet<String>,
    pub vars: BTreeMap<String, i64>,
    pub inventory: Inventory,
    /// map name -> entity uid -> state
    pub entities: BTreeMap<String, BTreeMap<u32, EntityState>>,
    /// Keys of `once` triggers that already fired.
    pub fired: BTreeSet<String>,
    pub current_map: String,
    pub player_pos: (f32, f32),
    pub player_facing: Facing,
    /// `None` until the game starts (then set to the player's max HP).
    #[serde(default)]
    pub player_hp: Option<i32>,
    /// Old saves only: item id of the equipped weapon (migrated into `equipment`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub equipped: String,
    #[serde(default)]
    pub equipment: Equipment,
    /// Quick action bar: item ids per slot ("" = empty).
    #[serde(default)]
    pub quickbar: Vec<String>,
    /// Seconds since the Unix epoch when this was saved.
    #[serde(default)]
    pub saved_at: u64,
    /// Total play time in seconds.
    #[serde(default)]
    pub play_time: f64,
    /// Time of day in hours (0..24), advanced while playing.
    #[serde(default = "crate::project::default_start_hour")]
    pub hour: f32,
    /// Days passed since the game began (the clock wrapping past midnight, sleeping).
    #[serde(default)]
    pub day: u32,
    /// map -> spawner zone uid -> characters defeated for good (`Respawn::Never`).
    #[serde(default)]
    pub spawner_kills: BTreeMap<String, BTreeMap<u32, u32>>,
    /// map -> spawner zones switched off by name ("" = all of the map's spawners).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spawners_off: BTreeMap<String, std::collections::BTreeSet<String>>,
    /// map -> entity uid -> contents of containers that have been opened.
    #[serde(default)]
    pub containers: BTreeMap<String, BTreeMap<u32, Inventory>>,
    /// map -> entity uid -> current stock of merchants the player traded with.
    #[serde(default)]
    pub shops: BTreeMap<String, BTreeMap<u32, Vec<crate::defs::ShopEntry>>>,
    /// quest id -> progress (started quests only).
    #[serde(default)]
    pub quests: BTreeMap<String, crate::quest::QuestProgress>,
    /// The player character's name and sex, chosen in the character creator.
    #[serde(default)]
    pub player_name: String,
    #[serde(default)]
    pub player_sex: Sex,
    /// The player's race (`races::RaceDef`; "" = none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub player_race: String,
    /// The player's standing with factions that changed (others start from the attitude table).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub standing: BTreeMap<String, i32>,
    /// map -> items the player dropped there (they stay until picked up again).
    #[serde(default)]
    pub dropped: BTreeMap<String, Vec<DroppedItem>>,
    /// Counter for the uids of dropped items (see [`DROPPED_UID_BASE`]).
    #[serde(default)]
    pub next_drop: u32,
    /// skill id -> level (bought with skill points).
    #[serde(default)]
    pub skills: BTreeMap<String, u32>,
    #[serde(default)]
    pub skill_points: u32,
    /// Learned spells.
    #[serde(default)]
    pub spells: BTreeSet<String>,
    /// Current mana.
    #[serde(default)]
    pub mana: f32,
    /// Fog of war: explored tiles per map (only maps with fog of war).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub explored: BTreeMap<String, crate::fog::Explored>,
    /// The look chosen in the character creator (figure and colours).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_appearance: Option<crate::defs::Appearance>,
    /// NPCs whose equipment or carried items changed (scripts, actions): map -> uid -> items.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_items: BTreeMap<String, BTreeMap<u32, NpcItems>>,
    /// The player definition's clothing was put on (saves from before clothing get it once).
    #[serde(default)]
    pub dressed: bool,
    /// Experience points gained in total (experience and levels).
    #[serde(default)]
    pub xp: u64,
    /// The player's level; 0 in old saves and fresh worlds means 1 (see [`WorldState::player_level`]).
    #[serde(default)]
    pub level: u32,
    /// The player's running status effects (poison, haste, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<crate::effects::ActiveEffect>,
    /// Bestiary: character definition id -> how many the player has defeated.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bestiary: BTreeMap<String, u32>,
    /// The last dialog lines (oldest first, at most [`DIALOG_LOG_MAX`]) for the journal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dialog_log: Vec<LogLine>,
    /// Weather set by actions / scripts, by map name (overrides the map's own weather).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub weather: BTreeMap<String, crate::map::Weather>,
    /// Characters following the player, in the order they joined.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companions: Vec<Companion>,
    /// Spells characters learned, by where they are placed ("map/uid"): companions cast them.
    /// Kept when a companion is dismissed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_spells: BTreeMap<String, Vec<String>>,
}

/// A placed character that follows the player from map to map. Its look, items and triggers
/// come from where it is placed (`map`, `uid`); dismissed, it goes back there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    pub map: String,
    pub uid: u32,
    /// Its name (scripts address it by name on every map).
    #[serde(default)]
    pub name: String,
    /// Its HP (None = full).
    #[serde(default)]
    pub hp: Option<i32>,
    /// Knocked down: where it lies on the current map (world pixels); None = on its feet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down: Option<[f32; 2]>,
    /// How it fights (the player's order).
    #[serde(default, skip_serializing_if = "Order::is_attack")]
    pub order: Order,
    /// Told to stay: where it waits on the current map (world pixels); None = follows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post: Option<[f32; 2]>,
    /// Its mana (None = full).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<f32>,
}

/// How a companion fights (`Companion::order`); staying put is separate (`Companion::post`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// Fights hostile characters near the player.
    #[default]
    Attack,
    /// Only fights what hunts the player or itself.
    Defend,
    /// Never fights; backs away from enemies close to it.
    Passive,
}

impl Order {
    pub const ALL: [Order; 3] = [Order::Attack, Order::Defend, Order::Passive];

    fn is_attack(&self) -> bool {
        *self == Order::Attack
    }

    /// The name scripts use (`order("guard", "defend")`).
    pub fn id(self) -> &'static str {
        match self {
            Order::Attack => "attack",
            Order::Defend => "defend",
            Order::Passive => "passive",
        }
    }

    pub fn from_id(s: &str) -> Option<Order> {
        Order::ALL.into_iter().find(|o| o.id().eq_ignore_ascii_case(s.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Order::Attack => "Attack",
            Order::Defend => "Defend",
            Order::Passive => "Passive",
        }
    }
}

/// How many dialog lines the journal keeps.
pub const DIALOG_LOG_MAX: usize = 1000;

/// One line of the dialog log: who said what, or what the player answered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub speaker: String,
    pub text: String,
    /// The option the player picked in a choice.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub answer: bool,
}

/// Worn and carried items of an NPC (saved once they differ from the map / definition).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NpcItems {
    pub equipment: Equipment,
    pub inventory: Vec<ItemStack>,
}

/// Uids of dropped items start here, far above the uids of map entities and of spawned
/// characters (`0x4000_0000..`).
pub const DROPPED_UID_BASE: u32 = 0x6000_0000;

/// An item stack the player dropped on a map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DroppedItem {
    pub uid: u32,
    pub item: String,
    pub count: u32,
    /// Feet position in pixels.
    pub x: f32,
    pub y: f32,
}

/// The player character's sex (for dialogs, conditions and scripts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sex {
    #[default]
    Male,
    Female,
}

impl Sex {
    pub const ALL: [Sex; 2] = [Sex::Male, Sex::Female];
    pub fn label(self) -> &'static str {
        match self {
            Sex::Male => "Male",
            Sex::Female => "Female",
        }
    }
    /// "male" / "female" (scripts).
    pub fn key(self) -> &'static str {
        match self {
            Sex::Male => "male",
            Sex::Female => "female",
        }
    }
}

impl WorldState {
    /// In-game hours since the game began (days and the hour of the day).
    pub fn total_hours(&self) -> f64 {
        self.day as f64 * 24.0 + self.hour as f64
    }

    /// Moves the clock forward by `hours`, counting the days that pass.
    pub fn advance_hours(&mut self, hours: f32) {
        let t = self.total_hours() + hours.max(0.0) as f64;
        self.day = (t / 24.0).floor() as u32;
        self.hour = (t - self.day as f64 * 24.0) as f32;
    }

    /// The placed character `uid` of `map` follows the player.
    pub fn is_companion(&self, map: &str, uid: u32) -> bool {
        self.companions.iter().any(|c| c.uid == uid && c.map == map)
    }

    /// The spells the character placed as `uid` on `map` has learned.
    pub fn npc_spells(&self, map: &str, uid: u32) -> &[String] {
        self.npc_spells.get(&format!("{map}/{uid}")).map_or(&[], Vec::as_slice)
    }

    /// Teaches it `spell`; false when it knew it already.
    pub fn teach_npc(&mut self, map: &str, uid: u32, spell: &str) -> bool {
        let known = self.npc_spells.entry(format!("{map}/{uid}")).or_default();
        if known.iter().any(|s| s == spell) {
            return false;
        }
        known.push(spell.to_string());
        true
    }

    /// Adds a line to the dialog log (dropping the oldest beyond [`DIALOG_LOG_MAX`]).
    pub fn log_dialog(&mut self, speaker: &str, text: &str, answer: bool) {
        if text.trim().is_empty() {
            return;
        }
        self.dialog_log.push(LogLine { speaker: speaker.to_string(), text: text.to_string(), answer });
        if self.dialog_log.len() > DIALOG_LOG_MAX {
            let extra = self.dialog_log.len() - DIALOG_LOG_MAX;
            self.dialog_log.drain(..extra);
        }
    }

    /// How many characters of a definition have been defeated.
    pub fn defeated(&self, character: &str) -> u32 {
        self.bestiary.get(character).copied().unwrap_or(0)
    }

    /// The player's level (at least 1).
    pub fn player_level(&self) -> u32 {
        self.level.max(1)
    }

    pub fn skill_level(&self, id: &str) -> u32 {
        self.skills.get(id).copied().unwrap_or(0)
    }

    /// Fills in text placeholders (dialogs, choices, quest texts):
    /// `{player}` = the player's name, `{his|her}` = the first word for a male player, the
    /// second for a female one, `{name}` = the value of variable `name`, `{key:confirm}` = the key
    /// or gamepad button of an action (any action id; `interact` = confirm). Anything else in
    /// braces is kept as it is.
    /// The text in the current language (`lang::tr`, looked up before the placeholders are
    /// filled in) with `{player}`, `{sir|madam}` and `{var}` filled in.
    pub fn expand_text(&self, text: &str) -> String {
        let translated = crate::lang::tr(text);
        let text = translated.as_str();
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            let Some(j) = after.find('}') else {
                out.push_str(&rest[i..]);
                return out;
            };
            let inner = &after[..j];
            let ident = !inner.is_empty() && inner.chars().all(|c| c.is_alphanumeric() || c == '_');
            if inner == "player" {
                out.push_str(&self.player_name);
            } else if let Some(key) = inner.strip_prefix("key:").and_then(crate::runtime::glyphs::key_text) {
                // {key:confirm}: the key ("E") or, after gamepad use, the button ("A") for it.
                out.push_str(&key);
            } else if inner.contains('|') && !inner.contains('{') {
                let mut parts = inner.splitn(2, '|');
                let (m, f) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                out.push_str(if self.player_sex == Sex::Female { f } else { m });
            } else if ident {
                out.push_str(&self.var(inner).to_string());
            } else {
                out.push('{');
                rest = after;
                continue;
            }
            rest = &after[j + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Puts dropped items on a map: onto a pile of the same item within `merge` pixels, else
    /// as a new pile. Returns the pile's uid and whether it is new.
    pub fn add_dropped(&mut self, map: &str, item: &str, count: u32, x: f32, y: f32, merge: f32) -> (u32, bool) {
        let piles = self.dropped.entry(map.to_string()).or_default();
        if let Some(p) = piles.iter_mut().find(|p| p.item == item && (p.x - x).hypot(p.y - y) <= merge) {
            p.count += count;
            return (p.uid, false);
        }
        let uid = DROPPED_UID_BASE + self.next_drop;
        self.next_drop += 1;
        piles.push(DroppedItem { uid, item: item.to_string(), count, x, y });
        (uid, true)
    }

    /// Saves from 0.0.5 numbered dropped piles from 0x4000_0000, the range of spawned
    /// characters: give such piles uids in the current range.
    pub fn renumber_old_drops(&mut self) {
        let mut next = self.next_drop;
        for p in self.dropped.values_mut().flatten() {
            if p.uid < DROPPED_UID_BASE {
                p.uid = DROPPED_UID_BASE + next;
                next += 1;
            }
        }
        self.next_drop = next;
    }

    /// Updates the size of a dropped pile (0 removes it).
    pub fn set_dropped_count(&mut self, map: &str, uid: u32, count: u32) {
        if let Some(piles) = self.dropped.get_mut(map) {
            if count == 0 {
                piles.retain(|p| p.uid != uid);
            } else if let Some(p) = piles.iter_mut().find(|p| p.uid == uid) {
                p.count = count;
            }
            if piles.is_empty() {
                self.dropped.remove(map);
            }
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
    pub fn set_flag(&mut self, name: &str, value: bool) {
        if value {
            self.flags.insert(name.to_string());
        } else {
            self.flags.remove(name);
        }
    }
    pub fn var(&self, name: &str) -> i64 {
        self.vars.get(name).copied().unwrap_or(0)
    }
    pub fn set_var(&mut self, name: &str, v: i64) {
        self.vars.insert(name.to_string(), v);
    }
    /// Items in the inventory plus equipped ones.
    pub fn item_count(&self, item: &str) -> u32 {
        self.inventory.count(item) + self.equipment.count(item)
    }
    /// Adds items to the player's inventory; returns how many did not fit.
    pub fn give_item(&mut self, item: &str, count: u32) -> u32 {
        self.inventory.add(item, count)
    }
    /// Removes up to `count` items; returns how many were removed.
    /// Takes from the inventory first, then from equipped items.
    pub fn take_item(&mut self, item: &str, count: u32) -> u32 {
        let mut taken = self.inventory.remove(item, count);
        for slot in EquipSlot::ALL {
            if taken == count {
                break;
            }
            let s = self.equipment.slot_mut(slot);
            if let Some(st) = s.as_mut().filter(|st| st.item == item) {
                let n = (count - taken).min(st.count);
                st.count -= n;
                taken += n;
                if st.count == 0 {
                    *s = None;
                }
            }
        }
        taken
    }

    pub fn entity_state(&self, map: &str, uid: u32) -> Option<&EntityState> {
        self.entities.get(map)?.get(&uid)
    }
    pub fn set_entity_state(&mut self, map: &str, uid: u32, st: EntityState) {
        self.entities.entry(map.to_string()).or_default().insert(uid, st);
    }

    pub fn to_ron(&self) -> anyhow::Result<String> {
        crate::project::ron_text(self)
    }
    pub fn from_ron(s: &str) -> anyhow::Result<Self> {
        let mut w: WorldState = ron::from_str(s)?;
        let extra = w.dialog_log.len().saturating_sub(DIALOG_LOG_MAX);
        w.dialog_log.drain(..extra);
        Ok(w)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn dropped_piles() {
        let mut w = WorldState::default();
        let (a, new_a) = w.add_dropped("village", "apple", 2, 100.0, 100.0, 16.0);
        // Same item close by joins the pile; another item or far away makes a new one.
        assert_eq!(w.add_dropped("village", "apple", 3, 105.0, 100.0, 16.0), (a, false));
        let (b, new_b) = w.add_dropped("village", "sword", 1, 100.0, 100.0, 16.0);
        assert!(new_a && new_b && a != b && a >= DROPPED_UID_BASE);
        assert_eq!(w.dropped["village"][0].count, 5);
        w.set_dropped_count("village", a, 1);
        assert_eq!(w.dropped["village"][0].count, 1);
        w.set_dropped_count("village", a, 0);
        w.set_dropped_count("village", b, 0);
        assert!(!w.dropped.contains_key("village"));
        // Piles of old saves move out of the spawned characters' uid range.
        let mut old = WorldState::default();
        old.dropped.insert("m".into(), vec![DroppedItem { uid: 0x4000_0000, item: "coin".into(), count: 1, x: 0.0, y: 0.0 }]);
        old.renumber_old_drops();
        assert_eq!((old.dropped["m"][0].uid, old.next_drop), (DROPPED_UID_BASE, 1));
        // Piles survive a save / load round trip.
        w.add_dropped("forest", "coin", 7, 1.0, 2.0, 16.0);
        let back: WorldState = ron::from_str(&ron::to_string(&w).unwrap()).unwrap();
        assert_eq!(back.dropped["forest"][0].count, 7);
        assert_eq!(back.next_drop, w.next_drop);
    }

    use super::*;

    #[test]
    fn equipment_counts_and_takes() {
        let mut w = WorldState::default();
        w.give_item("sword", 1);
        w.equipment.weapon = Some(ItemStack::new("sword", 1));
        assert_eq!(w.item_count("sword"), 2);
        assert_eq!(w.take_item("sword", 2), 2);
        assert!(w.equipment.weapon.is_none() && w.inventory.is_empty());
        // Old saves without `equipment` still load; `equipped` is kept for migration.
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {"bow": 1}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down, equipped: "bow")"#).unwrap();
        assert_eq!((old.equipped.as_str(), old.equipment.weapon.is_none()), ("bow", true));
        assert!(old.quickbar.is_empty());
        // Saves from before experience and levels start at level 1 without XP.
        assert_eq!((old.xp, old.player_level()), (0, 1));
        assert!(old.effects.is_empty() && old.bestiary.is_empty() && old.dialog_log.is_empty());
        let mut w = WorldState { xp: 250, level: 3, ..Default::default() };
        crate::effects::apply(&mut w.effects, &crate::effects::EffectSpec::new(crate::effects::EffectKind::Haste, 30.0, 12.5));
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!((back.xp, back.player_level()), (250, 3));
        assert_eq!(back.effects, w.effects);
        let w = WorldState { bestiary: [("slime".to_string(), 4)].into(), ..Default::default() };
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!((back.defeated("slime"), back.defeated("goblin")), (4, 0));
        let w = WorldState { quickbar: vec!["potion".into(), String::new()], ..Default::default() };
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.quickbar, w.quickbar);
    }

    #[test]
    fn weather_overrides_are_saved() {
        use crate::map::{Weather, WeatherKind};
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down)"#).unwrap();
        assert!(old.weather.is_empty(), "saves from before weather load");
        let mut w = WorldState::default();
        w.weather.insert("forest".into(), Weather::new(WeatherKind::Rain, 0.8));
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.weather.get("forest"), Some(&Weather::new(WeatherKind::Rain, 0.8)));
    }

    #[test]
    fn dialog_log_keeps_the_newest_lines() {
        let mut w = WorldState::default();
        w.log_dialog("Bob", "  ", false);
        assert!(w.dialog_log.is_empty(), "empty text is not logged");
        for i in 0..DIALOG_LOG_MAX + 5 {
            w.log_dialog("Bob", &format!("line {i}"), false);
        }
        w.log_dialog("Alex", "Yes", true);
        assert_eq!(w.dialog_log.len(), DIALOG_LOG_MAX);
        assert_eq!(w.dialog_log[0].text, "line 6");
        assert_eq!(w.dialog_log.last().unwrap(), &LogLine { speaker: "Alex".into(), text: "Yes".into(), answer: true });
        // Saved and loaded; an over-long log (older versions, edited saves) is trimmed on load.
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.dialog_log, w.dialog_log);
        let mut long = w.clone();
        long.dialog_log.extend(w.dialog_log.iter().take(50).cloned());
        assert_eq!(WorldState::from_ron(&long.to_ron().unwrap()).unwrap().dialog_log.len(), DIALOG_LOG_MAX);
    }

    #[test]
    fn inventory_and_save_roundtrip() {
        let mut w = WorldState::default();
        w.give_item("coin", 3);
        assert_eq!(w.take_item("coin", 5), 3);
        assert_eq!(w.item_count("coin"), 0);
        assert!(w.inventory.is_empty());
        w.set_flag("a", true);
        w.set_var("x", -4);
        w.set_entity_state(
            "village",
            7,
            EntityState { active: false, frame: 1, x: 1.0, y: 2.0, facing: Facing::Up, count: None, unlocked: true, state: Some("open".into()), patrol: None, schedule_off: false, changed_at: Some(30.0), planted: None },
        );
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert!(back.flag("a"));
        assert_eq!(back.var("x"), -4);
        assert_eq!(back.entity_state("village", 7).unwrap().frame, 1);
    }

    #[test]
    fn entity_states_of_old_saves_follow_their_schedule() {
        let old: EntityState = ron::from_str("(active: true, frame: 0, x: 1.0, y: 2.0, facing: Down)").unwrap();
        assert!(!old.schedule_off);
        let off = EntityState { schedule_off: true, ..old.clone() };
        let text = ron::to_string(&off).unwrap();
        assert_eq!(ron::from_str::<EntityState>(&text).unwrap(), off);
        assert!(!ron::to_string(&old).unwrap().contains("schedule_off"), "only written when stopped");
    }

    #[test]
    fn companions_in_saves() {
        // A save without companions (all older ones) loads with none.
        let old = WorldState::default().to_ron().unwrap();
        assert!(!old.contains("companions"));
        assert!(WorldState::from_ron(&old).unwrap().companions.is_empty());
        let mut w = WorldState::default();
        w.companions.push(Companion { map: "village".into(), uid: 22, name: "guard".into(), hp: Some(5), down: None, order: Order::Defend, post: Some([100.0, 200.0]), mana: None });
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.companions, w.companions);
        assert!(back.is_companion("village", 22) && !back.is_companion("forest", 22));
        // Companions saved before they could be knocked down are on their feet.
        let c: Companion = ron::from_str(r#"(map: "village", uid: 22, name: "guard", hp: Some(5))"#).unwrap();
        assert_eq!(c.down, None);
        let mut down = c.clone();
        down.down = Some([272.0, 590.0]);
        assert_eq!(ron::from_str::<Companion>(&ron::to_string(&down).unwrap()).unwrap(), down);
        assert!(!ron::to_string(&c).unwrap().contains("down"));
        // ... and follow with the Attack order.
        assert_eq!((c.order, c.post), (Order::Attack, None));
        assert!(!ron::to_string(&c).unwrap().contains("order"));
    }

    #[test]
    fn tool_slot_in_saves() {
        // Saves from before the Tool slot have no "tool" field.
        let old: Equipment = ron::from_str("(head: None, body: None, weapon: Some((item: \"sword\", count: 1)), offhand: None)").unwrap();
        assert_eq!(old.get(EquipSlot::Tool), None);
        let mut eq = old.clone();
        *eq.slot_mut(EquipSlot::Tool) = Some(ItemStack::new("fishing_rod", 1));
        let back: Equipment = ron::from_str(&ron::to_string(&eq).unwrap()).unwrap();
        assert_eq!(back.item(EquipSlot::Tool), Some("fishing_rod"));
        assert_eq!(back.count("fishing_rod"), 1, "tools count as owned");
        assert!(!ron::to_string(&old).unwrap().contains("tool"), "no tool, nothing written");
        assert_eq!(EquipSlot::parse("tool"), Some(EquipSlot::Tool));
    }
}

#[cfg(test)]
mod day_tests {
    use super::*;

    #[test]
    fn days_pass_with_the_clock() {
        // Saves from before days: day 0, entity states without a change time.
        let mut w = WorldState { hour: 22.5, ..Default::default() };
        w.day = 7;
        let ron = w.to_ron().unwrap().replace("    day: 7,\n", "");
        assert!(!ron.contains("day:"), "{ron}");
        let old = WorldState::from_ron(&ron).unwrap();
        assert_eq!((old.day, old.hour), (0, 22.5));
        let st: EntityState = ron::from_str("(active: true, frame: 0, x: 1.0, y: 2.0, facing: Down)").unwrap();
        assert_eq!(st.changed_at, None);
        let mut w = old;
        w.advance_hours(3.0);
        assert_eq!((w.day, w.hour), (1, 1.5));
        w.advance_hours(72.0);
        assert_eq!((w.day, w.hour), (4, 1.5));
        assert_eq!(w.total_hours(), 97.5);
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.day, 4);
    }
}
