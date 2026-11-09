//! Patrolling characters: walk from waypoint to waypoint (A* on the tile grid around solid
//! tiles), pause at each, loop or go back and forth. Talking to them, dialogs and scripted
//! moves pause the patrol; its progress is saved with the character's state.

use macroquad::prelude::*;

use super::Game;
use crate::entity::{EntityKind, Facing, PatrolRoute};
use crate::path::{find_path, TilePos};
use crate::state::PatrolState;

/// A* budget per waypoint (routes are short; this keeps big maps cheap).
const MAX_NODES: usize = 6_000;
/// Seconds without progress before the path is planned again / the waypoint is skipped.
const REPLAN_AFTER: f32 = 1.0;
const SKIP_AFTER: f32 = 4.0;

/// Runtime patrol state of a character.
#[derive(Clone, Debug, Default)]
pub(super) struct Patrol {
    pub route: PatrolRoute,
    /// The route came from an action / script (saved with the character).
    pub custom: bool,
    pub on: bool,
    pub index: usize,
    pub back: bool,
    pub wait: f32,
    walker: Walker,
}

/// A character on its way to a tile: the remaining path and how long it has been blocked.
#[derive(Clone, Debug, Default)]
pub(super) struct Walker {
    /// Remaining tiles to the goal (next first).
    pub path: Vec<TilePos>,
    stuck: f32,
    /// It was blocked on its way: plan around the tiles other characters stand on (two
    /// characters waiting for each other in a doorway find another way).
    avoid: bool,
}

/// What a step of [`Game::walk_to_tile`] came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Walk {
    Walking,
    Arrived,
    /// No path to the goal.
    Unreachable,
    /// Blocked for a few seconds.
    GaveUp,
}

impl Patrol {
    pub fn new(route: PatrolRoute) -> Self {
        Patrol { route, on: true, ..Default::default() }
    }

    /// Applies saved progress.
    pub fn restore(&mut self, st: &PatrolState) {
        if let Some(r) = &st.route {
            self.route = r.clone();
            self.custom = true;
        }
        self.index = (st.index as usize).min(self.route.points.len().saturating_sub(1));
        self.back = st.back;
        self.on = !st.stopped;
    }

    pub fn state(&self) -> PatrolState {
        PatrolState { index: self.index as u32, back: self.back, stopped: !self.on, route: self.custom.then(|| self.route.clone()) }
    }

    /// Starts over with another route.
    pub fn set_route(&mut self, route: PatrolRoute, custom: bool) {
        *self = Patrol { route, custom, on: true, ..Default::default() };
    }

    fn advance(&mut self) {
        let (i, back) = self.route.next(self.index, self.back);
        self.index = i;
        self.back = back;
        self.walker = Walker::default();
    }
}

impl Game {
    /// Feet position a character aims for on a tile.
    pub(super) fn tile_spot(&self, t: TilePos) -> Vec2 {
        let ts = self.ts();
        vec2((t.0 as f32 + 0.5) * ts, (t.1 as f32 + 1.0) * ts - 2.0)
    }

    pub(super) fn tile_of(&self, p: Vec2) -> TilePos {
        let ts = self.ts();
        ((p.x / ts).floor() as i32, ((p.y - 1.0) / ts).floor() as i32)
    }

    /// Moves a patrolling character. Returns false if it doesn't patrol (other behaviours apply).
    pub(super) fn patrol_step(&mut self, i: usize, dt: f32) -> bool {
        let Some(p) = self.loaded.entities[i].patrol.as_ref() else { return false };
        if p.route.points.is_empty() {
            return false;
        }
        if !p.on {
            self.loaded.entities[i].body.moving = false;
            return true; // stopped: stands still
        }
        if p.wait > 0.0 {
            let e = &mut self.loaded.entities[i];
            e.body.moving = false;
            if let Some(p) = &mut e.patrol {
                p.wait -= dt;
            }
            return true;
        }
        let goal = p.route.points[p.index.min(p.route.points.len() - 1)];
        let speed = p.route.speed;
        let mut walker = std::mem::take(&mut self.loaded.entities[i].patrol.as_mut().unwrap().walker);
        let walked = self.walk_to_tile(i, goal, &mut walker, speed, dt);
        let e = &mut self.loaded.entities[i];
        let p = e.patrol.as_mut().unwrap();
        p.walker = walker;
        match walked {
            Walk::Walking => {}
            Walk::Arrived => {
                // Pause, then head for the next waypoint.
                p.wait = p.route.pause.max(0.0);
                p.advance();
            }
            Walk::Unreachable => {
                eprintln!("[game] {} can't reach patrol point {goal:?}; skipping it", e.def);
                p.wait = 1.0;
                p.advance();
            }
            // Blocked for long (by the player, another character): on to the next waypoint.
            Walk::GaveUp => p.advance(),
        }
        true
    }

    /// One step of character `i` towards tile `goal`, along the path in `w` (planned with A*
    /// around solid tiles when empty). `speed` 0 = the character's own.
    pub(super) fn walk_to_tile(&mut self, i: usize, goal: TilePos, w: &mut Walker, speed: f32, dt: f32) -> Walk {
        let pos = self.loaded.entities[i].body.pos;
        let here = self.tile_of(pos);
        // Plan the way to the goal when needed.
        if w.path.is_empty() {
            // Around solid tiles and props (statues, wells, closed gates); characters move, so
            // they are waited for instead.
            let props = self.solid_entity_tiles(|e| e.kind == EntityKind::Prop);
            let me = self.loaded.entities[i].uid;
            let people = if w.avoid { self.solid_entity_tiles(|e| e.kind == EntityKind::Character && e.uid != me) } else { Default::default() };
            let blocked = |x: i32, y: i32| !self.loaded.map.in_bounds(x, y) || self.tile_blocks_npc(x, y) || ((x, y) != goal && (x, y) != here && (props.contains(&(x, y)) || people.contains(&(x, y))));
            let planned = find_path(here, goal, |t| t == goal, blocked, MAX_NODES).or_else(|| {
                // Hemmed in by people: plan as if they weren't there (they move on).
                let blocked = |x: i32, y: i32| !self.loaded.map.in_bounds(x, y) || self.tile_blocks_npc(x, y) || ((x, y) != goal && props.contains(&(x, y)));
                find_path(here, goal, |t| t == goal, blocked, MAX_NODES)
            });
            let at_goal = pos.distance(self.tile_spot(goal)) < 2.0;
            match planned {
                Some(path) if path.is_empty() && at_goal => {
                    self.loaded.entities[i].body.moving = false;
                    *w = Walker::default();
                    return Walk::Arrived;
                }
                Some(path) if path.is_empty() => w.path = vec![goal],
                Some(path) => w.path = path,
                None => {
                    self.loaded.entities[i].body.moving = false;
                    *w = Walker::default();
                    return Walk::Unreachable;
                }
            }
        }
        let e = &self.loaded.entities[i];
        let target = self.tile_spot(w.path[0]);
        let speed = if speed > 0.0 { speed } else { e.body.speed.max(30.0) };
        let to = target - pos;
        let step = to.clamp_length_max(speed * dt);
        let (body, uid) = (e.body.clone(), e.uid);
        let new_pos = if step.length() > 0.0 { self.try_move(&body, step, Some(uid), true) } else { pos };
        let e = &mut self.loaded.entities[i];
        e.body.facing = Facing::from_vec(step.x, step.y).unwrap_or(e.body.facing);
        e.body.pos = new_pos;
        e.body.moving = new_pos != pos;
        e.body.anim += dt;
        e.home = new_pos;
        if new_pos.distance(target) < 1.0 {
            w.path.remove(0);
            w.stuck = 0.0;
        } else if new_pos.distance(pos) < 0.01 {
            // Blocked (by the player, another character, a corner): wait, re-plan, give up.
            w.stuck += dt;
            if w.stuck > SKIP_AFTER {
                *w = Walker { avoid: true, ..Walker::default() };
                return Walk::GaveUp;
            } else if w.stuck > REPLAN_AFTER && w.path.len() > 1 {
                w.path.clear();
                w.avoid = true;
            }
        } else {
            w.stuck = 0.0;
        }
        Walk::Walking
    }

    /// Saves the progress of every patrolling character on the current map (before leaving
    /// it and before saving the game).
    pub(super) fn persist_patrols(&mut self) {
        if self.loaded.map.name.is_empty() {
            return;
        }
        // Companions' HP goes into the world state too (saved, carried to the next map).
        for i in 0..self.loaded.entities.len() {
            self.sync_companion_hp(i);
        }
        for i in 0..self.loaded.entities.len() {
            if self.loaded.entities[i].patrol.is_some() && self.loaded.entities[i].spawned_by.is_none() {
                self.persist(i);
            }
        }
    }

    /// The "Patrol" action: start (the character's own route or `points`) or stop.
    pub(super) fn set_patrol(&mut self, i: usize, active: bool, points: &[(i32, i32)]) {
        let own = self.loaded.map.entity(self.loaded.entities[i].uid).and_then(|e| e.patrol.clone());
        let e = &mut self.loaded.entities[i];
        if !active {
            if let Some(p) = &mut e.patrol {
                p.on = false;
                p.walker = Walker::default();
            }
            e.body.moving = false;
        } else if !points.is_empty() {
            // A route from the action / script; mode, pause and speed as the own route.
            let base = own.or_else(|| e.patrol.as_ref().map(|p| p.route.clone())).unwrap_or_default();
            let route = PatrolRoute { points: points.to_vec(), ..base };
            match &mut e.patrol {
                Some(p) => p.set_route(route, true),
                None => e.patrol = Some(Patrol { custom: true, ..Patrol::new(route) }),
            }
        } else {
            match (&mut e.patrol, own) {
                (Some(p), _) => p.on = true,
                (None, Some(r)) => e.patrol = Some(Patrol::new(r)),
                (None, None) => eprintln!("[game] {} has no patrol route", e.def),
            }
        }
        self.persist(i);
    }

    /// Patrolling characters on this map (for automated play-throughs): "name:index@x,y".
    pub(super) fn patrol_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter_map(|e| {
                let p = e.patrol.as_ref()?;
                let t = self.tile_of(e.body.pos);
                let name = if e.name.is_empty() { e.def.as_str() } else { e.name.as_str() };
                Some(format!("{name}:{}{}@{},{}", p.index, if p.on { "" } else { "(stopped)" }, t.0, t.1))
            })
            .collect()
    }
}
