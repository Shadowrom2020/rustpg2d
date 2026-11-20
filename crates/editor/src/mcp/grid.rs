//! Maps as text for agents: the region an agent asks for, the collision grid and the objects in
//! the region. Pure functions over the map data.

use rustpg2d_engine::map::{CollisionOverride, Map, ZoneKind};

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

/// The collision grid: `#` blocked, `.` free (ground, decorations), `S` / `W` where a
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
        ZoneKind::Ambience { sound, volume, fade } => format!("ambience {sound} {volume:.2} (fades over {fade} tiles)"),
        ZoneKind::Encounter { monsters, difficulty, respawn } => {
            let list: Vec<String> = monsters.iter().map(|m| format!("{} x{}", m.character, m.count)).collect();
            format!("encounter {} [{}] ({respawn:?})", difficulty.id(), list.join(", "))
        }
        ZoneKind::Trap { trap } => format!("trap {} (find DC {}, disarm DC {})", if trap.preset.is_empty() { "custom" } else { trap.preset.as_str() }, trap.detect_dc, trap.disarm_dc),
        ZoneKind::Drop { height_ft, target_map, target_spawn } => format!("drop {height_ft} ft -> {}/{}", if target_map.is_empty() { "(this map)" } else { target_map }, if target_spawn.is_empty() { "(beyond)" } else { target_spawn }),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_clipped_and_bounded() {
        assert_eq!(region(6, 3, None, None, None, None).unwrap(), Region { x: 0, y: 0, w: 6, h: 3 });
        assert!(region(6, 3, Some(6), None, None, None).is_err());
        assert!(region(1000, 1000, None, None, None, None).is_err(), "too big without a region");
        assert_eq!(region(1000, 1000, Some(990), Some(5), Some(50), Some(50)).unwrap(), Region { x: 990, y: 5, w: 10, h: 50 });
    }

    #[test]
    fn collision_grid_shows_overrides() {
        let mut m = Map::new("t", 6, 3);
        m.set_collision(2, 2, CollisionOverride::Solid);
        let r = region(6, 3, None, None, None, None).unwrap();
        let text = render_collision(&m, &r, &|x, y| y == 0 && x < 3);
        assert!(text.contains("    0 ###...\n    1 ......\n    2 ..S...\n"), "{text}");
    }
}
