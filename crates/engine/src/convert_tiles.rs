//! One-time conversion of tile maps (R(ust)PG2D 0.20 and older) into painted maps: the ground
//! layer becomes painted ground in materials, the other layers decorations, tile structures
//! structures of ground cells and decorations.
//!
//! The conversion works tile by tile, so maps and structures share it:
//! - a **terrain tile** (any layer) paints the 8 px cells its neighbour mask says are the terrain
//!   with the terrain's material (its inside tile as the texture); on the ground layer the other
//!   cells get the terrain's base (plain grass under a path's edge), on upper layers they keep
//!   what is below;
//! - any other **ground-layer tile** becomes a material of its own (the tile as a 32 px texture,
//!   repeated on the tile grid, so it looks as before; crisp edges without noise);
//! - any other **upper-layer tile** becomes a decoration (Detail = `Flat`, Overhead = `Above`) of a
//!   prop generated for the tile: a sheet over the tileset image, 32 px frames; solid, low, ladder,
//!   light carried over (an animated tile shows its first frame), a 32 x 32 hitbox.
//!
//! Materials are shared by picture: a material whose texture has exactly the tile's pixels is
//! used instead of making another one (projects that already have materials keep them).
//! Every terrain of every tileset becomes a material, used or not (the project keeps its palette).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::decor::{Decor, DecorLayer};
use crate::defs::{Database, PropDef, SheetDef, SpriteRef, StructureDef};
use crate::ground::{MaterialDef, Walk, CELLS};
use crate::imageio::Rgba;
use crate::map::Map;
use crate::old_tiles::{OldTiles, Terrain, Tile, TilesetDef, BLOB_MASKS, EAST, NORTH, NORTH_EAST, NORTH_WEST, SOUTH, SOUTH_EAST, SOUTH_WEST, WEST};
use crate::project::TILE_SIZE;

/// The 4 x 4 cells of a tile as a bit mask (bit `cy * 4 + cx`).
pub type CellMask = u16;
const ALL_CELLS: CellMask = 0xffff;

/// The cells of a terrain tile that show the terrain, from its neighbour mask: the middle
/// always; an edge row or column only where that neighbour is the same terrain; a corner cell
/// where both sides are (and, for a blob set, the diagonal too: inner corners).
pub fn terrain_cells(mask: usize, blob: bool) -> CellMask {
    let mut out = 0;
    for cy in 0..CELLS {
        for cx in 0..CELLS {
            let (n, s, w, e) = (cy == 0, cy == CELLS - 1, cx == 0, cx == CELLS - 1);
            let need = |cond: bool, bit: usize| !cond || mask & bit != 0;
            let mut inside = need(n, NORTH) && need(s, SOUTH) && need(w, WEST) && need(e, EAST);
            if blob {
                inside &= need(n && e, NORTH_EAST) && need(s && e, SOUTH_EAST) && need(s && w, SOUTH_WEST) && need(n && w, NORTH_WEST);
            }
            if inside {
                out |= 1 << (cy * CELLS + cx);
            }
        }
    }
    out
}

/// What a tile turns into.
#[derive(Clone, Debug, PartialEq)]
pub enum Converted {
    /// Ground: `cells` get `material`, the others `rest` (None = keep what is there).
    Ground { material: String, cells: CellMask, rest: Option<String> },
    /// A decoration of this prop.
    Decor(String),
    /// Nothing (the tileset or its image is missing).
    Nothing,
}

/// What a conversion did.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub maps: Vec<String>,
    pub structures: usize,
    pub materials_added: Vec<String>,
    pub props_added: Vec<String>,
    pub sheets_added: Vec<String>,
    pub warnings: Vec<String>,
    /// Where the project was copied first (`upgrade`).
    pub backup: Option<PathBuf>,
}

impl Report {
    pub fn summary(&self) -> String {
        let mut s = format!(
            "Converted the tile maps into painted maps: {} map(s), {} structure(s); {} new material(s), {} new prop(s)",
            self.maps.len(),
            self.structures,
            self.materials_added.len(),
            self.props_added.len()
        );
        if let Some(b) = &self.backup {
            s.push_str(&format!(". Backup of the old project: {}", b.display()));
        }
        if !self.warnings.is_empty() {
            s.push_str(&format!(" ({} warning(s), see terminal)", self.warnings.len()));
        }
        s
    }
}

/// Terrain membership of a tile: (terrain index, neighbour mask, blob set).
type TerrainOf = HashMap<u32, (usize, usize, bool)>;

/// Converts tiles of one project (its tilesets) into materials and props of `db`.
pub struct TileConverter {
    root: PathBuf,
    tilesets: Vec<TilesetDef>,
    terrain_of: HashMap<String, TerrainOf>,
    images: HashMap<String, Option<Rgba>>,
    by_texture: HashMap<Rgba, String>,
    materials: HashMap<(String, u32), Option<String>>,
    props: HashMap<(String, u32), Option<String>>,
    sheets: HashMap<String, String>,
    pub report: Report,
}

impl TileConverter {
    /// A converter for the project at `root` whose tilesets are `tilesets`; reads the textures of
    /// `db`'s materials to share them.
    pub fn new(root: &Path, tilesets: Vec<TilesetDef>, db: &Database) -> Self {
        let mut by_texture = HashMap::new();
        for m in &db.materials {
            if let Some(img) = std::fs::read(root.join(&m.texture)).ok().and_then(|b| Rgba::decode(&b).ok()) {
                by_texture.entry(img).or_insert_with(|| m.id.clone());
            }
        }
        let mut terrain_of = HashMap::new();
        for ts in &tilesets {
            let mut of = TerrainOf::new();
            for (ti, t) in ts.terrains.iter().enumerate() {
                for (k, tile) in t.blob.iter().enumerate() {
                    if let (Some(tile), Some(&m)) = (tile, BLOB_MASKS.get(k)) {
                        of.entry(*tile).or_insert((ti, m as usize, true));
                    }
                }
                for (m, tile) in t.tiles.iter().enumerate() {
                    if let Some(tile) = tile {
                        of.entry(*tile).or_insert((ti, m, false));
                    }
                }
            }
            terrain_of.insert(ts.id.clone(), of);
        }
        TileConverter {
            root: root.to_path_buf(),
            tilesets,
            terrain_of,
            images: HashMap::new(),
            by_texture,
            materials: HashMap::new(),
            props: HashMap::new(),
            sheets: HashMap::new(),
            report: Report::default(),
        }
    }

    fn tileset(&self, id: &str) -> Option<&TilesetDef> {
        self.tilesets.iter().find(|t| t.id == id)
    }

    fn image(&mut self, ts: &str) -> Option<&Rgba> {
        if !self.images.contains_key(ts) {
            let img = self.tileset(ts).and_then(|d| std::fs::read(self.root.join(&d.image)).ok()).and_then(|b| Rgba::decode(&b).ok());
            if img.is_none() {
                self.report.warnings.push(format!("tileset {ts}: its image can't be read; its tiles are left out"));
            }
            self.images.insert(ts.to_string(), img);
        }
        self.images.get(ts).and_then(|i| i.as_ref())
    }

    /// The 32 x 32 picture of a tile.
    fn tile_picture(&mut self, ts: &str, index: u32) -> Option<Rgba> {
        let img = self.image(ts)?;
        let cols = (img.w / TILE_SIZE).max(1);
        let (x0, y0) = ((index % cols * TILE_SIZE) as i32, (index / cols * TILE_SIZE) as i32);
        if y0 as u32 + TILE_SIZE > img.h {
            return None;
        }
        let mut out = Rgba::new(TILE_SIZE, TILE_SIZE);
        for y in 0..TILE_SIZE as i32 {
            for x in 0..TILE_SIZE as i32 {
                out.set(x, y, img.get(x0 + x, y0 + y));
            }
        }
        Some(out)
    }

    /// The material showing tile `index` of `ts` (made on first use; shared with any material
    /// whose texture is the same picture).
    fn material(&mut self, db: &mut Database, ts: &str, index: u32, terrain: Option<(usize, &Terrain)>) -> Option<String> {
        let key = (ts.to_string(), index);
        if let Some(m) = self.materials.get(&key) {
            return m.clone();
        }
        let made = self.make_material(db, ts, index, terrain);
        self.materials.insert(key, made.clone());
        made
    }

    fn make_material(&mut self, db: &mut Database, ts: &str, index: u32, terrain: Option<(usize, &Terrain)>) -> Option<String> {
        let def = self.tileset(ts)?.clone();
        // An animated tile: its frames side by side (the material animates the same way).
        let anim = def.animations.iter().find(|a| a.tile == index && a.frames.len() > 1 && a.frames.len() <= 64).cloned();
        let pic = match &anim {
            Some(a) => {
                let pics: Vec<Rgba> = a.frames.iter().filter_map(|&f| self.tile_picture(ts, f)).collect();
                if pics.len() != a.frames.len() {
                    return None;
                }
                let mut strip = Rgba::new(TILE_SIZE * pics.len() as u32, TILE_SIZE);
                for (k, p) in pics.iter().enumerate() {
                    for y in 0..TILE_SIZE as i32 {
                        for x in 0..TILE_SIZE as i32 {
                            strip.set(k as i32 * TILE_SIZE as i32 + x, y, p.get(x, y));
                        }
                    }
                }
                strip
            }
            None => self.tile_picture(ts, index)?,
        };
        if let Some(id) = self.by_texture.get(&pic) {
            return Some(id.clone());
        }
        let label = match terrain {
            Some((_, t)) => t.name.clone(),
            None => format!("{} tile {index}", crate::defs::SheetDef::new(ts, "", 1, 1).label()),
        };
        let id = crate::refactor::unique_id(&crate::release::slug(&label).replace('-', "_"), |c| db.material(c).is_some());
        let texture = format!("materials/{id}.png");
        let path = self.root.join(&texture);
        let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).map_err(anyhow::Error::from).and_then(|_| pic.encode()).and_then(|png| std::fs::write(&path, png).map_err(Into::into));
        if let Err(e) = written {
            self.report.warnings.push(format!("material {id}: can't write {}: {e}", path.display()));
            return None;
        }
        let solid = def.solid.contains(&index);
        let walk = match (solid, def.low.contains(&index)) {
            (false, _) => Walk::Walk,
            (true, true) => Walk::Low,
            (true, false) => Walk::Solid,
        };
        let step = match terrain {
            Some((_, t)) if !t.step.is_empty() => t.step.clone(),
            _ => def.steps.iter().find(|s| s.tiles.contains(&index)).map(|s| s.sound.clone()).unwrap_or_default(),
        };
        let m = MaterialDef {
            id: id.clone(),
            name: label,
            texture,
            // Terrains get ragged pixel edges; single tiles keep their tile's square.
            edge: if terrain.is_some() { crate::ground::Edge::Crisp } else { crate::ground::Edge::Straight },
            edge_noise: if terrain.is_some() { 3.0 } else { 0.0 },
            priority: match terrain {
                Some((ti, _)) => (30 + 2 * ti).min(250) as u8,
                None => 10,
            },
            walk,
            step,
            frames: anim.as_ref().map_or(1, |a| a.frames.len() as u8),
            fps: anim.as_ref().map_or(0.0, |a| a.fps),
            ..MaterialDef::default()
        };
        db.materials.push(m);
        self.by_texture.insert(pic, id.clone());
        self.report.materials_added.push(id.clone());
        Some(id)
    }

    /// The sheet over a tileset's image (32 px frames: frame = tile index).
    fn sheet(&mut self, db: &mut Database, ts: &str) -> Option<String> {
        if let Some(s) = self.sheets.get(ts) {
            return Some(s.clone());
        }
        let image = self.tileset(ts)?.image.clone();
        let id = match db.sheets.iter().find(|s| s.image == image && s.frame_w == TILE_SIZE && s.frame_h == TILE_SIZE && s.scale <= 1) {
            Some(s) => s.id.clone(),
            None => {
                let id = crate::refactor::unique_id(&format!("{ts}_tiles"), |c| db.sheet(c).is_some());
                db.sheets.push(SheetDef::new(&id, &image, TILE_SIZE, TILE_SIZE));
                self.report.sheets_added.push(id.clone());
                id
            }
        };
        self.sheets.insert(ts.to_string(), id.clone());
        Some(id)
    }

    /// The prop of an upper-layer tile (made on first use).
    fn prop(&mut self, db: &mut Database, ts: &str, index: u32) -> Option<String> {
        let key = (ts.to_string(), index);
        if let Some(p) = self.props.get(&key) {
            return p.clone();
        }
        let made = self.make_prop(db, ts, index);
        self.props.insert(key, made.clone());
        made
    }

    fn make_prop(&mut self, db: &mut Database, ts: &str, index: u32) -> Option<String> {
        self.image(ts)?;
        let def = self.tileset(ts)?.clone();
        let sheet = self.sheet(db, ts)?;
        let id = crate::refactor::unique_id(&format!("{ts}_tile_{index}"), |c| db.prop(c).is_some());
        let p = PropDef {
            id: id.clone(),
            name: format!("{} tile {index}", SheetDef::new(ts, "", 1, 1).label()),
            sprite: SpriteRef { sheet, frame: index, w: 1, h: 1 },
            solid: def.solid.contains(&index),
            low: def.low.contains(&index),
            climb: def.ladder.contains(&index),
            hitbox: [TILE_SIZE as f32, TILE_SIZE as f32],
            light: def.lights.iter().find(|l| l.tile == index).map(|l| l.light.clone()),
            snap: TILE_SIZE as u16,
            ..PropDef::default()
        };
        db.props.push(p);
        self.report.props_added.push(id.clone());
        Some(id)
    }

    /// What tile `index` of tileset `ts` becomes on the ground layer (`ground`) or above it.
    pub fn convert_tile(&mut self, db: &mut Database, ts: &str, index: u32, ground: bool) -> Converted {
        let terrain = self.terrain_of.get(ts).and_then(|of| of.get(&index)).copied();
        if let Some((ti, mask, blob)) = terrain {
            let Some(t) = self.tileset(ts).and_then(|d| d.terrains.get(ti)).cloned() else { return Converted::Nothing };
            let Some(material) = t.inside().and_then(|inside| self.material(db, ts, inside, Some((ti, &t)))) else { return Converted::Nothing };
            let rest = if ground { t.base.and_then(|b| self.material(db, ts, b, None)) } else { None };
            return Converted::Ground { material, cells: terrain_cells(mask, blob), rest };
        }
        if ground {
            return match self.material(db, ts, index, None) {
                Some(material) => Converted::Ground { material, cells: ALL_CELLS, rest: None },
                None => Converted::Nothing,
            };
        }
        match self.prop(db, ts, index) {
            Some(p) => Converted::Decor(p),
            None => Converted::Nothing,
        }
    }

    /// Makes a material of every terrain of every tileset (the project keeps its palette).
    pub fn convert_terrains(&mut self, db: &mut Database) {
        for ts in self.tilesets.clone() {
            for (ti, t) in ts.terrains.iter().enumerate() {
                if let Some(inside) = t.inside() {
                    self.material(db, &ts.id, inside, Some((ti, t)));
                }
            }
        }
    }

    /// Paints an old map's tiles into its ground and decorations.
    pub fn convert_map(&mut self, db: &mut Database, map: &mut Map, tiles: OldTiles) {
        let OldTiles { tilesets, layers } = tiles;
        let ts = TILE_SIZE as i32;
        let mut full = false;
        for (li, layer) in layers.iter().enumerate() {
            let layer_kind = if layer.above_entities { DecorLayer::Above } else { DecorLayer::Flat };
            for y in 0..map.height {
                for x in 0..map.width {
                    let Some((slot, index)) = layer.get(x, y).parts() else { continue };
                    let Some(set) = tilesets.get(slot) else { continue };
                    match self.convert_tile(db, set, index, li == 0) {
                        Converted::Ground { material, cells, rest } => {
                            let v = map.material_value(&material);
                            let r = rest.and_then(|r| map.material_value(&r));
                            if v.is_none() {
                                full = true;
                            }
                            paint_cells(&mut map.ground, x, y, cells, v, r);
                        }
                        Converted::Decor(prop) => {
                            let def = map.decor_slot(&prop);
                            map.decor.push(Decor { def, x: x as i32 * ts + ts / 2, y: (y as i32 + 1) * ts, flip: false, state: 0, layer: layer_kind });
                        }
                        Converted::Nothing => {}
                    }
                }
            }
        }
        if full {
            self.report.warnings.push(format!("map {}: more than {} materials; some ground was left out", map.name, crate::ground::MAX_MATERIALS));
        }
        map.ground.compact();
        self.report.maps.push(map.name.clone());
    }

    /// Converts a tile structure's layers into ground cells and decorations; tiles it cleared
    /// clear the ground (ground layer) or the decorations (upper layers) where it is placed.
    pub fn convert_structure(&mut self, db: &mut Database, s: &mut StructureDef) {
        let layers = std::mem::take(&mut s.layers);
        let tilesets = std::mem::take(&mut s.tilesets);
        if layers.is_empty() {
            return;
        }
        let (w, h) = (s.w, s.h);
        let gw = w * CELLS;
        let mut ground = vec![0u8; (gw * h * CELLS) as usize];
        let mut clear = vec![0u8; (w * h) as usize];
        let ts = TILE_SIZE as i32;
        for (li, layer) in layers.iter().enumerate() {
            // Structures don't know which layers were "above": the third and later ones were.
            let layer_kind = if li >= 2 { DecorLayer::Above } else { DecorLayer::Flat };
            for y in 0..h {
                for x in 0..w {
                    let v = layer.get((y * w + x) as usize).copied().unwrap_or(0);
                    if v == 0 {
                        continue;
                    }
                    if v == StructureDef::CLEAR {
                        if li == 0 {
                            set_tile_cells(&mut ground, gw, x, y, ALL_CELLS, StructureDef::CLEAR_GROUND);
                        } else {
                            clear[(y * w + x) as usize] = 1;
                        }
                        continue;
                    }
                    let Some((slot, index)) = Tile(v).parts() else { continue };
                    let Some(set) = tilesets.get(slot) else { continue };
                    match self.convert_tile(db, set, index, li == 0) {
                        Converted::Ground { material, cells, rest } => {
                            let v = s.material_value(&material);
                            set_tile_cells(&mut ground, gw, x, y, cells, v);
                            if let Some(r) = rest {
                                let r = s.material_value(&r);
                                set_tile_cells(&mut ground, gw, x, y, !cells, r);
                            }
                        }
                        Converted::Decor(prop) => {
                            let def = s.decor_slot(&prop);
                            s.decor.push(Decor { def, x: x as i32 * ts + ts / 2, y: (y as i32 + 1) * ts, flip: false, state: 0, layer: layer_kind });
                        }
                        Converted::Nothing => {}
                    }
                }
            }
        }
        if ground.iter().any(|&v| v != 0) {
            s.ground = ground;
        }
        if clear.iter().any(|&v| v != 0) {
            s.clear = clear;
        }
        self.report.structures += 1;
    }
}

/// Sets the cells of tile (x, y) in a structure's ground grid (`gw` cells wide).
fn set_tile_cells(ground: &mut [u8], gw: u32, x: u32, y: u32, cells: CellMask, v: u8) {
    for cy in 0..CELLS {
        for cx in 0..CELLS {
            if cells & (1 << (cy * CELLS + cx)) != 0 {
                if let Some(c) = ground.get_mut(((y * CELLS + cy) * gw + x * CELLS + cx) as usize) {
                    *c = v;
                }
            }
        }
    }
}

/// Paints the cells of tile (x, y): `cells` with `v`, the others with `rest` (None = keep).
fn paint_cells(g: &mut crate::ground::Ground, x: u32, y: u32, cells: CellMask, v: Option<u8>, rest: Option<u8>) {
    for cy in 0..CELLS {
        for cx in 0..CELLS {
            let on = cells & (1 << (cy * CELLS + cx)) != 0;
            if let Some(v) = if on { v } else { rest } {
                g.set((x * CELLS + cx) as i32, (y * CELLS + cy) as i32, v);
            }
        }
    }
}

/// Whether a project still has tilesets (it was made before maps were painted).
pub fn needs_conversion(root: &Path) -> bool {
    root.join(TILESETS).is_file()
}

/// The tilesets of an old project.
const TILESETS: &str = "defs/tilesets.ron";

/// Converts a whole project folder (its maps, structures and tilesets) and saves it; the
/// tilesets are removed from its definitions. `backup` = copy the folder there first.
pub fn convert_project(root: &Path, backup: Option<&Path>) -> Result<Report> {
    if let Some(b) = backup {
        crate::migrate::copy_dir(root, b).context("making a backup")?;
    }
    let mut project = crate::project::Project::load(root)?;
    let tilesets = crate::old_tiles::read_tilesets(root)?;
    let mut conv = TileConverter::new(root, tilesets, &project.db);
    conv.report.backup = backup.map(Path::to_path_buf);
    conv.convert_terrains(&mut project.db);
    for name in project.list_maps() {
        let (mut map, tiles) = crate::map::io::load_tiles(&project.map_path(&name)).with_context(|| format!("loading map {name}"))?;
        if tiles.layers.is_empty() {
            continue;
        }
        conv.convert_map(&mut project.db, &mut map, tiles);
        project.save_map(&map).with_context(|| format!("saving map {name}"))?;
    }
    let mut structures = std::mem::take(&mut project.db.structures);
    for s in &mut structures {
        conv.convert_structure(&mut project.db, s);
    }
    project.db.structures = structures;
    project.save_defs()?;
    if root.join(TILESETS).is_file() {
        std::fs::remove_file(root.join(TILESETS)).context("removing defs/tilesets.ron")?;
    }
    Ok(conv.report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(c: CellMask) -> u32 {
        c.count_ones()
    }

    #[test]
    fn terrain_cells_follow_the_mask() {
        assert_eq!(terrain_cells(15, false), ALL_CELLS);
        assert_eq!(terrain_cells(255, true), ALL_CELLS);
        // On its own: the middle 2 x 2.
        assert_eq!(count(terrain_cells(0, false)), 4);
        // An edge (all but north): three rows.
        assert_eq!(count(terrain_cells(EAST | SOUTH | WEST, false)), 12);
        // An outer corner (south and east): 3 x 3.
        assert_eq!(count(terrain_cells(SOUTH | EAST, false)), 9);
        // A blob tile with all four sides but no diagonals: the four corner cells are outside.
        assert_eq!(count(terrain_cells(15, true)), 12);
        assert_eq!(count(terrain_cells(15 | NORTH_EAST, true)), 13);
    }

    #[test]
    fn an_animated_ground_tile_becomes_an_animated_material() {
        let root = std::env::temp_dir().join(format!("rustpg2d_conv_anim_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("tilesets")).unwrap();
        // Four tiles in a row, each one colour: tile 0 animates through 0, 1, 2, 1.
        let mut img = Rgba::new(TILE_SIZE * 4, TILE_SIZE);
        for t in 0..4u8 {
            for y in 0..TILE_SIZE as i32 {
                for x in 0..TILE_SIZE as i32 {
                    img.set(t as i32 * TILE_SIZE as i32 + x, y, [t * 60, 100, 200, 255]);
                }
            }
        }
        std::fs::write(root.join("tilesets/water.png"), img.encode().unwrap()).unwrap();
        let ts = TilesetDef {
            id: "water".into(),
            image: "tilesets/water.png".into(),
            animations: vec![crate::old_tiles::TileAnim { tile: 0, frames: vec![0, 1, 2, 1], fps: 3.0 }],
            ..TilesetDef::default()
        };
        let mut db = Database::default();
        let mut conv = TileConverter::new(&root, vec![ts], &db);
        let (Converted::Ground { material: still, .. }, Converted::Ground { material: anim, .. }) = (conv.convert_tile(&mut db, "water", 3, true), conv.convert_tile(&mut db, "water", 0, true)) else {
            panic!("both are ground")
        };
        let (still, anim) = (db.material(&still).unwrap().clone(), db.material(&anim).unwrap().clone());
        assert_eq!((still.frames, still.fps), (1, 0.0));
        assert_eq!((anim.frames, anim.fps), (4, 3.0));
        let strip = Rgba::decode(&std::fs::read(root.join(&anim.texture)).unwrap()).unwrap();
        assert_eq!((strip.w, strip.h), (TILE_SIZE * 4, TILE_SIZE));
        let shades: Vec<u8> = (0..4).map(|k| strip.get(k * TILE_SIZE as i32 + 5, 5)[0]).collect();
        assert_eq!(shades, [0, 60, 120, 60]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
