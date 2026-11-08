//! Binary map format.
//!
//! Version 2: `MAGIC | version (u32 LE) | lz4(postcard(Core))`, where `Core` holds the
//! tile data in compact binary form and everything else (entities, zones, links,
//! triggers, properties) as a RON string. RON is self-describing, so new optional
//! fields can be added to [`Meta`] without breaking existing maps.
//!
//! Version 1 stored the whole map with postcard; it is still readable.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{EdgeLinks, Map, TileLayer, Zone, ZoneKind};
use crate::entity::EntityInstance;
use crate::events::Trigger;

const MAGIC: &[u8; 6] = b"R2DMAP";
pub const FORMAT_VERSION: u32 = 2;

#[derive(Serialize)]
struct CoreRef<'a> {
    name: &'a str,
    width: u32,
    height: u32,
    tilesets: &'a [String],
    layers: &'a [TileLayer],
    collision: &'a [u8],
    meta: String,
}

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

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Meta {
    entities: Vec<EntityInstance>,
    zones: Vec<Zone>,
    edges: EdgeLinks,
    triggers: Vec<Trigger>,
    next_uid: u32,
    background: [u8; 3],
    music: String,
    lighting: crate::map::MapLighting,
    fog: bool,
    /// Clear weather isn't written (maps without weather stay as they were).
    #[serde(skip_serializing_if = "crate::map::Weather::is_clear")]
    weather: crate::map::Weather,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parallax: Vec<crate::map::ParallaxLayer>,
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
        }
    }
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
    let meta = Meta {
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
    };
    let core = CoreRef {
        name: &map.name,
        width: map.width,
        height: map.height,
        tilesets: &map.tilesets,
        layers: &map.layers,
        collision: &map.collision,
        meta: ron::to_string(&meta).context("serializing map objects")?,
    };
    let raw = postcard::to_stdvec(&core).context("serializing map")?;
    let mut out = Vec::with_capacity(raw.len() / 4 + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&lz4_flex::compress_prepend_size(&raw));
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Map> {
    if bytes.len() < 10 || &bytes[..6] != MAGIC {
        bail!("not an R(ust)PG2D map file");
    }
    let version = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
    let raw = lz4_flex::decompress_size_prepended(&bytes[10..]).context("decompressing map")?;
    let map = match version {
        1 => {
            let m: MapV1 = postcard::from_bytes(&raw).context("deserializing v1 map")?;
            Map {
                name: m.name,
                width: m.width,
                height: m.height,
                tilesets: m.tilesets,
                layers: m.layers,
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
            }
        }
        2 => {
            let c: Core = postcard::from_bytes(&raw).context("deserializing map")?;
            let meta: Meta = ron::from_str(&c.meta).context("parsing map objects")?;
            Map {
                name: c.name,
                width: c.width,
                height: c.height,
                tilesets: c.tilesets,
                layers: c.layers,
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
            }
        }
        v => bail!("unsupported map format version {v} (newest supported: {FORMAT_VERSION})"),
    };
    if map.collision.len() != (map.width * map.height) as usize {
        bail!("corrupt map: collision grid size mismatch");
    }
    Ok(map)
}

pub fn save(map: &Map, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = encode(map)?;
    // Write atomically so a crash never leaves a half-written map.
    let tmp = path.with_extension("map.tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Map> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode(&bytes).with_context(|| format!("loading map {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityKind;
    use crate::map::{CollisionOverride, Tile};

    #[test]
    fn roundtrip_max_size_map() {
        let mut m = Map::new("huge", 1024, 1024);
        let slot = m.tileset_slot("terrain");
        for y in 0..1024 {
            for x in 0..1024 {
                m.layers[0].set(x, y, Tile::new(slot, (x * 7 + y * 3) % 64));
            }
        }
        m.layers[1].set(1023, 1023, Tile::new(slot, 5));
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
        assert_eq!(back.layers[0].get(1000, 999), m.layers[0].get(1000, 999));
        assert_eq!(back.layers[1].get(1023, 1023), Tile::new(slot, 5));
        assert_eq!(back.layers[1].allocated_chunks(), 1);
        assert_eq!(back.collision_at(512, 512), CollisionOverride::Solid);
        assert_eq!(back.tilesets, vec!["terrain".to_string()]);
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
        let mut m = Map::new("old", 10, 10);
        m.layers[0].set(3, 3, Tile::new(0, 9));
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
            tilesets: &m.tilesets,
            layers: &m.layers,
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
        let back = decode(&bytes).unwrap();
        assert_eq!(back.layers[0].get(3, 3), Tile::new(0, 9));
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
