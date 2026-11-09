//! Pixel ↔ tile conventions shared by the game and the editor. A position is the *feet*: the
//! bottom centre of the hitbox. Whoever stands on tile `(tx, ty)` has its feet one pixel above
//! the tile's bottom edge ([`FEET_INSET`]), so `feet_tile(feet_spot(t)) == t`, and a position
//! exactly on a tile boundary belongs to the tile above it (where the sprite's bottom row is).
//! The editor writes every placed entity at `feet_spot` (or [`snap_feet`] for wide props), so
//! these functions give exactly the tile the editor shows.

use macroquad::prelude::{vec2, Rect, Vec2};

pub use crate::path::TilePos;

/// How far above a tile's bottom edge feet stand on it, in pixels.
pub const FEET_INSET: f32 = 1.0;

/// The tile under feet at `p` (`ts` = tile size in pixels).
pub fn feet_tile(p: Vec2, ts: f32) -> TilePos {
    ((p.x / ts).floor() as i32, ((p.y - FEET_INSET) / ts).floor() as i32)
}

/// Where feet stand on tile `t`: its bottom centre, [`FEET_INSET`] up.
pub fn feet_spot(t: TilePos, ts: f32) -> Vec2 {
    vec2((t.0 as f32 + 0.5) * ts, (t.1 as f32 + 1.0) * ts - FEET_INSET)
}

/// Where a placed entity `tiles_wide` tiles wide stands when dropped at `world`: props an even
/// number of tiles wide stand on the nearest tile edge, odd ones on their tile's centre, so they
/// cover whole tiles; the feet go to the bottom of the tile under `world`.
pub fn snap_feet(world: Vec2, tiles_wide: u32, ts: f32) -> Vec2 {
    let ty = (world.y / ts).floor();
    let x = if tiles_wide.is_multiple_of(2) { (world.x / ts).round() * ts } else { ((world.x / ts).floor() + 0.5) * ts };
    vec2(x, (ty + 1.0) * ts - FEET_INSET)
}

/// The pixel rectangle of tile `t`.
pub fn tile_rect(t: TilePos, ts: f32) -> Rect {
    Rect::new(t.0 as f32 * ts, t.1 as f32 * ts, ts, ts)
}

/// The tiles a rectangle covers. Its far edges are exclusive: a box ending exactly on a tile
/// line doesn't touch the next tile (how collision has always counted them).
pub fn tiles_of_rect(r: Rect, ts: f32) -> impl Iterator<Item = TilePos> {
    let x0 = (r.x / ts).floor() as i32;
    let y0 = (r.y / ts).floor() as i32;
    let x1 = ((r.x + r.w - 0.001) / ts).floor() as i32;
    let y1 = ((r.y + r.h - 0.001) / ts).floor() as i32;
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feet_round_trip() {
        let ts = 32.0;
        for t in [(0, 0), (3, 7), (-2, 5), (1023, 1023)] {
            assert_eq!(feet_tile(feet_spot(t, ts), ts), t);
        }
        // A position exactly on a tile line belongs to the tile above it; one pixel below it
        // belongs to the tile below.
        assert_eq!(feet_tile(vec2(48.0, 64.0), ts), (1, 1));
        assert_eq!(feet_tile(vec2(48.0, 65.0), ts), (1, 2));
        assert_eq!(feet_tile(vec2(48.0, 63.0), ts), (1, 1));
        assert_eq!(feet_spot((2, 3), ts), vec2(80.0, 127.0));
    }

    #[test]
    fn snapping() {
        let ts = 32.0;
        // Odd widths stand on the tile's centre, even ones on the nearest tile edge.
        assert_eq!(snap_feet(vec2(70.0, 100.0), 1, ts), vec2(80.0, 127.0));
        assert_eq!(snap_feet(vec2(70.0, 100.0), 3, ts), vec2(80.0, 127.0));
        assert_eq!(snap_feet(vec2(70.0, 100.0), 2, ts), vec2(64.0, 127.0));
        assert_eq!(snap_feet(vec2(90.0, 100.0), 2, ts), vec2(96.0, 127.0));
        // The centre of tile x snaps a 2-wide prop onto tiles x and x + 1.
        let p = snap_feet(feet_spot((4, 2), ts) - vec2(0.0, ts / 2.0), 2, ts);
        assert_eq!(p, vec2(160.0, 95.0));
    }

    #[test]
    fn rect_tiles() {
        let ts = 32.0;
        let tiles: Vec<TilePos> = tiles_of_rect(Rect::new(30.0, 60.0, 4.0, 4.0), ts).collect();
        assert_eq!(tiles, [(0, 1), (1, 1)]);
        // Ending exactly on a tile line doesn't reach the next tile.
        let tiles: Vec<TilePos> = tiles_of_rect(Rect::new(32.0, 32.0, 32.0, 32.0), ts).collect();
        assert_eq!(tiles, [(1, 1)]);
        assert_eq!(tile_rect((2, 1), ts), Rect::new(64.0, 32.0, 32.0, 32.0));
    }
}
