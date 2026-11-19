//! Key locks on props (gates, doors, chests).

use macroquad::prelude::*;

use super::{Game, Source};
use crate::defs::{LockSpec, OnUnlock};
use crate::events::GameEvent;

/// Padlocks are shown on locked props closer than this (tiles).
const PADLOCK_DIST: f32 = 4.0;

impl Game {
    /// The lock of entity `i` if it is still locked.
    pub(super) fn active_lock(&self, i: usize) -> Option<LockSpec> {
        let e = &self.loaded.entities[i];
        if e.unlocked {
            return None;
        }
        let inst = self.loaded.map.entity(e.uid)?;
        self.project.db.lock_of(inst).cloned()
    }

    /// The lock of entity `i` if it is open and can be locked again.
    pub(super) fn relockable(&self, i: usize) -> Option<LockSpec> {
        let e = &self.loaded.entities[i];
        if !e.unlocked || !e.active {
            return None;
        }
        let inst = self.loaded.map.entity(e.uid)?;
        self.project.db.lock_of(inst).filter(|l| l.can_relock()).cloned()
    }

    /// Locks the opened entity `i` again if the player has its key (`using` = the item being
    /// used on it). Returns false if it can't be relocked by the player now (no key / not
    /// relockable), true if handled (locked, or blocked with a message).
    pub(super) fn try_relock(&mut self, i: usize, using: Option<&str>) -> bool {
        let Some(lock) = self.relockable(i) else { return false };
        if !lock.key.is_empty() {
            let has_key = using.is_none_or(|u| u == lock.key) && self.world.item_count(&lock.key) > 0;
            if !has_key {
                return false;
            }
        } else if using.is_some() {
            return false;
        }
        // Closing a door / gate on somebody is not possible.
        if matches!(lock.on_unlock, OnUnlock::Open { .. } | OnUnlock::ToState { .. }) && self.closed_solid(i) {
            let r = self.loaded.entities[i].body.rect();
            let uid = self.loaded.entities[i].uid;
            let blocked = self.player.rect().overlaps(&r)
                || self.loaded.entities.iter().any(|e| e.active && e.uid != uid && e.kind == crate::entity::EntityKind::Character && e.body.rect().overlaps(&r));
            if blocked {
                self.ui.toast("Something is in the way.".into());
                return true;
            }
        }
        let msg = if !lock.lock_text.is_empty() {
            lock.lock_text.clone()
        } else if lock.key.is_empty() {
            "Closed.".to_string()
        } else {
            format!("Locked with the {}.", self.item_name(&lock.key))
        };
        self.ui.toast(msg);
        let sfx = if lock.key.is_empty() { self.project.config.sfx.portal.clone() } else { self.project.config.sfx.unlock.clone() };
        let at = self.loaded.entities[i].body.pos;
        self.sound_from(&sfx, at);
        self.apply_lock(i);
        let uid = self.loaded.entities[i].uid;
        self.fire(GameEvent::Lock, Source::Entity(uid));
        true
    }

    /// Whether entity `i` is a lootable container.
    pub(super) fn is_container(&self, i: usize) -> bool {
        let uid = self.loaded.entities[i].uid;
        self.loaded.map.entity(uid).is_some_and(|e| self.project.db.container_of(e).is_some())
    }

    /// The state a placed prop starts in (its "closed" state for locks).
    pub(super) fn initial_state(&self, i: usize) -> String {
        let uid = self.loaded.entities[i].uid;
        self.loaded.map.entity(uid).map(|e| e.state.clone()).unwrap_or_default()
    }

    /// Whether entity `i` blocks movement in its closed (initial) state.
    fn closed_solid(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        let start = self.initial_state(i);
        self.project.db.prop(&e.def).is_some_and(|d| d.state_index(&start).is_some_and(|s| d.state_look(s).1))
    }

    /// Marks entity `i` locked and undoes the lock's "open" effect (saved).
    pub(super) fn apply_lock(&mut self, i: usize) {
        let uid = self.loaded.entities[i].uid;
        let inst = self.loaded.map.entity(uid);
        let closed_frame = inst.map(|e| e.frame);
        let on = inst.and_then(|e| self.project.db.lock_of(e)).map(|l| l.on_unlock.clone());
        let solid = self.closed_solid(i);
        let start = self.initial_state(i);
        self.loaded.entities[i].unlocked = false;
        match on {
            Some(OnUnlock::Open { .. }) => {
                let e = &mut self.loaded.entities[i];
                e.frame = closed_frame.unwrap_or(0);
                e.solid = solid;
            }
            Some(OnUnlock::ToState { .. }) => {
                self.set_prop_state(i, &start);
            }
            _ => {}
        }
        self.persist(i);
    }

    /// For scripts (`is_locked`): the props with a lock on this map: (uid, name, locked).
    pub(super) fn lock_list(&self) -> Vec<(u32, String, bool)> {
        (0..self.loaded.entities.len())
            .filter(|&i| self.loaded.entities[i].kind == crate::entity::EntityKind::Prop)
            .filter_map(|i| {
                let e = &self.loaded.entities[i];
                self.loaded.map.entity(e.uid).and_then(|inst| self.project.db.lock_of(inst)).map(|_| (e.uid, e.name.clone(), !e.unlocked))
            })
            .collect()
    }

    pub(super) fn item_name(&self, id: &str) -> String {
        self.project.db.item(id).map_or(id.to_string(), |d| d.name.clone())
    }

    /// Checks the lock of entity `i` against the player's items (or the item being used).
    /// Unlocks (consuming the key) if possible. Returns true if the entity is now unlocked.
    pub(super) fn try_unlock(&mut self, i: usize, using: Option<&str>) -> bool {
        let Some(lock) = self.active_lock(i) else { return true };
        // Picking (without the key at hand): a DEX (Thieves' Tools) check against the lock's DC.
        // A failed lock can be tried again after a while. The key, if the player has it, is
        // used instead.
        let has_key = !lock.key.is_empty() && self.world.item_count(&lock.key) > 0;
        if using.is_none() && !has_key && lock.pick_dc > 0 {
            // Picking needs thieves' tools in the bag (the SRD's; a project without them needs none).
            let tools = |d: &crate::defs::ItemDef| d.srd == "thieves-tools";
            if self.project.db.items.iter().any(tools) && !self.world.inventory.slots.iter().flatten().any(|s| self.project.db.item(&s.item).is_some_and(tools)) {
                self.ui.toast(crate::lang::tr_ui("You need thieves' tools to pick this lock.").into_owned());
                return false;
            }
            let uid = self.loaded.entities[i].uid;
            let at = (self.loaded.map.name.clone(), uid);
            if self.lock_jams.get(&at).is_some_and(|until| self.world.total_hours() < *until) {
                let msg = if lock.locked_text.is_empty() { "The lock won't give. Try again later.".to_string() } else { crate::lang::tr(&lock.locked_text) };
                self.ui.toast(msg);
                return false;
            }
            let picked = self.pick_check(lock.pick_dc).is_some_and(|r| r.success == Some(true));
            if !picked {
                self.lock_jams.insert(at, self.world.total_hours() + PICK_RETRY_HOURS);
                return false;
            }
            self.lock_jams.remove(&at);
            self.ui.toast("You pick the lock.".into());
            self.audio.play(&self.project.config.sfx.unlock);
            self.apply_unlock(i, &lock.on_unlock);
            self.fire(GameEvent::Unlock, Source::Entity(uid));
            return true;
        }
        // No key item: a door / gate that simply opens when used.
        if lock.key.is_empty() {
            if using.is_some() {
                return false;
            }
            if !lock.unlock_text.is_empty() {
                self.ui.toast(lock.unlock_text.clone());
            }
            self.audio.play(&self.project.config.sfx.portal);
            self.apply_unlock(i, &lock.on_unlock);
            let uid = self.loaded.entities[i].uid;
            self.fire(GameEvent::Unlock, Source::Entity(uid));
            return true;
        }
        let key_name = self.item_name(&lock.key);
        let has_key = match using {
            Some(item) => item == lock.key && self.world.item_count(item) > 0,
            None => self.world.item_count(&lock.key) > 0,
        };
        if !has_key {
            let msg = if lock.locked_text.is_empty() { format!("It's locked. You need the {key_name}.") } else { lock.locked_text.clone() };
            self.ui.toast(msg);
            return false;
        }
        if lock.consume_key {
            self.world.take_item(&lock.key, 1);
        }
        let msg = if lock.unlock_text.is_empty() { format!("Unlocked with the {key_name}.") } else { lock.unlock_text.clone() };
        self.ui.toast(msg);
        self.audio.play(&self.project.config.sfx.unlock);
        self.apply_unlock(i, &lock.on_unlock);
        let uid = self.loaded.entities[i].uid;
        self.fire(GameEvent::Unlock, Source::Entity(uid));
        true
    }

    /// Marks entity `i` unlocked and applies the lock's effect (saved).
    pub(super) fn apply_unlock(&mut self, i: usize, on: &OnUnlock) {
        let e = &mut self.loaded.entities[i];
        e.unlocked = true;
        match on {
            OnUnlock::Nothing => {}
            OnUnlock::Disappear => e.active = false,
            OnUnlock::Open { frame } => {
                e.frame = *frame;
                e.solid = false;
            }
            OnUnlock::ToState { state } => {
                let state = state.clone();
                self.set_prop_state(i, &state);
            }
        }
        self.persist(i);
    }

    /// Restores runtime effects of an unlocked lock when a map is loaded.
    pub(super) fn restore_unlocked(&mut self) {
        for i in 0..self.loaded.entities.len() {
            if !self.loaded.entities[i].unlocked {
                continue;
            }
            let uid = self.loaded.entities[i].uid;
            let open = self
                .loaded
                .map
                .entity(uid)
                .and_then(|inst| self.project.db.lock_of(inst))
                .is_some_and(|l| matches!(l.on_unlock, OnUnlock::Open { .. }));
            if open {
                self.loaded.entities[i].solid = false;
            }
        }
    }

    /// A lock picking check: DEX (Thieves' Tools) against `dc`, into the roll log.
    pub(super) fn pick_check(&mut self, dc: u32) -> Option<crate::srd::RollRecord> {
        self.world.check("dex:thieves-tools", dc as i32, crate::srd::Mode::Normal, false)
    }

    /// Small padlocks over nearby locked props (world space).
    pub(super) fn draw_padlocks(&self) {
        let ts = self.ts();
        for (i, e) in self.loaded.entities.iter().enumerate() {
            // Doors without a key get no padlock (pickable locks do).
            if !e.active || e.unlocked || e.body.pos.distance(self.player.pos) > PADLOCK_DIST * ts || self.active_lock(i).is_none_or(|l| l.key.is_empty() && l.pick_dc == 0) {
                continue;
            }
            let r = self.entity_click_rect(e);
            let u = self.u();
            let (x, y) = ((r.x + r.w - 4.0 * u).round(), (r.y - 3.0 * u).round());
            let gold = Color::new(0.95, 0.78, 0.25, 1.0);
            let dark = Color::new(0.25, 0.18, 0.05, 1.0);
            draw_rectangle_lines(x + u, y - 3.0 * u, 4.0 * u, 4.0 * u, u, dark);
            draw_rectangle(x, y, 6.0 * u, 5.0 * u, gold);
            draw_rectangle(x + 2.5 * u, y + 1.5 * u, u, 2.0 * u, dark);
        }
    }
}

/// Game hours before a lock a pick failed on can be tried again (ten minutes).
const PICK_RETRY_HOURS: f64 = 10.0 / 60.0;
