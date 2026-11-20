//! Props as the SRD's objects (`PropDef::object`, `srd/objects.rs`): a swing outside fights
//! attacks one in reach (the hero's melee attack against its armor class, damage past its
//! threshold), and at 0 hit points it breaks (its lock gone; it disappears or takes its broken
//! state; its *Defeated* triggers fire). A locked one with a `force_dc` is forced open with
//! Strength (Athletics) when neither key nor picking opens it.

use macroquad::prelude::*;

use super::{Game, Source};
use crate::defs::LockSpec;
use crate::entity::EntityKind;
use crate::events::GameEvent;
use crate::srd::objects::{Broken, ObjectStats};

/// Game hours before another try at forcing what didn't give.
const FORCE_RETRY_HOURS: f64 = 10.0 / 60.0;

impl Game {
    /// Prop `i`'s object statistics (an active prop that has them and isn't broken yet).
    pub(super) fn object_of(&self, i: usize) -> Option<ObjectStats> {
        let e = &self.loaded.entities[i];
        if e.kind != EntityKind::Prop || !e.active || e.obj_hp == Some(0) {
            return None;
        }
        self.project.db.prop(&e.def)?.object.clone()
    }

    /// Prop `i`'s hit points now and whole.
    pub(super) fn object_hp(&self, i: usize) -> Option<(i32, i32)> {
        let o = self.object_of(i)?;
        let max = o.max_hp();
        Some((self.loaded.entities[i].obj_hp.unwrap_or(max).min(max), max))
    }

    /// A swing at prop `i` (outside fights): the hero's melee attack against its armor class
    /// (the roll log has both rolls), damage past its threshold. True = it was an object.
    pub(super) fn strike_object(&mut self, i: usize) -> bool {
        let Some(o) = self.object_of(i) else { return false };
        let spec = self.melee_spec(super::turns::Who::Player);
        let (bonus, dice, kind) = spec.map_or((self.world.check_bonus("str", false).unwrap_or(0) as i32, "1+STR".to_string(), Some(crate::srd::DamageType::Bludgeoning)), |s| (s.bonus, s.dice.clone(), s.damage_type));
        let pos = self.loaded.entities[i].body.pos - vec2(0.0, self.ts() * 0.6);
        let hit = self.world.roll_dice(&format!("1d20{bonus:+}")).map_or(0, |r| r.total);
        let hit_sfx = self.project.config.sfx.hit.clone();
        self.sound_from(&hit_sfx, pos);
        if hit < o.armor_class() as i64 {
            self.miss_number(pos);
            return true;
        }
        let rolled = self.world.roll_dice(&dice).map_or(1, |r| r.total.max(1) as i32);
        let dmg = o.damage_taken(rolled, kind);
        self.damage_object(i, dmg);
        true
    }

    /// `n` damage to prop `i` (already past its threshold): the number over it, and breaking
    /// at 0. A hit below the threshold shows "0".
    pub(super) fn damage_object(&mut self, i: usize, n: i32) {
        let Some((hp, _)) = self.object_hp(i) else { return };
        let pos = self.loaded.entities[i].body.pos - vec2(0.0, self.ts() * 0.6);
        self.damage_number(pos, n.max(0), super::juice::NumberKind::Hit);
        if n <= 0 {
            return;
        }
        let left = (hp - n).max(0);
        // (Breaking sets the 0 itself: a prop at 0 is no object any more.)
        if left == 0 {
            self.break_object(i);
        } else {
            self.loaded.entities[i].obj_hp = Some(left);
            self.persist(i);
        }
    }

    /// Prop `i` breaks: its lock is gone, it disappears or takes its broken state, its
    /// *Defeated* triggers fire.
    pub(super) fn break_object(&mut self, i: usize) {
        let Some(o) = self.object_of(i) else { return };
        let name = self.project.db.prop(&self.loaded.entities[i].def).map_or(String::new(), |d| crate::lang::tr(&d.name).to_string());
        self.ui.toast(crate::lang::tr_ui("{} breaks.").replacen("{}", &name, 1));
        let e = &mut self.loaded.entities[i];
        e.unlocked = true;
        e.obj_hp = Some(0);
        match &o.broken {
            Broken::Disappear => e.active = false,
            Broken::ToState { state } => {
                let state = state.clone();
                self.set_prop_state(i, &state);
            }
        }
        self.persist(i);
        let uid = self.loaded.entities[i].uid;
        self.fire(GameEvent::Defeat, Source::Entity(uid));
    }

    /// Forcing locked prop `i` open (E when neither its key nor picking opens it): Strength
    /// (Athletics) against its `force_dc`; a failure waits a while. `None` = it can't be forced.
    pub(super) fn try_force(&mut self, i: usize, lock: &LockSpec) -> Option<bool> {
        let dc = self.object_of(i).map_or(0, |o| o.force_dc);
        if dc == 0 {
            return None;
        }
        let at = (self.loaded.map.name.clone(), self.loaded.entities[i].uid);
        if self.lock_jams.get(&at).is_some_and(|until| self.world.total_hours() < *until) {
            self.ui.toast(crate::lang::tr_ui("It won't budge.").into_owned());
            return Some(false);
        }
        let ok = self.world.check("athletics", dc as i32, crate::srd::Mode::Normal, false).is_some_and(|r| r.success == Some(true));
        if !ok {
            self.lock_jams.insert(at, self.world.total_hours() + FORCE_RETRY_HOURS);
            self.ui.toast(crate::lang::tr_ui("It won't budge.").into_owned());
            return Some(false);
        }
        self.lock_jams.remove(&at);
        self.ui.toast(crate::lang::tr_ui("You force it open.").into_owned());
        let hit = self.project.config.sfx.hit.clone();
        let pos = self.loaded.entities[i].body.pos;
        self.sound_from(&hit, pos);
        self.apply_unlock(i, &lock.on_unlock);
        let uid = self.loaded.entities[i].uid;
        self.fire(GameEvent::Unlock, Source::Entity(uid));
        Some(true)
    }

    /// `objects=` for status: the damaged or broken props, `name:hp/max@x,y` (the definition id
    /// when unnamed).
    pub(super) fn objects_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Prop)
            .filter_map(|e| {
                let hp = e.obj_hp?;
                let max = self.project.db.prop(&e.def).and_then(|d| d.object.as_ref()).map_or(0, |o| o.max_hp());
                let t = self.tile_of(e.body.pos);
                Some(format!("{}:{hp}/{max}@{},{}", if e.name.is_empty() { &e.def } else { &e.name }, t.0, t.1))
            })
            .collect()
    }
}
