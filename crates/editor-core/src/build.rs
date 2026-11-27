//! Building on a map (the Build tool): walls painted along tile edges, rooms, houses and roofs
//! dragged as rectangles, doors and windows clicked into walls. Walls are props drawn as blocks
//! or thin models (a fence); north-to-south runs are the same prop turned a quarter. Rooms paint
//! their floor and put walls on their back sides (or all four); houses and roofs are new
//! building props in the look of an existing one, sized to the rectangle; a house dragged
//! against another one is joined to it (an L-shaped house under one roof).
//!
//! Each thing built is one step of the history: the map's change and, for houses and roofs, the
//! new props with it.

use std::collections::BTreeSet;

use gfx::prelude::{vec2, Rect, Vec2};
use rustpg2d_engine::decor::{Decor, DecorLayer, DecorLook};
use rustpg2d_engine::defs::{BuildingShape, Database, Join, Opening, PropDef, Ridge, RoofKind, SpriteRef, WallSide};
use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::map::Map;
use rustpg2d_engine::render::iso;

use crate::workspace::Workspace;

/// The rectangles (tiles: x, y, w, h) a set of tiles is cut into for roofs: the largest one first,
/// then the largest of the rest, until none is left.
pub fn cut_into_rects(cells: &std::collections::BTreeSet<(i32, i32)>) -> Vec<(i32, i32, i32, i32)> {
    let mut left = cells.clone();
    let mut out = Vec::new();
    while !left.is_empty() {
        let mut best = (0, 0, 0, 0);
        for &(x, y) in &left {
            let mut w = 0;
            while left.contains(&(x + w, y)) {
                w += 1;
            }
            // Each width from the widest down, as tall as all its rows reach.
            let mut width = w;
            while width > 0 {
                let mut h = 1;
                while (0..width).all(|k| left.contains(&(x + k, y + h))) {
                    h += 1;
                }
                let better = width * h > best.2 * best.3 || width * h == best.2 * best.3 && (width - h).abs() < (best.2 - best.3).abs();
                if better {
                    best = (x, y, width, h);
                }
                width -= 1;
            }
        }
        let (x, y, w, h) = best;
        for ty in y..y + h {
            for tx in x..x + w {
                left.remove(&(tx, ty));
            }
        }
        out.push(best);
    }
    out
}

/// Props that make walls: blocks (stone, timber, plaster…) and thin models (fences) that line up
/// on the tile grid; doors aren't.
pub fn is_wall(p: &PropDef) -> bool {
    !p.door && p.snap >= 32 && (p.block > 0.0 || (!p.model.is_empty() && p.hitbox[0] >= 30.0 && p.hitbox[1] <= 8.0))
}

/// Wall props with a window in them.
pub fn is_window(p: &PropDef) -> bool {
    is_wall(p) && (p.id.contains("window") || p.name.to_lowercase().contains("window"))
}

pub fn is_house(p: &PropDef) -> bool {
    p.building.as_ref().is_some_and(|b| b.walls)
}

/// Whether wall prop `p` stands on a tile's edge (thin) rather than filling the tile.
pub fn thin(p: &PropDef) -> bool {
    p.hitbox[1] <= 12.0
}

/// The tile edge nearest to `v` (world pixels).
pub fn edge(v: f32, ts: f32) -> f32 {
    (v / ts).round() * ts
}

/// Where painting wall `p` is at `world`: the nearest tile corner (thin walls) or the tile under
/// it (thick ones).
pub fn wall_point(p: &PropDef, world: Vec2, ts: f32) -> (i32, i32) {
    if thin(p) {
        ((world.x / ts).round() as i32, (world.y / ts).round() as i32)
    } else {
        ((world.x / ts).floor() as i32, (world.y / ts).floor() as i32)
    }
}

/// A wall point in world pixels: the corner, or the tile's middle.
pub fn point_px(p: &PropDef, (x, y): (i32, i32), ts: f32) -> Vec2 {
    if thin(p) {
        vec2(x as f32 * ts, y as f32 * ts)
    } else {
        vec2((x as f32 + 0.5) * ts, (y as f32 + 0.5) * ts)
    }
}

/// The straight runs from wall point `a` to `b`: one, or two around a corner (the longer way
/// first).
pub fn path_runs(a: (i32, i32), b: (i32, i32)) -> Vec<((i32, i32), (i32, i32))> {
    if a == b {
        return vec![];
    }
    if a.0 == b.0 || a.1 == b.1 {
        return vec![(a, b)];
    }
    let corner = if (b.0 - a.0).abs() >= (b.1 - a.1).abs() { (b.0, a.1) } else { (a.0, b.1) };
    vec![(a, corner), (corner, b)]
}

/// Places a wall of `prop` from `a` to `b` (world pixels, along the longer axis). Thin walls run
/// along the tile edge nearest to the drag, north-to-south runs turned a quarter; `outside`: the
/// wall stands south / east of that edge instead of north / west. Thick walls fill the tiles the
/// drag passes. Skips spots a piece of it stands on already; returns the decorations added.
pub fn wall_run(map: &mut Map, p: &PropDef, a: Vec2, b: Vec2, outside: bool, ts: f32) -> Vec<Decor> {
    let slot = map.decor_slot(&p.id);
    let horizontal = (b.x - a.x).abs() >= (b.y - a.y).abs();
    let mut spots: Vec<(i32, i32, u8)> = Vec::new();
    if thin(p) {
        let depth = p.hitbox[1];
        if horizontal {
            let y = edge(a.y, ts) + if outside { depth } else { 0.0 };
            let (t0, t1) = ((a.x.min(b.x) / ts).round() as i32, (a.x.max(b.x) / ts).round() as i32);
            for t in t0..t1.max(t0 + 1) {
                spots.push(((t as f32 * ts + ts / 2.0) as i32, y as i32, 0));
            }
        } else {
            let x = edge(a.x, ts) + if outside { depth / 2.0 } else { -depth / 2.0 };
            let (t0, t1) = ((a.y.min(b.y) / ts).round() as i32, (a.y.max(b.y) / ts).round() as i32);
            for t in t0..t1.max(t0 + 1) {
                spots.push((x as i32, ((t + 1) as f32 * ts) as i32, 1));
            }
        }
    } else {
        let (ta, tb) = ((a / ts).floor(), (b / ts).floor());
        let (x0, x1, y0, y1) = if horizontal { (ta.x.min(tb.x), ta.x.max(tb.x), ta.y, ta.y) } else { (ta.x, ta.x, ta.y.min(tb.y), ta.y.max(tb.y)) };
        for ty in y0 as i32..=y1 as i32 {
            for tx in x0 as i32..=x1 as i32 {
                spots.push(((tx as f32 * ts + ts / 2.0) as i32, ((ty + 1) as f32 * ts) as i32, 0));
            }
        }
    }
    let mut added = Vec::new();
    for (x, y, turn) in spots {
        if map.decor.iter().any(|d| d.def == slot && d.x == x && d.y == y) {
            continue;
        }
        let mut d = Decor { def: slot, x, y, flip: false, state: 0, layer: DecorLayer::Sorted };
        d.set_turn(turn);
        map.decor.push(d);
        added.push(d);
    }
    added
}

/// Screen lift (at zoom 1) of a point one world pixel above the ground.
const LIFT: f32 = iso::HEIGHT * iso::UPRIGHT;

/// Where the line of sight through ground point `g` (what the cursor shows on the ground) first
/// meets box `r` raised `h` world pixels: the height of that point and its face (0 = the top,
/// 1 = the south face, 2 = the east face). Seen from straight above (`iso` false): `g` inside
/// `r` meets its top.
pub fn sight_hit(g: Vec2, r: Rect, h: f32, iso: bool) -> Option<(f32, u8)> {
    if !iso {
        return r.contains(g).then_some((h, 0));
    }
    // Points at height z on the line: g + (z, z) * LIFT.
    let (zx0, zx1) = ((r.x - g.x) / LIFT, (r.x + r.w - g.x) / LIFT);
    let (zy0, zy1) = ((r.y - g.y) / LIFT, (r.y + r.h - g.y) / LIFT);
    let lo = zx0.max(zy0).max(0.0);
    let hi = zx1.min(zy1).min(h);
    if lo > hi {
        return None;
    }
    let face = if hi >= h - 1e-3 {
        0
    } else if (hi - zx1).abs() < 1e-3 {
        2
    } else {
        1
    };
    Some((hi, face))
}

/// How much higher every further storey makes a house (world pixels).
const STOREY: f32 = 48.0;

/// A building shape for a house `w` x `d` world pixels in the look of `style` with `storeys`
/// storeys: its walls, roof and pictures, a door in the middle of the front (the south wall, else
/// the first side not in `closed`: sides joined to another building) and windows along the walls,
/// on every storey.
pub fn house_shape(style: &BuildingShape, w: f32, d: f32, storeys: u8, closed: &[WallSide]) -> BuildingShape {
    let door = style.openings.iter().find(|o| o.sill <= 0.0).cloned();
    let window = style.openings.iter().find(|o| o.sill > 0.0).cloned();
    let storeys = storeys.clamp(1, 3);
    let mut openings = Vec::new();
    let open = |s: WallSide| !closed.contains(&s);
    let len = |s: WallSide| if matches!(s, WallSide::South | WallSide::North) { w } else { d };
    let front = [WallSide::South, WallSide::East, WallSide::West, WallSide::North].into_iter().find(|s| open(*s)).unwrap_or(WallSide::South);
    if let Some(mut o) = door {
        o.side = front;
        o.at = len(front) / 2.0;
        openings.push(o);
    }
    if let Some(win) = window {
        let step = 64.0;
        let n = ((w - 48.0) / step).floor().max(0.0) as i32;
        for floor in 0..storeys {
            let sill = win.sill + floor as f32 * STOREY;
            // (Each wall's middle stays free for the door on the ground floor.)
            let free = |side: WallSide, at: f32| floor > 0 || side != front || (at - len(side) / 2.0).abs() > 28.0;
            for side in [WallSide::South, WallSide::North] {
                if !open(side) || side == WallSide::North && floor == 0 {
                    continue;
                }
                for k in 0..=n {
                    let at = 32.0 + k as f32 * step;
                    if free(side, at) && at < w - 16.0 {
                        openings.push(Opening { side, at, sill, ..win.clone() });
                    }
                }
            }
            if d >= 96.0 {
                for side in [WallSide::East, WallSide::West] {
                    if open(side) && free(side, d / 2.0) {
                        openings.push(Opening { side, at: d / 2.0, sill, ..win.clone() });
                    }
                }
            }
        }
    }
    let chimneys = style.chimneys.first().map(|c| vec![rustpg2d_engine::defs::Chimney { at: [w * 0.75, d / 2.0], ..c.clone() }]).unwrap_or_default();
    BuildingShape { openings, chimneys, height: style.height + STOREY * (storeys - 1) as f32, joins: Vec::new(), walls: true, ..style.clone() }
}

/// The side of footprint `a` that touches footprint `b` along at least `min` pixels, if any.
pub fn touching(a: Rect, b: Rect, min: f32) -> Option<WallSide> {
    let near = |p: f32, q: f32| (p - q).abs() < 1.0;
    let over_x = a.right().min(b.right()) - a.x.max(b.x);
    let over_y = a.bottom().min(b.bottom()) - a.y.max(b.y);
    if over_x >= min && near(a.bottom(), b.y) {
        Some(WallSide::South)
    } else if over_x >= min && near(a.y, b.bottom()) {
        Some(WallSide::North)
    } else if over_y >= min && near(a.right(), b.x) {
        Some(WallSide::East)
    } else if over_y >= min && near(a.x, b.right()) {
        Some(WallSide::West)
    } else {
        None
    }
}

/// Rectangle `r` cut back to the edge of `other` where it reaches into it (a house dragged from
/// inside another one: the rest of it, against that one's wall); the largest of the cuts.
pub fn cut_against(r: Rect, other: Rect) -> Rect {
    let Some(i) = r.intersect(other) else { return r };
    if i.w <= 0.0 || i.h <= 0.0 || (i.w >= r.w && i.h >= r.h) {
        return r;
    }
    let cuts = [
        (r.bottom() > other.bottom()).then(|| Rect::new(r.x, other.bottom(), r.w, r.bottom() - other.bottom())),
        (r.y < other.y).then(|| Rect::new(r.x, r.y, r.w, other.y - r.y)),
        (r.right() > other.right()).then(|| Rect::new(other.right(), r.y, r.right() - other.right(), r.h)),
        (r.x < other.x).then(|| Rect::new(r.x, r.y, other.x - r.x, r.h)),
    ];
    cuts.into_iter().flatten().max_by(|a, b| (a.w * a.h).total_cmp(&(b.w * b.h))).unwrap_or(r)
}

fn opposite(s: WallSide) -> WallSide {
    match s {
        WallSide::South => WallSide::North,
        WallSide::North => WallSide::South,
        WallSide::East => WallSide::West,
        WallSide::West => WallSide::East,
    }
}

/// Removes the openings of `shape` (footprint `r`) on its side `side` where footprint `other`
/// covers it.
fn close_side(shape: &mut BuildingShape, r: Rect, side: WallSide, other: Rect) {
    let (lo, hi) = match side {
        WallSide::South | WallSide::North => (other.x - r.x, other.right() - r.x),
        WallSide::East | WallSide::West => (other.y - r.y, other.bottom() - r.y),
    };
    shape.openings.retain(|o| o.side != side || o.at + o.size[0] / 2.0 < lo || o.at - o.size[0] / 2.0 > hi);
}

/// A new prop id from `base` (its `_built_N` / `_roof_N` ending dropped) and `what`, unused in
/// `db` and not in `taken`.
fn new_id(db: &Database, base: &str, what: &str, taken: &[&str]) -> String {
    let stem = match base.rfind(&format!("_{what}_")) {
        Some(i) if base[i + what.len() + 2..].chars().all(|c| c.is_ascii_digit()) => &base[..i],
        _ => base,
    };
    (1..).map(|k| format!("{stem}_{what}_{k}")).find(|c| db.prop(c).is_none() && !taken.contains(&c.as_str())).unwrap()
}

/// What a click in the Doors mode puts into a wall.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Put {
    #[default]
    Door,
    Window,
    /// A wall piece taken out (houses: none).
    Gap,
}

/// What the pointer shows of the things built: the scenery's place in the map's list, its prop
/// and footprint, how high up the line of sight meets it and on which face (0 = the top, 1 =
/// the south face, 2 = the east face).
pub struct Hit {
    pub index: usize,
    pub prop: PropDef,
    pub rect: Rect,
    pub z: f32,
    pub face: u8,
}

/// The rectangle (world pixels, on tile edges) a drag from `a` to `b` covers: at least a tile.
pub fn drag_rect(a: Vec2, b: Vec2, ts: f32) -> Rect {
    let (x0, y0) = ((a.x.min(b.x) / ts).floor() * ts, (a.y.min(b.y) / ts).floor() * ts);
    let (x1, y1) = ((a.x.max(b.x) / ts).ceil() * ts, (a.y.max(b.y) / ts).ceil() * ts);
    Rect::new(x0, y0, (x1 - x0).max(ts), (y1 - y0).max(ts))
}

/// The tiles of rectangle `r` (world pixels).
pub fn tiles_in(r: Rect, ts: f32) -> BTreeSet<(i32, i32)> {
    let (x0, y0) = ((r.x / ts).round() as i32, (r.y / ts).round() as i32);
    let (w, h) = (((r.w / ts).round() as i32).max(1), ((r.h / ts).round() as i32).max(1));
    (y0..y0 + h).flat_map(|y| (x0..x0 + w).map(move |x| (x, y))).collect()
}

fn building_key(p: &PropDef) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}{:?}{}{}", p.hitbox, p.building, p.model, p.name).hash(&mut h);
    h.finish()
}

impl Workspace {
    /// The building props' models follow their definitions: built again where a shape, a
    /// footprint or a name changed since the last look (a form, an undo), dropped where the
    /// prop is gone or no building any more.
    pub fn sync_buildings(&mut self) {
        if self.gpu == crate::Gpu::No {
            return;
        }
        let now: std::collections::HashMap<String, u64> = self.project.db.props.iter().filter(|p| p.building.is_some()).map(|p| (p.id.clone(), building_key(p))).collect();
        let Some(was) = self.buildings.replace(now.clone()) else { return };
        let changed: Vec<&String> = now.iter().filter(|(id, key)| was.get(*id) != Some(*key)).map(|(id, _)| id).chain(was.keys().filter(|id| !now.contains_key(*id))).collect();
        for id in changed {
            let p = self.project.db.prop(id).cloned().unwrap_or_else(|| PropDef::fresh(id));
            self.assets.rebuild_building(&p, &self.project.db);
            self.stale = true;
        }
    }

    /// New or changed building props: their models are built and their pictures rendered from
    /// them (what the editor shows of them and how big the game takes them to be).
    fn built_props(&mut self, ids: &[String]) {
        self.project.db.reindex();
        self.defs_dirty = true;
        self.stale = true;
        if self.gpu == crate::Gpu::No {
            return;
        }
        for id in ids {
            if let Some(p) = self.project.db.prop(id).cloned() {
                self.assets.rebuild_building(&p, &self.project.db);
            }
        }
        for id in ids {
            self.prop_pictures(Some(id));
        }
        self.buildings = Some(self.project.db.props.iter().filter(|p| p.building.is_some()).map(|p| (p.id.clone(), building_key(p))).collect());
    }

    /// The prop and footprint (world pixels) of a piece of scenery.
    pub fn build_footprint(&self, d: &Decor) -> Option<(PropDef, Rect)> {
        let doc = self.doc.as_ref()?;
        let id = doc.map.decor_defs.get(d.def as usize)?;
        let p = self.project.db.prop(id)?.clone();
        let (x, y, w, h) = DecorLook::of(&self.project.db, id, 0).rect_at(d);
        Some((p, Rect::new(x, y, w, h)))
    }

    /// The scenery the pointer (ground point `g`; `iso`: in the game's view, not from above)
    /// shows nearest the viewer among those `keep` accepts.
    pub fn build_pick(&self, g: Vec2, iso: bool, keep: impl Fn(&PropDef) -> bool) -> Option<Hit> {
        let doc = self.doc.as_ref()?;
        let db = &self.project.db;
        let mut best: Option<(usize, &PropDef, Rect, f32, u8)> = None;
        for (index, d) in doc.map.decor.iter().enumerate() {
            let Some(prop) = doc.map.decor_defs.get(d.def as usize).and_then(|id| db.prop(id)) else { continue };
            if !keep(prop) {
                continue;
            }
            let [w, h] = if prop.turns() && d.turn() % 2 == 1 { [prop.hitbox[1], prop.hitbox[0]] } else { prop.hitbox };
            let rect = Rect::new(d.x as f32 - w / 2.0, d.y as f32 - h, w, h);
            let height = match &prop.building {
                Some(b) => b.height,
                None => prop.block.max(8.0),
            };
            let Some((z, face)) = sight_hit(g, rect, height, iso) else { continue };
            if best.as_ref().is_none_or(|b| z > b.3) {
                best = Some((index, prop, rect, z, face));
            }
        }
        best.map(|(index, prop, rect, z, face)| Hit { index, prop: prop.clone(), rect, z, face })
    }

    fn wall_prop(&self, wall: &str) -> Result<PropDef, String> {
        self.project.db.prop(wall).filter(|p| is_wall(p)).cloned().ok_or_else(|| if wall.is_empty() { "pick a wall first".to_string() } else { format!("\"{wall}\" is no wall") })
    }

    /// The look a new house or roof takes: the chosen building, else the first one.
    fn build_style(&self, style: &str) -> Option<PropDef> {
        let db = &self.project.db;
        db.prop(style).filter(|p| is_house(p)).or_else(|| db.props.iter().find(|p| is_house(p))).cloned()
    }

    // ------------------------------------------------------------ walls

    /// A painted wall goes on from wall point `a` to `b` (corners for thin walls, tiles for
    /// thick ones; round a corner when they aren't in line). No step by itself: take
    /// [`Self::snap_map`] before the first stretch and [`Self::commit_map`] after the last.
    /// Returns the pieces added.
    pub fn wall_to(&mut self, wall: &str, a: (i32, i32), b: (i32, i32)) -> usize {
        let ts = self.ts();
        let Ok(p) = self.wall_prop(wall) else { return 0 };
        let Some(doc) = &mut self.doc else { return 0 };
        let runs = if a == b { vec![(a, b)] } else { path_runs(a, b) };
        let mut n = 0;
        for (s, e) in runs {
            if s == e && thin(&p) {
                continue;
            }
            n += wall_run(&mut doc.map, &p, point_px(&p, s, ts), point_px(&p, e, ts), false, ts).len();
        }
        doc.overview.decor_stale |= n > 0;
        n
    }

    /// A straight wall from world point `a` towards `b` (along the longer way). No step by
    /// itself, as [`Self::wall_to`].
    pub fn wall_line(&mut self, wall: &str, a: Vec2, b: Vec2) -> usize {
        let ts = self.ts();
        let Ok(p) = self.wall_prop(wall) else { return 0 };
        let Some(doc) = &mut self.doc else { return 0 };
        let n = wall_run(&mut doc.map, &p, a, b, false, ts).len();
        doc.overview.decor_stale |= n > 0;
        n
    }

    /// A wall of prop `wall` through wall points `points`, as one step. Returns the pieces built.
    pub fn build_walls(&mut self, wall: &str, points: &[(i32, i32)]) -> Result<usize, String> {
        let p = self.wall_prop(wall)?;
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let mut n = 0;
        if let [one] = points {
            n += self.wall_to(wall, *one, *one);
        }
        for w in points.windows(2) {
            n += self.wall_to(wall, w[0], w[1]);
        }
        self.commit_map(*snap, &format!("Build {n} pieces of {}", self.def_label(EntityKind::Prop, &p.id)));
        Ok(n)
    }

    /// Takes the wall piece the pointer shows away.
    pub fn take_wall(&mut self, g: Vec2, iso: bool) -> Result<String, String> {
        let Some(hit) = self.build_pick(g, iso, is_wall) else { return Err("there is no wall piece here".into()) };
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        if let Some(doc) = &mut self.doc {
            doc.map.decor.remove(hit.index);
        }
        let name = self.def_label(EntityKind::Prop, &hit.prop.id);
        self.commit_map(*snap, &format!("Take a piece of {name} away"));
        Ok(format!("Took a piece of {name} away"))
    }

    // ------------------------------------------------------------ rooms

    /// A room on rectangle `r` (world pixels, on tile edges): its floor of material `floor`
    /// ("" = the ground stays), walls of prop `wall` ("" = none) on its back sides (north and
    /// west) or, `all_sides`, on all four with a doorway in the south wall, and with `roof` (a
    /// building whose look it takes, and its kind) a roof over it.
    pub fn build_room(&mut self, r: Rect, wall: &str, floor: &str, all_sides: bool, roof: Option<(&str, RoofKind)>) -> Result<String, String> {
        let ts = self.ts();
        let wall_def = if wall.is_empty() { None } else { Some(self.wall_prop(wall)?) };
        if roof.is_some_and(|(style, _)| self.build_style(style).is_none()) {
            return Err("this project has no building whose roof to copy: make a prop with a Building shape first".into());
        }
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let (mut cells, mut value) = (Vec::new(), 0);
        if !floor.is_empty() {
            let Some(v) = doc.map.material_value(floor) else { return Err("this map has no room for another material".into()) };
            cells = doc.map.ground.paint_rect((r.x, r.y), (r.x + r.w, r.y + r.h), v);
            value = v;
        }
        let mut n = 0;
        // The floor's tiles, and the tiles a roof covers: with thick walls theirs too.
        let (x0, y0) = ((r.x / ts).round() as i32, (r.y / ts).round() as i32);
        let (x1, y1) = (x0 + ((r.w / ts).round() as i32).max(1) - 1, y0 + ((r.h / ts).round() as i32).max(1) - 1);
        let mut roofed = r;
        match &wall_def {
            Some(p) if thin(p) => {
                // Thin walls stand on the floor's edges.
                let (l, t, rt, bt) = (r.x, r.y, r.x + r.w, r.y + r.h);
                n += wall_run(&mut doc.map, p, vec2(l, t), vec2(rt, t), false, ts).len();
                n += wall_run(&mut doc.map, p, vec2(l, t), vec2(l, bt), false, ts).len();
                if all_sides {
                    // The south wall with a doorway in its middle.
                    let door = ((r.w / ts / 2.0).floor() * ts) + l;
                    if door > l {
                        n += wall_run(&mut doc.map, p, vec2(l, bt), vec2(door, bt), true, ts).len();
                    }
                    if door + ts < rt {
                        n += wall_run(&mut doc.map, p, vec2(door + ts, bt), vec2(rt, bt), true, ts).len();
                    }
                    n += wall_run(&mut doc.map, p, vec2(rt, t), vec2(rt, bt), true, ts).len();
                }
            }
            Some(p) => {
                // Thick walls fill the tiles around the floor, which stays as big as dragged.
                let mid = |x: i32, y: i32| vec2((x as f32 + 0.5) * ts, (y as f32 + 0.5) * ts);
                let (east, south) = if all_sides { (x1 + 1, y1 + 1) } else { (x1, y1) };
                n += wall_run(&mut doc.map, p, mid(x0 - 1, y0 - 1), mid(east, y0 - 1), false, ts).len();
                n += wall_run(&mut doc.map, p, mid(x0 - 1, y0 - 1), mid(x0 - 1, south), false, ts).len();
                if all_sides {
                    let door = x0 + (x1 - x0 + 1) / 2;
                    n += wall_run(&mut doc.map, p, mid(x0 - 1, south), mid(door - 1, south), false, ts).len();
                    n += wall_run(&mut doc.map, p, mid(door + 1, south), mid(east, south), false, ts).len();
                    n += wall_run(&mut doc.map, p, mid(east, y0 - 1), mid(east, south), false, ts).len();
                }
                roofed = Rect::new((x0 - 1) as f32 * ts, (y0 - 1) as f32 * ts, (east - x0 + 2) as f32 * ts, (south - y0 + 2) as f32 * ts);
            }
            None => {}
        }
        let tiles = format!("{} x {}", (r.w / ts) as i32, (r.h / ts) as i32);
        let label = format!("Build a room of {tiles} tiles");
        if let Some((style, kind)) = roof {
            self.roof_over(&tiles_in(roofed, ts), style, kind, wall)?;
        }
        if !self.commit_built(*snap, cells, value, &label) {
            return Err("nothing to build here: the room is there already".into());
        }
        Ok(format!("Built a room of {tiles} tiles ({n} wall pieces)"))
    }

    // ------------------------------------------------------------ houses

    /// A house on rectangle `r` (world pixels, on tile edges) in the look of building prop
    /// `style` with `storeys` storeys: a new prop of that size, its door in the middle of the
    /// front, windows along the walls. Dragged from inside another house it starts at that
    /// one's wall; touching one, it becomes its wing (as high, in its look, one roof over
    /// both). Returns what to tell.
    pub fn build_house(&mut self, r: Rect, style: &str, storeys: u8) -> Result<String, String> {
        let ts = self.ts();
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let mut r = r;
        // Dragged from inside another house: the part outside it.
        let houses: Vec<Rect> = self.doc.as_ref().map_or(Vec::new(), |doc| doc.map.decor.iter().filter_map(|d| self.build_footprint(d).filter(|(p, _)| is_house(p)).map(|(_, fr)| fr)).collect());
        for fr in houses {
            r = cut_against(r, fr);
        }
        let Some(style) = self.build_style(style) else { return Err("this project has no building to build like: make a prop with a Building shape first".into()) };
        // A house it touches: the new one is its wing, in its look.
        let neighbour = self.doc.as_ref().and_then(|doc| {
            doc.map.decor.iter().enumerate().find_map(|(i, d)| {
                let (p, fr) = self.build_footprint(d)?;
                let shape = p.building.as_ref().filter(|s| s.walls && d.turn() == 0)?.clone();
                let side = touching(r, fr, ts)?;
                Some((i, p, shape, fr, side))
            })
        });
        let (look, shape0) = match &neighbour {
            Some((_, p, s, ..)) => (p.clone(), s.clone()),
            None => (style.clone(), style.building.clone().unwrap_or_default()),
        };
        let closed: Vec<WallSide> = neighbour.iter().map(|n| n.4).collect();
        let storeys = if neighbour.is_some() { 1 } else { storeys.clamp(1, 3) };
        let mut shape = house_shape(&shape0, r.w, r.h, storeys, &closed);
        let id = new_id(&self.project.db, &look.id, "built", &[]);
        let mut made = vec![id.clone()];
        let mut joined = String::new();
        if let Some((i, mp, mut ms, fr, side)) = neighbour {
            // The wing: as high, its ridge along its longer side (whichever ridge points into the
            // other building runs on into its roof); each knows the other.
            shape.height = shape0.height;
            shape.ridge = Ridge::Auto;
            close_side(&mut ms, fr, opposite(side), r);
            ms.joins.push(Join { prop: id.clone(), at: [r.x - fr.x, r.y - fr.y] });
            // That house changes: a copy of its prop when others use it.
            let uses = self.doc.as_ref().map_or(0, |doc| doc.map.decor.iter().filter(|d| doc.map.decor_defs.get(d.def as usize) == Some(&mp.id)).count());
            joined = if mp.id.contains("_built_") && uses == 1 { mp.id.clone() } else { new_id(&self.project.db, &mp.id, "built", &[&id]) };
            shape.joins = vec![Join { prop: joined.clone(), at: [fr.x - r.x, fr.y - r.y] }];
            if joined == mp.id {
                if let Some(q) = self.project.db.props.iter_mut().find(|q| q.id == joined) {
                    q.building = Some(ms);
                }
            } else {
                self.project.db.props.push(PropDef { id: joined.clone(), name: format!("{} (joined)", mp.name), building: Some(ms), model: String::new(), ..mp.clone() });
                self.project.db.reindex();
                if let Some(doc) = &mut self.doc {
                    let slot = doc.map.decor_slot(&joined);
                    doc.map.decor[i].def = slot;
                }
            }
            made.push(joined.clone());
        }
        let tiles = format!("{}x{}", (r.w / ts) as i32, (r.h / ts) as i32);
        let name = if storeys > 1 { format!("{} ({tiles}, {storeys} storeys)", look.name) } else { format!("{} ({tiles})", look.name) };
        self.project.db.props.push(PropDef { id: id.clone(), name: name.clone(), hitbox: [r.w, r.h], building: Some(shape), model: String::new(), ..look });
        self.built_props(&made);
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let slot = doc.map.decor_slot(&id);
        doc.map.decor.push(Decor { def: slot, x: (r.x + r.w / 2.0) as i32, y: (r.y + r.h) as i32, flip: false, state: 0, layer: DecorLayer::Sorted });
        self.commit_map(*snap, &format!("Build {name}"));
        Ok(if joined.is_empty() { format!("Built the house '{id}' ({tiles} tiles): its definition changes its doors, windows and roof") } else { format!("Built '{id}' ({tiles} tiles) as a wing of '{joined}': one roof over both") })
    }

    // ------------------------------------------------------------ roofs

    /// The height (world pixels) a roof over tiles `cells` sits at: the tallest wall piece
    /// standing on or beside them, else prop `wall`'s, else a storey.
    fn roof_height(&self, cells: &BTreeSet<(i32, i32)>, wall: &str) -> f32 {
        let ts = self.ts();
        let chosen = self.project.db.prop(wall).filter(|w| w.block > 0.0).map_or(48.0, |w| w.block);
        let Some(doc) = self.doc.as_ref() else { return chosen };
        let near = |r: Rect| cells.iter().any(|&(x, y)| Rect::new((x - 1) as f32 * ts, (y - 1) as f32 * ts, 3.0 * ts, 3.0 * ts).overlaps(&r));
        doc.map.decor.iter().filter_map(|d| self.build_footprint(d)).filter(|(p, r)| is_wall(p) && p.block > 0.0 && near(*r)).map(|(p, _)| p.block).fold(None, |m: Option<f32>, h| Some(m.map_or(h, |m| m.max(h)))).unwrap_or(chosen)
    }

    /// The roofs over `cells` put on the map (no step: the caller's).
    fn roof_over(&mut self, cells: &BTreeSet<(i32, i32)>, style: &str, kind: RoofKind, wall: &str) -> Result<(usize, f32), String> {
        if cells.is_empty() {
            return Err("mark the roof's tiles first".into());
        }
        let ts = self.ts();
        let Some(style) = self.build_style(style) else { return Err("this project has no building whose roof to copy: make a prop with a Building shape first".into()) };
        let height = self.roof_height(cells, wall);
        let rects: Vec<Rect> = cut_into_rects(cells).into_iter().map(|(x, y, w, h)| Rect::new(x as f32 * ts, y as f32 * ts, w as f32 * ts, h as f32 * ts)).collect();
        let mut ids: Vec<String> = Vec::new();
        for _ in &rects {
            let taken: Vec<&str> = ids.iter().map(String::as_str).collect();
            let id = new_id(&self.project.db, &style.id, "roof", &taken);
            ids.push(id);
        }
        let base = style.building.clone().unwrap_or_default();
        for (k, r) in rects.iter().enumerate() {
            let joins = rects.iter().enumerate().filter(|(j, o)| *j != k && touching(*r, **o, ts).is_some()).map(|(j, o)| Join { prop: ids[j].clone(), at: [o.x - r.x, o.y - r.y] }).collect();
            // The biggest part's ridge runs along its longer side; every other part's points into
            // the bigger part it meets (the cut comes biggest first), so their roofs meet.
            let ridge = rects[..k].iter().find_map(|o| touching(*r, *o, ts)).map_or(Ridge::Auto, |side| if matches!(side, WallSide::North | WallSide::South) { Ridge::NorthSouth } else { Ridge::EastWest });
            let shape = BuildingShape { walls: false, height, roof: kind, openings: Vec::new(), chimneys: Vec::new(), joins, base_height: 0.0, ridge, ..base.clone() };
            let tiles = format!("{}x{}", (r.w / ts) as i32, (r.h / ts) as i32);
            self.project.db.props.push(PropDef { id: ids[k].clone(), name: format!("Roof ({tiles})"), hitbox: [r.w, r.h], building: Some(shape), model: String::new(), solid: false, states: Vec::new(), triggers: Vec::new(), ..style.clone() });
        }
        self.built_props(&ids);
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        for (r, id) in rects.iter().zip(&ids) {
            let d = Decor { def: doc.map.decor_slot(id), x: (r.x + r.w / 2.0) as i32, y: (r.y + r.h) as i32, flip: false, state: 0, layer: DecorLayer::Sorted };
            doc.map.decor.push(d);
        }
        Ok((rects.len(), height))
    }

    /// A roof alone over the tiles `cells` (any shape) in the look of building prop `style`,
    /// of kind `kind`, as high as the walls under it (else as prop `wall`): cut into
    /// rectangles, each a roof prop of its own, joined where they touch so their roofs run
    /// into each other.
    pub fn build_roof(&mut self, cells: &BTreeSet<(i32, i32)>, style: &str, kind: RoofKind, wall: &str) -> Result<String, String> {
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let (parts, height) = self.roof_over(cells, style, kind, wall)?;
        self.commit_map(*snap, &format!("Build a roof over {} tiles", cells.len()));
        Ok(format!("Built a roof over {} tiles ({parts} {}) at {height} px", cells.len(), if parts == 1 { "part" } else { "parts" }))
    }

    /// The tiles the walls around ground point `at` enclose, and those the walls stand on: what
    /// a roof over that room covers. An error when the walls don't close it in.
    pub fn roof_fill(&self, at: Vec2) -> Result<BTreeSet<(i32, i32)>, String> {
        let ts = self.ts();
        let Some(doc) = self.doc.as_ref() else { return Err("no map is open".into()) };
        // Thin walls block crossing their tile edge; thick ones fill their tile.
        let mut edges: std::collections::HashSet<((i32, i32), (i32, i32))> = Default::default();
        let mut solid: BTreeSet<(i32, i32)> = Default::default();
        for d in &doc.map.decor {
            let Some((p, r)) = self.build_footprint(d) else { continue };
            if !is_wall(&p) {
                continue;
            }
            if thin(&p) {
                let c = vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
                if r.w >= r.h {
                    // Along a horizontal edge: between the tiles north and south of it.
                    let (x, e) = ((c.x / ts).floor() as i32, (c.y / ts).round() as i32);
                    edges.insert(((x, e - 1), (x, e)));
                } else {
                    let (e, y) = ((c.x / ts).round() as i32, (c.y / ts).floor() as i32);
                    edges.insert(((e - 1, y), (e, y)));
                }
            } else {
                solid.insert((((r.x + r.w / 2.0) / ts).floor() as i32, ((r.y + r.h / 2.0) / ts).floor() as i32));
            }
        }
        // A door put into a wall closes its edge like the piece it took the place of.
        for e in doc.map.entities.iter().filter(|e| e.kind == EntityKind::Prop) {
            use rustpg2d_engine::entity::Facing;
            if !self.project.db.prop(&e.def).is_some_and(|p| p.door) {
                continue;
            }
            if matches!(e.facing, Facing::Left | Facing::Right) {
                // (In a north-to-south wall: it stands where that piece did, its feet at the south end.)
                let (edge, y) = ((e.x / ts).round() as i32, ((e.y - ts / 2.0) / ts).floor() as i32);
                edges.insert(((edge - 1, y), (edge, y)));
            } else {
                let (x, edge) = ((e.x / ts).floor() as i32, (e.y / ts).round() as i32);
                edges.insert(((x, edge - 1), (x, edge)));
            }
        }
        let start = ((at.x / ts).floor() as i32, (at.y / ts).floor() as i32);
        if solid.contains(&start) {
            return Err("click inside the walls, not on them".into());
        }
        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
        let mut seen: BTreeSet<(i32, i32)> = [start].into();
        let mut todo = vec![start];
        let mut walls_on: BTreeSet<(i32, i32)> = Default::default();
        while let Some((x, y)) = todo.pop() {
            if seen.len() > 1200 || x <= 0 || y <= 0 || x >= mw - 1 || y >= mh - 1 {
                return Err("those walls don't close in a room: paint the roof instead, or close the walls".into());
            }
            for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                let key = if n < (x, y) { (n, (x, y)) } else { ((x, y), n) };
                if edges.contains(&key) || seen.contains(&n) {
                    continue;
                }
                if solid.contains(&n) {
                    walls_on.insert(n);
                    continue;
                }
                seen.insert(n);
                todo.push(n);
            }
        }
        seen.extend(walls_on);
        Ok(seen)
    }

    // ------------------------------------------------------------ doors and windows

    /// The building prop scenery `i` shows, ready to be changed: its own prop, or a copy when
    /// other scenery of the map shows it too or it wasn't built here (the scenery then shows
    /// the copy).
    fn own_building(&mut self, i: usize) -> Option<String> {
        let doc = self.doc.as_ref()?;
        let id = doc.map.decor_defs.get(doc.map.decor.get(i)?.def as usize)?.clone();
        let uses = doc.map.decor.iter().filter(|d| doc.map.decor_defs.get(d.def as usize) == Some(&id)).count();
        if id.contains("_built_") && uses == 1 {
            return Some(id);
        }
        let p = self.project.db.prop(&id)?.clone();
        let nid = new_id(&self.project.db, &id, "built", &[]);
        self.project.db.props.push(PropDef { id: nid.clone(), name: format!("{} (own)", p.name), ..p });
        self.project.db.reindex();
        let doc = self.doc.as_mut()?;
        let slot = doc.map.decor_slot(&nid);
        doc.map.decor[i].def = slot;
        Some(nid)
    }

    fn set_house_shape(&mut self, id: &str, shape: BuildingShape) {
        if let Some(q) = self.project.db.props.iter_mut().find(|q| q.id == id) {
            q.building = Some(shape);
        }
        self.built_props(&[id.to_string()]);
    }

    /// Puts a door, a window or a gap into the house or wall piece the pointer (ground point
    /// `g`) shows. A house gets an opening of its own where the pointer meets its south or
    /// east wall; a wall piece becomes door prop `piece` (an object: it opens in play), window
    /// wall prop `piece`, or nothing.
    pub fn put_opening(&mut self, g: Vec2, iso: bool, what: Put, piece: &str) -> Result<String, String> {
        let Some(hit) = self.build_pick(g, iso, |p| is_wall(p) || is_house(p)) else { return Err("click a wall or a house".into()) };
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        if hit.prop.building.is_some() {
            return self.house_opening(hit, g, iso, what == Put::Door, what == Put::Gap, *snap);
        }
        let wall_name = self.def_label(EntityKind::Prop, &hit.prop.id);
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let old = doc.map.decor[hit.index];
        let (label, msg) = match what {
            Put::Gap => {
                doc.map.decor.remove(hit.index);
                (format!("Take a piece of {wall_name} out"), format!("Took a piece of {wall_name} out"))
            }
            Put::Window => {
                let Some(w) = self.project.db.prop(piece).filter(|w| is_window(w)) else { return Err("pick a window piece first".into()) };
                doc.map.decor.remove(hit.index);
                let new = Decor { def: doc.map.decor_slot(&w.id), ..old };
                doc.map.decor.push(new);
                (format!("Put {} into the wall", w.name), format!("Put {} into the wall", w.name))
            }
            Put::Door => {
                let Some(dp) = self.project.db.prop(piece).filter(|d| d.door) else { return Err("pick a door first".into()) };
                doc.map.decor.remove(hit.index);
                let uid = doc.map.alloc_uid();
                let mut e = EntityInstance::new(uid, EntityKind::Prop, &dp.id, old.x as f32, old.y as f32);
                e.facing = rustpg2d_engine::decor::turn_facing(old.turn());
                doc.map.entities.push(e);
                (format!("Put {} into the wall", dp.name), format!("Put {} into the wall (it opens in play)", dp.name))
            }
        };
        self.commit_map(*snap, &label);
        Ok(msg)
    }

    /// A door or window where the pointer meets a house's south or east wall.
    fn house_opening(&mut self, hit: Hit, g: Vec2, iso: bool, door: bool, gap: bool, snap: crate::mapedit::MapSnap) -> Result<String, String> {
        if gap {
            return Err("houses get doors and windows; right-click one to take it away".into());
        }
        if hit.face == 0 {
            return Err("that is the roof: click a wall".into());
        }
        let (r, z) = (hit.rect, hit.z);
        let k = if iso { LIFT } else { 0.0 };
        let at = g + vec2(z, z) * k;
        let (side, along, len) = if hit.face == 2 { (WallSide::East, at.y - r.y, r.h) } else { (WallSide::South, at.x - r.x, r.w) };
        let style = self.build_style("");
        let Some(id) = self.own_building(hit.index) else { return Err("that house can't be changed".into()) };
        let Some(mut shape) = self.project.db.prop(&id).and_then(|p| p.building.clone()) else { return Err("that house can't be changed".into()) };
        let template = shape.openings.iter().find(|o| (o.sill <= 0.0) == door).cloned().or_else(|| style.and_then(|s| s.building?.openings.into_iter().find(|o| (o.sill <= 0.0) == door)));
        let mut o = template.unwrap_or(Opening { side, at: 0.0, picture: SpriteRef::default(), size: if door { [26.0, 40.0] } else { [20.0, 20.0] }, sill: if door { 0.0 } else { 26.0 } });
        o.side = side;
        o.at = ((along / 8.0).round() * 8.0).clamp(o.size[0] / 2.0 + 2.0, (len - o.size[0] / 2.0 - 2.0).max(o.size[0] / 2.0 + 2.0));
        if !door && z > o.sill + o.size[1] + 8.0 {
            // An upper window: as high as clicked.
            o.sill = (((z - o.size[1] / 2.0) / 4.0).round() * 4.0).clamp(o.sill, (shape.height - o.size[1] - 4.0).max(o.sill));
        }
        shape.openings.retain(|q| !(q.side == side && (q.at - o.at).abs() < (q.size[0] + o.size[0]) / 2.0 && (q.sill - o.sill).abs() < (q.size[1] + o.size[1]) / 2.0));
        shape.openings.push(o);
        self.set_house_shape(&id, shape);
        let what = if door { "door" } else { "window" };
        self.commit_map(snap, &format!("Put a {what} into {}", self.def_label(EntityKind::Prop, &id)));
        self.begin_unnamed(&format!("Put a {what} into {}", self.def_label(EntityKind::Prop, &id)));
        Ok(format!("Put a {what} into '{id}'"))
    }

    /// Takes away the house's door or window the pointer shows, or a door put into a wall
    /// (wall prop `wall` comes back in its place; "" = a gap stays).
    pub fn take_opening(&mut self, g: Vec2, iso: bool, wall: &str) -> Result<String, String> {
        let ts = self.ts();
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let door = self.doc.as_ref().and_then(|doc| {
            doc.map
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Prop)
                .filter_map(|e| Some((e, self.project.db.prop(&e.def).filter(|d| d.door)?)))
                .find(|(e, d)| sight_hit(g, Rect::new(e.x - ts / 2.0, e.y - ts, ts, ts), d.block.max(ts), iso).is_some())
                .map(|(e, _)| (e.uid, e.x, e.y, e.facing))
        });
        if let Some((uid, x, y, facing)) = door {
            let wall = self.project.db.prop(wall).filter(|w| is_wall(w)).cloned();
            let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
            doc.map.entities.retain(|e| e.uid != uid);
            if let Some(w) = wall {
                let mut d = Decor { def: doc.map.decor_slot(&w.id), x: x as i32, y: y as i32, flip: false, state: 0, layer: DecorLayer::Sorted };
                d.set_turn(rustpg2d_engine::decor::facing_turn(facing));
                doc.map.decor.push(d);
            }
            self.commit_map(*snap, "Take a door out of the wall");
            return Ok("Took the door away".into());
        }
        let Some(hit) = self.build_pick(g, iso, is_house).filter(|h| h.face != 0) else { return Err("right-click a door or window in a wall".into()) };
        let Some(shape) = &hit.prop.building else { return Err("right-click a door or window in a wall".into()) };
        let k = if iso { LIFT } else { 0.0 };
        let at = g + vec2(hit.z, hit.z) * k;
        let (side, along) = if hit.face == 2 { (WallSide::East, at.y - hit.rect.y) } else { (WallSide::South, at.x - hit.rect.x) };
        let Some(n) = shape.openings.iter().position(|o| o.side == side && (o.at - along).abs() <= o.size[0] / 2.0 + 4.0 && (o.sill - 4.0..=o.sill + o.size[1] + 4.0).contains(&hit.z)) else { return Err("there is no door or window here".into()) };
        let Some(id) = self.own_building(hit.index) else { return Err("that house can't be changed".into()) };
        let Some(mut shape) = self.project.db.prop(&id).and_then(|p| p.building.clone()) else { return Err("that house can't be changed".into()) };
        shape.openings.remove(n);
        self.set_house_shape(&id, shape);
        let label = format!("Take a door or window out of {}", self.def_label(EntityKind::Prop, &id));
        self.commit_map(*snap, &label);
        self.begin_unnamed(&label);
        Ok(format!("Took a door or window out of '{id}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walls_run_along_tile_edges_and_turn_north_to_south() {
        let wall = PropDef { block: 48.0, hitbox: [32.0, 8.0], ..PropDef::fresh("wall") };
        let mut map = Map::new("m", 10, 10);
        // East to west along the edge at y = 64 (dragged a little off it): three pieces on it.
        let a = wall_run(&mut map, &wall, vec2(30.0, 70.0), vec2(130.0, 60.0), false, 32.0);
        assert_eq!(a.len(), 3);
        assert!(a.iter().all(|d| d.y == 64 && d.turn() == 0));
        // North to south along x = 32: turned, west of the edge.
        let b = wall_run(&mut map, &wall, vec2(30.0, 64.0), vec2(34.0, 160.0), false, 32.0);
        assert_eq!(b.len(), 3);
        assert!(b.iter().all(|d| d.x == 28 && d.turn() == 1));
        // Again: nothing new.
        assert!(wall_run(&mut map, &wall, vec2(30.0, 70.0), vec2(130.0, 60.0), false, 32.0).is_empty());
    }

    #[test]
    fn a_painted_wall_goes_round_corners() {
        assert_eq!(path_runs((1, 1), (4, 3)), vec![((1, 1), (4, 1)), ((4, 1), (4, 3))]);
        assert_eq!(path_runs((1, 1), (2, 5)), vec![((1, 1), (1, 5)), ((1, 5), (2, 5))]);
        assert_eq!(path_runs((1, 1), (1, 3)), vec![((1, 1), (1, 3))]);
        assert!(path_runs((2, 2), (2, 2)).is_empty());
        let wall = PropDef { block: 48.0, hitbox: [32.0, 8.0], snap: 32, ..PropDef::fresh("wall") };
        assert!(is_wall(&wall) && !is_wall(&PropDef { door: true, ..wall.clone() }));
        // A thin wall paints from corner to corner; a thick one from tile to tile.
        assert_eq!(wall_point(&wall, vec2(70.0, 50.0), 32.0), (2, 2));
        let block = PropDef { hitbox: [32.0, 32.0], ..wall.clone() };
        assert_eq!(wall_point(&block, vec2(70.0, 50.0), 32.0), (2, 1));
        // An L painted in one drag: three along the north edge, two down the east one.
        let mut map = Map::new("m", 10, 10);
        let mut n = 0;
        for (s, e) in path_runs((1, 1), (4, 3)) {
            n += wall_run(&mut map, &wall, point_px(&wall, s, 32.0), point_px(&wall, e, 32.0), false, 32.0).len();
        }
        assert_eq!(n, 5);
        assert_eq!(map.decor.iter().filter(|d| d.turn() == 1).count(), 2);
    }

    #[test]
    fn a_house_gets_a_door_in_its_front_and_windows() {
        let door = Opening { side: WallSide::West, at: 5.0, picture: Default::default(), size: [26.0, 40.0], sill: 0.0 };
        let win = Opening { sill: 30.0, size: [20.0, 20.0], ..door.clone() };
        let style = BuildingShape { openings: vec![door, win], ..Default::default() };
        let s = house_shape(&style, 256.0, 128.0, 1, &[]);
        let d: Vec<&Opening> = s.openings.iter().filter(|o| o.sill <= 0.0).collect();
        assert_eq!((d.len(), d[0].side, d[0].at), (1, WallSide::South, 128.0));
        assert!(s.openings.iter().filter(|o| o.sill > 0.0 && o.side == WallSide::South).all(|o| (o.at - 128.0).abs() > 28.0));
        assert!(s.openings.iter().any(|o| o.side == WallSide::East));
        // Two storeys: taller, windows upstairs too; joined on the south: the door goes east.
        let two = house_shape(&style, 256.0, 128.0, 2, &[WallSide::South]);
        assert_eq!(two.height, style.height + STOREY);
        assert!(two.openings.iter().any(|o| o.sill > STOREY));
        assert!(two.openings.iter().all(|o| o.side != WallSide::South));
        assert!(two.openings.iter().any(|o| o.sill <= 0.0 && o.side == WallSide::East));
    }

    #[test]
    fn houses_side_by_side_touch() {
        let a = Rect::new(0.0, 0.0, 128.0, 96.0);
        assert_eq!(touching(a, Rect::new(64.0, 96.0, 64.0, 96.0), 32.0), Some(WallSide::South));
        assert_eq!(touching(a, Rect::new(128.0, 32.0, 64.0, 64.0), 32.0), Some(WallSide::East));
        assert_eq!(touching(a, Rect::new(160.0, 0.0, 64.0, 64.0), 32.0), None);
        // Corners alone don't count.
        assert_eq!(touching(a, Rect::new(128.0, 96.0, 64.0, 64.0), 32.0), None);
    }

    #[test]
    fn hand_drawn_roofs_are_cut_into_rectangles() {
        let cells = |v: &[(i32, i32, i32, i32)]| -> std::collections::BTreeSet<(i32, i32)> { v.iter().flat_map(|&(x, y, w, h)| (y..y + h).flat_map(move |ty| (x..x + w).map(move |tx| (tx, ty)))).collect() };
        // An L: a 6 x 3 hall and a 3 x 3 wing below its east end.
        let l = cells(&[(0, 0, 6, 3), (3, 3, 3, 3)]);
        let rects = cut_into_rects(&l);
        assert_eq!(rects.len(), 2, "{rects:?}");
        assert_eq!(rects.iter().map(|r| r.2 * r.3).sum::<i32>(), 27);
        assert_eq!(rects[0], (0, 0, 6, 3));
        // A plain rectangle stays one; a cross is three.
        assert_eq!(cut_into_rects(&cells(&[(2, 2, 4, 3)])), vec![(2, 2, 4, 3)]);
        assert_eq!(cut_into_rects(&cells(&[(0, 2, 7, 3), (2, 0, 3, 2), (2, 5, 3, 2)])).len(), 3);
    }

    #[test]
    fn a_house_dragged_out_of_another_one_starts_at_its_wall() {
        let m = Rect::new(0.0, 0.0, 224.0, 160.0);
        assert_eq!(cut_against(Rect::new(96.0, 128.0, 128.0, 192.0), m), Rect::new(96.0, 160.0, 128.0, 160.0));
        assert_eq!(cut_against(Rect::new(192.0, 32.0, 128.0, 96.0), m), Rect::new(224.0, 32.0, 96.0, 96.0));
        // Apart, or all inside: unchanged.
        assert_eq!(cut_against(Rect::new(300.0, 0.0, 64.0, 64.0), m), Rect::new(300.0, 0.0, 64.0, 64.0));
        assert_eq!(cut_against(Rect::new(32.0, 32.0, 64.0, 64.0), m), Rect::new(32.0, 32.0, 64.0, 64.0));
    }

    #[test]
    fn the_line_of_sight_meets_the_nearest_face() {
        let r = Rect::new(0.0, 0.0, 64.0, 32.0);
        // Straight onto the top.
        assert_eq!(sight_hit(vec2(32.0 - 40.0 * LIFT, 16.0 - 40.0 * LIFT), r, 40.0, true).map(|h| h.1), Some(0));
        // Low on the south face: the ground point lies a little north-west of the wall's foot.
        let (z, face) = sight_hit(vec2(30.0 - 10.0 * LIFT, 32.0 - 10.0 * LIFT), r, 40.0, true).unwrap();
        assert!(face == 1 && (z - 10.0).abs() < 1e-3, "{z} {face}");
        // The east face.
        let (_, face) = sight_hit(vec2(64.0 - 5.0 * LIFT, 10.0 - 5.0 * LIFT), r, 40.0, true).unwrap();
        assert_eq!(face, 2);
        assert!(sight_hit(vec2(200.0, 200.0), r, 40.0, true).is_none());
    }

    #[test]
    fn built_ids_dont_pile_up_endings() {
        let db = Database { props: vec![PropDef::fresh("house_built_1")], ..Default::default() };
        assert_eq!(new_id(&db, "house", "built", &[]), "house_built_2");
        assert_eq!(new_id(&db, "house_built_1", "built", &[]), "house_built_2");
        assert_eq!(new_id(&db, "house", "built", &["house_built_2"]), "house_built_3");
        assert_eq!(new_id(&db, "house", "roof", &[]), "house_roof_1");
    }
}
