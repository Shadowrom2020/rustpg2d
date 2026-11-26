//! Changing the map being worked on: what the map view's tools, its Inspector, the commands and
//! scripts do. Each function here ends as one step of the map's history (found and named by
//! [`Workspace::history_frame`]).

use gfx::math::{vec2, Vec2};
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
            Selection::Region(_) | Selection::None => String::new(),
        }
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

    /// Removes placed things and zones from the map. Returns how many went.
    pub fn delete_objects(&mut self, what: &[Selection]) -> usize {
        let label = match what {
            [one] => format!("Delete {}", self.describe_object(*one)),
            _ => format!("Delete {} things", what.len()),
        };
        let Some(doc) = &mut self.doc else { return 0 };
        let before = Objects::of(&doc.map);
        let n = doc.map.entities.len() + doc.map.zones.len();
        doc.map.entities.retain(|e| !what.contains(&Selection::Entity(e.uid)));
        doc.map.zones.retain(|z| !what.contains(&Selection::Zone(z.uid)));
        let gone = n - doc.map.entities.len() - doc.map.zones.len();
        if gone > 0 {
            doc.record_objects(before, "delete");
            self.begin_unnamed(&label);
            self.stale = true;
        }
        gone
    }

    /// Copies placed things and zones, a tile to the east of themselves. Returns the copies.
    pub fn duplicate_objects(&mut self, what: &[Selection]) -> Vec<Selection> {
        let ts = self.ts();
        let label = match what {
            [one] => format!("Duplicate {}", self.describe_object(*one)),
            _ => format!("Duplicate {} things", what.len()),
        };
        let Some(doc) = &mut self.doc else { return Vec::new() };
        let before = Objects::of(&doc.map);
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
                Selection::Region(_) | Selection::None => {}
            }
        }
        if !made.is_empty() {
            doc.record_objects(before, "duplicate");
            self.begin_unnamed(&label);
            self.stale = true;
        }
        made
    }

    /// Moves placed things and zones by whole tiles (zones stay on the map).
    pub fn move_objects(&mut self, what: &[Selection], dx: i32, dy: i32) -> usize {
        let ts = self.ts();
        let Some(doc) = &mut self.doc else { return 0 };
        let before = Objects::of(&doc.map);
        let (mw, mh) = (doc.map.width as i32, doc.map.height as i32);
        let mut n = 0;
        for e in doc.map.entities.iter_mut().filter(|e| what.contains(&Selection::Entity(e.uid))) {
            let to = vec2(e.x + dx as f32 * ts, e.y + dy as f32 * ts);
            if to.x >= 0.0 && to.y >= 0.0 && to.x < mw as f32 * ts && to.y <= mh as f32 * ts {
                (e.x, e.y) = (to.x, to.y);
                n += 1;
            }
        }
        for z in doc.map.zones.iter_mut().filter(|z| what.contains(&Selection::Zone(z.uid))) {
            z.x = (z.x as i32 + dx).clamp(0, (mw - z.w as i32).max(0)) as u32;
            z.y = (z.y as i32 + dy).clamp(0, (mh - z.h as i32).max(0)) as u32;
            n += 1;
        }
        doc.record_objects(before, "nudge");
        self.begin_unnamed("Move");
        self.stale = true;
        n
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

    /// A drag on the map view starts: what the map's objects are now, for [`Self::end_drag`].
    pub fn begin_drag(&self) -> Option<Box<Objects>> {
        self.doc.as_ref().map(|d| Box::new(Objects::of(&d.map)))
    }

    /// A drag ended: what it moved since `before` becomes one step called `label`.
    pub fn end_drag(&mut self, before: Objects, label: &str) {
        let Some(doc) = &mut self.doc else { return };
        if Objects::of(&doc.map) != before {
            doc.record_objects(before, "drag");
            self.begin_unnamed(label);
            self.stale = true;
        }
    }
}
