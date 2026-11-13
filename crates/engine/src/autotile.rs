//! Auto-tiling terrains: paint "grass / water" or "dirt path" and the editor picks the edge
//! tiles. A terrain lives in a tileset (`TilesetDef::terrains`) and has one tile per
//! combination of its four neighbours (north, east, south, west) being the same terrain: 16
//! tiles. Which cells belong to a terrain is read from the tiles themselves (a cell whose tile
//! is one of the 16), so maps store nothing new and the game needs nothing to show them.
//! Terrains can lie on each other: a terrain *joins* others whose cells count as its own for
//! its edges (sand joins the shore water lying on it, so sand shows no grass edge there).
//! A terrain with a *blob* set (`Terrain::blob`, 47 tiles) also looks at the four diagonal
//! neighbours, so an L-shaped path gets an inner corner instead of a hard one.

use serde::{Deserialize, Serialize};

use crate::map::{Map, Tile};

/// Neighbour bits of a terrain mask: that neighbour is the same terrain.
pub const NORTH: usize = 1;
pub const EAST: usize = 2;
pub const SOUTH: usize = 4;
pub const WEST: usize = 8;
/// Diagonal bits of an 8-neighbour mask (blob terrains). A diagonal counts only when both
/// sides next to it are set (`reduce`), which leaves 47 distinct masks.
pub const NORTH_EAST: usize = 16;
pub const SOUTH_EAST: usize = 32;
pub const SOUTH_WEST: usize = 64;
pub const NORTH_WEST: usize = 128;
/// Number of slots of a blob set.
pub const BLOB_SLOTS: usize = 47;

/// An 8-neighbour mask without the diagonals whose two sides aren't both set.
pub const fn reduce(m: usize) -> usize {
    let mut r = m & 15;
    if m & NORTH_EAST != 0 && m & (NORTH | EAST) == NORTH | EAST {
        r |= NORTH_EAST;
    }
    if m & SOUTH_EAST != 0 && m & (SOUTH | EAST) == SOUTH | EAST {
        r |= SOUTH_EAST;
    }
    if m & SOUTH_WEST != 0 && m & (SOUTH | WEST) == SOUTH | WEST {
        r |= SOUTH_WEST;
    }
    if m & NORTH_WEST != 0 && m & (NORTH | WEST) == NORTH | WEST {
        r |= NORTH_WEST;
    }
    r
}

/// The 47 reduced masks in slot order: ascending, so slots 0–15 are the masks without
/// diagonals (in the 16-tile order) and slot 46 is the inside (255).
pub const BLOB_MASKS: [u8; BLOB_SLOTS] = {
    let mut out = [0u8; BLOB_SLOTS];
    let (mut m, mut n) = (0, 0);
    while m < 256 {
        if reduce(m) == m {
            out[n] = m as u8;
            n += 1;
        }
        m += 1;
    }
    out
};

/// Slot of every raw 8-neighbour mask (256 → 47).
pub const BLOB_SLOT: [u8; 256] = {
    let mut out = [0u8; 256];
    let mut m = 0;
    while m < 256 {
        let r = reduce(m);
        let mut k = 0;
        while BLOB_MASKS[k] as usize != r {
            k += 1;
        }
        out[m] = k as u8;
        m += 1;
    }
    out
};

/// An auto-tiling terrain of a tileset.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Terrain {
    pub name: String,
    /// Tile index (in the terrain's tileset) per neighbour mask (`NORTH | EAST | …`):
    /// `tiles[15]` is the inside, `tiles[0]` a cell on its own. `None` = not set (another
    /// tile of the terrain is used instead).
    pub tiles: [Option<u32>; 16],
    /// What erasing the terrain leaves on the cell (e.g. plain grass); `None` = no tile.
    #[serde(default)]
    pub base: Option<u32>,
    /// Names of other terrains of the tileset that lie on this one: their cells count as this
    /// terrain when its edges are chosen (sand joins the shore water inside it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<String>,
    /// The 47-tile ("blob") set, one tile per slot of `BLOB_MASKS`: also looks at the diagonal
    /// neighbours (inner corners). Empty = a 16-tile terrain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blob: Vec<Option<u32>>,
    /// Footstep sound of its tiles (sound id; "" = none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub step: String,
}

impl Terrain {
    /// Whether a tile of the tileset is one of the terrain's tiles.
    pub fn contains(&self, tile: u32) -> bool {
        self.tiles.contains(&Some(tile)) || self.blob.contains(&Some(tile))
    }

    /// Whether the terrain uses its 47-tile set (8 neighbours).
    pub fn is_blob(&self) -> bool {
        !self.blob.is_empty()
    }

    /// Blob slot `k`'s tile, if set.
    pub fn blob_tile(&self, k: usize) -> Option<u32> {
        self.blob.get(k).copied().flatten()
    }

    /// The tile for a neighbour mask (8 bits; a 16-tile terrain looks at the four sides only).
    /// A blob slot that isn't set falls back to the 16-tile set's tile for its sides, then the
    /// inside, then any tile, so a half-filled blob set still paints.
    pub fn tile_for(&self, mask: usize) -> Option<u32> {
        let blob = if self.is_blob() { self.blob_tile(BLOB_SLOT[mask & 255] as usize) } else { None };
        blob.or(self.tiles[mask & 15]).or_else(|| self.inside()).or_else(|| self.tiles.iter().chain(&self.blob).flatten().next().copied())
    }

    /// The inside tile (all neighbours the same terrain).
    pub fn inside(&self) -> Option<u32> {
        self.blob_tile(BLOB_SLOTS - 1).or(self.tiles[15])
    }

    /// Number of masks that have their own tile (of 16, or of 47 for a blob set).
    pub fn tiles_set(&self) -> usize {
        if self.is_blob() {
            self.blob.iter().flatten().count()
        } else {
            self.tiles.iter().flatten().count()
        }
    }

    /// How many tiles a full set has: 16 or 47.
    pub fn slots(&self) -> usize {
        if self.is_blob() {
            BLOB_SLOTS
        } else {
            16
        }
    }

    /// Tile index of blob slot `k` in a 7×7 block laid out in slot order (row by row) whose
    /// top-left tile is `first`.
    pub fn blob_block_tile(first: u32, columns: u32, k: usize) -> u32 {
        first + (k as u32 / 7) * columns + k as u32 % 7
    }

    /// Sets all 47 blob slots from such a 7×7 block (the last two cells are unused).
    pub fn fill_blob_from_block(&mut self, first: u32, columns: u32) {
        self.blob = (0..BLOB_SLOTS).map(|k| Some(Self::blob_block_tile(first, columns, k))).collect();
    }

    /// Turns the 47-tile set on (empty slots) or off (dropped).
    pub fn set_blob(&mut self, on: bool) {
        if on && !self.is_blob() {
            self.blob = vec![None; BLOB_SLOTS];
        } else if !on {
            self.blob.clear();
        }
    }

    /// Tile index of mask `k` in a 4×4 block laid out in mask order (row by row) whose
    /// top-left tile is `first`, in a tileset `columns` tiles wide.
    pub fn block_tile(first: u32, columns: u32, k: usize) -> u32 {
        first + (k as u32 / 4) * columns + k as u32 % 4
    }

    /// Sets all 16 tiles from such a block.
    pub fn fill_from_block(&mut self, first: u32, columns: u32) {
        for k in 0..16 {
            self.tiles[k] = Some(Self::block_tile(first, columns, k));
        }
    }
}

/// Neighbour mask of cell `(x, y)`: which of its four neighbours are `member`s.
pub fn mask(x: i32, y: i32, member: impl Fn(i32, i32) -> bool) -> usize {
    let mut m = 0;
    for (bit, dx, dy) in [(NORTH, 0, -1), (EAST, 1, 0), (SOUTH, 0, 1), (WEST, -1, 0)] {
        if member(x + dx, y + dy) {
            m |= bit;
        }
    }
    m
}

/// 8-neighbour mask of cell `(x, y)` (sides and diagonals that are `member`s; not reduced).
pub fn mask8(x: i32, y: i32, member: impl Fn(i32, i32) -> bool) -> usize {
    let mut m = mask(x, y, &member);
    for (bit, dx, dy) in [(NORTH_EAST, 1, -1), (SOUTH_EAST, 1, 1), (SOUTH_WEST, -1, 1), (NORTH_WEST, -1, -1)] {
        if member(x + dx, y + dy) {
            m |= bit;
        }
    }
    m
}

/// Which of a tileset's terrains a tile belongs to.
pub fn terrain_of(terrains: &[Terrain], tile: u32) -> Option<usize> {
    terrains.iter().position(|t| t.contains(tile))
}

/// Whether a tile counts as terrain `k` for its edges: one of its tiles, or of a terrain it
/// joins.
pub fn counts_as(terrains: &[Terrain], k: usize, tile: u32) -> bool {
    let Some(t) = terrains.get(k) else { return false };
    t.contains(tile) || terrains.iter().any(|o| t.joins.contains(&o.name) && o.contains(tile))
}

/// The tile changes for painting (`erase` false) or erasing one cell of terrain `k` (of the
/// tileset's `terrains`) on `layer`: the cell gets terrain `k`'s tile for its neighbours, and
/// its four neighbours that belong to a terrain of the tileset are re-tiled (each with its own
/// terrain; the diagonal neighbours too when their terrain has a blob set). `slot` is the map's tileset slot of that tileset. Cells outside the map count as
/// every terrain (terrains continue past the edge). Erasing changes only a cell of terrain `k`:
/// it gets the terrain's base tile (re-tiled if that is a tile of another terrain). Returns `(x, y, tile)` for every cell whose tile changes.
pub fn paint(map: &Map, layer: usize, slot: u16, terrains: &[Terrain], k: usize, (x, y): (i32, i32), erase: bool) -> Vec<(u32, u32, Tile)> {
    let (Some(l), Some(terrain)) = (map.layers.get(layer), terrains.get(k)) else { return Vec::new() };
    if !map.in_bounds(x, y) {
        return Vec::new();
    }
    let of_slot = |cx: i32, cy: i32| l.get(cx as u32, cy as u32).parts().filter(|&(s, _)| s == slot as usize).map(|(_, i)| i);
    if erase && !of_slot(x, y).is_some_and(|i| terrain.contains(i)) {
        return Vec::new();
    }
    let Some(placeholder) = terrain.inside() else { return Vec::new() };
    // The tile index each cell will have (the painted cell with any tile of its terrain).
    let virt = |cx: i32, cy: i32| if (cx, cy) == (x, y) { if erase { terrain.base } else { Some(placeholder) } } else { of_slot(cx, cy) };
    let mut out = Vec::new();
    let cells = [(x, y), (x, y - 1), (x + 1, y), (x, y + 1), (x - 1, y), (x + 1, y - 1), (x + 1, y + 1), (x - 1, y + 1), (x - 1, y - 1)];
    for (n, &(cx, cy)) in cells.iter().enumerate() {
        if !map.in_bounds(cx, cy) {
            continue;
        }
        let diagonal = n >= 5;
        // An erased cell gets the base tile, re-tiled when the base belongs to a terrain.
        let plain_base = erase && (cx, cy) == (x, y) && !terrain.base.is_some_and(|b| terrain_of(terrains, b).is_some());
        let new = if plain_base {
            terrain.base.map_or(Tile::EMPTY, |b| Tile::new(slot, b))
        } else {
            // The painted cell is terrain `k`; a neighbour whose tile several terrains share is
            // taken as `k` too when it is one of them (a tile should belong to one terrain).
            let kk = if (cx, cy) == (x, y) && !erase {
                k
            } else {
                let Some(i) = virt(cx, cy) else { continue };
                if terrain.contains(i) {
                    k
                } else {
                    let Some(o) = terrain_of(terrains, i) else { continue };
                    o
                }
            };
            // A diagonal neighbour's sides don't change: only blob terrains look at it.
            if diagonal && !terrains[kk].is_blob() {
                continue;
            }
            let member = |nx: i32, ny: i32| !map.in_bounds(nx, ny) || virt(nx, ny).is_some_and(|i| counts_as(terrains, kk, i));
            match terrains[kk].tile_for(mask8(cx, cy, member)) {
                Some(t) => Tile::new(slot, t),
                None => continue,
            }
        };
        if l.get(cx as u32, cy as u32) != new {
            out.push((cx as u32, cy as u32, new));
        }
    }
    out
}

/// Paints a terrain cell straight into a map (tests and tools; the editor goes through its
/// undo history instead).
pub fn paint_into(map: &mut Map, layer: usize, slot: u16, terrains: &[Terrain], k: usize, cell: (i32, i32), erase: bool) {
    for (cx, cy, t) in paint(map, layer, slot, terrains, k, cell, erase) {
        map.layers[layer].set(cx, cy, t);
    }
}

/// Re-tiles a whole layer at once (generators): every cell `pick` assigns a terrain gets that
/// terrain's tile for its neighbours, where `member(k, x, y)` says whether a cell counts as
/// terrain `k`. Membership is decided on the layer as it was before (no order effects); cells
/// outside the map count as every terrain.
pub fn retile(map: &mut Map, layer: usize, slot: u16, terrains: &[Terrain], pick: impl Fn(&Map, i32, i32) -> Option<usize>, member: impl Fn(&Map, usize, i32, i32) -> bool) {
    let (w, h) = (map.width as i32, map.height as i32);
    let mut changes = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let Some(k) = pick(map, x, y) else { continue };
            let m = mask8(x, y, |nx, ny| !map.in_bounds(nx, ny) || member(map, k, nx, ny));
            if let Some(t) = terrains.get(k).and_then(|t| t.tile_for(m)) {
                changes.push((x as u32, y as u32, Tile::new(slot, t)));
            }
        }
    }
    for (x, y, t) in changes {
        map.layers[layer].set(x, y, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A terrain whose tile for mask `m` is `100 + m`, erasing to tile 7.
    fn terrain() -> Terrain {
        let mut t = Terrain { name: "pond".into(), base: Some(7), ..Default::default() };
        t.fill_from_block(100, 4);
        t
    }

    fn idx(map: &Map, x: u32, y: u32) -> Option<u32> {
        map.layers[0].get(x, y).parts().map(|p| p.1)
    }

    #[test]
    fn block_layout_and_fallbacks() {
        assert_eq!(Terrain::block_tile(40, 8, 0), 40);
        assert_eq!(Terrain::block_tile(40, 8, 5), 49);
        assert_eq!(Terrain::block_tile(40, 8, 15), 67);
        let mut t = terrain();
        assert_eq!((t.tile_for(NORTH | SOUTH), t.tiles_set()), (Some(105), 16));
        t.tiles[5] = None;
        assert_eq!(t.tile_for(5), Some(115));
        assert!(t.contains(100) && !t.contains(7));
        assert_eq!(Terrain::default().tile_for(3), None);
    }

    #[test]
    fn painting_picks_edges_and_updates_neighbours() {
        let mut map = Map::new("m", 6, 6);
        let slot = map.tileset_slot("terrain");
        let ts = [terrain()];
        // A single cell has no terrain neighbours.
        paint_into(&mut map, 0, slot, &ts, 0, (2, 2), false);
        assert_eq!(idx(&map, 2, 2), Some(100));
        // A second cell east of it: both get their joined edge.
        paint_into(&mut map, 0, slot, &ts, 0, (3, 2), false);
        assert_eq!((idx(&map, 2, 2), idx(&map, 3, 2)), (Some(100 + EAST as u32), Some(100 + WEST as u32)));
        // At the map edge the terrain continues outside.
        paint_into(&mut map, 0, slot, &ts, 0, (0, 0), false);
        assert_eq!(idx(&map, 0, 0), Some(100 + (NORTH | WEST) as u32));
        // Erasing leaves the base tile and fixes the neighbour; erasing other tiles does nothing.
        paint_into(&mut map, 0, slot, &ts, 0, (3, 2), true);
        assert_eq!((idx(&map, 2, 2), idx(&map, 3, 2)), (Some(100), Some(7)));
        assert!(paint(&map, 0, slot, &ts, 0, (3, 2), true).is_empty());
        assert!(paint(&map, 0, slot, &ts, 0, (9, 9), false).is_empty());
        // Tiles of another tileset slot are not the terrain.
        let other = map.tileset_slot("other");
        map.layers[0].set(2, 1, Tile::new(other, 100 + 15));
        paint_into(&mut map, 0, slot, &ts, 0, (2, 3), false);
        assert_eq!(idx(&map, 2, 2), Some(100 + SOUTH as u32));
    }

    #[test]
    fn joined_terrains_lie_on_each_other() {
        // Sand (tiles 100..) joins Shore (tiles 200..): sand next to shore water has no edge
        // there, and painting shore re-tiles the sand around it.
        let mut sand = Terrain { name: "Sand".into(), base: Some(7), joins: vec!["Shore".into()], ..Default::default() };
        sand.fill_from_block(100, 4);
        let mut shore = Terrain { name: "Shore".into(), base: Some(115), ..Default::default() };
        shore.fill_from_block(200, 4);
        let ts = [sand, shore];
        let mut map = Map::new("m", 7, 3);
        let slot = map.tileset_slot("t");
        for x in 1..6 {
            paint_into(&mut map, 0, slot, &ts, 0, (x, 1), false);
        }
        assert_eq!(idx(&map, 3, 1), Some(100 + (EAST | WEST) as u32));
        assert!(counts_as(&ts, 0, 203) && !counts_as(&ts, 1, 103));
        paint_into(&mut map, 0, slot, &ts, 1, (3, 1), false);
        // The shore cell has sand on both sides (not shore): its own single tile.
        assert_eq!(idx(&map, 3, 1), Some(200));
        // The sand still joins up with it.
        assert_eq!(idx(&map, 2, 1), Some(100 + (EAST | WEST) as u32));
        assert_eq!(terrain_of(&ts, 200), Some(1));
        // Erasing the shore leaves its base, a sand tile, re-tiled as sand.
        paint_into(&mut map, 0, slot, &ts, 1, (3, 1), true);
        assert_eq!(idx(&map, 3, 1), Some(100 + (EAST | WEST) as u32));
        // Whole-layer retiling with a custom membership.
        let mut m2 = Map::new("m2", 3, 1);
        retile(&mut m2, 0, slot, &ts, |_, x, _| (x == 1).then_some(0), |_, _, x, _| x == 1);
        assert_eq!(idx(&m2, 1, 0), Some(100 + (NORTH | SOUTH) as u32));
    }

    /// A 90° clockwise turn of an 8-neighbour mask (north becomes east, north-east south-east).
    fn rotate(m: usize) -> usize {
        let card = ((m & 15) << 1 | (m & 15) >> 3) & 15;
        let diag = ((m >> 4) << 1 | (m >> 4) >> 3) & 15;
        card | diag << 4
    }

    /// A blob terrain whose tile for slot `k` is `300 + k`.
    fn blob_terrain() -> Terrain {
        let mut t = Terrain { name: "path".into(), base: Some(7), ..Default::default() };
        t.blob = (0..BLOB_SLOTS).map(|k| Some(300 + k as u32)).collect();
        t
    }

    fn slot_of(m: usize) -> u32 {
        300 + BLOB_SLOT[m] as u32
    }

    #[test]
    fn blob_table_has_47_masks_closed_under_rotation() {
        let distinct: std::collections::BTreeSet<u8> = BLOB_SLOT.iter().copied().collect();
        assert_eq!(distinct.len(), 47);
        assert_eq!((BLOB_MASKS[0], BLOB_MASKS[15], BLOB_MASKS[46]), (0, 15, 255));
        for (k, &m) in BLOB_MASKS.iter().enumerate() {
            assert_eq!(BLOB_SLOT[m as usize] as usize, k);
            // A turned valid mask is valid again (the set is symmetric).
            assert_eq!(reduce(rotate(m as usize)), rotate(m as usize));
        }
        // Diagonals without both sides don't count.
        assert_eq!(BLOB_SLOT[NORTH_EAST | NORTH], BLOB_SLOT[NORTH]);
        assert_eq!(BLOB_SLOT[255], 46);
        assert_eq!(rotate(NORTH | NORTH_EAST), EAST | SOUTH_EAST);
    }

    #[test]
    fn blob_painting_gives_inner_corners() {
        let mut map = Map::new("m", 6, 6);
        let slot = map.tileset_slot("t");
        let ts = [blob_terrain()];
        // An L of 2-wide arms: (1..4, 1..3) and (1..3, 3..5).
        for (x, y) in [(1, 1), (2, 1), (3, 1), (1, 2), (2, 2), (3, 2), (1, 3), (2, 3), (1, 4), (2, 4)] {
            paint_into(&mut map, 0, slot, &ts, 0, (x, y), false);
        }
        // The cell in the bend has terrain on all four sides but not south-east: inner corner.
        assert_eq!(idx(&map, 2, 2), Some(slot_of(255 & !SOUTH_EAST)));
        assert_ne!(idx(&map, 2, 2), Some(slot_of(255)));
        // The top-left cell: east, south and south-east.
        assert_eq!(idx(&map, 1, 1), Some(slot_of(EAST | SOUTH | SOUTH_EAST)));
        // Painting the bend's missing diagonal re-tiles the diagonal neighbour too.
        paint_into(&mut map, 0, slot, &ts, 0, (3, 3), false);
        assert_eq!(idx(&map, 2, 2), Some(slot_of(255)));
        // Erasing it brings the inner corner back.
        paint_into(&mut map, 0, slot, &ts, 0, (3, 3), true);
        assert_eq!(idx(&map, 2, 2), Some(slot_of(255 & !SOUTH_EAST)));
        // Whole-layer retiling looks at diagonals too.
        let mut m2 = map.clone();
        retile(&mut m2, 0, slot, &ts, |m, x, y| m.layers[0].get(x as u32, y as u32).parts().filter(|p| ts[0].contains(p.1)).map(|_| 0), |m, _, x, y| {
            m.layers[0].get(x as u32, y as u32).parts().is_some_and(|p| ts[0].contains(p.1))
        });
        assert_eq!(idx(&m2, 2, 2), Some(slot_of(255 & !SOUTH_EAST)));
    }

    #[test]
    fn half_filled_blob_sets_fall_back() {
        let mut t = terrain();
        t.set_blob(true);
        assert_eq!((t.slots(), t.tiles_set()), (47, 0));
        // No blob tiles yet: the 16-tile set answers.
        assert_eq!(t.tile_for(NORTH | EAST | NORTH_EAST), Some(100 + (NORTH | EAST) as u32));
        t.blob[46] = Some(999);
        assert_eq!((t.inside(), t.tile_for(255), t.tile_for(15)), (Some(999), Some(999), Some(115)));
        t.fill_blob_from_block(40, 10);
        assert_eq!((t.blob_tile(0), t.blob_tile(7), t.blob_tile(46)), (Some(40), Some(50), Some(40 + 6 * 10 + 4)));
        t.set_blob(false);
        assert!(!t.is_blob() && t.tile_for(255) == Some(115));
        // A blob set without 16 tiles paints from the blob alone.
        let b = blob_terrain();
        assert_eq!(b.tile_for(NORTH | NORTH_EAST), Some(slot_of(NORTH)));
        assert!(b.contains(346) && !b.contains(115));
    }

    #[test]
    fn old_tileset_files_load_without_terrains() {
        // A terrain of 0.4.0 (no blob set) loads and paints as before.
        let old = r#"(id: "t", image: "t.png", solid: [], animations: [], terrains: [(name: "pond", tiles: (Some(1), None, None, None, None, None, None, None, None, None, None, None, None, None, None, Some(2)), base: None)])"#;
        let d: crate::defs::TilesetDef = ron::from_str(old).unwrap();
        assert!(!d.terrains[0].is_blob());
        assert_eq!(d.terrains[0].tile_for(255), Some(2));
        assert!(!ron::to_string(&d).unwrap().contains("blob"));
        let d: crate::defs::TilesetDef = ron::from_str(r#"(id: "t", image: "t.png", solid: [1], animations: [])"#).unwrap();
        assert!(d.terrains.is_empty());
        let mut d2 = d.clone();
        d2.terrains.push(terrain());
        let back: crate::defs::TilesetDef = ron::from_str(&ron::to_string(&d2).unwrap()).unwrap();
        assert_eq!(back.terrains, d2.terrains);
        assert!(!ron::to_string(&d).unwrap().contains("terrains"));
    }
}
