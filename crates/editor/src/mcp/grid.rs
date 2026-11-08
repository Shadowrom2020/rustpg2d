//! Maps as text for agents: a region of a tile layer as rows of glyphs with a legend (the most
//! frequent tile gets `a`, …), the collision grid, the objects in the region — and the reverse,
//! parsing such rows for `paint_grid`. Pure functions over the map data.

use std::collections::{BTreeMap, HashMap};

use rustpg2d_engine::map::{CollisionOverride, Map, Tile, ZoneKind};

/// Glyphs in the order they are handed out (`.` = empty, `#` = more kinds than glyphs).
pub const GLYPHS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!$%&*+-/:;<=>?@^~";
/// Largest region per call (tiles).
pub const MAX_CELLS: u32 = 80 * 50;

/// A clipped region of a map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The region an agent asked for, clipped to the map; without one, the whole map when it fits.
pub fn region(map_w: u32, map_h: u32, x: Option<u32>, y: Option<u32>, w: Option<u32>, h: Option<u32>) -> Result<Region, String> {
    let (x, y) = (x.unwrap_or(0), y.unwrap_or(0));
    if x >= map_w || y >= map_h {
        return Err(format!("{x},{y} is outside the map ({map_w}x{map_h})"));
    }
    let w = w.unwrap_or(map_w - x).min(map_w - x);
    let h = h.unwrap_or(map_h - y).min(map_h - y);
    if w == 0 || h == 0 {
        return Err("the region is empty".into());
    }
    if w * h > MAX_CELLS {
        return Err(format!("the region {w}x{h} has more than {MAX_CELLS} tiles: ask for a part (e.g. w 80, h 50) or several calls"));
    }
    Ok(Region { x, y, w, h })
}

/// Column rulers (tens and units) and the row prefix width used by every grid.
fn rulers(r: &Region) -> String {
    let mut tens = String::from("      ");
    let mut units = String::from("      ");
    for x in r.x..r.x + r.w {
        tens.push(if x % 10 == 0 { char::from_digit(x / 10 % 10, 10).unwrap() } else { ' ' });
        units.push(char::from_digit(x % 10, 10).unwrap());
    }
    format!("{tens}\n{units}\n")
}

/// One layer of `map` in `r` as glyph rows plus the legend; `describe(tileset id, index)` names a
/// tile (terrain, solid, …) for the legend.
pub fn render_layer(map: &Map, layer: usize, r: &Region, describe: &dyn Fn(&str, u32) -> String) -> String {
    let Some(l) = map.layers.get(layer) else { return format!("layer {layer}: the map has {} layers\n", map.layers.len()) };
    // Frequency per tile, then glyphs by frequency.
    let mut counts: HashMap<Tile, usize> = HashMap::new();
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            let t = l.get(x, y);
            if t != Tile::EMPTY {
                *counts.entry(t).or_default() += 1;
            }
        }
    }
    let mut order: Vec<(Tile, usize)> = counts.into_iter().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0 .0.cmp(&b.0 .0)));
    let glyphs: Vec<char> = GLYPHS.chars().collect();
    let mut glyph_of: HashMap<Tile, char> = HashMap::new();
    let mut legend = Vec::new();
    let mut overflow = 0;
    for (i, (t, n)) in order.iter().enumerate() {
        if let Some(&g) = glyphs.get(i) {
            glyph_of.insert(*t, g);
            let (slot, idx) = t.parts().unwrap_or((0, 0));
            let ts = map.tilesets.get(slot).map_or("?", |s| s.as_str());
            legend.push(format!("{g} = {ts}#{idx} {} x{n}", describe(ts, idx)));
        } else {
            overflow += n;
        }
    }
    let mut out = format!("layer {layer} \"{}\"{} x{}..{} y{}..{} (. = empty{}):\n", l.name, if l.above_entities { " (above characters)" } else { "" }, r.x, r.x + r.w - 1, r.y, r.y + r.h - 1, if overflow > 0 { ", # = other tiles" } else { "" });
    out += &rulers(r);
    for y in r.y..r.y + r.h {
        out += &format!("{y:>5} ");
        for x in r.x..r.x + r.w {
            let t = l.get(x, y);
            out.push(if t == Tile::EMPTY { '.' } else { *glyph_of.get(&t).unwrap_or(&'#') });
        }
        out.push('\n');
    }
    if legend.is_empty() {
        out += "legend: (the region is empty on this layer)\n";
    } else {
        out += &format!("legend: {}\n", legend.join(" · "));
        if overflow > 0 {
            out += &format!("        # = {overflow} tiles of other kinds (ask for a smaller region)\n");
        }
    }
    out
}

/// The collision grid: `#` blocked, `.` free (from the tiles and overrides), `S` / `W` where a
/// Solid / Walkable override is set.
pub fn render_collision(map: &Map, r: &Region, solid: &dyn Fn(u32, u32) -> bool) -> String {
    let mut out = format!("collision x{}..{} y{}..{} (# blocked, . free, S/W = solid/walkable override):\n", r.x, r.x + r.w - 1, r.y, r.y + r.h - 1);
    out += &rulers(r);
    for y in r.y..r.y + r.h {
        out += &format!("{y:>5} ");
        for x in r.x..r.x + r.w {
            out.push(match map.collision_at(x, y) {
                CollisionOverride::Solid => 'S',
                CollisionOverride::Walkable => 'W',
                CollisionOverride::Auto => {
                    if solid(x, y) {
                        '#'
                    } else {
                        '.'
                    }
                }
            });
        }
        out.push('\n');
    }
    out
}

/// Short text for a zone's kind.
pub fn zone_kind(k: &ZoneKind) -> String {
    match k {
        ZoneKind::Spawn => "spawn".into(),
        ZoneKind::Portal { target_map, target_spawn } => format!("portal -> {target_map}/{target_spawn}"),
        ZoneKind::Trigger => "trigger".into(),
        ZoneKind::Spawner { character, count, respawn } => format!("spawner {character} x{count} ({respawn:?})"),
        ZoneKind::Transition { target_map, target_zone } => format!("transition -> {target_map}/{target_zone}"),
        ZoneKind::FishingSpot { rod } => format!("fishing spot (rod {})", if rod.is_empty() { "none" } else { rod }),
    }
}

/// The entities standing in `r` and the zones touching it, one line each.
pub fn render_objects(map: &Map, r: &Region, ts: f32) -> String {
    let mut out = String::new();
    let inside = |tx: i32, ty: i32| tx >= r.x as i32 && ty >= r.y as i32 && tx < (r.x + r.w) as i32 && ty < (r.y + r.h) as i32;
    let mut ents: Vec<&_> = map.entities.iter().filter(|e| inside((e.x / ts).floor() as i32, (e.y / ts).floor() as i32)).collect();
    ents.sort_by_key(|e| e.uid);
    for e in ents {
        let (tx, ty) = ((e.x / ts).floor() as i32, (e.y / ts).floor() as i32);
        out += &format!(
            "E {} {:?} {}{} @{tx},{ty} facing {:?}{}{}{}\n",
            e.uid,
            e.kind,
            e.def,
            if e.name.is_empty() { String::new() } else { format!(" \"{}\"", e.name) },
            e.facing,
            if e.state.is_empty() { String::new() } else { format!(" state {}", e.state) },
            if e.triggers.is_empty() { String::new() } else { format!(" triggers {}", e.triggers.len()) },
            if e.active { "" } else { " inactive" }
        );
    }
    let mut zones: Vec<&_> = map.zones.iter().filter(|z| z.x < r.x + r.w && z.x + z.w > r.x && z.y < r.y + r.h && z.y + z.h > r.y).collect();
    zones.sort_by_key(|z| z.uid);
    for z in zones {
        out += &format!("Z {} \"{}\" {} @{},{} {}x{}{}\n", z.uid, z.name, zone_kind(&z.kind), z.x, z.y, z.w, z.h, if z.triggers.is_empty() { String::new() } else { format!(" triggers {}", z.triggers.len()) });
    }
    if out.is_empty() {
        out = "objects: none in the region\n".into();
    } else {
        out = format!("objects (E = entity uid kind def, Z = zone uid name kind):\n{out}");
    }
    out
}

/// Rows of glyphs → cells `(dx, dy, tile index or None = erase)`; `.` is skipped.
pub fn parse_grid(rows: &[String], legend: &BTreeMap<String, i64>) -> Result<Vec<(u32, u32, Option<u32>)>, String> {
    let mut by_glyph: HashMap<char, Option<u32>> = HashMap::new();
    for (k, v) in legend {
        let mut chars = k.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else { return Err(format!("legend key \"{k}\" must be one character")) };
        if c == '.' {
            return Err("'.' is reserved for 'leave the cell as it is'".into());
        }
        by_glyph.insert(c, if *v < 0 { None } else { Some(*v as u32) });
    }
    let mut cells = Vec::new();
    for (dy, row) in rows.iter().enumerate() {
        for (dx, c) in row.chars().enumerate() {
            if c == '.' || c == ' ' {
                continue;
            }
            match by_glyph.get(&c) {
                Some(t) => cells.push((dx as u32, dy as u32, *t)),
                None => return Err(format!("row {dy} has '{c}', which the legend doesn't define")),
            }
        }
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Map {
        let mut m = Map::new("t", 6, 3);
        let slot = m.tileset_slot("terrain");
        for x in 0..6 {
            m.layers[0].set(x, 0, Tile::new(slot, 1));
        }
        m.layers[0].set(0, 1, Tile::new(slot, 5));
        m.layers[0].set(1, 1, Tile::new(slot, 5));
        m.layers[0].set(5, 2, Tile::new(slot, 9));
        m
    }

    #[test]
    fn grids_name_tiles_by_frequency_and_round_trip() {
        let m = map();
        let r = region(6, 3, None, None, None, None).unwrap();
        let text = render_layer(&m, 0, &r, &|_, i| format!("t{i}"));
        assert!(text.contains("    0 aaaaaa\n    1 bb....\n    2 .....c\n"), "{text}");
        assert!(text.contains("a = terrain#1 t1 x6 · b = terrain#5 t5 x2 · c = terrain#9 t9 x1"), "{text}");
        let legend: BTreeMap<String, i64> = [("a".to_string(), 1), ("b".to_string(), 5), ("x".to_string(), -1)].into_iter().collect();
        let cells = parse_grid(&["ab.".into(), "x".into()], &legend).unwrap();
        assert_eq!(cells, vec![(0, 0, Some(1)), (1, 0, Some(5)), (0, 1, None)]);
        assert!(parse_grid(&["q".into()], &legend).is_err());
        assert!(region(6, 3, Some(6), None, None, None).is_err());
        assert!(region(1000, 1000, None, None, None, None).is_err(), "too big without a region");
        assert_eq!(region(1000, 1000, Some(990), Some(5), Some(50), Some(50)).unwrap(), Region { x: 990, y: 5, w: 10, h: 50 });
    }

    #[test]
    fn collision_grid_shows_overrides() {
        let mut m = map();
        m.set_collision(2, 2, CollisionOverride::Solid);
        let r = region(6, 3, None, None, None, None).unwrap();
        let text = render_collision(&m, &r, &|x, y| y == 0 && x < 3);
        assert!(text.contains("    0 ###...\n    1 ......\n    2 ..S...\n"), "{text}");
    }
}
