//! The SRD's poisons in play (`srd/poisons.rs`): using an injury or contact poison coats the
//! hero's weapon (or ammunition); the next hit with it delivers the dose (`deliver_coating`
//! after a fight's hit). *Expose to poison* (`Action::Poison`, `poison()`) gives any poison to the
//! hero or a character: a Constitution save (with advantage from Antitoxin, Protection from
//! Poison or a dwarf's resilience), the damage and the conditions.

use super::turns::Who;
use super::Game;
use crate::srd::conditions::{self, Buff, Condition};

/// The source of an unconsciousness from a poison: damage ends it.
const POISON_SLEEP: &str = "poison";
use crate::srd::poisons::{Coating, PoisonDef};
use crate::srd::{DamageType, Mode};

/// The source prefix of an unconsciousness a hit ends (Sleep's).
pub(super) const WAKE_ON_HIT: &str = "wake:";

impl Game {
    /// The poison of item `item`, if it is one.
    fn poison_of(&self, item: &str) -> Option<PoisonDef> {
        self.project.db.item(item).and_then(|d| d.poison.clone())
    }

    /// Using `item`: an injury or contact poison coats the weapon in hand (slashing or piercing)
    /// or, for a weapon that shoots, its ammunition. True = the use was this (done or refused
    /// with a message).
    pub(super) fn coat_with(&mut self, item: &str) -> bool {
        let Some(p) = self.poison_of(item).filter(|p| p.kind.coats()) else { return false };
        let db = &self.project.db;
        let weapon = self.world.equipment.item(crate::defs::EquipSlot::Weapon).map(str::to_string);
        let w = weapon.as_deref().and_then(|id| db.item(id)).and_then(|d| d.weapon.clone());
        let on = match (&weapon, &w) {
            // A bow or crossbow: the ammunition it shoots next.
            (Some(_), Some(w)) if w.ranged.as_ref().is_some_and(|r| !r.ammo.is_empty()) => w.ranged.as_ref().and_then(|r| self.next_ammo(&r.ammo)).map(|(a, ..)| a),
            (Some(id), Some(w)) if matches!(w.damage_type, Some(DamageType::Piercing | DamageType::Slashing)) => Some(id.clone()),
            _ => None,
        };
        let Some(on) = on else {
            self.ui.toast(crate::lang::tr_ui("Nothing to coat: hold a piercing or slashing weapon, or one with ammunition.").into_owned());
            return true;
        };
        // In a fight it is the action of the turn (Use an Object).
        if let Err(why) = self.fight_item_use() {
            self.ui.toast(crate::lang::tr_ui(why).into_owned());
            return true;
        }
        self.world.coating = Some(Coating { poison: item.into(), on: on.clone(), left: (p.minutes > 0.0).then_some(p.minutes * 60.0) });
        self.world.take_item(item, 1);
        let name = self.project.db.item(&on).map_or(on.clone(), |d| d.name.clone());
        self.ui.toast(crate::lang::tr_ui("You coat your {}.").replacen("{}", &crate::lang::tr(&name), 1));
        true
    }

    /// The poison on an attack made of `made_of` (the weapon, the ammunition shot): the coating's
    /// item when it is on one of them.
    pub(super) fn coated_attack(&self, made_of: &[String]) -> String {
        match &self.world.coating {
            Some(c) if made_of.contains(&c.on) => c.poison.clone(),
            _ => String::new(),
        }
    }

    /// `dt` seconds pass for the coating (frames outside fights, rounds on the hero's turns).
    pub(super) fn tick_coating(&mut self, dt: f32) {
        if self.world.coating.as_mut().is_some_and(|c| !c.tick(dt)) {
            self.world.coating = None;
            self.ui.toast(crate::lang::tr_ui("The poison on your weapon has dried.").into_owned());
        }
    }

    /// A hit with a coated weapon: the dose goes into `target` and the coating is used up.
    pub(super) fn deliver_coating(&mut self, who: Who, target: Who, item: &str) {
        if who == Who::Player && self.world.coating.as_ref().is_some_and(|c| c.poison == item) {
            self.world.coating = None;
        }
        self.expose_to_poison(target, item, Some(who));
    }

    /// How `who` saves against poison: with advantage from Antitoxin or Protection from Poison,
    /// and for the hero from a race that resists poison (a dwarf's, a stout halfling's resilience).
    pub(super) fn poison_save_mode(&self, who: Who) -> Mode {
        let buff = conditions::buffs(self.who_conditions(who)).any(|b| matches!(b, Buff::PoisonSaves));
        let race = who == Who::Player && self.player_race().is_some_and(|r| r.resistances.iter().any(|t| t.eq_ignore_ascii_case("poison")));
        if buff || race {
            Mode::Advantage
        } else {
            Mode::Normal
        }
    }

    /// Gives poison `item` to `target` (`by`: who poisoned it, in a fight): its Constitution save,
    /// then the damage (half or none on a success) and the conditions of a failure. False = no
    /// such poison.
    pub(super) fn expose_to_poison(&mut self, target: Who, item: &str, by: Option<Who>) -> bool {
        let Some(p) = self.poison_of(item) else {
            eprintln!("[events] Expose to poison: '{item}' is no poison");
            return false;
        };
        let name = self.who_name(target);
        self.poison_save = true;
        let rec = self.who_check(target, "con", p.dc, true);
        self.poison_save = false;
        let saved = rec.as_ref().is_some_and(|r| r.success == Some(true));
        let missed_by = rec.as_ref().map_or(0, |r| (p.dc as i64 - r.total).max(0));
        let rolled = if p.damage.is_empty() { 0 } else { super::turns::roll_damage(&p.damage, false) };
        let (dmg, given) = p.outcome(rolled, saved, missed_by);
        if self.fight.is_some() {
            let save_text = rec.as_ref().map_or(String::new(), |r| format!("CON {}/{} {}", r.total, p.dc, crate::lang::tr_ui(if saved { "saved" } else { "failed" })));
            self.fight_log(format!("{name}: {save_text}{}", if dmg > 0 { format!(", {dmg}") } else { String::new() }));
        }
        if dmg > 0 {
            self.poison_damage(target, dmg, by);
        }
        // The hero passing out outside a fight sleeps it off: the hours go by at once.
        let out = given.iter().filter(|c| c.condition == Some(Condition::Unconscious)).map(|c| c.seconds).fold(0.0, f32::max);
        if target == Who::Player && self.fight.is_none() && out > 0.0 && !given.is_empty() {
            self.world.advance_hours(out / 3600.0);
            let hours = (out / 3600.0).round().max(1.0) as i64;
            self.ui.toast(crate::lang::tr_ui("You pass out and wake {} hours later.").replacen("{}", &hours.to_string(), 1));
            // (What lasted no longer than the sleep is over.)
            for spec in given.iter().filter(|c| c.seconds > out) {
                self.apply_player_effect(spec);
            }
            return true;
        }
        for spec in given {
            match target {
                Who::Player => self.apply_player_effect(&spec),
                Who::Npc(_) => {
                    if let Some(i) = self.who_index(target) {
                        self.apply_entity_effect(i, &spec);
                    }
                }
            }
            // (A poison's sleep: damage wakes it, `wake_poisoned`.)
            if spec.condition == Some(Condition::Unconscious) {
                if let Some(a) = self.who_conditions_mut(target).and_then(|l| l.iter_mut().find(|a| a.condition == Some(Condition::Unconscious) && a.source.is_empty())) {
                    a.source = POISON_SLEEP.into();
                }
            }
        }
        true
    }

    /// `who` took damage: asleep from a poison, it wakes.
    pub(super) fn wake_poisoned(&mut self, who: Who) {
        let woke = match self.who_conditions_mut(who) {
            Some(l) => {
                let before = l.len();
                l.retain(|a| !(a.condition == Some(Condition::Unconscious) && (a.source == POISON_SLEEP || a.source.starts_with(WAKE_ON_HIT))));
                l.len() < before
            }
            None => false,
        };
        if woke && self.fight.is_some() {
            let line = format!("{} {}", self.who_name(who), crate::lang::tr_ui("wakes up"));
            self.fight_log(line);
        }
    }

    /// Poison damage to `target`: in a fight the turns' damage, outside one the hero's or the
    /// character's (through its defenses).
    fn poison_damage(&mut self, target: Who, dmg: i32, by: Option<Who>) {
        if self.fight.is_some() {
            self.combat_damage_parts(target, &[(dmg, Some(DamageType::Poison))], false, by, crate::srd::HitKind::Plain);
            return;
        }
        match target {
            Who::Player => {
                let from = self.player.pos;
                self.hurt_player_by(dmg, from, Some(DamageType::Poison));
            }
            Who::Npc(_) => {
                let Some(i) = self.who_index(target) else { return };
                let n = self.who_block(target).map_or(dmg, |b| b.defenses().apply(dmg, Some(DamageType::Poison)));
                if n > 0 {
                    let from = self.loaded.entities[i].body.pos;
                    self.damage_entity(i, n, from, 0.0);
                }
            }
        }
    }

    /// *Expose to poison*: `target` is "player", "self" (the trigger's owner) or a character's
    /// name.
    pub(super) fn poison_named(&mut self, target: &str, owner: Option<u32>, item: &str) {
        let who = if target == "player" {
            Some(Who::Player)
        } else {
            self.entity_index(target, owner).map(|i| Who::Npc(self.loaded.entities[i].uid))
        };
        match who {
            Some(w) => {
                self.expose_to_poison(w, item, None);
            }
            None => eprintln!("[events] Expose to poison: no character '{target}' on this map"),
        }
    }

    /// `coating=` for status: `POISON>ITEM[:SECONDSs]` or `-`.
    pub(super) fn coating_status(&self) -> String {
        match &self.world.coating {
            Some(c) => match c.left {
                Some(l) => format!("{}>{}:{:.0}s", c.poison, c.on, l.max(0.0)),
                None => format!("{}>{}", c.poison, c.on),
            },
            None => "-".into(),
        }
    }
}
