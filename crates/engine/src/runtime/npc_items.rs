//! What NPCs wear, hold and carry: combat values from their equipment, dropping it all when
//! defeated, and changes by actions / scripts (saved per NPC in `WorldState::npc_items`).

use macroquad::prelude::*;

use super::{Game, RtEntity};
use crate::defs::{Database, EquipSlot};
use crate::inventory::ItemStack;
use crate::state::NpcItems;

/// Weapon and defense of an NPC: an equipped weapon replaces the definition's, worn armor and
/// clothing add to its defense.
pub(super) fn apply_gear(db: &Database, e: &mut RtEntity) {
    let Some(d) = db.character(&e.def) else { return };
    e.weapon = e.equipment.item(EquipSlot::Weapon).and_then(|id| db.item(id)?.weapon.clone()).or_else(|| d.weapon.clone());
    let armor: i32 = [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Offhand]
        .into_iter()
        .filter_map(|s| db.item(e.equipment.item(s)?)?.armor.as_ref().map(|a| a.defense))
        .sum();
    e.defense = d.defense + armor;
}

/// Adds `count` of `item` to a list of stacks.
fn add(list: &mut Vec<ItemStack>, item: &str, count: u32) {
    match list.iter_mut().find(|s| s.item == item) {
        Some(s) => s.count += count,
        None => list.push(ItemStack::new(item, count)),
    }
}

/// Removes up to `count`; returns how many were removed.
fn remove(list: &mut Vec<ItemStack>, item: &str, count: u32) -> u32 {
    let Some(k) = list.iter().position(|s| s.item == item) else { return 0 };
    let n = list[k].count.min(count);
    list[k].count -= n;
    if list[k].count == 0 {
        list.remove(k);
    }
    n
}

impl Game {
    /// Saves NPC `i`'s items (spawned characters are not saved individually).
    pub(super) fn persist_npc_items(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        if e.spawned_by.is_some() {
            return;
        }
        let items = NpcItems { equipment: e.equipment.clone(), inventory: e.inventory.clone() };
        // A companion's items are kept where it is placed.
        let (map, uid) = match &e.companion {
            Some(l) => (l.map.clone(), l.uid),
            None => (self.loaded.map.name.clone(), e.uid),
        };
        self.world.npc_items.entry(map).or_default().insert(uid, items);
    }

    /// Puts `item` into its slot on NPC `i`; what it wore there goes to its carried items.
    pub(super) fn npc_equip(&mut self, i: usize, item: &str) -> bool {
        let Some(slot) = self.project.db.item(item).and_then(|d| d.equip_slot()) else {
            eprintln!("[game] equip: '{item}' can't be worn or held");
            return false;
        };
        let e = &mut self.loaded.entities[i];
        // Take it from what the NPC carries if it has one.
        remove(&mut e.inventory, item, 1);
        if let Some(old) = e.equipment.slot_mut(slot).replace(ItemStack::new(item, 1)) {
            add(&mut e.inventory, &old.item, old.count);
        }
        apply_gear(&self.project.db, &mut self.loaded.entities[i]);
        self.persist_npc_items(i);
        true
    }

    /// Takes off what NPC `i` wears in `slot` (into its carried items).
    pub(super) fn npc_unequip(&mut self, i: usize, slot: EquipSlot) {
        let e = &mut self.loaded.entities[i];
        if let Some(old) = e.equipment.slot_mut(slot).take() {
            add(&mut e.inventory, &old.item, old.count);
        }
        apply_gear(&self.project.db, &mut self.loaded.entities[i]);
        self.persist_npc_items(i);
    }

    /// Adds to (`count` > 0) or takes from NPC `i`'s carried items.
    pub(super) fn npc_give(&mut self, i: usize, item: &str, count: i64) {
        let e = &mut self.loaded.entities[i];
        if count >= 0 {
            add(&mut e.inventory, item, count as u32);
        } else {
            remove(&mut e.inventory, item, (-count) as u32);
        }
        self.persist_npc_items(i);
        // A scroll or spell book given to a companion teaches it the spell.
        if count > 0 {
            self.npc_received(i, item);
        }
    }

    /// A defeated NPC drops everything it wore and carried (next to its loot).
    pub(super) fn drop_npc_items(&mut self, i: usize) {
        let e = &mut self.loaded.entities[i];
        let feet = e.body.pos;
        let mut stacks: Vec<ItemStack> = e.equipment.stacks().cloned().collect();
        stacks.append(&mut e.inventory);
        e.equipment = Default::default();
        if stacks.is_empty() {
            return;
        }
        self.persist_npc_items(i);
        let u = self.u();
        for (k, s) in stacks.into_iter().enumerate() {
            // Spread out in a ring outside the loot table's spots.
            let a = k as f32 * 2.4 + 0.6;
            self.place_drop(&s.item, s.count, feet + vec2(a.cos() * 14.0, a.sin() * 8.0 + 4.0) * u);
        }
    }

    /// NPC items for `Game::status()`: "name:clothing,weapon|carried@body/hair".
    pub(super) fn npc_items_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter(|e| e.active && (!e.equipment.is_empty() || !e.inventory.is_empty()))
            .map(|e| {
                let worn: Vec<&str> = e.equipment.stacks().map(|s| s.item.as_str()).collect();
                let carried: Vec<String> = e.inventory.iter().map(|s| format!("{}x{}", s.item, s.count)).collect();
                let name = if e.name.is_empty() { &e.def } else { &e.name };
                format!("{name}:{}|{}@{}/{}", worn.join(","), carried.join(","), e.appearance.body, e.appearance.hair)
            })
            .collect()
    }
}
