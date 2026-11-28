//! Decorations: trees, rocks, flowers, fences, walls and buildings placed freely on a map (pixel
//! positions, not tiles). A decoration is a picture of an ordinary prop definition with that
//! prop's hitbox, solidity and light, but without uid, state changes or triggers, so a map can
//! hold tens of thousands of them cheaply; a prop that does something (a choppable tree, a door)
//! is placed as an entity instead.
//!
//! Scatter sets ([`ScatterDef`], `defs/scatter.ron`) place many at once (a forest, flowers in a
//! meadow); wall sets (`PropDef::pieces`) pick their pieces by the neighbours, like auto-tiling.

use serde::{Deserialize, Serialize};

use crate::defs::Database;

/// Where a decoration is drawn relative to characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum DecorLayer {
    /// On the ground, under everyone (rugs, puddles, flowers lying flat).
    Flat,
    /// Sorted with characters by its feet (trees, rocks, walls, houses).
    #[default]
    Sorted,
    /// Over everyone (treetops, roofs, arches).
    Above,
}

impl DecorLayer {
    pub const ALL: [DecorLayer; 3] = [DecorLayer::Flat, DecorLayer::Sorted, DecorLayer::Above];
    pub fn label(self) -> &'static str {
        match self {
            DecorLayer::Flat => "Flat",
            DecorLayer::Sorted => "Sorted",
            DecorLayer::Above => "Above",
        }
    }
    pub fn parse(s: &str) -> Option<DecorLayer> {
        DecorLayer::ALL.into_iter().find(|l| l.label().eq_ignore_ascii_case(s))
    }
}

/// One decoration on a map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Decor {
    /// Index into the map's `decor_defs` (prop ids).
    pub def: u16,
    /// Feet position in world pixels (the sprite's bottom middle).
    pub x: i32,
    pub y: i32,
    /// Mirrored left to right.
    pub flip: bool,
    /// The prop state whose picture it shows (0 = the default state).
    pub state: u8,
    pub layer: DecorLayer,
}

impl Decor {
    /// The two high bits of [`Decor::state`] hold a quarter turn (props that turn, see
    /// [`crate::defs::PropDef::turns`]); the map format stays as it was, and older versions read
    /// a turned decoration's state as out of range (the default state).
    pub const TURN_SHIFT: u8 = 6;

    /// The prop state it shows (0 = the default state).
    pub fn state_index(&self) -> usize {
        (self.state & ((1 << Self::TURN_SHIFT) - 1)) as usize
    }

    /// Quarter turns from facing south (0..3: south, west, north, east).
    pub fn turn(&self) -> u8 {
        self.state >> Self::TURN_SHIFT
    }

    /// Sets the quarter turns (0..3), keeping the state.
    pub fn set_turn(&mut self, turn: u8) {
        self.state = (self.state & ((1 << Self::TURN_SHIFT) - 1)) | ((turn & 3) << Self::TURN_SHIFT);
    }

    /// The way it faces (turned decorations of props that turn).
    pub fn facing(&self) -> crate::entity::Facing {
        turn_facing(self.turn())
    }
}

/// The facing of `turn` quarter turns: south, west, north, east.
pub fn turn_facing(turn: u8) -> crate::entity::Facing {
    use crate::entity::Facing;
    [Facing::Down, Facing::Left, Facing::Up, Facing::Right][(turn & 3) as usize]
}

/// The quarter turns of facing `f` ([`turn_facing`]'s inverse).
pub fn facing_turn(f: crate::entity::Facing) -> u8 {
    use crate::entity::Facing;
    match f {
        Facing::Down => 0,
        Facing::Left => 1,
        Facing::Up => 2,
        Facing::Right => 3,
    }
}

/// A scatter set: props placed at random by the scatter brush (a forest, a flower meadow).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct ScatterDef {
    pub id: String,
    pub name: String,
    /// Prop ids and how often each comes (weights).
    pub props: Vec<(String, u32)>,
    /// Decorations per tile at full strength (0.05 = a sparse wood, 0.5 = flowers).
    pub density: f32,
    /// The least distance between two of them, in pixels.
    pub spacing: f32,
    /// Mirror half of them at random.
    pub flip: bool,
    pub layer: DecorLayer,
}

impl Default for ScatterDef {
    fn default() -> Self {
        ScatterDef { id: String::new(), name: String::new(), props: Vec::new(), density: 0.15, spacing: 24.0, flip: true, layer: DecorLayer::Sorted }
    }
}

impl ScatterDef {
    pub fn fresh(id: &str) -> Self {
        ScatterDef { id: id.to_string(), name: "New scatter set".into(), ..Default::default() }
    }

    /// Points for a brush of `radius` px at `center`: about `density` per tile, at least
    /// `spacing` apart from each other and from `existing` (positions already on the map), each
    /// with its prop (weighted) and a flip. Deterministic for a `seed`.
    pub fn scatter(&self, center: (f32, f32), radius: f32, existing: &[(f32, f32)], seed: u64) -> Vec<(String, f32, f32, bool)> {
        let total: u32 = self.props.iter().map(|p| p.1).sum();
        if total == 0 || radius <= 0.0 {
            return Vec::new();
        }
        let ts = crate::project::TILE_SIZE as f32;
        let area = std::f32::consts::PI * radius * radius / (ts * ts);
        let want = (area * self.density.max(0.0)).round() as usize;
        let mut rng = Rng(seed | 1);
        let mut placed: Vec<(f32, f32)> = Vec::new();
        let mut out = Vec::new();
        let min2 = self.spacing.max(0.0).powi(2);
        // Darts: try a few times as many points as wanted, keep those far enough apart.
        for _ in 0..want * 8 {
            if out.len() >= want {
                break;
            }
            let (a, d) = (rng.f() * std::f32::consts::TAU, rng.f().sqrt() * radius);
            let p = (center.0 + a.cos() * d, center.1 + a.sin() * d);
            let far = |q: &(f32, f32)| (q.0 - p.0).powi(2) + (q.1 - p.1).powi(2) >= min2;
            if !placed.iter().all(far) || !existing.iter().all(far) {
                continue;
            }
            let mut pick = (rng.f() * total as f32) as u32;
            let prop = self.props.iter().find(|(_, w)| {
                if pick < *w {
                    true
                } else {
                    pick -= w;
                    false
                }
            });
            let Some((prop, _)) = prop else { continue };
            placed.push(p);
            out.push((prop.clone(), p.0.round(), p.1.round(), self.flip && rng.f() < 0.5));
        }
        out
    }
}

/// A small xorshift generator (scatter positions; no dependency).
struct Rng(u64);

impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The pieces of a wall set by neighbour mask: bit 1 = a piece of the set north, 2 = east, 4 =
/// south, 8 = west (`PropDef::pieces[mask]`; empty = the set's own prop).
pub const NORTH: usize = 1;
pub const EAST: usize = 2;
pub const SOUTH: usize = 4;
pub const WEST: usize = 8;

/// What a map's decorations do, by `decor_defs` slot: built when a map is entered (and by the
/// editor) from the prop definitions.
#[derive(Clone, Debug, Default)]
pub struct DecorLook {
    /// Hitbox size (w, h) in pixels, bottom-centred at the feet.
    pub hitbox: [f32; 2],
    pub solid: bool,
    pub low: bool,
    pub climb: bool,
    /// The Athletics DC to climb it (0 = a ladder: none).
    pub climb_dc: u32,
    /// Difficult terrain / lightly obscuring where it stands (fights).
    pub difficult: bool,
    pub obscures: bool,
    /// The prop turns in quarters ([`crate::defs::PropDef::turns`]): turned decorations swap the
    /// hitbox's sides.
    pub turns: bool,
}

impl DecorLook {
    pub fn of(db: &Database, id: &str, state: u8) -> DecorLook {
        match db.prop(id) {
            Some(d) => DecorLook { hitbox: d.hitbox, solid: d.state_look(state as usize).1, low: d.low, climb: d.climb, climb_dc: d.climb_dc, difficult: d.difficult, obscures: d.obscures, turns: d.turns() },
            None => DecorLook::default(),
        }
    }

    /// The hitbox at feet (x, y), as (x, y, w, h).
    pub fn rect(&self, x: f32, y: f32) -> (f32, f32, f32, f32) {
        (x - self.hitbox[0] / 2.0, y - self.hitbox[1], self.hitbox[0], self.hitbox[1])
    }

    /// The hitbox of decoration `d` (turned a quarter: its sides swapped), as (x, y, w, h).
    pub fn rect_at(&self, d: &Decor) -> (f32, f32, f32, f32) {
        let [w, h] = if self.turns && d.turn() % 2 == 1 { [self.hitbox[1], self.hitbox[0]] } else { self.hitbox };
        (d.x as f32 - w / 2.0, d.y as f32 - h, w, h)
    }
}

/// Marks the tiles solid decorations of `map` cover in `solid` (one entry per tile): those whose
/// inner square (2 px in) a hitbox overlaps, as route planning counts them.
pub fn mark_solid_tiles(map: &crate::map::Map, db: &Database, solid: &mut [bool]) {
    let ts = crate::project::TILE_SIZE as f32;
    let looks: Vec<DecorLook> = map.decor_defs.iter().map(|id| DecorLook::of(db, id, 0)).collect();
    for d in &map.decor {
        let Some(l) = looks.get(d.def as usize).filter(|l| l.solid) else { continue };
        let (x, y, w, h) = l.rect_at(d);
        for ty in (y / ts).floor() as i32..=((y + h) / ts).floor() as i32 {
            for tx in (x / ts).floor() as i32..=((x + w) / ts).floor() as i32 {
                let (ix, iy) = (tx as f32 * ts + 2.0, ty as f32 * ts + 2.0);
                let overlaps = x < ix + ts - 4.0 && x + w > ix && y < iy + ts - 4.0 && y + h > iy;
                if overlaps && map.in_bounds(tx, ty) {
                    solid[(ty as u32 * map.width + tx as u32) as usize] = true;
                }
            }
        }
    }
}

/// The decorations of a map by chunk (`map::CHUNK_SIZE` tiles), for drawing what is on screen
/// and testing collision near a point without looking at all of them.
#[derive(Clone, Debug, Default)]
pub struct DecorIndex {
    chunks_w: u32,
    chunks_h: u32,
    /// Decoration indices per chunk (by their feet).
    cells: Vec<Vec<u32>>,
}

/// How far a sprite may reach out of its feet's chunk (pixels): drawing looks this far around.
pub const REACH: f32 = 256.0;

impl DecorIndex {
    pub fn build(decor: &[Decor], width: u32, height: u32) -> Self {
        let span = (crate::map::CHUNK_SIZE * crate::project::TILE_SIZE) as i32;
        let (cw, ch) = (width.div_ceil(crate::map::CHUNK_SIZE).max(1), height.div_ceil(crate::map::CHUNK_SIZE).max(1));
        let mut cells = vec![Vec::new(); (cw * ch) as usize];
        for (i, d) in decor.iter().enumerate() {
            let cx = (d.x.div_euclid(span)).clamp(0, cw as i32 - 1) as u32;
            let cy = (d.y.div_euclid(span)).clamp(0, ch as i32 - 1) as u32;
            cells[(cy * cw + cx) as usize].push(i as u32);
        }
        DecorIndex { chunks_w: cw, chunks_h: ch, cells }
    }

    /// Indices of the decorations whose feet lie in the chunks overlapping the rectangle
    /// (world pixels) grown by `margin`.
    pub fn near(&self, x: f32, y: f32, w: f32, h: f32, margin: f32) -> impl Iterator<Item = u32> + '_ {
        let span = (crate::map::CHUNK_SIZE * crate::project::TILE_SIZE) as f32;
        let cx0 = ((x - margin) / span).floor().max(0.0) as u32;
        let cy0 = ((y - margin) / span).floor().max(0.0) as u32;
        let cx1 = (((x + w + margin) / span).floor().max(0.0) as u32).min(self.chunks_w.saturating_sub(1));
        let cy1 = (((y + h + margin) / span).floor().max(0.0) as u32).min(self.chunks_h.saturating_sub(1));
        (cy0..=cy1).flat_map(move |cy| (cx0..=cx1).map(move |cx| (cy * self.chunks_w + cx) as usize)).filter_map(|i| self.cells.get(i)).flatten().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_decoration_keeps_its_state_and_turns_its_hitbox() {
        let mut d = Decor { def: 0, x: 100, y: 200, flip: false, state: 2, layer: DecorLayer::Sorted };
        d.set_turn(1);
        assert_eq!((d.state_index(), d.turn(), d.facing()), (2, 1, crate::entity::Facing::Left));
        d.set_turn(6);
        assert_eq!((d.state_index(), d.turn()), (2, 2));
        let look = DecorLook { hitbox: [64.0, 32.0], turns: true, ..Default::default() };
        assert_eq!(look.rect_at(&d), (68.0, 168.0, 64.0, 32.0));
        d.set_turn(3);
        assert_eq!(look.rect_at(&d), (84.0, 136.0, 32.0, 64.0));
        // Props that don't turn keep their hitbox whatever the bits say.
        assert_eq!(DecorLook { turns: false, ..look }.rect_at(&d), (68.0, 168.0, 64.0, 32.0));
        for t in 0..4 {
            assert_eq!(facing_turn(turn_facing(t)), t);
        }
    }

    #[test]
    fn scattering_keeps_its_spacing_and_density() {
        let s = ScatterDef { props: vec![("oak".into(), 3), ("pine".into(), 1)], density: 0.5, spacing: 20.0, ..ScatterDef::fresh("wood") };
        let pts = s.scatter((500.0, 500.0), 96.0, &[(500.0, 500.0)], 7);
        // pi * 3^2 tiles * 0.5 = about 14.
        assert!((8..=15).contains(&pts.len()), "{}", pts.len());
        for (i, a) in pts.iter().enumerate() {
            assert!((a.1 - 500.0).hypot(a.2 - 500.0) <= 97.0);
            assert!((a.1 - 500.0).hypot(a.2 - 500.0) >= 19.0, "keeps off what is there");
            for b in &pts[i + 1..] {
                assert!((a.1 - b.1).hypot(a.2 - b.2) >= 19.0);
            }
        }
        assert!(pts.iter().any(|p| p.0 == "oak") && pts.iter().all(|p| p.0 == "oak" || p.0 == "pine"));
        assert_eq!(pts, s.scatter((500.0, 500.0), 96.0, &[(500.0, 500.0)], 7), "the same seed, the same forest");
        assert!(ScatterDef::fresh("none").scatter((0.0, 0.0), 50.0, &[], 1).is_empty());
    }

    #[test]
    fn the_index_finds_what_is_near() {
        let d = |x, y| Decor { def: 0, x, y, flip: false, state: 0, layer: DecorLayer::Sorted };
        let decor = vec![d(10, 10), d(2000, 10), d(5000, 5000)];
        let ix = DecorIndex::build(&decor, 200, 200);
        let near: Vec<u32> = ix.near(0.0, 0.0, 50.0, 50.0, 0.0).collect();
        assert_eq!(near, [0]);
        let mut wide: Vec<u32> = ix.near(0.0, 0.0, 2100.0, 50.0, 0.0).collect();
        wide.sort();
        assert_eq!(wide, [0, 1]);
        assert_eq!(ix.near(4990.0, 4990.0, 20.0, 20.0, 0.0).collect::<Vec<_>>(), [2]);
    }
}
