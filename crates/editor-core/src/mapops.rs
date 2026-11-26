//! Edits of maps that tools and agents share: transitions, lines of props, strokes of ground.

use gfx::math::{vec2, Vec2};
use rustpg2d_engine::decor::{Decor, DecorLayer};
use rustpg2d_engine::ground::Ground;
use rustpg2d_engine::map::{Map, Zone, ZoneKind};

/// Points the transition zone `zone` on `map` at `target_map`:`target_zone`.
pub fn set_target(map: &mut Map, zone: &str, target_map: &str, target_zone: &str) -> bool {
    match map.zones.iter_mut().find(|z| z.name == zone) {
        Some(z) => {
            z.kind = ZoneKind::Transition { target_map: target_map.into(), target_zone: target_zone.into() };
            true
        }
        None => false,
    }
}

/// Adds a transition zone to `map` leading to `target_map`:`target_zone`. Returns its name.
pub fn add_transition(map: &mut Map, rect: (u32, u32, u32, u32), target_map: &str, target_zone: &str) -> String {
    let name = unique_zone_name(map, &format!("to_{target_map}"));
    let uid = map.alloc_uid();
    map.zones.push(Zone {
        uid,
        name: name.clone(),
        x: rect.0,
        y: rect.1,
        w: rect.2,
        h: rect.3,
        kind: ZoneKind::Transition { target_map: target_map.into(), target_zone: target_zone.into() },
        triggers: vec![],
        loot: None,
    });
    name
}

/// A zone name not used on `map` yet.
pub fn unique_zone_name(map: &Map, base: &str) -> String {
    rustpg2d_engine::refactor::unique_id(base, |c| map.zones.iter().any(|z| z.name == c))
}

/// The grid point (feet) of a spot for a prop that snaps to `s` pixels: the middle of the cell's
/// bottom edge.
pub fn snap_point(p: Vec2, s: f32) -> Vec2 {
    vec2((p.x / s).floor() * s + s / 2.0, (p.y / s).floor() * s + s)
}

/// Places a line of prop `prop` from `a` to `b` (along the longer axis) on its grid (its *Snap
/// to*, else a tile), skipping spots it stands on already; returns (added, removed).
pub fn wall_line(map: &mut Map, db: &rustpg2d_engine::defs::Database, prop: &str, a: Vec2, b: Vec2, layer: DecorLayer) -> (Vec<Decor>, Vec<Decor>) {
    let s = db.prop(prop).map_or(32, |d| if d.snap == 0 { 32 } else { d.snap }) as f32;
    let (a, b) = (snap_point(a, s), snap_point(b, s));
    let b = if (b.x - a.x).abs() >= (b.y - a.y).abs() { vec2(b.x, a.y) } else { vec2(a.x, b.y) };
    let n = (((b - a).length() / s).round() as i32).max(0);
    let dir = if n == 0 { Vec2::ZERO } else { (b - a) / n as f32 };
    let slot = map.decor_slot(prop);
    let mut added = Vec::new();
    for k in 0..=n {
        let p = a + dir * k as f32;
        let (x, y) = (p.x.round() as i32, p.y.round() as i32);
        if map.decor.iter().any(|d| d.x == x && d.y == y && d.def == slot) {
            continue;
        }
        let d = Decor { def: slot, x, y, flip: false, state: 0, layer };
        map.decor.push(d);
        added.push(d);
    }
    (added, Vec::new())
}

/// For `Ground` users outside this module (the agent tools): a stroke through several points.
pub fn paint_polyline(g: &mut Ground, points: &[(f32, f32)], width: f32, v: u8) -> Vec<(i32, i32, u8)> {
    let mut cells = Vec::new();
    match points {
        [] => {}
        [p] => cells.extend(g.paint_circle(*p, width / 2.0, v)),
        _ => {
            for w in points.windows(2) {
                cells.extend(g.paint_line(w[0], w[1], width, v));
            }
        }
    }
    cells
}

