//! Structures: assets of several tiles (houses, walls, rooms, camps…) that the editor places on
//! a map in one click and captures from a map area: painted ground cells, decorations,
//! collision overrides and entities. See `defs::StructureDef`.

use crate::decor::Decor;
use crate::defs::StructureDef;
use crate::entity::EntityInstance;
use crate::ground::CELLS;
use crate::map::{CollisionOverride, Map};
use crate::project::TILE_SIZE;

impl StructureDef {
    /// Collision value: reset the map's override to "automatic" (0 keeps it).
    pub const COLLISION_AUTO: u8 = 3;
    /// Ground cell value: no ground here (0 keeps the map's).
    pub const CLEAR_GROUND: u8 = 255;

    /// Size in ground cells.
    pub fn cells_size(&self) -> (u32, u32) {
        (self.w * CELLS, self.h * CELLS)
    }

    /// Ground value of cell (cx, cy): 0 = keep, [`Self::CLEAR_GROUND`], else a material slot + 1.
    pub fn ground_at(&self, cx: u32, cy: u32) -> u8 {
        let (gw, _) = self.cells_size();
        self.ground.get((cy * gw + cx) as usize).copied().unwrap_or(0)
    }

    /// The material id of a ground value.
    pub fn material_of(&self, v: u8) -> Option<&str> {
        (v > 0 && v != Self::CLEAR_GROUND).then(|| self.materials.get(v as usize - 1).map(String::as_str)).flatten()
    }

    /// Sets a ground cell (cells outside are ignored).
    pub fn set_ground(&mut self, cx: u32, cy: u32, v: u8) {
        let (gw, gh) = self.cells_size();
        if cx >= gw || cy >= gh {
            return;
        }
        self.ground.resize((gw * gh) as usize, 0);
        self.ground[(cy * gw + cx) as usize] = v;
    }

    /// The ground value of a material, registering it.
    pub fn material_value(&mut self, id: &str) -> u8 {
        match self.materials.iter().position(|m| m == id) {
            Some(i) => i as u8 + 1,
            None => {
                self.materials.push(id.to_string());
                self.materials.len() as u8
            }
        }
    }

    /// The `decor_defs` slot of a prop, registering it.
    pub fn decor_slot(&mut self, prop: &str) -> u16 {
        match self.decor_defs.iter().position(|d| d == prop) {
            Some(i) => i as u16,
            None => {
                self.decor_defs.push(prop.to_string());
                (self.decor_defs.len() - 1) as u16
            }
        }
    }

    /// Collision override at a cell (0 = keep).
    pub fn collision_at(&self, x: u32, y: u32) -> u8 {
        self.collision.get((y * self.w + x) as usize).copied().unwrap_or(0)
    }

    pub fn set_collision(&mut self, x: u32, y: u32, c: u8) {
        if x >= self.w || y >= self.h {
            return;
        }
        self.collision.resize((self.w * self.h) as usize, 0);
        self.collision[(y * self.w + x) as usize] = c;
    }

    /// Changes the size, keeping the content at the top-left (what lies outside is dropped).
    pub fn resize(&mut self, w: u32, h: u32) {
        let (w, h) = (w.clamp(1, 256), h.clamp(1, 256));
        let remap = |old: &Vec<u8>, (ow, oh): (u32, u32), (nw, nh): (u32, u32)| -> Vec<u8> {
            let mut out = vec![0; (nw * nh) as usize];
            for y in 0..nh.min(oh) {
                for x in 0..nw.min(ow) {
                    out[(y * nw + x) as usize] = old.get((y * ow + x) as usize).copied().unwrap_or(0);
                }
            }
            out
        };
        let (ow, oh) = (self.w, self.h);
        if !self.ground.is_empty() {
            self.ground = remap(&self.ground, (ow * CELLS, oh * CELLS), (w * CELLS, h * CELLS));
        }
        if !self.collision.is_empty() {
            self.collision = remap(&self.collision, (ow, oh), (w, h));
        }
        if !self.clear.is_empty() {
            self.clear = remap(&self.clear, (ow, oh), (w, h));
        }
        let ts = TILE_SIZE as f32;
        self.entities.retain(|e| e.x < w as f32 * ts && e.y < h as f32 * ts);
        let (pw, ph) = ((w * TILE_SIZE) as i32, (h * TILE_SIZE) as i32);
        self.decor.retain(|d| d.x < pw && d.y <= ph);
        self.w = w;
        self.h = h;
    }

    /// Tidies up after editing: drops materials and props nothing uses (renumbering), and
    /// grids that do nothing.
    pub fn prune(&mut self) {
        let mut used = vec![false; self.materials.len()];
        for &v in self.ground.iter().filter(|&&v| v > 0 && v != Self::CLEAR_GROUND) {
            if let Some(u) = used.get_mut(v as usize - 1) {
                *u = true;
            }
        }
        let mut new_value = vec![0u8; used.len()];
        let mut kept = Vec::new();
        for (i, m) in self.materials.iter().enumerate() {
            if used[i] {
                kept.push(m.clone());
                new_value[i] = kept.len() as u8;
            }
        }
        for v in self.ground.iter_mut().filter(|v| **v > 0 && **v != Self::CLEAR_GROUND) {
            *v = new_value.get(*v as usize - 1).copied().unwrap_or(0);
        }
        self.materials = kept;
        let mut used = vec![false; self.decor_defs.len()];
        for d in &self.decor {
            if let Some(u) = used.get_mut(d.def as usize) {
                *u = true;
            }
        }
        let mut new_slot = vec![0u16; used.len()];
        let mut kept = Vec::new();
        for (i, p) in self.decor_defs.iter().enumerate() {
            if used[i] {
                new_slot[i] = kept.len() as u16;
                kept.push(p.clone());
            }
        }
        self.decor.retain(|d| (d.def as usize) < used.len());
        for d in &mut self.decor {
            d.def = new_slot[d.def as usize];
        }
        self.decor_defs = kept;
        for grid in [&mut self.ground, &mut self.collision, &mut self.clear] {
            if grid.iter().all(|&c| c == 0) {
                grid.clear();
            }
        }
    }

    /// Captures a rectangle of `map`: its painted ground (`ground`; off for things that sit on
    /// any ground), the decorations and entities whose feet are inside, and the collision
    /// overrides. With the ground, placing it clears the decorations of the area first (trees
    /// inside a room); cells without ground keep the map's.
    pub fn capture(map: &Map, id: &str, name: &str, rect: (u32, u32, u32, u32), ground: bool) -> StructureDef {
        let (x0, y0, w, h) = rect;
        let mut d = StructureDef { id: id.into(), name: name.into(), w, h, ..Default::default() };
        if ground {
            for cy in 0..h * CELLS {
                for cx in 0..w * CELLS {
                    let v = map.ground.get((x0 * CELLS + cx) as i32, (y0 * CELLS + cy) as i32);
                    if let Some(m) = map.material_of(v) {
                        let s = d.material_value(m);
                        d.set_ground(cx, cy, s);
                    }
                }
            }
            d.clear = vec![1; (w * h) as usize];
        }
        let ts = TILE_SIZE as i32;
        let (left, top) = (x0 as i32 * ts, y0 as i32 * ts);
        let (right, bottom) = (left + w as i32 * ts, top + h as i32 * ts);
        for dec in &map.decor {
            if dec.x >= left && dec.x < right && dec.y > top && dec.y <= bottom {
                let Some(p) = map.decor_defs.get(dec.def as usize) else { continue };
                let def = d.decor_slot(p);
                d.decor.push(Decor { def, x: dec.x - left, y: dec.y - top, ..*dec });
            }
        }
        let mut col = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let (mx, my) = (x0 + x, y0 + y);
                if map.in_bounds(mx as i32, my as i32) {
                    col[(y * w + x) as usize] = match map.collision_at(mx, my) {
                        CollisionOverride::Auto => Self::COLLISION_AUTO,
                        c => c as u8,
                    };
                }
            }
        }
        if col.iter().any(|&c| c != 0 && c != Self::COLLISION_AUTO) {
            d.collision = col;
        }
        let ts = TILE_SIZE as f32;
        let (left, top) = (x0 as f32 * ts, y0 as f32 * ts);
        let (right, bottom) = (left + w as f32 * ts, top + h as f32 * ts);
        for e in &map.entities {
            if e.x >= left && e.x < right && e.y >= top && e.y < bottom {
                let mut c = e.clone();
                c.uid = 0;
                c.x -= left;
                c.y -= top;
                d.entities.push(c);
            }
        }
        d
    }

    /// Places the structure with its top-left corner at tile `(x, y)`: paints its ground
    /// (registering the materials on the map), clears and adds decorations, sets collision
    /// overrides and adds the entities with new uids. What falls outside the map is skipped.
    /// Returns what changed (for undo).
    pub fn place(&self, map: &mut Map, x: i32, y: i32) -> Placement {
        let mut out = Placement::default();
        let (gw, _) = self.cells_size();
        let values: Vec<Option<u8>> = self.materials.iter().map(|m| map.material_value(m)).collect();
        let (cx0, cy0) = (x * CELLS as i32, y * CELLS as i32);
        for (i, &v) in self.ground.iter().enumerate() {
            let new = match v {
                0 => continue,
                Self::CLEAR_GROUND => 0,
                v => match values.get(v as usize - 1).copied().flatten() {
                    Some(n) => n,
                    None => continue,
                },
            };
            let (cx, cy) = (cx0 + (i as u32 % gw) as i32, cy0 + (i as u32 / gw) as i32);
            let (tx, ty) = (cx.div_euclid(CELLS as i32), cy.div_euclid(CELLS as i32));
            if !map.in_bounds(tx, ty) {
                continue;
            }
            let old = map.ground.set(cx, cy, new);
            if old != new {
                out.ground.push((cx, cy, old, new));
            }
        }
        let ts = TILE_SIZE as i32;
        let cleared = |dx: i32, dy: i32| -> bool {
            let (tx, ty) = (dx.div_euclid(ts) - x, (dy - 1).div_euclid(ts) - y);
            tx >= 0 && ty >= 0 && (tx as u32) < self.w && (ty as u32) < self.h && self.clear.get((ty as u32 * self.w + tx as u32) as usize) == Some(&1)
        };
        if !self.clear.is_empty() {
            let (gone, kept): (Vec<Decor>, Vec<Decor>) = map.decor.iter().partition(|d| cleared(d.x, d.y));
            map.decor = kept;
            out.decor_removed = gone;
        }
        let (px, py) = (x * ts, y * ts);
        for d in &self.decor {
            let Some(prop) = self.decor_defs.get(d.def as usize) else { continue };
            let n = Decor { def: map.decor_slot(prop), x: d.x + px, y: d.y + py, ..*d };
            if map.in_bounds(n.x.div_euclid(ts), (n.y - 1).div_euclid(ts)) {
                map.decor.push(n);
                out.decor_added.push(n);
            }
        }
        for (i, &c) in self.collision.iter().enumerate() {
            let (mx, my) = (x + (i as u32 % self.w) as i32, y + (i as u32 / self.w) as i32);
            if c == 0 || !map.in_bounds(mx, my) {
                continue;
            }
            let new = match c {
                1 => CollisionOverride::Solid,
                2 => CollisionOverride::Walkable,
                _ => CollisionOverride::Auto,
            };
            let old = map.set_collision(mx as u32, my as u32, new);
            if old != new {
                out.collision.push((mx as u32, my as u32, old as u8, new as u8));
            }
        }
        let ts = TILE_SIZE as f32;
        for e in &self.entities {
            let mut n: EntityInstance = e.clone();
            n.uid = map.alloc_uid();
            n.x += x as f32 * ts;
            n.y += y as f32 * ts;
            if map.in_bounds((n.x / ts) as i32, ((n.y - 1.0) / ts) as i32) {
                out.entities.push(n.uid);
                map.entities.push(n);
            }
        }
        out
    }
}

/// What placing a structure changed: ground cells `(x, y, old, new)`, the decorations it
/// removed and added, collision bytes and the uids of the added entities.
#[derive(Clone, Debug, Default)]
pub struct Placement {
    pub ground: Vec<(i32, i32, u8, u8)>,
    pub decor_removed: Vec<Decor>,
    pub decor_added: Vec<Decor>,
    pub collision: Vec<(u32, u32, u8, u8)>,
    pub entities: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decor::DecorLayer;
    use crate::entity::EntityKind;

    fn decor(map: &mut Map, prop: &str, x: i32, y: i32) {
        let def = map.decor_slot(prop);
        map.decor.push(Decor { def, x, y, flip: false, state: 0, layer: DecorLayer::Sorted });
    }

    #[test]
    fn capture_and_place_roundtrip() {
        let mut src = Map::new("src", 10, 10);
        // A 3x2 "hut": a wooden floor, walls along the top, a walkable door, a chest.
        let wood = src.material_value("wood").unwrap();
        for cy in 3 * 4..5 * 4 {
            for cx in 2 * 4..5 * 4 {
                src.ground.set(cx, cy, wood);
            }
        }
        for x in 2..5 {
            decor(&mut src, "wall", x * 32 + 16, 4 * 32);
        }
        decor(&mut src, "far_tree", 8 * 32, 8 * 32);
        src.set_collision(3, 4, CollisionOverride::Walkable);
        let uid = src.alloc_uid();
        src.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 4.5 * 32.0, 5.0 * 32.0 - 1.0));
        let d = StructureDef::capture(&src, "hut", "Hut", (2, 3, 3, 2), true);
        assert_eq!((d.w, d.h), (3, 2));
        assert_eq!(d.materials, ["wood"]);
        assert_eq!(d.ground.len(), 12 * 8);
        assert!(d.ground.iter().all(|&v| v == 1));
        assert_eq!((d.decor.len(), d.decor_defs.as_slice()), (3, &["wall".to_string()][..]));
        assert_eq!((d.decor[0].x, d.decor[0].y), (16, 32));
        assert_eq!(d.collision[4], CollisionOverride::Walkable as u8);
        assert_eq!(d.entities.len(), 1);
        assert_eq!((d.entities[0].x, d.entities[0].y), (2.5 * 32.0, 2.0 * 32.0 - 1.0));

        // Place it on a map with grass and a tree where the hut goes.
        let mut m = Map::new("dst", 8, 8);
        let grass = m.material_value("grass").unwrap();
        m.ground.fill(grass);
        decor(&mut m, "tree", 5 * 32 + 10, 7 * 32);
        decor(&mut m, "tree", 32, 2 * 32);
        m.next_uid = 7;
        let p = d.place(&mut m, 4, 5);
        let wood = m.materials.iter().position(|x| x == "wood").unwrap() as u8 + 1;
        assert_eq!(m.ground.get(4 * 4, 5 * 4), wood);
        assert_eq!(m.ground.get(4 * 4 - 1, 5 * 4), grass);
        assert_eq!(p.decor_removed.len(), 1, "the tree inside the hut's area");
        assert_eq!(p.decor_added.len(), 3);
        assert_eq!(m.decor.len(), 4);
        assert_eq!(m.decor_defs[m.decor[1].def as usize], "wall");
        assert_eq!((m.decor[1].x, m.decor[1].y), (4 * 32 + 16, 6 * 32));
        assert_eq!(m.collision_at(5, 6), CollisionOverride::Walkable);
        assert_eq!(p.entities, vec![7]);
        assert_eq!((m.entities[0].x, m.entities[0].y), (6.5 * 32.0, 7.0 * 32.0 - 1.0));
        assert!(p.ground.iter().all(|&(_, _, old, new)| old == grass && new == wood));
        // Partly outside the map: only what is inside is placed, no panic.
        let p = d.place(&mut m, 6, 7);
        assert!(p.ground.len() == 2 * 4 * 4 && p.entities.is_empty());
    }

    #[test]
    fn editing_helpers() {
        let mut d = StructureDef { id: "x".into(), w: 2, h: 2, ..Default::default() };
        let a = d.material_value("a");
        let b = d.material_value("b");
        d.set_ground(7, 7, b);
        d.set_ground(0, 0, a);
        d.set_ground(1, 0, StructureDef::CLEAR_GROUND);
        d.set_collision(1, 0, 2);
        let slot = d.decor_slot("rock");
        d.decor.push(Decor { def: slot, x: 48, y: 64, flip: false, state: 0, layer: DecorLayer::Sorted });
        d.resize(3, 1);
        assert_eq!((d.w, d.ground.len(), d.collision_at(1, 0)), (3, 12 * 4, 2));
        assert!(d.decor.is_empty(), "the rock stood in the second row");
        // The only "b" cell was cut off: "b" goes; so does the unused rock.
        d.prune();
        assert_eq!(d.materials, ["a"]);
        assert_eq!((d.ground_at(0, 0), d.ground_at(1, 0)), (1, StructureDef::CLEAR_GROUND));
        assert!(d.decor_defs.is_empty());
        // Removing "a" while "b" stays renumbers b's cells.
        let mut d = StructureDef { id: "y".into(), w: 1, h: 1, ..Default::default() };
        let _ = d.material_value("a");
        let b = d.material_value("b");
        d.set_ground(0, 0, b);
        d.prune();
        assert_eq!((d.materials.as_slice(), d.ground_at(0, 0)), (&["b".to_string()][..], 1));
    }

    #[test]
    fn capture_without_ground_keeps_the_map_ground() {
        let mut src = Map::new("src", 4, 4);
        let grass = src.material_value("grass").unwrap();
        src.ground.fill(grass);
        decor(&mut src, "tree", 48, 64);
        let d = StructureDef::capture(&src, "tree", "Tree", (1, 1, 1, 1), false);
        assert!(d.ground.is_empty() && d.clear.is_empty());
        assert_eq!(d.decor_defs, ["tree"]);
        let mut m = Map::new("m", 4, 4);
        decor(&mut m, "bush", 48, 64);
        d.place(&mut m, 1, 1);
        assert_eq!(m.decor.len(), 2, "nothing cleared");
    }
}
