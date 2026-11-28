//! Slot-based inventories, used by the player and by containers (chests, barrels, ...).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Stack limit for items without an explicit `max_stack`.
pub const DEFAULT_STACK: u32 = 99;
/// Player inventory size when the project doesn't set one.
pub const DEFAULT_CAPACITY: usize = 24;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ItemStack {
    pub item: String,
    pub count: u32,
    /// A magic item's charges used (0 = all left): a stack with used charges never merges with
    /// another one.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spent: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl ItemStack {
    pub fn new(item: &str, count: u32) -> Self {
        ItemStack { item: item.to_string(), count, spent: 0 }
    }
}

/// Fixed number of slots, each empty or holding one stack.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(from = "InventoryRepr")]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    /// Max stack size per item id (not saved; installed from the item definitions).
    #[serde(skip)]
    limits: Arc<HashMap<String, u32>>,
}

/// Accepts the current format and the old `{ item: count }` map from earlier saves.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
enum InventoryRepr {
    Slots { slots: Vec<Option<ItemStack>> },
    Legacy(BTreeMap<String, u32>),
}

impl From<InventoryRepr> for Inventory {
    fn from(r: InventoryRepr) -> Self {
        match r {
            InventoryRepr::Slots { slots } => Inventory { slots, limits: Arc::default() },
            InventoryRepr::Legacy(map) => {
                let slots = map.into_iter().filter(|(_, n)| *n > 0).map(|(item, count)| Some(ItemStack::new(&item, count))).collect();
                let mut inv = Inventory { slots, limits: Arc::default() };
                inv.resize(DEFAULT_CAPACITY);
                inv
            }
        }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory::new(DEFAULT_CAPACITY)
    }
}

impl PartialEq for Inventory {
    fn eq(&self, other: &Self) -> bool {
        self.slots == other.slots
    }
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Inventory { slots: vec![None; capacity], limits: Arc::default() }
    }

    /// Builds a container inventory with `stacks` placed in order.
    pub fn with_stacks(capacity: usize, stacks: &[ItemStack], limits: Arc<HashMap<String, u32>>) -> Self {
        let mut inv = Inventory { slots: vec![None; capacity.max(1)], limits };
        for s in stacks.iter().filter(|s| s.count > 0 && !s.item.is_empty()) {
            inv.add_stack(s.clone());
        }
        inv
    }

    pub fn set_limits(&mut self, limits: Arc<HashMap<String, u32>>) {
        self.limits = limits;
    }

    pub fn limits(&self) -> Arc<HashMap<String, u32>> {
        self.limits.clone()
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Grows to `capacity`; shrinking only drops trailing empty slots (items are never lost).
    pub fn resize(&mut self, capacity: usize) {
        if capacity >= self.slots.len() {
            self.slots.resize(capacity, None);
        } else {
            while self.slots.len() > capacity && self.slots.last().is_some_and(|s| s.is_none()) {
                self.slots.pop();
            }
        }
    }

    /// A copy with `capacity` slots whose stacks beyond it moved into free slots before it (a
    /// backpack taken off); None when they don't fit. Growing just adds empty slots.
    pub fn packed_to(&self, capacity: usize) -> Option<Inventory> {
        let mut packed = self.clone();
        for k in capacity..packed.slots.len() {
            if let Some(s) = packed.slots[k].take() {
                let free = packed.slots[..capacity].iter().position(|s| s.is_none())?;
                packed.slots[free] = Some(s);
            }
        }
        packed.slots.resize(capacity, None);
        Some(packed)
    }

    pub fn max_stack(&self, item: &str) -> u32 {
        self.limits.get(item).copied().unwrap_or(DEFAULT_STACK).max(1)
    }

    pub fn count(&self, item: &str) -> u32 {
        self.slots.iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
    }

    pub fn used_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Distinct item ids in slot order.
    pub fn item_ids(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in self.slots.iter().flatten() {
            if !out.contains(&s.item) {
                out.push(s.item.clone());
            }
        }
        out
    }

    /// Item totals (for logs and debugging).
    pub fn summary(&self) -> BTreeMap<String, u32> {
        let mut m = BTreeMap::new();
        for s in self.slots.iter().flatten() {
            *m.entry(s.item.clone()).or_insert(0) += s.count;
        }
        m
    }

    /// How many of `item` would still fit.
    pub fn space_for(&self, item: &str) -> u32 {
        let max = self.max_stack(item);
        self.slots
            .iter()
            .map(|s| match s {
                None => max,
                Some(s) if s.item == item => max.saturating_sub(s.count),
                Some(_) => 0,
            })
            .sum()
    }

    /// Adds items, topping up existing stacks first. Returns how many did not fit.
    pub fn add(&mut self, item: &str, mut count: u32) -> u32 {
        let max = self.max_stack(item);
        for s in self.slots.iter_mut().flatten() {
            if count == 0 {
                break;
            }
            if s.item == item && s.count < max && s.spent == 0 {
                let n = count.min(max - s.count);
                s.count += n;
                count -= n;
            }
        }
        for slot in self.slots.iter_mut() {
            if count == 0 {
                break;
            }
            if slot.is_none() {
                let n = count.min(max);
                *slot = Some(ItemStack::new(item, n));
                count -= n;
            }
        }
        count
    }

    /// Adds a whole stack: one with used charges goes into a free slot as it is (never merged;
    /// all or nothing), others like [`Inventory::add`]. Returns how many did not fit.
    pub fn add_stack(&mut self, stack: ItemStack) -> u32 {
        if stack.spent == 0 {
            return self.add(&stack.item, stack.count);
        }
        match self.slots.iter().position(|s| s.is_none()) {
            Some(k) => {
                self.slots[k] = Some(stack);
                0
            }
            None => stack.count,
        }
    }

    /// Removes up to `count` items (from the last stacks first). Returns how many were removed.
    pub fn remove(&mut self, item: &str, count: u32) -> u32 {
        let mut left = count;
        for slot in self.slots.iter_mut().rev() {
            if left == 0 {
                break;
            }
            if let Some(s) = slot {
                if s.item == item {
                    let n = left.min(s.count);
                    s.count -= n;
                    left -= n;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        count - left
    }

    /// Takes up to `count` items out of one slot (`None` = the whole stack).
    pub fn take_from_slot(&mut self, i: usize, count: Option<u32>) -> Option<ItemStack> {
        let slot = self.slots.get_mut(i)?;
        let s = slot.as_mut()?;
        let n = count.unwrap_or(s.count).min(s.count);
        if n == 0 {
            return None;
        }
        s.count -= n;
        let mut out = ItemStack::new(&s.item, n);
        if s.count == 0 {
            // (The whole stack: its used charges go with it.)
            out.spent = s.spent;
            *slot = None;
        }
        Some(out)
    }

    /// Moves a slot onto another slot of the same inventory: merges equal items
    /// (up to the stack limit) or swaps.
    pub fn move_slot(&mut self, from: usize, to: usize) {
        if from == to || from >= self.slots.len() || to >= self.slots.len() {
            return;
        }
        let same_item = matches!((&self.slots[from], &self.slots[to]), (Some(a), Some(b)) if a.item == b.item && a.spent == 0 && b.spent == 0);
        if same_item {
            let max = self.max_stack(&self.slots[to].as_ref().unwrap().item);
            let room = max.saturating_sub(self.slots[to].as_ref().unwrap().count);
            let n = room.min(self.slots[from].as_ref().unwrap().count);
            self.slots[to].as_mut().unwrap().count += n;
            let src = self.slots[from].as_mut().unwrap();
            src.count -= n;
            if src.count == 0 {
                self.slots[from] = None;
            }
        } else {
            self.slots.swap(from, to);
        }
    }

    /// Moves up to `count` items from `src[i]` into `dst` (anywhere they fit).
    /// Returns how many moved; the rest stays in the source slot.
    pub fn transfer(src: &mut Inventory, i: usize, dst: &mut Inventory, count: Option<u32>) -> u32 {
        let Some(stack) = src.take_from_slot(i, count) else { return 0 };
        let left = dst.add_stack(stack.clone());
        if left > 0 {
            // Put back what didn't fit, into the same slot.
            match &mut src.slots[i] {
                Some(s) => s.count += left,
                slot @ None => *slot = Some(ItemStack { count: left, ..stack.clone() }),
            }
        }
        stack.count - left
    }

    /// Moves a stack from `src[i]` onto a specific slot `dst[j]` (merge if same item and
    /// room, place if empty, otherwise swap when the swapped stack fits back).
    pub fn transfer_to_slot(src: &mut Inventory, i: usize, dst: &mut Inventory, j: usize) -> bool {
        if i >= src.slots.len() || j >= dst.slots.len() || src.slots[i].is_none() {
            return false;
        }
        match (&src.slots[i], &dst.slots[j]) {
            (Some(a), Some(b)) if a.item == b.item && a.spent == 0 && b.spent == 0 => {
                let room = dst.max_stack(&b.item).saturating_sub(b.count);
                let n = room.min(a.count);
                dst.slots[j].as_mut().unwrap().count += n;
                let s = src.slots[i].as_mut().unwrap();
                s.count -= n;
                if s.count == 0 {
                    src.slots[i] = None;
                }
                n > 0
            }
            (Some(_), None) => {
                dst.slots[j] = src.slots[i].take();
                true
            }
            _ => {
                std::mem::swap(&mut src.slots[i], &mut dst.slots[j]);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacks_with_used_charges_keep_them_and_never_merge() {
        let mut inv = Inventory::new(4);
        inv.add("wand", 1);
        assert_eq!(inv.add_stack(ItemStack { spent: 3, ..ItemStack::new("wand", 1) }), 0);
        assert_eq!(inv.slots.iter().flatten().map(|s| s.spent).collect::<Vec<_>>(), vec![0, 3]);
        // Moving it keeps the charges; it doesn't join the full one.
        inv.move_slot(1, 0);
        assert_eq!(inv.slots[0].as_ref().map(|s| (s.count, s.spent)), Some((1, 3)));
        let mut other = Inventory::new(2);
        assert_eq!(Inventory::transfer(&mut inv, 0, &mut other, None), 1);
        assert_eq!(other.slots[0].as_ref().map(|s| s.spent), Some(3));
        assert_eq!(other.take_from_slot(0, None).map(|s| s.spent), Some(3));
        // Saves of 0.30 and older: no charges used.
        let s: ItemStack = ron::from_str("(item: \"potion\", count: 2)").unwrap();
        assert_eq!(s, ItemStack::new("potion", 2));
        assert_eq!(ron::to_string(&s).unwrap(), "(item:\"potion\",count:2)");
    }

    fn limits(pairs: &[(&str, u32)]) -> Arc<HashMap<String, u32>> {
        Arc::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
    }

    #[test]
    fn stacking_and_capacity() {
        let mut inv = Inventory::new(3);
        inv.set_limits(limits(&[("arrow", 10), ("bow", 1)]));
        assert_eq!(inv.add("arrow", 15), 0); // 10 + 5
        assert_eq!(inv.add("bow", 2), 1); // one slot left
        assert_eq!(inv.used_slots(), 3);
        assert_eq!(inv.space_for("arrow"), 5);
        assert_eq!(inv.add("arrow", 8), 3); // tops up the 5-stack, 3 don't fit
        assert_eq!(inv.count("arrow"), 20);
        assert_eq!(inv.remove("arrow", 12), 12);
        assert_eq!(inv.count("arrow"), 8);
        assert_eq!(inv.remove("potion", 1), 0);
        assert_eq!(inv.item_ids(), vec!["arrow".to_string(), "bow".to_string()]);
    }

    #[test]
    fn transfers_between_inventories() {
        let l = limits(&[("coin", 50)]);
        let mut chest = Inventory::with_stacks(4, &[ItemStack::new("coin", 70), ItemStack::new("key", 1)], l.clone());
        assert_eq!(chest.slots[0], Some(ItemStack::new("coin", 50)));
        assert_eq!(chest.slots[1], Some(ItemStack::new("coin", 20)));
        assert_eq!(chest.slots[2], Some(ItemStack::new("key", 1)));
        let mut player = Inventory::new(1);
        player.set_limits(l);
        // Only one coin stack fits.
        assert_eq!(Inventory::transfer(&mut chest, 0, &mut player, None), 50);
        assert_eq!(Inventory::transfer(&mut chest, 1, &mut player, None), 0);
        assert_eq!(chest.slots[1], Some(ItemStack::new("coin", 20)));
        // The key doesn't fit either and stays in the chest.
        assert_eq!(Inventory::transfer(&mut chest, 2, &mut player, Some(1)), 0);
        assert_eq!(chest.count("key"), 1);
        // Put 10 coins back, then take one.
        assert_eq!(Inventory::transfer(&mut player, 0, &mut chest, Some(10)), 10);
        assert_eq!(chest.count("coin"), 30);
        assert_eq!(Inventory::transfer(&mut chest, 1, &mut player, Some(1)), 1);
        assert_eq!(player.count("coin"), 41);
    }

    #[test]
    fn moving_slots_merges_or_swaps() {
        let mut inv = Inventory::new(4);
        inv.set_limits(limits(&[("a", 10)]));
        inv.slots[0] = Some(ItemStack::new("a", 7));
        inv.slots[1] = Some(ItemStack::new("a", 6));
        inv.slots[2] = Some(ItemStack::new("b", 1));
        inv.move_slot(0, 1); // merge up to 10
        assert_eq!((inv.slots[0].clone(), inv.slots[1].clone()), (Some(ItemStack::new("a", 3)), Some(ItemStack::new("a", 10))));
        inv.move_slot(2, 0); // swap
        assert_eq!(inv.slots[0], Some(ItemStack::new("b", 1)));
        assert_eq!(inv.slots[2], Some(ItemStack::new("a", 3)));
        let mut other = Inventory::new(2);
        assert!(Inventory::transfer_to_slot(&mut inv, 0, &mut other, 1));
        assert_eq!(other.slots[1], Some(ItemStack::new("b", 1)));
        assert!(inv.slots[0].is_none());
    }

    #[test]
    fn reads_legacy_and_current_saves() {
        let legacy: Inventory = ron::from_str(r#"{"key": 1, "coin": 30}"#).unwrap();
        assert_eq!(legacy.count("coin"), 30);
        assert_eq!(legacy.capacity(), DEFAULT_CAPACITY);
        let mut inv = Inventory::new(3);
        inv.add("bow", 1);
        let back: Inventory = ron::from_str(&ron::to_string(&inv).unwrap()).unwrap();
        assert_eq!(back, inv);
        assert_eq!(back.capacity(), 3);
    }

    #[test]
    fn resize_never_drops_items() {
        let mut inv = Inventory::new(4);
        inv.slots[2] = Some(ItemStack::new("x", 1));
        inv.resize(1);
        assert_eq!(inv.capacity(), 3);
        inv.resize(6);
        assert_eq!(inv.capacity(), 6);
    }

    #[test]
    fn a_backpacks_slots_pack_into_free_ones_or_stay() {
        // 4 slots + a backpack's 2: the extra slots hold "b" and "c".
        let mut inv = Inventory::new(6);
        for (k, id) in ["a", "x", "b", "c"].into_iter().enumerate() {
            inv.slots[[0, 1, 4, 5][k]] = Some(ItemStack::new(id, 1));
        }
        // Taken off: they move into the free slots 2 and 3.
        let packed = inv.packed_to(4).expect("room for both");
        assert_eq!(packed.capacity(), 4);
        assert_eq!(packed.slots.iter().flatten().map(|s| s.item.as_str()).collect::<Vec<_>>(), ["a", "x", "b", "c"]);
        // With one free slot only, nothing changes.
        inv.slots[2] = Some(ItemStack::new("y", 1));
        assert!(inv.packed_to(4).is_none());
        // Growing adds empty slots.
        assert_eq!(inv.packed_to(9).unwrap().capacity(), 9);
    }
}
