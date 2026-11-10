//! Which tiles solid entities stand on, rebuilt once per frame: path planning (round props,
//! round people when blocked) and the broad phase of collision look here instead of scanning
//! every entity each time.

use macroquad::prelude::Rect;

use crate::entity::EntityKind;
use crate::grid::{tiles_of_rect, TilePos};

/// A solid prop covers the tile (its inner square, see [`Occupancy::covered`]): routes go round it.
pub(super) const OCC_PROP: u8 = 1;
/// A solid character covers the tile: routes planned in `avoid` mode go round it.
pub(super) const OCC_CHAR: u8 = 2;
/// A solid entity's hitbox touches this tile or a neighbour: collision has to look at entities.
pub(super) const OCC_NEAR: u8 = 4;
/// A solid prop that isn't low covers the tile: it stops shots and sight.
pub(super) const OCC_HIGH: u8 = 8;

#[derive(Default)]
pub(super) struct Occupancy {
    w: u32,
    h: u32,
    cells: Vec<u8>,
}

impl Occupancy {
    pub fn new(w: u32, h: u32) -> Self {
        Occupancy { w, h, cells: vec![0; (w as usize) * (h as usize)] }
    }

    pub fn clear(&mut self) {
        self.cells.fill(0);
    }

    /// Marks what a solid entity of `kind` with hitbox `r` covers ([`Self::covered`]), and
    /// `OCC_NEAR` on every tile the hitbox touches and their neighbours (an entity moves less
    /// than a tile a frame, so the mark holds until the next rebuild).
    pub fn add(&mut self, kind: EntityKind, r: Rect, ts: f32) {
        let flag = match kind {
            EntityKind::Prop => OCC_PROP,
            EntityKind::Character => OCC_CHAR,
            EntityKind::Item => 0,
        };
        if flag != 0 {
            for (x, y) in Self::covered(r, ts) {
                self.or(x, y, flag);
            }
        }
        let (x0, y0) = ((r.x / ts).floor() as i32 - 1, (r.y / ts).floor() as i32 - 1);
        let (x1, y1) = (((r.x + r.w) / ts).floor() as i32 + 1, ((r.y + r.h) / ts).floor() as i32 + 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.or(x, y, OCC_NEAR);
            }
        }
    }

    /// Marks the tiles a solid, not low prop with hitbox `r` covers as stopping shots.
    pub fn add_high(&mut self, r: Rect, ts: f32) {
        for (x, y) in Self::covered(r, ts) {
            self.or(x, y, OCC_HIGH);
        }
    }

    /// The tiles a hitbox covers for planning: those whose inner square (2 px in) it really
    /// overlaps, so a small hitbox at the bottom of its tile blocks only that tile.
    pub fn covered(r: Rect, ts: f32) -> impl Iterator<Item = TilePos> {
        let (x0, y0) = ((r.x / ts).floor() as i32, (r.y / ts).floor() as i32);
        let (x1, y1) = (((r.x + r.w) / ts).floor() as i32, ((r.y + r.h) / ts).floor() as i32);
        (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y))).filter(move |&(x, y)| r.overlaps(&Rect::new(x as f32 * ts + 2.0, y as f32 * ts + 2.0, ts - 4.0, ts - 4.0)))
    }

    fn or(&mut self, x: i32, y: i32, flag: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            self.cells[(y as u32 * self.w + x as u32) as usize] |= flag;
        }
    }

    /// The flags of a tile (0 outside the map).
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            0
        } else {
            self.cells[(y as u32 * self.w + x as u32) as usize]
        }
    }

    pub fn has(&self, x: i32, y: i32, flag: u8) -> bool {
        self.get(x, y) & flag != 0
    }

    /// Does a tile under `r` carry `flag`? (Far edges exclusive, like collision.)
    pub fn any_in(&self, r: Rect, ts: f32, flag: u8) -> bool {
        tiles_of_rect(r, ts).any(|(x, y)| self.has(x, y, flag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_and_lookups() {
        let ts = 32.0;
        let mut o = Occupancy::new(8, 8);
        // A 24 x 16 hitbox with its feet at the bottom of tile (3, 3): covers only that tile.
        o.add(EntityKind::Prop, Rect::new(3.0 * ts + 4.0, 3.0 * ts + 15.0, 24.0, 16.0), ts);
        assert!(o.has(3, 3, OCC_PROP) && !o.has(3, 2, OCC_PROP) && !o.has(3, 4, OCC_PROP));
        assert!(!o.has(3, 3, OCC_CHAR));
        // The broad-phase mark reaches the neighbours; two tiles away it doesn't.
        assert!(o.has(2, 2, OCC_NEAR) && o.has(4, 4, OCC_NEAR) && !o.has(5, 5, OCC_NEAR) && !o.has(3, 1, OCC_NEAR));
        assert!(o.any_in(Rect::new(4.0 * ts, 4.0 * ts, 8.0, 8.0), ts, OCC_NEAR));
        assert!(!o.any_in(Rect::new(6.0 * ts, 6.0 * ts, 8.0, 8.0), ts, OCC_NEAR));
        // A character straddling two tiles covers both; items cover nothing but are "near".
        o.add(EntityKind::Character, Rect::new(5.0 * ts - 8.0, 5.0 * ts + 10.0, 16.0, 20.0), ts);
        assert!(o.has(4, 5, OCC_CHAR) && o.has(5, 5, OCC_CHAR));
        o.add(EntityKind::Item, Rect::new(0.0, 0.0, 8.0, 8.0), ts);
        assert_eq!(o.get(0, 0), OCC_NEAR);
        assert_eq!(o.get(-1, 0), 0);
        o.clear();
        assert_eq!(o.get(3, 3), 0);
    }
}
