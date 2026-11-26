//! An open map with undo/redo history and editing operations.

use std::collections::HashMap;
use std::time::Instant;

use rustpg2d_engine::map::Map;
use rustpg2d_engine::render::{Assets, Overview};

/// Everything on a map that isn't tile data (the map file's own RON part).
pub type Objects = rustpg2d_engine::map::MapObjects;

pub enum Edit {
    Collision { changes: Vec<(u32, u32, u8, u8)> },
    /// Painted ground cells (x, y, old, new).
    Ground { changes: Vec<(i32, i32, u8, u8)> },
    /// Decorations added and removed.
    Decor { added: Vec<rustpg2d_engine::decor::Decor>, removed: Vec<rustpg2d_engine::decor::Decor> },
    Objects { before: Box<Objects>, after: Box<Objects>, tag: &'static str, time: Instant },
    /// Whole-map replacement (resize, layer add/remove).
    Map { before: Box<Map>, after: Box<Map> },
    /// Several edits that undo / redo together (placing a structure).
    Group(Vec<Edit>),
}

/// Collision overrides painted during a mouse drag.
#[derive(Default)]
pub struct Stroke {
    col: HashMap<(u32, u32), (u8, u8)>,
}

pub struct MapDoc {
    pub map: Map,
    pub overview: Overview,
    /// History entries with an id each (ids are never reused).
    undo: Vec<(u64, Edit)>,
    redo: Vec<(u64, Edit)>,
    next_id: u64,
    /// History position of the saved file: `Some(id of the newest undo entry)` (`Some(None)`
    /// = empty history), or `None` when no undo/redo can get back to it.
    saved: Option<Option<u64>>,
    stroke: Option<Stroke>,
}

const MAX_HISTORY: usize = 200;

impl MapDoc {
    pub fn new(map: Map, assets: &Assets) -> Self {
        let overview = Overview::build(&map, assets);
        MapDoc { map, overview, undo: Vec::new(), redo: Vec::new(), next_id: 0, saved: Some(None), stroke: None }
    }

    pub fn rebuild_overview(&mut self, assets: &Assets) {
        self.overview = Overview::build(&self.map, assets);
    }

    fn push(&mut self, e: Edit) {
        self.next_id += 1;
        self.undo.push((self.next_id, e));
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
            if self.saved == Some(None) {
                self.saved = None; // the saved state was before the dropped entry
            }
        }
        self.redo.clear();
    }

    /// History position right now.
    fn position(&self) -> Option<u64> {
        self.undo.last().map(|(id, _)| *id)
    }

    /// How many steps can be undone (the workspace's timeline watches it grow).
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Whether the map differs from the saved file. Undoing back to the saved state (or
    /// redoing to it) makes it clean again.
    pub fn is_dirty(&self) -> bool {
        self.stroke.is_some() || self.saved != Some(self.position())
    }

    /// The map was written to (or freshly loaded from) disk.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.position());
    }

    /// The map was changed outside the undo history: only saving makes it clean again.
    pub fn mark_changed(&mut self) {
        self.saved = None;
    }

    /// Drops undo/redo history (after edits made outside the history, e.g. id renames).
    pub fn clear_history(&mut self) {
        self.end_stroke();
        let clean = !self.is_dirty();
        self.undo.clear();
        self.redo.clear();
        self.saved = if clean { Some(None) } else { None };
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ------------------------------------------------------------ strokes

    pub fn begin_stroke(&mut self) {
        self.end_stroke();
        self.stroke = Some(Stroke::default());
    }

    pub fn end_stroke(&mut self) {
        let Some(s) = self.stroke.take() else { return };
        let changes: Vec<_> = s.col.into_iter().filter(|(_, (a, b))| a != b).map(|((x, y), (a, b))| (x, y, a, b)).collect();
        if !changes.is_empty() {
            self.push(Edit::Collision { changes });
        }
    }

    pub fn paint_collision(&mut self, x: i32, y: i32, value: u8) {
        if !self.map.in_bounds(x, y) {
            return;
        }
        if self.stroke.is_none() {
            self.begin_stroke();
        }
        let i = (y as u32 * self.map.width + x as u32) as usize;
        let old = std::mem::replace(&mut self.map.collision[i], value);
        let s = self.stroke.as_mut().unwrap();
        s.col.entry((x as u32, y as u32)).or_insert((old, value)).1 = value;
    }

    // ------------------------------------------------------------ objects

    /// Records a change to entities/zones/edges/triggers. Consecutive edits with the
    /// same tag within a second are merged (e.g. typing into a text field).
    pub fn record_objects(&mut self, before: Objects, tag: &'static str) {
        let after = Objects::of(&self.map);
        if after == before {
            return;
        }
        let now = Instant::now();
        let at_saved = self.saved == Some(self.position());
        if let Some((_, Edit::Objects { after: last_after, tag: last_tag, time, .. })) = self.undo.last_mut() {
            // Not into the saved entry: undo must still be able to get back to the saved state.
            if *last_tag == tag && now.duration_since(*time).as_secs_f32() < 1.0 && self.redo.is_empty() && !at_saved {
                **last_after = after;
                *time = now;
                return;
            }
        }
        self.push(Edit::Objects { before: Box::new(before), after: Box::new(after), tag, time: now });
    }

    /// Records a whole-map change (resize, layer operations).
    pub fn record_map(&mut self, before: Map, assets: &Assets) {
        let after = self.map.clone();
        self.push(Edit::Map { before: Box::new(before), after: Box::new(after) });
        self.rebuild_overview(assets);
    }

    /// Places a structure with its top-left corner at tile `(x, y)` as one undo step. Returns
    /// the uids of the entities it added.
    pub fn place_structure(&mut self, def: &rustpg2d_engine::defs::StructureDef, x: i32, y: i32, assets: &Assets) -> Vec<u32> {
        self.end_stroke();
        let before = Objects::of(&self.map);
        self.place_recorded(def, x, y, before, "structure", assets)
    }

    /// Empties a rectangle of tiles `(x, y, w, h)` as one undo step: the painted ground, the
    /// decorations and entities whose feet are inside (the ones `StructureDef::capture`
    /// copies) and the collision overrides (back to automatic). Zones stay.
    pub fn clear_region(&mut self, rect: (u32, u32, u32, u32), assets: &Assets) {
        use rustpg2d_engine::defs::StructureDef;
        self.end_stroke();
        let before = Objects::of(&self.map);
        let (x, y, w, h) = rect;
        let ts = rustpg2d_engine::project::TILE_SIZE as f32;
        let r = gfx::math::Rect::new(x as f32 * ts, y as f32 * ts, w as f32 * ts, h as f32 * ts);
        self.map.entities.retain(|e| !(e.x >= r.x && e.x < r.right() && e.y >= r.y && e.y < r.bottom()));
        let n = (w * h) as usize;
        let cells = (n as u32 * rustpg2d_engine::ground::CELLS * rustpg2d_engine::ground::CELLS) as usize;
        let blank = StructureDef { w, h, ground: vec![StructureDef::CLEAR_GROUND; cells], clear: vec![1; n], collision: vec![StructureDef::COLLISION_AUTO; n], ..Default::default() };
        self.place_recorded(&blank, x as i32, y as i32, before, "clear", assets);
    }

    /// Places `def` and records the ground, decoration, collision and object changes since
    /// `before` as one `Edit::Group`.
    fn place_recorded(&mut self, def: &rustpg2d_engine::defs::StructureDef, x: i32, y: i32, before: Objects, tag: &'static str, assets: &Assets) -> Vec<u32> {
        let p = def.place(&mut self.map, x, y);
        let mut group: Vec<Edit> = Vec::new();
        if !p.ground.is_empty() {
            self.ground_overview(assets, p.ground.iter().map(|c| (c.0, c.1)));
            group.push(Edit::Ground { changes: p.ground });
        }
        if !p.decor_added.is_empty() || !p.decor_removed.is_empty() {
            self.overview.decor_stale = true;
            group.push(Edit::Decor { added: p.decor_added, removed: p.decor_removed });
        }
        if !p.collision.is_empty() {
            group.push(Edit::Collision { changes: p.collision });
        }
        let after = Objects::of(&self.map);
        if after != before {
            // (Registering materials and props on the map is part of this too: those lists
            // aren't undone, an unused slot is harmless and dropped when saving.)
            group.push(Edit::Objects { before: Box::new(before), after: Box::new(after), tag, time: Instant::now() });
        }
        if !group.is_empty() {
            self.push(Edit::Group(group));
        }
        p.entities
    }

    // ------------------------------------------------------------ undo / redo

    fn apply(&mut self, e: &Edit, forward: bool, assets: &Assets) {
        match e {
            Edit::Collision { changes } => {
                for &(x, y, old, new) in changes {
                    self.map.collision[(y * self.map.width + x) as usize] = if forward { new } else { old };
                }
            }
            Edit::Ground { changes } => {
                for &(x, y, old, new) in changes {
                    self.map.ground.set(x, y, if forward { new } else { old });
                }
                self.ground_overview(assets, changes.iter().map(|c| (c.0, c.1)));
            }
            Edit::Decor { added, removed } => {
                let (gone, back) = if forward { (removed, added) } else { (added, removed) };
                for d in gone {
                    remove_one(&mut self.map.decor, d);
                }
                self.map.decor.extend(back.iter().copied());
                self.overview.decor_stale = true;
            }
            Edit::Objects { before, after, .. } => (if forward { after } else { before }).apply(&mut self.map),
            Edit::Map { before, after } => {
                self.map = (if forward { after } else { before }).as_ref().clone();
                self.rebuild_overview(assets);
            }
            Edit::Group(edits) => {
                if forward {
                    edits.iter().for_each(|e| self.apply(e, true, assets));
                } else {
                    edits.iter().rev().for_each(|e| self.apply(e, false, assets));
                }
            }
        }
    }

    /// Records painted ground cells (`(x, y, old)` from the `Ground` painting functions, which
    /// already set them to `new`) as one undo step.
    pub fn record_ground(&mut self, cells: Vec<(i32, i32, u8)>, new: u8, assets: &Assets) {
        if cells.is_empty() {
            return;
        }
        self.end_stroke();
        self.ground_overview(assets, cells.iter().map(|c| (c.0, c.1)));
        self.push(Edit::Ground { changes: cells.into_iter().map(|(x, y, old)| (x, y, old, new)).collect() });
    }

    /// Records what a Build tool step changed as one undo step: painted ground cells (`(x, y,
    /// old)`, now `new`), decorations added and removed, and the objects since `before` (a door
    /// put into a wall).
    pub fn record_build(&mut self, cells: Vec<(i32, i32, u8)>, new: u8, added: Vec<rustpg2d_engine::decor::Decor>, removed: Vec<rustpg2d_engine::decor::Decor>, before: Option<Objects>, assets: &Assets) {
        self.end_stroke();
        let mut group = Vec::new();
        if !cells.is_empty() {
            self.ground_overview(assets, cells.iter().map(|c| (c.0, c.1)));
            group.push(Edit::Ground { changes: cells.into_iter().map(|(x, y, old)| (x, y, old, new)).collect() });
        }
        if !added.is_empty() || !removed.is_empty() {
            self.overview.decor_stale = true;
            group.push(Edit::Decor { added, removed });
        }
        if let Some(before) = before {
            let after = Objects::of(&self.map);
            if after != before {
                group.push(Edit::Objects { before: Box::new(before), after: Box::new(after), tag: "build", time: Instant::now() });
            }
        }
        if !group.is_empty() {
            self.push(Edit::Group(group));
        }
    }

    /// Records decorations already added to / removed from the map as one undo step.
    pub fn record_decor(&mut self, added: Vec<rustpg2d_engine::decor::Decor>, removed: Vec<rustpg2d_engine::decor::Decor>) {
        if added.is_empty() && removed.is_empty() {
            return;
        }
        self.end_stroke();
        self.overview.decor_stale = true;
        self.push(Edit::Decor { added, removed });
    }

    /// The overview pixels of the tiles holding these cells.
    fn ground_overview(&mut self, assets: &Assets, cells: impl Iterator<Item = (i32, i32)>) {
        let n = rustpg2d_engine::ground::CELLS as i32;
        let tiles: std::collections::BTreeSet<(i32, i32)> = cells.map(|(x, y)| (x.div_euclid(n), y.div_euclid(n))).collect();
        for (x, y) in tiles {
            if self.map.in_bounds(x, y) {
                self.overview.update_tile(&self.map, assets, x as u32, y as u32);
            }
        }
    }

    pub fn undo(&mut self, assets: &Assets) {
        self.end_stroke();
        if let Some((id, e)) = self.undo.pop() {
            self.apply(&e, false, assets);
            self.redo.push((id, e));
        }
    }

    pub fn redo(&mut self, assets: &Assets) {
        self.end_stroke();
        if let Some((id, e)) = self.redo.pop() {
            self.apply(&e, true, assets);
            self.undo.push((id, e));
        }
    }
}

/// Removes the first decoration equal to `d` (decorations have no ids; order doesn't matter).
pub fn remove_one(list: &mut Vec<rustpg2d_engine::decor::Decor>, d: &rustpg2d_engine::decor::Decor) -> bool {
    match list.iter().position(|x| x == d) {
        Some(i) => {
            list.swap_remove(i);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::entity::{EntityInstance, EntityKind};

    fn assets() -> Assets {
        Assets::empty(rustpg2d_engine::project::TILE_SIZE as f32)
    }

    #[test]
    fn collision_and_object_edits() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        d.paint_collision(2, 2, 1);
        d.end_stroke();
        assert_eq!(d.map.collision[2 * 8 + 2], 1);

        let before = Objects::of(&d.map);
        let uid = d.map.alloc_uid();
        d.map.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 8.0, 15.0));
        d.record_objects(before, "place");
        // Two quick inspector edits merge into one undo step.
        for name in ["a", "ab"] {
            let before = Objects::of(&d.map);
            d.map.entities[0].name = name.into();
            d.record_objects(before, "inspector");
        }
        d.undo(&a);
        assert_eq!(d.map.entities[0].name, "");
        d.undo(&a);
        assert!(d.map.entities.is_empty());
        d.undo(&a);
        assert_eq!(d.map.collision[2 * 8 + 2], 0);
        // Unchanged snapshots don't create history.
        let before = Objects::of(&d.map);
        d.record_objects(before, "noop");
        d.redo(&a);
        assert_eq!(d.map.collision[2 * 8 + 2], 1);
    }

    #[test]
    fn dirty_follows_the_saved_history_position() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        assert!(!d.is_dirty());
        let paint = |d: &mut MapDoc, x| {
            d.paint_collision(x, 0, 1);
            d.end_stroke();
        };
        paint(&mut d, 1);
        assert!(d.is_dirty());
        d.undo(&a);
        assert!(!d.is_dirty(), "undo back to the loaded state is clean");
        d.redo(&a);
        assert!(d.is_dirty());
        d.mark_saved();
        paint(&mut d, 2);
        d.undo(&a);
        assert!(!d.is_dirty(), "undo back to the saved state is clean");
        d.undo(&a);
        assert!(d.is_dirty(), "undo past the saved state is a change");
        d.redo(&a);
        assert!(!d.is_dirty());
        // A new edit after undoing past the save drops the saved state from the redo list.
        d.undo(&a);
        paint(&mut d, 3);
        d.undo(&a);
        assert!(d.is_dirty());
        // Inspector edits merge, but never into the saved entry.
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        let uid = d.map.alloc_uid();
        let before = Objects::of(&d.map);
        d.map.entities.push(EntityInstance::new(uid, EntityKind::Prop, "chest", 8.0, 15.0));
        d.record_objects(before, "inspector");
        d.mark_saved();
        let before = Objects::of(&d.map);
        d.map.entities[0].name = "x".into();
        d.record_objects(before, "inspector");
        assert!(d.is_dirty());
        d.undo(&a);
        assert!(!d.is_dirty() && d.map.entities.len() == 1);
        // Changes outside the history stay dirty until saved.
        d.mark_changed();
        assert!(d.is_dirty());
        d.clear_history();
        assert!(d.is_dirty());
        d.mark_saved();
        d.clear_history();
        assert!(!d.is_dirty());
    }

    #[test]
    fn placing_a_structure_is_one_undo_step() {
        use rustpg2d_engine::decor::{Decor, DecorLayer};
        let a = assets();
        let mut src = Map::new("s", 4, 4);
        let wood = src.material_value("wood").unwrap();
        for cx in 0..12 {
            src.ground.set(cx, 5, wood);
        }
        let wall = src.decor_slot("wall");
        for x in 0..3 {
            src.decor.push(Decor { def: wall, x: x * 32 + 16, y: 32, flip: false, state: 0, layer: DecorLayer::Sorted });
        }
        src.set_collision(1, 1, rustpg2d_engine::map::CollisionOverride::Walkable);
        let uid = src.alloc_uid();
        src.entities.push(EntityInstance::new(uid, EntityKind::Prop, "barrel", 48.0, 63.0));
        let def = rustpg2d_engine::defs::StructureDef::capture(&src, "s", "S", (0, 0, 3, 2), true);

        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        let tree = d.map.decor_slot("tree");
        d.map.decor.push(Decor { def: tree, x: 3 * 32 + 16, y: 4 * 32, flip: false, state: 0, layer: DecorLayer::Sorted });
        d.mark_saved();
        let before = (d.map.ground.get(2 * 4, 3 * 4 + 5), d.map.decor.clone(), d.map.collision.clone(), d.map.entities.len());
        let added = d.place_structure(&def, 2, 3, &a);
        assert_eq!(added.len(), 1);
        assert!(d.is_dirty() && d.map.decor.len() == 3, "the tree is cleared, three walls stand");
        d.undo(&a);
        assert_eq!((d.map.ground.get(2 * 4, 3 * 4 + 5), d.map.decor.clone(), d.map.collision.clone(), d.map.entities.len()), before);
        assert!(!d.is_dirty());
        d.redo(&a);
        assert_eq!(d.map.entities.len(), 1);
        assert_eq!(d.map.material_of(d.map.ground.get(2 * 4, 3 * 4 + 5)), Some("wood"));
        assert_eq!(d.map.collision_at(3, 4), rustpg2d_engine::map::CollisionOverride::Walkable);
    }

    #[test]
    fn cut_and_paste_a_region() {
        use rustpg2d_engine::decor::{Decor, DecorLayer};
        use rustpg2d_engine::defs::StructureDef;
        use rustpg2d_engine::map::CollisionOverride;
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 10, 10), &a);
        let grass = d.map.material_value("grass").unwrap();
        let sand = d.map.material_value("sand").unwrap();
        d.map.ground.fill(grass);
        d.map.ground.set(2 * 4, 2 * 4, sand);
        let rock = d.map.decor_slot("rock");
        d.map.decor.push(Decor { def: rock, x: 3 * 32 + 16, y: 3 * 32, flip: false, state: 0, layer: DecorLayer::Sorted });
        d.map.set_collision(2, 3, CollisionOverride::Solid);
        let (inside, outside) = (d.map.alloc_uid(), d.map.alloc_uid());
        d.map.entities.push(EntityInstance::new(inside, EntityKind::Prop, "barrel", 3.5 * 32.0, 4.0 * 32.0 - 1.0));
        d.map.entities.push(EntityInstance::new(outside, EntityKind::Prop, "barrel", 8.5 * 32.0, 9.0 * 32.0 - 1.0));
        d.mark_saved();

        // Cut (2, 2, 2x2): copy, then one undo step empties it.
        let clip = StructureDef::capture(&d.map, "clip", "clip", (2, 2, 2, 2), true);
        d.clear_region((2, 2, 2, 2), &a);
        assert_eq!((d.map.ground.get(2 * 4, 2 * 4), d.map.decor.len()), (0, 0));
        assert_eq!(d.map.collision_at(2, 3), CollisionOverride::Auto);
        assert_eq!(d.map.entities.iter().map(|e| e.uid).collect::<Vec<_>>(), vec![outside]);
        assert_eq!(d.map.ground.get(4 * 4, 2 * 4), grass, "outside the region");

        // Paste at (6, 0): an exact copy, the cut is still undoable separately.
        let added = d.place_structure(&clip, 6, 0, &a);
        assert_eq!(added.len(), 1);
        assert_eq!((d.map.ground.get(6 * 4, 0), d.map.ground.get(7 * 4, 0)), (sand, grass));
        assert_eq!((d.map.decor.len(), d.map.decor[0].x, d.map.decor[0].y), (1, 7 * 32 + 16, 32));
        assert_eq!(d.map.collision_at(6, 1), CollisionOverride::Solid);
        let e = d.map.entity(added[0]).unwrap();
        assert_eq!((e.x, e.y), (7.5 * 32.0, 2.0 * 32.0 - 1.0));
        assert!(d.overview.decor_stale);
        d.undo(&a);
        d.undo(&a);
        assert_eq!((d.map.ground.get(2 * 4, 2 * 4), d.map.ground.get(6 * 4, 0)), (sand, grass));
        assert_eq!((d.map.decor.len(), d.map.decor[0].x), (1, 3 * 32 + 16));
        assert_eq!(d.map.collision_at(2, 3), CollisionOverride::Solid);
        assert_eq!(d.map.entities.len(), 2);
        assert!(!d.is_dirty());
    }

    #[test]
    fn whole_map_edits() {
        let a = assets();
        let mut d = MapDoc::new(Map::new("t", 8, 8), &a);
        let before = d.map.clone();
        d.map.resize(30, 4);
        d.record_map(before, &a);
        assert_eq!(d.overview.image.width, 30);
        d.undo(&a);
        assert_eq!((d.map.width, d.overview.image.width), (8, 8));
    }
}
