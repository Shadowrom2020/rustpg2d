//! How places sound: *Ambience* zones (`ZoneKind::Ambience`: a looping sound, full inside the
//! zone, fading out `fade` tiles around it; several mix, the same sound plays once at its
//! loudest) and footsteps (a step sound per terrain or tile, `TilesetDef::step_of`; the topmost
//! layer below the characters with one decides). A step falls on every half of the walk
//! animation's cycle (two per cycle, where the feet land), so steps keep pace with the drawn walk
//! and quicken when running; characters nearby step too, quieter. Each step's loudness varies a
//! little, so a walk doesn't sound like a machine.

use std::collections::HashMap;

use macroquad::prelude::*;

use super::{Body, Game};
use crate::map::{Map, ZoneKind};

/// The walk cycle (seconds) of a character without a walk clip.
const DEFAULT_CYCLE: f32 = 1.0;
/// Characters' steps are heard this many tiles from the player (fading), at most this loud.
const NPC_STEP_TILES: f32 = 6.0;
const NPC_STEP_GAIN: f32 = 0.35;

/// Distance (tiles) from `p` (tile units) to a zone's rectangle; 0 inside.
fn distance_to(p: Vec2, x: u32, y: u32, w: u32, h: u32) -> f32 {
    let (x0, y0, x1, y1) = (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
    let dx = (x0 - p.x).max(0.0).max(p.x - x1);
    let dy = (y0 - p.y).max(0.0).max(p.y - y1);
    (dx * dx + dy * dy).sqrt()
}

/// The ambience loops heard at `p` (in tiles) on `map`: sound -> loudness (0..1).
pub(super) fn ambience_at(map: &Map, p: Vec2) -> HashMap<String, f32> {
    let mut out: HashMap<String, f32> = HashMap::new();
    for z in &map.zones {
        let ZoneKind::Ambience { sound, volume, fade } = &z.kind else { continue };
        if sound.is_empty() {
            continue;
        }
        let d = distance_to(p, z.x, z.y, z.w, z.h);
        let v = if d <= 0.0 {
            *volume
        } else if *fade > 0.0 {
            volume * (1.0 - d / fade).max(0.0)
        } else {
            0.0
        };
        if v > 0.0 {
            let e = out.entry(sound.clone()).or_insert(0.0);
            *e = e.max(v.clamp(0.0, 1.0));
        }
    }
    out
}

impl Game {
    /// The ambience loops for where the player stands (none on the title screen).
    pub(super) fn update_soundscape(&mut self, dt: f32) {
        let wanted = if self.on_title() { HashMap::new() } else { ambience_at(&self.loaded.map, self.player.pos / self.ts()) };
        self.audio.set_ambient(&wanted, dt);
    }

    /// The footstep sound under feet at `pos`: the topmost layer below the characters whose
    /// tile has one.
    fn step_sound(&self, pos: Vec2) -> Option<String> {
        let m = &self.loaded.map;
        let (tx, ty) = crate::grid::feet_tile(pos, self.ts());
        if !m.in_bounds(tx, ty) {
            return None;
        }
        m.layers.iter().rev().filter(|l| !l.above_entities).find_map(|l| {
            let (slot, idx) = l.get(tx as u32, ty as u32).parts()?;
            let id = m.tilesets.get(slot)?;
            self.project.db.tileset(id)?.step_of(idx).map(str::to_string)
        })
    }

    /// The walk cycle (seconds) of a look's model.
    fn walk_cycle(&self, a: &crate::defs::Appearance) -> f32 {
        let model = self.look(a).model;
        self.assets.models.cycle_seconds(&model, "walk").unwrap_or(DEFAULT_CYCLE).max(0.2)
    }

    /// Plays the steps of everyone whose walk reached its next half cycle.
    pub(super) fn footsteps(&mut self) {
        // A walker's half cycle now; true when it is a new one (a foot lands).
        fn landed(b: &mut Body, cycle: f32) -> bool {
            let half = (b.anim / (cycle / 2.0)) as u32;
            let new = b.moving && half != b.step_half;
            b.step_half = half;
            new
        }
        let cycle = self.player_appearance().map_or(DEFAULT_CYCLE, |a| self.walk_cycle(&a));
        if landed(&mut self.player, cycle) {
            if let Some(s) = self.step_sound(self.player.pos) {
                self.audio.play_at(&s, rand::gen_range(0.8, 1.0));
            }
        }
        let (me, ts) = (self.player.pos, self.ts());
        let cycles: Vec<f32> = self.loaded.entities.iter().map(|e| if e.body.moving && e.active && e.kind == crate::entity::EntityKind::Character { self.walk_cycle(&e.appearance) } else { DEFAULT_CYCLE }).collect();
        let mut steps = Vec::new();
        for (e, cycle) in self.loaded.entities.iter_mut().zip(cycles) {
            if !landed(&mut e.body, cycle) || !e.active || e.kind != crate::entity::EntityKind::Character {
                continue;
            }
            let d = e.body.pos.distance(me) / ts;
            if d < NPC_STEP_TILES {
                steps.push((e.body.pos, NPC_STEP_GAIN * (1.0 - d / NPC_STEP_TILES) * rand::gen_range(0.8, 1.0)));
            }
        }
        for (pos, gain) in steps {
            if let Some(s) = self.step_sound(pos) {
                self.audio.play_at(&s, gain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Zone;

    #[test]
    fn steps_come_from_the_tile_then_its_terrain_not_a_base() {
        use crate::autotile::Terrain;
        use crate::defs::{TileSteps, TilesetDef};
        let mut path = Terrain { name: "Dirt path".into(), base: Some(0), step: "step_dirt".into(), ..Default::default() };
        path.tiles[15] = Some(7);
        let ts = TilesetDef { terrains: vec![path], steps: vec![TileSteps { sound: "step_wood".into(), tiles: vec![6] }], ..TilesetDef::new("t", "t.png") };
        assert_eq!(ts.step_of(6), Some("step_wood"));
        assert_eq!(ts.step_of(7), Some("step_dirt"));
        // Grass is the path's base (what erasing it leaves), not a path tile: silent here.
        assert_eq!(ts.step_of(0), None);
    }

    #[test]
    fn ambience_is_full_inside_and_fades_around() {
        let mut map = Map::new("m", 40, 40);
        let zone = |x, y, sound: &str, volume, fade| Zone { uid: 1, name: "a".into(), x, y, w: 4, h: 4, kind: ZoneKind::Ambience { sound: sound.into(), volume, fade }, triggers: vec![], loot: None };
        map.zones.push(zone(10, 10, "birds", 0.8, 4.0));
        map.zones.push(zone(20, 10, "birds", 0.4, 4.0));
        map.zones.push(zone(10, 20, "river", 1.0, 0.0));
        let at = |x: f32, y: f32| ambience_at(&map, vec2(x, y));
        assert_eq!(at(12.0, 12.0).get("birds"), Some(&0.8));
        // Two tiles off the zone's edge: halfway faded.
        assert!((at(16.0, 12.0)["birds"] - 0.4).abs() < 1e-4);
        // The same sound in two zones in reach (3 tiles from each): the louder one counts, they
        // don't add up (0.8 x 0.25 against 0.4 x 0.25).
        assert!((at(17.0, 12.0)["birds"] - 0.2).abs() < 1e-4);
        assert!(!at(30.0, 30.0).contains_key("birds"));
        // No fade: only inside.
        assert_eq!(at(11.0, 21.0).get("river"), Some(&1.0));
        assert!(!at(11.0, 25.5).contains_key("river"));
    }
}
