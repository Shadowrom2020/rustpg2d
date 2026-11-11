//! Equipment (head, body, weapon, left hand): equipping from the inventory, two-handed weapons,
//! derived stats, armor overlays.

use macroquad::prelude::*;

use super::Game;
use crate::defs::{EquipSlot, Weapon};
use crate::inventory::ItemStack;

/// Player stats shown on the character sheet.
pub(super) struct Stats {
    pub attack: i32,
    pub defense: i32,
    pub speed: f32,
    pub cooldown: f32,
    /// Melee reach or ranged range, in tiles.
    pub reach: f32,
    pub ranged: bool,
}

/// Reach of bare-handed attacks (tiles).
const UNARMED_REACH: f32 = 0.8;
const UNARMED_COOLDOWN: f32 = 0.35;

impl Game {
    /// The equipped weapon's definition.
    pub(super) fn current_weapon(&self) -> Option<Weapon> {
        let id = self.world.equipment.item(EquipSlot::Weapon)?;
        self.project.db.item(id).and_then(|d| d.weapon.clone())
    }

    /// The weapon used for attacks: the equipped one, or bare hands (player damage).
    pub(super) fn attack_weapon(&self) -> Weapon {
        let mut w = self.current_weapon().unwrap_or(Weapon {
            reach: UNARMED_REACH,
            ..Weapon::melee(self.combat.damage, UNARMED_COOLDOWN)
        });
        // Combat skills raise the damage: one for melee (and bare hands), one for ranged weapons.
        let f = if w.ranged.is_some() { self.ranged_damage_factor() } else { self.melee_damage_factor() };
        w.damage = crate::effects::scale_damage(&self.world.effects, (w.damage as f32 * f).round() as i32);
        w
    }

    fn armor_pieces(&self) -> impl Iterator<Item = &crate::defs::Armor> {
        [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Offhand]
            .into_iter()
            .filter_map(|s| self.world.equipment.item(s))
            .filter_map(|id| self.project.db.item(id)?.armor.as_ref())
    }

    pub(super) fn player_defense(&self) -> i32 {
        self.combat.defense + self.armor_pieces().map(|a| a.defense).sum::<i32>() + self.skill_defense() + crate::effects::defense_bonus(&self.world.effects) + self.player_race().map_or(0, |r| r.defense)
    }

    /// Movement speed factor from armor (heavy armor slows down).
    pub(super) fn speed_factor(&self) -> f32 {
        let race = self.player_race().map_or(1.0, |r| r.speed_factor());
        let base = ((1.0 + self.armor_pieces().map(|a| a.speed).sum::<f32>()) * self.skill_speed_factor() * race).clamp(0.3, 2.0);
        base * crate::effects::speed_factor(&self.world.effects)
    }

    pub(super) fn stats(&self) -> Stats {
        let w = self.attack_weapon();
        Stats {
            attack: w.damage,
            defense: self.player_defense(),
            speed: self.player.speed * self.speed_factor(),
            cooldown: w.cooldown,
            reach: w.ranged.as_ref().map_or(w.reach, |r| r.range),
            ranged: w.ranged.is_some(),
        }
    }

    /// Old saves referenced the equipped weapon by id; move it into the weapon slot.
    pub(super) fn migrate_equipment(&mut self) {
        let id = std::mem::take(&mut self.world.equipped);
        if !id.is_empty() && self.world.equipment.weapon.is_none() && self.world.inventory.remove(&id, 1) == 1 {
            self.world.equipment.weapon = Some(ItemStack::new(&id, 1));
        }
    }

    /// Equips one item from inventory slot `i`; the previously worn item goes back to the
    /// inventory (into the freed slot if possible). Returns false if impossible.
    pub(super) fn equip_from_slot(&mut self, i: usize) -> bool {
        let Some(stack) = self.world.inventory.slots.get(i).cloned().flatten() else { return false };
        let Some(slot) = self.project.db.item(&stack.item).and_then(|d| d.equip_slot()) else { return false };
        // A two-handed weapon and something in the left hand don't go together: the other one
        // goes back into the inventory first.
        if let Some(other) = self.hand_conflict(&stack.item, slot) {
            let name = |g: &Self, id: &str| g.project.db.item(id).map_or(id.to_string(), |d| d.name.clone());
            let (new, old) = (name(self, &stack.item), self.world.equipment.item(other).map(|id| name(self, id)).unwrap_or_default());
            if !self.unequip(other, None) {
                self.ui.toast(format!("No room to put the {old} away"));
                return false;
            }
            let why = if slot == EquipSlot::Weapon { format!("The {new} needs both hands") } else { format!("Both hands were on the {old}") };
            self.ui.toast(format!("{why}: {old} put away"));
        }
        let one = self.world.inventory.take_from_slot(i, Some(1)).unwrap();
        let old = self.world.equipment.slot_mut(slot).replace(one);
        if let Some(old) = old {
            let inv = &mut self.world.inventory;
            if inv.slots[i].is_none() {
                inv.slots[i] = Some(old);
            } else if inv.add(&old.item, old.count) > 0 {
                // No room for the old item: undo.
                let new = self.world.equipment.slot_mut(slot).replace(old).unwrap();
                self.world.inventory.add(&new.item, new.count);
                self.ui.toast("Inventory full".into());
                return false;
            }
        }
        let name = self.project.db.item(&stack.item).map_or(stack.item.clone(), |d| d.name.clone());
        self.ui.toast(format!("Equipped {name}"));
        true
    }

    /// The equipped slot that has to be emptied before `item` goes into `slot`: the left hand
    /// for a two-handed weapon, a two-handed weapon for a left-hand item.
    fn hand_conflict(&self, item: &str, slot: EquipSlot) -> Option<EquipSlot> {
        let two_handed = |id: &str| self.project.db.item(id).and_then(|d| d.weapon.as_ref()).is_some_and(|w| w.two_handed);
        match slot {
            EquipSlot::Weapon if two_handed(item) && self.world.equipment.offhand.is_some() => Some(EquipSlot::Offhand),
            EquipSlot::Offhand if self.world.equipment.item(EquipSlot::Weapon).is_some_and(two_handed) => Some(EquipSlot::Weapon),
            _ => None,
        }
    }

    /// Whether the equipped weapon needs both hands.
    pub(super) fn two_handed_equipped(&self) -> bool {
        self.current_weapon().is_some_and(|w| w.two_handed)
    }

    /// Equips the first inventory stack of `item`.
    pub(super) fn equip_item(&mut self, item: &str) -> bool {
        match self.world.inventory.slots.iter().position(|s| s.as_ref().is_some_and(|s| s.item == item)) {
            Some(i) => self.equip_from_slot(i),
            None => false,
        }
    }

    /// Takes off an equipped item; `to` = preferred inventory slot.
    pub(super) fn unequip(&mut self, slot: EquipSlot, to: Option<usize>) -> bool {
        let Some(stack) = self.world.equipment.get(slot).cloned() else { return false };
        let inv = &mut self.world.inventory;
        let placed = match to {
            Some(j) if inv.slots.get(j).is_some_and(|s| s.is_none()) => {
                inv.slots[j] = Some(stack.clone());
                true
            }
            _ => inv.space_for(&stack.item) >= stack.count && inv.add(&stack.item, stack.count) == 0,
        };
        if !placed {
            self.ui.toast("Inventory full".into());
            return false;
        }
        *self.world.equipment.slot_mut(slot) = None;
        true
    }

    /// Q: cycles bare hands → owned weapons (sorted by name) → bare hands.
    pub(super) fn cycle_weapon(&mut self) {
        let mut ids: Vec<String> = self
            .world
            .inventory
            .item_ids()
            .into_iter()
            .chain(self.world.equipment.item(EquipSlot::Weapon).map(str::to_string))
            .filter(|id| self.project.db.item(id).is_some_and(|d| d.weapon.is_some()))
            .collect();
        ids.sort();
        ids.dedup();
        ids.insert(0, String::new());
        let cur = self.world.equipment.item(EquipSlot::Weapon).unwrap_or("").to_string();
        let pos = ids.iter().position(|w| *w == cur).unwrap_or(0);
        let next = ids[(pos + 1) % ids.len()].clone();
        if next.is_empty() {
            if self.unequip(EquipSlot::Weapon, None) {
                self.ui.toast("Weapon: bare hands".into());
            }
        } else {
            self.equip_item(&next);
        }
    }

    /// Draws the player's idle front frame scaled into `dest` (character sheet portrait).
    pub(super) fn draw_portrait(&self, dest: Rect) {
        let Some(a) = self.player_appearance() else { return };
        let (fw, fh) = self.character_frame_size(&a);
        let scale = (dest.w / fw).min(dest.h / fh).floor().max(1.0);
        let (w, h) = (fw * scale, fh * scale);
        self.draw_character_in(&a, Rect::new(dest.x + (dest.w - w) / 2.0, dest.y + dest.h - h, w, h));
    }
}
