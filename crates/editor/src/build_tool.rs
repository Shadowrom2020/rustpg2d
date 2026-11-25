//! The Build tool (B): walls painted along tile edges, rooms, houses and roofs dragged as
//! rectangles, doors and windows clicked into walls. Walls are props drawn as blocks or thin
//! models (a fence); north-to-south runs are the same prop turned a quarter. Rooms paint their
//! floor and put walls on their back sides (or all four); houses and roofs are new building props
//! in the look of an existing one, sized to the rectangle; a house dragged against another one is
//! joined to it (an L-shaped house). What was built can be saved as a structure in one click.

use egui_macroquad::egui;
use macroquad::prelude::*;
use rustpg2d_engine::decor::{Decor, DecorLayer, DecorLook};
use rustpg2d_engine::defs::{BuildingShape, Database, Join, Opening, PropDef, Ridge, RoofKind, SpriteRef, WallSide};
use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::map::Map;
use rustpg2d_engine::render::iso;

use crate::app::Editor;
use crate::doc::Objects;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BuildMode {
    /// Drag along tile edges: a wall that follows the mouse.
    #[default]
    Walls,
    /// Drag a rectangle: a floor and walls around it.
    Room,
    /// Drag a rectangle: a building of that size.
    House,
    /// Drag a rectangle: a roof alone (over a room of walls).
    Roof,
    /// Click a wall or a house: a door, a window or a gap.
    Openings,
}

impl BuildMode {
    pub const ALL: [BuildMode; 5] = [BuildMode::Walls, BuildMode::Room, BuildMode::House, BuildMode::Roof, BuildMode::Openings];

    pub fn label(self) -> &'static str {
        match self {
            BuildMode::Walls => "Walls",
            BuildMode::Room => "Room",
            BuildMode::House => "House",
            BuildMode::Roof => "Roof",
            BuildMode::Openings => "Doors",
        }
    }

    pub fn parse(s: &str) -> Option<BuildMode> {
        BuildMode::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(s) || format!("{m:?}").eq_ignore_ascii_case(s))
    }
}

/// What a click in the Doors mode puts into a wall.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OpeningKind {
    #[default]
    Door,
    Window,
    /// A wall piece taken out (houses: none).
    Gap,
}

pub struct BuildTool {
    pub mode: BuildMode,
    /// The wall prop (Walls, Room; Roof: as high as it).
    pub wall: String,
    /// The floor's material (Room; "" = leave the ground).
    pub floor: String,
    /// Room: walls on all four sides (with a doorway in the south wall) instead of the back two.
    pub all_sides: bool,
    /// Room: a roof over it too.
    pub room_roof: bool,
    /// House, Roof: the building prop whose look a new one takes.
    pub style: String,
    /// House: storeys (1-3).
    pub storeys: u8,
    /// Roof: its kind.
    pub roof: RoofKind,
    /// Doors: what a click puts in, and the door prop / window wall prop for wall pieces.
    pub opening: OpeningKind,
    pub door: String,
    pub window: String,
    /// Where the drag started (world pixels).
    pub start: Option<Vec2>,
    /// Walls: the corner (thin walls) or tile (thick ones) the painting has reached.
    pub last: Option<(i32, i32)>,
    /// Walls: the pieces the drag has placed (one undo step when it ends).
    pub stroke: Vec<Decor>,
    /// The tiles built on (x0, y0, x1, y1, inclusive) since a structure was saved from them,
    /// and the map they are on.
    pub area: Option<(i32, i32, i32, i32)>,
    pub area_map: String,
    /// The id the next "Save as structure" takes.
    pub structure_id: String,
}

impl Default for BuildTool {
    fn default() -> Self {
        BuildTool {
            mode: BuildMode::Walls,
            wall: String::new(),
            floor: String::new(),
            all_sides: false,
            room_roof: false,
            style: String::new(),
            storeys: 1,
            roof: RoofKind::Gable,
            opening: OpeningKind::Door,
            door: String::new(),
            window: String::new(),
            start: None,
            last: None,
            stroke: Vec::new(),
            area: None,
            area_map: String::new(),
            structure_id: String::new(),
        }
    }
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

fn is_house(p: &PropDef) -> bool {
    p.building.as_ref().is_some_and(|b| b.walls)
}

/// Whether wall prop `p` stands on a tile's edge (thin) rather than filling the tile.
fn thin(p: &PropDef) -> bool {
    p.hitbox[1] <= 12.0
}

/// The tile edge nearest to `v` (world pixels).
fn edge(v: f32, ts: f32) -> f32 {
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
fn point_px(p: &PropDef, (x, y): (i32, i32), ts: f32) -> Vec2 {
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

impl Editor {
    fn build_footprint(&self, d: &Decor) -> Option<(PropDef, Rect)> {
        let doc = self.doc.as_ref()?;
        let id = doc.map.decor_defs.get(d.def as usize)?;
        let p = self.project.db.prop(id)?.clone();
        let (x, y, w, h) = DecorLook::of(&self.project.db, id, 0).rect_at(d);
        Some((p, Rect::new(x, y, w, h)))
    }

    /// The decoration the cursor (ground point `g`) shows nearest the viewer among those `keep`
    /// accepts: its index, prop, footprint and where the line of sight meets it.
    fn build_pick(&self, g: Vec2, keep: impl Fn(&PropDef) -> bool) -> Option<(usize, PropDef, Rect, f32, u8)> {
        let doc = self.doc.as_ref()?;
        let iso = self.iso().is_some();
        let mut best: Option<(usize, PropDef, Rect, f32, u8)> = None;
        for (i, d) in doc.map.decor.iter().enumerate() {
            let Some((p, r)) = self.build_footprint(d) else { continue };
            if !keep(&p) {
                continue;
            }
            let h = match &p.building {
                Some(b) => b.height,
                None => p.block.max(8.0),
            };
            let Some((z, face)) = sight_hit(g, r, h, iso) else { continue };
            if best.as_ref().is_none_or(|b| z > b.3) {
                best = Some((i, p, r, z, face));
            }
        }
        best
    }

    /// Adds footprint `r` (world pixels) to the area built on.
    fn built_on(&mut self, r: Rect) {
        let ts = self.ts();
        let map = self.doc.as_ref().map_or(String::new(), |d| d.map.name.clone());
        if self.build.area_map != map {
            self.build.area = None;
            self.build.area_map = map;
        }
        let (x0, y0) = ((r.x / ts).floor() as i32, (r.y / ts).floor() as i32);
        let (x1, y1) = (((r.right() / ts).ceil() as i32 - 1).max(x0), ((r.bottom() / ts).ceil() as i32 - 1).max(y0));
        self.build.area = Some(match self.build.area {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
    }

    fn built_decor(&mut self, added: &[Decor]) {
        let rects: Vec<Rect> = added.iter().filter_map(|d| self.build_footprint(d).map(|(_, r)| r)).collect();
        for r in rects {
            self.built_on(r);
        }
    }

    pub fn build_down(&mut self, world: Vec2) {
        self.build.start = Some(world);
        self.build.last = None;
        self.build.stroke.clear();
        match self.build.mode {
            BuildMode::Walls if !crate::canvas::modifier_shift() => {
                let Some(p) = self.project.db.prop(&self.build.wall).cloned() else { return self.set_status("Pick a wall first") };
                let at = wall_point(&p, world, self.ts());
                self.build.last = Some(at);
                if !thin(&p) {
                    self.paint_wall_to(&p, at, at);
                }
            }
            BuildMode::Openings => {
                self.build.start = None;
                self.put_opening(world);
            }
            _ => {}
        }
    }

    /// Walls: the painted wall from `a` on to `b` (wall points).
    fn paint_wall_to(&mut self, p: &PropDef, a: (i32, i32), b: (i32, i32)) {
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return };
        let runs = if a == b { vec![(a, b)] } else { path_runs(a, b) };
        for (s, e) in runs {
            let added = wall_run(&mut doc.map, p, point_px(p, s, ts), point_px(p, e, ts), false, ts);
            self.build.stroke.extend(added);
        }
    }

    /// The mouse moved with the button down: painted walls follow it.
    pub fn build_drag(&mut self, world: Vec2) {
        if self.build.mode != BuildMode::Walls {
            return;
        }
        let Some(last) = self.build.last else { return };
        let Some(p) = self.project.db.prop(&self.build.wall).cloned() else { return };
        let at = wall_point(&p, world, self.ts());
        if at != last {
            self.paint_wall_to(&p, last, at);
            self.build.last = Some(at);
        }
    }

    /// The drag ended at `world`: the wall, room, house or roof is made (one undo step each).
    pub fn build_up(&mut self, world: Vec2) {
        let Some(start) = self.build.start.take() else { return };
        let ts = self.ts();
        match self.build.mode {
            BuildMode::Walls => {
                let Some(p) = self.project.db.prop(&self.build.wall).cloned() else { return self.set_status("Pick a wall first") };
                if self.build.last.take().is_some() {
                    self.build_drag_end(&p, world);
                } else if let Some(doc) = &mut self.doc {
                    // Shift: a straight line from where the drag began.
                    let added = wall_run(&mut doc.map, &p, start, world, false, ts);
                    self.build.stroke.extend(added);
                }
                let added = std::mem::take(&mut self.build.stroke);
                let n = added.len();
                self.built_decor(&added);
                if let Some(doc) = &mut self.doc {
                    doc.record_build(Vec::new(), 0, added, Vec::new(), None, &self.assets);
                }
                self.set_status(format!("Built {n} pieces of {}", p.name));
            }
            BuildMode::Room => self.build_room(start, world),
            BuildMode::House => self.build_house(start, world),
            BuildMode::Roof => {
                let r = self.build_rect(start, world);
                self.build_roof(r);
            }
            BuildMode::Openings => {}
        }
    }

    /// The painted wall's last stretch, to where the button was let go.
    fn build_drag_end(&mut self, p: &PropDef, world: Vec2) {
        self.build.last = Some(self.build.last.unwrap_or_else(|| wall_point(p, world, self.ts())));
        self.build_drag(world);
        self.build.last = None;
    }

    /// Right-click: Walls / Room take the wall piece under the cursor away, Doors a house's door or
    /// window (or a door put into a wall, back to the wall).
    pub fn build_right(&mut self, world: Vec2) {
        match self.build.mode {
            BuildMode::Walls | BuildMode::Room => {
                let Some((i, p, ..)) = self.build_pick(world, is_wall) else { return };
                let Some(doc) = &mut self.doc else { return };
                let d = doc.map.decor.remove(i);
                doc.record_build(Vec::new(), 0, Vec::new(), vec![d], None, &self.assets);
                self.set_status(format!("Took away a piece of {}", p.name));
            }
            BuildMode::Openings => self.take_opening(world),
            _ => {}
        }
    }

    /// The rectangle (world pixels, on tile edges) a drag from `a` to `b` covers: at least a tile.
    fn build_rect(&self, a: Vec2, b: Vec2) -> Rect {
        let ts = self.ts();
        let (x0, y0) = ((a.x.min(b.x) / ts).floor() * ts, (a.y.min(b.y) / ts).floor() * ts);
        let (x1, y1) = ((a.x.max(b.x) / ts).ceil() * ts, (a.y.max(b.y) / ts).ceil() * ts);
        Rect::new(x0, y0, (x1 - x0).max(ts), (y1 - y0).max(ts))
    }

    fn build_room(&mut self, a: Vec2, b: Vec2) {
        let r = self.build_rect(a, b);
        let ts = self.ts();
        let wall = self.project.db.prop(&self.build.wall).cloned();
        let (floor, all) = (self.build.floor.clone(), self.build.all_sides);
        let Some(doc) = &mut self.doc else { return };
        let mut cells = Vec::new();
        let mut value = 0;
        if !floor.is_empty() {
            if let Some(v) = doc.map.material_value(&floor) {
                cells = doc.map.ground.paint_rect((r.x, r.y), (r.x + r.w, r.y + r.h), v);
                value = v;
            }
        }
        let mut added = Vec::new();
        if let Some(p) = &wall {
            let (l, t, rt, bt) = (r.x, r.y, r.x + r.w, r.y + r.h);
            added.extend(wall_run(&mut doc.map, p, vec2(l, t), vec2(rt, t), false, ts));
            added.extend(wall_run(&mut doc.map, p, vec2(l, t), vec2(l, bt), false, ts));
            if all {
                // The south wall with a doorway in its middle.
                let door = ((r.w / ts / 2.0).floor() * ts) + l;
                if door > l {
                    added.extend(wall_run(&mut doc.map, p, vec2(l, bt), vec2(door, bt), true, ts));
                }
                if door + ts < rt {
                    added.extend(wall_run(&mut doc.map, p, vec2(door + ts, bt), vec2(rt, bt), true, ts));
                }
                added.extend(wall_run(&mut doc.map, p, vec2(rt, t), vec2(rt, bt), true, ts));
            }
        }
        let n = added.len();
        self.built_on(r);
        self.built_decor(&added);
        if let Some(doc) = &mut self.doc {
            doc.record_build(cells, value, added, Vec::new(), None, &self.assets);
        }
        if self.build.room_roof {
            self.build_roof(r);
        }
        self.set_status(format!("Built a room of {} x {} tiles ({n} wall pieces)", (r.w / ts) as i32, (r.h / ts) as i32));
    }

    /// The look a new house or roof takes: the chosen building, else the first one.
    fn build_style(&self) -> Option<PropDef> {
        let db = &self.project.db;
        db.prop(&self.build.style).filter(|p| is_house(p)).or_else(|| db.props.iter().find(|p| is_house(p))).cloned()
    }

    /// A new prop `p` in the project, its building model built (and those joined to it).
    fn add_built_prop(&mut self, p: PropDef) {
        let id = p.id.clone();
        self.project.db.props.push(p);
        self.project.db.reindex();
        self.defs_dirty = true;
        if let Some(p) = self.project.db.prop(&id).cloned() {
            self.assets.rebuild_building(&p, &self.project.db);
        }
    }

    fn build_house(&mut self, a: Vec2, b: Vec2) {
        let mut r = self.build_rect(a, b);
        let ts = self.ts();
        // Dragged from inside another house: the part outside it.
        let houses: Vec<Rect> = self.doc.as_ref().map_or(Vec::new(), |doc| doc.map.decor.iter().filter_map(|d| self.build_footprint(d).filter(|(p, _)| is_house(p)).map(|(_, fr)| fr)).collect());
        for fr in houses {
            r = cut_against(r, fr);
        }
        let Some(style) = self.build_style() else { return self.set_status("Pick a house to build like") };
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
        let storeys = if neighbour.is_some() { 1 } else { self.build.storeys };
        let mut shape = house_shape(&shape0, r.w, r.h, storeys, &closed);
        let id = new_id(&self.project.db, &look.id, "built", &[]);
        let mut joined = String::new();
        let mut moved: Option<(Decor, Decor)> = None;
        if let Some((i, mp, mut ms, fr, side)) = neighbour.clone() {
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
                    let old = doc.map.decor[i];
                    doc.map.decor[i].def = slot;
                    moved = Some((old, doc.map.decor[i]));
                }
            }
            self.defs_dirty = true;
        }
        let tiles = format!("{}x{}", (r.w / ts) as i32, (r.h / ts) as i32);
        let name = if storeys > 1 { format!("{} ({tiles}, {storeys} storeys)", look.name) } else { format!("{} ({tiles})", look.name) };
        self.add_built_prop(PropDef { id: id.clone(), name, hitbox: [r.w, r.h], building: Some(shape), model: String::new(), ..look });
        let Some(doc) = &mut self.doc else { return };
        let slot = doc.map.decor_slot(&id);
        let d = Decor { def: slot, x: (r.x + r.w / 2.0) as i32, y: (r.y + r.h) as i32, flip: false, state: 0, layer: DecorLayer::Sorted };
        doc.map.decor.push(d);
        let (mut added, mut removed) = (vec![d], Vec::new());
        if let Some((old, new)) = moved {
            removed.push(old);
            added.push(new);
        }
        doc.record_build(Vec::new(), 0, added, removed, None, &self.assets);
        self.built_on(r);
        let _ = self.render_prop_pictures(Some(&id));
        if !joined.is_empty() {
            let _ = self.render_prop_pictures(Some(&joined));
            self.set_status(format!("Built '{id}' ({tiles} tiles) as a wing of '{joined}': one roof over both"));
        } else {
            self.set_status(format!("Built the house '{id}' ({tiles} tiles); Database › Props › {id} changes its doors, windows and roof"));
        }
    }

    /// A roof alone over rectangle `r` (world pixels) as high as the chosen wall, in the chosen
    /// look.
    fn build_roof(&mut self, r: Rect) {
        let ts = self.ts();
        let Some(style) = self.build_style() else { return self.set_status("Pick a building whose roof to copy") };
        let wall_h = self.project.db.prop(&self.build.wall).filter(|w| w.block > 0.0).map_or(48.0, |w| w.block);
        let base = style.building.clone().unwrap_or_default();
        let shape = BuildingShape { walls: false, height: wall_h, roof: self.build.roof, openings: Vec::new(), chimneys: Vec::new(), joins: Vec::new(), base_height: 0.0, ..base };
        let id = new_id(&self.project.db, &style.id, "roof", &[]);
        let tiles = format!("{}x{}", (r.w / ts) as i32, (r.h / ts) as i32);
        let p = PropDef { id: id.clone(), name: format!("Roof ({tiles})"), hitbox: [r.w, r.h], building: Some(shape), model: String::new(), solid: false, states: Vec::new(), triggers: Vec::new(), ..style };
        self.add_built_prop(p);
        let Some(doc) = &mut self.doc else { return };
        let slot = doc.map.decor_slot(&id);
        let d = Decor { def: slot, x: (r.x + r.w / 2.0) as i32, y: (r.y + r.h) as i32, flip: false, state: 0, layer: DecorLayer::Sorted };
        doc.map.decor.push(d);
        doc.record_build(Vec::new(), 0, vec![d], Vec::new(), None, &self.assets);
        self.built_on(r);
        let _ = self.render_prop_pictures(Some(&id));
        self.set_status(format!("Built the roof '{id}' ({tiles} tiles) at {wall_h} px"));
    }

    /// The building prop decoration `i` shows, ready to be changed: its own prop, or a copy when
    /// other decorations of the map use it too or it came from elsewhere (the decoration then
    /// shows the copy).
    fn own_building(&mut self, i: usize) -> Option<String> {
        let doc = self.doc.as_ref()?;
        let id = doc.map.decor_defs.get(doc.map.decor[i].def as usize)?.clone();
        let uses = doc.map.decor.iter().filter(|d| doc.map.decor_defs.get(d.def as usize) == Some(&id)).count();
        if id.contains("_built_") && uses == 1 {
            return Some(id);
        }
        let p = self.project.db.prop(&id)?.clone();
        let nid = new_id(&self.project.db, &id, "built", &[]);
        self.project.db.props.push(PropDef { id: nid.clone(), name: format!("{} (own)", p.name), ..p });
        self.project.db.reindex();
        self.defs_dirty = true;
        let doc = self.doc.as_mut()?;
        let slot = doc.map.decor_slot(&nid);
        let old = doc.map.decor[i];
        doc.map.decor[i].def = slot;
        let new = doc.map.decor[i];
        doc.record_build(Vec::new(), 0, vec![new], vec![old], None, &self.assets);
        Some(nid)
    }

    /// Doors: a click at ground point `world` puts the chosen door, window or gap into the house or
    /// wall piece the cursor shows.
    fn put_opening(&mut self, world: Vec2) {
        let pick = self.build_pick(world, |p| is_wall(p) || is_house(p));
        let Some((i, p, r, z, face)) = pick else { return self.set_status("Click a wall or a house") };
        if p.building.is_some() {
            return self.house_opening(i, r, world, z, face);
        }
        let door = self.project.db.prop(&self.build.door).filter(|d| d.door).cloned();
        let window = self.project.db.prop(&self.build.window).filter(|w| is_window(w)).cloned();
        let Some(doc) = &mut self.doc else { return };
        let before = Objects::of(&doc.map);
        let old = doc.map.decor[i];
        let (msg, added) = match self.build.opening {
            OpeningKind::Gap => {
                doc.map.decor.remove(i);
                (format!("Took a piece of {} out", p.name), vec![])
            }
            OpeningKind::Window => {
                let Some(w) = window else { return self.set_status("Pick a window piece first") };
                doc.map.decor.remove(i);
                let new = Decor { def: doc.map.decor_slot(&w.id), ..old };
                doc.map.decor.push(new);
                (format!("Put {} into the wall", w.name), vec![new])
            }
            OpeningKind::Door => {
                let Some(dp) = door else { return self.set_status("Pick a door first") };
                doc.map.decor.remove(i);
                let uid = doc.map.alloc_uid();
                let mut e = EntityInstance::new(uid, EntityKind::Prop, &dp.id, old.x as f32, old.y as f32);
                e.facing = rustpg2d_engine::decor::turn_facing(old.turn());
                doc.map.entities.push(e);
                (format!("Put {} into the wall (it opens in play)", dp.name), vec![])
            }
        };
        doc.record_build(Vec::new(), 0, added, vec![old], Some(before), &self.assets);
        self.set_status(msg);
    }

    /// Doors on a house: a door or window where the cursor meets its south or east wall.
    fn house_opening(&mut self, i: usize, r: Rect, g: Vec2, z: f32, face: u8) {
        if self.build.opening == OpeningKind::Gap {
            return self.set_status("Houses get doors and windows; right-click one to take it away");
        }
        if face == 0 {
            return self.set_status("That is the roof: click a wall");
        }
        let door = self.build.opening == OpeningKind::Door;
        let k = if self.iso().is_some() { LIFT } else { 0.0 };
        let hit = g + vec2(z, z) * k;
        let (side, along, len) = if face == 2 { (WallSide::East, hit.y - r.y, r.h) } else { (WallSide::South, hit.x - r.x, r.w) };
        let Some(id) = self.own_building(i) else { return };
        let Some(mut shape) = self.project.db.prop(&id).and_then(|p| p.building.clone()) else { return };
        let template = shape.openings.iter().find(|o| (o.sill <= 0.0) == door).cloned().or_else(|| self.build_style().and_then(|s| s.building?.openings.into_iter().find(|o| (o.sill <= 0.0) == door)));
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
        self.set_status(format!("Put a {} into '{id}'", if door { "door" } else { "window" }));
    }

    fn set_house_shape(&mut self, id: &str, shape: BuildingShape) {
        if let Some(q) = self.project.db.props.iter_mut().find(|q| q.id == id) {
            q.building = Some(shape);
        }
        self.defs_dirty = true;
        if let Some(p) = self.project.db.prop(id).cloned() {
            self.assets.rebuild_building(&p, &self.project.db);
        }
        let _ = self.render_prop_pictures(Some(id));
    }

    /// Doors, right-click: takes away the house's door or window under the cursor, or a door put
    /// into a wall (the chosen wall comes back).
    fn take_opening(&mut self, world: Vec2) {
        let iso = self.iso().is_some();
        let ts = self.ts();
        let door = self.doc.as_ref().and_then(|doc| {
            doc.map
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Prop)
                .filter_map(|e| Some((e, self.project.db.prop(&e.def).filter(|d| d.door)?)))
                .find(|(e, d)| sight_hit(world, Rect::new(e.x - ts / 2.0, e.y - ts, ts, ts), d.block.max(ts), iso).is_some())
                .map(|(e, _)| (e.uid, e.x, e.y, e.facing))
        });
        if let Some((uid, x, y, facing)) = door {
            let wall = self.project.db.prop(&self.build.wall).filter(|w| is_wall(w)).cloned();
            let Some(doc) = &mut self.doc else { return };
            let before = Objects::of(&doc.map);
            doc.map.entities.retain(|e| e.uid != uid);
            let mut added = Vec::new();
            if let Some(w) = wall {
                let mut d = Decor { def: doc.map.decor_slot(&w.id), x: x as i32, y: y as i32, flip: false, state: 0, layer: DecorLayer::Sorted };
                d.set_turn(rustpg2d_engine::decor::facing_turn(facing));
                doc.map.decor.push(d);
                added.push(d);
            }
            doc.record_build(Vec::new(), 0, added, Vec::new(), Some(before), &self.assets);
            return self.set_status("Took the door away");
        }
        let Some((i, p, r, z, face)) = self.build_pick(world, is_house).filter(|h| h.4 != 0) else { return self.set_status("Right-click a door or window in a wall") };
        let Some(shape) = &p.building else { return };
        let k = if iso { LIFT } else { 0.0 };
        let hit = world + vec2(z, z) * k;
        let (side, along) = if face == 2 { (WallSide::East, hit.y - r.y) } else { (WallSide::South, hit.x - r.x) };
        let Some(n) = shape.openings.iter().position(|o| o.side == side && (o.at - along).abs() <= o.size[0] / 2.0 + 4.0 && (o.sill - 4.0..=o.sill + o.size[1] + 4.0).contains(&z)) else {
            return self.set_status("No door or window there");
        };
        let Some(id) = self.own_building(i) else { return };
        let Some(mut shape) = self.project.db.prop(&id).and_then(|p| p.building.clone()) else { return };
        shape.openings.remove(n);
        self.set_house_shape(&id, shape);
        self.set_status(format!("Took a door or window out of '{id}'"));
    }

    /// Saves the tiles built on as structure `id` (also its name), ground included.
    pub fn save_built_structure(&mut self, id: &str) -> anyhow::Result<String> {
        let map = self.doc.as_ref().map_or(String::new(), |d| d.map.name.clone());
        let Some((x0, y0, x1, y1)) = self.build.area.filter(|_| self.build.area_map == map) else { anyhow::bail!("nothing built on this map yet") };
        let (mw, mh) = self.doc.as_ref().map_or((0, 0), |d| (d.map.width as i32, d.map.height as i32));
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(mw - 1), y1.min(mh - 1));
        anyhow::ensure!(x1 >= x0 && y1 >= y0, "the built area lies outside the map");
        let id = id.trim().to_string();
        let msg = self.create_structure((x0 as u32, y0 as u32, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32), &id, &id, "Built", true)?;
        self.build.area = None;
        self.build.structure_id.clear();
        Ok(msg)
    }

    /// Canvas: what the drag would build, or what a click in the Doors mode would change.
    pub fn draw_build_hover(&self, world: Vec2, px: f32) {
        let ts = self.ts();
        let col = Color::new(1.0, 0.85, 0.2, 0.9);
        if self.build.mode == BuildMode::Openings {
            if let Some((_, _, r, _, _)) = self.build_pick(world, |p| is_wall(p) || is_house(p)) {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0 * px, col);
            }
            return;
        }
        let Some(a) = self.build.start else {
            match self.project.db.prop(&self.build.wall).filter(|_| self.build.mode == BuildMode::Walls) {
                Some(p) if !thin(p) => {
                    let c = point_px(p, wall_point(p, world, ts), ts);
                    draw_rectangle_lines(c.x - ts / 2.0, c.y - ts / 2.0, ts, ts, 2.0 * px, col);
                }
                _ => {
                    let e = vec2(edge(world.x, ts), edge(world.y, ts));
                    draw_circle(e.x, e.y, 4.0 * px, col);
                }
            }
            return;
        };
        match self.build.mode {
            BuildMode::Walls => {
                if self.build.last.is_none() {
                    let horizontal = (world.x - a.x).abs() >= (world.y - a.y).abs();
                    let p = vec2(edge(a.x, ts), edge(a.y, ts));
                    let q = if horizontal { vec2(edge(world.x, ts), p.y) } else { vec2(p.x, edge(world.y, ts)) };
                    draw_line(p.x, p.y, q.x, q.y, 4.0 * px, col);
                }
            }
            _ => {
                let r = self.build_rect(a, world);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 0.85, 0.2, 0.15));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0 * px, col);
            }
        }
    }

    /// The Build tool's left panel.
    pub fn build_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading("Build");
        ui.horizontal_wrapped(|ui| {
            for m in BuildMode::ALL {
                let tip = match m {
                    BuildMode::Walls => "Drag: a wall that follows the mouse along the tile edges (Shift: a straight line). Right-click takes a piece away",
                    BuildMode::Room => "Drag a rectangle: its floor and walls around it (and a roof, if you like)",
                    BuildMode::House => "Drag a rectangle: a house of that size; dragged against another house, a wing of it",
                    BuildMode::Roof => "Drag a rectangle: a roof alone, over walls you built",
                    BuildMode::Openings => "Click a wall or a house: a door, a window or a gap. Right-click takes one away",
                };
                ui.selectable_value(&mut self.build.mode, m, m.label()).on_hover_text(tip);
            }
        });
        ui.weak(match self.build.mode {
            BuildMode::Walls => "Drag: the wall follows the mouse along the tile edges, round corners too (Shift: a straight line). Thin walls and fences stand on the edges, thick walls fill the tiles. Right-click a piece to take it away.",
            BuildMode::Room => "Drag the room's floor. Its back walls (north, west) are built, or all four with a doorway in the south wall; with a roof, it gets one over it.",
            BuildMode::House => "Drag the house's footprint: a new building prop in the chosen look, its door in the middle of the front, windows along the walls. Drag it against another house and it becomes a wing of that one: an L-shaped house under one roof.",
            BuildMode::Roof => "Drag over a room of walls: a roof alone, as high as the chosen wall, in the look of the chosen building. In play it fades out like anything else in front of the hero.",
            BuildMode::Openings => "Click a house's wall: a door or window there. Click a wall piece: a door (it opens in play), a window piece or a gap. Right-click a house's door or window, or a door in a wall, to take it away.",
        });
        type Entry = (String, String, SpriteRef);
        let db = &self.project.db;
        let entries = |keep: &dyn Fn(&PropDef) -> bool| -> Vec<Entry> { db.props.iter().filter(|p| keep(p)).map(|p| (p.id.clone(), p.name.clone(), p.sprite.clone())).collect() };
        let looks = entries(&|p| is_house(p) && !p.id.contains("_built_"));
        let walls = entries(&|p| is_wall(p) && !p.id.ends_with("_side"));
        let doors = entries(&|p| p.door);
        let windows = entries(&|p| is_window(p) && !p.id.ends_with("_side"));
        let (tex, assets) = (&self.tex, &self.assets);
        let thumbs = |ui: &mut egui::Ui, salt: &str, list: &[Entry], chosen: &mut String| {
            egui::ScrollArea::vertical().id_salt(salt).max_height(240.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (id, name, sprite) in list {
                        if crate::widgets::sprite_thumb(ui, tex, assets, sprite, 40.0, id == chosen).on_hover_text(format!("{name} ({id})")).clicked() {
                            *chosen = id.clone();
                        }
                    }
                });
            });
        };
        let b = &mut self.build;
        match b.mode {
            BuildMode::House | BuildMode::Roof => {
                if b.mode == BuildMode::House {
                    ui.horizontal(|ui| {
                        ui.label("Storeys");
                        for n in 1..=3u8 {
                            ui.selectable_value(&mut b.storeys, n, n.to_string());
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Roof");
                        for k in [RoofKind::Gable, RoofKind::Hip, RoofKind::Shed, RoofKind::Flat] {
                            ui.selectable_value(&mut b.roof, k, format!("{k:?}"));
                        }
                    });
                    ui.label("As high as the wall");
                    thumbs(ui, "build_roof_walls", &walls, &mut b.wall);
                }
                ui.label("Look");
                thumbs(ui, "build_looks", &looks, &mut b.style);
            }
            BuildMode::Openings => {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut b.opening, OpeningKind::Door, "Door");
                    ui.selectable_value(&mut b.opening, OpeningKind::Window, "Window");
                    ui.selectable_value(&mut b.opening, OpeningKind::Gap, "Gap");
                });
                match b.opening {
                    OpeningKind::Door => {
                        ui.label("Door for wall pieces (houses take their own)");
                        thumbs(ui, "build_doors", &doors, &mut b.door);
                    }
                    OpeningKind::Window => {
                        ui.label("Window piece for walls (houses take their own)");
                        thumbs(ui, "build_windows", &windows, &mut b.window);
                    }
                    OpeningKind::Gap => {
                        ui.weak("A click takes the wall piece away: a doorway.");
                    }
                }
                ui.label("Wall that comes back when you right-click a door");
                thumbs(ui, "build_walls", &walls, &mut b.wall);
            }
            BuildMode::Walls | BuildMode::Room => {
                if b.mode == BuildMode::Room {
                    ui.checkbox(&mut b.all_sides, "Walls on all four sides").on_hover_text("Off: only the back walls (north and west), open to the viewer like the classic interiors");
                    ui.checkbox(&mut b.room_roof, "With a roof").on_hover_text("A roof over the room, in the look and kind chosen in the Roof mode");
                    egui::ComboBox::from_label("Floor").selected_text(if b.floor.is_empty() { "(leave the ground)".to_string() } else { b.floor.clone() }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut b.floor, String::new(), "(leave the ground)");
                        for m in &db.materials {
                            ui.selectable_value(&mut b.floor, m.id.clone(), &m.name);
                        }
                    });
                }
                ui.label("Wall");
                thumbs(ui, "build_walls", &walls, &mut b.wall);
            }
        }
        // What was built, as a structure in one click (not while a structure is open itself).
        if self.editing_structure.is_some() {
            return;
        }
        ui.separator();
        let map = self.doc.as_ref().map_or(String::new(), |d| d.map.name.clone());
        let mut save = false;
        match self.build.area.filter(|_| self.build.area_map == map) {
            Some((x0, y0, x1, y1)) => {
                ui.label(format!("Built here: {} x {} tiles", x1 - x0 + 1, y1 - y0 + 1));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.build.structure_id).hint_text("structure id").desired_width(130.0));
                    save = ui
                        .add_enabled(!self.build.structure_id.trim().is_empty(), egui::Button::new(crate::ui::t(crate::ui::ph::CUBE, "Save as structure")))
                        .on_hover_text("Everything on the tiles you built on (ground, walls, houses, roofs, doors) becomes a structure, placed with the Structures tool")
                        .clicked();
                    if ui.small_button(crate::ui::ph::X).on_hover_text("Forget the built area (start a new one)").clicked() {
                        self.build.area = None;
                    }
                });
            }
            None => {
                ui.weak("What you build here can then be saved as a structure in one click.");
            }
        }
        if save {
            let id = self.build.structure_id.clone();
            let r = self.save_built_structure(&id);
            self.show_result(r);
        }
    }

    /// `build=` for the editor's `status`: mode:wall:sides:style[:storeysN][:roof][:opening:door:window][:area=WxH@X,Y].
    pub fn build_status(&self) -> String {
        let b = &self.build;
        let mut s = format!("{:?}:{}:{}:{}", b.mode, b.wall, if b.all_sides { "all" } else { "back" }, b.style).to_lowercase();
        if b.storeys > 1 {
            s += &format!(":storeys{}", b.storeys);
        }
        if b.mode == BuildMode::Roof {
            s += &format!(":{:?}", b.roof).to_lowercase();
        }
        if b.mode == BuildMode::Openings {
            s += &format!(":{:?}:{}:{}", b.opening, b.door, b.window).to_lowercase();
        }
        if let Some((x0, y0, x1, y1)) = b.area {
            s += &format!(":area={}x{}@{x0},{y0}", x1 - x0 + 1, y1 - y0 + 1);
        }
        s
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
