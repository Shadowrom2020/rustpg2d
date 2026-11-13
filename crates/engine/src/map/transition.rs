//! Zones (portals, spawn points, trigger areas) and map-edge links.

use macroquad::math::{vec2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::events::Trigger;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ZoneKind {
    /// Named arrival point (`Zone::name` is the spawn name).
    Spawn,
    /// Entering the zone moves the player to `target_spawn` on `target_map`.
    Portal { target_map: String, target_spawn: String },
    /// Generic area firing `Enter` / `Leave` / `Interact` triggers.
    Trigger,
    /// Invisible area where up to `count` characters spawn (typically enemies).
    Spawner { character: String, count: u32, respawn: Respawn },
    /// One end of a two-way map transition: entering it moves the player next to the zone
    /// `target_zone` on `target_map` (its counterpart, which leads back here).
    Transition { target_map: String, target_zone: String },
    /// The water of a fishing spot: facing it from the shore, Interact casts a line onto it
    /// (the fishing minigame); fish shadows mark it. The zone's loot table is the catch
    /// (chances are weights: one catch per fish landed). `rod`: item needed ("" = none).
    FishingSpot { rod: String },
    /// A looping sound heard here: at `volume` (0..1) inside the zone, fading to nothing
    /// `fade` tiles away from it (birds over a meadow, a river, a tavern's murmur).
    Ambience { sound: String, volume: f32, fade: f32 },
}

/// When defeated spawner characters come back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Respawn {
    /// Defeated ones stay dead for good (saved).
    Never,
    /// The full group is back every time the map is entered.
    OnEnter,
    /// Each defeated one returns after `seconds` (and the group is full on map enter).
    After { seconds: f32 },
}

impl Respawn {
    pub fn label(&self) -> &'static str {
        match self {
            Respawn::Never => "Never",
            Respawn::OnEnter => "On map enter",
            Respawn::After { .. } => "After time",
        }
    }
}

impl ZoneKind {
    pub fn label(&self) -> &'static str {
        match self {
            ZoneKind::Spawn => "Spawn",
            ZoneKind::Portal { .. } => "Portal",
            ZoneKind::Trigger => "Trigger",
            ZoneKind::Spawner { .. } => "Spawner",
            ZoneKind::Transition { .. } => "Map transition",
            ZoneKind::FishingSpot { .. } => "Fishing spot",
            ZoneKind::Ambience { .. } => "Ambience",
        }
    }
}

/// The tile next to an area (x, y, w, h) where arriving players stand: on the side away from
/// the map edge the area touches, or else facing the map's center.
pub fn beside_tile(map_w: u32, map_h: u32, (x, y, w, h): (u32, u32, u32, u32)) -> (u32, u32) {
    let (mx, my) = (x + w / 2, y + h / 2);
    let side = if x == 0 {
        (1, 0)
    } else if x + w >= map_w {
        (-1, 0)
    } else if y == 0 {
        (0, 1)
    } else if y + h >= map_h {
        (0, -1)
    } else {
        let (dx, dy) = (map_w as i32 / 2 - mx as i32, map_h as i32 / 2 - my as i32);
        if dx.abs() > dy.abs() { (dx.signum(), 0) } else { (0, if dy == 0 { 1 } else { dy.signum() }) }
    };
    let (tx, ty) = match side {
        (1, _) => (x as i32 + w as i32, my as i32),
        (-1, _) => (x as i32 - 1, my as i32),
        (_, 1) => (mx as i32, y as i32 + h as i32),
        _ => (mx as i32, y as i32 - 1),
    };
    (tx.clamp(0, map_w as i32 - 1) as u32, ty.clamp(0, map_h as i32 - 1) as u32)
}

/// Rectangular area in tile coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub uid: u32,
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub kind: ZoneKind,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Spawners: loot table of the spawned characters (None = the character's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<Vec<crate::defs::LootEntry>>,
}

impl Zone {
    pub fn pixel_rect(&self, tile_size: f32) -> Rect {
        Rect::new(
            self.x as f32 * tile_size,
            self.y as f32 * tile_size,
            self.w as f32 * tile_size,
            self.h as f32 * tile_size,
        )
    }

    /// Where players arriving through a map transition are placed: the tile next to the zone
    /// on the side away from the map edge it touches (else facing the map's center), so they
    /// don't walk straight back through it. Bottom-center of that tile, in pixels.
    pub fn beside_point(&self, map_w: u32, map_h: u32, tile_size: f32) -> Vec2 {
        let (tx, ty) = beside_tile(map_w, map_h, (self.x, self.y, self.w, self.h));
        vec2((tx as f32 + 0.5) * tile_size, (ty as f32 + 1.0) * tile_size - 1.0)
    }

    /// Where actors arriving at this zone are placed (bottom-center of the zone's center tile).
    pub fn arrival_point(&self, tile_size: f32) -> Vec2 {
        let cx = self.x + self.w / 2;
        let cy = self.y + self.h / 2;
        vec2((cx as f32 + 0.5) * tile_size, (cy as f32 + 1.0) * tile_size - 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    North,
    South,
    East,
    West,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::North, Edge::South, Edge::East, Edge::West];
    pub fn label(self) -> &'static str {
        match self {
            Edge::North => "North",
            Edge::South => "South",
            Edge::East => "East",
            Edge::West => "West",
        }
    }
    pub fn opposite(self) -> Edge {
        match self {
            Edge::North => Edge::South,
            Edge::South => Edge::North,
            Edge::East => Edge::West,
            Edge::West => Edge::East,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeLink {
    pub target_map: String,
    /// Added (in tiles) to the coordinate along the edge when crossing.
    #[serde(default)]
    pub offset: i32,
    /// Thickness (in tiles) of the transition strip along the edge.
    #[serde(default = "one")]
    pub depth: u32,
}

fn one() -> u32 {
    1
}

impl EdgeLink {
    pub fn new(target_map: &str) -> Self {
        EdgeLink { target_map: target_map.to_string(), offset: 0, depth: 1 }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeLinks {
    pub north: Option<EdgeLink>,
    pub south: Option<EdgeLink>,
    pub east: Option<EdgeLink>,
    pub west: Option<EdgeLink>,
}

impl EdgeLinks {
    pub fn get(&self, e: Edge) -> Option<&EdgeLink> {
        match e {
            Edge::North => self.north.as_ref(),
            Edge::South => self.south.as_ref(),
            Edge::East => self.east.as_ref(),
            Edge::West => self.west.as_ref(),
        }
    }
    pub fn get_mut(&mut self, e: Edge) -> &mut Option<EdgeLink> {
        match e {
            Edge::North => &mut self.north,
            Edge::South => &mut self.south,
            Edge::East => &mut self.east,
            Edge::West => &mut self.west,
        }
    }

    /// Pixel rect of the transition strip for an edge, if linked.
    pub fn strip_rect(&self, e: Edge, map_w: u32, map_h: u32, ts: f32) -> Option<Rect> {
        let d = self.get(e)?.depth.max(1) as f32 * ts;
        let (w, h) = (map_w as f32 * ts, map_h as f32 * ts);
        Some(match e {
            Edge::North => Rect::new(0.0, 0.0, w, d),
            Edge::South => Rect::new(0.0, h - d, w, d),
            Edge::West => Rect::new(0.0, 0.0, d, h),
            Edge::East => Rect::new(w - d, 0.0, d, h),
        })
    }

    /// The linked edge the actor at `pos` is crossing while moving with `dir`, if any.
    pub fn crossing(&self, pos: Vec2, dir: Vec2, map_w: u32, map_h: u32, ts: f32) -> Option<Edge> {
        Edge::ALL.into_iter().find(|&e| {
            let moving_out = match e {
                Edge::North => dir.y < 0.0,
                Edge::South => dir.y > 0.0,
                Edge::West => dir.x < 0.0,
                Edge::East => dir.x > 0.0,
            };
            moving_out
                && self
                    .strip_rect(e, map_w, map_h, ts)
                    .is_some_and(|r| r.contains(pos))
        })
    }
}

/// Where an actor leaving through `edge` at `pos` arrives on the destination map.
/// The coordinate along the edge is kept (plus `link.offset` tiles); the actor is
/// placed just inside the opposite edge, past the destination's transition strip.
pub fn edge_destination(
    edge: Edge,
    pos: Vec2,
    link: &EdgeLink,
    dst_w: u32,
    dst_h: u32,
    dst_links: &EdgeLinks,
    ts: f32,
) -> Vec2 {
    let back_depth = dst_links.get(edge.opposite()).map_or(0, |l| l.depth) as f32;
    let inset = (back_depth + 0.5) * ts;
    let (w, h) = (dst_w as f32 * ts, dst_h as f32 * ts);
    let off = link.offset as f32 * ts;
    let clamp_x = |x: f32| x.clamp(ts * 0.5, w - ts * 0.5);
    let clamp_y = |y: f32| y.clamp(ts, h - 1.0);
    match edge {
        Edge::North => vec2(clamp_x(pos.x + off), clamp_y(h - inset)),
        Edge::South => vec2(clamp_x(pos.x + off), clamp_y(inset + ts * 0.5)),
        Edge::West => vec2(w - inset, clamp_y(pos.y + off)),
        Edge::East => vec2(inset, clamp_y(pos.y + off)),
    }
}

#[cfg(test)]
mod beside_tests {
    use super::*;

    #[test]
    fn arrival_is_next_to_the_zone_inside_the_map() {
        assert_eq!(beside_tile(40, 30, (39, 14, 1, 3)), (38, 15));
        assert_eq!(beside_tile(40, 30, (18, 0, 3, 1)), (19, 1));
        assert_eq!(beside_tile(40, 30, (5, 14, 2, 2)), (7, 15));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_requires_outward_movement() {
        let links = EdgeLinks { east: Some(EdgeLink::new("forest")), ..Default::default() };
        let p = vec2(10.0 * 16.0 - 4.0, 50.0);
        assert_eq!(links.crossing(p, vec2(1.0, 0.0), 10, 10, 16.0), Some(Edge::East));
        assert_eq!(links.crossing(p, vec2(-1.0, 0.0), 10, 10, 16.0), None);
        assert_eq!(links.crossing(p, vec2(0.0, -1.0), 10, 10, 16.0), None);
        // Unlinked edge never triggers.
        assert_eq!(links.crossing(vec2(4.0, 50.0), vec2(-1.0, 0.0), 10, 10, 16.0), None);
    }

    #[test]
    fn destination_keeps_parallel_coordinate() {
        let ts = 16.0;
        let mut link = EdgeLink::new("forest");
        let dst = EdgeLinks {
            west: Some(EdgeLink { target_map: "village".into(), offset: 0, depth: 2 }),
            ..Default::default()
        };
        // Leaving east at y=100 lands on the west side of the destination, past its 2-tile strip.
        let d = edge_destination(Edge::East, vec2(159.0, 100.0), &link, 20, 20, &dst, ts);
        assert_eq!(d, vec2(2.5 * ts, 100.0));
        assert!(dst.crossing(d, vec2(-1.0, 0.0), 20, 20, ts).is_none());
        // Offset shifts along the edge, and results are clamped to the destination map.
        link.offset = 3;
        let d = edge_destination(Edge::North, vec2(40.0, 2.0), &link, 4, 8, &EdgeLinks::default(), ts);
        assert_eq!(d.x, 4.0 * ts - ts * 0.5);
        assert_eq!(d.y, 8.0 * ts - 0.5 * ts);
    }
}
