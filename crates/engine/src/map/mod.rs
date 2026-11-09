//! Tile maps: chunked, sparse layers so maps up to 1024x1024 tiles stay cheap.

pub mod io;
pub mod transition;

use serde::{Deserialize, Serialize};

use crate::entity::EntityInstance;
use crate::events::Trigger;
pub use transition::{Edge, EdgeLink, EdgeLinks, Respawn, Zone, ZoneKind};

pub const MAX_MAP_SIZE: u32 = 1024;
pub const CHUNK_SIZE: u32 = 32;
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
    chunks: Vec<Option<Box<Chunk>>>,
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
}

/// Per-tile collision override painted in the editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CollisionOverride {
    /// Use the tileset's solidity flags.
    #[default]
    Auto = 0,
    Solid = 1,
    Walkable = 2,
}

#[derive(Clone, Debug)]
pub struct Map {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Tileset ids used by this map; `Tile` slots index into this list.
    pub tilesets: Vec<String>,
    pub layers: Vec<TileLayer>,
    /// One byte per tile (`CollisionOverride`).
    pub collision: Vec<u8>,
    pub entities: Vec<EntityInstance>,
    pub zones: Vec<Zone>,
    pub edges: EdgeLinks,
    /// Map-level triggers (MapEnter, Custom events).
    pub triggers: Vec<Trigger>,
    pub next_uid: u32,
    /// Background color (RGB) shown outside the map / under empty tiles.
    pub background: [u8; 3],
    /// Looping music sound id ("" = keep whatever is playing).
    pub music: String,
    /// Darkness of the map: always bright, day / night, or dark (dungeons).
    pub lighting: MapLighting,
    /// Fog of war: the map screen only shows what the player has explored.
    pub fog: bool,
    /// Rain, snow or fog over the map (a save game can override it: `WorldState::weather`).
    pub weather: Weather,
    /// Image layers moving at their own speed behind or over the map (sky, mist, clouds).
    pub parallax: Vec<ParallaxLayer>,
}

/// An image layer of a map that moves at its own speed: behind the tiles (seen under empty
/// tiles and beyond the map's edge: a sky, an abyss) or over everything (cloud shadows, mist).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParallaxLayer {
    /// Sprite sheet id: its whole image is drawn.
    pub sheet: String,
    /// How much the layer follows the camera per axis: 1 = moves with the map, 0 = fixed to
    /// the screen, between = slower than the map (far away).
    pub factor: [f32; 2],
    /// Position of the image (world pixels; with factor 0: from the screen's top-left).
    pub offset: [f32; 2],
    /// Constant movement, world pixels per second (drifting clouds).
    pub drift: [f32; 2],
    /// Repeats the image along x / y.
    pub repeat: [bool; 2],
    /// Drawn over the map and its characters (before lighting and weather), else behind the
    /// tiles.
    pub above: bool,
    /// Opacity 0..1.
    pub alpha: f32,
    /// Size of the image (1 = one image pixel per world pixel).
    pub scale: f32,
}

impl Default for ParallaxLayer {
    fn default() -> Self {
        ParallaxLayer { sheet: String::new(), factor: [0.5, 0.5], offset: [0.0; 2], drift: [0.0; 2], repeat: [true, true], above: false, alpha: 1.0, scale: 1.0 }
    }
}

impl ParallaxLayer {
    /// World position of the image's top-left corner for a view whose top-left is `view_min`,
    /// `time` seconds in.
    pub fn origin(&self, view_min: [f32; 2], time: f32) -> [f32; 2] {
        let axis = |k: usize| view_min[k] * (1.0 - self.factor[k]) + self.offset[k] + self.drift[k] * time;
        [axis(0), axis(1)]
    }

    /// Where the copies of an image `size` big go along one axis to cover `[from, to)`: first
    /// position and count (one copy when not repeating).
    pub fn copies(origin: f32, size: f32, repeat: bool, from: f32, to: f32) -> (f32, usize) {
        if !repeat || size <= 0.0 {
            return (origin, 1);
        }
        let first = origin + ((from - origin) / size).floor() * size;
        (first, (((to - first) / size).ceil().max(1.0) as usize).min(256))
    }
}

/// How dark a map is when nothing lights it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightMode {
    /// No darkness at all (the default; lights are not drawn).
    #[default]
    Bright,
    /// Follows the time of day: bright by day, `darkness` at night.
    Outdoor,
    /// Always `darkness` (1 = pitch black: only light sources show anything).
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapLighting {
    pub mode: LightMode,
    /// 0..1: how dark it gets without lights (outdoor: at midnight).
    #[serde(default = "default_darkness")]
    pub darkness: f32,
}

fn default_darkness() -> f32 {
    0.7
}

/// What falls from the sky on a map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    #[default]
    None,
    Rain,
    Snow,
    Fog,
}

impl WeatherKind {
    pub const ALL: [WeatherKind; 4] = [WeatherKind::None, WeatherKind::Rain, WeatherKind::Snow, WeatherKind::Fog];
    pub fn label(self) -> &'static str {
        match self {
            WeatherKind::None => "Clear",
            WeatherKind::Rain => "Rain",
            WeatherKind::Snow => "Snow",
            WeatherKind::Fog => "Fog",
        }
    }
    /// Script name: "none", "rain", "snow", "fog".
    pub fn id(self) -> &'static str {
        match self {
            WeatherKind::None => "none",
            WeatherKind::Rain => "rain",
            WeatherKind::Snow => "snow",
            WeatherKind::Fog => "fog",
        }
    }
    pub fn from_id(s: &str) -> Option<WeatherKind> {
        WeatherKind::ALL.into_iter().find(|k| k.id().eq_ignore_ascii_case(s.trim()) || (s.trim().eq_ignore_ascii_case("clear") && *k == WeatherKind::None))
    }
}

/// A map's weather: kind and strength (0..1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub kind: WeatherKind,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
}

fn default_intensity() -> f32 {
    0.6
}

impl Default for Weather {
    fn default() -> Self {
        Weather { kind: WeatherKind::None, intensity: default_intensity() }
    }
}

impl Weather {
    pub fn new(kind: WeatherKind, intensity: f32) -> Self {
        Weather { kind, intensity: intensity.clamp(0.0, 1.0) }
    }
    /// No weather (not written into map files).
    pub fn is_clear(&self) -> bool {
        self.kind == WeatherKind::None
    }
    /// Whether anything shows (a kind other than none with some intensity).
    pub fn active(&self) -> bool {
        self.kind != WeatherKind::None && self.intensity > 0.0
    }
}

impl Default for MapLighting {
    fn default() -> Self {
        MapLighting { mode: LightMode::Bright, darkness: default_darkness() }
    }
}

impl Map {
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        let width = width.clamp(1, MAX_MAP_SIZE);
        let height = height.clamp(1, MAX_MAP_SIZE);
        Map {
            name: name.to_string(),
            width,
            height,
            tilesets: Vec::new(),
            layers: vec![
                TileLayer::new("Ground", width, height, false),
                TileLayer::new("Detail", width, height, false),
                TileLayer::new("Overhead", width, height, true),
            ],
            collision: vec![0; (width * height) as usize],
            entities: Vec::new(),
            zones: Vec::new(),
            edges: EdgeLinks::default(),
            triggers: Vec::new(),
            next_uid: 1,
            background: [20, 20, 28],
            music: String::new(),
            lighting: MapLighting::default(),
            fog: false,
            weather: Weather::default(),
            parallax: Vec::new(),
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    pub fn add_layer(&mut self, name: &str, above_entities: bool) {
        self.layers.push(TileLayer::new(name, self.width, self.height, above_entities));
    }

    /// Returns the slot for a tileset id, registering it if needed.
    pub fn tileset_slot(&mut self, tileset_id: &str) -> u16 {
        if let Some(i) = self.tilesets.iter().position(|t| t == tileset_id) {
            return i as u16;
        }
        self.tilesets.push(tileset_id.to_string());
        (self.tilesets.len() - 1) as u16
    }

    pub fn collision_at(&self, x: u32, y: u32) -> CollisionOverride {
        match self.collision[(y * self.width + x) as usize] {
            1 => CollisionOverride::Solid,
            2 => CollisionOverride::Walkable,
            _ => CollisionOverride::Auto,
        }
    }

    pub fn set_collision(&mut self, x: u32, y: u32, c: CollisionOverride) -> CollisionOverride {
        let i = (y * self.width + x) as usize;
        let old = self.collision_at(x, y);
        self.collision[i] = c as u8;
        old
    }

    /// Allocates a map-unique id for entities and zones.
    pub fn alloc_uid(&mut self) -> u32 {
        let uid = self.next_uid.max(1);
        self.next_uid = uid + 1;
        uid
    }

    pub fn entity(&self, uid: u32) -> Option<&EntityInstance> {
        self.entities.iter().find(|e| e.uid == uid)
    }

    pub fn spawn_point(&self, name: &str) -> Option<&Zone> {
        self.zones
            .iter()
            .find(|z| matches!(z.kind, ZoneKind::Spawn) && z.name == name)
    }

    /// Whether a tile blocks movement, given a tile-solidity lookup
    /// `(tileset_slot, tile_index) -> solid`. Overhead layers never collide.
    pub fn tile_solid(&self, x: u32, y: u32, is_solid: &impl Fn(usize, u32) -> bool) -> bool {
        match self.collision_at(x, y) {
            CollisionOverride::Solid => true,
            CollisionOverride::Walkable => false,
            CollisionOverride::Auto => self.layers.iter().any(|l| {
                !l.above_entities && l.get(x, y).parts().is_some_and(|(s, i)| is_solid(s, i))
            }),
        }
    }

    /// Computes the walkability grid for the whole map.
    pub fn compute_solid(&self, is_solid: impl Fn(usize, u32) -> bool) -> Vec<bool> {
        let mut out = vec![false; (self.width * self.height) as usize];
        for y in 0..self.height {
            for x in 0..self.width {
                out[(y * self.width + x) as usize] = self.tile_solid(x, y, &is_solid);
            }
        }
        out
    }

    /// Resizes the map, keeping the top-left area. Entities and zones are kept as-is.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.clamp(1, MAX_MAP_SIZE);
        let height = height.clamp(1, MAX_MAP_SIZE);
        let (cw, ch) = (self.width.min(width), self.height.min(height));
        for layer in &mut self.layers {
            let mut nl = TileLayer::new(&layer.name, width, height, layer.above_entities);
            nl.visible = layer.visible;
            for y in 0..ch {
                for x in 0..cw {
                    if layer.chunk_allocated(x, y) {
                        nl.set(x, y, layer.get(x, y));
                    }
                }
            }
            *layer = nl;
        }
        let mut col = vec![0u8; (width * height) as usize];
        for y in 0..ch {
            for x in 0..cw {
                col[(y * width + x) as usize] = self.collision[(y * self.width + x) as usize];
            }
        }
        self.collision = col;
        self.width = width;
        self.height = height;
    }

    pub fn compact(&mut self) {
        for l in &mut self.layers {
            l.compact();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_encoding_roundtrip() {
        let t = Tile::new(3, 12345);
        assert_eq!(t.parts(), Some((3, 12345)));
        assert_eq!(Tile::EMPTY.parts(), None);
        assert_eq!(Tile::new(0, 0).parts(), Some((0, 0)));
    }

    #[test]
    fn get_set_across_chunk_boundaries() {
        let mut l = TileLayer::new("t", 100, 70, false);
        let pts = [(0, 0), (31, 31), (32, 31), (31, 32), (99, 69), (64, 0)];
        for (i, &(x, y)) in pts.iter().enumerate() {
            l.set(x, y, Tile::new(0, i as u32 + 1));
        }
        for (i, &(x, y)) in pts.iter().enumerate() {
            assert_eq!(l.get(x, y), Tile::new(0, i as u32 + 1));
        }
        assert_eq!(l.get(33, 33), Tile::EMPTY);
        // Setting empty in an unallocated chunk must not allocate.
        let before = l.allocated_chunks();
        l.set(50, 50, Tile::EMPTY);
        assert_eq!(l.allocated_chunks(), before);
        l.set(99, 69, Tile::EMPTY);
        l.compact();
        assert_eq!(l.allocated_chunks(), before - 1);
    }

    #[test]
    fn resize_keeps_overlap() {
        let mut m = Map::new("m", 40, 40);
        m.layers[0].set(39, 39, Tile::new(0, 1));
        m.layers[0].set(5, 5, Tile::new(0, 2));
        m.set_collision(5, 5, CollisionOverride::Solid);
        m.resize(70, 20);
        assert_eq!((m.width, m.height), (70, 20));
        assert_eq!(m.layers[0].get(5, 5), Tile::new(0, 2));
        assert_eq!(m.collision_at(5, 5), CollisionOverride::Solid);
        assert_eq!(m.layers[0].get(69, 19), Tile::EMPTY);
    }

    #[test]
    fn map_size_is_clamped() {
        let m = Map::new("big", 5000, 0);
        assert_eq!((m.width, m.height), (1024, 1));
    }

    #[test]
    fn solidity_respects_overrides() {
        let mut m = Map::new("m", 4, 4);
        m.layers[0].set(1, 1, Tile::new(0, 7));
        m.layers[0].set(2, 2, Tile::new(0, 7));
        m.set_collision(2, 2, CollisionOverride::Walkable);
        m.set_collision(3, 3, CollisionOverride::Solid);
        let solid = m.compute_solid(|_, i| i == 7);
        assert!(solid[4 + 1]);
        assert!(!solid[2 * 4 + 2]);
        assert!(solid[3 * 4 + 3]);
        assert!(!solid[0]);
    }
}
