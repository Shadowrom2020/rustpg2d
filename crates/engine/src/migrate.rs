//! Converts projects made with another tile size (e.g. 16 px) to the engine's fixed
//! [`TILE_SIZE`] of 32 px.
//!
//! A backup of the whole project folder is made first. Then:
//! - images that are shrunken copies of asset-library art are replaced by the library
//!   originals; all other images are enlarged without loss (nearest neighbour),
//! - tilesets whose image already is a full-size library original (used in small "quarter"
//!   tiles before) keep the image, and the tiles painted with them are remapped to the full tile
//!   they were cut from,
//!   (maps painted with such whole tiles as aligned 2x2 blocks of quarters get a grid half as
//!   big instead, showing the same picture, so the world keeps its size),
//! - frame sizes, hitboxes, walking and projectile speeds are scaled,
//! - entity positions in maps and positions in save games are scaled,
//! - the pixel scale is reduced so the game shows the same part of the world.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::imageio::Rgba;
use crate::map::{CollisionOverride, Edge, Map, Tile, TileLayer};
use crate::library::{all_assets, Bundle};
use crate::project::{write_ron, Project, ProjectConfig, TILE_SIZE};
use crate::state::WorldState;

/// What [`upgrade`] did.
#[derive(Debug)]
pub struct Upgrade {
    pub from: u32,
    pub backup: PathBuf,
    /// Images replaced by the asset library's originals.
    pub restored: Vec<String>,
    /// Images enlarged (no library original found).
    pub enlarged: Vec<String>,
    /// Tilesets kept at full size whose painted tiles were remapped.
    pub remapped: Vec<String>,
    /// Maps painted in whole-tile blocks whose grid was shrunk (world size kept).
    pub shrunk_maps: Vec<String>,
}

impl Upgrade {
    pub fn summary(&self) -> String {
        format!(
            "Converted the project from {}px to {TILE_SIZE}px tiles ({} image(s) restored from the asset library, {} enlarged, {} tileset(s) remapped, {} map(s) re-gridded). Backup: {}",
            self.from,
            self.restored.len(),
            self.enlarged.len(),
            self.remapped.len(),
            self.shrunk_maps.len(),
            self.backup.display()
        )
    }
}

/// The tile size stored in a project's `project.ron`.
pub fn stored_tile_size(root: &Path) -> Result<u32> {
    let s = std::fs::read_to_string(root.join("project.ron")).with_context(|| format!("reading {}", root.join("project.ron").display()))?;
    Ok(ron::from_str::<ProjectConfig>(&s)?.tile_size)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Converts the project at `root` to [`TILE_SIZE`] if it uses another tile size. `library`
/// supplies original art for images that were shrunken copies of it. Returns `None` if the
/// project already uses 32 px tiles.
pub fn upgrade(root: &Path, library: Option<&Project>) -> Result<Option<Upgrade>> {
    let from = stored_tile_size(root)?;
    if from == TILE_SIZE || from == 0 {
        return Ok(None);
    }
    let f = TILE_SIZE as f32 / from as f32;
    let mut project = Project::load(root)?;

    // 1. Backup next to the project.
    let name = root.canonicalize()?.file_name().map_or("project".into(), |n| n.to_string_lossy().into_owned());
    let parent = root.canonicalize()?.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let mut backup = parent.join(format!("{name}-backup-{from}px"));
    let mut n = 2;
    while backup.exists() {
        backup = parent.join(format!("{name}-backup-{from}px-{n}"));
        n += 1;
    }
    copy_dir(root, &backup).context("making a backup")?;

    // 2. Definitions and images (the bundle rescale does images, frames, hitboxes, speeds).
    let mut b = Bundle::collect(&project.db, root, from, &project.config.name, &all_assets(&project.db))?;
    let originals = b.files.clone();
    b.rescale(TILE_SIZE)?;
    // Library art shrunken to this project's size → its original file.
    let mut lib_art: HashMap<Rgba, Vec<u8>> = HashMap::new();
    let mut lib_full: std::collections::HashSet<Rgba> = std::collections::HashSet::new();
    if let Some(lib) = library {
        let images = lib.db.tilesets.iter().map(|d| &d.image).chain(lib.db.sheets.iter().map(|d| &d.image));
        for rel in images {
            let Ok(bytes) = std::fs::read(lib.root.join(rel)) else { continue };
            let Ok(img) = Rgba::decode(&bytes) else { continue };
            let small = img.resized(((img.w as f32 / f).round() as u32).max(1), ((img.h as f32 / f).round() as u32).max(1));
            lib_art.insert(small, bytes);
            lib_full.insert(img);
        }
    }
    // Tilesets that already use full-size library art: keep the image, remap tile indices.
    // (tileset id → (columns before, columns after))
    let mut remap: HashMap<String, (u32, u32)> = HashMap::new();
    let k = (TILE_SIZE / from.max(1)).max(1);
    for t in &mut b.db.tilesets {
        let Some(img) = originals.get(&t.image).and_then(|bytes| Rgba::decode(bytes).ok()) else { continue };
        if !lib_full.contains(&img) || !TILE_SIZE.is_multiple_of(from) {
            continue;
        }
        let (old_cols, new_cols) = ((img.w / from).max(1), (img.w / TILE_SIZE).max(1));
        let map_idx = |i: u32| (i / old_cols / k) * new_cols + (i % old_cols) / k;
        let mut solid: Vec<u32> = t.solid.iter().map(|&i| map_idx(i)).collect();
        solid.sort_unstable();
        solid.dedup();
        t.solid = solid;
        for a in &mut t.animations {
            a.tile = map_idx(a.tile);
            a.frames = a.frames.iter().map(|&i| map_idx(i)).collect();
        }
        let mut seen = std::collections::HashSet::new();
        t.animations.retain(|a| seen.insert(a.tile));
        b.files.insert(t.image.clone(), originals[&t.image].clone());
        remap.insert(t.id.clone(), (old_cols, new_cols));
    }
    let kept: Vec<String> = b.db.tilesets.iter().filter(|t| remap.contains_key(&t.id)).map(|t| t.image.clone()).collect();
    let (mut restored, mut enlarged) = (Vec::new(), Vec::new());
    let images: Vec<String> = b.db.tilesets.iter().map(|d| d.image.clone()).chain(b.db.sheets.iter().map(|d| d.image.clone())).collect();
    for rel in images {
        if restored.contains(&rel) || enlarged.contains(&rel) || kept.contains(&rel) {
            continue;
        }
        let original = originals.get(&rel).and_then(|bytes| Rgba::decode(bytes).ok());
        match original.and_then(|img| lib_art.get(&img)) {
            Some(lib_bytes) => {
                b.files.insert(rel.clone(), lib_bytes.clone());
                restored.push(rel);
            }
            None => enlarged.push(rel),
        }
    }
    for (rel, bytes) in &b.files {
        std::fs::write(root.join(rel), bytes).with_context(|| format!("writing {rel}"))?;
    }
    project.db = b.db;

    // 3. Maps.
    let mut shrunk_maps = Vec::new();
    for m in project.list_maps() {
        let map = project.load_map(&m)?;
        let map = if painted_in_blocks(&map, &remap, k) {
            shrunk_maps.push(m.clone());
            shrink_map(&map, &remap, k, from)
        } else {
            enlarge_map(map, &remap, k, f)
        };
        project.save_map(&map)?;
    }

    // 4. Save games.
    if let Ok(rd) = std::fs::read_dir(root.join("saves")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "ron") {
                continue;
            }
            let Ok(mut w) = std::fs::read_to_string(&p).map_err(anyhow::Error::from).and_then(|s| WorldState::from_ron(&s)) else { continue };
            // Positions on re-gridded maps keep their pixel values (the world size is kept).
            if !shrunk_maps.contains(&w.current_map) {
                w.player_pos = (w.player_pos.0 * f, w.player_pos.1 * f);
            }
            for (map, states) in w.entities.iter_mut() {
                if shrunk_maps.contains(map) {
                    continue;
                }
                for st in states.values_mut() {
                    st.x *= f;
                    st.y *= f;
                }
            }
            std::fs::write(&p, w.to_ron()?)?;
        }
    }

    // 5. Config: same view of the world with the bigger tiles.
    project.config.tile_size = TILE_SIZE;
    project.config.scale = (project.config.scale / f).max(1.0);
    write_ron(&root.join("project.ron"), &project.config)?;
    project.save_defs()?;
    let remapped = remap.into_keys().collect();
    Ok(Some(Upgrade { from, backup, restored, enlarged, remapped, shrunk_maps }))
}

/// Index of the full tile a quarter tile was cut from.
fn full_index(i: u32, (old_cols, new_cols): (u32, u32), k: u32) -> u32 {
    (i / old_cols / k) * new_cols + (i % old_cols) / k
}

/// Was the map painted only with remapped tilesets, in whole tiles laid as aligned k×k blocks
/// of their quarters (as the editor's multi-tile stamp does)?
fn painted_in_blocks(map: &Map, remap: &HashMap<String, (u32, u32)>, k: u32) -> bool {
    if k < 2 {
        return false;
    }
    let (mut aligned, mut total) = (0usize, 0usize);
    for l in &map.layers {
        for y in 0..map.height {
            for x in 0..map.width {
                let Some((slot, i)) = l.get(x, y).parts() else { continue };
                let Some(&(old_cols, _)) = map.tilesets.get(slot).and_then(|id| remap.get(id)) else { return false };
                total += 1;
                if (i % old_cols) % k == x % k && (i / old_cols) % k == y % k {
                    aligned += 1;
                }
            }
        }
    }
    total > 0 && aligned * 10 >= total * 8
}

/// Same picture on a grid k times coarser: each k×k block becomes the whole tile it shows.
fn shrink_map(map: &Map, remap: &HashMap<String, (u32, u32)>, k: u32, from: u32) -> Map {
    let (w, h) = (map.width.div_ceil(k), map.height.div_ceil(k));
    let mut m = Map::new(&map.name, w, h);
    m.tilesets = map.tilesets.clone();
    m.layers = map
        .layers
        .iter()
        .map(|l| {
            let mut nl = TileLayer::new(&l.name, w, h, l.above_entities);
            nl.visible = l.visible;
            for ny in 0..h {
                for nx in 0..w {
                    // The block's cells, the one in the top-left corner first.
                    let cells = (0..k).flat_map(|dy| (0..k).map(move |dx| (nx * k + dx, ny * k + dy)));
                    let tile = cells.filter(|&(x, y)| x < map.width && y < map.height).find_map(|(x, y)| l.get(x, y).parts());
                    if let Some((slot, i)) = tile {
                        let i = map.tilesets.get(slot).and_then(|id| remap.get(id)).map_or(i, |c| full_index(i, *c, k));
                        nl.set(nx, ny, Tile::new(slot as u16, i));
                    }
                }
            }
            nl
        })
        .collect();
    for ny in 0..h {
        for nx in 0..w {
            let cells: Vec<CollisionOverride> = (0..k)
                .flat_map(|dy| (0..k).map(move |dx| (nx * k + dx, ny * k + dy)))
                .filter(|&(x, y)| x < map.width && y < map.height)
                .map(|(x, y)| map.collision_at(x, y))
                .collect();
            let c = if cells.contains(&CollisionOverride::Solid) {
                CollisionOverride::Solid
            } else if cells.contains(&CollisionOverride::Walkable) {
                CollisionOverride::Walkable
            } else {
                CollisionOverride::Auto
            };
            m.set_collision(nx, ny, c);
        }
    }
    // Entities keep their pixel position; ones snapped to the old grid snap to the new one.
    let t = TILE_SIZE as f32;
    let fr = from as f32;
    m.entities = map.entities.clone();
    for e in &mut m.entities {
        if (e.x.rem_euclid(fr) - fr / 2.0).abs() < 0.01 || e.x.rem_euclid(fr).abs() < 0.01 {
            e.x = if e.x.rem_euclid(t).abs() < 0.01 { e.x } else { ((e.x / t).floor() + 0.5) * t };
        }
        if ((e.y + 1.0).rem_euclid(fr)).abs() < 0.01 {
            e.y = ((e.y / t).floor() + 1.0) * t - 1.0;
        }
    }
    m.zones = map.zones.clone();
    for z in &mut m.zones {
        let (x1, y1) = ((z.x + z.w).div_ceil(k), (z.y + z.h).div_ceil(k));
        z.x /= k;
        z.y /= k;
        z.w = (x1 - z.x).max(1);
        z.h = (y1 - z.y).max(1);
    }
    m.edges = map.edges.clone();
    for e in Edge::ALL {
        if let Some(l) = m.edges.get_mut(e) {
            l.offset = (l.offset as f32 / k as f32).round() as i32;
            l.depth = l.depth.div_ceil(k).max(1);
        }
    }
    m.triggers = map.triggers.clone();
    m.next_uid = map.next_uid;
    m.background = map.background;
    m.music = map.music.clone();
    m
}

/// Same grid, everything twice (k times) as big: entity positions scale, tiles of kept
/// full-size tilesets point at the whole tile they were cut from.
fn enlarge_map(mut map: Map, remap: &HashMap<String, (u32, u32)>, k: u32, f: f32) -> Map {
    for e in &mut map.entities {
        e.x *= f;
        e.y *= f;
    }
    let slots: HashMap<usize, (u32, u32)> = map.tilesets.iter().enumerate().filter_map(|(i, id)| remap.get(id).map(|c| (i, *c))).collect();
    if !slots.is_empty() {
        for l in &mut map.layers {
            for y in 0..map.height {
                for x in 0..map.width {
                    if let Some((slot, i)) = l.get(x, y).parts() {
                        if let Some(&c) = slots.get(&slot) {
                            l.set(x, y, Tile::new(slot as u16, full_index(i, c, k)));
                        }
                    }
                }
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{Database, PropDef, SheetDef, SpriteRef};
    use crate::entity::{EntityInstance, EntityKind};
    use crate::map::Map;

    #[test]
    fn upgrades_16px_project() {
        let tmp = std::env::temp_dir().join(format!("rustpg2d_migrate_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        // A library with a 32px sheet, and a project holding a shrunken copy of it plus an
        // image of its own.
        let lib_root = tmp.join("lib");
        std::fs::create_dir_all(lib_root.join("sprites")).unwrap();
        let mut art = Rgba::new(4, 2);
        for x in 0..4 {
            art.set(x, 0, [x as u8 * 60, 0, 0, 255]);
            art.set(x, 1, [0, x as u8 * 60, 0, 255]);
        }
        std::fs::write(lib_root.join("sprites/hero.png"), art.encode().unwrap()).unwrap();
        // A 64x32 library tileset (2x1 tiles of 32 px), all pixels distinct.
        let mut terrain = Rgba::new(64, 32);
        for y in 0..32 {
            for x in 0..64 {
                terrain.set(x, y, [x as u8 * 4, y as u8 * 8, 7, 255]);
            }
        }
        std::fs::create_dir_all(lib_root.join("tilesets")).unwrap();
        std::fs::write(lib_root.join("tilesets/terrain.png"), terrain.encode().unwrap()).unwrap();
        let lib = Project {
            root: lib_root.clone(),
            config: ProjectConfig::new("lib"),
            db: Database {
                sheets: vec![SheetDef { id: "hero".into(), image: "sprites/hero.png".into(), frame_w: 32, frame_h: 32, kind: Default::default(), female: false, playable: false, tags: vec![] }],
                tilesets: vec![crate::defs::TilesetDef { id: "terrain".into(), image: "tilesets/terrain.png".into(), solid: vec![], animations: vec![], lights: vec![], terrains: vec![] }],
                ..Default::default()
            },
        };
        let root = tmp.join("old");
        std::fs::create_dir_all(root.join("sprites")).unwrap();
        std::fs::write(root.join("sprites/hero.png"), art.resized(2, 1).encode().unwrap()).unwrap();
        let mut own = Rgba::new(1, 1);
        own.set(0, 0, [1, 2, 3, 255]);
        std::fs::write(root.join("sprites/own.png"), own.encode().unwrap()).unwrap();
        // The full-size library tileset used with 16 px tiles: 4x2 quarter tiles.
        std::fs::create_dir_all(root.join("tilesets")).unwrap();
        std::fs::write(root.join("tilesets/big.png"), terrain.encode().unwrap()).unwrap();
        let db = Database {
            sheets: vec![
                SheetDef { id: "hero".into(), image: "sprites/hero.png".into(), frame_w: 16, frame_h: 16, kind: Default::default(), female: false, playable: false, tags: vec![] },
                SheetDef { id: "own".into(), image: "sprites/own.png".into(), frame_w: 16, frame_h: 16, kind: Default::default(), female: false, playable: false, tags: vec![] },
            ],
            tilesets: vec![crate::defs::TilesetDef { id: "big".into(), image: "tilesets/big.png".into(), solid: vec![3, 7], animations: vec![], lights: vec![], terrains: vec![] }],
            props: vec![PropDef { id: "p".into(), name: "P".into(), sprite: SpriteRef::new("own", 0), solid: true, hitbox: [12.0, 6.0], triggers: vec![], container: None, lock: None, default_state: "default".into(), states: vec![], light: None, use_spot: None, gather: None, regrow: None, plot: false }],
            ..Default::default()
        };
        let p = Project { root: root.clone(), config: ProjectConfig { tile_size: 16, scale: 3.0, ..ProjectConfig::new("Old") }, db };
        p.save_config().unwrap();
        p.save_defs().unwrap();
        let mut map = Map::new("start", 4, 4);
        map.entities.push(EntityInstance::new(1, EntityKind::Prop, "p", 24.0, 31.0));
        let slot = map.tileset_slot("big");
        // Quarter tiles 3 (top row, 4th) and 6 (bottom row, 3rd) both lie in full tile 1.
        map.layers[0].set(0, 0, crate::map::Tile::new(slot, 3));
        map.layers[0].set(1, 0, crate::map::Tile::new(slot, 6));
        map.layers[0].set(2, 0, crate::map::Tile::new(slot, 0));
        p.save_map(&map).unwrap();
        // A map painted with whole tiles as aligned 2x2 blocks: full tile 1 at the top-left,
        // full tile 0 next to it; a gate snapped to the 16 px grid; a 2x2 zone.
        let mut blocks = Map::new("blocks", 4, 4);
        let s = blocks.tileset_slot("big");
        for (x, y, i) in [(0, 0, 2), (1, 0, 3), (0, 1, 6), (1, 1, 7), (2, 0, 0), (3, 0, 1), (2, 1, 4), (3, 1, 5)] {
            blocks.layers[0].set(x, y, crate::map::Tile::new(s, i));
        }
        blocks.set_collision(3, 3, CollisionOverride::Solid);
        blocks.entities.push(EntityInstance::new(1, EntityKind::Prop, "p", 40.0, 47.0));
        blocks.zones.push(crate::map::Zone { uid: 2, name: "z".into(), x: 2, y: 2, w: 2, h: 2, kind: crate::map::ZoneKind::Spawn, triggers: vec![], loot: None });
        p.save_map(&blocks).unwrap();

        let up = upgrade(&root, Some(&lib)).unwrap().unwrap();
        assert_eq!(up.from, 16);
        assert_eq!(up.restored, vec!["sprites/hero.png".to_string()]);
        assert_eq!(up.enlarged, vec!["sprites/own.png".to_string()]);
        assert!(up.backup.join("project.ron").exists());
        let p = Project::load(&root).unwrap();
        assert_eq!((p.config.tile_size, p.config.scale), (32, 1.5));
        assert_eq!(p.db.sheet("hero").unwrap().frame_w, 32);
        assert_eq!(p.db.prop("p").unwrap().hitbox, [24.0, 12.0]);
        assert_eq!(Rgba::decode(&std::fs::read(root.join("sprites/hero.png")).unwrap()).unwrap(), art);
        assert_eq!(Rgba::decode(&std::fs::read(root.join("sprites/own.png")).unwrap()).unwrap().w, 2);
        assert_eq!(up.remapped, vec!["big".to_string()]);
        assert_eq!(Rgba::decode(&std::fs::read(root.join("tilesets/big.png")).unwrap()).unwrap(), terrain, "kept at full size");
        assert_eq!(p.db.tileset("big").unwrap().solid, vec![1]);
        let m = p.load_map("start").unwrap();
        let idx = |x| m.layers[0].get(x, 0).parts().unwrap().1;
        assert_eq!((idx(0), idx(1), idx(2)), (1, 1, 0));
        assert_eq!(up.shrunk_maps, vec!["blocks".to_string()]);
        let b = p.load_map("blocks").unwrap();
        assert_eq!((b.width, b.height), (2, 2));
        assert_eq!(b.layers[0].get(0, 0).parts().unwrap().1, 1);
        assert_eq!(b.layers[0].get(1, 0).parts().unwrap().1, 0);
        assert_eq!(b.collision_at(1, 1), CollisionOverride::Solid);
        assert_eq!((b.entities[0].x, b.entities[0].y), (48.0, 63.0));
        assert_eq!((b.zones[0].x, b.zones[0].y, b.zones[0].w, b.zones[0].h), (1, 1, 1, 1));
        let e = &p.load_map("start").unwrap().entities[0];
        assert_eq!((e.x, e.y), (48.0, 62.0));
        // Already converted: nothing to do.
        assert!(upgrade(&root, Some(&lib)).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
