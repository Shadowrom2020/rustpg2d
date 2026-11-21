//! Thrown weapons (the SRD's Thrown property): a throw takes the weapon out of the thrower's
//! hand and it lands on the ground next to the target, as an ordinary item pile. The thrower
//! fights with what it has left (the hero with another weapon or bare hands, a character with its
//! other attacks, else an unarmed strike) until it steps onto the weapon again, which picks it up
//! (free in a fight). After the fight the surviving characters take back what they threw; the
//! hero's lies there until picked up.

use macroquad::prelude::*;

use super::{AttackSpec, Who};
use crate::defs::{EquipSlot, ItemDef};
use crate::grid::TilePos;
use crate::runtime::Game;

/// A weapon a fighter threw this fight.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct Thrown {
    /// The stat block attack it was ("" = the hero's weapon).
    pub attack: String,
    /// The item that landed (None: no item stands for the attack, nothing lies there).
    pub item: Option<String>,
    /// Its pile on the ground (a dropped item's uid).
    pub drop: Option<u32>,
}

/// Whether item `d` is the weapon a stat block attack named `attack` uses (by name, id or SRD
/// index: "Spear", "spear", "Light Hammer" ~ `light-hammer`).
pub(super) fn item_for_attack(d: &ItemDef, attack: &str) -> bool {
    let a = attack.trim();
    !a.is_empty() && d.weapon.is_some() && (d.name.eq_ignore_ascii_case(a) || d.id.eq_ignore_ascii_case(a) || (!d.srd.is_empty() && d.srd == a.to_ascii_lowercase().replace(' ', "-")))
}

impl Game {
    /// The item a thrown attack throws: the hero's weapon in hand; a character's weapon in hand
    /// when it is the attack's, else the project's item for the attack (an orc's javelin).
    pub(super) fn thrown_item(&self, who: Who, spec: &AttackSpec) -> Option<String> {
        match who {
            Who::Player => self.world.equipment.item(EquipSlot::Weapon).map(str::to_string),
            Who::Npc(_) => {
                let held = self.who_index(who).and_then(|i| self.loaded.entities[i].equipment.item(EquipSlot::Weapon)).filter(|id| self.project.db.item(id).is_some_and(|d| item_for_attack(d, &spec.name) || spec.name.is_empty()));
                held.map(str::to_string).or_else(|| self.project.db.items.iter().find(|d| item_for_attack(d, &spec.name)).map(|d| d.id.clone()))
            }
        }
    }

    /// What a throw throws, as the player reads it ("Spear").
    pub(super) fn thrown_item_name(&self, who: Who, spec: &AttackSpec) -> String {
        self.thrown_item(who, spec).and_then(|id| self.project.db.item(&id).map(|d| crate::lang::tr(&d.name))).unwrap_or_else(|| spec.name.clone())
    }

    /// After a throw: the weapon leaves the hand and lands next to the target; the attack is gone
    /// until it is picked up again. A consumable one (a flask, a vial) shatters instead.
    pub(super) fn after_throw(&mut self, who: Who, target: Who, spec: &AttackSpec) {
        let item = self.thrown_item(who, spec);
        // Out of the hand.
        match who {
            Who::Player => {
                if item.is_some() {
                    let slot = self.world.equipment.slot_mut(EquipSlot::Weapon);
                    match slot {
                        Some(s) if s.count > 1 => s.count -= 1,
                        _ => *slot = None,
                    }
                }
            }
            Who::Npc(_) => {
                if let (Some(i), Some(id)) = (self.who_index(who), &item) {
                    let e = &mut self.loaded.entities[i];
                    if e.equipment.item(EquipSlot::Weapon) == Some(id.as_str()) {
                        *e.equipment.slot_mut(EquipSlot::Weapon) = None;
                        crate::runtime::npc_items::apply_gear(&self.project.db, &mut self.loaded.entities[i]);
                        self.persist_npc_items(i);
                    }
                }
            }
        }
        // A consumable one shatters (a flask of alchemist's fire, a vial of acid): nothing lands.
        let shatters = item.as_ref().and_then(|id| self.project.db.item(id)).is_some_and(|d| d.consumable);
        // On the ground next to the target.
        let drop = item.as_ref().filter(|_| !shatters).map(|id| {
            let at = self.landing_spot(who, target);
            self.place_drop(id, 1, at)
        });
        let name = self.who_name(who);
        let what = item.as_ref().and_then(|id| self.project.db.item(id)).map_or(spec.name.clone(), |d| crate::lang::tr(&d.name));
        self.fight_log(crate::lang::tr_ui("{} throws the {}").replacen("{}", &name, 1).replacen("{}", &what, 1));
        if shatters {
            return;
        }
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.thrown.push(Thrown { attack: if who == Who::Player { String::new() } else { spec.name.clone() }, item, drop });
        }
    }

    /// Where a thrown weapon lands: a free tile next to the target on the thrower's side, else
    /// the target's own tile.
    fn landing_spot(&self, who: Who, target: Who) -> Vec2 {
        let (from, at) = (self.who_tile(who), self.who_tile(target));
        // (Round the target's whole space.)
        let space = self.who_space(target);
        let (a, n) = (self.space_anchor(target), self.space_n(target));
        let mut near: Vec<TilePos> = (-1..=n).flat_map(|dx| (-1..=n).map(move |dy| (a.0 + dx, a.1 + dy))).filter(|t| !space.contains(t)).collect();
        near.sort_by_key(|t| (t.0 - from.0).abs().max((t.1 - from.1).abs()));
        let free = near.into_iter().find(|t| self.loaded.map.in_bounds(t.0, t.1) && !self.tile_solid(t.0, t.1) && self.who_at(*t).is_none());
        self.tile_spot(free.unwrap_or(at))
    }

    /// The tiles where weapons `who` threw lie (its AI walks there to take them back).
    pub(super) fn thrown_tiles(&self, who: Who) -> Vec<TilePos> {
        let Some(f) = self.fight.as_ref().and_then(|c| c.fighter(who)) else { return Vec::new() };
        f.thrown.iter().filter_map(|t| t.drop).filter_map(|uid| self.loaded.entities.iter().find(|e| e.uid == uid && e.active)).map(|e| self.tile_of(e.body.pos)).collect()
    }

    /// `who` stepped onto tile `t`: it picks up the weapons it threw that lie there.
    pub(super) fn pick_up_thrown(&mut self, who: Who, t: TilePos) {
        let Some(thrown) = self.fight.as_ref().and_then(|c| c.fighter(who)).map(|f| f.thrown.clone()) else { return };
        for (k, th) in thrown.iter().enumerate().rev() {
            let Some(uid) = th.drop else { continue };
            let Some(i) = self.loaded.entities.iter().position(|e| e.uid == uid && e.active) else { continue };
            if self.tile_of(self.loaded.entities[i].body.pos) != t {
                continue;
            }
            self.take_back(who, i, th);
            if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
                f.thrown.remove(k);
            }
        }
    }

    /// The pile `i` (a thrown weapon) goes back into `who`'s hand (into the bag when the hand
    /// is full).
    fn take_back(&mut self, who: Who, i: usize, th: &Thrown) {
        let Some(item) = th.item.clone() else { return };
        let e = &mut self.loaded.entities[i];
        e.count = e.count.saturating_sub(1);
        e.active = e.count > 0;
        let (map, uid, left) = (self.loaded.map.name.clone(), e.uid, e.count);
        self.world.set_dropped_count(&map, uid, left);
        match who {
            Who::Player => {
                if self.world.equipment.item(EquipSlot::Weapon).is_none() {
                    *self.world.equipment.slot_mut(EquipSlot::Weapon) = Some(crate::inventory::ItemStack::new(&item, 1));
                } else {
                    self.world.give_item(&item, 1);
                }
            }
            Who::Npc(_) => {
                if let Some(j) = self.who_index(who) {
                    let e = &mut self.loaded.entities[j];
                    if e.equipment.item(EquipSlot::Weapon).is_none() {
                        *e.equipment.slot_mut(EquipSlot::Weapon) = Some(crate::inventory::ItemStack::new(&item, 1));
                    } else {
                        e.inventory.push(crate::inventory::ItemStack::new(&item, 1));
                    }
                    crate::runtime::npc_items::apply_gear(&self.project.db, &mut self.loaded.entities[j]);
                    self.persist_npc_items(j);
                }
            }
        }
        if self.fight.is_some() {
            let name = self.who_name(who);
            let what = self.project.db.item(&item).map_or(item.clone(), |d| crate::lang::tr(&d.name));
            self.fight_log(crate::lang::tr_ui("{} picks up the {}").replacen("{}", &name, 1).replacen("{}", &what, 1));
        }
    }

    /// The fight is over: the surviving characters take back what they threw (the hero's
    /// weapons lie there until picked up).
    pub(super) fn return_thrown(&mut self) {
        let Some(c) = &self.fight else { return };
        let back: Vec<(Who, Thrown)> = c.order.iter().filter(|f| f.who != Who::Player && !f.out).flat_map(|f| f.thrown.iter().map(move |t| (f.who, t.clone()))).collect();
        for (who, th) in back {
            if let Some(i) = th.drop.and_then(|uid| self.loaded.entities.iter().position(|e| e.uid == uid && e.active)) {
                self.take_back(who, i, &th);
            }
        }
    }

    /// The stat block attacks `who` threw away this fight (left out of its attacks).
    pub(super) fn thrown_attacks(&self, who: Who) -> Vec<String> {
        self.fight.as_ref().and_then(|c| c.fighter(who)).map_or(Vec::new(), |f| f.thrown.iter().map(|t| t.attack.clone()).filter(|a| !a.is_empty()).collect())
    }
}

impl Game {
    /// The item piles lying on tile `t` (entity indices).
    pub(super) fn piles_at(&self, t: TilePos) -> Vec<usize> {
        (0..self.loaded.entities.len()).filter(|&i| {
            let e = &self.loaded.entities[i];
            e.active && e.kind == crate::entity::EntityKind::Item && e.count > 0 && self.tile_of(e.body.pos) == t
        }).collect()
    }

    /// `who` picks up the piles on tile `t` (free in a fight): the hero into the bag (what doesn't
    /// fit stays), a party member into its own. Returns how many stacks it took.
    pub(super) fn pick_up_at(&mut self, who: Who, t: TilePos) -> usize {
        let mut taken = Vec::new();
        for i in self.piles_at(t) {
            let (item, count) = (self.loaded.entities[i].def.clone(), self.loaded.entities[i].count);
            let spent = self.world.dropped_spent(&self.loaded.map.name, self.loaded.entities[i].uid);
            let stack = crate::inventory::ItemStack { spent, ..crate::inventory::ItemStack::new(&item, count) };
            let left = match who {
                Who::Player => self.world.inventory.add_stack(stack),
                Who::Npc(_) => match self.who_index(who) {
                    Some(j) => {
                        self.loaded.entities[j].inventory.push(stack);
                        self.persist_npc_items(j);
                        0
                    }
                    None => count,
                },
            };
            if left == count {
                continue;
            }
            let e = &mut self.loaded.entities[i];
            e.count = left;
            e.active = left > 0;
            self.persist(i);
            let name = self.project.db.item(&item).map_or(item.clone(), |d| crate::lang::tr(&d.name));
            taken.push(if count - left > 1 { format!("{} × {name}", count - left) } else { name });
        }
        if !taken.is_empty() {
            self.audio.play(&self.project.config.sfx.pickup);
            let line = crate::lang::tr_ui("{} picks up {}").replacen("{}", &self.who_name(who), 1).replacen("{}", &taken.join(", "), 1);
            if self.fight.is_some() {
                self.fight_log(line);
            } else {
                self.ui.toast(line);
            }
        }
        taken.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stat_block_attack_finds_its_weapon() {
        let spear = ItemDef { name: "Spear".into(), weapon: Some(crate::defs::Weapon::melee(2, 0.5)), ..ItemDef::fresh("spear") };
        let hammer = ItemDef { name: "Hammer of Doom".into(), srd: "light-hammer".into(), weapon: Some(crate::defs::Weapon::melee(2, 0.5)), ..ItemDef::fresh("lh") };
        assert!(item_for_attack(&spear, "Spear") && item_for_attack(&spear, "spear"));
        assert!(item_for_attack(&hammer, "Light Hammer"), "by its SRD index");
        assert!(!item_for_attack(&spear, "Javelin") && !item_for_attack(&spear, ""));
        let rope = ItemDef { name: "Spear".into(), ..ItemDef::fresh("rope") };
        assert!(!item_for_attack(&rope, "Spear"), "only weapons");
    }
}
