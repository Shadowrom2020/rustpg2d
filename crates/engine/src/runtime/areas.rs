//! Lasting spell areas (0.32, the SRD's Fog Cloud, Darkness, Spirit Guardians, Moonbeam, Spike
//! Growth, Silence, walls): a spell's `SpellPart::Lasting` keeps its area on the map for the
//! spell's duration (its concentration, while the hero stays on the map; not saved). They act in
//! fights by rounds and while exploring every six seconds:
//! - **obscured** and **dark** tiles hide who stands in them (`tile_obscured`), darkness from
//!   darkvision too;
//! - **difficult** tiles cost double (`difficult_for`), for the caster's foes only with
//!   `foes_only`;
//! - **repeat**: the spell's save, damage and condition act again on a creature that starts its
//!   turn inside or enters (once a turn), with `per_tile` on every tile it moves in it;
//! - **silence**: no spell with a verbal component is cast inside;
//! - **walls** stop walking and sight (the tiles become solid while it stands).

use macroquad::prelude::*;

use super::turns::{Side, Who};
use super::Game;
use crate::grid::feet_tile;
use crate::path::TilePos;
use crate::skills::{Lasting, SpellDef};

/// A lasting area on the map.
pub(super) struct LiveArea {
    pub spell: String,
    pub slot: u32,
    pub source: String,
    pub caster: Who,
    pub effect: Lasting,
    /// Its tiles (a following area: around the caster, `radius` tiles).
    pub tiles: Vec<TilePos>,
    pub radius: i32,
    /// Seconds left.
    pub left: f32,
    pub map: String,
    /// Who it acted on this turn (once a turn).
    pub hit: Vec<Who>,
    /// A wall's tiles and what they were (solid, stopping shots), back when it falls.
    pub walls: Vec<(usize, bool, bool, bool)>,
    /// Seconds to its next tick outside fights.
    pub tick: f32,
}

impl Game {
    /// A spell's lasting area starts (cast at `point`, or around the caster).
    pub(super) fn start_area(&mut self, caster: Who, d: &SpellDef, slot: u32, point: Option<TilePos>, effect: &Lasting, seconds: f32) {
        let source = self.spell_source(caster, d);
        // (Cast outside a fight: around the caster, a wall two tiles ahead of it.)
        let at = point.unwrap_or_else(|| {
            let t = self.who_tile(caster);
            if !effect.wall {
                return t;
            }
            let f = match caster {
                Who::Player => self.player.facing,
                w => self.who_index(w).map_or(self.player.facing, |i| self.loaded.entities[i].body.facing),
            };
            let (dx, dy) = f.dir();
            (t.0 + dx.round() as i32 * 2, t.1 + dy.round() as i32 * 2)
        });
        let radius = (effect.feet / 5) as i32;
        let mut tiles = if effect.wall {
            self.wall_tiles(caster, at, (effect.feet / 5).max(2) as i32)
        } else if d.area.is_some() && !effect.follows {
            self.area_tiles(caster, d, at)
        } else {
            self.circle_tiles(at, radius.max(1))
        };
        tiles.sort();
        tiles.dedup();
        // (A wall: its tiles stop walking and sight while it stands.)
        let mut walls = Vec::new();
        if effect.wall {
            let w = self.loaded.map.width;
            for t in &tiles {
                if !self.loaded.map.in_bounds(t.0, t.1) {
                    continue;
                }
                let k = (t.1 as u32 * w + t.0 as u32) as usize;
                let (Some(s), Some(h), Some(w)) = (self.loaded.solid.get(k).copied(), self.loaded.shot.get(k).copied(), self.loaded.walls.get(k).copied()) else { continue };
                walls.push((k, s, h, w));
                self.loaded.solid[k] = true;
                self.loaded.shot[k] = true;
                self.loaded.walls[k] = true;
            }
        }
        let map = self.loaded.map.name.clone();
        self.areas.retain(|a| a.source != source || a.walls.is_empty());
        self.areas.push(LiveArea { spell: d.id.clone(), slot, source, caster, effect: effect.clone(), tiles, radius, left: seconds, map, hit: Vec::new(), walls, tick: crate::srd::conditions::ROUND });
    }

    /// Tiles within `r` of `c` (the grid's distance).
    fn circle_tiles(&self, c: TilePos, r: i32) -> Vec<TilePos> {
        let mut out = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                out.push((c.0 + dx, c.1 + dy));
            }
        }
        out
    }

    /// A wall `len` tiles long across `at`, square to the line from the caster (free tiles only).
    fn wall_tiles(&self, caster: Who, at: TilePos, len: i32) -> Vec<TilePos> {
        let from = self.who_tile(caster);
        let (dx, dy) = (at.0 - from.0, at.1 - from.1);
        // (Across the way to the caster: a horizontal wall when it looks up or down.)
        let along = if dy.abs() >= dx.abs() { (1, 0) } else { (0, 1) };
        (-(len / 2)..len - len / 2)
            .map(|k| (at.0 + along.0 * k, at.1 + along.1 * k))
            .filter(|t| self.who_at(*t).is_none() && feet_tile(self.player.pos, self.ts()) != *t && !self.tile_solid(t.0, t.1))
            .collect()
    }

    /// The tiles an area covers now (a following one: around its caster).
    fn area_now(&self, k: usize) -> Vec<TilePos> {
        let a = &self.areas[k];
        if a.effect.follows {
            self.circle_tiles(self.who_tile(a.caster), a.radius.max(1))
        } else {
            a.tiles.clone()
        }
    }

    /// The areas covering tile `t` on this map.
    fn areas_at(&self, t: TilePos) -> Vec<usize> {
        (0..self.areas.len()).filter(|&k| self.areas[k].map == self.loaded.map.name && self.area_now(k).contains(&t)).collect()
    }

    /// Whether `who` is one the area hinders (all, or the caster's foes with `foes_only`; the
    /// caster's own following area spares it).
    fn area_touches(&self, k: usize, who: Who) -> bool {
        let a = &self.areas[k];
        if who == a.caster && (a.effect.follows || a.effect.foes_only) {
            return false;
        }
        if !a.effect.foes_only {
            return true;
        }
        let side = |w: Who| match &self.fight {
            Some(c) => c.fighter(w).map(|f| f.side),
            None => Some(if w == Who::Player || self.who_index(w).is_some_and(|i| self.loaded.entities[i].companion.is_some()) { Side::Party } else { Side::Enemy }),
        };
        side(who) != side(a.caster)
    }

    /// Lasting areas that hide tile `t`: `Some("darkness")` (darkvision doesn't help),
    /// `Some("fog")`.
    pub(super) fn area_obscures(&self, t: TilePos) -> Option<&'static str> {
        let ks = self.areas_at(t);
        if ks.iter().any(|k| self.areas[*k].effect.dark) {
            Some("darkness")
        } else if ks.iter().any(|k| self.areas[*k].effect.obscures) {
            Some("fog")
        } else {
            None
        }
    }

    /// Whether a lasting area makes tile `t` difficult for `who`.
    pub(super) fn area_difficult(&self, t: TilePos, who: Who) -> bool {
        self.areas_at(t).into_iter().any(|k| self.areas[k].effect.difficult && self.area_touches(k, who))
    }

    /// Whether `who` stands in a Silence.
    pub(super) fn silenced(&self, who: Who) -> bool {
        let t = self.who_tile(who);
        self.areas_at(t).into_iter().any(|k| self.areas[k].effect.silence)
    }

    /// The area's parts act again on `who` (a turn's start, entering, a step).
    fn area_acts(&mut self, k: usize, who: Who) {
        let (spell, slot, caster) = (self.areas[k].spell.clone(), self.areas[k].slot, self.areas[k].caster);
        let Some(d) = self.project.db.spell(&spell).cloned() else { return };
        self.areas[k].hit.push(who);
        self.spell_on(caster, &d, slot, &[who], None, true);
    }

    /// `who` starts its turn (in a fight) or six seconds pass (outside one): the repeating areas it
    /// is in act on it.
    pub(super) fn areas_turn(&mut self, who: Who) {
        let t = self.who_tile(who);
        for k in self.areas_at(t) {
            if k < self.areas.len() && self.areas[k].effect.repeat && self.area_touches(k, who) {
                self.areas[k].hit.retain(|w| *w != who);
                self.area_acts(k, who);
            }
        }
    }

    /// `who` stepped onto `t`: the repeating areas there act (once a turn; every tile with
    /// `per_tile`).
    pub(super) fn areas_step(&mut self, who: Who, t: TilePos) {
        for k in self.areas_at(t) {
            if k >= self.areas.len() {
                continue;
            }
            let a = &self.areas[k];
            if !a.effect.repeat || !self.area_touches(k, who) || (!a.effect.per_tile && a.hit.contains(&who)) {
                continue;
            }
            self.area_acts(k, who);
        }
    }

    /// The `hit` lists start over with `who`'s turn.
    pub(super) fn areas_new_turn(&mut self, who: Who) {
        for a in &mut self.areas {
            a.hit.retain(|w| *w != who);
        }
    }

    /// Time passes: `dt` seconds outside fights (every six seconds everyone inside a repeating area
    /// is hit), in a fight a round on the caster's turn (`only`). Areas of another map are gone.
    pub(super) fn tick_areas(&mut self, dt: f32, only: Option<Who>) {
        let map = self.loaded.map.name.clone();
        self.areas.retain(|a| a.map == map);
        let mut k = 0;
        while k < self.areas.len() {
            if only.is_none_or(|w| w == self.areas[k].caster) {
                self.areas[k].left -= dt;
                if self.areas[k].left <= 0.0 {
                    self.end_area(k);
                    continue;
                }
            }
            k += 1;
        }
        if only.is_some() || self.fight.is_some() {
            return;
        }
        let mut due = Vec::new();
        for (k, a) in self.areas.iter_mut().enumerate() {
            a.tick -= dt;
            if a.tick <= 0.0 {
                a.tick += crate::srd::conditions::ROUND;
                if a.effect.repeat {
                    due.push(k);
                }
            }
        }
        for k in due {
            let mut inside: Vec<Who> = vec![Who::Player];
            inside.extend(self.loaded.entities.iter().filter(|e| e.active && e.kind == crate::entity::EntityKind::Character && e.max_hp > 0 && e.hp > 0).map(|e| Who::Npc(e.uid)));
            let tiles = self.area_now(k);
            for w in inside {
                if k < self.areas.len() && tiles.contains(&self.who_tile(w)) && self.area_touches(k, w) {
                    self.area_acts(k, w);
                }
            }
        }
    }

    /// Area `k` ends (a wall's tiles are as they were).
    fn end_area(&mut self, k: usize) {
        let a = self.areas.remove(k);
        for (i, s, h, w) in a.walls {
            if let Some(x) = self.loaded.solid.get_mut(i) {
                *x = s;
            }
            if let Some(x) = self.loaded.walls.get_mut(i) {
                *x = w;
            }
            if let Some(y) = self.loaded.shot.get_mut(i) {
                *y = h;
            }
        }
    }

    /// The spell sources of the lasting areas covering tile `p` (Dispel Magic).
    pub(super) fn area_sources_at(&self, p: TilePos) -> Vec<String> {
        self.areas_at(p).into_iter().map(|k| self.areas[k].source.clone()).collect()
    }

    /// The areas of a spell's source end with it (concentration).
    pub(super) fn end_areas_of(&mut self, source: &str) {
        while let Some(k) = self.areas.iter().position(|a| a.source == source) {
            self.end_area(k);
        }
    }

    /// Draws the areas on the map (world space): fog grey, darkness black, walls stone, the
    /// others a tint by what they do.
    pub(super) fn draw_areas(&self) {
        let ts = self.ts();
        for k in 0..self.areas.len() {
            let a = &self.areas[k];
            if a.map != self.loaded.map.name {
                continue;
            }
            let e = &a.effect;
            let col = if e.dark {
                Color::new(0.0, 0.0, 0.02, 0.85)
            } else if e.wall {
                Color::new(0.45, 0.43, 0.4, 0.95)
            } else if e.obscures {
                Color::new(0.78, 0.8, 0.82, 0.6)
            } else if e.silence {
                Color::new(0.4, 0.5, 0.9, 0.18)
            } else if e.foes_only {
                Color::new(1.0, 0.85, 0.4, 0.18)
            } else if e.difficult && !e.repeat {
                Color::new(0.45, 0.3, 0.15, 0.25)
            } else {
                Color::new(1.0, 0.5, 0.3, 0.2)
            };
            for t in self.area_now(k) {
                draw_rectangle(t.0 as f32 * ts, t.1 as f32 * ts, ts, ts, col);
            }
        }
    }

    /// `areas=` for `status`: `spell:tiles:seconds` per area (`-` without).
    pub(super) fn areas_status(&self) -> String {
        if self.areas.is_empty() {
            return "-".into();
        }
        (0..self.areas.len()).map(|k| format!("{}:{}:{:.0}s", self.areas[k].spell, self.area_now(k).len(), self.areas[k].left)).collect::<Vec<_>>().join(",")
    }
}

#[cfg(test)]
mod tests {
    use crate::skills::{Lasting, SpellDef, SpellPart};

    #[test]
    fn the_library_spells_keep_their_areas() {
        let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/assets/starter_library/defs/spells.ron");
        let spells: Vec<SpellDef> = ron::from_str(&std::fs::read_to_string(lib).unwrap()).unwrap();
        let lasting = |id: &str| spells.iter().find(|s| s.id == id).and_then(|s| s.parts.iter().find_map(|p| if let SpellPart::Lasting { effect } = p { Some(effect.clone()) } else { None }));
        let sg = lasting("spirit-guardians").unwrap();
        assert!(sg.follows && sg.foes_only && sg.repeat && sg.later && sg.feet == 15);
        assert!(lasting("darkness").unwrap().dark && lasting("wall-of-stone").unwrap().wall);
        let spike = lasting("spike-growth").unwrap();
        assert!(spike.per_tile && spike.difficult);
        // (Only the switches set are written.)
        let text = ron::to_string(&Lasting { obscures: true, ..Default::default() }).unwrap();
        assert_eq!(text, "(obscures:true)");
    }

    #[test]
    fn revival_spells_need_their_diamonds() {
        let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/assets/starter_library/defs/spells.ron");
        let spells: Vec<SpellDef> = ron::from_str(&std::fs::read_to_string(lib).unwrap()).unwrap();
        let get = |id: &str| spells.iter().find(|s| s.id == id).unwrap();
        let r = get("revivify");
        assert_eq!(r.costly, Some(crate::skills::Costly { item: "diamond".into(), count: 3 }));
        assert!(matches!(r.parts[..], [SpellPart::Revive { full: false, .. }]));
        assert!(matches!(get("resurrection").parts[..], [SpellPart::Revive { full: true, .. }]));
        // (A spell without one writes nothing.)
        assert!(get("fireball").costly.is_none());
    }
}
