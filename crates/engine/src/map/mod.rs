//! Maps: painted ground (8 px cells in materials), decorations, entities and zones on a grid of
//! 32 px tiles (collision, route planning, zones); up to 1024 x 1024 tiles.

pub mod io;
/// The non-tile part of a map (objects and settings), the editor's undo snapshot: see `io::Meta`.
pub use io::Meta as MapObjects;
pub mod transition;

use serde::{Deserialize, Serialize};

use crate::entity::EntityInstance;
use crate::events::Trigger;
pub use transition::{Edge, EdgeLink, EdgeLinks, Respawn, Zone, ZoneKind};

pub const MAX_MAP_SIZE: u32 = 1024;
pub const CHUNK_SIZE: u32 = 32;

/// Per-tile collision override painted in the editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// Material ids used by the painted ground; a ground cell holds its slot here plus one.
    pub materials: Vec<String>,
    /// The painted ground.
    pub ground: crate::ground::Ground,
    /// Prop ids of the decorations; `Decor::def` indexes into this list.
    pub decor_defs: Vec<String>,
    /// Decorations: props placed freely as pictures (trees, rocks, walls, buildings).
    pub decor: Vec<crate::decor::Decor>,
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
    /// The music while the player fights: None = the project's battle music, `""` = none.
    pub battle_music: Option<String>,
    /// Darkness of the map: always bright, day / night, or dark (dungeons).
    pub lighting: MapLighting,
    /// Fog of war: the map screen only shows what the player has explored.
    pub fog: bool,
    /// Rain, snow or fog over the map (a save game can override it: `WorldState::weather`).
    pub weather: Weather,
    /// Image layers moving at their own speed behind or over the map (sky, mist, clouds).
    pub parallax: Vec<ParallaxLayer>,
}

/// What a map is besides its ground and what stands on it: the settings its form shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MapSettings {
    /// The colour beyond the map's edge and where no ground is painted.
    pub background: [u8; 3],
    /// The music that loops on this map (none: whatever is playing goes on).
    #[serde(default)]
    pub music: String,
    /// The music of fights on this map: not set = the game's battle music, empty = none.
    #[serde(default)]
    pub battle_music: Option<String>,
    /// How dark the map is: always bright (indoors), day and night with the clock, or dark
    /// (dungeons: only lights show the way).
    #[serde(default)]
    pub lighting: MapLighting,
    /// Fog of war: the map screen shows only what the player has explored.
    #[serde(default)]
    pub fog: bool,
    /// Rain, snow or fog over the map.
    #[serde(default)]
    pub weather: Weather,
    /// Pictures moving at their own speed behind the map (a sky, an abyss) or over it (cloud
    /// shadows, mist).
    #[serde(default)]
    pub parallax: Vec<ParallaxLayer>,
    /// The maps that begin where this one ends: walking off an edge arrives on the other.
    #[serde(default)]
    pub edges: EdgeLinks,
    /// The map's own events: on entering it, at an hour, on a custom event.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
}

impl MapSettings {
    pub fn of(map: &Map) -> MapSettings {
        MapSettings { background: map.background, music: map.music.clone(), battle_music: map.battle_music.clone(), lighting: map.lighting, fog: map.fog, weather: map.weather, parallax: map.parallax.clone(), edges: map.edges.clone(), triggers: map.triggers.clone() }
    }

    pub fn apply(self, map: &mut Map) {
        map.background = self.background;
        map.music = self.music;
        map.battle_music = self.battle_music;
        map.lighting = self.lighting;
        map.fog = self.fog;
        map.weather = self.weather;
        map.parallax = self.parallax;
        map.edges = self.edges;
        map.triggers = self.triggers;
    }
}

/// An image layer of a map that moves at its own speed: behind the tiles (seen under empty
/// tiles and beyond the map's edge: a sky, an abyss) or over everything (cloud shadows, mist).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
            materials: Vec::new(),
            ground: crate::ground::Ground::new(width, height),
            decor_defs: Vec::new(),
            decor: Vec::new(),
            collision: vec![0; (width * height) as usize],
            entities: Vec::new(),
            zones: Vec::new(),
            edges: EdgeLinks::default(),
            triggers: Vec::new(),
            next_uid: 1,
            background: [20, 20, 28],
            music: String::new(),
            battle_music: None,
            lighting: MapLighting::default(),
            fog: false,
            weather: Weather::default(),
            parallax: Vec::new(),
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    /// The ground cell value of a material id, registering it in the map's list; `None` when
    /// the map already uses [`crate::ground::MAX_MATERIALS`] materials.
    pub fn material_value(&mut self, material: &str) -> Option<u8> {
        if let Some(i) = self.materials.iter().position(|m| m == material) {
            return Some(i as u8 + 1);
        }
        if self.materials.len() >= crate::ground::MAX_MATERIALS {
            return None;
        }
        self.materials.push(material.to_string());
        Some(self.materials.len() as u8)
    }

    /// The material id of a ground cell value (`None` for 0 = no ground).
    pub fn material_of(&self, v: u8) -> Option<&str> {
        (v > 0).then(|| self.materials.get(v as usize - 1).map(String::as_str)).flatten()
    }

    /// The `decor_defs` slot of a prop id, registering it.
    pub fn decor_slot(&mut self, prop: &str) -> u16 {
        if let Some(i) = self.decor_defs.iter().position(|d| d == prop) {
            return i as u16;
        }
        self.decor_defs.push(prop.to_string());
        (self.decor_defs.len() - 1) as u16
    }

    /// Drops prop ids no decoration uses any more from `decor_defs` (renumbering).
    pub fn prune_decor_defs(&mut self) {
        let mut used = vec![false; self.decor_defs.len()];
        for d in &self.decor {
            if let Some(u) = used.get_mut(d.def as usize) {
                *u = true;
            }
        }
        if used.iter().all(|u| *u) {
            return;
        }
        let mut map = vec![0u16; used.len()];
        let mut kept = Vec::new();
        for (i, id) in self.decor_defs.iter().enumerate() {
            if used[i] {
                map[i] = kept.len() as u16;
                kept.push(id.clone());
            }
        }
        self.decor.retain(|d| (d.def as usize) < map.len());
        for d in &mut self.decor {
            d.def = map[d.def as usize];
        }
        self.decor_defs = kept;
    }

    /// Drops materials no ground cell uses any more from the list (renumbering the cells).
    pub fn prune_materials(&mut self) {
        let used = self.ground.used();
        if (1..=self.materials.len() as u8).all(|v| used.contains(&v)) {
            return;
        }
        let mut map = vec![0u8; self.materials.len() + 1];
        let mut kept = Vec::new();
        for (i, id) in self.materials.iter().enumerate() {
            if used.contains(&(i as u8 + 1)) {
                kept.push(id.clone());
                map[i + 1] = kept.len() as u8;
            }
        }
        self.ground.remap(|v| map.get(v as usize).copied().unwrap_or(0));
        self.materials = kept;
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

    /// The tiles painted solid in the collision layer (ground and decorations add theirs when
    /// a map is entered; tiles painted walkable keep out of those).
    pub fn compute_solid(&self) -> Vec<bool> {
        self.collision.iter().map(|&c| c == CollisionOverride::Solid as u8).collect()
    }

    /// Resizes the map, keeping the top-left area. Entities and zones are kept as-is.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.clamp(1, MAX_MAP_SIZE);
        let height = height.clamp(1, MAX_MAP_SIZE);
        let (cw, ch) = (self.width.min(width), self.height.min(height));
        let mut col = vec![0u8; (width * height) as usize];
        for y in 0..ch {
            for x in 0..cw {
                col[(y * width + x) as usize] = self.collision[(y * self.width + x) as usize];
            }
        }
        self.collision = col;
        self.ground = self.ground.resized(width, height);
        let (pw, ph) = ((width * crate::project::TILE_SIZE) as i32, (height * crate::project::TILE_SIZE) as i32);
        self.decor.retain(|d| d.x < pw && d.y <= ph);
        self.width = width;
        self.height = height;
    }

    pub fn compact(&mut self) {
        self.ground.compact();
        self.prune_materials();
        self.prune_decor_defs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_keeps_overlap() {
        let mut m = Map::new("m", 40, 40);
        m.ground.set(5 * 4, 5 * 4, 2);
        m.set_collision(5, 5, CollisionOverride::Solid);
        m.resize(70, 20);
        assert_eq!((m.width, m.height), (70, 20));
        assert_eq!(m.ground.get(5 * 4, 5 * 4), 2);
        assert_eq!(m.collision_at(5, 5), CollisionOverride::Solid);
        assert_eq!(m.ground.get(69 * 4, 19 * 4), 0);
    }

    #[test]
    fn map_size_is_clamped() {
        let m = Map::new("big", 5000, 0);
        assert_eq!((m.width, m.height), (1024, 1));
    }

    #[test]
    fn painted_collision_is_solid() {
        let mut m = Map::new("m", 4, 4);
        m.set_collision(2, 2, CollisionOverride::Walkable);
        m.set_collision(3, 3, CollisionOverride::Solid);
        let solid = m.compute_solid();
        assert!(solid[3 * 4 + 3]);
        assert!(!solid[2 * 4 + 2]);
        assert!(!solid[0]);
    }
}
