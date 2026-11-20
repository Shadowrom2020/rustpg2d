//! Spellcasting (SRD 5.1) of the hero and party members: their class's spellcasting
//! ([`crate::classes::Spellcasting`]), spell slots by class level (pact magic: the warlock's come
//! back after a short rest), the spells they know (cantrips, a wizard's spellbook) and prepare,
//! the spell save DC and attack bonus, concentration, scrolls, and casting outside fights (the
//! Spells tab, the quick bar, `cast_spell`). Casting in a fight and the parts' effects are
//! `turns/casting.rs`.

use super::turns::Who;
use super::Game;
use crate::classes::{CasterKind, ClassDef, Spellcasting};
use crate::skills::SpellDef;

/// The SRD's component pouch and spellcasting foci (arcane, holy symbols, druidic): one in the
/// bag lets a caster cast spells with material components.
use crate::srd::equipment::FOCUS;

impl Game {
    // ------------------------------------------------------------------- the caster

    /// A combatant's class level (the hero's level; a party member's).
    pub(super) fn who_class_level(&self, who: Who) -> u32 {
        match who {
            Who::Player => self.world.player_level(),
            Who::Npc(uid) => self.member_class(uid).map_or(1, |m| m.1),
        }
    }

    /// A monster's own spellcasting (its stat block's), when it has no class to cast with.
    pub(super) fn monster_casting(&self, who: Who) -> Option<crate::srd::StatSpellcasting> {
        if who == Who::Player || self.spellcasting(who).is_some() {
            return None;
        }
        self.who_block(who)?.spellcasting
    }

    fn monster_key(&self, who: Who) -> Option<(String, u32)> {
        match who {
            Who::Npc(uid) => Some((self.loaded.map.name.clone(), uid)),
            Who::Player => None,
        }
    }

    /// A monster's spells it can cast now: (spell, slot level; an innate one at its level).
    fn monster_cast_options(&self, who: Who, sc: &crate::srd::StatSpellcasting) -> Vec<(String, u32)> {
        let spent = self.monster_key(who).and_then(|k| self.skills.monster_spent.get(&k)).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for s in &sc.spells {
            if self.project.db.spell(&s.spell).is_none_or(|d| d.story_only) {
                continue;
            }
            if s.innate {
                if s.per_day == 0 || spent.1.get(&s.spell).copied().unwrap_or(0) < s.per_day {
                    out.push((s.spell.clone(), s.level));
                }
            } else if s.level == 0 {
                out.push((s.spell.clone(), 0));
            } else {
                for l in s.level..=sc.slots.len() as u32 {
                    if sc.slots[l as usize - 1] > spent.0[l as usize - 1] {
                        out.push((s.spell.clone(), l));
                    }
                }
            }
        }
        out.dedup();
        out
    }

    /// A casting monster's slots left as `1:2,2:1` (None = no slots).
    pub(super) fn monster_slots_left(&self, who: Who) -> Option<String> {
        let sc = self.monster_casting(who)?;
        if sc.slots.is_empty() {
            return None;
        }
        let spent = self.monster_key(who).and_then(|k| self.skills.monster_spent.get(&k)).map(|s| s.0).unwrap_or_default();
        Some(sc.slots.iter().enumerate().map(|(k, n)| format!("{}:{}", k + 1, n.saturating_sub(spent[k]))).collect::<Vec<_>>().join(","))
    }

    /// A monster pays for a cast: an innate spell's use, else a slot of `slot`'s level.
    pub(super) fn monster_pay(&mut self, who: Who, spell: &str, slot: u32) {
        let Some(sc) = self.monster_casting(who) else { return };
        let Some(key) = self.monster_key(who) else { return };
        let innate = sc.spells.iter().any(|s| s.spell == spell && s.innate) && !sc.spells.iter().any(|s| s.spell == spell && !s.innate);
        let e = self.skills.monster_spent.entry(key).or_default();
        if innate {
            *e.1.entry(spell.to_string()).or_insert(0) += 1;
        } else if (1..=9).contains(&slot) {
            e.0[slot as usize - 1] += 1;
        }
    }

    /// The class a combatant casts with, its spellcasting and its level.
    pub(super) fn spellcasting(&self, who: Who) -> Option<(&ClassDef, &Spellcasting, u32)> {
        let c = match who {
            Who::Player => self.hero_class()?,
            Who::Npc(uid) => self.project.db.class(&self.member_class(uid)?.0.class)?,
        };
        let sc = c.spellcasting.as_ref()?;
        Some((c, sc, self.who_class_level(who)))
    }

    fn caster_abilities(&self, who: Who) -> crate::srd::Abilities {
        match who {
            Who::Player => self.world.abilities,
            Who::Npc(uid) => self.member_block(uid).map(|b| b.abilities).unwrap_or_default(),
        }
    }

    /// The spellcasting ability's modifier (0 without spellcasting).
    pub(super) fn spell_mod(&self, who: Who) -> i32 {
        if let Some(sc) = self.monster_casting(who) {
            return self.who_block(who).map_or(0, |b| b.abilities.modifier(sc.ability));
        }
        self.spellcasting(who).map_or(0, |(_, sc, _)| self.caster_abilities(who).modifier(sc.ability))
    }

    fn spell_prof(&self, who: Who) -> i32 {
        crate::srd::abilities::proficiency_bonus(self.who_class_level(who))
    }

    /// The spell save DC: 8 + proficiency + the spellcasting modifier.
    /// (A spell cast from an item with its own DC: that one.)
    pub(super) fn spell_dc(&self, who: Who) -> i32 {
        if let Some(dc) = self.item_dc() {
            return dc as i32;
        }
        if let Some(sc) = self.monster_casting(who) {
            return sc.dc;
        }
        8 + self.spell_prof(who) + self.spell_mod(who) + self.bonus_of(who, crate::classes::BonusTo::SpellDc)
    }

    /// The spell attack bonus: proficiency + the spellcasting modifier (an item's DC - 8).
    pub(super) fn spell_attack_bonus(&self, who: Who) -> i32 {
        if let Some(dc) = self.item_dc() {
            return dc as i32 - 8;
        }
        if let Some(sc) = self.monster_casting(who) {
            return sc.attack;
        }
        self.spell_prof(who) + self.spell_mod(who) + self.bonus_of(who, crate::classes::BonusTo::SpellAttack)
    }

    // ------------------------------------------------------------------------ slots

    /// The spell slots of each level (index 0 = level 1) at the caster's class level.
    pub(super) fn slot_max(&self, who: Who) -> [u32; 9] {
        let mut out = [0; 9];
        if let Some((c, _, level)) = self.spellcasting(who) {
            for (k, slot) in out.iter_mut().enumerate() {
                *slot = c.number(level, &format!("spellcasting.spell_slots_level_{}", k + 1)).max(0) as u32;
            }
        }
        out
    }

    fn slots_spent_of(&self, who: Who) -> [u32; 9] {
        match who {
            Who::Player => self.world.slots_spent,
            Who::Npc(uid) => self.member_save_index(uid).map_or([0; 9], |k| self.world.companions[k].slots_spent),
        }
    }

    /// Slots left of each level.
    pub(super) fn slots_left(&self, who: Who) -> [u32; 9] {
        let (max, spent) = (self.slot_max(who), self.slots_spent_of(who));
        std::array::from_fn(|k| max[k].saturating_sub(spent[k]))
    }

    pub(super) fn spend_slot(&mut self, who: Who, level: u32) {
        if !(1..=9).contains(&level) {
            return;
        }
        let k = level as usize - 1;
        match who {
            Who::Player => self.world.slots_spent[k] += 1,
            Who::Npc(uid) => {
                if let Some(i) = self.member_save_index(uid) {
                    self.world.companions[i].slots_spent[k] += 1;
                }
            }
        }
    }

    /// The highest spell level the caster has slots of (0 = cantrips only, or none).
    pub(super) fn max_spell_level(&self, who: Who) -> u32 {
        self.slot_max(who).iter().rposition(|n| *n > 0).map_or(0, |k| k as u32 + 1)
    }

    /// The lowest slot level at or above `level` with a slot left.
    pub(super) fn lowest_slot(&self, who: Who, level: u32) -> Option<u32> {
        let left = self.slots_left(who);
        (level.max(1)..=9).find(|l| left[*l as usize - 1] > 0)
    }

    /// A rest gives slots back: a short rest the pact magic's, a long rest all (the hero's and
    /// the party's).
    pub(super) fn rest_slots(&mut self, long: bool) {
        let pact = |g: &Game, who: Who| g.spellcasting(who).is_some_and(|(_, sc, _)| sc.kind == CasterKind::Pact);
        if long || pact(self, Who::Player) {
            self.world.slots_spent = [0; 9];
        }
        let members: Vec<(u32, usize)> = self.loaded.entities.iter().filter(|e| e.companion.is_some()).filter_map(|e| Some((e.uid, self.member_save_index(e.uid)?))).collect();
        let on_map: Vec<usize> = members.iter().map(|m| m.1).collect();
        for (uid, k) in members {
            if long || pact(self, Who::Npc(uid)) {
                self.world.companions[k].slots_spent = [0; 9];
            }
        }
        if long {
            for (k, c) in self.world.companions.iter_mut().enumerate() {
                if !on_map.contains(&k) {
                    c.slots_spent = [0; 9];
                }
            }
        }
    }

    /// The saved entry of party member `uid`.
    pub(super) fn member_save_index(&self, uid: u32) -> Option<usize> {
        let l = self.loaded.entities.iter().find(|e| e.uid == uid)?.companion.as_ref()?;
        self.world.companions.iter().position(|c| c.uid == l.uid && c.map == l.map)
    }

    // ----------------------------------------------------------------------- spells

    /// The spells a caster knows: its cantrips, known spells, a wizard's spellbook (the hero's
    /// learned ones; a party member's).
    pub(super) fn known_of(&self, who: Who) -> Vec<String> {
        match who {
            Who::Player => self.world.spells.iter().cloned().collect(),
            Who::Npc(uid) => self.member_save_index(uid).map_or(Vec::new(), |k| self.world.companions[k].spells.clone()),
        }
    }

    fn prepared_list(&self, who: Who) -> Vec<String> {
        match who {
            Who::Player => self.world.prepared.clone(),
            Who::Npc(uid) => self.member_save_index(uid).map_or(Vec::new(), |k| self.world.companions[k].prepared.clone()),
        }
    }

    /// Whether `spell` is on the caster's class list.
    pub(super) fn on_list(&self, who: Who, spell: &SpellDef) -> bool {
        self.spellcasting(who).is_some_and(|(c, _, _)| spell.classes.contains(&c.id))
    }

    /// How many spells a preparing caster prepares: the modifier + its level (half its level for
    /// a half caster), at least 1; 0 for casters that know their spells.
    pub(super) fn prepare_max(&self, who: Who) -> usize {
        let Some((_, sc, level)) = self.spellcasting(who) else { return 0 };
        if !matches!(sc.kind, CasterKind::Prepared | CasterKind::Spellbook) {
            return 0;
        }
        let lv = if sc.half { level / 2 } else { level } as i32;
        (self.caster_abilities(who).modifier(sc.ability) + lv).max(1) as usize
    }

    /// The spells a preparing caster may prepare: its class list (a wizard: its spellbook) of
    /// levels it has slots of.
    pub(super) fn preparable(&self, who: Who) -> Vec<String> {
        let Some((c, sc, _)) = self.spellcasting(who) else { return Vec::new() };
        let max = self.max_spell_level(who);
        let known = self.known_of(who);
        self.project
            .db
            .spells
            .iter()
            .filter(|d| d.level >= 1 && d.level <= max && !d.story_only && d.classes.contains(&c.id))
            .filter(|d| sc.kind != CasterKind::Spellbook || known.contains(&d.id))
            .map(|d| d.id.clone())
            .collect()
    }

    /// The prepared spells: the chosen ones, or (none chosen yet) the first of what it may
    /// prepare, so a new caster can cast at once.
    pub(super) fn prepared_of(&self, who: Who) -> Vec<String> {
        let max = self.prepare_max(who);
        if max == 0 {
            return Vec::new();
        }
        let chosen = self.prepared_list(who);
        if !chosen.is_empty() {
            return chosen;
        }
        let mut options: Vec<String> = self.preparable(who);
        // (Healing and damage first: what a new caster wants at hand.)
        options.sort_by_key(|id| self.project.db.spell(id).map_or((9, 1), |d| (d.level, (!d.heals() && !d.offensive()) as u32)));
        options.truncate(max);
        options
    }

    /// Prepares or unprepares a spell (`Err` = why not).
    pub(super) fn set_prepared(&mut self, who: Who, id: &str, on: bool) -> Result<(), &'static str> {
        let mut list = self.prepared_of(who);
        if on {
            if list.iter().any(|x| x == id) {
                return Ok(());
            }
            if !self.preparable(who).iter().any(|x| x == id) {
                return Err("You can't prepare that spell");
            }
            if list.len() >= self.prepare_max(who) {
                return Err("No more spells can be prepared");
            }
            list.push(id.to_string());
        } else {
            list.retain(|x| x != id);
        }
        match who {
            Who::Player => self.world.prepared = list,
            Who::Npc(uid) => {
                if let Some(k) = self.member_save_index(uid) {
                    self.world.companions[k].prepared = list;
                }
            }
        }
        Ok(())
    }

    /// Adds spells to what a caster knows (level-up picks, a wizard copying a scroll).
    pub(super) fn add_known(&mut self, who: Who, ids: &[String]) {
        match who {
            Who::Player => self.world.spells.extend(ids.iter().cloned()),
            Who::Npc(uid) => {
                if let Some(k) = self.member_save_index(uid) {
                    let s = &mut self.world.companions[k].spells;
                    for id in ids {
                        if !s.contains(id) {
                            s.push(id.clone());
                        }
                    }
                }
            }
        }
    }

    /// What a caster can cast now, with the slot levels it can use: cantrips (slot 0), its
    /// known (a known caster's) or prepared spells at each level with a slot left.
    /// Why `who` can't cast spell `id` now: armor it isn't trained in, a material component
    /// without a pouch or focus, no slot left; else `other` (not known or prepared).
    pub(super) fn cast_refusal(&self, who: Who, id: &str, other: &'static str) -> &'static str {
        if self.who_armor_untrained(who) {
            "Untrained armor"
        } else if !self.has_focus(who) && self.project.db.spell(id).is_some_and(|d| d.components.contains('M')) {
            "Needs a component pouch or a focus"
        } else if self.known_or_prepared(who, id) {
            "No spell slot left"
        } else {
            other
        }
    }

    pub(super) fn cast_options(&self, who: Who) -> Vec<(String, u32)> {
        // (A monster's stat block spells: its slots and innate spells, no focus needed.)
        if let Some(sc) = self.monster_casting(who) {
            return self.monster_cast_options(who, &sc);
        }
        let Some((_, sc, _)) = self.spellcasting(who) else { return Vec::new() };
        // (Armor without proficiency: no spells at all.)
        if self.who_armor_untrained(who) {
            return Vec::new();
        }
        let focus = self.has_focus(who);
        let known = self.known_of(who);
        let leveled: Vec<String> = match sc.kind {
            CasterKind::Known | CasterKind::Pact => known.clone(),
            CasterKind::Prepared | CasterKind::Spellbook => self.prepared_of(who),
        };
        let left = self.slots_left(who);
        let mut out = Vec::new();
        for d in &self.project.db.spells {
            if d.story_only || (!focus && d.components.contains('M')) {
                continue;
            }
            if d.level == 0 {
                if known.contains(&d.id) {
                    out.push((d.id.clone(), 0));
                }
            } else if leveled.contains(&d.id) {
                for l in d.level..=9 {
                    if left[l as usize - 1] > 0 {
                        out.push((d.id.clone(), l));
                    }
                }
            }
        }
        out
    }

    /// Whether a caster has a component pouch or a spellcasting focus (arcane, holy, druidic) for
    /// spells with material components; a project without such items needs none.
    pub(super) fn has_focus(&self, who: Who) -> bool {
        let is_focus = |id: &str| self.project.db.item(id).is_some_and(|d| FOCUS.contains(&d.srd.as_str()));
        if !self.project.db.items.iter().any(|d| FOCUS.contains(&d.srd.as_str())) {
            return true;
        }
        match who {
            Who::Player => self.world.inventory.slots.iter().flatten().any(|s| is_focus(&s.item)) || crate::defs::EquipSlot::ALL.iter().any(|s| self.world.equipment.item(*s).is_some_and(is_focus)),
            Who::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).is_some_and(|e| e.inventory.iter().any(|s| is_focus(&s.item)) || crate::defs::EquipSlot::ALL.iter().any(|s| e.equipment.item(*s).is_some_and(is_focus))),
        }
    }

    /// Whether a caster can cast `id` at all now (a slot left; a cantrip it knows).
    pub(super) fn can_cast(&self, who: Who, id: &str) -> bool {
        self.cast_options(who).iter().any(|(s, _)| s == id)
    }

    /// The level a spell's dice scale with: a cantrip's the caster's level, else the slot's.
    pub(super) fn scale_level(&self, who: Who, d: &SpellDef, slot: u32) -> u32 {
        if d.level == 0 {
            // (A monster's cantrips grow with its spellcaster level.)
            self.monster_casting(who).map_or_else(|| self.who_class_level(who), |sc| sc.level.max(1))
        } else {
            slot.max(d.level)
        }
    }

    /// Dice with `MOD` replaced by the caster's spellcasting modifier ("1d8 + MOD" -> "1d8+3").
    pub(super) fn spell_dice(&self, who: Who, dice: &str) -> String {
        let m = self.spell_mod(who);
        let d = dice.replace(' ', "");
        d.replace("+MOD", &format!("{m:+}")).replace("MOD", &m.to_string())
    }

    // ---------------------------------------------------------------- concentration

    /// The spell `who` concentrates on (its effects' source `spell@caster`).
    pub(super) fn concentration_of(&self, who: Who) -> Option<String> {
        match who {
            Who::Player => self.world.concentration.as_ref().map(|c| c.0.clone()),
            Who::Npc(uid) => self.skills.npc_concentration.get(&uid).map(|c| c.0.clone()),
        }
    }

    /// Starts concentrating on a spell (the old one ends) for `seconds`.
    pub(super) fn begin_concentration(&mut self, who: Who, source: &str, seconds: f32) {
        self.end_concentration(who);
        match who {
            Who::Player => self.world.concentration = Some((source.to_string(), seconds)),
            Who::Npc(uid) => {
                self.skills.npc_concentration.insert(uid, (source.to_string(), seconds));
            }
        }
    }

    /// Ends `who`'s concentration: what the spell gave anyone ends with it.
    pub(super) fn end_concentration(&mut self, who: Who) {
        let source = match who {
            Who::Player => self.world.concentration.take().map(|c| c.0),
            Who::Npc(uid) => self.skills.npc_concentration.remove(&uid).map(|c| c.0),
        };
        let Some(source) = source else { return };
        crate::srd::conditions::remove_source(&mut self.world.effects, &source);
        for e in &mut self.loaded.entities {
            crate::srd::conditions::remove_source(&mut e.effects, &source);
        }
        if source.starts_with("light@") || self.project.db.spell(source.split('@').next().unwrap_or("")).is_some_and(|d| d.parts.iter().any(|p| matches!(p, crate::skills::SpellPart::Light { .. }))) {
            self.skills.light = None;
        }
    }

    /// Concentration's time runs (outside fights by the clock, in fights by rounds).
    pub(super) fn tick_concentration(&mut self, dt: f32) {
        if let Some((_, t)) = &mut self.world.concentration {
            *t -= dt;
            if *t <= 0.0 {
                self.end_concentration(Who::Player);
            }
        }
        let over: Vec<u32> = self
            .skills
            .npc_concentration
            .iter_mut()
            .filter_map(|(uid, (_, t))| {
                *t -= dt;
                (*t <= 0.0).then_some(*uid)
            })
            .collect();
        for uid in over {
            self.end_concentration(Who::Npc(uid));
        }
    }

    /// A caster that can't act (incapacitated, stunned, unconscious at 0 HP…) loses its
    /// concentration.
    pub(super) fn drop_concentration_when_incapacitated(&mut self) {
        if self.world.concentration.is_some() && !crate::srd::conditions::can_act(&self.world.effects) {
            self.end_concentration(Who::Player);
        }
        let out: Vec<u32> = self
            .skills
            .npc_concentration
            .keys()
            .copied()
            .filter(|uid| self.loaded.entities.iter().find(|e| e.uid == *uid).is_some_and(|e| !crate::srd::conditions::can_act(&e.effects)))
            .collect();
        for uid in out {
            self.end_concentration(Who::Npc(uid));
        }
    }

    // --------------------------------------------------------------- outside fights

    /// Casts a spell outside a fight (the Spells tab, the quick bar, `cast_spell`): healing and
    /// buffs on the hero (or whom a heal picks), light, Knock on the lock ahead, a ritual
    /// without a slot (10 minutes pass); attacks and saves wait for a fight. `slot`: the slot
    /// level (None = the lowest left); `free`: no slot spent (a scroll, an event).
    pub fn cast_out_of_fight(&mut self, id: &str, slot: Option<u32>, free: bool) -> Result<(), &'static str> {
        let d = self.project.db.spell(id).cloned().ok_or("No such spell")?;
        if self.fight.is_some() {
            return Err("In a fight: use the action bar");
        }
        if d.offensive() && !d.heals() {
            return Err("Only in a fight");
        }
        if d.story_only && !free {
            return Err("Nothing happens");
        }
        // A ritual: no slot, ten minutes.
        let ritual = !free && d.ritual && self.spellcasting(Who::Player).is_some_and(|(_, sc, _)| sc.ritual) && self.known_or_prepared(Who::Player, id);
        let slot = if free || d.level == 0 || ritual {
            slot.unwrap_or(d.level)
        } else {
            if !self.can_cast(Who::Player, id) {
                return Err(self.cast_refusal(Who::Player, id, "You can't cast that spell"));
            }
            let s = match slot {
                Some(s) if s >= d.level && self.slots_left(Who::Player)[s.clamp(1, 9) as usize - 1] > 0 => s,
                Some(_) => return Err("No spell slot left"),
                None => self.lowest_slot(Who::Player, d.level).ok_or("No spell slot left")?,
            };
            self.spend_slot(Who::Player, s);
            s
        };
        if ritual {
            self.world.advance_hours(10.0 / 60.0);
        }
        self.resolve_spell(Who::Player, &d, slot, &[Who::Player], None);
        self.ui.toast(crate::lang::tr(&d.name));
        Ok(())
    }

    /// *Cast spell* / `cast_spell`: the hero casts `spell` on `target` ("player" or a character
    /// on this map), with a slot unless `free`.
    pub(super) fn cast_spell_action(&mut self, spell: &str, target: &str, free: bool, owner: Option<u32>) {
        let Some(d) = self.project.db.spell(spell).cloned() else {
            eprintln!("[game] Cast spell: no spell '{spell}'");
            return;
        };
        let who = if target.is_empty() || target.eq_ignore_ascii_case("player") {
            Who::Player
        } else {
            match self.entity_index(target, owner) {
                Some(i) => Who::Npc(self.loaded.entities[i].uid),
                None => {
                    eprintln!("[game] Cast spell: no character '{target}' on this map");
                    return;
                }
            }
        };
        let slot = if free || d.level == 0 {
            d.level
        } else {
            match self.lowest_slot(Who::Player, d.level) {
                Some(s) if self.known_or_prepared(Who::Player, spell) => {
                    self.spend_slot(Who::Player, s);
                    s
                }
                _ => {
                    self.ui.toast(format!("{}: {}", crate::lang::tr(&d.name), crate::lang::tr_ui("No spell slot left")));
                    return;
                }
            }
        };
        let at = self.who_tile(who);
        self.spell_impact(&d, crate::grid::feet_spot(at, self.ts()));
        self.resolve_spell(Who::Player, &d, slot, &[who], Some(at));
    }

    /// Whether the caster knows (or has prepared) a spell: what it may cast with a slot or as a ritual.
    pub(super) fn known_or_prepared(&self, who: Who, id: &str) -> bool {
        self.known_of(who).iter().any(|x| x == id) || self.prepared_of(who).iter().any(|x| x == id)
    }

    /// A spell scroll or book (an item that `teaches`): a wizard with the spell on its list copies
    /// it into the spellbook; another caster with it on its list casts it once from a scroll (a
    /// `consumable` item; a book only teaches wizards); anyone else can't use it. Returns true
    /// when the item is used up.
    pub(super) fn read_scroll(&mut self, spell: &str, consumable: bool) -> bool {
        let Some(d) = self.project.db.spell(spell).cloned() else {
            eprintln!("[game] unknown spell '{spell}'");
            return false;
        };
        let name = crate::lang::tr(&d.name);
        if !self.on_list(Who::Player, &d) {
            self.ui.toast(crate::lang::tr_ui("You can't make sense of this scroll").into_owned());
            return false;
        }
        let wizard = self.spellcasting(Who::Player).is_some_and(|(_, sc, _)| sc.kind == CasterKind::Spellbook);
        if wizard && d.level > self.max_spell_level(Who::Player) {
            self.ui.toast(crate::lang::tr_ui("This spell is beyond you yet").into_owned());
            return false;
        }
        if wizard && d.level >= 1 && !self.world.spells.contains(spell) {
            self.add_known(Who::Player, &[spell.to_string()]);
            self.audio.play(&self.project.config.sfx.pickup);
            self.ui.toast(crate::lang::tr_ui("Copied into your spellbook: {}").replace("{}", &name));
            return true;
        }
        if !consumable {
            let msg = if wizard { crate::lang::tr_ui("You already know {}").replace("{}", &name) } else { crate::lang::tr_ui("Only a wizard can learn from this book").into_owned() };
            self.ui.toast(msg);
            return false;
        }
        if self.fight.is_some() {
            return self.scroll_in_fight(spell);
        }
        match self.cast_out_of_fight(spell, Some(d.level), true) {
            Ok(()) => true,
            Err(why) => {
                self.ui.toast(crate::lang::tr_ui(why).into_owned());
                false
            }
        }
    }

    /// The level-up's spell picks of class `c` at `level` (see [`ClassDef::spell_gains`]).
    pub(super) fn spell_gains(c: &ClassDef, level: u32) -> (u32, u32) {
        c.spell_gains(level)
    }

    /// The spells of class `c`'s list a level-up offers: cantrips, or spells of levels with a slot at `level`.
    pub(super) fn spell_options(&self, c: &ClassDef, level: u32, cantrips: bool, known: &[String]) -> Vec<(String, String)> {
        let max = (1..=9u32).rev().find(|l| c.number(level, &format!("spellcasting.spell_slots_level_{l}")) > 0).unwrap_or(0);
        self.project
            .db
            .spells
            .iter()
            .filter(|d| !d.story_only && d.classes.contains(&c.id) && !known.contains(&d.id))
            .filter(|d| if cantrips { d.level == 0 } else { d.level >= 1 && d.level <= max })
            .map(|d| (d.id.clone(), format!("{} ({})", d.name, if d.level == 0 { "cantrip".to_string() } else { format!("{}", d.level) })))
            .collect()
    }

    /// `slots=1:2/3,2:0/2 prepared=[…] concentration=…` (status): the hero's.
    pub(super) fn spells_status(&self) -> String {
        let (max, left) = (self.slot_max(Who::Player), self.slots_left(Who::Player));
        let slots: Vec<String> = (0..9).filter(|k| max[*k] > 0).map(|k| format!("{}:{}/{}", k + 1, left[k], max[k])).collect();
        format!(
            "slots={} prepared={:?} concentration={} dc={} spell_attack={:+}",
            if slots.is_empty() { "-".to_string() } else { slots.join(",") },
            self.prepared_of(Who::Player),
            self.world.concentration.as_ref().map_or("-".to_string(), |c| format!("{}:{:.0}", c.0.split('@').next().unwrap_or(""), c.1)),
            self.spell_dc(Who::Player),
            self.spell_attack_bonus(Who::Player)
        )
    }

    /// What a spell does, in a line: "Ranged spell attack, 1d10 fire", "DEX save for half, 8d6
    /// fire", "Heals 1d8+3", the conditions and buffs (the dice at `slot`, a cantrip's at the
    /// caster's level).
    pub(super) fn spell_summary(&self, who: Who, d: &SpellDef, slot: u32) -> String {
        use crate::skills::SpellPart as P;
        let lvl = self.scale_level(who, d, slot);
        let mut out: Vec<String> = Vec::new();
        for p in &d.parts {
            match p {
                P::Attack { ranged } => out.push(crate::lang::tr_ui(if *ranged { "Ranged spell attack" } else { "Melee spell attack" }).into_owned()),
                P::Save { ability, half } => out.push(format!("{} {}{}", ability.short(), crate::lang::tr_ui("save"), if *half { format!(" ({})", crate::lang::tr_ui("half")) } else { String::new() })),
                P::Damage { by_level, damage_type } => {
                    if let Some(x) = crate::skills::dice_at(by_level, lvl) {
                        out.push(format!("{}{}", self.spell_dice(who, x), damage_type.map_or(String::new(), |t| format!(" {}", crate::lang::tr_ui(t.name())))));
                    }
                }
                P::Heal { by_level } => {
                    if let Some(x) = crate::skills::dice_at(by_level, lvl) {
                        out.push(format!("{} {}", crate::lang::tr_ui("Heals"), self.spell_dice(who, x)));
                    }
                }
                P::TempHp { by_level } => {
                    if let Some(x) = crate::skills::dice_at(by_level, lvl) {
                        out.push(format!("{} {}", self.spell_dice(who, x), crate::lang::tr_ui("temporary HP")));
                    }
                }
                P::Condition { condition, .. } => out.push(crate::lang::tr_ui(condition.name()).into_owned()),
                P::Targets { count, per_slot } => out.push(format!("x{}", count + per_slot * slot.saturating_sub(d.level.max(1)))),
                P::Buff { buff } => {
                    use crate::srd::conditions::Buff as B;
                    out.push(match buff {
                        B::AttackDie { dice, minus } => format!("{}{dice} {}", if *minus { "-" } else { "+" }, crate::lang::tr_ui("to attack rolls")),
                        B::SaveDie { dice, minus } => format!("{}{dice} {}", if *minus { "-" } else { "+" }, crate::lang::tr_ui("to saving throws")),
                        B::Ac { bonus } => format!("AC {bonus:+}"),
                        B::AcBase { base } => format!("AC {base} + DEX"),
                        B::Speed { feet } => format!("{} {feet:+} ft", crate::lang::tr_ui("Speed")),
                        B::Haste => crate::lang::tr_ui("Haste").into_owned(),
                        B::Resistance { types } => format!("{} {}", crate::lang::tr_ui("Resistance"), types.join(", ")),
                        B::AbilityScore { ability, score } => format!("{} {score}", ability.short()),
                    });
                }
                P::Light { .. } => out.push(crate::lang::tr_ui("Light").into_owned()),
                P::Unlock => out.push(crate::lang::tr_ui("Opens locks").into_owned()),
                P::Stabilize => out.push(crate::lang::tr_ui("Stabilizes").into_owned()),
                P::Script { .. } => {}
            }
        }
        if d.story_only {
            out.push(crate::lang::tr_ui("Story only").into_owned());
        }
        out.join(", ")
    }

}
