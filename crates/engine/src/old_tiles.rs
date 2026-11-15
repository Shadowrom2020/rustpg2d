//! The tiles of R(ust)PG2D 0.20 and older, kept only to read old maps, definitions and Tiled
//! files and turn them into painted ground and decorations ([`crate::convert_tiles`]): maps were
//! grids of 32 px tiles from tilesets, in chunked layers (ground, detail, overhead), and
//! auto-tiling terrains picked the edge tiles by the neighbours (16 tiles, or 47 with diagonals).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::defs::LightSpec;
use crate::map::CHUNK_SIZE;

const CHUNK_AREA: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// A tile reference. `0` is the empty tile; otherwise it encodes
/// a tileset slot (index into `Map::tilesets`) and a tile index inside that tileset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tile(pub u32);

impl Tile {
    pub const EMPTY: Tile = Tile(0);
    const INDEX_BITS: u32 = 20;
    const INDEX_MASK: u32 = (1 << Self::INDEX_BITS) - 1;

    pub fn new(tileset_slot: u16, index: u32) -> Tile {
        debug_assert!(tileset_slot < 4095 && index <= Self::INDEX_MASK);
        Tile(((tileset_slot as u32 + 1) << Self::INDEX_BITS) | (index & Self::INDEX_MASK))
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// `(tileset_slot, tile_index)`, or `None` for the empty tile (and for values without a
    /// tileset slot, which only bad data contains).
    pub fn parts(self) -> Option<(usize, u32)> {
        let slot = self.0 >> Self::INDEX_BITS;
        (slot > 0).then(|| ((slot - 1) as usize, self.0 & Self::INDEX_MASK))
    }
}

/// Frozen: stored with postcard (by position, no defaults) inside map files, like [`TileLayer`].
/// A new property of layers or chunks goes into the map's RON `Meta` (`io.rs`), or the format
/// version is bumped with a reader for the old layout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub tiles: Vec<Tile>,
}

impl Chunk {
    fn new() -> Self {
        Chunk { tiles: vec![Tile::EMPTY; CHUNK_AREA] }
    }
    fn is_empty(&self) -> bool {
        self.tiles.iter().all(|t| t.is_empty())
    }
}

/// One drawable layer of tiles, stored as a grid of optional chunks.
///
/// Frozen: map files store it with postcard (fields by position, no defaults), so the `serde`
/// defaults below only apply to other formats. A new layer property goes into the map's RON
/// `Meta` (`io.rs`), or the format version is bumped with a reader for the old layout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileLayer {
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Drawn after entities (tree tops, roofs, ...).
    #[serde(default)]
    pub above_entities: bool,
    chunks_w: u32,
    chunks_h: u32,
    pub(crate) chunks: Vec<Option<Box<Chunk>>>,
}

fn yes() -> bool {
    true
}

impl TileLayer {
    pub fn new(name: &str, width: u32, height: u32, above_entities: bool) -> Self {
        let chunks_w = width.div_ceil(CHUNK_SIZE);
        let chunks_h = height.div_ceil(CHUNK_SIZE);
        TileLayer {
            name: name.to_string(),
            visible: true,
            above_entities,
            chunks_w,
            chunks_h,
            chunks: vec![None; (chunks_w * chunks_h) as usize],
        }
    }

    /// Checks a layer read from a file against the map's size: `get` and `set` index the chunk
    /// grid without bounds checks, so a layer that doesn't fit is refused when loading.
    pub fn validate(&self, width: u32, height: u32) -> Result<(), String> {
        let (cw, ch) = (width.div_ceil(CHUNK_SIZE), height.div_ceil(CHUNK_SIZE));
        if self.chunks_w != cw || self.chunks_h != ch {
            return Err(format!("layer '{}' has a {}x{} chunk grid, the map needs {cw}x{ch}", self.name, self.chunks_w, self.chunks_h));
        }
        if self.chunks.len() != cw as usize * ch as usize {
            return Err(format!("layer '{}' has {} chunks, not {}", self.name, self.chunks.len(), cw as usize * ch as usize));
        }
        if let Some(c) = self.chunks.iter().flatten().find(|c| c.tiles.len() != CHUNK_AREA) {
            return Err(format!("layer '{}' has a chunk of {} tiles, not {CHUNK_AREA}", self.name, c.tiles.len()));
        }
        Ok(())
    }

    #[inline]
    fn locate(&self, x: u32, y: u32) -> (usize, usize) {
        let ci = (y / CHUNK_SIZE * self.chunks_w + x / CHUNK_SIZE) as usize;
        let ti = ((y % CHUNK_SIZE) * CHUNK_SIZE + x % CHUNK_SIZE) as usize;
        (ci, ti)
    }

    /// Caller must ensure `x,y` are inside the map.
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> Tile {
        let (ci, ti) = self.locate(x, y);
        match &self.chunks[ci] {
            Some(c) => c.tiles[ti],
            None => Tile::EMPTY,
        }
    }

    /// Sets a tile and returns the previous value.
    pub fn set(&mut self, x: u32, y: u32, tile: Tile) -> Tile {
        let (ci, ti) = self.locate(x, y);
        let slot = &mut self.chunks[ci];
        if slot.is_none() {
            if tile.is_empty() {
                return Tile::EMPTY;
            }
            *slot = Some(Box::new(Chunk::new()));
        }
        let chunk = slot.as_mut().unwrap();
        std::mem::replace(&mut chunk.tiles[ti], tile)
    }

    /// Every non-empty tile as `(x, y, tile)`, chunk by chunk (skips unallocated chunks).
    pub fn tiles(&self) -> impl Iterator<Item = (u32, u32, Tile)> + '_ {
        self.chunks.iter().enumerate().filter_map(|(ci, c)| c.as_ref().map(|c| (ci as u32, c))).flat_map(move |(ci, c)| {
            let (cx, cy) = (ci % self.chunks_w * CHUNK_SIZE, ci / self.chunks_w * CHUNK_SIZE);
            c.tiles.iter().enumerate().filter(|(_, t)| !t.is_empty()).map(move |(i, t)| (cx + i as u32 % CHUNK_SIZE, cy + i as u32 / CHUNK_SIZE, *t))
        })
    }

    /// Frees chunks that became completely empty (called before saving).
    pub fn compact(&mut self) {
        for slot in &mut self.chunks {
            if slot.as_ref().is_some_and(|c| c.is_empty()) {
                *slot = None;
            }
        }
    }

    pub fn allocated_chunks(&self) -> usize {
        self.chunks.iter().filter(|c| c.is_some()).count()
    }

    /// Whether the chunk containing tile `(x, y)` holds any data.
    pub fn chunk_allocated(&self, x: u32, y: u32) -> bool {
        self.chunks[self.locate(x, y).0].is_some()
    }

    /// The tiles of chunk (`cx`, `cy`), row by row (`CHUNK_SIZE` wide); None while it is empty
    /// or outside the layer.
    pub fn chunk_tiles(&self, cx: u32, cy: u32) -> Option<&[Tile]> {
        if cx >= self.chunks_w || cy >= self.chunks_h {
            return None;
        }
        self.chunks[(cy * self.chunks_w + cx) as usize].as_deref().map(|c| &c.tiles[..])
    }
}

/// A tileset image cut into 32 px squares ([`crate::project::TILE_SIZE`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TilesetDef {
    pub id: String,
    /// Image path relative to the project root.
    pub image: String,
    /// Indices of tiles that block movement.
    #[serde(default)]
    pub solid: Vec<u32>,
    /// Ladder tiles: the player climbs them (only up and down, slower, seen from behind);
    /// characters keep off them. Not solid by themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ladder: Vec<u32>,
    /// Low tiles (fences, rocks, water, a table): when solid they stop walking but not shots or
    /// sight. Solid tiles not listed here stop both (walls, trees).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub low: Vec<u32>,
    /// Animated tiles: wherever `tile` is placed, it cycles through `frames`.
    #[serde(default)]
    pub animations: Vec<TileAnim>,
    /// Tiles that give light wherever they are placed (wall torches, lamps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<TileLight>,
    /// Auto-tiling terrains (grass / dirt, grass / water…): the editor's Terrain tool picks
    /// their edge tiles. Editor-only; games just show the painted tiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terrains: Vec<Terrain>,
    /// Footstep sounds of single tiles (floors that aren't terrains); before the terrains'.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<TileSteps>,
}

/// Tiles that sound like `sound` when walked on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileSteps {
    pub sound: String,
    pub tiles: Vec<u32>,
}

/// The serde defaults (what an entry with only `id` and `image` loads as). The editor's "+ New"
/// value is [`TilesetDef::fresh`].
impl Default for TilesetDef {
    fn default() -> Self {
        TilesetDef { id: String::new(), image: String::new(), solid: vec![], ladder: vec![], low: vec![], animations: vec![], lights: vec![], terrains: vec![], steps: vec![] }
    }
}

impl TilesetDef {
    pub fn new(id: &str, image: &str) -> Self {
        TilesetDef { id: id.into(), image: image.into(), ..Default::default() }
    }

    pub fn light_of(&self, tile: u32) -> Option<&LightSpec> {
        self.lights.iter().find(|l| l.tile == tile).map(|l| &l.light)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileAnim {
    pub tile: u32,
    pub frames: Vec<u32>,
    pub fps: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileLight {
    pub tile: u32,
    pub light: LightSpec,
}

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

}


/// The tile part of an old map: the tilesets its tiles come from and its layers.
#[derive(Clone, Debug, Default)]
pub struct OldTiles {
    pub tilesets: Vec<String>,
    pub layers: Vec<TileLayer>,
}

/// The tilesets of an old project (`defs/tilesets.ron`; none when it has no such file).
pub fn read_tilesets(root: &Path) -> anyhow::Result<Vec<TilesetDef>> {
    use anyhow::Context;
    let path = root.join("defs/tilesets.ron");
    match std::fs::read_to_string(&path) {
        Ok(s) => ron::from_str(&s).with_context(|| format!("reading {}", path.display())),
        Err(_) => Ok(Vec::new()),
    }
}

impl TileAnim {
    /// Tile index shown at time `t` (seconds).
    pub fn frame_at(&self, t: f64) -> u32 {
        if self.frames.is_empty() || self.fps <= 0.0 {
            return self.tile;
        }
        let i = (t * self.fps as f64).floor() as usize % self.frames.len();
        self.frames[i]
    }
}

