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

    // ------------------------------------------------------------ steps

    /// The map's objects and scenery now: change them, then [`Self::commit_map`].
    pub fn snap_map(&self) -> Option<Box<MapSnap>> {
        self.doc.as_ref().map(|d| Box::new(MapSnap { objects: Objects::of(&d.map), decor: d.map.decor.clone() }))
    }

    /// What changed on the map since `snap` becomes one step of its history called `label`.
    /// False when nothing changed.
    pub fn commit_map(&mut self, snap: MapSnap, label: &str) -> bool {
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
        if !objects && added.is_empty() && removed.is_empty() {
            return false;
        }
        doc.record_build(Vec::new(), 0, added, removed, objects.then_some(snap.objects), &self.assets);
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
