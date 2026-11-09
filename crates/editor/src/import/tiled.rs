//! Tiled maps: `.tmx` (XML) and `.tmj` (JSON), with embedded or external tilesets (`.tsx` /
//! `.tsj`). Orthogonal, fixed-size maps; tile data as CSV, base64, base64 + zlib / gzip.
//! Tilesets are repacked into plain 32 px grids (margins and spacing removed, other tile sizes
//! scaled), tile layers become map layers, a layer named `collision` becomes collision,
//! rectangles of object layers become trigger zones and points named `spawn:NAME` spawn points.
//! Wang sets become auto-tiling terrains: *mixed* sets (sides and corners) 47-tile terrains,
//! *edge* sets 16-tile terrains; *corner* sets don't fit the side-based terrains and are reported.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rustpg2d_engine::autotile::Terrain;
use rustpg2d_engine::defs::{Database, TileAnim, TilesetDef};
use rustpg2d_engine::imageio::Rgba;
use rustpg2d_engine::map::{CollisionOverride, Map, TileLayer, Tile, Zone, ZoneKind};

use super::{sanitize, unique_id, write_new, TS};

/// A tileset as Tiled describes it (tile coordinates in its own pixel size).
#[derive(Clone, Debug, Default)]
pub struct Tileset {
    pub firstgid: u32,
    pub name: String,
    /// Absolute path of the image (`None`: a collection of single images, not supported).
    pub image: Option<PathBuf>,
    pub tile_w: u32,
    pub tile_h: u32,
    pub spacing: u32,
    pub margin: u32,
    pub columns: u32,
    pub tilecount: u32,
    /// Tiles with a `solid` property (or class / type "solid").
    pub solid: BTreeSet<u32>,
    /// Animated tiles: tile id → (frame tile id, duration ms).
    pub anims: BTreeMap<u32, Vec<(u32, u32)>>,
    pub wangsets: Vec<WangSet>,
}

/// A Tiled Wang set: its kind (`corner`, `edge`, `mixed`), colour names (colour 1 = index 0)
/// and tiles with their 8 Wang colours (top, top-right, right, …, top-left; 0 = none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WangSet {
    pub name: String,
    pub kind: String,
    pub colors: Vec<String>,
    pub tiles: Vec<(u32, [u32; 8])>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Object {
    pub name: String,
    pub class: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub point: bool,
    /// A tile object (`gid`): not imported.
    pub tile: bool,
}

#[derive(Clone, Debug)]
pub enum LayerData {
    /// Global tile ids, row by row (flip flags still set).
    Tiles(Vec<u32>),
    Objects(Vec<Object>),
    /// Image layers are not imported.
    Image,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    /// A boolean property `above`.
    pub above_prop: bool,
    pub data: LayerData,
}

#[derive(Clone, Debug, Default)]
pub struct Parsed {
    pub width: u32,
    pub height: u32,
    pub tile_w: u32,
    pub tile_h: u32,
    pub tilesets: Vec<Tileset>,
    pub layers: Vec<Layer>,
}

const FLIP_FLAGS: u32 = 0xF000_0000;

// ------------------------------------------------------------------ tile data

/// Standard base64 (whitespace ignored).
pub fn base64(text: &str) -> Result<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c).context("invalid base64 data")? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// A layer's tile ids from its encoded data.
pub fn decode_data(encoding: Option<&str>, compression: Option<&str>, text: &str, expected: usize) -> Result<Vec<u32>> {
    let ids: Vec<u32> = match encoding.unwrap_or("") {
        "csv" => text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse::<u32>().context("bad CSV tile data")).collect::<Result<_>>()?,
        "base64" => {
            let raw = base64(text)?;
            let bytes = match compression.unwrap_or("") {
                "" => raw,
                "zlib" => {
                    let mut v = Vec::new();
                    flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut v).context("zlib tile data")?;
                    v
                }
                "gzip" => {
                    let mut v = Vec::new();
                    flate2::read::GzDecoder::new(&raw[..]).read_to_end(&mut v).context("gzip tile data")?;
                    v
                }
                "zstd" => bail!("zstd-compressed tile data isn't supported: in Tiled choose Map › Map Properties › Tile Layer Format › CSV (or base64 zlib)"),
                c => bail!("unknown tile data compression '{c}'"),
            };
            bytes.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect()
        }
        "" => bail!("tile data as XML <tile> elements isn't supported: in Tiled choose Map › Map Properties › Tile Layer Format › CSV"),
        e => bail!("unknown tile data encoding '{e}'"),
    };
    if ids.len() != expected {
        bail!("a layer has {} tiles, the map needs {expected}", ids.len());
    }
    Ok(ids)
}

// ------------------------------------------------------------------ XML

/// A minimal XML element tree.
#[derive(Clone, Debug, Default)]
pub struct El {
    pub name: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<El>,
    pub text: String,
}

impl El {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.get(k).map(String::as_str)
    }
    fn num(&self, k: &str) -> u32 {
        self.attr(k).and_then(|v| v.trim().parse::<f64>().ok()).map_or(0, |v| v.max(0.0) as u32)
    }
    fn float(&self, k: &str) -> f32 {
        self.attr(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)
    }
    fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a El> {
        self.children.iter().filter(move |c| c.name == name)
    }
    fn kid(&self, name: &str) -> Option<&El> {
        self.children.iter().find(|c| c.name == name)
    }
    /// `<properties><property name=.. value=../></properties>`.
    fn prop(&self, name: &str) -> Option<&str> {
        self.kid("properties")?.kids("property").find(|p| p.attr("name") == Some(name)).and_then(|p| p.attr("value").or(Some(p.text.as_str())))
    }
}

pub fn parse_xml(text: &str) -> Result<El> {
    use quick_xml::events::Event;
    let mut r = quick_xml::Reader::from_str(text);
    let mut stack: Vec<El> = vec![El::default()];
    let element = |e: &quick_xml::events::BytesStart| -> Result<El> {
        let mut el = El { name: String::from_utf8_lossy(e.name().as_ref()).into_owned(), ..Default::default() };
        for a in e.attributes() {
            let a = a.context("bad XML attribute")?;
            let v = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).context("bad XML attribute value")?;
            el.attrs.insert(String::from_utf8_lossy(a.key.as_ref()).into_owned(), v.into_owned());
        }
        Ok(el)
    };
    loop {
        match r.read_event().context("reading XML")? {
            Event::Start(e) => stack.push(element(&e)?),
            Event::Empty(e) => {
                let el = element(&e)?;
                stack.last_mut().unwrap().children.push(el);
            }
            Event::End(_) => {
                let el = stack.pop().context("unbalanced XML")?;
                stack.last_mut().context("unbalanced XML")?.children.push(el);
            }
            Event::Text(t) => stack.last_mut().unwrap().text.push_str(&t.decode().context("bad XML text")?),
            Event::CData(t) => stack.last_mut().unwrap().text.push_str(&String::from_utf8_lossy(&t)),
            Event::Eof => break,
            _ => {}
        }
    }
    stack.pop().and_then(|root| root.children.into_iter().next()).context("empty XML document")
}

fn tileset_from_xml(el: &El, dir: &Path, firstgid: u32) -> Tileset {
    let mut ts = Tileset {
        firstgid,
        name: el.attr("name").unwrap_or("tileset").to_string(),
        tile_w: el.num("tilewidth"),
        tile_h: el.num("tileheight"),
        spacing: el.num("spacing"),
        margin: el.num("margin"),
        columns: el.num("columns"),
        tilecount: el.num("tilecount"),
        ..Default::default()
    };
    if let Some(img) = el.kid("image") {
        ts.image = img.attr("source").map(|s| dir.join(s));
        if ts.columns == 0 && ts.tile_w > 0 {
            ts.columns = (img.num("width").saturating_sub(ts.margin) + ts.spacing) / (ts.tile_w + ts.spacing);
        }
    }
    for t in el.kids("tile") {
        let id = t.num("id");
        let class = t.attr("class").or(t.attr("type")).unwrap_or("");
        if class.eq_ignore_ascii_case("solid") || t.prop("solid").is_some_and(|v| v == "true" || v == "1") {
            ts.solid.insert(id);
        }
        if let Some(a) = t.kid("animation") {
            ts.anims.insert(id, a.kids("frame").map(|f| (f.num("tileid"), f.num("duration"))).collect());
        }
    }
    for w in el.kid("wangsets").into_iter().flat_map(|w| w.kids("wangset")) {
        ts.wangsets.push(WangSet {
            name: w.attr("name").unwrap_or("").to_string(),
            kind: w.attr("type").unwrap_or("").to_string(),
            colors: w.kids("wangcolor").map(|c| c.attr("name").unwrap_or("").to_string()).collect(),
            tiles: w.kids("wangtile").filter_map(|t| Some((t.num("tileid"), wang_id(t.attr("wangid")?.split(',').map(|v| v.trim().parse().unwrap_or(0)))?))).collect(),
        });
    }
    ts
}

/// The 8 colours of a Wang id (`None` for the old hexadecimal format or a wrong length).
fn wang_id(vals: impl Iterator<Item = u32>) -> Option<[u32; 8]> {
    let v: Vec<u32> = vals.collect();
    v.try_into().ok()
}

fn layers_from_xml(el: &El, out: &mut Vec<Layer>, w: u32, h: u32) -> Result<()> {
    for c in &el.children {
        let name = c.attr("name").unwrap_or("").to_string();
        let above_prop = c.prop("above").is_some_and(|v| v == "true");
        match c.name.as_str() {
            "layer" => {
                let d = c.kid("data").context("a tile layer without data")?;
                if d.kid("chunk").is_some() {
                    bail!("infinite maps aren't supported: in Tiled uncheck Map › Map Properties › Infinite");
                }
                let ids = decode_data(d.attr("encoding"), d.attr("compression"), &d.text, (w * h) as usize).with_context(|| format!("layer '{name}'"))?;
                out.push(Layer { name, above_prop, data: LayerData::Tiles(ids) });
            }
            "objectgroup" => {
                let objs = c
                    .kids("object")
                    .map(|o| {
                        let mut ob = Object {
                            name: o.attr("name").unwrap_or("").to_string(),
                            class: o.attr("class").or(o.attr("type")).unwrap_or("").to_string(),
                            x: o.float("x"),
                            y: o.float("y"),
                            w: o.float("width"),
                            h: o.float("height"),
                            point: o.kid("point").is_some(),
                            tile: o.attr("gid").is_some(),
                        };
                        // Polygons / polylines: their bounding box.
                        if let Some(p) = o.kid("polygon").or(o.kid("polyline")).and_then(|p| p.attr("points")) {
                            bounds(&mut ob, p.split_whitespace().filter_map(|xy| xy.split_once(',')).filter_map(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))));
                        }
                        ob
                    })
                    .collect();
                out.push(Layer { name, above_prop, data: LayerData::Objects(objs) });
            }
            "imagelayer" => out.push(Layer { name, above_prop, data: LayerData::Image }),
            "group" => layers_from_xml(c, out, w, h)?,
            _ => {}
        }
    }
    Ok(())
}

/// Sets an object's rectangle to the bounding box of its polygon points (relative to x, y).
fn bounds(o: &mut Object, pts: impl Iterator<Item = (f32, f32)>) {
    let pts: Vec<(f32, f32)> = pts.collect();
    if pts.is_empty() {
        return;
    }
    let (minx, maxx) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (miny, maxy) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    o.x += minx;
    o.y += miny;
    o.w = maxx - minx;
    o.h = maxy - miny;
}

/// A `.tmx` map; external tilesets are read relative to `dir`.
pub fn parse_tmx(text: &str, dir: &Path) -> Result<Parsed> {
    let m = parse_xml(text)?;
    if m.name != "map" {
        bail!("not a Tiled map (.tmx)");
    }
    check_map(m.attr("orientation"), m.attr("infinite") == Some("1"))?;
    let mut p = Parsed { width: m.num("width"), height: m.num("height"), tile_w: m.num("tilewidth"), tile_h: m.num("tileheight"), ..Default::default() };
    for t in m.kids("tileset") {
        let firstgid = t.num("firstgid");
        match t.attr("source") {
            Some(src) => p.tilesets.push(external_tileset(&dir.join(src), firstgid)?),
            None => p.tilesets.push(tileset_from_xml(t, dir, firstgid)),
        }
    }
    layers_from_xml(&m, &mut p.layers, p.width, p.height)?;
    Ok(p)
}

fn check_map(orientation: Option<&str>, infinite: bool) -> Result<()> {
    if orientation.is_some_and(|o| o != "orthogonal") {
        bail!("only orthogonal Tiled maps can be imported (this one is {})", orientation.unwrap_or(""));
    }
    if infinite {
        bail!("infinite maps aren't supported: in Tiled uncheck Map › Map Properties › Infinite");
    }
    Ok(())
}

/// An external tileset (`.tsx` or `.tsj` / `.json`).
fn external_tileset(path: &Path, firstgid: u32) -> Result<Tileset> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading tileset {}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    if path.extension().is_some_and(|e| e == "tsx") {
        let el = parse_xml(&text)?;
        Ok(tileset_from_xml(&el, dir, firstgid))
    } else {
        Ok(tileset_from_json(&json(&text)?, dir, firstgid))
    }
}

// ------------------------------------------------------------------ JSON

type JMap = rhai::Map;

pub fn json(text: &str) -> Result<JMap> {
    rhai::Engine::new_raw().parse_json(text, true).map_err(|e| anyhow::anyhow!("bad JSON: {e}"))
}

pub(crate) fn j_num(m: &JMap, k: &str) -> f64 {
    m.get(k).and_then(|v| v.as_float().ok().or_else(|| v.as_int().ok().map(|i| i as f64))).unwrap_or(0.0)
}
pub(crate) fn j_str(m: &JMap, k: &str) -> String {
    m.get(k).and_then(|v| v.clone().into_string().ok()).unwrap_or_default()
}
pub(crate) fn j_arr(m: &JMap, k: &str) -> rhai::Array {
    m.get(k).and_then(|v| v.clone().try_cast::<rhai::Array>()).unwrap_or_default()
}
pub(crate) fn j_map(v: &rhai::Dynamic) -> Option<JMap> {
    v.clone().try_cast::<JMap>()
}
fn j_prop(m: &JMap, name: &str) -> Option<String> {
    j_arr(m, "properties").iter().filter_map(j_map).find(|p| j_str(p, "name") == name).and_then(|p| p.get("value").map(|v| v.to_string()))
}

fn tileset_from_json(t: &JMap, dir: &Path, firstgid: u32) -> Tileset {
    let mut ts = Tileset {
        firstgid,
        name: j_str(t, "name"),
        tile_w: j_num(t, "tilewidth") as u32,
        tile_h: j_num(t, "tileheight") as u32,
        spacing: j_num(t, "spacing") as u32,
        margin: j_num(t, "margin") as u32,
        columns: j_num(t, "columns") as u32,
        tilecount: j_num(t, "tilecount") as u32,
        ..Default::default()
    };
    let img = j_str(t, "image");
    if !img.is_empty() {
        ts.image = Some(dir.join(img));
    }
    for tile in j_arr(t, "tiles").iter().filter_map(j_map) {
        let id = j_num(&tile, "id") as u32;
        let class = if tile.contains_key("class") { j_str(&tile, "class") } else { j_str(&tile, "type") };
        if class.eq_ignore_ascii_case("solid") || j_prop(&tile, "solid").is_some_and(|v| v == "true") {
            ts.solid.insert(id);
        }
        let frames: Vec<(u32, u32)> = j_arr(&tile, "animation").iter().filter_map(j_map).map(|f| (j_num(&f, "tileid") as u32, j_num(&f, "duration") as u32)).collect();
        if !frames.is_empty() {
            ts.anims.insert(id, frames);
        }
    }
    for w in j_arr(t, "wangsets").iter().filter_map(j_map) {
        ts.wangsets.push(WangSet {
            name: j_str(&w, "name"),
            kind: j_str(&w, "type"),
            colors: j_arr(&w, "colors").iter().filter_map(j_map).map(|c| j_str(&c, "name")).collect(),
            tiles: j_arr(&w, "wangtiles")
                .iter()
                .filter_map(j_map)
                .filter_map(|t| Some((j_num(&t, "tileid") as u32, wang_id(j_arr(&t, "wangid").iter().map(|v| v.as_int().unwrap_or(0).max(0) as u32))?)))
                .collect(),
        });
    }
    ts
}

fn layers_from_json(arr: &rhai::Array, out: &mut Vec<Layer>, w: u32, h: u32) -> Result<()> {
    for l in arr.iter().filter_map(j_map) {
        let name = j_str(&l, "name");
        let above_prop = j_prop(&l, "above").is_some_and(|v| v == "true");
        match j_str(&l, "type").as_str() {
            "tilelayer" => {
                if l.contains_key("chunks") {
                    bail!("infinite maps aren't supported: in Tiled uncheck Map › Map Properties › Infinite");
                }
                let data = l.get("data").cloned().unwrap_or_default();
                let ids = match data.clone().try_cast::<rhai::Array>() {
                    Some(a) => a.iter().map(|v| v.as_int().unwrap_or(0) as u32).collect(),
                    None => {
                        let enc = j_str(&l, "encoding");
                        let comp = j_str(&l, "compression");
                        decode_data(Some(&enc), Some(&comp), &data.into_string().unwrap_or_default(), (w * h) as usize)?
                    }
                };
                if ids.len() != (w * h) as usize {
                    bail!("layer '{name}' has {} tiles, the map needs {}", ids.len(), w * h);
                }
                out.push(Layer { name, above_prop, data: LayerData::Tiles(ids) });
            }
            "objectgroup" => {
                let objs = j_arr(&l, "objects")
                    .iter()
                    .filter_map(j_map)
                    .map(|o| {
                        let mut ob = Object {
                            name: j_str(&o, "name"),
                            class: if o.contains_key("class") { j_str(&o, "class") } else { j_str(&o, "type") },
                            x: j_num(&o, "x") as f32,
                            y: j_num(&o, "y") as f32,
                            w: j_num(&o, "width") as f32,
                            h: j_num(&o, "height") as f32,
                            point: o.get("point").and_then(|p| p.as_bool().ok()).unwrap_or(false),
                            tile: o.contains_key("gid"),
                        };
                        let pts = if o.contains_key("polygon") { j_arr(&o, "polygon") } else { j_arr(&o, "polyline") };
                        bounds(&mut ob, pts.iter().filter_map(j_map).map(|p| (j_num(&p, "x") as f32, j_num(&p, "y") as f32)));
                        ob
                    })
                    .collect();
                out.push(Layer { name, above_prop, data: LayerData::Objects(objs) });
            }
            "imagelayer" => out.push(Layer { name, above_prop, data: LayerData::Image }),
            "group" => layers_from_json(&j_arr(&l, "layers"), out, w, h)?,
            _ => {}
        }
    }
    Ok(())
}

/// A `.tmj` (JSON) map.
pub fn parse_tmj(text: &str, dir: &Path) -> Result<Parsed> {
    let m = json(text)?;
    if j_str(&m, "type") != "map" && !m.contains_key("layers") {
        bail!("not a Tiled map (.tmj)");
    }
    let orientation = j_str(&m, "orientation");
    check_map((!orientation.is_empty()).then_some(orientation.as_str()), m.get("infinite").and_then(|v| v.as_bool().ok()).unwrap_or(false))?;
    let mut p = Parsed { width: j_num(&m, "width") as u32, height: j_num(&m, "height") as u32, tile_w: j_num(&m, "tilewidth") as u32, tile_h: j_num(&m, "tileheight") as u32, ..Default::default() };
    for t in j_arr(&m, "tilesets").iter().filter_map(j_map) {
        let firstgid = j_num(&t, "firstgid") as u32;
        let src = j_str(&t, "source");
        if src.is_empty() {
            p.tilesets.push(tileset_from_json(&t, dir, firstgid));
        } else {
            p.tilesets.push(external_tileset(&dir.join(src), firstgid)?);
        }
    }
    layers_from_json(&j_arr(&m, "layers"), &mut p.layers, p.width, p.height)?;
    Ok(p)
}

// ------------------------------------------------------------------ conversion

/// What an import did and dropped (the message shown afterwards).
#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub tilesets: Vec<String>,
    pub scaled: Vec<String>,
    pub layers: usize,
    pub zones: usize,
    pub spawns: usize,
    pub collision: usize,
    pub flips: usize,
    pub dropped_objects: usize,
    pub image_layers: usize,
    pub dropped_tilesets: Vec<String>,
    pub uneven_anims: usize,
    /// Terrains made from Wang sets (`name (47 tiles)`).
    pub terrains: Vec<String>,
    /// Wang sets that don't fit (corner sets, or tiles whose corners lack their sides).
    pub dropped_wang: Vec<String>,
}

impl Report {
    pub fn message(&self, map: &Map) -> String {
        let mut s = format!(
            "Imported Tiled map '{}' ({}×{}, {} layers, tilesets {}; {} zones, {} spawn points, {} collision cells).",
            map.name,
            map.width,
            map.height,
            self.layers,
            if self.tilesets.is_empty() { "none".into() } else { self.tilesets.join(", ") },
            self.zones,
            self.spawns,
            self.collision
        );
        let mut notes = Vec::new();
        if !self.scaled.is_empty() {
            notes.push(format!("scaled to 32 px tiles: {}", self.scaled.join(", ")));
        }
        if self.flips > 0 {
            notes.push(format!("{} flipped / rotated tiles lost their flip", self.flips));
        }
        if self.dropped_objects > 0 {
            notes.push(format!("{} objects not imported (tile objects, points not named spawn:…)", self.dropped_objects));
        }
        if self.image_layers > 0 {
            notes.push(format!("{} image layers not imported", self.image_layers));
        }
        if !self.dropped_tilesets.is_empty() {
            notes.push(format!("tilesets of single images not imported: {}", self.dropped_tilesets.join(", ")));
        }
        if !self.terrains.is_empty() {
            s += &format!(" Terrains: {}.", self.terrains.join(", "));
        }
        if !self.dropped_wang.is_empty() {
            notes.push(format!("Wang sets not (fully) imported: {}", self.dropped_wang.join("; ")));
        }
        if self.uneven_anims > 0 {
            notes.push(format!("{} animations with uneven frame times use their average speed", self.uneven_anims));
        }
        if !notes.is_empty() {
            s += &format!(" Note: {}.", notes.join("; "));
        }
        s
    }
}

/// Repacks a Tiled tileset image into a plain grid of 32 px tiles (same columns).
fn repack(img: &Rgba, ts: &Tileset) -> Rgba {
    let cols = ts.columns.max(1);
    let rows = ts.tilecount.max(1).div_ceil(cols);
    let mut out = Rgba::new(cols * TS, rows * TS);
    for i in 0..ts.tilecount {
        let (c, r) = (i % cols, i / cols);
        let (sx, sy) = (ts.margin + c * (ts.tile_w + ts.spacing), ts.margin + r * (ts.tile_h + ts.spacing));
        for dy in 0..TS {
            for dx in 0..TS {
                let (x, y) = (sx + dx * ts.tile_w / TS, sy + dy * ts.tile_h / TS);
                out.set((c * TS + dx) as i32, (r * TS + dy) as i32, img.get(x as i32, y as i32));
            }
        }
    }
    out
}

/// Terrains from a tileset's Wang sets, one per colour: *mixed* sets fill the 47-tile set
/// (a tile whose corner colour lacks one of the two sides next to it doesn't fit and is
/// skipped), *edge* sets the 16-tile set; *corner* sets are reported.
pub fn wang_terrains(sets: &[WangSet], rep: &mut Report) -> Vec<Terrain> {
    use rustpg2d_engine::autotile::{reduce, BLOB_SLOT, BLOB_SLOTS, EAST, NORTH, NORTH_EAST, NORTH_WEST, SOUTH, SOUTH_EAST, SOUTH_WEST, WEST};
    // Wang id order: top, top-right, right, bottom-right, bottom, bottom-left, left, top-left.
    const BITS: [usize; 8] = [NORTH, NORTH_EAST, EAST, SOUTH_EAST, SOUTH, SOUTH_WEST, WEST, NORTH_WEST];
    let mut out = Vec::new();
    for w in sets {
        let (blob, edge) = (w.kind == "mixed", w.kind == "edge");
        if !blob && !edge {
            rep.dropped_wang.push(format!("{} ({} set: terrains here follow the sides, not the corners)", w.name, if w.kind.is_empty() { "old" } else { &w.kind }));
            continue;
        }
        let mut misfits = 0;
        for (c, color) in w.colors.iter().enumerate() {
            let c = c as u32 + 1;
            let name = if w.colors.len() == 1 || color.is_empty() { if w.name.is_empty() { color.clone() } else { w.name.clone() } } else { color.clone() };
            let mut t = Terrain { name, ..Default::default() };
            if blob {
                t.blob = vec![None; BLOB_SLOTS];
            }
            for (tile, id) in &w.tiles {
                let mask = id.iter().zip(BITS).filter(|(v, _)| **v == c).fold(0, |m, (_, b)| m | b);
                // A tile without this colour belongs to other colours (or none).
                if mask == 0 {
                    continue;
                }
                if blob {
                    if reduce(mask) != mask {
                        misfits += 1;
                        continue;
                    }
                    t.blob[BLOB_SLOT[mask] as usize] = Some(*tile);
                } else {
                    t.tiles[mask & 15] = Some(*tile);
                }
            }
            if t.tiles_set() > 0 {
                rep.terrains.push(format!("{} ({} tiles)", t.name, if blob { 47 } else { 16 }));
                out.push(t);
            }
        }
        if misfits > 0 {
            rep.dropped_wang.push(format!("{}: {misfits} tiles with a corner but not both of its sides", w.name));
        }
    }
    out
}

/// Builds the R(ust)PG2D map: tilesets are written into `<root>/tilesets/` and added to `db`
/// (never overwriting anything), the map is returned (not saved yet).
pub fn convert(p: &Parsed, name: &str, root: &Path, db: &mut Database) -> Result<(Map, Report)> {
    if p.width == 0 || p.height == 0 || p.tile_w == 0 {
        bail!("the map has no size");
    }
    if p.tile_w != p.tile_h {
        bail!("only square tiles can be imported (the map uses {}×{})", p.tile_w, p.tile_h);
    }
    let mut rep = Report::default();
    let mut map = Map::new(name, p.width, p.height);
    // Tilesets: (firstgid, slot or None when dropped).
    let mut slots: Vec<(u32, Option<u16>)> = Vec::new();
    for ts in &p.tilesets {
        let Some(path) = ts.image.as_ref() else {
            rep.dropped_tilesets.push(ts.name.clone());
            slots.push((ts.firstgid, None));
            continue;
        };
        if ts.tile_w == 0 || ts.tile_h == 0 {
            bail!("tileset '{}' has no tile size", ts.name);
        }
        let bytes = std::fs::read(path).with_context(|| format!("reading tileset image {}", path.display()))?;
        let img = Rgba::decode(&bytes).with_context(|| format!("{} (tileset images must be PNG)", path.display()))?;
        let mut ts = ts.clone();
        if ts.columns == 0 {
            ts.columns = ((img.w.saturating_sub(ts.margin) + ts.spacing) / (ts.tile_w + ts.spacing)).max(1);
        }
        if ts.tilecount == 0 {
            ts.tilecount = ts.columns * ((img.h.saturating_sub(ts.margin) + ts.spacing) / (ts.tile_h + ts.spacing)).max(1);
        }
        let png = repack(&img, &ts).encode()?;
        let base = sanitize(if ts.name.is_empty() { name } else { &ts.name });
        let id = unique_id(&base, |c| db.tileset(c).is_some());
        let rel = write_new(root, "tilesets", &id, "png", &png)?;
        let mut animations = Vec::new();
        for (tile, frames) in &ts.anims {
            let total: u32 = frames.iter().map(|f| f.1).sum();
            if frames.iter().any(|f| f.1 != frames[0].1) {
                rep.uneven_anims += 1;
            }
            let ms = (total as f32 / frames.len().max(1) as f32).max(1.0);
            animations.push(TileAnim { tile: *tile, frames: frames.iter().map(|f| f.0).collect(), fps: 1000.0 / ms });
        }
        let terrains = wang_terrains(&ts.wangsets, &mut rep);
        db.tilesets.push(TilesetDef { id: id.clone(), image: rel, solid: ts.solid.iter().copied().collect(), animations, terrains, ..Default::default() });
        if (ts.tile_w, ts.tile_h) != (TS, TS) {
            rep.scaled.push(format!("{id} ({} px)", ts.tile_w));
        }
        rep.tilesets.push(id.clone());
        slots.push((ts.firstgid, Some(map.tileset_slot(&id))));
    }
    slots.sort_by_key(|s| s.0);
    let tile_of = |gid: u32| -> Option<Tile> {
        let gid = gid & !FLIP_FLAGS;
        if gid == 0 {
            return None;
        }
        let (first, slot) = slots.iter().rev().find(|s| s.0 <= gid)?;
        Some(Tile::new((*slot)?, gid - first))
    };
    let (w, h) = (p.width, p.height);
    let mut layers = Vec::new();
    for l in &p.layers {
        match &l.data {
            LayerData::Tiles(ids) if l.name.eq_ignore_ascii_case("collision") => {
                for (i, &gid) in ids.iter().enumerate() {
                    if gid & !FLIP_FLAGS != 0 {
                        map.set_collision(i as u32 % w, i as u32 / w, CollisionOverride::Solid);
                        rep.collision += 1;
                    }
                }
            }
            LayerData::Tiles(ids) => {
                let lower = l.name.to_lowercase();
                let above = l.above_prop || lower.contains("over") || lower.contains("above");
                let mut layer = TileLayer::new(if l.name.is_empty() { "Layer" } else { &l.name }, w, h, above);
                for (i, &gid) in ids.iter().enumerate() {
                    if gid & FLIP_FLAGS != 0 {
                        rep.flips += 1;
                    }
                    if let Some(t) = tile_of(gid) {
                        layer.set(i as u32 % w, i as u32 / w, t);
                    }
                }
                layers.push(layer);
            }
            LayerData::Objects(objs) => {
                let tw = p.tile_w as f32;
                for o in objs {
                    if o.tile {
                        rep.dropped_objects += 1;
                        continue;
                    }
                    let spawn = o.name.strip_prefix("spawn:").map(str::to_string).or_else(|| o.class.eq_ignore_ascii_case("spawn").then(|| o.name.clone()));
                    let (x, y) = ((o.x / tw).floor().max(0.0) as u32, (o.y / tw).floor().max(0.0) as u32);
                    if x >= w || y >= h {
                        rep.dropped_objects += 1;
                        continue;
                    }
                    let uid = map.alloc_uid();
                    if let Some(s) = spawn {
                        map.zones.push(Zone { uid, name: if s.is_empty() { format!("spawn_{uid}") } else { s }, x, y, w: 1, h: 1, kind: ZoneKind::Spawn, triggers: vec![], loot: None });
                        rep.spawns += 1;
                    } else if o.point || o.w <= 0.0 || o.h <= 0.0 {
                        rep.dropped_objects += 1;
                    } else {
                        let (zw, zh) = (((o.x + o.w) / tw).ceil() as u32 - x, ((o.y + o.h) / tw).ceil() as u32 - y);
                        let name = if o.name.is_empty() { format!("zone_{uid}") } else { o.name.clone() };
                        map.zones.push(Zone { uid, name, x, y, w: zw.clamp(1, w - x), h: zh.clamp(1, h - y), kind: ZoneKind::Trigger, triggers: vec![], loot: None });
                        rep.zones += 1;
                    }
                }
            }
            LayerData::Image => rep.image_layers += 1,
        }
    }
    if !layers.is_empty() {
        rep.layers = layers.len();
        map.layers = layers;
    } else {
        rep.layers = map.layers.len();
    }
    Ok((map, rep))
}

/// Reads a `.tmx` / `.tmj` file.
pub fn parse_file(file: &Path) -> Result<Parsed> {
    let text = std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let dir = file.parent().unwrap_or(Path::new("."));
    match file.extension().and_then(|e| e.to_str()) {
        Some("tmj") | Some("json") => parse_tmj(&text, dir),
        _ => parse_tmx(&text, dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn data_encodings() {
        assert_eq!(base64("AQAAAAIAAAA=").unwrap(), vec![1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(decode_data(Some("csv"), None, "1,2,\n3,0", 4).unwrap(), vec![1, 2, 3, 0]);
        assert_eq!(decode_data(Some("base64"), None, "AQAAAAIAAAA=", 2).unwrap(), vec![1, 2]);
        // zlib of [5, 0, 0, 0] (made with Python's zlib.compress).
        assert_eq!(decode_data(Some("base64"), Some("zlib"), "eJxjZWBgAAAAGAAG", 1).unwrap(), vec![5]);
        assert!(decode_data(Some("base64"), Some("zstd"), "AA==", 1).is_err());
        assert!(decode_data(Some("csv"), None, "1,2", 3).is_err());
    }

    fn check(p: &Parsed) {
        assert_eq!((p.width, p.height, p.tile_w), (6, 4, 16));
        assert_eq!(p.tilesets.len(), 2);
        let ground = &p.tilesets[0];
        assert_eq!((ground.firstgid, ground.columns, ground.tilecount), (1, 2, 4));
        assert!(ground.solid.contains(&3));
        assert_eq!(ground.anims.get(&1), Some(&vec![(1, 250), (2, 250)]));
        assert_eq!(ground.wangsets.len(), 2);
        assert_eq!((ground.wangsets[0].kind.as_str(), ground.wangsets[0].colors.clone()), ("mixed", vec!["Path".to_string()]));
        assert_eq!(ground.wangsets[0].tiles[2], (2, [1, 0, 1, 0, 0, 0, 0, 0]));
        let deco = &p.tilesets[1];
        assert_eq!((deco.firstgid, deco.spacing, deco.margin, deco.tile_w), (5, 2, 1, 32));
        let names: Vec<&str> = p.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["Ground", "Overhead trees", "collision", "Objects"]);
        let LayerData::Objects(objs) = &p.layers[3].data else { panic!() };
        assert_eq!(objs.len(), 4);
    }

    #[test]
    fn tmx_and_tmj_parse_alike() {
        let dir = fixtures();
        let tmx = parse_file(&dir.join("tiny.tmx")).unwrap();
        check(&tmx);
        let tmj = parse_file(&dir.join("tiny.tmj")).unwrap();
        check(&tmj);
        let ids = |p: &Parsed, k: usize| match &p.layers[k].data {
            LayerData::Tiles(t) => t.clone(),
            _ => panic!(),
        };
        assert_eq!(ids(&tmx, 0), ids(&tmj, 0));
        assert_eq!(ids(&tmx, 1), ids(&tmj, 1), "base64 + zlib in the tmx, an array in the tmj");
    }

    #[test]
    fn converts_to_a_map() {
        let root = std::env::temp_dir().join(format!("rustpg2d_tiled_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let p = parse_file(&fixtures().join("tiny.tmx")).unwrap();
        let mut db = Database::default();
        db.tilesets.push(TilesetDef { id: "ground".into(), image: "x.png".into(), ..Default::default() });
        let (map, rep) = convert(&p, "tiny", &root, &mut db).unwrap();
        // Ids never clash with existing ones; images are 32 px grids.
        assert_eq!(rep.tilesets, vec!["ground_2".to_string(), "decorations".to_string()]);
        let img = Rgba::decode(&std::fs::read(root.join("tilesets/ground_2.png")).unwrap()).unwrap();
        assert_eq!((img.w, img.h), (64, 64), "2×2 tiles of 16 px, upscaled");
        let deco = Rgba::decode(&std::fs::read(root.join("tilesets/decorations.png")).unwrap()).unwrap();
        assert_eq!((deco.w, deco.h), (64, 32), "margin and spacing removed");
        // Its pixels: tile 1 of the deco set is blue in the fixture.
        assert_eq!(deco.get(40, 10), [0, 0, 255, 255]);
        let g = db.tileset("ground_2").unwrap();
        assert_eq!(g.solid, vec![3]);
        assert_eq!(g.animations, vec![TileAnim { tile: 1, frames: vec![1, 2], fps: 4.0 }]);
        assert_eq!(map.layers.len(), 2);
        assert!(map.layers[1].above_entities, "'Overhead trees'");
        assert_eq!(map.layers[0].get(0, 0), Tile::new(0, 0));
        assert_eq!(map.layers[0].get(5, 3), Tile::new(0, 3));
        assert_eq!(map.layers[1].get(2, 1), Tile::new(1, 1));
        assert_eq!(rep.flips, 1);
        assert_eq!((rep.collision, map.collision_at(0, 3)), (2, CollisionOverride::Solid));
        assert_eq!((rep.zones, rep.spawns, rep.dropped_objects), (2, 1, 1));
        let zones: Vec<(String, u32, u32, u32, u32, bool)> = map.zones.iter().map(|z| (z.name.clone(), z.x, z.y, z.w, z.h, z.kind == ZoneKind::Spawn)).collect();
        assert_eq!(
            zones,
            vec![("door".into(), 1, 1, 2, 1, false), ("start".into(), 3, 2, 1, 1, true), ("pond".into(), 3, 0, 2, 2, false)]
        );
        // The mixed Wang set became a 47-tile terrain; the corner set and the misfit are reported.
        let t = &g.terrains[0];
        use rustpg2d_engine::autotile::{BLOB_SLOT, EAST, NORTH};
        assert_eq!((t.name.as_str(), t.is_blob(), t.tiles_set()), ("Paths", true, 3));
        assert_eq!((t.inside(), t.blob_tile(BLOB_SLOT[NORTH] as usize), t.blob_tile(BLOB_SLOT[NORTH | EAST] as usize)), (Some(0), Some(1), Some(2)));
        assert_eq!(rep.dropped_wang.len(), 2, "{:?}", rep.dropped_wang);
        let msg = rep.message(&map);
        assert!(msg.contains("1 flipped") && msg.contains("ground_2 (16 px)") && msg.contains("Paths (47 tiles)"), "{msg}");
        let _ = std::fs::remove_dir_all(root);
    }
}
