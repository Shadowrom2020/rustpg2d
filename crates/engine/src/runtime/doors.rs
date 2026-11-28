//! Doors (`PropDef::door`): E opens and closes them; a lock keeps a door shut until its key or a
//! lockpick opens it, and closing a door with its key locks it again. Characters walking their
//! routes open unlocked doors (and locked ones they carry the key of) as they reach them and close
//! them behind them.

use gfx::prelude::*;

use super::occupancy::{Occupancy, OCC_PROP};
use super::{Game, Source};
use crate::entity::EntityKind;
use crate::events::GameEvent;
use crate::path::TilePos;

/// The state an open door is in.
pub(super) const OPEN: &str = "open";
/// Seconds a door a character opened stays open once the doorway is clear.
const CLOSE_AFTER: f32 = 1.0;

/// A door a character opened on its way: closed again (and locked, if it was) once clear.
#[derive(Clone, Debug)]
pub(super) struct NpcDoor {
    uid: u32,
    relock: bool,
    clear: f32,
}

impl Game {
    /// Whether entity `i` is a door.
    pub(super) fn is_door(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        e.kind == EntityKind::Prop && self.project.db.prop(&e.def).is_some_and(|d| d.door)
    }

    /// Whether door `i` stands open.
    fn door_open(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        self.project.db.prop(&e.def).and_then(|d| d.state_index(OPEN)).is_some_and(|s| s == e.state)
    }

    /// Whether the player or a character stands in door `i` (it can't be closed then).
    fn doorway_taken(&self, i: usize) -> bool {
        let r = self.loaded.entities[i].body.rect();
        let uid = self.loaded.entities[i].uid;
        self.player.rect().overlaps(&r) || self.loaded.entities.iter().any(|e| e.active && e.uid != uid && e.kind == EntityKind::Character && e.body.rect().overlaps(&r))
    }

    /// E at door `i`: opens it (unlocking it first: its key or a lockpick) or closes it (with its
    /// key: locks it). Returns false if `i` is no door.
    pub(super) fn use_door(&mut self, i: usize) -> bool {
        if !self.is_door(i) {
            return false;
        }
        let e = &self.loaded.entities[i];
        if self.project.db.prop(&e.def).and_then(|d| d.state_index(OPEN)).is_none() {
            eprintln!("[game] door '{}' has no \"{OPEN}\" state", e.def);
            return false;
        }
        let (uid, at) = (e.uid, e.body.pos);
        if self.door_open(i) {
            // With its key closing it locks it.
            if self.try_relock(i, None) {
                return true;
            }
            if self.doorway_taken(i) {
                self.ui.toast("Something is in the way.".into());
                return true;
            }
            let closed = self.initial_state(i);
            self.set_prop_state(i, &closed);
        } else {
            if self.active_lock(i).is_some() {
                // (Unlocking plays its own sound; a lock that doesn't open the door itself.)
                if self.try_unlock(i, None) && !self.door_open(i) && self.loaded.entities[i].active {
                    self.set_prop_state(i, OPEN);
                }
                return true;
            }
            self.set_prop_state(i, OPEN);
        }
        let sfx = self.project.config.sfx.portal.clone();
        self.sound_from(&sfx, at);
        self.fire(GameEvent::Interact, Source::Entity(uid));
        true
    }

    /// Whether character `i` may walk through closed door `d`: it isn't locked, or the character
    /// carries its key.
    fn npc_may_open(&self, i: usize, d: usize) -> bool {
        match self.active_lock(d) {
            None => true,
            Some(l) => !l.key.is_empty() && self.loaded.entities[i].inventory.iter().any(|s| s.item == l.key),
        }
    }

    /// The closed doors on this map character `i` may walk through, and the tiles each covers.
    fn npc_doors_for(&self, i: usize) -> Vec<(usize, Vec<TilePos>)> {
        let ts = self.ts();
        (0..self.loaded.entities.len())
            .filter(|&d| self.loaded.entities[d].active && self.is_door(d) && !self.door_open(d) && self.npc_may_open(i, d))
            .map(|d| (d, Occupancy::covered(self.loaded.entities[d].body.rect(), ts).collect()))
            .collect()
    }

    /// Tiles of closed doors character `i` may walk through (route planning goes through them).
    pub(super) fn npc_door_tiles(&self, i: usize) -> Vec<TilePos> {
        self.npc_doors_for(i).into_iter().flat_map(|(_, tiles)| tiles).collect()
    }

    /// Character `i` is about to step onto `tile`: a closed door there that it may pass opens (a
    /// locked one with the key it carries) and closes behind it.
    pub(super) fn npc_open_door(&mut self, i: usize, tile: TilePos) {
        if !self.loaded.occupancy.has(tile.0, tile.1, OCC_PROP) {
            return;
        }
        let Some((d, _)) = self.npc_doors_for(i).into_iter().find(|(_, tiles)| tiles.contains(&tile)) else { return };
        let relock = self.active_lock(d).is_some();
        if relock {
            self.loaded.entities[d].unlocked = true;
        }
        self.set_prop_state(d, OPEN);
        let (uid, at) = (self.loaded.entities[d].uid, self.loaded.entities[d].body.pos);
        let sfx = self.project.config.sfx.portal.clone();
        self.sound_from(&sfx, at);
        self.npc_doors.push(NpcDoor { uid, relock, clear: 0.0 });
    }

    /// Closes the doors characters opened once nobody stands in them any more.
    pub(super) fn update_npc_doors(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.npc_doors.len() {
            let uid = self.npc_doors[k].uid;
            // (Gone, or closed by the player meanwhile.)
            let Some(i) = self.loaded.entities.iter().position(|e| e.active && e.uid == uid).filter(|&i| self.door_open(i)) else {
                self.npc_doors.remove(k);
                continue;
            };
            let taken = self.doorway_taken(i);
            let d = &mut self.npc_doors[k];
            d.clear = if taken { 0.0 } else { d.clear + dt };
            if d.clear < CLOSE_AFTER {
                k += 1;
                continue;
            }
            let relock = d.relock;
            self.npc_doors.remove(k);
            self.close_npc_door(i, relock);
        }
    }

    fn close_npc_door(&mut self, i: usize, relock: bool) {
        if relock {
            self.apply_lock(i);
        } else {
            let closed = self.initial_state(i);
            self.set_prop_state(i, &closed);
        }
    }

    /// Leaving the map: the doors characters opened close at once.
    pub(super) fn close_npc_doors_now(&mut self) {
        for d in std::mem::take(&mut self.npc_doors) {
            if let Some(i) = self.loaded.entities.iter().position(|e| e.active && e.uid == d.uid).filter(|&i| self.door_open(i)) {
                self.close_npc_door(i, d.relock);
            }
        }
    }

    /// For `status()`: the doors on this map as name:open|closed[:locked]@x,y.
    pub(super) fn doors_status(&self) -> Vec<String> {
        let ts = self.ts();
        (0..self.loaded.entities.len())
            .filter(|&i| self.loaded.entities[i].active && self.is_door(i))
            .map(|i| {
                let e = &self.loaded.entities[i];
                let name = if e.name.is_empty() { e.def.as_str() } else { e.name.as_str() };
                let (x, y) = crate::grid::feet_tile(e.body.pos, ts);
                let open = if self.door_open(i) { "open" } else { "closed" };
                let locked = if self.active_lock(i).is_some() { ":locked" } else { "" };
                format!("{name}:{open}{locked}@{x},{y}")
            })
            .collect()
    }
}
