//! Daily routines: a placed character with a schedule (`EntityInstance.schedule`) walks to the
//! stop of the current hour (A*, like patrols) and stays there, doing what its definition's
//! behaviour does (standing, wandering around the stop). A stop can be indoors (out of sight
//! until the next stop), use furniture (`ScheduleStop::prop`: it sits or lies on it) or be on
//! another map (`ScheduleStop::map`): the character walks to the door / portal leading there and
//! leaves; on that map it is a *visitor* built from its placement on its home map (its look,
//! items and triggers come along). Entering a map puts scheduled characters straight at their
//! current stop (they went on with their day while the player was away: other maps aren't
//! simulated).

use macroquad::prelude::*;

use super::patrol::{Walk, Walker};
use super::{Game, RtEntity};
use crate::entity::{current_stop, EntityInstance, ScheduleStop};
use crate::map::ZoneKind;

/// Runtime uids of visitors (`VISITOR_UID_BASE + index`), apart from placed entities,
/// companions and dropped items.
pub(super) const VISITOR_UID_BASE: u32 = 0x5800_0000;
const _: () = assert!(super::companions::COMPANION_UID_BASE < VISITOR_UID_BASE && VISITOR_UID_BASE < crate::state::DROPPED_UID_BASE);

/// Runtime schedule of a character.
#[derive(Clone, Debug, Default)]
pub(super) struct Schedule {
    pub stops: Vec<ScheduleStop>,
    /// The map it is placed on (stops without a map are there).
    pub home: String,
    /// The stop it is at or heading for.
    pub current: Option<usize>,
    /// At the current stop (or gave up reaching it).
    pub arrived: bool,
    /// Followed (false: stopped by the Schedule action / script; saved).
    pub on: bool,
    /// Gone to another map (out of sight here until a stop brings it back).
    pub elsewhere: bool,
    walker: Walker,
}

impl Schedule {
    pub fn new(stops: Vec<ScheduleStop>, home: &str) -> Self {
        Schedule { stops, home: home.to_string(), on: true, ..Default::default() }
    }

    /// The map of stop `k`.
    fn map_of(&self, k: usize) -> &str {
        match self.stops.get(k).map(|s| s.map.as_str()) {
            Some(m) if !m.is_empty() => m,
            _ => &self.home,
        }
    }
}

/// A scheduled character that visits other maps (found once in all maps).
#[derive(Clone, Debug)]
pub(super) struct VisitorDef {
    pub home: String,
    pub inst: EntityInstance,
}

/// What makes a runtime entity a visitor: where it is placed (its triggers come along).
#[derive(Clone, Debug)]
pub(super) struct Visitor {
    pub map: String,
    pub uid: u32,
    pub triggers: Vec<crate::events::Trigger>,
}

impl Game {
    /// The scheduled characters of all maps whose stops reach other maps (loaded once).
    fn visitor_defs(&mut self) -> &[VisitorDef] {
        if self.visitor_defs.is_none() {
            let mut out = Vec::new();
            for name in self.project.list_maps() {
                let map = if self.loaded.map.name == name { Ok(self.loaded.map.clone()) } else { self.project.load_map(&name) };
                let Ok(map) = map else { continue };
                for e in &map.entities {
                    if e.schedule.iter().any(|s| !s.map.is_empty() && s.map != name) {
                        out.push(VisitorDef { home: name.clone(), inst: e.clone() });
                    }
                }
            }
            self.visitor_defs = Some(out);
        }
        self.visitor_defs.as_deref().unwrap_or_default()
    }

    /// Runtime entities of the characters from other maps whose schedule brings them to `map`
    /// (built on entering it; out of sight while their stop is elsewhere).
    pub(super) fn build_visitors(&mut self, map: &str) -> Vec<RtEntity> {
        let defs: Vec<VisitorDef> = self.visitor_defs().iter().filter(|v| v.home != map && v.inst.schedule.iter().any(|s| s.map == map)).cloned().collect();
        let mut out = Vec::new();
        for (k, v) in defs.into_iter().enumerate() {
            // Companions follow the player instead; a stopped schedule keeps it at home.
            let st = self.world.entity_state(&v.home, v.inst.uid);
            if self.world.is_companion(&v.home, v.inst.uid) || st.is_some_and(|s| s.schedule_off || !s.active) {
                continue;
            }
            let mut rt = self.build_placed(&v.home, &v.inst);
            rt.uid = VISITOR_UID_BASE + k as u32;
            rt.patrol = None;
            rt.visitor = Some(Box::new(Visitor { map: v.home.clone(), uid: v.inst.uid, triggers: v.inst.triggers.clone() }));
            out.push(rt);
        }
        out
    }

    /// Puts character `i` at its current stop right away (entering a map), or out of sight
    /// when that stop is on another map.
    pub(super) fn place_on_schedule(&mut self, i: usize) {
        let (hour, here) = (self.world.hour, self.loaded.map.name.clone());
        let e = &mut self.loaded.entities[i];
        let Some(s) = e.schedule.as_mut().filter(|s| s.on) else { return };
        let Some(k) = current_stop(&s.stops, hour) else { return };
        s.current = Some(k);
        s.walker = Walker::default();
        e.npc_seat = None;
        if s.map_of(k) != here {
            s.elsewhere = true;
            s.arrived = true;
            e.active = false;
            e.away = true;
            return;
        }
        s.elsewhere = false;
        e.active = true;
        e.away = false;
        let tile = (s.stops[k].x, s.stops[k].y);
        self.loaded.entities[i].body.pos = self.tile_spot(tile);
        self.arrive(i);
    }

    /// Reached the current stop: face its way, settle there, use its furniture, go inside.
    fn arrive(&mut self, i: usize) {
        let e = &mut self.loaded.entities[i];
        let Some(s) = e.schedule.as_mut() else { return };
        s.arrived = true;
        let Some(stop) = s.current.and_then(|k| s.stops.get(k)).cloned() else { return };
        if let Some(f) = stop.facing {
            e.body.facing = f;
        }
        e.body.moving = false;
        e.home = e.body.pos;
        if stop.inside && e.active {
            e.active = false;
            e.away = true;
            return;
        }
        if !stop.prop.is_empty() {
            self.npc_use_furniture(i, &stop.prop);
        }
    }

    /// The door / portal tile of this map leading to `map` (None: no way there).
    fn exit_to(&self, map: &str) -> Option<crate::path::TilePos> {
        self.loaded.map.zones.iter().find_map(|z| match &z.kind {
            ZoneKind::Portal { target_map, .. } | ZoneKind::Transition { target_map, .. } if target_map == map => Some((z.x as i32 + z.w as i32 / 2, z.y as i32 + z.h as i32 / 2)),
            _ => None,
        })
    }

    /// Moves a character along its schedule. None: it has none (or it is stopped), other
    /// behaviours apply; Some(true): walking (or inside, elsewhere, on furniture), nothing else
    /// to do; Some(false): at its stop, where its definition's behaviour applies.
    pub(super) fn schedule_step(&mut self, i: usize, dt: f32) -> Option<bool> {
        let (hour, here) = (self.world.hour, self.loaded.map.name.clone());
        let e = &self.loaded.entities[i];
        let s = e.schedule.as_ref().filter(|s| s.on)?;
        let now = current_stop(&s.stops, hour)?;
        if s.current != Some(now) {
            // A new stop: get up, come out, and set off (from the door if it comes from
            // another map).
            let prev = s.current.map_or_else(|| s.home.clone(), |k| s.map_of(k).to_string());
            let target = s.map_of(now).to_string();
            let elsewhere = s.elsewhere;
            self.npc_stand_up(i);
            let entry = (elsewhere && target == here).then(|| self.exit_to(&prev).or_else(|| self.exit_to(&self.loaded.entities[i].schedule.as_ref().map_or(String::new(), |s| s.home.clone())))).flatten();
            let stop_tile = self.loaded.entities[i].schedule.as_ref().map(|s| (s.stops[now].x, s.stops[now].y)).unwrap_or_default();
            let entry_pos = entry.map(|t| self.tile_spot(t));
            let stop_pos = self.tile_spot(stop_tile);
            let e = &mut self.loaded.entities[i];
            let s = e.schedule.as_mut()?;
            s.current = Some(now);
            s.arrived = false;
            s.walker = Walker::default();
            if target == here && e.away {
                e.away = false;
                e.active = true;
                s.elsewhere = false;
                if let Some(p) = entry_pos {
                    e.body.pos = p;
                } else if elsewhere {
                    // No door from there: it just turns up at its stop.
                    e.body.pos = stop_pos;
                }
            }
        }
        let e = &self.loaded.entities[i];
        if e.away {
            return Some(true);
        }
        if e.npc_seat.is_some() {
            return Some(true);
        }
        let s = e.schedule.as_ref()?;
        if s.arrived {
            return Some(false);
        }
        let target = s.map_of(now).to_string();
        // Off to another map: out through the door leading there (or home, then on).
        let (goal, leaving) = if target != here {
            let home = s.home.clone();
            match self.exit_to(&target).or_else(|| self.exit_to(&home)) {
                Some(t) => (t, true),
                None => {
                    self.leave_map(i);
                    return Some(true);
                }
            }
        } else {
            ((s.stops[now].x, s.stops[now].y), false)
        };
        let e = &mut self.loaded.entities[i];
        let s = e.schedule.as_mut()?;
        let mut walker = std::mem::take(&mut s.walker);
        let walked = self.walk_to_tile(i, goal, &mut walker, 0.0, dt);
        if let Some(s) = &mut self.loaded.entities[i].schedule {
            s.walker = walker;
        }
        // Furniture blocks the last steps (a bed's foot): close enough, it just uses it.
        let near_prop = !leaving && self.loaded.entities[i].schedule.as_ref().is_some_and(|s| !s.stops[now].prop.is_empty()) && self.loaded.entities[i].body.pos.distance(self.tile_spot(goal)) < self.ts() * 1.5;
        match (walked, leaving) {
            (Walk::GaveUp, false) if near_prop => self.arrive(i),
            (Walk::Walking | Walk::GaveUp, _) => {}
            (Walk::Arrived, true) => self.leave_map(i),
            (Walk::Arrived, false) => self.arrive(i),
            (Walk::Unreachable, _) => {
                eprintln!("[game] {} can't reach its schedule stop {goal:?}; staying here", self.loaded.entities[i].def);
                if leaving {
                    self.leave_map(i);
                } else {
                    self.arrive(i);
                }
            }
        }
        Some(true)
    }

    /// Character `i` leaves this map (through a door to its stop's map).
    fn leave_map(&mut self, i: usize) {
        let e = &mut self.loaded.entities[i];
        e.active = false;
        e.away = true;
        e.body.moving = false;
        if let Some(s) = &mut e.schedule {
            s.elsewhere = true;
            s.arrived = true;
        }
    }

    /// The "Schedule" action: follow the daily routine again, or stop following it (the
    /// character stays where it is and does what its definition says).
    pub(super) fn set_schedule(&mut self, i: usize, on: bool) {
        self.npc_stand_up(i);
        let e = &mut self.loaded.entities[i];
        let Some(s) = &mut e.schedule else {
            eprintln!("[game] {} has no schedule", e.def);
            return;
        };
        s.on = on;
        s.current = None; // on: head for the current stop
        s.arrived = false;
        s.walker = Walker::default();
        if e.away && !s.elsewhere {
            e.away = false;
            e.active = true;
        }
        e.body.moving = false;
        self.persist(i);
    }

    /// Characters with a schedule on this map (for automated play-throughs):
    /// "name:stop[>map]@x,y" plus "(walking)", "(inside)", "(elsewhere)", "(stopped)" and
    /// ":sit" / ":lie" on furniture; stop = "HH:MM".
    pub(super) fn schedule_status(&self) -> Vec<String> {
        (0..self.loaded.entities.len())
            .filter_map(|i| {
                let e = &self.loaded.entities[i];
                let s = e.schedule.as_ref()?;
                let t = self.tile_of(e.body.pos);
                let name = if e.name.is_empty() { e.def.as_str() } else { e.name.as_str() };
                let stop = s.current.and_then(|k| s.stops.get(k)).map_or("-".to_string(), |st| if st.map.is_empty() { st.time_text() } else { format!("{}>{}", st.time_text(), st.map) });
                let how = if !s.on {
                    "(stopped)"
                } else if e.away && s.elsewhere {
                    "(elsewhere)"
                } else if e.away {
                    "(inside)"
                } else if !s.arrived {
                    "(walking)"
                } else {
                    ""
                };
                let pose = self.npc_pose(i);
                let pose = if pose.is_empty() { String::new() } else { format!(":{pose}") };
                Some(format!("{name}:{stop}{how}{pose}@{},{}", t.0, t.1))
            })
            .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_on_other_maps() {
        // Stops from before maps and furniture are on the character's own map.
        let old: ScheduleStop = ron::from_str("(hour: 20.5, x: 20, y: 7, inside: true)").unwrap();
        assert!(old.map.is_empty() && old.prop.is_empty());
        let night = ScheduleStop { map: "house".into(), prop: "guest_bed".into(), ..ScheduleStop::new(20.5, 6, 8) };
        let s = Schedule::new(vec![ScheduleStop::new(7.0, 1, 1), night.clone()], "village");
        assert_eq!((s.map_of(0), s.map_of(1), s.map_of(9)), ("village", "house", "village"));
        let back: ScheduleStop = ron::from_str(&ron::to_string(&night).unwrap()).unwrap();
        assert_eq!(back, night);
        assert!(!ron::to_string(&ScheduleStop::new(1.0, 2, 3)).unwrap().contains("map"), "defaults aren't written");
    }
}
