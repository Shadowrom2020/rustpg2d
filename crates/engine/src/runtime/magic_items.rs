//! Magic items at runtime (`ItemDef::magic`, `srd/magic_items.rs`): which items work (worn or
//! held, attuned when they need it), attunement (at most three; an hour outside fights), their
//! effects folded into the traits the numbers read, ability scores raised by items, charges on
//! the stacks (`ItemStack::spent`) and their return at dawn, what using an item does (its spell
//! parts), and spells cast from items.

use crate::classes::{BonusTo, ClassTraits, FeatureEffect};
use crate::defs::{EquipSlot, ItemDef};
use crate::inventory::ItemStack;
use crate::skills::{SpellDef, SpellRange};
use crate::srd::magic_items::{ItemSpell, MAX_ATTUNED};
use crate::srd::Ability;

use super::turns::Who;
use super::Game;

/// The slots whose items work while equipped (not the tool, which is only held while used).
const WORKING: [EquipSlot; 13] = [
    EquipSlot::Clothing,
    EquipSlot::Head,
    EquipSlot::Body,
    EquipSlot::Weapon,
    EquipSlot::Offhand,
    EquipSlot::Back,
    EquipSlot::Neck,
    EquipSlot::Cloak,
    EquipSlot::Belt,
    EquipSlot::Feet,
    EquipSlot::Hands,
    EquipSlot::Ring,
    EquipSlot::Ring2,
];

/// Where a stack of a character's items is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StackAt {
    Equip(EquipSlot),
    Bag(usize),
}

/// A spell an item casts, as the Cast window offers it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct ItemCast {
    pub item: String,
    pub spell: ItemSpell,
    /// The slot level it is cast at and the charges that costs.
    pub slot: u32,
    pub charges: u32,
}

impl Game {
    // ------------------------------------------------------------------ what works

    /// The magic items `who` is attuned to.
    pub(super) fn attuned_of(&self, who: Who) -> Vec<String> {
        match who {
            Who::Player => self.world.attuned.clone(),
            Who::Npc(uid) => self.member_save_index(uid).map_or(Vec::new(), |k| self.world.companions[k].attuned.clone()),
        }
    }

    /// Whether the hero is attuned to `item`.
    pub(super) fn is_attuned(&self, item: &str) -> bool {
        self.world.attuned.iter().any(|a| a == item)
    }

    fn equipment_of(&self, who: Who) -> Option<&crate::state::Equipment> {
        match who {
            Who::Player => Some(&self.world.equipment),
            Who::Npc(_) => self.who_index(who).map(|i| &self.loaded.entities[i].equipment),
        }
    }

    /// The magic items working for `who`: equipped (a weapon in hand, armor, rings…), attuned
    /// when they need it.
    pub(super) fn magic_in_effect(&self, who: Who) -> Vec<&ItemDef> {
        let Some(eq) = self.equipment_of(who) else { return Vec::new() };
        let attuned = self.attuned_of(who);
        WORKING
            .iter()
            .filter_map(|s| eq.item(*s))
            .filter_map(|id| self.project.db.item(id))
            .filter(|d| d.magic.is_some() && (!d.needs_attunement() || attuned.contains(&d.id)))
            .collect()
    }

    /// The passive effects of `who`'s working magic items.
    pub(super) fn item_effects(&self, who: Who) -> Vec<FeatureEffect> {
        self.magic_in_effect(who).into_iter().flat_map(|d| d.magic.as_ref().map(|m| m.effects.clone()).unwrap_or_default()).collect()
    }

    /// Class traits plus the items' effects: None only without a class and without item effects.
    pub(super) fn with_items(&self, who: Who, class: Option<ClassTraits>, level: u32) -> Option<ClassTraits> {
        let effects = self.item_effects(who);
        if effects.is_empty() {
            return class;
        }
        let mut t = class.unwrap_or_else(|| ClassTraits { level, ..ClassTraits::plain() });
        t.add_effects(&effects, None, "");
        Some(t)
    }

    /// A flat bonus from `who`'s items (and features).
    pub(super) fn bonus_of(&self, who: Who, to: BonusTo) -> i32 {
        self.who_traits(who).map_or(0, |t| t.bonus(to))
    }

    /// The +N of an item (its `magic.bonus`; 0 = none or not attuned when it must be).
    pub(super) fn item_bonus(&self, who: Who, item: &str) -> i32 {
        let Some(d) = self.project.db.item(item) else { return 0 };
        if d.needs_attunement() && !self.attuned_of(who).iter().any(|a| a == item) {
            return 0;
        }
        d.magic_bonus()
    }

    /// The hero's ability scores raised by items and buffs (a Belt of Giant Strength's 21, a
    /// potion's): `world.abilities` holds the raised scores while `ability_base` keeps the base.
    /// Once a frame.
    pub(super) fn refresh_item_abilities(&mut self) {
        let mut floors: Vec<(Ability, i32)> = self.item_effects(Who::Player).into_iter().filter_map(|e| if let FeatureEffect::AbilityScore { ability, score } = e { Some((ability, score)) } else { None }).collect();
        floors.extend(crate::srd::conditions::buffs(&self.world.effects).filter_map(|b| if let crate::srd::conditions::Buff::AbilityScore { ability, score } = b { Some((*ability, *score)) } else { None }));
        let base = self.world.ability_base.unwrap_or(self.world.abilities);
        if floors.is_empty() {
            if let Some(b) = self.world.ability_base.take() {
                self.world.abilities = b;
            }
            return;
        }
        let mut raised = base;
        for (a, s) in floors {
            if raised.score(a) < s {
                raised.set(a, s);
            }
        }
        self.world.ability_base = Some(base);
        self.world.abilities = raised;
    }

    /// Once a frame: ability scores raised by items, dawn, and the training (save advantages,
    /// checks with advantage, proficiencies) when the worn items or attunements changed.
    pub(super) fn update_magic_items(&mut self) {
        self.refresh_item_abilities();
        self.check_dawn();
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for s in self.world.equipment.stacks() {
            s.item.hash(&mut h);
        }
        self.world.attuned.hash(&mut h);
        let key = h.finish();
        if key != self.magic_key {
            self.magic_key = key;
            self.refresh_training();
        }
    }

    // ------------------------------------------------------------------ attunement

    /// Whether the hero may attune to item `d` (`attune_by`: classes, races, `spellcaster`).
    fn may_attune(&self, d: &ItemDef) -> bool {
        let Some(m) = &d.magic else { return false };
        if m.attune_by.is_empty() {
            return true;
        }
        let class = self.world.class.as_ref().map(|c| c.class.clone()).unwrap_or_default();
        m.attune_by.iter().any(|w| match w.as_str() {
            "spellcaster" => self.spellcasting(Who::Player).is_some(),
            w => w == class || (!self.world.player_race.is_empty() && self.world.player_race.contains(w)),
        })
    }

    /// The hero attunes to `item` (carried or worn): an hour outside fights, at most three.
    pub(super) fn attune(&mut self, item: &str) -> Result<(), &'static str> {
        let d = self.project.db.item(item).cloned().ok_or("No such item")?;
        if !d.needs_attunement() {
            return Err("It needs no attunement");
        }
        if self.is_attuned(item) {
            return Err("Already attuned");
        }
        if self.world.item_count(item) == 0 {
            return Err("You don't have it");
        }
        if self.fight.is_some() {
            return Err("Not in a fight");
        }
        if self.world.attuned.len() >= MAX_ATTUNED {
            return Err("You can attune to at most three items");
        }
        if !self.may_attune(&d) {
            return Err("You can't attune to this item");
        }
        self.world.attuned.push(item.to_string());
        // (The SRD: a short rest focused on the item.)
        self.world.advance_hours(1.0);
        self.refresh_training();
        self.ui.toast(crate::lang::tr_ui("Attuned to {}").replacen("{}", &crate::lang::tr(&d.name), 1));
        Ok(())
    }

    /// The hero ends attunement to `item` (free).
    pub(super) fn end_attunement(&mut self, item: &str) -> bool {
        let before = self.world.attuned.len();
        self.world.attuned.retain(|a| a != item);
        let done = self.world.attuned.len() < before;
        if done {
            self.refresh_training();
            let name = self.project.db.item(item).map_or(item.to_string(), |d| crate::lang::tr(&d.name));
            self.ui.toast(crate::lang::tr_ui("No longer attuned to {}").replacen("{}", &name, 1));
        }
        done
    }

    /// Attune / end attunement, whichever applies (the character window's key).
    pub(super) fn toggle_attunement(&mut self, item: &str) {
        if self.is_attuned(item) {
            self.end_attunement(item);
        } else if let Err(why) = self.attune(item) {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
        }
    }

    /// A party member attunes to an item it wears or holds (an hour, outside fights, three at
    /// most) or ends it (the companion window's key).
    pub(super) fn toggle_member_attunement(&mut self, uid: u32, item: &str) {
        let Some(k) = self.member_save_index(uid) else {
            self.ui.toast(crate::lang::tr_ui("Only party members attune").into_owned());
            return;
        };
        let name = self.project.db.item(item).map_or(item.to_string(), |d| crate::lang::tr(&d.name));
        if self.world.companions[k].attuned.iter().any(|a| a == item) {
            self.world.companions[k].attuned.retain(|a| a != item);
            self.ui.toast(crate::lang::tr_ui("No longer attuned to {}").replacen("{}", &name, 1));
            return;
        }
        let why = match self.project.db.item(item) {
            None => Some("No such item"),
            Some(d) if !d.needs_attunement() => Some("It needs no attunement"),
            _ if self.fight.is_some() => Some("Not in a fight"),
            _ if self.world.companions[k].attuned.len() >= MAX_ATTUNED => Some("You can attune to at most three items"),
            Some(d) => {
                let class = self.member_class(uid).map(|c| c.0.class).unwrap_or_default();
                let ok = d.magic.as_ref().is_some_and(|m| m.attune_by.is_empty() || m.attune_by.iter().any(|w| *w == class || (w == "spellcaster" && self.spellcasting(Who::Npc(uid)).is_some())));
                (!ok).then_some("You can't attune to this item")
            }
        };
        if let Some(why) = why {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
            return;
        }
        self.world.companions[k].attuned.push(item.to_string());
        self.world.advance_hours(1.0);
        self.ui.toast(crate::lang::tr_ui("Attuned to {}").replacen("{}", &name, 1));
    }

    // ------------------------------------------------------------------ charges

    /// The stacks of `item` `who` has (worn first, then the bag) with their charges left.
    fn charged_stacks(&self, who: Who, item: &str) -> Vec<(StackAt, u32)> {
        let max = self.project.db.item(item).map_or(0, |d| d.max_charges());
        let mut out = Vec::new();
        let left = |s: &ItemStack| max.saturating_sub(s.spent);
        if let Some(eq) = self.equipment_of(who) {
            for slot in EquipSlot::ALL {
                if let Some(s) = eq.get(slot).filter(|s| s.item == item) {
                    out.push((StackAt::Equip(slot), left(s)));
                }
            }
        }
        match who {
            Who::Player => {
                for (k, s) in self.world.inventory.slots.iter().enumerate() {
                    if let Some(s) = s.as_ref().filter(|s| s.item == item) {
                        out.push((StackAt::Bag(k), left(s)));
                    }
                }
            }
            Who::Npc(_) => {
                if let Some(i) = self.who_index(who) {
                    for (k, s) in self.loaded.entities[i].inventory.iter().enumerate().filter(|(_, s)| s.item == item) {
                        out.push((StackAt::Bag(k), left(s)));
                    }
                }
            }
        }
        out
    }

    /// The most charges left on one of `who`'s stacks of `item` (None: it has none / no charges).
    pub(super) fn charges_left(&self, who: Who, item: &str) -> Option<u32> {
        if self.project.db.item(item).is_none_or(|d| d.max_charges() == 0) {
            return None;
        }
        self.charged_stacks(who, item).into_iter().map(|s| s.1).max()
    }

    fn stack_mut(&mut self, who: Who, at: StackAt) -> Option<&mut ItemStack> {
        match (who, at) {
            (Who::Player, StackAt::Equip(s)) => self.world.equipment.slot_mut(s).as_mut(),
            (Who::Player, StackAt::Bag(k)) => self.world.inventory.slots.get_mut(k)?.as_mut(),
            (Who::Npc(_), at) => {
                let i = self.who_index(who)?;
                let e = &mut self.loaded.entities[i];
                match at {
                    StackAt::Equip(s) => e.equipment.slot_mut(s).as_mut(),
                    StackAt::Bag(k) => e.inventory.get_mut(k),
                }
            }
        }
    }

    /// Spends `n` charges of `who`'s `item` (the stack with the most left). The last charge of
    /// an item that can be destroyed rolls a d20: on a 1 it crumbles.
    pub(super) fn spend_charges(&mut self, who: Who, item: &str, n: u32) -> Result<(), &'static str> {
        let Some(d) = self.project.db.item(item).cloned() else { return Err("No such item") };
        let max = d.max_charges();
        let Some((at, left)) = self.charged_stacks(who, item).into_iter().max_by_key(|s| s.1) else { return Err("You don't have it") };
        if left < n {
            return Err("No charges left");
        }
        if let Some(s) = self.stack_mut(who, at) {
            s.spent = (s.spent + n).min(max);
        }
        if left == n && d.magic.as_ref().is_some_and(|m| m.destroy_on_last) && crate::srd::dice::die(20) == 1 {
            self.destroy_stack(who, at);
            self.ui.toast(crate::lang::tr_ui("The {} crumbles to dust").replacen("{}", &crate::lang::tr(&d.name), 1));
        } else if left == n && d.magic.is_none() && d.uses > 0 {
            // A plain item's last use (a healer's kit) uses it up.
            self.destroy_stack(who, at);
        }
        if let Some(i) = self.who_index(who) {
            self.persist_npc_items(i);
        }
        Ok(())
    }

    fn destroy_stack(&mut self, who: Who, at: StackAt) {
        match (who, at) {
            (Who::Player, StackAt::Equip(s)) => *self.world.equipment.slot_mut(s) = None,
            (Who::Player, StackAt::Bag(k)) => {
                self.world.inventory.take_from_slot(k, Some(1));
            }
            (Who::Npc(_), at) => {
                let Some(i) = self.who_index(who) else { return };
                let e = &mut self.loaded.entities[i];
                match at {
                    StackAt::Equip(s) => *e.equipment.slot_mut(s) = None,
                    StackAt::Bag(k) => {
                        if k < e.inventory.len() {
                            e.inventory.remove(k);
                        }
                    }
                }
                crate::runtime::npc_items::apply_gear(&self.project.db, &mut self.loaded.entities[i]);
            }
        }
    }

    /// Sets the charges left on the hero's first stack of `item` (scripts, the console).
    pub(super) fn set_charges(&mut self, item: &str, left: u32) -> bool {
        let max = self.project.db.item(item).map_or(0, |d| d.max_charges());
        let Some((at, _)) = self.charged_stacks(Who::Player, item).into_iter().next() else { return false };
        if let Some(s) = self.stack_mut(Who::Player, at) {
            s.spent = max.saturating_sub(left.min(max));
            return true;
        }
        false
    }

    /// Dawn: every charged item the hero and the party here carry regains its charges.
    pub(super) fn recharge_items(&mut self) {
        let db = &self.project.db;
        let regain = |s: &mut ItemStack| {
            if s.spent == 0 {
                return;
            }
            if let Some(m) = db.item(&s.item).and_then(|d| d.magic.as_ref()) {
                s.spent = m.recharged(s.spent, |dice| crate::srd::Expr::parse(dice).map(|e| e.roll(&crate::srd::dice::NoMods).total.max(0) as u32).unwrap_or(0));
            }
        };
        for s in self.world.equipment.stacks_mut() {
            regain(s);
        }
        for s in self.world.inventory.slots.iter_mut().flatten() {
            regain(s);
        }
        let members: Vec<usize> = (0..self.loaded.entities.len()).filter(|&i| self.loaded.entities[i].companion.is_some()).collect();
        for &i in &members {
            let e = &mut self.loaded.entities[i];
            for s in e.equipment.stacks_mut() {
                regain(s);
            }
            for s in e.inventory.iter_mut() {
                regain(s);
            }
        }
        for i in members {
            self.persist_npc_items(i);
        }
    }

    /// Recharges the items when the clock passed dawn (6:00) since the last check. Once a frame.
    pub(super) fn check_dawn(&mut self) {
        let dawn = ((self.world.total_hours() - 6.0) / 24.0).floor() as i64;
        match self.world.last_dawn {
            Some(d) if dawn > d => {
                self.world.last_dawn = Some(dawn);
                self.recharge_items();
            }
            Some(_) => {}
            None => self.world.last_dawn = Some(dawn),
        }
    }

    // ------------------------------------------------------------------ using

    /// The spell an item's use parts make (resolved like a spell on its user or target).
    fn use_spell(d: &ItemDef) -> Option<SpellDef> {
        let m = d.magic.as_ref().filter(|m| !m.use_parts.is_empty())?;
        let mut s = SpellDef::fresh(&format!("item:{}", d.id));
        s.name = d.name.clone();
        s.level = 1;
        s.components = String::new();
        s.range = SpellRange::Touch;
        s.parts = m.use_parts.clone();
        s.duration_rounds = (m.use_seconds / crate::srd::conditions::ROUND).round() as u32;
        s.duration = String::new();
        Some(s)
    }

    /// Whether using the item does something by its magic (its use parts heal: "heal whom?").
    pub(super) fn item_use_heals(&self, item: &str) -> bool {
        self.project.db.item(item).and_then(Self::use_spell).is_some_and(|s| s.heals())
    }

    /// Uses a magic item's parts on `target` (the hero or a party member). False when it has none.
    pub(super) fn use_magic_item(&mut self, item: &str, target: Who) -> bool {
        let Some(s) = self.project.db.item(item).and_then(Self::use_spell) else { return false };
        self.resolve_spell(Who::Player, &s, 1, &[target], None);
        if self.fight.is_some() {
            let name = self.who_name(Who::Player);
            self.fight_log(format!("{name}: {}", crate::lang::tr(&s.name)));
        }
        true
    }

    // ------------------------------------------------------------------ spells from items

    /// The spells `who`'s items cast now: each slot level it can reach with the charges left
    /// (a wand of Magic Missiles: slot 1 for 1 charge, slot 2 for 2…). Worn items and the bag
    /// (a wand is held to use it: drawn as the action).
    pub(super) fn item_casts(&self, who: Who) -> Vec<(ItemCast, Option<&'static str>)> {
        let mut ids: Vec<String> = Vec::new();
        if let Some(eq) = self.equipment_of(who) {
            ids.extend(EquipSlot::ALL.iter().filter_map(|s| eq.item(*s).map(str::to_string)));
        }
        match who {
            Who::Player => ids.extend(self.world.inventory.item_ids()),
            Who::Npc(_) => ids.extend(self.who_index(who).map(|i| self.loaded.entities[i].inventory.iter().map(|s| s.item.clone()).collect::<Vec<_>>()).unwrap_or_default()),
        }
        let mut seen = Vec::new();
        let attuned = self.attuned_of(who);
        let mut out = Vec::new();
        for id in ids {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());
            let Some(d) = self.project.db.item(&id) else { continue };
            let Some(m) = &d.magic else { continue };
            let left = if m.charges == 0 { u32::MAX } else { self.charges_left(who, &id).unwrap_or(0) };
            let why = (m.attunement && !attuned.contains(&id)).then_some("Not attuned");
            for sp in &m.spells {
                let Some(sd) = self.project.db.spell(&sp.spell) else { continue };
                let base = if sp.slot > 0 { sp.slot } else { sd.level };
                let top = if sp.upcast_to > base { sp.upcast_to } else { base };
                for slot in base..=top {
                    let charges = sp.charges + (slot - base);
                    let why = why.or((charges > left).then_some("No charges left"));
                    out.push((ItemCast { item: id.clone(), spell: sp.clone(), slot, charges }, why));
                    // (Only the first unaffordable step is shown.)
                    if charges > left {
                        break;
                    }
                }
            }
        }
        out
    }

    /// Use on a wand in a fight: the Cast window with its spells (on the hero's turn).
    pub(super) fn open_item_casts(&mut self, item: &str) {
        if !self.players_turn() {
            return;
        }
        let who = self.fight.as_ref().map_or(Who::Player, |c| c.current().who);
        let rows = self.item_cast_rows(who, Some(item));
        if !self.open_fight_menu(super::turns::menu::MenuKind::Cast, rows) {
            self.ui.toast(crate::lang::tr_ui("Nothing happens").into_owned());
        }
    }

    /// Casting a spell from an item: its DC (0 = the caster's own) while it resolves.
    pub(super) fn item_dc(&self) -> Option<u32> {
        self.casting_item.as_ref().map(|c| c.spell.dc).filter(|dc| *dc > 0)
    }

    /// Casts an item's spell outside a fight (Use on a wand, a staff): charges instead of a slot.
    pub(super) fn cast_from_item_now(&mut self, item: &str) -> Result<(), &'static str> {
        let (cast, why) = self.item_casts(Who::Player).into_iter().find(|(c, _)| c.item == item).ok_or("Nothing happens")?;
        if let Some(why) = why {
            return Err(why);
        }
        self.casting_item = Some(cast.clone());
        let r = self.cast_out_of_fight(&cast.spell.spell, Some(cast.slot), true);
        self.casting_item = None;
        r?;
        if cast.charges > 0 {
            self.spend_charges(Who::Player, item, cast.charges)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------ autoplay

    /// Autoplay `attune ITEM`, `unattune ITEM`, `use ITEM`, `equip ITEM`: what happened.
    pub fn item_command(&mut self, cmd: &str, item: &str) -> String {
        match cmd {
            "attune" => match self.attune(item) {
                Ok(()) => "ok".into(),
                Err(why) => why.into(),
            },
            "unattune" => if self.end_attunement(item) { "ok".into() } else { "not attuned".into() },
            "equip" => if self.equip_item(item) { "ok".into() } else { "can't".into() },
            _ => {
                if self.world.item_count(item) == 0 {
                    return "you don't have it".into();
                }
                self.use_item_by_id(item);
                "used".into()
            }
        }
    }

    // ------------------------------------------------------------------ status

    /// `attuned=` for status: the hero's attuned items.
    pub(super) fn attuned_status(&self) -> String {
        if self.world.attuned.is_empty() { "-".into() } else { self.world.attuned.join(",") }
    }

    /// `charges=` for status: item:left/max of the hero's charged items.
    pub(super) fn charges_status(&self) -> String {
        let mut ids: Vec<String> = self.world.equipment.stacks().map(|s| s.item.clone()).chain(self.world.inventory.item_ids()).collect();
        ids.dedup();
        let mut seen = Vec::new();
        let v: Vec<String> = ids
            .into_iter()
            .filter(|id| {
                let new = !seen.contains(id);
                seen.push(id.clone());
                new
            })
            .filter_map(|id| {
                let max = self.project.db.item(&id)?.max_charges();
                (max > 0).then(|| format!("{id}:{}/{max}", self.charges_left(Who::Player, &id).unwrap_or(0)))
            })
            .collect();
        if v.is_empty() { "-".into() } else { v.join(",") }
    }
}
