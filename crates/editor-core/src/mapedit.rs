//! Changing the map being worked on: what the map view's tools, its Inspector, the commands and
//! scripts do. Each function here ends as one step of the map's history (found and named by
//! [`Workspace::history_frame`]).

use gfx::math::{vec2, Vec2};
use rustpg2d_engine::decor::{Decor, DecorLayer};
use rustpg2d_engine::entity::{EntityInstance, EntityKind};
use rustpg2d_engine::map::{Zone, ZoneKind};
use rustpg2d_engine::refactor::RefKind;
use serde_json::Value;

use crate::doc::Objects;
use crate::workspace::{Selection, Workspace};

/// The kind of definition a placed thing of `kind` is one of.
pub fn kind_ref(kind: EntityKind) -> RefKind {
    match kind {
        EntityKind::Prop => RefKind::Prop,
        EntityKind::Character => RefKind::Character,
        EntityKind::Item => RefKind::Item,
    }
}

/// The kind of placed thing a definition kind gives (`None`: it isn't placed as an object).
pub fn entity_kind(kind: RefKind) -> Option<EntityKind> {
    match kind {
        RefKind::Prop => Some(EntityKind::Prop),
        RefKind::Character => Some(EntityKind::Character),
        RefKind::Item => Some(EntityKind::Item),
        _ => None,
    }
}

/// A map's objects and scenery at one moment: what [`Workspace::commit_map`] compares with.
pub struct MapSnap {
    objects: Objects,
    decor: Vec<Decor>,
}

/// A point of a placed character's day on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// A waypoint of its patrol route, by its place in the route.
    Waypoint(usize),
    /// A stop of its daily schedule, by its place in the list.
    Stop(usize),
}

/// Things copied from a map ([`Workspace::copy_objects`]): pasted on any map of the project.
#[derive(Clone, Debug, Default)]
pub struct Clip {
    entities: Vec<EntityInstance>,
    zones: Vec<Zone>,
    /// Scenery with its prop (each map numbers its props itself).
    decor: Vec<(String, Decor)>,
    /// A region of tiles copied with its ground, scenery, blocked tiles and the things standing
    /// in it (zones aren't copied), as a structure is captured.
    region: Option<rustpg2d_engine::defs::StructureDef>,
    /// The tile the north-west one of them stood on.
    origin: (i32, i32),
    /// The map they were copied from.
    pub from: String,
}

impl Clip {
    /// How many things it holds (a region: the things standing in it, at least one).
    pub fn len(&self) -> usize {
        let region = self.region.as_ref().map_or(0, |r| (r.entities.len() + r.decor.len()).max(1));
        self.entities.len() + self.zones.len() + self.decor.len() + region
    }

    /// The size in tiles of a region copied with its ground.
    pub fn region_size(&self) -> Option<(u32, u32)> {
        self.region.as_ref().map(|r| (r.w, r.h))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// How the brush of the Terrain tool paints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    /// The radius in world pixels (a line's half width).
    pub size: f32,
    pub square: bool,
    /// Squares, rectangles and line ends on whole tiles (else on the ground's cells).
    pub snap: bool,
}

impl Brush {
    fn grid(&self) -> f32 {
        if self.snap {
            rustpg2d_engine::project::TILE_SIZE as f32
        } else {
            rustpg2d_engine::ground::CELL as f32
        }
    }

    /// A rectangle's corner: the nearest grid corner.
    pub fn corner(&self, p: Vec2) -> Vec2 {
        (p / self.grid()).round() * self.grid()
    }

    /// A line's end: the middle of the grid square under it when snapping.
    pub fn line_end(&self, p: Vec2) -> Vec2 {
        if self.snap {
            ((p / self.grid()).floor() + 0.5) * self.grid()
        } else {
            p
        }
    }

    /// The square brush at `p`: a block of grid squares about the brush's size around the one
    /// under the pointer, as (top-left, bottom-right).
    pub fn square_at(&self, p: Vec2) -> (Vec2, Vec2) {
        let g = self.grid();
        let n = ((self.size * 2.0 / g).round() as i32).max(1);
        let start = (p / g).floor() - vec2(((n - 1) / 2) as f32, ((n - 1) / 2) as f32);
        (start * g, (start + vec2(n as f32, n as f32)) * g)
    }
}

/// What the zone kinds are called when a new one gets its name (`spawn_12`).
fn zone_prefix(kind: &ZoneKind) -> &'static str {
    match kind {
        ZoneKind::Spawn => "spawn",
        ZoneKind::Portal { .. } => "portal",
        ZoneKind::Trigger => "zone",
        ZoneKind::Spawner { .. } => "spawner",
        ZoneKind::Transition { .. } => "transition",
        ZoneKind::FishingSpot { .. } => "fishing",
        ZoneKind::Ambience { .. } => "ambience",
        ZoneKind::Encounter { .. } => "encounter",
        ZoneKind::Drop { .. } => "drop",
        ZoneKind::Trap { .. } => "trap",
    }
}

impl Workspace {
    /// A definition's name for people (its id when it has none).
    pub fn def_label(&self, kind: EntityKind, def: &str) -> String {
        let db = &self.project.db;
        let name = match kind {
            EntityKind::Prop => db.prop(def).map(|d| d.name.clone()),
            EntityKind::Character => db.character(def).map(|d| d.name.clone()),
            EntityKind::Item => db.item(def).map(|d| d.name.clone()),
        };
        name.filter(|n| !n.is_empty()).unwrap_or_else(|| def.to_string())
    }

    /// What a placed thing is called in messages: its name, else its definition's.
    pub fn describe_object(&self, sel: Selection) -> String {
        let Some(doc) = &self.doc else { return String::new() };
        match sel {
            Selection::Entity(uid) => doc.map.entity(uid).map_or_else(String::new, |e| if e.name.is_empty() { self.def_label(e.kind, &e.def) } else { e.name.clone() }),
            Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid).map_or_else(String::new, |z| z.name.clone()),
            Selection::Scenery(d) => doc.map.decor_defs.get(d.def as usize).map_or_else(String::new, |id| self.def_label(EntityKind::Prop, id)),
            Selection::Region(_) | Selection::None => String::new(),
        }
    }

    // ------------------------------------------------------------ whole maps

    /// A new map of `width` x `height` tiles, its ground filled with `fill` when one is named:
    /// written as its file and made the map being worked on (the others stay open). Says
    /// what was made.
    pub fn create_map(&mut self, name: &str, width: u32, height: u32, fill: Option<&str>) -> Result<String, String> {
        use rustpg2d_engine::map::MAX_MAP_SIZE;
        let name = name.trim();
        if !self.project_open {
            return Err("no project is open".into());
        }
        if !crate::rename::valid_name(name) {
            return Err("map names use letters, digits, '_' and '-'".into());
        }
        if self.maps.iter().any(|m| m == name) {
            return Err(format!("a map named \"{name}\" exists"));
        }
        if width == 0 || height == 0 || width > MAX_MAP_SIZE || height > MAX_MAP_SIZE {
            return Err(format!("the size must be 1..{MAX_MAP_SIZE} tiles each way"));
        }
        let mut map = rustpg2d_engine::map::Map::new(name, width, height);
        let fill = fill.filter(|f| !f.is_empty());
        if let Some(f) = fill {
            if self.project.db.material(f).is_none() {
                return Err(format!("\"{f}\" is not a ground material"));
            }
            if let Some(v) = map.material_value(f) {
                map.ground.fill(v);
            }
        }
        self.load_doc(map);
        self.save_map();
        if !self.maps.iter().any(|m| m == name) {
            // (Its file couldn't be written: the status says why.)
            self.close_map(name);
            return Err(self.status.0.clone());
        }
        self.search_stale = true;
        Ok(format!("Made the map {name} ({width} x {height} tiles{})", fill.map_or(String::new(), |f| format!(", its ground {f}"))))
    }

    /// The map being worked on made `width` x `height` tiles, as one step: what is at its top
    /// left stays, ground and scenery beyond the new edges go (characters, objects and zones
    /// out there are kept: the check names them).
    pub fn resize_map(&mut self, width: u32, height: u32) -> Result<String, String> {
        use rustpg2d_engine::map::MAX_MAP_SIZE;
        if width == 0 || height == 0 || width > MAX_MAP_SIZE || height > MAX_MAP_SIZE {
            return Err(format!("the size must be 1..{MAX_MAP_SIZE} tiles each way"));
        }
        let assets = &self.assets;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if crate::structdoc::is_scratch(&doc.map.name) {
            return Err("a structure's size is changed in its own tab".into());
        }
        let (name, old) = (doc.map.name.clone(), (doc.map.width, doc.map.height));
        if old == (width, height) {
            return Ok(format!("{name} is {width} x {height} tiles already"));
        }
        let before = doc.map.clone();
        doc.map.resize(width, height);
        doc.record_map(before, assets);
        self.begin_unnamed(&format!("Resize the map {name}"));
        self.stale = true;
        Ok(format!("{name}: {} x {} tiles became {width} x {height} (its top left kept)", old.0, old.1))
    }

    // ------------------------------------------------------------ steps

    /// The map's objects and scenery now: change them, then [`Self::commit_map`].
    pub fn snap_map(&self) -> Option<Box<MapSnap>> {
        self.doc.as_ref().map(|d| Box::new(MapSnap { objects: Objects::of(&d.map), decor: d.map.decor.clone() }))
    }

    /// What changed on the map since `snap` becomes one step of its history called `label`.
    /// False when nothing changed.
    pub fn commit_map(&mut self, snap: MapSnap, label: &str) -> bool {
        self.commit_built(snap, Vec::new(), 0, label)
    }

    /// [`Self::commit_map`] with ground painted in the same step: `cells` (x, y, what was
    /// there) now hold `value`.
    pub fn commit_built(&mut self, snap: MapSnap, cells: Vec<(i32, i32, u8)>, value: u8, label: &str) -> bool {
        let Some(doc) = &mut self.doc else { return false };
        // Scenery has no numbers: what is there now and wasn't, and the other way round.
        let mut was: std::collections::HashMap<Decor, i32> = std::collections::HashMap::new();
        for d in &snap.decor {
            *was.entry(*d).or_default() += 1;
        }
        let mut added = Vec::new();
        for d in &doc.map.decor {
            match was.get_mut(d) {
                Some(n) if *n > 0 => *n -= 1,
                _ => added.push(*d),
            }
        }
        let removed: Vec<Decor> = snap.decor.iter().filter(|d| was.get_mut(*d).is_some_and(|n| if *n > 0 { *n -= 1; true } else { false })).copied().collect();
        let objects = Objects::of(&doc.map) != snap.objects;
        if !objects && added.is_empty() && removed.is_empty() && cells.is_empty() {
            return false;
        }
        doc.record_build(cells, value, added, removed, objects.then_some(snap.objects), &self.assets);
        self.begin_unnamed(label);
        self.stale = true;
        true
    }

    // ------------------------------------------------------------ scenery

    /// Whether a prop does something by itself (events, states to switch between, things in
    /// it, a lock, a use, something to gather): placed, it is an object of the map. A prop that
    /// only stands there is scenery.
    pub fn prop_acts(&self, prop: &str) -> bool {
        self.project.db.prop(prop).is_some_and(|p| {
            !p.triggers.is_empty() || p.states.len() > 1 || p.container.is_some() || p.lock.is_some() || p.use_spot.is_some() || p.gather.is_some() || p.regrow.is_some() || p.plot || p.door || p.carry.is_some() || p.climb || p.object.is_some() || p.trap.is_some()
        })
    }

    /// Where scenery of `prop` stands when it is put down at `world`: on the prop's own grid
    /// when it has one, else where the pointer is.
    pub fn scenery_spot(&self, prop: &str, world: Vec2) -> Vec2 {
        match self.project.db.prop(prop).map_or(0, |p| p.snap) {
            0 => world.round(),
            s => crate::mapops::snap_point(world, s as f32),
        }
    }

    /// Puts `prop` down as scenery at `world` (see [`Self::scenery_spot`]), a quarter `turn`ed
    /// when it turns, in `layer` (`None`: on the ground when it is flat, else standing).
    pub fn place_scenery(&mut self, prop: &str, world: Vec2, layer: Option<DecorLayer>, flip: bool, turn: u8) -> Result<Decor, String> {
        let Some(def) = self.project.db.prop(prop) else { return Err(format!("there is no prop \"{prop}\"")) };
        let (flat, turns) = (def.flat, def.turns());
        let at = self.scenery_spot(prop, world);
        let label = self.def_label(EntityKind::Prop, prop);
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if !doc.map.in_bounds((at.x / ts).floor() as i32, ((at.y - 0.5) / ts).floor() as i32) {
            return Err("that is outside the map".into());
        }
        let slot = doc.map.decor_slot(prop);
        let mut d = Decor { def: slot, x: at.x as i32, y: at.y as i32, flip, state: 0, layer: layer.unwrap_or(if flat { DecorLayer::Flat } else { DecorLayer::Sorted }) };
        if turns {
            d.set_turn(turn);
        }
        doc.map.decor.push(d);
        doc.record_decor(vec![d], vec![]);
        self.begin_unnamed(&format!("Place {label}"));
        self.stale = true;
        Ok(d)
    }

    /// A row of `prop` as scenery from `a` to `b` along its grid (walls, fences). Returns how
    /// many were put down.
    pub fn place_row(&mut self, prop: &str, a: Vec2, b: Vec2, layer: Option<DecorLayer>) -> Result<usize, String> {
        let Some(def) = self.project.db.prop(prop) else { return Err(format!("there is no prop \"{prop}\"")) };
        let layer = layer.unwrap_or(if def.flat { DecorLayer::Flat } else { DecorLayer::Sorted });
        let label = self.def_label(EntityKind::Prop, prop);
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let (added, removed) = crate::mapops::wall_line(&mut doc.map, &self.project.db, prop, a, b, layer);
        let n = added.len();
        doc.record_decor(added, removed);
        self.begin_unnamed(&format!("Place a row of {label}"));
        self.stale = true;
        Ok(n)
    }

    /// Replaces a piece of scenery with `new` (its layer, mirroring, state or turn changed).
    pub fn set_scenery(&mut self, old: Decor, new: Decor) -> bool {
        let label = format!("Change {}", self.describe_object(Selection::Scenery(old)));
        let Some(doc) = &mut self.doc else { return false };
        if old == new || !crate::doc::remove_one(&mut doc.map.decor, &old) {
            return false;
        }
        doc.map.decor.push(new);
        doc.record_decor(vec![new], vec![old]);
        self.begin_unnamed(&label);
        true
    }

    /// Scenery becomes an object of the map (with a number, a name to give, events and states):
    /// the same prop on the same spot. Returns its number.
    pub fn make_interactive(&mut self, d: Decor) -> Result<u32, String> {
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(prop) = doc.map.decor_defs.get(d.def as usize).cloned() else { return Err("it is gone from the map".into()) };
        if !crate::doc::remove_one(&mut doc.map.decor, &d) {
            return Err("it is gone from the map".into());
        }
        let uid = doc.map.alloc_uid();
        let mut e = EntityInstance::new(uid, EntityKind::Prop, &prop, d.x as f32, d.y as f32);
        if let Some(def) = self.project.db.prop(&prop) {
            if def.turns() {
                e.facing = d.facing();
            }
            if let Some(state) = def.states.get(d.state_index()).filter(|_| d.state_index() > 0) {
                e.state = state.name.clone();
            }
        }
        self.project.db.upgrade_instance(&mut e);
        doc.map.entities.push(e);
        let label = self.def_label(EntityKind::Prop, &prop);
        self.commit_map(*snap, &format!("Make {label} interactive"));
        Ok(uid)
    }

    /// What keeps a placed prop from being scenery: something of its own that scenery can't
    /// hold. `None`: it can be.
    pub fn keeps_from_scenery(&self, uid: u32) -> Option<&'static str> {
        let e = self.doc.as_ref()?.map.entity(uid)?;
        if e.kind != EntityKind::Prop {
            return Some("only props can be scenery");
        }
        if !e.name.is_empty() {
            return Some("it has a name (events and scripts may use it)");
        }
        if !e.triggers.is_empty() || e.container.is_some() || e.lock.is_some() || e.loot.is_some() || e.light.is_some() {
            return Some("it has events or things of its own");
        }
        None
    }

    /// A placed prop with nothing of its own becomes scenery on the same spot.
    pub fn make_scenery(&mut self, uid: u32) -> Result<Decor, String> {
        if let Some(why) = self.keeps_from_scenery(uid) {
            return Err(why.into());
        }
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(e) = doc.map.entity(uid).cloned() else { return Err("it is gone from the map".into()) };
        let def = self.project.db.prop(&e.def);
        let slot = doc.map.decor_slot(&e.def);
        let state = def.and_then(|p| p.state_index(&e.state)).unwrap_or(0) as u8;
        let mut d = Decor { def: slot, x: e.x.round() as i32, y: e.y.round() as i32, flip: false, state, layer: if def.is_some_and(|p| p.flat) { DecorLayer::Flat } else { DecorLayer::Sorted } };
        if def.is_some_and(|p| p.turns()) {
            d.set_turn(rustpg2d_engine::decor::facing_turn(e.facing));
        }
        doc.map.entities.retain(|x| x.uid != uid);
        doc.map.decor.push(d);
        let label = self.def_label(EntityKind::Prop, &e.def);
        self.commit_map(*snap, &format!("Make {label} scenery"));
        Ok(d)
    }

    /// A dab of scatter set `set` around `at` (radius in world pixels), keeping its spacing to
    /// what stands there. Not a step by itself: [`Self::snap_map`] before the stroke,
    /// [`Self::commit_map`] after it. Returns how many were put down.
    pub fn scatter(&mut self, set: &str, at: Vec2, radius: f32, seed: u64) -> usize {
        let Some(set) = self.project.db.scatter_set(set).cloned() else { return 0 };
        let Some(doc) = &mut self.doc else { return 0 };
        let near: Vec<(f32, f32)> = doc.map.decor.iter().filter(|d| vec2(d.x as f32, d.y as f32).distance(at) <= radius + set.spacing).map(|d| (d.x as f32, d.y as f32)).collect();
        let (w, h) = (doc.map.width as f32 * self.assets.tile_size, doc.map.height as f32 * self.assets.tile_size);
        let mut n = 0;
        for (prop, x, y, flip) in set.scatter((at.x, at.y), radius, &near, seed) {
            if x < 0.0 || y < 0.0 || x >= w || y > h {
                continue;
            }
            let slot = doc.map.decor_slot(&prop);
            doc.map.decor.push(Decor { def: slot, x: x as i32, y: y as i32, flip, state: 0, layer: set.layer });
            n += 1;
        }
        n
    }

    /// The tiles the things of `what` stand on, as one rectangle `(x, y, w, h)`: what *Save as
    /// structure* takes. Big things count with their footprint.
    pub fn tiles_of(&self, what: &[Selection]) -> Option<(u32, u32, u32, u32)> {
        let map = &self.doc.as_ref()?.map;
        let ts = self.ts();
        let mut area: Option<(f32, f32, f32, f32)> = None;
        let mut add = |x0: f32, y0: f32, x1: f32, y1: f32| {
            area = Some(match area {
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
                None => (x0, y0, x1, y1),
            });
        };
        for s in what {
            match *s {
                Selection::Entity(uid) => {
                    let Some(e) = map.entity(uid) else { continue };
                    let [w, h] = match e.kind {
                        EntityKind::Prop => self.project.db.prop(&e.def).map_or([1.0, 1.0], |p| p.hitbox_facing(e.facing)),
                        _ => [1.0, 1.0],
                    };
                    add(e.x - w / 2.0, e.y - h, e.x + w / 2.0, e.y);
                }
                Selection::Scenery(d) => {
                    let [w, h] = map.decor_defs.get(d.def as usize).and_then(|id| self.project.db.prop(id)).map_or([1.0, 1.0], |p| if p.turns() { p.hitbox_facing(d.facing()) } else { p.hitbox });
                    add(d.x as f32 - w / 2.0, d.y as f32 - h, d.x as f32 + w / 2.0, d.y as f32);
                }
                Selection::Zone(uid) => {
                    if let Some(z) = map.zones.iter().find(|z| z.uid == uid) {
                        add(z.x as f32 * ts, z.y as f32 * ts, (z.x + z.w) as f32 * ts, (z.y + z.h) as f32 * ts);
                    }
                }
                Selection::Region(_) | Selection::None => {}
            }
        }
        let (x0, y0, x1, y1) = area?;
        let (tx0, ty0) = ((x0 / ts).floor().max(0.0) as u32, (y0 / ts).floor().max(0.0) as u32);
        let (tx1, ty1) = (((x1 - 0.5) / ts).floor().max(0.0) as u32, ((y1 - 0.5) / ts).floor().max(0.0) as u32);
        let (tx1, ty1) = (tx1.min(map.width.saturating_sub(1)), ty1.min(map.height.saturating_sub(1)));
        (tx1 >= tx0 && ty1 >= ty0).then_some((tx0, ty0, tx1 - tx0 + 1, ty1 - ty0 + 1))
    }

    /// Makes a structure called `name` of the tiles `rect` of the map being worked on: what
    /// stands there, and the painted ground when `ground` (off for things that sit on any
    /// ground). Returns its id (made from the name).
    pub fn save_as_structure(&mut self, rect: (u32, u32, u32, u32), name: &str, ground: bool) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("the structure needs a name".into());
        }
        let base = crate::defs::id_from_name(name);
        let id = rustpg2d_engine::refactor::unique_id(&base, |c| self.project.db.structure(c).is_some());
        let Some(doc) = &self.doc else { return Err("no map is open".into()) };
        if rect.2 == 0 || rect.3 == 0 || rect.0 + rect.2 > doc.map.width || rect.1 + rect.3 > doc.map.height {
            return Err("that is outside the map".into());
        }
        let d = rustpg2d_engine::defs::StructureDef::capture(&doc.map, &id, name, rect, ground);
        self.begin(&format!("Save {name} as a structure"));
        self.project.db.structures.push(d);
        self.project.db.reindex();
        self.defs_dirty = true;
        self.stale = true;
        Ok(id)
    }

    /// Places structure `id` with its middle on `tile`. Returns the numbers of what it added.
    pub fn place_structure(&mut self, id: &str, tile: (i32, i32)) -> Result<Vec<u32>, String> {
        let Some(def) = self.project.db.structure(id).cloned() else { return Err(format!("there is no structure \"{id}\"")) };
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let origin = (tile.0 - (def.w as i32 - 1) / 2, tile.1 - (def.h as i32 - 1) / 2);
        let made = doc.place_structure(&def, origin.0, origin.1, &self.assets);
        self.begin_unnamed(&format!("Place {}", if def.name.is_empty() { id } else { &def.name }));
        self.stale = true;
        Ok(made)
    }

    // ------------------------------------------------------------ the ground

    /// The value the map's ground holds for `material` (registering it on the map); "" = no
    /// ground (erasing).
    pub fn ground_value(&mut self, material: &str) -> Result<u8, String> {
        if material.is_empty() {
            return Ok(0);
        }
        if self.project.db.material(material).is_none() {
            return Err(format!("there is no material \"{material}\""));
        }
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        doc.map.material_value(material).ok_or_else(|| format!("A map can use at most {} materials", rustpg2d_engine::ground::MAX_MATERIALS))
    }

    /// One dab of the brush at `p`. Returns the cells it changed (for [`Self::ground_commit`]).
    pub fn ground_dab(&mut self, brush: Brush, p: Vec2, value: u8) -> Vec<(i32, i32, u8)> {
        let Some(doc) = &mut self.doc else { return Vec::new() };
        if brush.square {
            let (a, b) = brush.square_at(p);
            doc.map.ground.paint_rect((a.x, a.y), (b.x, b.y), value)
        } else {
            doc.map.ground.paint_circle((p.x, p.y), brush.size, value)
        }
    }

    /// The brush dragged from `a` to `b`.
    pub fn ground_stroke(&mut self, brush: Brush, a: Vec2, b: Vec2, value: u8) -> Vec<(i32, i32, u8)> {
        if brush.square {
            // Dabs every few pixels along the way.
            let steps = (b.distance(a) / (rustpg2d_engine::ground::CELL as f32 * 0.5)).ceil().max(1.0) as i32;
            return (1..=steps).flat_map(|k| self.ground_dab(brush, a.lerp(b, k as f32 / steps as f32), value)).collect();
        }
        let Some(doc) = &mut self.doc else { return Vec::new() };
        doc.map.ground.paint_line((a.x, a.y), (b.x, b.y), brush.size * 2.0, value)
    }

    /// A straight line as wide as the brush (a road, a stream).
    pub fn ground_line(&mut self, brush: Brush, a: Vec2, b: Vec2, value: u8) -> Vec<(i32, i32, u8)> {
        let (a, b) = (brush.line_end(a), brush.line_end(b));
        let Some(doc) = &mut self.doc else { return Vec::new() };
        doc.map.ground.paint_line((a.x, a.y), (b.x, b.y), brush.size * 2.0, value)
    }

    /// A filled rectangle between two corners (a floor, a field).
    pub fn ground_rect(&mut self, brush: Brush, a: Vec2, b: Vec2, value: u8) -> Vec<(i32, i32, u8)> {
        let (a, b) = (brush.corner(a), brush.corner(b));
        let Some(doc) = &mut self.doc else { return Vec::new() };
        doc.map.ground.paint_rect((a.x, a.y), (b.x, b.y), value)
    }

    /// The connected area of one material (or of no ground) around `p`.
    pub fn ground_fill(&mut self, p: Vec2, value: u8) -> Vec<(i32, i32, u8)> {
        let c = rustpg2d_engine::ground::CELL as f32;
        let Some(doc) = &mut self.doc else { return Vec::new() };
        doc.map.ground.flood_fill((p.x / c).floor() as i32, (p.y / c).floor() as i32, value)
    }

    /// The material painted at `p`.
    pub fn ground_at(&self, p: Vec2) -> Option<String> {
        let c = rustpg2d_engine::ground::CELL as f32;
        let map = &self.doc.as_ref()?.map;
        map.material_of(map.ground.get((p.x / c).floor() as i32, (p.y / c).floor() as i32)).map(str::to_string)
    }

    /// Painted cells (as the painting functions returned them, now holding `value`) become one
    /// step. Returns how many cells changed.
    pub fn ground_commit(&mut self, cells: Vec<(i32, i32, u8)>, value: u8) -> usize {
        let Some(doc) = &mut self.doc else { return 0 };
        // (A stroke passes a cell many times: its first value is the one before the stroke.)
        let mut seen = std::collections::HashSet::new();
        let cells: Vec<(i32, i32, u8)> = cells.into_iter().filter(|c| seen.insert((c.0, c.1)) && c.2 != value).collect();
        let n = cells.len();
        if n == 0 {
            return 0;
        }
        let what = doc.map.material_of(value).map(str::to_string);
        doc.record_ground(cells, value, &self.assets);
        let label = match what.and_then(|id| self.project.db.material(&id).map(|m| if m.name.is_empty() { id.clone() } else { m.name.clone() })) {
            Some(name) => format!("Paint {name}"),
            None => "Erase ground".to_string(),
        };
        self.begin_unnamed(&label);
        self.stale = true;
        n
    }

    /// The placed thing or zone called `name` on the map being worked on.
    pub fn find_object(&self, name: &str) -> Option<Selection> {
        let map = &self.doc.as_ref()?.map;
        map.entities.iter().find(|e| e.name == name).map(|e| Selection::Entity(e.uid)).or_else(|| map.zones.iter().find(|z| z.name == name).map(|z| Selection::Zone(z.uid)))
    }

    /// A placed object as data (how the Inspector's form sees it).
    pub fn entity_value(&self, uid: u32) -> Option<Value> {
        self.doc.as_ref()?.map.entity(uid).and_then(|e| serde_json::to_value(e).ok())
    }

    /// Replaces placed object `uid` with `value` (its data). Nothing changes when the value
    /// isn't a valid object: the error says why.
    pub fn set_entity_value(&mut self, uid: u32, value: Value) -> Result<(), String> {
        let mut new: EntityInstance = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(old) = doc.map.entity(uid) else { return Err("it is gone from the map".into()) };
        if new.uid != uid || new.kind != old.kind {
            return Err("what a placed thing is doesn't change: place another".into());
        }
        if !self.project.db.has(kind_ref(new.kind), &new.def) {
            return Err(format!("there is no {} \"{}\"", kind_ref(new.kind).name(), new.def));
        }
        if new == *old {
            return Ok(());
        }
        self.project.db.upgrade_instance(&mut new);
        let before = Objects::of(&doc.map);
        if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
            *e = new;
        }
        doc.record_objects(before, "inspector");
        let what = self.describe_object(Selection::Entity(uid));
        self.begin_unnamed(&format!("Change {what}"));
        self.stale = true;
        Ok(())
    }

    /// The settings of the map being worked on as data (light, weather, music, what lies
    /// beyond its edges, its own events).
    pub fn map_settings_value(&self) -> Option<Value> {
        serde_json::to_value(rustpg2d_engine::map::MapSettings::of(&self.doc.as_ref()?.map)).ok()
    }

    /// Replaces the map's settings with `value`. Nothing changes when it isn't valid settings,
    /// or names music, a sheet or a map the project doesn't have.
    pub fn set_map_settings_value(&mut self, value: Value) -> Result<(), String> {
        let new: rustpg2d_engine::map::MapSettings = serde_json::from_value(value).map_err(|e| e.to_string())?;
        for m in std::iter::once(&new.music).chain(new.battle_music.iter()).filter(|m| !m.is_empty()) {
            if self.project.db.sound(m).is_none() {
                return Err(format!("there is no sound \"{m}\""));
            }
        }
        for (side, e) in [("north", &new.edges.north), ("south", &new.edges.south), ("east", &new.edges.east), ("west", &new.edges.west)] {
            if let Some(e) = e.as_ref().filter(|e| !self.maps.contains(&e.target_map)) {
                return Err(format!("the {side} edge leads to \"{}\", which isn't a map", e.target_map));
            }
        }
        if let Some(l) = new.parallax.iter().find(|l| self.project.db.sheet(&l.sheet).is_none()) {
            return Err(format!("there is no sprite sheet \"{}\"", l.sheet));
        }
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if new == rustpg2d_engine::map::MapSettings::of(&doc.map) {
            return Ok(());
        }
        let before = Objects::of(&doc.map);
        new.apply(&mut doc.map);
        doc.record_objects(before, "map-settings");
        self.begin_unnamed("Change the map's settings");
        self.stale = true;
        Ok(())
    }

    /// A zone as data.
    pub fn zone_value(&self, uid: u32) -> Option<Value> {
        self.doc.as_ref()?.map.zones.iter().find(|z| z.uid == uid).and_then(|z| serde_json::to_value(z).ok())
    }

    /// Replaces zone `uid` with `value`. Nothing changes when the value isn't a valid zone, or
    /// the zone would leave the map.
    pub fn set_zone_value(&mut self, uid: u32, value: Value) -> Result<(), String> {
        let new: Zone = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(old) = doc.map.zones.iter().find(|z| z.uid == uid) else { return Err("it is gone from the map".into()) };
        if new.uid != uid {
            return Err("a zone keeps its number".into());
        }
        if new.name.trim().is_empty() {
            return Err("a zone needs a name: events and other maps find it by it".into());
        }
        if doc.map.zones.iter().any(|z| z.uid != uid && z.name == new.name) {
            return Err(format!("another zone of this map is called \"{}\"", new.name));
        }
        if new.w == 0 || new.h == 0 || new.x + new.w > doc.map.width || new.y + new.h > doc.map.height {
            return Err(format!("the zone must lie on the map ({} x {} tiles)", doc.map.width, doc.map.height));
        }
        if new == *old {
            return Ok(());
        }
        let before = Objects::of(&doc.map);
        if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) {
            *z = new;
        }
        doc.record_objects(before, "inspector-zone");
        let what = self.describe_object(Selection::Zone(uid));
        self.begin_unnamed(&format!("Change zone {what}"));
        self.stale = true;
        Ok(())
    }

    /// Where something of `def` stands when it is put down at `world`: on the tile grid (a prop
    /// an even number of tiles wide on a tile edge, so it covers whole tiles), or anywhere when
    /// `free`.
    pub fn snap_feet(&self, kind: EntityKind, def: &str, world: Vec2, free: bool) -> Vec2 {
        if free {
            return world.round();
        }
        rustpg2d_engine::grid::snap_feet(world, self.def_tiles_wide(kind, def), self.ts())
    }

    /// Places a `def` with its feet at `feet` (world pixels). Returns its number on the map.
    pub fn place_entity(&mut self, kind: EntityKind, def: &str, feet: Vec2) -> Result<u32, String> {
        if !self.project.db.has(kind_ref(kind), def) {
            return Err(format!("there is no {} \"{def}\"", kind_ref(kind).name()));
        }
        let ts = self.ts();
        let label = self.def_label(kind, def);
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if !doc.map.in_bounds((feet.x / ts).floor() as i32, ((feet.y - 0.5) / ts).floor() as i32) {
            return Err("that is outside the map".into());
        }
        let before = Objects::of(&doc.map);
        let uid = doc.map.alloc_uid();
        let mut e = EntityInstance::new(uid, kind, def, feet.x, feet.y);
        self.project.db.upgrade_instance(&mut e);
        doc.map.entities.push(e);
        doc.record_objects(before, "place");
        self.begin_unnamed(&format!("Place {label}"));
        self.stale = true;
        Ok(uid)
    }

    /// Adds a zone of `kind` over the tiles `(x, y, w, h)` (cut to the map). Returns its number.
    pub fn add_zone(&mut self, kind: ZoneKind, rect: (i32, i32, i32, i32)) -> Result<u32, String> {
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
        let (x0, y0) = (rect.0.max(0), rect.1.max(0));
        let (x1, y1) = ((rect.0 + rect.2).min(mw), (rect.1 + rect.3).min(mh));
        if x1 <= x0 || y1 <= y0 {
            return Err("that is outside the map".into());
        }
        let before = Objects::of(&doc.map);
        let uid = doc.map.alloc_uid();
        let prefix = zone_prefix(&kind);
        let name = crate::mapops::unique_zone_name(&doc.map, &format!("{prefix}_{uid}"));
        doc.map.zones.push(Zone { uid, name, x: x0 as u32, y: y0 as u32, w: (x1 - x0) as u32, h: (y1 - y0) as u32, kind, triggers: Vec::new(), loot: None });
        doc.record_objects(before, "zone");
        self.begin_unnamed(&format!("Add a {prefix} zone"));
        self.stale = true;
        Ok(uid)
    }

    /// Removes placed things, scenery and zones from the map. Returns how many went.
    pub fn delete_objects(&mut self, what: &[Selection]) -> usize {
        // A region of tiles is emptied: its ground, scenery, blocked tiles and things (as *Cut*
        // takes them; zones stay). One step.
        if let [Selection::Region(r)] = what {
            let Some(doc) = &mut self.doc else { return 0 };
            doc.clear_region(*r, &self.assets);
            self.begin_unnamed(&format!("Clear a region of {} x {} tiles", r.2, r.3));
            self.stale = true;
            return 1;
        }
        let label = match what {
            [one] => format!("Delete {}", self.describe_object(*one)),
            _ => format!("Delete {} things", what.len()),
        };
        let Some(snap) = self.snap_map() else { return 0 };
        let Some(doc) = &mut self.doc else { return 0 };
        let n = doc.map.entities.len() + doc.map.zones.len() + doc.map.decor.len();
        doc.map.entities.retain(|e| !what.contains(&Selection::Entity(e.uid)));
        doc.map.zones.retain(|z| !what.contains(&Selection::Zone(z.uid)));
        for s in what {
            if let Selection::Scenery(d) = s {
                crate::doc::remove_one(&mut doc.map.decor, d);
            }
        }
        let gone = n - doc.map.entities.len() - doc.map.zones.len() - doc.map.decor.len();
        self.commit_map(*snap, &label);
        gone
    }

    /// Copies placed things, scenery and zones, a tile to the east of themselves. Returns the
    /// copies.
    pub fn duplicate_objects(&mut self, what: &[Selection]) -> Vec<Selection> {
        let ts = self.ts();
        let label = match what {
            [one] => format!("Duplicate {}", self.describe_object(*one)),
            _ => format!("Duplicate {} things", what.len()),
        };
        let Some(snap) = self.snap_map() else { return Vec::new() };
        let Some(doc) = &mut self.doc else { return Vec::new() };
        let mut made = Vec::new();
        for s in what {
            match *s {
                Selection::Entity(uid) => {
                    let Some(mut e) = doc.map.entity(uid).cloned() else { continue };
                    e.uid = doc.map.alloc_uid();
                    e.x += ts;
                    if !e.name.is_empty() {
                        e.name = format!("{}_copy", e.name);
                    }
                    made.push(Selection::Entity(e.uid));
                    doc.map.entities.push(e);
                }
                Selection::Zone(uid) => {
                    let Some(mut z) = doc.map.zones.iter().find(|z| z.uid == uid).cloned() else { continue };
                    z.uid = doc.map.alloc_uid();
                    z.x = (z.x + 1).min(doc.map.width.saturating_sub(z.w));
                    z.name = crate::mapops::unique_zone_name(&doc.map, &format!("{}_copy", z.name));
                    made.push(Selection::Zone(z.uid));
                    doc.map.zones.push(z);
                }
                Selection::Scenery(d) => {
                    if !doc.map.decor.contains(&d) {
                        continue;
                    }
                    let copy = Decor { x: d.x + ts as i32, ..d };
                    doc.map.decor.push(copy);
                    made.push(Selection::Scenery(copy));
                }
                Selection::Region(_) | Selection::None => {}
            }
        }
        self.commit_map(*snap, &label);
        made
    }

    // ------------------------------------------------------------ a character's day

    /// Adds a waypoint on `tile` to placed character `uid`'s patrol route (its first one starts
    /// the route). One step. Returns how many it has now.
    pub fn add_waypoint(&mut self, uid: u32, tile: (i32, i32)) -> Result<usize, String> {
        let who = self.describe_object(Selection::Entity(uid));
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        if !doc.map.in_bounds(tile.0, tile.1) {
            return Err("that is outside the map".into());
        }
        let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid && e.kind == EntityKind::Character) else { return Err("only a character walks a route".into()) };
        let route = e.patrol.get_or_insert_with(Default::default);
        if route.points.last() == Some(&tile) {
            return Err("its last waypoint is there already".into());
        }
        route.points.push(tile);
        let n = route.points.len();
        self.commit_map(*snap, &format!("Add a waypoint for {who}"));
        Ok(n)
    }

    /// Takes the last waypoint of placed character `uid`'s patrol route away (the last one
    /// left: the route with it). One step. False when it has none.
    pub fn remove_waypoint(&mut self, uid: u32) -> bool {
        let who = self.describe_object(Selection::Entity(uid));
        let Some(snap) = self.snap_map() else { return false };
        let Some(e) = self.doc.as_mut().and_then(|d| d.map.entities.iter_mut().find(|e| e.uid == uid)) else { return false };
        let Some(route) = &mut e.patrol else { return false };
        route.points.pop();
        if route.points.is_empty() {
            e.patrol = None;
        }
        self.commit_map(*snap, &format!("Take a waypoint of {who} away"))
    }

    /// Where the points of placed character `uid`'s day are on this map: its patrol route's
    /// waypoints and its schedule's stops here, as tiles.
    pub fn spots_of(&self, uid: u32) -> Vec<(Spot, (i32, i32))> {
        let Some(doc) = &self.doc else { return Vec::new() };
        let Some(e) = doc.map.entity(uid) else { return Vec::new() };
        let route = e.patrol.iter().flat_map(|r| r.points.iter().enumerate().map(|(k, p)| (Spot::Waypoint(k), *p)));
        let stops = e.schedule.iter().enumerate().filter(|(_, s)| s.map.is_empty() || s.map == doc.map.name).map(|(k, s)| (Spot::Stop(k), (s.x, s.y)));
        route.chain(stops).collect()
    }

    /// Puts a point of placed character `uid`'s day on `tile`. No step by itself (a drag moves
    /// it many times): take [`Self::snap_map`] before and [`Self::commit_map`] after.
    pub fn set_spot(&mut self, uid: u32, spot: Spot, tile: (i32, i32)) {
        let Some(doc) = &mut self.doc else { return };
        if !doc.map.in_bounds(tile.0, tile.1) {
            return;
        }
        let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) else { return };
        match spot {
            Spot::Waypoint(k) => {
                if let Some(p) = e.patrol.as_mut().and_then(|r| r.points.get_mut(k)) {
                    *p = tile;
                }
            }
            Spot::Stop(k) => {
                if let Some(s) = e.schedule.get_mut(k) {
                    (s.x, s.y) = tile;
                }
            }
        }
    }

    /// What moving a point of placed character `uid`'s day is called in the history.
    pub fn spot_label(&self, uid: u32, spot: Spot) -> String {
        let who = self.describe_object(Selection::Entity(uid));
        match spot {
            Spot::Waypoint(k) => format!("Move waypoint {} of {who}", k + 1),
            Spot::Stop(k) => {
                let time = self.doc.as_ref().and_then(|d| d.map.entity(uid)).and_then(|e| e.schedule.get(k)).map_or_else(String::new, |s| s.time_text());
                format!("Move {who}'s {time} stop")
            }
        }
    }

    /// The things `what` as they are, to be pasted on this map or another (`None`: nothing of
    /// them is there).
    pub fn copy_objects(&self, what: &[Selection]) -> Option<Clip> {
        let ts = self.ts();
        let doc = self.doc.as_ref()?;
        // A region of tiles: with its ground, as a structure is captured.
        if let Some(Selection::Region(r)) = what.iter().find(|s| matches!(s, Selection::Region(_))) {
            let region = rustpg2d_engine::defs::StructureDef::capture(&doc.map, "clipboard", "Clipboard", *r, true);
            return Some(Clip { region: Some(region), origin: (r.0 as i32, r.1 as i32), from: doc.map.name.clone(), ..Default::default() });
        }
        let mut clip = Clip::default();
        let tile = |x: f32, y: f32| ((x / ts).floor() as i32, ((y - 0.5) / ts).floor() as i32);
        let mut tiles = Vec::new();
        for s in what {
            match *s {
                Selection::Entity(uid) => {
                    if let Some(e) = doc.map.entity(uid) {
                        tiles.push(tile(e.x, e.y));
                        clip.entities.push(e.clone());
                    }
                }
                Selection::Zone(uid) => {
                    if let Some(z) = doc.map.zones.iter().find(|z| z.uid == uid) {
                        tiles.push((z.x as i32, z.y as i32));
                        clip.zones.push(z.clone());
                    }
                }
                Selection::Scenery(d) => {
                    if let Some(prop) = doc.map.decor_defs.get(d.def as usize).filter(|_| doc.map.decor.contains(&d)) {
                        tiles.push(tile(d.x as f32, d.y as f32));
                        clip.decor.push((prop.clone(), d));
                    }
                }
                Selection::Region(_) | Selection::None => {}
            }
        }
        clip.origin = (tiles.iter().map(|t| t.0).min()?, tiles.iter().map(|t| t.1).min()?);
        clip.from = doc.map.name.clone();
        Some(clip)
    }

    /// Puts copied things on the map being worked on, the tile their north-west one stood on
    /// landing on `tile`: with new numbers, names that are free here, zones cut to the map; what
    /// would stand outside the map is left out. One step. Returns what was put down.
    pub fn paste_objects(&mut self, clip: &Clip, tile: (i32, i32)) -> Vec<Selection> {
        if let Some(region) = &clip.region {
            return self.paste_region(region, tile);
        }
        let ts = self.ts();
        let Some(snap) = self.snap_map() else { return Vec::new() };
        let Some(doc) = &mut self.doc else { return Vec::new() };
        let (dx, dy) = (tile.0 - clip.origin.0, tile.1 - clip.origin.1);
        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
        let inside = |x: f32, y: f32| x >= 0.0 && y >= 0.0 && x < mw as f32 * ts && y <= mh as f32 * ts;
        let mut made = Vec::new();
        for e in &clip.entities {
            let mut e = e.clone();
            (e.x, e.y) = (e.x + dx as f32 * ts, e.y + dy as f32 * ts);
            if !inside(e.x, e.y) {
                continue;
            }
            e.uid = doc.map.alloc_uid();
            if !e.name.is_empty() {
                e.name = rustpg2d_engine::refactor::unique_id(&e.name, |c| doc.map.entities.iter().any(|o| o.name == c));
            }
            made.push(Selection::Entity(e.uid));
            doc.map.entities.push(e);
        }
        for z in &clip.zones {
            let mut z = z.clone();
            let (x, y) = (z.x as i32 + dx, z.y as i32 + dy);
            if x >= mw || y >= mh || x + z.w as i32 <= 0 || y + z.h as i32 <= 0 {
                continue;
            }
            let (x0, y0) = (x.max(0), y.max(0));
            (z.w, z.h) = (((x + z.w as i32).min(mw) - x0) as u32, ((y + z.h as i32).min(mh) - y0) as u32);
            (z.x, z.y) = (x0 as u32, y0 as u32);
            z.uid = doc.map.alloc_uid();
            z.name = crate::mapops::unique_zone_name(&doc.map, &z.name);
            made.push(Selection::Zone(z.uid));
            doc.map.zones.push(z);
        }
        for (prop, d) in &clip.decor {
            let new = Decor { def: doc.map.decor_slot(prop), x: d.x + dx * ts as i32, y: d.y + dy * ts as i32, ..*d };
            if inside(new.x as f32, new.y as f32) && !doc.map.decor.contains(&new) {
                doc.map.decor.push(new);
                made.push(Selection::Scenery(new));
            }
        }
        let label = match made.as_slice() {
            [one] => format!("Paste {}", self.describe_object(*one)),
            many => format!("Paste {} things", many.len()),
        };
        self.commit_map(*snap, &label);
        made
    }

    /// A region copied with its ground put down with its north-west tile on `tile`: its
    /// ground, scenery, blocked tiles and things (named ones get names free on this map), what
    /// falls outside the map left out. One step. Returns the region where it landed.
    fn paste_region(&mut self, region: &rustpg2d_engine::defs::StructureDef, tile: (i32, i32)) -> Vec<Selection> {
        let Some(doc) = &mut self.doc else { return Vec::new() };
        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
        let (x0, y0) = (tile.0.max(0), tile.1.max(0));
        let (x1, y1) = ((tile.0 + region.w as i32).min(mw), (tile.1 + region.h as i32).min(mh));
        if x1 <= x0 || y1 <= y0 {
            return Vec::new();
        }
        let mut region = region.clone();
        let mut taken: std::collections::HashSet<String> = doc.map.entities.iter().map(|e| e.name.clone()).filter(|n| !n.is_empty()).collect();
        for e in region.entities.iter_mut().filter(|e| !e.name.is_empty()) {
            e.name = rustpg2d_engine::refactor::unique_id(&e.name, |c| taken.contains(c));
            taken.insert(e.name.clone());
        }
        doc.place_structure(&region, tile.0, tile.1, &self.assets);
        self.begin_unnamed(&format!("Paste a region of {} x {} tiles", region.w, region.h));
        self.stale = true;
        vec![Selection::Region((x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))]
    }

    /// The placed things and scenery of `what` that are of `kind` become definition `id` of
    /// that kind, each where it stands, as it is otherwise (name, events, facing). One step.
    /// Returns the selection as it is now, or why nothing can be replaced.
    pub fn replace_objects(&mut self, what: &[Selection], kind: RefKind, id: &str) -> Result<Vec<Selection>, String> {
        let Some(ek) = entity_kind(kind) else { return Err(format!("a {} isn't placed on a map", kind.name())) };
        let known = match ek {
            EntityKind::Prop => self.project.db.prop(id).is_some(),
            EntityKind::Character => self.project.db.character(id).is_some(),
            EntityKind::Item => self.project.db.item(id).is_some(),
        };
        if !known {
            return Err(format!("there is no {} \"{id}\"", kind.name()));
        }
        let Some(snap) = self.snap_map() else { return Err("no map is open".into()) };
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let (mut now, mut n) = (Vec::new(), 0);
        for s in what {
            match *s {
                Selection::Entity(uid) => {
                    if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid && e.kind == ek && e.def != id) {
                        e.def = id.to_string();
                        n += 1;
                    }
                    now.push(*s);
                }
                Selection::Scenery(d) if ek == EntityKind::Prop => {
                    let slot = doc.map.decor_slot(id);
                    // (A state the new prop may not have: back to its first.)
                    let mut new = Decor { def: slot, state: 0, ..d };
                    new.set_turn(d.turn());
                    match doc.map.decor.iter_mut().find(|x| **x == d).filter(|_| slot != d.def) {
                        Some(x) => {
                            *x = new;
                            n += 1;
                            now.push(Selection::Scenery(new));
                        }
                        None => now.push(*s),
                    }
                }
                _ => now.push(*s),
            }
        }
        if n == 0 {
            return Err(format!("nothing selected is a {} to replace", kind.label().to_lowercase()));
        }
        let name = self.def_label(ek, id);
        self.commit_map(*snap, &if n == 1 { format!("Replace with {name}") } else { format!("Replace {n} things with {name}") });
        Ok(now)
    }

    /// Puts the things `what` in a line with the first of them: `row` on its row (the same
    /// distance from the map's north edge), else in its column. One step. Returns them as they
    /// are now.
    pub fn align_objects(&mut self, what: &[Selection], row: bool) -> Vec<Selection> {
        let ts = self.ts();
        let Some(snap) = self.snap_map() else { return what.to_vec() };
        let Some(doc) = &mut self.doc else { return what.to_vec() };
        // Where the first one is: its feet (a zone: its north-west corner).
        let lead = what.iter().find_map(|s| match *s {
            Selection::Entity(uid) => doc.map.entity(uid).map(|e| (e.x, e.y)),
            Selection::Scenery(d) => Some((d.x as f32, d.y as f32)),
            Selection::Zone(uid) => doc.map.zones.iter().find(|z| z.uid == uid).map(|z| (z.x as f32 * ts, z.y as f32 * ts)),
            _ => None,
        });
        let Some((lx, ly)) = lead else { return what.to_vec() };
        let (mw, mh) = (doc.map.width, doc.map.height);
        let mut now = Vec::new();
        for s in what {
            match *s {
                Selection::Entity(uid) => {
                    if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                        if row {
                            e.y = ly;
                        } else {
                            e.x = lx;
                        }
                    }
                    now.push(*s);
                }
                Selection::Zone(uid) => {
                    if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) {
                        if row {
                            z.y = ((ly / ts).floor() as u32).min(mh.saturating_sub(z.h));
                        } else {
                            z.x = ((lx / ts).floor() as u32).min(mw.saturating_sub(z.w));
                        }
                    }
                    now.push(*s);
                }
                Selection::Scenery(d) => {
                    let to = if row { Decor { y: ly as i32, ..d } } else { Decor { x: lx as i32, ..d } };
                    match doc.map.decor.iter_mut().find(|x| **x == d) {
                        Some(x) => {
                            *x = to;
                            now.push(Selection::Scenery(to));
                        }
                        None => now.push(*s),
                    }
                }
                Selection::Region(_) | Selection::None => {}
            }
        }
        self.commit_map(*snap, if row { "Put in a row" } else { "Put in a column" });
        now
    }

    /// Moves placed things, scenery and zones by whole tiles (zones stay on the map). Returns
    /// them as they are now (scenery is told by where it is).
    pub fn move_objects(&mut self, what: &[Selection], dx: i32, dy: i32) -> Vec<Selection> {
        let ts = self.ts();
        let Some(snap) = self.snap_map() else { return what.to_vec() };
        let Some(doc) = &mut self.doc else { return what.to_vec() };
        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
        let inside = |x: f32, y: f32| x >= 0.0 && y >= 0.0 && x < mw as f32 * ts && y <= mh as f32 * ts;
        let mut now = Vec::new();
        for s in what {
            match *s {
                Selection::Entity(uid) => {
                    if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
                        let to = vec2(e.x + dx as f32 * ts, e.y + dy as f32 * ts);
                        if inside(to.x, to.y) {
                            (e.x, e.y) = (to.x, to.y);
                        }
                    }
                    now.push(*s);
                }
                Selection::Zone(uid) => {
                    if let Some(z) = doc.map.zones.iter_mut().find(|z| z.uid == uid) {
                        z.x = (z.x as i32 + dx).clamp(0, (mw - z.w as i32).max(0)) as u32;
                        z.y = (z.y as i32 + dy).clamp(0, (mh - z.h as i32).max(0)) as u32;
                    }
                    now.push(*s);
                }
                Selection::Scenery(d) => {
                    let to = Decor { x: d.x + dx * ts as i32, y: d.y + dy * ts as i32, ..d };
                    match doc.map.decor.iter_mut().find(|x| **x == d).filter(|_| inside(to.x as f32, to.y as f32)) {
                        Some(x) => {
                            *x = to;
                            now.push(Selection::Scenery(to));
                        }
                        None => now.push(*s),
                    }
                }
                Selection::Region(_) | Selection::None => {}
            }
        }
        self.commit_map(*snap, "Move");
        now
    }

    /// Turns a placed prop a quarter further (props that turn: a 3D model or pictures from four
    /// sides) or a character to its next side.
    pub fn turn_entity(&mut self, uid: u32) -> Result<(), String> {
        let what = self.describe_object(Selection::Entity(uid));
        let Some(doc) = &mut self.doc else { return Err("no map is open".into()) };
        let Some(e) = doc.map.entity(uid) else { return Err("it is gone from the map".into()) };
        if e.kind == EntityKind::Prop && !self.project.db.prop(&e.def).is_some_and(|d| d.turns()) {
            return Err("Only props with a 3D model or pictures from four sides turn".into());
        }
        if e.kind == EntityKind::Item {
            return Err("Items lie as they are".into());
        }
        let before = Objects::of(&doc.map);
        if let Some(e) = doc.map.entities.iter_mut().find(|e| e.uid == uid) {
            e.facing = rustpg2d_engine::decor::turn_facing(rustpg2d_engine::decor::facing_turn(e.facing) + 1);
        }
        doc.record_objects(before, "turn");
        self.begin_unnamed(&format!("Turn {what}"));
        Ok(())
    }

    /// Turns a piece of scenery a quarter further (props that turn). Returns it as it is now.
    pub fn turn_scenery(&mut self, d: Decor) -> Result<Decor, String> {
        let prop = self.doc.as_ref().and_then(|doc| doc.map.decor_defs.get(d.def as usize).cloned()).unwrap_or_default();
        if !self.project.db.prop(&prop).is_some_and(|p| p.turns()) {
            return Err("Only props with a 3D model or pictures from four sides turn".into());
        }
        let mut new = d;
        new.set_turn((d.turn() + 1) % 4);
        if self.set_scenery(d, new) {
            Ok(new)
        } else {
            Err("it is gone from the map".into())
        }
    }
}

#[cfg(test)]
mod map_tests {
    use crate::Workspace;

    #[test]
    fn a_map_is_made_with_its_ground_and_resized_as_a_step() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-newmap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.history_frame(None);
        // A name that is no file name, one that is taken, a size that is none: refused.
        assert!(ws.create_map("my forest", 20, 15, None).unwrap_err().contains("letters"));
        assert!(ws.create_map("start", 20, 15, None).unwrap_err().contains("exists"));
        assert!(ws.create_map("forest", 0, 15, None).unwrap_err().contains("size"));
        assert!(ws.create_map("forest", 20, 15, Some("no_such_ground")).unwrap_err().contains("not a ground material"));
        // Made: its file is there, it is the map being worked on, the one before stays open.
        ws.open_map("start");
        let said = ws.create_map("forest", 20, 15, None).unwrap();
        assert!(said.contains("forest (20 x 15 tiles)"), "{said}");
        assert!(ws.maps.iter().any(|m| m == "forest"));
        assert_eq!(ws.doc.as_ref().map(|d| (d.map.name.as_str(), d.map.width, d.map.height, d.is_dirty())), Some(("forest", 20, 15, false)));
        assert_eq!(ws.open_maps().len(), 2);
        assert!(ws.project.load_map("forest").is_ok(), "the map's file was written");
        // Resized: one step, undone as one.
        ws.history_frame(None);
        let steps = ws.timeline.done.len();
        let said = ws.resize_map(8, 6).unwrap();
        assert!(said.contains("20 x 15 tiles became 8 x 6"), "{said}");
        ws.history_frame(None);
        assert_eq!(ws.timeline.done.len(), steps + 1);
        assert_eq!(ws.doc.as_ref().map(|d| (d.map.width, d.map.height)), Some((8, 6)));
        ws.run("edit.undo", &[]).unwrap();
        assert_eq!(ws.doc.as_ref().map(|d| (d.map.width, d.map.height)), Some((20, 15)));
        // The same size again is no step; the commands do the same.
        assert!(ws.resize_map(20, 15).unwrap().contains("already"));
        assert!(ws.resize_map(0, 4).is_err());
        let said = ws.run_line("map.new cellar 8 6").unwrap();
        assert!(said.contains("cellar (8 x 6 tiles)"), "{said}");
        assert!(ws.run_line("map.new cellar").unwrap_err().contains("exists"));
        assert!(ws.run_line("map.new attic wide 6").unwrap_err().contains("isn't a number"));
        ws.run_line("map.resize 10 7").unwrap();
        assert_eq!(ws.doc.as_ref().map(|d| (d.map.name.as_str(), d.map.width, d.map.height)), Some(("cellar", 10, 7)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_region_is_copied_with_its_ground_and_pasted_as_one_step() {
        use crate::workspace::Selection;
        use rustpg2d_engine::entity::{EntityInstance, EntityKind};
        use rustpg2d_engine::map::CollisionOverride;
        let dir = std::env::temp_dir().join(format!("rustpg2d-region-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), crate::Gpu::No).unwrap();
        ws.project.db.materials.push(rustpg2d_engine::ground::MaterialDef { id: "stone".into(), name: "Stone".into(), ..Default::default() });
        ws.project.db.reindex();
        ws.open_map("start");
        ws.history_frame(None);
        // A 3 x 3 patch of stone at tiles 2..5, a blocked tile in it, a named barrel on it.
        let stone = ws.ground_value("stone").unwrap();
        let ts = ws.ts();
        let doc = ws.doc.as_mut().unwrap();
        doc.map.ground.paint_rect((2.0 * ts, 2.0 * ts), (5.0 * ts - 0.1, 5.0 * ts - 0.1), stone);
        doc.map.set_collision(3, 3, CollisionOverride::Solid);
        let uid = doc.map.alloc_uid();
        let mut barrel = EntityInstance::new(uid, EntityKind::Prop, "barrel", 3.5 * ts, 3.9 * ts);
        barrel.name = "barrel".into();
        doc.map.entities.push(barrel);
        let at = |x: f32, y: f32| gfx::math::vec2((x + 0.5) * ts, (y + 0.5) * ts);
        assert_eq!(ws.ground_at(at(4.0, 4.0)).as_deref(), Some("stone"));
        // Copied as a region; single things are still copied without ground.
        let clip = ws.copy_objects(&[Selection::Region((2, 2, 3, 3))]).unwrap();
        assert_eq!((clip.region_size(), clip.len()), (Some((3, 3)), 1));
        assert!(ws.copy_objects(&[Selection::Entity(uid)]).unwrap().region_size().is_none());
        // Pasted with its north-west tile on 10, 10: the ground, the blocked tile, the barrel
        // under a name of its own; the region where it landed is what is selected.
        ws.history_frame(None);
        let steps = ws.timeline.done.len();
        let made = ws.paste_objects(&clip, (10, 10));
        assert_eq!(made, [Selection::Region((10, 10, 3, 3))]);
        ws.history_frame(None);
        assert_eq!(ws.timeline.done.len(), steps + 1, "one step");
        assert_eq!(ws.ground_at(at(10.0, 10.0)).as_deref(), Some("stone"));
        assert_eq!(ws.ground_at(at(12.0, 12.0)).as_deref(), Some("stone"));
        assert_eq!(ws.ground_at(at(13.0, 13.0)), None, "only the region");
        let map = &ws.doc.as_ref().unwrap().map;
        assert_eq!(map.collision_at(11, 11), CollisionOverride::Solid);
        assert!(map.entities.iter().any(|e| e.def == "barrel" && e.name == "barrel_2" && (e.x / ts) as i32 == 11), "{:?}", map.entities.iter().map(|e| (&e.def, &e.name, e.x, e.y)).collect::<Vec<_>>());
        // Undone in one step: all of it gone.
        ws.run("edit.undo", &[]).unwrap();
        assert_eq!(ws.ground_at(at(10.0, 10.0)), None);
        let map = &ws.doc.as_ref().unwrap().map;
        assert!(map.collision_at(11, 11) != CollisionOverride::Solid && !map.entities.iter().any(|e| e.name == "barrel_2"));
        // Pasted over the map's edge: what fits.
        assert_eq!(ws.paste_objects(&clip, (38, 28)), [Selection::Region((38, 28, 2, 2))]);
        assert!(ws.paste_objects(&clip, (60, 60)).is_empty());
        // Deleting a region empties it (cut = copy, then this), one step.
        ws.history_frame(None);
        let steps = ws.timeline.done.len();
        assert_eq!(ws.delete_objects(&[Selection::Region((2, 2, 3, 3))]), 1);
        ws.history_frame(None);
        assert_eq!(ws.timeline.done.len(), steps + 1);
        assert_eq!(ws.ground_at(at(3.0, 3.0)), None);
        assert!(!ws.doc.as_ref().unwrap().map.entities.iter().any(|e| e.uid == uid));
        let _ = std::fs::remove_dir_all(dir);
    }
}
