//! Invisible spawner zones that keep a group of characters (usually enemies) on the map.

use macroquad::prelude::*;

use super::Game;
use crate::entity::{EntityKind, Facing};
use crate::map::{Respawn, ZoneKind};

/// Spawned characters get uids from here up, so they never collide with placed entities.
/// Spawned characters get uids from here (below `state::DROPPED_UID_BASE`).
pub(super) const SPAWN_UID_BASE: u32 = 1 << 30;
const _: () = assert!(SPAWN_UID_BASE < crate::state::DROPPED_UID_BASE);
/// Respawns don't happen closer than this to the player (tiles).
const MIN_PLAYER_DIST: f32 = 3.0;

pub(super) struct SpawnerRt {
    zone: u32,
    character: String,
    count: u32,
    respawn: Respawn,
    rect: (u32, u32, u32, u32),
    /// Pending respawn countdowns (`Respawn::After`).
    timers: Vec<f32>,
}

impl Game {
    /// Time passed (sleeping): every defeated character waiting to respawn comes back now.
    pub(super) fn spawners_catch_up(&mut self) {
        for sp in &mut self.spawners {
            sp.timers.fill(0.0);
        }
    }

    /// Is the spawner zone `zone` of map `map` switched off (`SetSpawners`)?
    fn spawner_off(&self, map: &str, zone: &str) -> bool {
        self.world.spawners_off.get(map).is_some_and(|s| s.contains("") || s.contains(zone))
    }

    /// `SetSpawners`: switches spawners off (or on again). On the current map the characters of
    /// switched-off spawners vanish (with a puff), switched-on ones start spawning.
    pub(super) fn set_spawners(&mut self, map: &str, zone: &str, active: bool) {
        let map = if map.is_empty() { self.loaded.map.name.clone() } else { map.to_string() };
        let set = self.world.spawners_off.entry(map.clone()).or_default();
        if active {
            // All on again, or one zone switched off by name (after "all off" use all on).
            if zone.is_empty() {
                set.clear();
            } else {
                set.remove(zone);
            }
        } else {
            set.insert(zone.to_string());
        }
        if set.is_empty() {
            self.world.spawners_off.remove(&map);
        }
        if map != self.loaded.map.name {
            return;
        }
        // The current map: the spawned characters of switched-off spawners vanish.
        let ts = self.ts();
        let names: std::collections::HashMap<u32, String> = self.loaded.map.zones.iter().map(|z| (z.uid, z.name.clone())).collect();
        let off: Vec<u32> = self.spawners.iter().map(|s| s.zone).filter(|z| self.spawner_off(&map, names.get(z).map_or("", |n| n.as_str()))).collect();
        for e in self.loaded.entities.iter_mut().filter(|e| e.active && e.spawned_by.is_some_and(|z| off.contains(&z))) {
            e.active = false;
            self.effects.push((e.body.pos - vec2(0.0, ts * 0.4), 0.0));
        }
        if active {
            // Rebuild: spawners switched on start over with their full group.
            self.loaded.entities.retain(|e| e.spawned_by.is_none());
            self.setup_spawners();
        } else {
            self.spawners.retain(|s| !off.contains(&s.zone));
        }
    }

    /// Called on map enter: creates the spawners' characters (not of switched-off spawners).
    pub(super) fn setup_spawners(&mut self) {
        self.spawners.clear();
        let map = self.loaded.map.name.clone();
        let zones: Vec<_> = self
            .loaded
            .map
            .zones
            .iter()
            .filter(|z| !self.spawner_off(&map, &z.name))
            .filter_map(|z| match &z.kind {
                ZoneKind::Spawner { character, count, respawn } => {
                    Some((z.uid, character.clone(), *count, respawn.clone(), (z.x, z.y, z.w, z.h)))
                }
                _ => None,
            })
            .collect();
        for (zone, character, count, respawn, rect) in zones {
            let killed = match respawn {
                Respawn::Never => self.world.spawner_kills.get(&map).and_then(|m| m.get(&zone)).copied().unwrap_or(0),
                _ => 0,
            };
            self.spawners.push(SpawnerRt { zone, character, count, respawn, rect, timers: Vec::new() });
            let si = self.spawners.len() - 1;
            for _ in 0..count.saturating_sub(killed) {
                self.spawn_one(si, false);
            }
        }
    }

    /// Spawns one character on a random free tile of the spawner's zone.
    fn spawn_one(&mut self, si: usize, effect: bool) -> bool {
        let ts = self.ts();
        let (x, y, w, h) = self.spawners[si].rect;
        for _ in 0..40 {
            let tx = x + rand::gen_range(0, w.max(1));
            let ty = y + rand::gen_range(0, h.max(1));
            if self.tile_blocks_npc(tx as i32, ty as i32) {
                continue;
            }
            let pos = vec2((tx as f32 + 0.5) * ts, (ty as f32 + 1.0) * ts - 1.0);
            if pos.distance(self.player.pos) < MIN_PLAYER_DIST * ts {
                continue;
            }
            let uid = self.next_spawn_uid;
            self.next_spawn_uid += 1;
            let sp = &self.spawners[si];
            let mut e = self.new_rt_entity(EntityKind::Character, &sp.character.clone(), uid, "", pos, Facing::Down, true, 0);
            e.spawned_by = Some(sp.zone);
            // The spawner's loot table goes to its characters.
            if let Some(loot) = self.loaded.map.zones.iter().find(|z| z.uid == sp.zone).and_then(|z| z.loot.clone()) {
                e.loot = loot;
            }
            self.loaded.entities.push(e);
            if effect {
                self.effects.push((pos - vec2(0.0, ts * 0.4), 0.0));
            }
            return true;
        }
        false
    }

    /// Bookkeeping when a spawned character is defeated.
    pub(super) fn spawned_defeated(&mut self, zone: u32) {
        let map = self.loaded.map.name.clone();
        let Some(sp) = self.spawners.iter_mut().find(|s| s.zone == zone) else { return };
        match sp.respawn {
            Respawn::Never => {
                *self.world.spawner_kills.entry(map).or_default().entry(zone).or_insert(0) += 1;
            }
            Respawn::After { seconds } => sp.timers.push(seconds.max(0.1)),
            Respawn::OnEnter => {}
        }
    }

    pub(super) fn update_spawners(&mut self, dt: f32) {
        // Defeated spawned characters are removed (they are never saved).
        self.loaded.entities.retain(|e| e.spawned_by.is_none() || e.active);
        for si in 0..self.spawners.len() {
            let sp = &mut self.spawners[si];
            for t in &mut sp.timers {
                *t -= dt;
            }
            let due = sp.timers.iter().filter(|t| **t <= 0.0).count();
            if due == 0 {
                continue;
            }
            let (zone, count) = (sp.zone, sp.count);
            let alive = self.loaded.entities.iter().filter(|e| e.spawned_by == Some(zone)).count() as u32;
            let mut spawned = 0;
            for _ in 0..due.min(count.saturating_sub(alive) as usize) {
                if self.spawn_one(si, true) {
                    spawned += 1;
                }
            }
            // Timers that couldn't spawn (player standing on the zone) retry shortly.
            let sp = &mut self.spawners[si];
            let mut removed = 0;
            sp.timers.retain_mut(|t| {
                if *t <= 0.0 && removed < spawned {
                    removed += 1;
                    false
                } else {
                    if *t <= 0.0 {
                        *t = 0.5;
                    }
                    true
                }
            });
        }
    }

    /// Tile of the nearest living hostile character (for automated play-throughs).
    pub fn nearest_enemy_tile(&self) -> Option<(f32, f32)> {
        let ts = self.ts();
        self.loaded
            .entities
            .iter()
            .filter(|e| e.active && e.hostile)
            .min_by(|a, b| a.body.pos.distance(self.player.pos).total_cmp(&b.body.pos.distance(self.player.pos)))
            .map(|e| ((e.body.pos.x / ts).floor(), ((e.body.pos.y - 4.0) / ts).floor()))
    }

    /// Tile of the active entity named `name` (for automated play-throughs: `talkto`).
    pub fn named_entity_tile(&self, name: &str) -> Option<(f32, f32)> {
        let ts = self.ts();
        self.loaded.entities.iter().find(|e| e.active && e.name == name).map(|e| ((e.body.pos.x / ts).floor(), ((e.body.pos.y - 4.0) / ts).floor()))
    }

    /// Spawner summary, e.g. `slime_den 3/4 (+1 pending)`.
    pub fn spawner_status(&self) -> String {
        self.spawners
            .iter()
            .map(|sp| {
                let name = self.loaded.map.zones.iter().find(|z| z.uid == sp.zone).map_or("?", |z| z.name.as_str());
                let alive = self.loaded.entities.iter().filter(|e| e.spawned_by == Some(sp.zone) && e.active).count();
                let killed = self.world.spawner_kills.get(&self.loaded.map.name).and_then(|m| m.get(&sp.zone)).copied().unwrap_or(0);
                format!("{name} {alive}/{} ({} pending, {killed} killed for good)", sp.count, sp.timers.len())
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Number of active hostile characters on the map (for status / tests).
    pub fn enemies_alive(&self) -> usize {
        self.loaded.entities.iter().filter(|e| e.active && e.hostile).count()
    }
}
