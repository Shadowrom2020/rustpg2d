//! Binary map format.
//!
//! Version 2: `MAGIC | version (u32 LE) | lz4(postcard(Core))`, where `Core` holds the
//! tile data in compact binary form and everything else (entities, zones, links,
//! triggers, properties) as a RON string. RON is self-describing, so new optional
//! fields can be added to [`Meta`] without breaking existing maps.
//!
//! Version 3 (R(ust)PG2D 0.21, painted maps) replaced the tile layers with the painted ground
//! (`materials`, `ground`) and decorations (`decor_defs`, `decor`).
//!
//! Version 1 stored the whole map with postcard. Versions 1 and 2 (tile maps) are still read:
//! [`decode_tiles`] hands their tiles to the converter (`convert_tiles`); everything else sees
//! the map without them.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{EdgeLinks, Map, Zone, ZoneKind, MAX_MAP_SIZE};
use crate::old_tiles::{OldTiles, TileLayer};
use crate::entity::EntityInstance;
use crate::events::Trigger;

const MAGIC: &[u8; 6] = b"R2DMAP";
pub const FORMAT_VERSION: u32 = 3;

#[derive(Serialize)]
struct CoreRef<'a> {
    name: &'a str,
    width: u32,
    height: u32,
    collision: &'a [u8],
    meta: String,
    materials: &'a [String],
    ground: &'a crate::ground::Ground,
    decor_defs: &'a [String],
    decor: &'a [crate::decor::Decor],
}

/// Version 2 (frozen).
#[derive(Deserialize)]
struct Core {
    name: String,
    width: u32,
    height: u32,
    tilesets: Vec<String>,
    layers: Vec<TileLayer>,
    collision: Vec<u8>,
    meta: String,
}

/// Version 3: the painted ground and decorations instead of tile layers (postcard: fields by
/// position).
#[derive(Deserialize)]
struct Core3 {
    name: String,
    width: u32,
    height: u32,
    collision: Vec<u8>,
    meta: String,
    materials: Vec<String>,
    ground: crate::ground::Ground,
    decor_defs: Vec<String>,
    decor: Vec<crate::decor::Decor>,
}

/// Everything on a map that isn't tile data: the RON part of a map file, and (as
/// `map::MapObjects`) the editor's undo snapshot of it. A new map property is added here, in
/// [`Meta::of`] and [`Meta::apply`], and nowhere else.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub entities: Vec<EntityInstance>,
    pub zones: Vec<Zone>,
    pub edges: EdgeLinks,
    pub triggers: Vec<Trigger>,
    pub next_uid: u32,
    pub background: [u8; 3],
    pub music: String,
    pub lighting: crate::map::MapLighting,
    pub fog: bool,
    /// Clear weather isn't written (maps without weather stay as they were).
    #[serde(skip_serializing_if = "crate::map::Weather::is_clear")]
    pub weather: crate::map::Weather,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parallax: Vec<crate::map::ParallaxLayer>,
    /// The map's battle music: None = the project's, `""` = none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battle_music: Option<String>,
}

impl Meta {
    /// A copy of the map's objects and settings.
    pub fn of(map: &Map) -> Meta {
        Meta {
            entities: map.entities.clone(),
            zones: map.zones.clone(),
            edges: map.edges.clone(),
            triggers: map.triggers.clone(),
            next_uid: map.next_uid,
            background: map.background,
            music: map.music.clone(),
            lighting: map.lighting,
            fog: map.fog,
            weather: map.weather,
            parallax: map.parallax.clone(),
            battle_music: map.battle_music.clone(),
        }
    }

    /// Puts these objects and settings onto `map` (its tiles stay).
    pub fn apply(&self, map: &mut Map) {
        map.entities = self.entities.clone();
        map.zones = self.zones.clone();
        map.edges = self.edges.clone();
        map.triggers = self.triggers.clone();
        map.next_uid = self.next_uid;
        map.background = self.background;
        map.music = self.music.clone();
        map.lighting = self.lighting;
        map.fog = self.fog;
        map.weather = self.weather;
        map.parallax = self.parallax.clone();
        map.battle_music = self.battle_music.clone();
    }
}

impl Default for Meta {
    fn default() -> Self {
        Meta {
            entities: vec![],
            zones: vec![],
            edges: EdgeLinks::default(),
            triggers: vec![],
            next_uid: 1,
            background: [20, 20, 28],
            music: String::new(),
            lighting: Default::default(),
            fog: false,
            weather: Default::default(),
            parallax: Vec::new(),
            battle_music: None,
        }
    }
}

/// `Meta` with its objects kept as text: read when `Meta` itself fails, so one entity, zone or
/// trigger that can't be read (a hand edit, a newer version's variant) costs only itself.
#[derive(Deserialize)]
#[serde(default)]
struct MetaLenient {
    entities: Vec<Box<ron::value::RawValue>>,
    zones: Vec<Box<ron::value::RawValue>>,
    edges: EdgeLinks,
    triggers: Vec<Box<ron::value::RawValue>>,
    next_uid: u32,
    background: [u8; 3],
    music: String,
    lighting: crate::map::MapLighting,
    fog: bool,
    weather: crate::map::Weather,
    parallax: Vec<crate::map::ParallaxLayer>,
    battle_music: Option<String>,
}

impl Default for MetaLenient {
    fn default() -> Self {
        let d = Meta::default();
        MetaLenient { entities: vec![], zones: vec![], edges: d.edges, triggers: vec![], next_uid: d.next_uid, background: d.background, music: d.music, lighting: d.lighting, fog: d.fog, weather: d.weather, parallax: d.parallax, battle_music: None }
    }
}

/// The map's objects: all of them, or (when one can't be read) all the others, each left-out
/// one reported through `warn`.
fn parse_meta(text: &str, warn: &mut dyn FnMut(String)) -> Result<Meta> {
    let whole = match ron::from_str::<Meta>(text) {
        Ok(m) => return Ok(m),
        Err(e) => e,
    };
    let l: MetaLenient = ron::from_str(text).with_context(|| format!("parsing map objects: {whole}"))?;
    let mut m = Meta { entities: vec![], zones: vec![], edges: l.edges, triggers: vec![], next_uid: l.next_uid, background: l.background, music: l.music, lighting: l.lighting, fog: l.fog, weather: l.weather, parallax: l.parallax, battle_music: l.battle_music };
    use crate::project::field_hint;
    for (i, raw) in l.entities.iter().enumerate() {
        match raw.into_rust::<EntityInstance>() {
            Ok(e) => m.entities.push(e),
            Err(e) => warn(format!("entity {} ({}, {}) can't be read and is left out: {e}", i + 1, field_hint(raw.get_ron(), "uid"), field_hint(raw.get_ron(), "def"))),
        }
    }
    for (i, raw) in l.zones.iter().enumerate() {
        match raw.into_rust::<Zone>() {
            Ok(z) => m.zones.push(z),
            Err(e) => warn(format!("zone {} ({}) can't be read and is left out: {e}", i + 1, field_hint(raw.get_ron(), "name"))),
        }
    }
    for (i, raw) in l.triggers.iter().enumerate() {
        match raw.into_rust::<Trigger>() {
            Ok(t) => m.triggers.push(t),
            Err(e) => warn(format!("map trigger {} can't be read and is left out: {e}", i + 1)),
        }
    }
    Ok(m)
}

/// Entity layout of format version 1 (frozen: postcard isn't self-describing).
#[derive(Serialize, Deserialize)]
struct EntityV1 {
    uid: u32,
    kind: crate::entity::EntityKind,
    def: String,
    name: String,
    x: f32,
    y: f32,
    facing: crate::entity::Facing,
    active: bool,
    frame: u32,
    count: u32,
    triggers: Vec<Trigger>,
}

impl From<EntityV1> for EntityInstance {
    fn from(e: EntityV1) -> Self {
        let mut n = EntityInstance::new(e.uid, e.kind, &e.def, e.x, e.y);
        n.name = e.name;
        n.facing = e.facing;
        n.active = e.active;
        n.frame = e.frame;
        n.count = e.count;
        n.triggers = e.triggers;
        n
    }
}

/// Zone layout of format version 1 (frozen like [`EntityV1`]).
#[derive(Serialize, Deserialize)]
struct ZoneV1 {
    uid: u32,
    name: String,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    kind: ZoneKind,
    triggers: Vec<Trigger>,
}

impl From<ZoneV1> for Zone {
    fn from(z: ZoneV1) -> Self {
        Zone { uid: z.uid, name: z.name, x: z.x, y: z.y, w: z.w, h: z.h, kind: z.kind, triggers: z.triggers, loot: None }
    }
}

/// Layout of format version 1 (whole map via postcard).
#[derive(Deserialize)]
struct MapV1 {
    name: String,
    width: u32,
    height: u32,
    tilesets: Vec<String>,
    layers: Vec<TileLayer>,
    collision: Vec<u8>,
    entities: Vec<EntityV1>,
    zones: Vec<ZoneV1>,
    edges: EdgeLinks,
    triggers: Vec<Trigger>,
    next_uid: u32,
    background: [u8; 3],
}

pub fn encode(map: &Map) -> Result<Vec<u8>> {
    let mut map = map.clone();
    map.compact();
    let meta = Meta::of(&map);
    let core = CoreRef {
        name: &map.name,
        width: map.width,
        height: map.height,
        collision: &map.collision,
        meta: ron::to_string(&meta).context("serializing map objects")?,
        materials: &map.materials,
        ground: &map.ground,
        decor_defs: &map.decor_defs,
        decor: &map.decor,
    };
    let raw = postcard::to_stdvec(&core).context("serializing map")?;
    let mut out = Vec::with_capacity(raw.len() / 4 + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress_prepend_size(&raw));
    Ok(out)
}

/// The most tile data a map file may claim to hold (a 1024 x 1024 map with ten full layers is
/// about 40 MiB): a corrupt size prefix can't make the game allocate gigabytes.
const MAX_RAW: u32 = 256 << 20;

/// Reads a map file's bytes. Objects that can't be read are left out and logged (`[map]`);
/// [`decode_with`] hands them to the caller.
pub fn decode(bytes: &[u8]) -> Result<Map> {
    decode_with(bytes, &mut |w| eprintln!("[map] {w}"))
}

/// [`decode`], reporting every object that was left out through `warn`. Tiles of a map from
/// before painted maps are left out too (the editor converts such projects when it opens them).
pub fn decode_with(bytes: &[u8], warn: &mut dyn FnMut(String)) -> Result<Map> {
    let (map, tiles) = decode_tiles(bytes, warn)?;
    if !tiles.layers.is_empty() {
        warn(format!("map {} still has tiles (made before R(ust)PG2D 0.21): open the project in the editor to convert it", map.name));
    }
    Ok(map)
}

/// [`decode_with`], also returning the tiles of a version 1 or 2 map (none for newer maps).
pub fn decode_tiles(bytes: &[u8], warn: &mut dyn FnMut(String)) -> Result<(Map, OldTiles)> {
    if bytes.len() < 14 || &bytes[..6] != MAGIC {
        bail!("not an R(ust)PG2D map file");
    }
    let version = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
    let claimed = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
    if claimed > MAX_RAW {
        bail!("corrupt map: {claimed} bytes of tile data claimed");
    }
    let raw = lz4_flex::decompress_size_prepended(&bytes[10..]).context("decompressing map")?;
    let mut tiles = OldTiles::default();
    let map = match version {
        1 => {
            let m: MapV1 = postcard::from_bytes(&raw).context("deserializing v1 map")?;
            tiles = OldTiles { tilesets: m.tilesets, layers: m.layers };
            Map {
                name: m.name,
                width: m.width,
                height: m.height,
                materials: Vec::new(),
                ground: crate::ground::Ground::new(m.width, m.height),
                decor_defs: Vec::new(),
                decor: Vec::new(),
                collision: m.collision,
                entities: m.entities.into_iter().map(EntityInstance::from).collect(),
                zones: m.zones.into_iter().map(Zone::from).collect(),
                edges: m.edges,
                triggers: m.triggers,
                next_uid: m.next_uid,
                background: m.background,
                music: String::new(),
                lighting: Default::default(),
                fog: false,
                weather: Default::default(),
                parallax: Vec::new(),
                battle_music: None,
            }
        }
        2 | 3 => {
            let c = if version == 2 {
                let c: Core = postcard::from_bytes(&raw).context("deserializing map")?;
                let ground = crate::ground::Ground::new(c.width, c.height);
                tiles = OldTiles { tilesets: c.tilesets, layers: c.layers };
                Core3 { name: c.name, width: c.width, height: c.height, collision: c.collision, meta: c.meta, materials: Vec::new(), ground, decor_defs: Vec::new(), decor: Vec::new() }
            } else {
                postcard::from_bytes(&raw).context("deserializing map")?
            };
            let meta = parse_meta(&c.meta, warn)?;
            Map {
                name: c.name,
                width: c.width,
                height: c.height,
                materials: c.materials,
                ground: c.ground,
                decor_defs: c.decor_defs,
                decor: c.decor,
                collision: c.collision,
                entities: meta.entities,
                zones: meta.zones,
                edges: meta.edges,
                triggers: meta.triggers,
                next_uid: meta.next_uid,
                background: meta.background,
                music: meta.music,
                lighting: meta.lighting,
                fog: meta.fog,
                weather: meta.weather,
                parallax: meta.parallax,
                battle_music: meta.battle_music,
            }
        }
        v => bail!("unsupported map format version {v} (newest supported: {FORMAT_VERSION})"),
    };
    if map.width == 0 || map.height == 0 || map.width > MAX_MAP_SIZE || map.height > MAX_MAP_SIZE {
        bail!("corrupt map: {} x {} tiles", map.width, map.height);
    }
    if map.collision.len() != map.width as usize * map.height as usize {
        bail!("corrupt map: collision grid size mismatch");
    }
    // Layers index their chunk grid without bounds checks: refuse one that doesn't fit.
    for layer in &tiles.layers {
        if let Err(e) = layer.validate(map.width, map.height) {
            bail!("corrupt map: {e}");
        }
    }
    if let Err(e) = map.ground.validate(map.width, map.height) {
        bail!("corrupt map: {e}");
    }
    if map.materials.len() > crate::ground::MAX_MATERIALS {
        bail!("corrupt map: {} ground materials", map.materials.len());
    }
    if map.decor.iter().any(|d| d.def as usize >= map.decor_defs.len()) {
        bail!("corrupt map: a decoration without its prop");
    }
    Ok((map, tiles))
}

pub fn save(map: &Map, path: &Path) -> Result<()> {
    let bytes = encode(map)?;
    // (`sys::write` is atomic: a crash never leaves a half-written map.)
    crate::sys::write(path, &bytes).with_context(|| format!("writing {}", path.display()))
}

/// [`load`] with the objects that were left out reported through `warn` (the editor shows them
/// and backs the file up before it saves without them).
pub fn load_with(path: &Path, warn: &mut dyn FnMut(String)) -> Result<Map> {
    let bytes = crate::sys::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode_with(&bytes, warn).with_context(|| format!("loading map {}", path.display()))
}

/// Writes a tile map in format version 2: only the 16 px upgrade (`migrate`) does, for the tile
/// converter that runs after it.
pub fn save_tiles(map: &Map, tiles: &OldTiles, path: &Path) -> Result<()> {
    #[derive(Serialize)]
    struct Core2<'a> {
        name: &'a str,
        width: u32,
        height: u32,
        tilesets: &'a [String],
        layers: &'a [TileLayer],
        collision: &'a [u8],
        meta: String,
    }
    let core = Core2 { name: &map.name, width: map.width, height: map.height, tilesets: &tiles.tilesets, layers: &tiles.layers, collision: &map.collision, meta: ron::to_string(&Meta::of(map))? };
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress_prepend_size(&postcard::to_stdvec(&core)?));
    crate::sys::write(path, &out).with_context(|| format!("writing {}", path.display()))
}

/// Loads a map with the tiles of an older version ([`decode_tiles`]).
pub fn load_tiles(path: &Path) -> Result<(Map, OldTiles)> {
    let bytes = crate::sys::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode_tiles(&bytes, &mut |w| eprintln!("[map] {w}")).with_context(|| format!("loading map {}", path.display()))
}

pub fn load(path: &Path) -> Result<Map> {
    let bytes = crate::sys::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode(&bytes).with_context(|| format!("loading map {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityKind;
    use crate::map::CollisionOverride;
    use crate::old_tiles::{Chunk, Tile};

    /// A tile map as version 2 wrote it.
    fn v2_bytes(m: &Map, tiles: &OldTiles) -> Vec<u8> {
        #[derive(Serialize)]
        struct Core2<'a> {
            name: &'a str,
            width: u32,
            height: u32,
            tilesets: &'a [String],
            layers: &'a [TileLayer],
            collision: &'a [u8],
            meta: String,
        }
        let core = Core2 { name: &m.name, width: m.width, height: m.height, tilesets: &tiles.tilesets, layers: &tiles.layers, collision: &m.collision, meta: ron::to_string(&Meta::of(m)).unwrap() };
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&lz4_flex::compress_prepend_size(&postcard::to_stdvec(&core).unwrap()));
        out
    }

    /// Old maps' three layers with one tile.
    fn old_tiles(w: u32, h: u32) -> OldTiles {
        let mut layers = vec![TileLayer::new("Ground", w, h, false), TileLayer::new("Detail", w, h, false), TileLayer::new("Overhead", w, h, true)];
        layers[0].set(3, 3, Tile::new(0, 9));
        OldTiles { tilesets: vec!["terrain".into()], layers }
    }

    #[test]
    fn battle_music_is_optional_in_maps() {
        // Older maps have none: the project's.
        let old: Meta = ron::from_str("(music: \"theme\")").unwrap();
        assert_eq!(old.battle_music, None);
        assert!(!ron::to_string(&old).unwrap().contains("battle_music"));
        // None at all, or a track of its own.
        for b in [Some(String::new()), Some("drums".to_string())] {
            let m = Meta { battle_music: b.clone(), ..Meta::default() };
            assert_eq!(ron::from_str::<Meta>(&ron::to_string(&m).unwrap()).unwrap().battle_music, b);
        }
    }

    /// One map object that can't be read (a newer version's variant, a hand edit) is left out
    /// with a warning; the rest of the map loads.
    #[test]
    fn unreadable_objects_are_left_out() {
        let mut m = Map::new("m", 8, 8);
        let uid = m.alloc_uid();
        m.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 1.0, 2.0));
        let uid = m.alloc_uid();
        m.entities.push(EntityInstance::new(uid, EntityKind::Character, "guard", 3.0, 4.0));
        let core: Core3 = postcard::from_bytes(&lz4_flex::decompress_size_prepended(&encode(&m).unwrap()[10..]).unwrap()).unwrap();
        let broken = core.meta.replace("kind:Character", "kind:Dragon");
        assert_ne!(broken, core.meta);
        let mut warnings = Vec::new();
        let meta = parse_meta(&broken, &mut |w| warnings.push(w)).unwrap();
        assert_eq!(meta.entities.iter().map(|e| e.def.as_str()).collect::<Vec<_>>(), ["chest"]);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("entity 2 (uid 2, def \"guard\") can't be read"), "{}", warnings[0]);
        assert_eq!(meta.next_uid, m.next_uid);
        // Healthy objects parse in one go.
        parse_meta(&core.meta, &mut |_| panic!("healthy")).unwrap();
    }

    /// Version 3 keeps the painted ground; version 2 maps load with none.
    #[test]
    fn painted_ground_round_trips() {
        let mut m = Map::new("g", 50, 40);
        let grass = m.material_value("grass").unwrap();
        let water = m.material_value("water").unwrap();
        m.ground.fill(grass);
        m.ground.paint_circle((400.0, 300.0), 60.0, water);
        let back = decode(&encode(&m).unwrap()).unwrap();
        assert_eq!(back.materials, ["grass", "water"]);
        assert_eq!(back.ground.get(50, 37), water);
        assert_eq!(back.ground.get(0, 0), grass);
        // A material nothing is painted with any more leaves the list when saving.
        m.ground.fill(water);
        let back = decode(&encode(&m).unwrap()).unwrap();
        assert_eq!(back.materials, ["water"]);
        assert_eq!(back.ground.get(0, 0), 1);
        // A version 2 map (tiles, no ground) loads without ground; its tiles come separately.
        let (old, tiles) = decode_tiles(&v2_bytes(&m, &old_tiles(50, 40)), &mut |_| {}).unwrap();
        assert!(old.ground.is_empty() && old.materials.is_empty());
        assert_eq!(tiles.layers[0].get(3, 3), Tile::new(0, 9));
        assert_eq!(tiles.tilesets, ["terrain"]);
        let mut warned = Vec::new();
        decode_with(&v2_bytes(&m, &old_tiles(50, 40)), &mut |w| warned.push(w)).unwrap();
        assert!(warned[0].contains("still has tiles"), "{warned:?}");
    }

    /// Decorations round trip; props no decoration uses leave the list when saving.
    #[test]
    fn decorations_round_trip() {
        use crate::decor::{Decor, DecorLayer};
        let mut m = Map::new("d", 30, 20);
        let oak = m.decor_slot("oak");
        let gone = m.decor_slot("gone");
        let rock = m.decor_slot("rock");
        assert_eq!((oak, gone, rock), (0, 1, 2));
        m.decor.push(Decor { def: oak, x: 100, y: 200, flip: true, state: 0, layer: DecorLayer::Sorted });
        m.decor.push(Decor { def: rock, x: -5, y: 640, flip: false, state: 1, layer: DecorLayer::Above });
        let back = decode(&encode(&m).unwrap()).unwrap();
        assert_eq!(back.decor_defs, ["oak", "rock"]);
        assert_eq!(back.decor.len(), 2);
        assert_eq!((back.decor_defs[back.decor[1].def as usize].as_str(), back.decor[1].x, back.decor[1].layer, back.decor[1].state), ("rock", -5, DecorLayer::Above, 1));
        assert!(back.decor[0].flip);
        // A decoration whose prop slot doesn't exist: a corrupt file.
        let mut bad = m.clone();
        bad.decor.push(Decor { def: 9, x: 0, y: 0, flip: false, state: 0, layer: DecorLayer::Flat });
        let core = CoreRef { name: "d", width: 30, height: 20, collision: &bad.collision, meta: ron::to_string(&Meta::of(&bad)).unwrap(), materials: &[], ground: &bad.ground, decor_defs: &bad.decor_defs, decor: &bad.decor };
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&lz4_flex::compress_prepend_size(&postcard::to_stdvec(&core).unwrap()));
        assert!(decode(&bytes).is_err());
    }

    /// Corrupt files are refused with an error, never a panic (the game loads untrusted data).
    #[test]
    fn corrupt_maps_are_refused() {
        let mut m = Map::new("m", 40, 40);
        let v = m.material_value("grass").unwrap();
        m.ground.paint_circle((100.0, 100.0), 50.0, v);
        let good = encode(&m).unwrap();
        assert!(decode(&good).is_ok());
        // Truncated anywhere: an error.
        for cut in [0, 5, 9, 12, 14, 20, good.len() / 2, good.len() - 1] {
            assert!(decode(&good[..cut]).is_err(), "cut at {cut}");
        }
        // A size prefix claiming gigabytes.
        let mut huge = good.clone();
        huge[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&huge).unwrap_err().to_string().contains("claimed"));
        // An old map's layer whose chunk grid doesn't match the map.
        let mut tiles = old_tiles(40, 40);
        tiles.layers[0].chunks.pop();
        assert!(decode(&v2_bytes(&m, &tiles)).unwrap_err().to_string().contains("chunks"));
        let mut tiles = old_tiles(40, 40);
        tiles.layers[1].chunks[0] = Some(Box::new(Chunk { tiles: vec![Tile::new(0, 1); 3] }));
        assert!(decode(&v2_bytes(&m, &tiles)).unwrap_err().to_string().contains("tiles"));
        // A size the format can't hold.
        let mut bad = m.clone();
        bad.width = 0;
        assert!(decode(&encode(&bad).unwrap()).is_err());
    }

    #[test]
    fn roundtrip_max_size_map() {
        let mut m = Map::new("huge", 1024, 1024);
        let grass = m.material_value("grass").unwrap();
        let sand = m.material_value("sand").unwrap();
        m.ground.fill(grass);
        for k in 0..200 {
            let p = (k as f32 * 157.0 % 32768.0, k as f32 * 311.0 % 32768.0);
            m.ground.paint_circle(p, 40.0, sand);
        }
        let oak = m.decor_slot("oak");
        for k in 0..50_000 {
            m.decor.push(crate::decor::Decor { def: oak, x: k * 13 % 32768, y: k * 29 % 32768, flip: k % 2 == 0, state: 0, layer: crate::decor::DecorLayer::Sorted });
        }
        m.set_collision(512, 512, CollisionOverride::Solid);
        m.music = "theme".into();
        m.lighting = crate::map::MapLighting { mode: crate::map::LightMode::Dark, darkness: 0.9 };
        m.fog = true;
        m.weather = crate::map::Weather::new(crate::map::WeatherKind::Snow, 0.3);
        let uid = m.alloc_uid();
        let mut chest = EntityInstance::new(uid, EntityKind::Prop, "chest", 1.5, 2.0);
        chest.container = Some(crate::defs::ContainerSpec {
            size: 6,
            items: vec![crate::inventory::ItemStack::new("key", 1)],
            locked_text: "locked".into(),
            ..Default::default()
        });
        m.entities.push(chest);
        // A placed character's own portrait lives in the map's RON part.
        let uid = m.alloc_uid();
        let mut tilda = EntityInstance::new(uid, EntityKind::Character, "merchant", 3.0, 4.0);
        tilda.portrait = Some(crate::defs::SpriteRef::new("portraits", 2));
        m.entities.push(tilda);
        // A map without parallax layers writes nothing for them (older versions read it).
        assert!(!ron::to_string(&Meta::default()).unwrap().contains("parallax"));
        m.parallax.push(crate::map::ParallaxLayer { sheet: "clouds".into(), factor: [1.0, 1.0], drift: [6.0, 2.0], above: true, alpha: 0.15, scale: 4.0, ..Default::default() });
        let bytes = encode(&m).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(back.parallax, m.parallax);
        assert_eq!((back.width, back.height), (1024, 1024));
        for (x, y) in [(0, 0), (1000, 999), (4095, 4095), (157 * 3 / 8, 311 * 3 / 8)] {
            assert_eq!(back.ground.get(x, y), m.ground.get(x, y));
        }
        assert_eq!(back.decor, m.decor);
        assert_eq!(back.collision_at(512, 512), CollisionOverride::Solid);
        assert_eq!(back.music, "theme");
        assert_eq!(back.lighting, m.lighting);
        assert!(back.fog);
        assert_eq!(back.weather, m.weather);
        assert_eq!(back.entities, m.entities);
        assert_eq!(back.next_uid, m.next_uid);
    }

    #[test]
    fn reads_version_1() {
        #[derive(Serialize)]
        struct V1<'a> {
            name: &'a str,
            width: u32,
            height: u32,
            tilesets: &'a [String],
            layers: &'a [TileLayer],
            collision: &'a [u8],
            entities: &'a [EntityV1],
            zones: &'a [ZoneV1],
            edges: &'a EdgeLinks,
            triggers: &'a [Trigger],
            next_uid: u32,
            background: [u8; 3],
        }
        let m = Map::new("old", 10, 10);
        let tiles = old_tiles(10, 10);
        let old_entities = vec![EntityV1 {
            uid: 4,
            kind: EntityKind::Item,
            def: "key".into(),
            name: String::new(),
            x: 8.0,
            y: 15.0,
            facing: crate::entity::Facing::Down,
            active: true,
            frame: 0,
            count: 2,
            triggers: vec![],
        }];
        let v1 = V1 {
            name: &m.name,
            width: 10,
            height: 10,
            tilesets: &[],
            layers: &tiles.layers,
            collision: &m.collision,
            entities: &old_entities,
            zones: &m
                .zones
                .iter()
                .map(|z| ZoneV1 { uid: z.uid, name: z.name.clone(), x: z.x, y: z.y, w: z.w, h: z.h, kind: z.kind.clone(), triggers: z.triggers.clone() })
                .collect::<Vec<_>>(),
            edges: &m.edges,
            triggers: &m.triggers,
            next_uid: 5,
            background: [1, 2, 3],
        };
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&lz4_flex::compress_prepend_size(&postcard::to_stdvec(&v1).unwrap()));
        // The committed copy freezes the postcard layout of TileLayer / Chunk (v2 stores them the
        // same way): `RUSTPG2D_WRITE_FIXTURES=1 cargo test -p rustpg2d-engine reads_version_1`
        // rewrites it after a deliberate format change.
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maps/v1-synthetic.map");
        if std::env::var("RUSTPG2D_WRITE_FIXTURES").is_ok() {
            std::fs::write(&fixture, &bytes).unwrap();
        }
        assert_eq!(std::fs::read(&fixture).unwrap(), bytes, "tests/fixtures/maps/v1-synthetic.map differs: the map file layout changed");
        let (back, old) = decode_tiles(&bytes, &mut |_| {}).unwrap();
        assert_eq!(old.layers[0].get(3, 3), Tile::new(0, 9));
        assert_eq!((back.entities[0].def.as_str(), back.entities[0].count), ("key", 2));
        assert!(back.entities[0].container.is_none());
        assert_eq!((back.next_uid, back.background), (5, [1, 2, 3]));
        assert!(back.music.is_empty());
    }

    #[test]
    fn parallax_layers_place_their_copies() {
        let l = crate::map::ParallaxLayer { factor: [0.25, 0.0], offset: [10.0, 0.0], drift: [2.0, 0.0], ..Default::default() };
        // Factor 0.25: the view moved 400 px, the layer 300 px less (it lags behind), drift 2 px/s.
        assert_eq!(l.origin([400.0, 50.0], 5.0), [400.0 * 0.75 + 10.0 + 10.0, 50.0]);
        // Repeating: from the copy left of the view to past its right edge.
        assert_eq!(crate::map::ParallaxLayer::copies(30.0, 100.0, true, 250.0, 520.0), (230.0, 3));
        assert_eq!(crate::map::ParallaxLayer::copies(30.0, 100.0, false, 250.0, 520.0), (30.0, 1));
        // A layer of an old map (only some fields) gets the defaults for the rest.
        let old: crate::map::ParallaxLayer = ron::from_str(r#"(sheet: "sky")"#).unwrap();
        assert_eq!((old.alpha, old.scale, old.repeat, old.above), (1.0, 1.0, [true, true], false));
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"hello world").is_err());
    }
}
