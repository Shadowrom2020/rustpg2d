//! Equipment (head, body, weapon, left hand): equipping from the inventory, two-handed weapons,
//! derived stats, armor overlays.

use macroquad::prelude::*;

use super::Game;
use crate::defs::{EquipSlot, Weapon};
use crate::inventory::ItemStack;

/// Player stats shown on the character sheet.
pub(super) struct Stats {
    pub attack: i32,
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
        self.current_weapon().unwrap_or(Weapon {
            reach: UNARMED_REACH,
            ..Weapon::melee(self.combat.damage, UNARMED_COOLDOWN)
        })
    }

    fn armor_pieces(&self) -> impl Iterator<Item = &crate::defs::Armor> {
        [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Offhand]
            .into_iter()
            .filter_map(|s| self.world.equipment.item(s))
            .filter_map(|id| self.project.db.item(id)?.armor.as_ref())
    }

    /// The armor the hero wears in a slot (its definition's armor part).
    fn worn_armor(&self, slot: EquipSlot) -> Option<&crate::defs::Armor> {
        let id = self.world.equipment.item(slot)?;
        self.project.db.item(id)?.armor.as_ref()
    }

    /// The hero's armor class (SRD): body armor's base + DEX up to its cap (10 + DEX without),
    /// a shield's bonus, the class's unarmored formulas and fighting style, the spells' buffs.
    pub(super) fn armor_class(&self) -> i32 {
        // (A beast's shape: its natural armor.)
        if let Some(b) = self.hero_form_block() {
            return self.buff_ac(super::turns::Who::Player, b.ac);
        }
        let dex = self.world.abilities.modifier(crate::srd::Ability::Dex);
        let (ac, body, shield) = crate::srd::equipment::armor_class(self.worn_armor(EquipSlot::Body), self.worn_armor(EquipSlot::Offhand), dex);
        // Magic armor and shields: their +N (on what they give); items: a Ring of Protection.
        let me = super::turns::Who::Player;
        let plus = |s: EquipSlot| self.world.equipment.item(s).filter(|_| self.worn_armor(s).is_some()).map_or(0, |id| self.item_bonus(me, id));
        let (ac, shield) = (ac + if body { plus(EquipSlot::Body) } else { 0 }, if shield > 0 { shield + plus(EquipSlot::Offhand) } else { 0 });
        let items = self.bonus_of(me, crate::classes::BonusTo::Ac);
        // (And spells: Mage Armor, Shield of Faith, Haste.)
        self.buff_ac(me, self.class_armor_class(ac + shield, body, shield) + items)
    }

    /// Whether the hero is proficient with all the armor and the shield worn.
    pub(super) fn armor_trained(&self) -> bool {
        let w = &self.world;
        [EquipSlot::Body, EquipSlot::Offhand]
            .into_iter()
            .filter_map(|s| self.worn_armor(s).and_then(|a| a.category))
            .all(|c| crate::srd::equipment::armor_proficient(|p| w.is_proficient(p), c))
    }

    /// Body armor too heavy for the hero's STR: 10 ft slower (not a dwarf: heavy armor doesn't
    /// slow them).
    pub(super) fn armor_too_heavy(&self) -> bool {
        let dwarf = self.world.player_race.contains("dwarf");
        !dwarf && self.worn_armor(EquipSlot::Body).is_some_and(|a| a.str_min > 0 && self.world.abilities.score(crate::srd::Ability::Str) < a.str_min)
    }

    /// The hero's walking speed in feet: the race's, the class's (Fast Movement, Unarmored
    /// Movement), 10 less in armor too heavy, then the load's.
    pub(super) fn hero_speed_ft(&self) -> u32 {
        // (A beast's shape: its speed, or its flying speed.)
        if let Some(b) = self.hero_form_block() {
            return b.speed_ft.max(b.speeds.fly);
        }
        let base = self.player_race().map_or(30, |r| r.walk_feet()) + self.class_speed_ft(self.unarmored());
        let base = if self.armor_too_heavy() { base.saturating_sub(10) } else { base };
        // (Fly: its flying speed when faster.)
        let fly = crate::srd::conditions::buffs(&self.world.effects).find_map(|b| if let crate::srd::conditions::Buff::Fly { feet } = b { Some(*feet) } else { None }).unwrap_or(0);
        let base = base.max(fly);
        // What the hero carries (encumbrance).
        self.hero_load().speed(base)
    }

    /// Whether a combatant wears armor or a shield it isn't proficient with (disadvantage on STR
    /// and DEX rolls and attacks, no spells): the hero's, a classed party member's.
    pub(super) fn who_armor_untrained(&self, who: super::turns::Who) -> bool {
        match who {
            super::turns::Who::Player => !self.armor_trained(),
            super::turns::Who::Npc(uid) => {
                let Some(e) = self.loaded.entities.iter().find(|e| e.uid == uid) else { return false };
                let Some(b) = self.member_block(uid) else { return false };
                [EquipSlot::Body, EquipSlot::Offhand]
                    .into_iter()
                    .filter_map(|s| e.equipment.item(s).and_then(|id| self.project.db.item(id)?.armor.as_ref()?.category))
                    .any(|c| !crate::srd::equipment::armor_proficient(|p| b.proficiencies.iter().any(|x| crate::srd::abilities::proficiency_id(x) == p), c))
            }
        }
    }

    /// `weapon=` for status: the hero's weapon by its SRD name, category and properties, and
    /// whether they are proficient (`longsword:martial:versatile:prof`; `-` bare-handed).
    pub(super) fn weapon_status(&self) -> String {
        let Some(id) = self.world.equipment.item(EquipSlot::Weapon) else { return "-".into() };
        let Some(d) = self.project.db.item(id) else { return "-".into() };
        let Some(w) = &d.weapon else { return "-".into() };
        let cat = match w.category {
            Some(crate::defs::WeaponCategory::Simple) => "simple",
            Some(crate::defs::WeaponCategory::Martial) => "martial",
            None => "-",
        };
        let props: Vec<String> = w.properties.iter().map(|p| p.label().to_ascii_lowercase().replace(' ', "-")).collect();
        let prof = crate::srd::equipment::weapon_proficient(|p| self.world.is_proficient(p), w, &d.srd);
        format!("{}:{cat}:{}:{}", if d.srd.is_empty() { id } else { &d.srd }, props.join("+"), if prof { "prof" } else { "noprof" })
    }

    /// `armor=` for status: body armor and shield categories, `untrained`, `heavy` (too heavy).
    pub(super) fn armor_status(&self) -> String {
        let cat = |s: EquipSlot| self.worn_armor(s).and_then(|a| a.category).map_or("-".to_string(), |c| c.label().to_ascii_lowercase().replace(' ', "-"));
        let mut s = format!("{}+{}", cat(EquipSlot::Body), cat(EquipSlot::Offhand));
        if !self.armor_trained() {
            s += ":untrained";
        }
        if self.armor_too_heavy() {
            s += ":heavy";
        }
        s
    }

    /// Whether the hero's weapons in both hands are light (two-weapon fighting).
    pub(super) fn both_light(&self, who: super::turns::Who) -> bool {
        if who != super::turns::Who::Player {
            return true;
        }
        let light = |s: EquipSlot| self.world.equipment.item(s).and_then(|id| self.project.db.item(id)?.weapon.as_ref()).is_some_and(|w| w.has(crate::defs::WeaponProperty::Light) || w.category.is_none());
        light(EquipSlot::Weapon) && light(EquipSlot::Offhand)
    }

    /// Armor and shield worn: the flags checks read (untrained armor: disadvantage on STR and
    /// DEX; Stealth disadvantage). Once a frame.
    pub(super) fn refresh_armor_training(&mut self) {
        let untrained = !self.armor_trained();
        let stealth = [EquipSlot::Body, EquipSlot::Offhand].into_iter().any(|s| self.worn_armor(s).is_some_and(|a| a.stealth_disadvantage));
        self.world.training.armor_untrained = untrained;
        self.world.training.stealth_disadvantage = stealth;
    }

    /// Whether the hero wears no body armor and holds no shield (unarmored features).
    pub(super) fn unarmored(&self) -> bool {
        [EquipSlot::Body, EquipSlot::Offhand].into_iter().all(|s| self.world.equipment.item(s).and_then(|id| self.project.db.item(id)?.armor.as_ref()).is_none())
    }

    pub(super) fn player_defense(&self) -> i32 {
        self.combat.defense + self.armor_pieces().map(|a| a.defense).sum::<i32>() + self.player_race().map_or(0, |r| r.defense)
    }

    /// What the hero does with damage of a type: the race's resistances, the armor's, and every
    /// type while petrified.
    pub(super) fn player_defenses(&self) -> crate::srd::Defenses {
        let mut resistances: Vec<String> = self.world.resistances.iter().cloned().collect();
        resistances.extend(self.armor_pieces().flat_map(|a| a.resistances.iter().cloned()));
        if crate::srd::conditions::rules(&self.world.effects).resist_all {
            resistances.push("all".into());
        }
        // The class's (Purity of Body: poison).
        let t = self.hero_traits();
        resistances.extend(t.iter().flat_map(|t| t.resistances.iter().cloned()));
        let immunities = t.map(|t| t.immunities).unwrap_or_default();
        crate::srd::Defenses { resistances, immunities, ..Default::default() }
    }

    /// Movement speed factor from armor (heavy armor slows down).
    pub(super) fn speed_factor(&self) -> f32 {
        let race = self.player_race().map_or(1.0, |r| r.speed_factor());
        let base = ((1.0 + self.armor_pieces().map(|a| a.speed).sum::<f32>()) * race).clamp(0.3, 2.0);
        base * crate::srd::conditions::rules(&self.world.effects).speed * self.load_factor()
    }

    pub(super) fn stats(&self) -> Stats {
        let w = self.attack_weapon();
        Stats {
            attack: w.damage,
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

    /// The player's inventory size: the project's, plus the slots of the backpack worn.
    pub(super) fn player_capacity(&self) -> usize {
        let bag = self.world.equipment.item(EquipSlot::Back).and_then(|id| self.project.db.item(id)).map_or(0, |d| d.bag_slots);
        (self.project.config.inventory_size.max(1) + bag) as usize
    }

    /// Sizes the inventory to [`Self::player_capacity`]: growing adds empty slots; shrinking packs
    /// the stacks of the slots that go into free slots before them first. False (and nothing
    /// changed) when they don't fit.
    pub(super) fn fit_inventory(&mut self) -> bool {
        match self.world.inventory.packed_to(self.player_capacity()) {
            Some(packed) => {
                self.world.inventory = packed;
                true
            }
            None => false,
        }
    }

    /// Runs an equipment change; when it would take away inventory slots that hold items
    /// which don't fit elsewhere (a backpack taken off or swapped for a smaller one), undoes it.
    fn keeping_room(&mut self, change: impl FnOnce(&mut Self) -> bool) -> bool {
        let before = (self.world.inventory.clone(), self.world.equipment.clone());
        if !change(self) {
            return false;
        }
        if self.fit_inventory() {
            return true;
        }
        (self.world.inventory, self.world.equipment) = before;
        self.ui.toast("Make room first: the backpack's slots aren't empty".into());
        false
    }

    /// Equips one item from inventory slot `i`; the previously worn item goes back to the
    /// inventory (into the freed slot if possible). Returns false if impossible.
    pub(super) fn equip_from_slot(&mut self, i: usize) -> bool {
        self.equip_from_slot_to(i, None)
    }

    /// Like [`Self::equip_from_slot`], into `target` when given (a ring into the second ring
    /// slot).
    pub(super) fn equip_from_slot_to(&mut self, i: usize, target: Option<EquipSlot>) -> bool {
        let Some(item) = self.world.inventory.slots.get(i).cloned().flatten().map(|s| s.item) else { return false };
        let slot = self.project.db.slot_of(&item);
        if let Err(why) = slot.map_or(Ok(()), |s| self.fight_object_use(s, &item)) {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
            return false;
        }
        self.keeping_room(|g| g.equip_from_slot_now(i, target))
    }

    /// Changing the hero's equipment in a fight, on the hero's turn (the SRD's object
    /// interaction): drawing or stowing a weapon is free once a turn, the next takes the action;
    /// a shield takes the action; armor, clothing and helmets take minutes. Free outside fights
    /// and for events and scripts.
    pub(super) fn fight_object_use(&mut self, slot: EquipSlot, item: &str) -> Result<(), &'static str> {
        if self.runner.busy() {
            return Ok(());
        }
        let Some(c) = self.fight.as_ref() else { return Ok(()) };
        if c.current().who != super::turns::Who::Player || c.current().ai {
            return Err("Not your turn");
        }
        if matches!(slot, EquipSlot::Body | EquipSlot::Clothing | EquipSlot::Head | EquipSlot::Back) {
            return Err("Takes minutes");
        }
        let shield = self.project.db.item(item).and_then(|d| d.armor.as_ref()).is_some_and(|a| a.category == Some(crate::defs::ArmorCategory::Shield));
        let c = self.fight.as_mut().ok_or("No fight")?;
        if shield || c.turn.interacted {
            if c.turn.action {
                return Err("Action used");
            }
            c.turn.action = true;
        } else {
            c.turn.interacted = true;
        }
        Ok(())
    }

    fn equip_from_slot_now(&mut self, i: usize, target: Option<EquipSlot>) -> bool {
        let Some(stack) = self.world.inventory.slots.get(i).cloned().flatten() else { return false };
        let Some(mut slot) = self.project.db.slot_of(&stack.item) else { return false };
        if slot == EquipSlot::Ammo {
            return self.equip_ammo_from(i);
        }
        // A ring: the slot asked for, else the first free ring slot.
        if slot == EquipSlot::Ring {
            slot = match target {
                Some(EquipSlot::Ring2) => EquipSlot::Ring2,
                Some(_) => EquipSlot::Ring,
                None if self.world.equipment.ring.is_some() && self.world.equipment.ring2.is_none() => EquipSlot::Ring2,
                None => EquipSlot::Ring,
            };
        }
        // A two-handed weapon and something in the left hand don't go together: the other one
        // goes back into the inventory first.
        if let Some(other) = self.hand_conflict(&stack.item, slot) {
            let name = |g: &Self, id: &str| g.project.db.item(id).map_or(id.to_string(), |d| d.name.clone());
            let (new, old) = (name(self, &stack.item), self.world.equipment.item(other).map(|id| name(self, id)).unwrap_or_default());
            if !self.unequip_now(other, None) {
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
            } else if inv.add_stack(old.clone()) > 0 {
                // No room for the old item: undo.
                let new = self.world.equipment.slot_mut(slot).replace(old).unwrap();
                self.world.inventory.add_stack(new);
                self.ui.toast("Inventory full".into());
                return false;
            }
        }
        let name = self.project.db.item(&stack.item).map_or(stack.item.clone(), |d| d.name.clone());
        self.ui.toast(format!("Equipped {name}"));
        true
    }

    /// Puts the whole ammunition stack of inventory slot `i` into the ammunition slot: onto the
    /// stack there when it is the same item (as much as fits), else in its place (the old stack
    /// goes into slot `i`).
    fn equip_ammo_from(&mut self, i: usize) -> bool {
        let Some(stack) = self.world.inventory.slots.get(i).cloned().flatten() else { return false };
        let name = self.project.db.item(&stack.item).map_or(stack.item.clone(), |d| d.name.clone());
        match self.world.equipment.ammo.as_mut() {
            Some(q) if q.item == stack.item && q.spent == stack.spent => {
                let max = self.project.db.item(&stack.item).map_or(1, |d| d.max_stack.max(1));
                let n = stack.count.min(max.saturating_sub(q.count));
                if n == 0 {
                    self.ui.toast(format!("The {name} stack is full"));
                    return false;
                }
                q.count += n;
                self.world.inventory.take_from_slot(i, Some(n));
            }
            _ => {
                let new = self.world.inventory.take_from_slot(i, None).unwrap();
                if let Some(old) = self.world.equipment.ammo.replace(new) {
                    self.world.inventory.slots[i] = Some(old);
                }
            }
        }
        self.ui.toast(format!("Equipped {name}"));
        true
    }

    /// The ammunition the hero's next shot takes for a weapon that shoots `weapon_ammo`
    /// (`Ranged::ammo`): (item, how many are left there, whether it is the ammunition slot's).
    /// The slot's stack when it fits the weapon, else the bag's: the weapon's own ammunition
    /// first, then other of its kind, plain before magic. None = none left (or `weapon_ammo` "").
    pub(super) fn next_ammo(&self, weapon_ammo: &str) -> Option<(String, u32, bool)> {
        let db = &self.project.db;
        if let Some(q) = self.world.equipment.ammo.as_ref().filter(|q| q.count > 0 && db.ammo_fits(&q.item, weapon_ammo)) {
            return Some((q.item.clone(), q.count, true));
        }
        let mut ids: Vec<String> = self.world.inventory.item_ids().into_iter().filter(|id| db.ammo_fits(id, weapon_ammo)).collect();
        ids.sort_by_key(|id| (id != weapon_ammo, db.item(id).is_some_and(|d| d.magic.is_some()), db.item(id).map_or(0, |d| d.magic_bonus())));
        let id = ids.into_iter().next()?;
        let n = self.world.inventory.count(&id);
        Some((id, n, false))
    }

    /// `ammo=` for status: the ammunition slot's stack (item:count, `-` = empty), and what the
    /// equipped weapon's next shot takes: `>item:left(slot|bag)`, `>none` when it has none left.
    pub(super) fn ammo_status(&self) -> String {
        let slot = self.world.equipment.ammo.as_ref().map_or("-".to_string(), |s| format!("{}:{}", s.item, s.count));
        let weapon = self.world.equipment.item(EquipSlot::Weapon).and_then(|id| self.project.db.item(id));
        match weapon.and_then(|d| d.weapon.as_ref()?.ranged.as_ref()).map(|r| r.ammo.clone()).filter(|a| !a.is_empty()) {
            Some(a) => match self.next_ammo(&a) {
                Some((id, n, from_slot)) => format!("{slot}>{id}:{n}({})", if from_slot { "slot" } else { "bag" }),
                None => format!("{slot}>none"),
            },
            None => slot,
        }
    }

    /// Spends one piece of the ammunition [`Self::next_ammo`] picks (false = none left).
    pub(super) fn spend_ammo(&mut self, weapon_ammo: &str) -> bool {
        let Some((id, _, from_slot)) = self.next_ammo(weapon_ammo) else { return false };
        // (Counted for the search after the fight.)
        if let Some(c) = self.fight.as_mut() {
            match c.shot.iter_mut().find(|s| s.0 == id) {
                Some(s) => s.1 += 1,
                None => c.shot.push((id.clone(), 1)),
            }
        }
        if from_slot {
            let q = &mut self.world.equipment.ammo;
            if let Some(s) = q.as_mut() {
                s.count -= 1;
                if s.count == 0 {
                    *q = None;
                }
            }
        } else {
            self.world.inventory.remove(&id, 1);
        }
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
        let Some(item) = self.world.equipment.item(slot).map(str::to_string) else { return false };
        if let Err(why) = self.fight_object_use(slot, &item) {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
            return false;
        }
        self.keeping_room(|g| g.unequip_now(slot, to))
    }

    fn unequip_now(&mut self, slot: EquipSlot, to: Option<usize>) -> bool {
        let Some(stack) = self.world.equipment.get(slot).cloned() else { return false };
        let inv = &mut self.world.inventory;
        let placed = match to {
            Some(j) if inv.slots.get(j).is_some_and(|s| s.is_none()) => {
                inv.slots[j] = Some(stack.clone());
                true
            }
            _ => inv.space_for(&stack.item) >= stack.count && inv.add_stack(stack.clone()) == 0,
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
        let gear = self.player_gear();
        self.draw_character_in(&a, &gear, Rect::new(dest.x + (dest.w - w) / 2.0, dest.y + dest.h - h, w, h));
    }
}
