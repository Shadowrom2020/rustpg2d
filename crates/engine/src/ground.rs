//! Painted ground: a map's ground is a grid of 8 px cells, each holding a material (grass, dirt,
//! water…). Materials ([`MaterialDef`], `defs/materials.ron`) say what a cell looks like (a seamless
//! texture, how its edges meet other materials) and what it does (walkable, solid, footsteps).
//! `render::ground` draws the grid with a shader; the runtime collides with the cells.
//!
//! The grid is chunked like the tile layers (`map::CHUNK_SIZE` tiles a chunk side, `CELLS` cells a
//! tile side); a chunk of one material is stored as that one value. A cell holds the material's
//! slot in the map's `materials` list plus one; 0 = no ground (the tiles or the background show).

use serde::{Deserialize, Serialize};

use crate::map::CHUNK_SIZE;

/// Size of a ground cell in pixels.
pub const CELL: u32 = 8;
/// Cells along a tile's side (`TILE_SIZE / CELL`).
pub const CELLS: u32 = crate::project::TILE_SIZE / CELL;
/// Cells along a chunk's side.
pub const CHUNK_CELLS: u32 = CHUNK_SIZE * CELLS;
const CHUNK_CELL_AREA: usize = (CHUNK_CELLS * CHUNK_CELLS) as usize;
/// The most materials one map can use (the ground shader's table).
pub const MAX_MATERIALS: usize = 64;

/// How a material's edge meets the materials under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Edge {
    /// A hard edge at the art's pixel size, made ragged by noise (pixel art).
    #[default]
    Crisp,
    /// A soft fade into the neighbour.
    Soft,
    /// Exactly along the 8 px cells: straight lines and square corners (floors, carpets, plazas,
    /// tiles); a rim is a border just outside them.
    Straight,
}

/// What a material does to things moving over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Walk {
    #[default]
    Walk,
    /// Blocks walking, shots and sight (a cliff, a void).
    Solid,
    /// Blocks walking only: shots and sight pass (water, a chasm).
    Low,
}

impl Walk {
    pub const ALL: [Walk; 3] = [Walk::Walk, Walk::Solid, Walk::Low];
    pub fn label(self) -> &'static str {
        match self {
            Walk::Walk => "Walkable",
            Walk::Solid => "Solid",
            Walk::Low => "Low (blocks walking, not shots)",
        }
    }
    pub fn blocks_walking(self) -> bool {
        self != Walk::Walk
    }
}

/// A fringe drawn just outside a material's edge (a sandy band round a pond).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Rim {
    pub color: [u8; 3],
    /// Width in pixels (1..8).
    pub width: u8,
}

/// A ground material.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct MaterialDef {
    pub id: String,
    pub name: String,
    /// Seamless picture repeated over the ground (path relative to the project; width and
    /// height up to 256 px).
    pub texture: String,
    pub edge: Edge,
    /// How ragged the edge is: how far noise moves it, in pixels (0..4).
    pub edge_noise: f32,
    /// The edge's step size in pixels (the art's pixel size: 1, 2, 4).
    pub edge_pixel: u8,
    /// A fringe outside the edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rim: Option<Rim>,
    /// Where two materials meet, the higher one is drawn over the lower one.
    pub priority: u8,
    pub walk: Walk,
    /// Footstep sound (sound id; "" = the tiles' or none).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub step: String,
    /// The texture drifts this many pixels per second (flowing water).
    #[serde(skip_serializing_if = "is_still")]
    pub scroll: [f32; 2],
    /// Animation frames: the texture is this many frames side by side (1 = a still picture).
    #[serde(skip_serializing_if = "is_one")]
    pub frames: u8,
    /// Animation frames shown per second (0 = the first frame only).
    #[serde(skip_serializing_if = "is_zero")]
    pub fps: f32,
    /// Difficult terrain in fights: a step onto it costs double movement (mud, deep snow,
    /// shallow water, rubble).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub difficult: bool,
}

impl MaterialDef {
    /// The size of one animation frame of a texture `w` x `h` pixels (frames side by side).
    pub fn frame_size(&self, w: u32, h: u32) -> (u32, u32) {
        (w / self.frames.max(1) as u32, h)
    }
}

fn is_still(s: &[f32; 2]) -> bool {
    *s == [0.0, 0.0]
}

fn is_one(n: &u8) -> bool {
    *n == 1
}

fn is_zero(n: &f32) -> bool {
    *n == 0.0
}

impl Default for MaterialDef {
    fn default() -> Self {
        MaterialDef {
            id: String::new(),
            name: String::new(),
            texture: String::new(),
            edge: Edge::Crisp,
            edge_noise: 3.0,
            edge_pixel: 2,
            rim: None,
            priority: 50,
            walk: Walk::Walk,
            step: String::new(),
            scroll: [0.0, 0.0],
            frames: 1,
            fps: 0.0,
            difficult: false,
        }
    }
}

impl MaterialDef {
    /// The editor's "+ New" value.
    pub fn fresh(id: &str) -> Self {
        MaterialDef { id: id.to_string(), name: "New material".into(), ..Default::default() }
    }
}

/// One chunk of the ground grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum GroundChunk {
    /// Every cell holds this value.
    Fill(u8),
    /// `CHUNK_CELLS` x `CHUNK_CELLS` cells, row by row.
    Cells(Vec<u8>),
}

/// A map's painted ground: cells of `CELL` pixels, `CELLS` per tile side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Ground {
    /// Size in cells.
    w: u32,
    h: u32,
    chunks_w: u32,
    chunks_h: u32,
    chunks: Vec<GroundChunk>,
    /// Edit counter per chunk (the renderer re-uploads a chunk when it changed; not saved).
    #[serde(skip)]
    revs: Vec<u32>,
}

impl Ground {
    /// Empty ground (no material anywhere) for a map of `width` x `height` tiles.
    pub fn new(width: u32, height: u32) -> Self {
        let (chunks_w, chunks_h) = (width.div_ceil(CHUNK_SIZE), height.div_ceil(CHUNK_SIZE));
        let n = (chunks_w * chunks_h) as usize;
        Ground { w: width * CELLS, h: height * CELLS, chunks_w, chunks_h, chunks: vec![GroundChunk::Fill(0); n], revs: vec![0; n] }
    }

    /// Size in cells.
    pub fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    pub fn chunks_size(&self) -> (u32, u32) {
        (self.chunks_w, self.chunks_h)
    }

    /// Checks ground read from a file against the map's size (cells are indexed without bounds
    /// checks inside a chunk).
    pub fn validate(&self, width: u32, height: u32) -> Result<(), String> {
        let (cw, ch) = (width.div_ceil(CHUNK_SIZE), height.div_ceil(CHUNK_SIZE));
        if (self.w, self.h) != (width * CELLS, height * CELLS) || (self.chunks_w, self.chunks_h) != (cw, ch) || self.chunks.len() != (cw * ch) as usize {
            return Err(format!("the ground grid doesn't fit a {width}x{height} map"));
        }
        if self.chunks.iter().any(|c| matches!(c, GroundChunk::Cells(v) if v.len() != CHUNK_CELL_AREA)) {
            return Err("a ground chunk has the wrong number of cells".into());
        }
        Ok(())
    }

    /// Nothing painted anywhere.
    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(|c| *c == GroundChunk::Fill(0))
    }

    /// The cell's value (0 = no ground; outside the map too).
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return 0;
        }
        let (x, y) = (x as u32, y as u32);
        match &self.chunks[(y / CHUNK_CELLS * self.chunks_w + x / CHUNK_CELLS) as usize] {
            GroundChunk::Fill(v) => *v,
            GroundChunk::Cells(c) => c[((y % CHUNK_CELLS) * CHUNK_CELLS + x % CHUNK_CELLS) as usize],
        }
    }

    /// Sets a cell; returns its old value (cells outside the map are ignored and return `v`).
    pub fn set(&mut self, x: i32, y: i32, v: u8) -> u8 {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return v;
        }
        let (x, y) = (x as u32, y as u32);
        let ci = (y / CHUNK_CELLS * self.chunks_w + x / CHUNK_CELLS) as usize;
        let i = ((y % CHUNK_CELLS) * CHUNK_CELLS + x % CHUNK_CELLS) as usize;
        let chunk = &mut self.chunks[ci];
        let old = match chunk {
            GroundChunk::Fill(f) if *f == v => return v,
            GroundChunk::Fill(f) => {
                let f = *f;
                let mut cells = vec![f; CHUNK_CELL_AREA];
                cells[i] = v;
                *chunk = GroundChunk::Cells(cells);
                f
            }
            GroundChunk::Cells(c) => std::mem::replace(&mut c[i], v),
        };
        if old != v {
            self.touch(ci);
        }
        old
    }

    fn touch(&mut self, ci: usize) {
        if self.revs.len() != self.chunks.len() {
            self.revs.resize(self.chunks.len(), 0);
        }
        self.revs[ci] = self.revs[ci].wrapping_add(1);
    }

    /// Edit counter of chunk (`cx`, `cy`).
    pub fn rev(&self, cx: u32, cy: u32) -> u32 {
        self.revs.get((cy * self.chunks_w + cx) as usize).copied().unwrap_or(0)
    }

    /// Chunk (`cx`, `cy`), `None` outside the grid.
    pub fn chunk(&self, cx: u32, cy: u32) -> Option<&GroundChunk> {
        (cx < self.chunks_w && cy < self.chunks_h).then(|| &self.chunks[(cy * self.chunks_w + cx) as usize])
    }

    /// Every cell to `v`.
    pub fn fill(&mut self, v: u8) {
        for ci in 0..self.chunks.len() {
            self.chunks[ci] = GroundChunk::Fill(v);
            self.touch(ci);
        }
    }

    /// Chunks whose cells all hold one value become a single value again (before saving).
    pub fn compact(&mut self) {
        for c in &mut self.chunks {
            if let GroundChunk::Cells(v) = c {
                if v.iter().all(|x| *x == v[0]) {
                    *c = GroundChunk::Fill(v[0]);
                }
            }
        }
    }

    /// The ground of a map resized to `width` x `height` tiles, keeping the top-left area.
    pub fn resized(&self, width: u32, height: u32) -> Ground {
        let mut g = Ground::new(width, height);
        for y in 0..self.h.min(g.h) as i32 {
            for x in 0..self.w.min(g.w) as i32 {
                g.set(x, y, self.get(x, y));
            }
        }
        g.compact();
        g
    }

    /// The values in use (0 included when some cell is empty).
    pub fn used(&self) -> std::collections::BTreeSet<u8> {
        let mut s = std::collections::BTreeSet::new();
        for c in &self.chunks {
            match c {
                GroundChunk::Fill(v) => {
                    s.insert(*v);
                }
                GroundChunk::Cells(v) => s.extend(v.iter().copied()),
            }
        }
        s
    }

    /// Replaces every cell value through `f` (a material left the map's list: renumbering).
    pub fn remap(&mut self, f: impl Fn(u8) -> u8) {
        for ci in 0..self.chunks.len() {
            match &mut self.chunks[ci] {
                GroundChunk::Fill(v) => *v = f(*v),
                GroundChunk::Cells(c) => c.iter_mut().for_each(|v| *v = f(*v)),
            }
            self.touch(ci);
        }
    }

    /// The cells (`x`, `y`, old value) a round brush of `radius` pixels at `center` (world
    /// pixels) changes to `v`; they are set.
    pub fn paint_circle(&mut self, center: (f32, f32), radius: f32, v: u8) -> Vec<(i32, i32, u8)> {
        let c = CELL as f32;
        let r = radius.max(c * 0.5);
        let (x0, x1) = (((center.0 - r) / c).floor() as i32, ((center.0 + r) / c).ceil() as i32);
        let (y0, y1) = (((center.1 - r) / c).floor() as i32, ((center.1 + r) / c).ceil() as i32);
        let mut changed = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = ((x as f32 + 0.5) * c, (y as f32 + 0.5) * c);
                if (px - center.0).powi(2) + (py - center.1).powi(2) <= r * r {
                    let old = self.set(x, y, v);
                    if old != v {
                        changed.push((x, y, old));
                    }
                }
            }
        }
        changed
    }

    /// A stroke from `a` to `b` (world pixels), `width` pixels wide, painted with `v`; returns
    /// the changed cells like [`Ground::paint_circle`].
    pub fn paint_line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, v: u8) -> Vec<(i32, i32, u8)> {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let steps = (len / (CELL as f32 * 0.5)).ceil().max(1.0) as i32;
        let mut changed = Vec::new();
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            changed.extend(self.paint_circle((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), width / 2.0, v));
        }
        changed
    }

    /// Paints the cells whose centres lie inside the rectangle between corners `a` and `b`
    /// (world pixels, any order) with `v`; returns the changed cells like [`Ground::paint_circle`].
    pub fn paint_rect(&mut self, a: (f32, f32), b: (f32, f32), v: u8) -> Vec<(i32, i32, u8)> {
        let c = CELL as f32;
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        // Cells whose centre (cx + 0.5) * c lies in [x0, x1).
        let (cx0, cx1) = ((x0 / c - 0.5).ceil() as i32, (x1 / c - 0.5).ceil() as i32);
        let (cy0, cy1) = ((y0 / c - 0.5).ceil() as i32, (y1 / c - 0.5).ceil() as i32);
        let mut changed = Vec::new();
        for y in cy0..cy1 {
            for x in cx0..cx1 {
                let old = self.set(x, y, v);
                if old != v {
                    changed.push((x, y, old));
                }
            }
        }
        changed
    }

    /// Paints the cells whose centres lie inside the polygon `points` (world pixels) with `v`;
    /// returns the changed cells like [`Ground::paint_circle`].
    pub fn paint_polygon(&mut self, points: &[(f32, f32)], v: u8) -> Vec<(i32, i32, u8)> {
        if points.len() < 3 {
            return Vec::new();
        }
        let c = CELL as f32;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in points {
            (x0, y0, x1, y1) = (x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1));
        }
        let inside = |px: f32, py: f32| {
            let mut odd = false;
            let mut j = points.len() - 1;
            for i in 0..points.len() {
                let (a, b) = (points[i], points[j]);
                if (a.1 > py) != (b.1 > py) && px < (b.0 - a.0) * (py - a.1) / (b.1 - a.1) + a.0 {
                    odd = !odd;
                }
                j = i;
            }
            odd
        };
        let mut changed = Vec::new();
        for y in (y0 / c).floor() as i32..=(y1 / c).ceil() as i32 {
            for x in (x0 / c).floor() as i32..=(x1 / c).ceil() as i32 {
                if inside((x as f32 + 0.5) * c, (y as f32 + 0.5) * c) {
                    let old = self.set(x, y, v);
                    if old != v {
                        changed.push((x, y, old));
                    }
                }
            }
        }
        changed
    }

    /// Fills the area of cells connected to (`x`, `y`) that hold the same value with `v`
    /// (4-neighbourhood); returns the changed cells.
    pub fn flood_fill(&mut self, x: i32, y: i32, v: u8) -> Vec<(i32, i32, u8)> {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return Vec::new();
        }
        let from = self.get(x, y);
        if from == v {
            return Vec::new();
        }
        let mut changed = Vec::new();
        let mut stack = vec![(x, y)];
        while let Some((x, y)) = stack.pop() {
            if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h || self.get(x, y) != from {
                continue;
            }
            self.set(x, y, v);
            changed.push((x, y, from));
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        changed
    }

    /// How many of tile (`tx`, `ty`)'s cells satisfy `f`.
    pub fn count_in_tile(&self, tx: u32, ty: u32, f: impl Fn(u8) -> bool) -> u32 {
        let (x0, y0) = ((tx * CELLS) as i32, (ty * CELLS) as i32);
        let mut n = 0;
        for y in y0..y0 + CELLS as i32 {
            for x in x0..x0 + CELLS as i32 {
                n += f(self.get(x, y)) as u32;
            }
        }
        n
    }

    /// The cells a rectangle (world pixels) overlaps, as (x0, y0, x1, y1) inclusive.
    pub fn cells_of_rect(x: f32, y: f32, w: f32, h: f32) -> (i32, i32, i32, i32) {
        let c = CELL as f32;
        ((x / c).floor() as i32, (y / c).floor() as i32, ((x + w - 0.001) / c).floor() as i32, ((y + h - 0.001) / c).floor() as i32)
    }
}

/// Per map slot (a cell value minus one), what each material does: built when a map is
/// entered from its `materials` list and the database.
#[derive(Clone, Debug, Default)]
pub struct MaterialTable {
    /// Indexed by cell value (0 = no ground: walkable).
    pub walk: Vec<Walk>,
    pub step: Vec<String>,
}

impl MaterialTable {
    pub fn new(materials: &[String], db: &crate::defs::Database) -> Self {
        let mut t = MaterialTable { walk: vec![Walk::Walk], step: vec![String::new()] };
        for id in materials {
            let d = db.material(id);
            t.walk.push(d.map_or(Walk::Walk, |d| d.walk));
            t.step.push(d.map_or(String::new(), |d| d.step.clone()));
        }
        t
    }

    pub fn walk_of(&self, v: u8) -> Walk {
        self.walk.get(v as usize).copied().unwrap_or(Walk::Walk)
    }

    /// A tile counts as blocked for route planning when at least half its cells are (A* and
    /// the tile grid); movement itself tests the cells.
    pub fn tile_blocked(&self, g: &Ground, tx: u32, ty: u32) -> bool {
        g.count_in_tile(tx, ty, |v| self.walk_of(v).blocks_walking()) * 2 >= CELLS * CELLS
    }

    /// A tile stops shots when at least half its cells are solid.
    pub fn tile_blocks_shot(&self, g: &Ground, tx: u32, ty: u32) -> bool {
        g.count_in_tile(tx, ty, |v| self.walk_of(v) == Walk::Solid) * 2 >= CELLS * CELLS
    }

    /// Adds the ground to a map's tile grids (`solid`, `shot`: one entry per tile, from the tile
    /// layers and the collision overrides); tiles painted *Walkable* in the collision layer stay
    /// free, *Solid* ones are already set.
    pub fn apply_to_tiles(&self, map: &crate::map::Map, solid: &mut [bool], shot: &mut [bool]) {
        if map.ground.is_empty() {
            return;
        }
        for ty in 0..map.height {
            for tx in 0..map.width {
                if map.collision_at(tx, ty) != crate::map::CollisionOverride::Auto {
                    continue;
                }
                let i = (ty * map.width + tx) as usize;
                if let Some(s) = solid.get_mut(i) {
                    *s |= self.tile_blocked(&map.ground, tx, ty);
                }
                if let Some(s) = shot.get_mut(i) {
                    *s |= self.tile_blocks_shot(&map.ground, tx, ty);
                }
            }
        }
    }

    /// The footstep sound of the ground under a point (world pixels), if its material has one.
    pub fn step_at(&self, g: &Ground, x: f32, y: f32) -> Option<&str> {
        let c = CELL as f32;
        let v = g.get((x / c).floor() as i32, (y / c).floor() as i32);
        self.step.get(v as usize).filter(|s| !s.is_empty()).map(String::as_str)
    }

    /// Whether any cell under the rectangle (world pixels) blocks walking.
    pub fn rect_blocked(&self, g: &Ground, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (x0, y0, x1, y1) = Ground::cells_of_rect(x, y, w, h);
        (y0..=y1).any(|cy| (x0..=x1).any(|cx| self.walk_of(g.get(cx, cy)).blocks_walking()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangles_take_the_cells_inside() {
        let mut g = Ground::new(4, 4);
        // A 1 x 2 tile rectangle at tile (1, 1): 4 x 8 cells, either corner first.
        let n = g.paint_rect((64.0, 96.0), (32.0, 32.0), 3).len();
        assert_eq!(n, 4 * 8);
        assert_eq!((g.get(4, 4), g.get(7, 11), g.get(8, 4), g.get(4, 12), g.get(3, 4)), (3, 3, 0, 0, 0));
        // An empty one paints nothing.
        assert!(g.paint_rect((40.0, 40.0), (40.0, 90.0), 5).is_empty());
    }

    #[test]
    fn cells_across_chunks_and_compaction() {
        let mut g = Ground::new(40, 40);
        assert!(g.is_empty());
        assert_eq!(g.size(), (160, 160));
        for &(x, y) in &[(0, 0), (127, 127), (128, 127), (159, 159)] {
            assert_eq!(g.set(x, y, 3), 0);
            assert_eq!(g.get(x, y), 3);
        }
        assert_eq!(g.get(-1, 0), 0);
        assert_eq!(g.get(160, 0), 0);
        assert_eq!(g.set(500, 0, 7), 7, "outside: ignored");
        assert!(g.rev(0, 0) > 0 && g.rev(1, 1) > 0);
        g.fill(2);
        g.set(5, 5, 9);
        g.set(5, 5, 2);
        g.compact();
        assert_eq!(g.chunk(0, 0), Some(&GroundChunk::Fill(2)));
        assert_eq!(g.used().into_iter().collect::<Vec<_>>(), [2]);
    }

    #[test]
    fn round_trip_through_postcard() {
        let mut g = Ground::new(70, 33);
        g.fill(1);
        g.paint_circle((300.0, 200.0), 40.0, 4);
        let back: Ground = postcard::from_bytes(&postcard::to_stdvec(&g).unwrap()).unwrap();
        assert_eq!(back.validate(70, 33), Ok(()));
        assert_eq!(back.size(), g.size());
        for y in 0..g.size().1 as i32 {
            for x in 0..g.size().0 as i32 {
                assert_eq!(back.get(x, y), g.get(x, y));
            }
        }
        assert!(back.validate(71, 33).is_err());
    }

    #[test]
    fn brushes_lines_and_fill() {
        let mut g = Ground::new(10, 10);
        let c = g.paint_circle((80.0, 80.0), 12.0, 1);
        assert!(!c.is_empty() && c.iter().all(|&(_, _, old)| old == 0));
        assert_eq!(g.get(10, 10), 1, "the centre cell");
        assert_eq!(g.get(0, 0), 0);
        g.paint_line((0.0, 300.0), (319.0, 300.0), 16.0, 2);
        assert!((0..40).all(|x| g.get(x, 37) == 2), "a road across the map");
        // The road cuts the map in two: filling above it leaves the cells below.
        let filled = g.flood_fill(0, 0, 3);
        assert!(!filled.is_empty());
        assert_eq!(g.get(0, 0), 3);
        assert_eq!(g.get(0, 39), 0);
        assert_eq!(g.get(10, 10), 1, "another material isn't filled");
    }

    #[test]
    fn polygons_fill_the_cells_inside() {
        let mut g = Ground::new(4, 4);
        // A triangle over the top-left tile's diagonal half.
        let c = g.paint_polygon(&[(0.0, 0.0), (32.0, 0.0), (0.0, 32.0)], 5);
        assert_eq!(g.get(0, 0), 5);
        assert_eq!(g.get(3, 3), 0, "past the diagonal");
        assert_eq!(c.len(), 6, "cells with their centres inside: 3 + 2 + 1");
        assert!(g.paint_polygon(&[(0.0, 0.0), (32.0, 0.0)], 5).is_empty());
    }

    #[test]
    fn tiles_count_blocked_when_half_their_cells_are() {
        let mut g = Ground::new(4, 4);
        let t = MaterialTable { walk: vec![Walk::Walk, Walk::Walk, Walk::Low, Walk::Solid], step: vec![String::new(); 4] };
        g.fill(1);
        // Tile (0,0): 8 of 16 cells water.
        for y in 0..2 {
            for x in 0..4 {
                g.set(x, y, 2);
            }
        }
        assert!(t.tile_blocked(&g, 0, 0));
        assert!(!t.tile_blocks_shot(&g, 0, 0), "water lets shots pass");
        // Tile (1,0): 4 cells of rock: walkable for planning, but a hitbox over them is blocked.
        for y in 0..2 {
            for x in 4..6 {
                g.set(x, y, 3);
            }
        }
        assert!(!t.tile_blocked(&g, 1, 0));
        assert!(t.rect_blocked(&g, 34.0, 2.0, 10.0, 10.0));
        assert!(!t.rect_blocked(&g, 50.0, 20.0, 10.0, 10.0));
        assert!(!t.tile_blocked(&g, 3, 3));
    }
}
