//! Move routes (`crate::routes`): who walks which steps, advanced every frame. A walking step
//! sets one tile's straight move (`Body::move_target`) at a time and waits for it; unless the route
//! walks *through*, the tile ahead must be free (no solid ground, prop or character), else the
//! step is skipped (`skip_blocked`) or waits. Routes end when the map changes.

use std::cell::RefCell;
use std::collections::HashMap;

use macroquad::prelude::*;

use super::occupancy::{OCC_CHAR, OCC_PROP};
use super::paperdoll::Anim;
use super::{Body, Game};
use crate::entity::Facing;
use crate::events::runner::ActionCtx;
use crate::events::Action;
use crate::grid::{feet_spot, feet_tile};
use crate::routes::RouteStep;

/// Seconds a jump takes.
const JUMP_TIME: f32 = 0.45;

thread_local! {
    /// Clip names as the drawing wants them (`Anim::Extra` takes `&'static str`): each name made
    /// once.
    static CLIPS: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
}

fn interned(clip: &str) -> &'static str {
    CLIPS.with(|c| *c.borrow_mut().entry(clip.to_string()).or_insert_with(|| Box::leak(clip.to_string().into_boxed_str())))
}

/// Who walks a route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Walker {
    Player,
    Entity(u32),
}

pub(super) struct RouteRun {
    who: Walker,
    steps: Vec<RouteStep>,
    idx: usize,
    repeat: bool,
    skip_blocked: bool,
    /// The event waits for it.
    pub(super) waiting: bool,
    through: bool,
    /// Its usual speed (given back when the route ends).
    base_speed: f32,
    /// Tiles left of the current walking step (None = the step hasn't started).
    tiles_left: Option<u32>,
    timer: f32,
    /// A jump in progress: from, to, seconds so far.
    jump: Option<(Vec2, Vec2, f32)>,
    /// An animation clip playing: name, seconds left.
    anim: Option<(&'static str, f32)>,
}

impl Game {
    fn walker(&self, target: &str, owner: Option<u32>) -> Option<Walker> {
        if target == "player" {
            return Some(Walker::Player);
        }
        self.entity_index(target, owner).map(|i| Walker::Entity(self.loaded.entities[i].uid))
    }

    fn walker_index(&self, w: Walker) -> Option<usize> {
        match w {
            Walker::Player => None,
            Walker::Entity(uid) => self.loaded.entities.iter().position(|e| e.uid == uid),
        }
    }

    fn walker_body(&mut self, w: Walker) -> Option<&mut Body> {
        match w {
            Walker::Player => Some(&mut self.player),
            Walker::Entity(uid) => self.loaded.entities.iter_mut().find(|e| e.uid == uid).map(|e| &mut e.body),
        }
    }

    /// *Move route*: starts a route (replacing the walker's current one). Returns whether the
    /// event should wait for it.
    pub(super) fn start_route(&mut self, target: &str, owner: Option<u32>, steps: &[RouteStep], repeat: bool, skip_blocked: bool, wait: bool) -> bool {
        let Some(who) = self.walker(target, owner) else {
            eprintln!("[routes] Move route: no character '{target}' here");
            return false;
        };
        self.end_route(who);
        if steps.is_empty() {
            return false;
        }
        let base_speed = self.walker_body(who).map_or(70.0, |b| b.speed);
        // (A repeating route never ends: the event doesn't wait for it.)
        let waiting = wait && !repeat;
        self.routes.push(RouteRun { who, steps: steps.to_vec(), idx: 0, repeat, skip_blocked, waiting, through: false, base_speed, tiles_left: None, timer: 0.0, jump: None, anim: None });
        waiting
    }

    /// Ends a walker's route (its speed back to usual).
    fn end_route(&mut self, who: Walker) {
        if let Some(k) = self.routes.iter().position(|r| r.who == who) {
            let r = self.routes.remove(k);
            if let Some(b) = self.walker_body(who) {
                b.speed = r.base_speed;
            }
        }
    }

    /// Whether an event waits for a route.
    pub(super) fn route_waiting(&self) -> bool {
        self.routes.iter().any(|r| r.waiting)
    }

    /// Routes end with the map.
    pub(super) fn clear_routes(&mut self) {
        let who: Vec<Walker> = self.routes.iter().map(|r| r.who).collect();
        for w in who {
            self.end_route(w);
        }
    }

    /// Whether tile `t` stops a walker (solid ground or decoration, a solid prop or character).
    fn route_blocked(&self, t: (i32, i32)) -> bool {
        self.tile_solid(t.0, t.1) || self.loaded.occupancy.has(t.0, t.1, OCC_PROP | OCC_CHAR)
    }

    /// Advances every route.
    pub(super) fn update_routes(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.routes.len() {
            if self.advance_route(k, dt) {
                k += 1;
            } else {
                let who = self.routes[k].who;
                self.end_route(who);
            }
        }
    }

    /// One frame of route `k`; false when it is over (or its walker is gone).
    fn advance_route(&mut self, k: usize, dt: f32) -> bool {
        let ts = self.ts();
        let who = self.routes[k].who;
        let Some(pos) = self.walker_body(who).map(|b| b.pos) else { return false };
        // A clip plays on while the route goes on.
        if let Some((_, left)) = &mut self.routes[k].anim {
            *left -= dt;
            if *left <= 0.0 {
                self.routes[k].anim = None;
            }
        }
        // A jump in the air.
        if let Some((from, to, t)) = self.routes[k].jump {
            let t = t + dt;
            let done = t >= JUMP_TIME;
            let p = if done { to } else { from.lerp(to, t / JUMP_TIME) };
            if let Some(b) = self.walker_body(who) {
                b.pos = p;
            }
            if done {
                self.routes[k].jump = None;
                if let Some(i) = self.walker_index(who) {
                    self.occupy(i);
                }
                self.next_step(k);
            } else {
                self.routes[k].jump = Some((from, to, t));
            }
            return true;
        }
        // Walking a tile: wait until it is there.
        if self.walker_body(who).is_some_and(|b| b.move_target.is_some()) {
            return true;
        }
        let r = &self.routes[k];
        if r.idx >= r.steps.len() {
            if r.repeat {
                self.routes[k].idx = 0;
                return true;
            }
            return false;
        }
        let step = r.steps[r.idx].clone();
        let player = self.player.pos;
        match step {
            RouteStep::Move { .. } | RouteStep::Toward { .. } | RouteStep::Away { .. } => {
                let total = match step {
                    RouteStep::Move { tiles, .. } | RouteStep::Toward { tiles } | RouteStep::Away { tiles } => tiles,
                    _ => 0,
                };
                let left = *self.routes[k].tiles_left.get_or_insert(total);
                if left == 0 {
                    self.next_step(k);
                    return true;
                }
                let here = feet_tile(pos, ts);
                let dir = match step {
                    RouteStep::Move { dir, .. } => Some(dir),
                    _ => {
                        let pt = feet_tile(player, ts);
                        let (dx, dy) = (pt.0 - here.0, pt.1 - here.1);
                        let toward = if dx.abs() >= dy.abs() && dx != 0 {
                            Some(if dx > 0 { Facing::Right } else { Facing::Left })
                        } else if dy != 0 {
                            Some(if dy > 0 { Facing::Down } else { Facing::Up })
                        } else {
                            None
                        };
                        if matches!(step, RouteStep::Away { .. }) {
                            toward.map(opposite)
                        } else {
                            toward
                        }
                    }
                };
                let Some(dir) = dir.filter(|_| who != Walker::Player || !matches!(step, RouteStep::Toward { .. } | RouteStep::Away { .. })) else {
                    // Already there (or the player walking toward itself).
                    self.next_step(k);
                    return true;
                };
                let (fx, fy) = dir.dir();
                let next = (here.0 + fx as i32, here.1 + fy as i32);
                if let Some(b) = self.walker_body(who) {
                    b.facing = dir;
                }
                if !self.routes[k].through && self.route_blocked(next) {
                    if self.routes[k].skip_blocked {
                        self.next_step(k);
                    }
                    return true;
                }
                if let Some(b) = self.walker_body(who) {
                    b.move_target = Some(feet_spot(next, ts));
                }
                self.routes[k].tiles_left = Some(left - 1);
            }
            RouteStep::Turn { dir } => {
                if let Some(b) = self.walker_body(who) {
                    b.facing = dir;
                }
                self.next_step(k);
            }
            RouteStep::TurnToPlayer => {
                let d = player - pos;
                if let (Some(f), Some(b)) = (Facing::from_vec(d.x, d.y), self.walker_body(who)) {
                    b.facing = f;
                }
                self.next_step(k);
            }
            RouteStep::TurnRandom => {
                let f = [Facing::Up, Facing::Down, Facing::Left, Facing::Right][rand::gen_range(0, 4)];
                if let Some(b) = self.walker_body(who) {
                    b.facing = f;
                }
                self.next_step(k);
            }
            RouteStep::Wait { seconds } => {
                self.routes[k].timer += dt;
                if self.routes[k].timer >= seconds {
                    self.next_step(k);
                }
            }
            RouteStep::Jump { dx, dy } => {
                let here = feet_tile(pos, ts);
                let land = (here.0 + dx, here.1 + dy);
                if (dx, dy) != (0, 0) && !self.routes[k].through && self.route_blocked(land) {
                    if self.routes[k].skip_blocked {
                        self.next_step(k);
                    }
                    return true;
                }
                if let (Some(f), Some(b)) = (Facing::from_vec(dx as f32, dy as f32), self.walker_body(who)) {
                    b.facing = f;
                }
                self.routes[k].jump = Some((pos, feet_spot(land, ts), 0.0));
            }
            RouteStep::Speed { speed } => {
                let s = self.routes[k].base_speed * speed.max(0.05);
                if let Some(b) = self.walker_body(who) {
                    b.speed = s;
                }
                self.next_step(k);
            }
            RouteStep::Anim { clip, seconds } => {
                self.routes[k].anim = Some((interned(&clip), seconds.max(0.05)));
                self.routes[k].timer += dt;
                if self.routes[k].timer >= seconds {
                    self.routes[k].anim = None;
                    self.next_step(k);
                }
            }
            RouteStep::Sound { sound } => {
                self.sound_from(&sound, pos);
                self.next_step(k);
            }
            RouteStep::Through { on } => {
                self.routes[k].through = on;
                self.next_step(k);
            }
            RouteStep::Script { function } => {
                let owner = match who {
                    Walker::Entity(uid) => Some(uid),
                    Walker::Player => None,
                };
                self.runner.push(&[Action::RunScript { function }], &ActionCtx { owner, ..Default::default() });
                self.next_step(k);
            }
        }
        true
    }

    fn next_step(&mut self, k: usize) {
        let r = &mut self.routes[k];
        r.idx += 1;
        r.tiles_left = None;
        r.timer = 0.0;
    }

    /// The clip a route plays on a walker (overrides its animation).
    pub(super) fn route_anim(&self, who: Walker) -> Option<Anim> {
        self.routes.iter().find(|r| r.who == who).and_then(|r| r.anim).map(|(clip, _)| Anim::Extra(clip, 0))
    }

    /// How high a jumping walker is drawn above its feet (pixels; 0 = on the ground).
    pub(super) fn route_hop(&self, who: Walker) -> f32 {
        let Some((from, to, t)) = self.routes.iter().find(|r| r.who == who).and_then(|r| r.jump) else { return 0.0 };
        let k = (t / JUMP_TIME).clamp(0.0, 1.0);
        let height = 0.4 * self.ts() + from.distance(to) * 0.25;
        (std::f32::consts::PI * k).sin() * height
    }

    pub(super) fn entity_walker(&self, i: usize) -> Walker {
        Walker::Entity(self.loaded.entities[i].uid)
    }

    /// For `status()`: `who:step/steps[:summary]` (`player` or the character's name / uid).
    pub(super) fn routes_status(&self) -> Vec<String> {
        self.routes
            .iter()
            .map(|r| {
                let who = match r.who {
                    Walker::Player => "player".to_string(),
                    Walker::Entity(uid) => self.loaded.entities.iter().find(|e| e.uid == uid).map_or(uid.to_string(), |e| if e.name.is_empty() { uid.to_string() } else { e.name.clone() }),
                };
                let now = r.steps.get(r.idx).map_or("end".to_string(), |s| s.summary());
                format!("{who}:{}/{}:{now}{}", (r.idx + 1).min(r.steps.len()), r.steps.len(), if r.repeat { "+repeat" } else { "" })
            })
            .collect()
    }
}

fn opposite(f: Facing) -> Facing {
    match f {
        Facing::Up => Facing::Down,
        Facing::Down => Facing::Up,
        Facing::Left => Facing::Right,
        Facing::Right => Facing::Left,
    }
}
