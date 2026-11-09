//! Enemy behaviour for `Behavior::Chase` characters: they chase the player they notice (within
//! their sight range in a clear line, or within half of it through walls and trees),
//! remember them for a few seconds after losing sight (walking to where they saw them last),
//! give up beyond their leash and walk home, flee when badly hurt, and shooters keep their
//! distance. The state lives in [`Ai`] on each character (not saved).

use macroquad::prelude::*;

use super::Game;
use crate::defs::Behavior;
use crate::entity::Facing;
use crate::path::{find_path, TilePos};

/// Seconds a character keeps hunting after it lost sight of the player.
const AGGRO_MEMORY: f32 = 4.0;
/// A character walking home gives up after being stuck this long (and stays where it is).
const HOME_STUCK_GIVE_UP: f32 = 4.0;
/// A* search limit for the way home.
const MAX_NODES: usize = 4000;

/// What a chasing character is doing.
#[derive(Clone, Debug, Default)]
pub(super) struct Ai {
    /// Seconds left of hunting without seeing the player.
    pub aggro: f32,
    /// Where it saw the player last.
    pub last_seen: Vec2,
    /// Walking back home after going too far (ignores the player meanwhile).
    pub returning: bool,
    /// Running away (badly hurt).
    pub fleeing: bool,
    /// The way home, tile by tile.
    path: Vec<TilePos>,
    stuck: f32,
}

impl Ai {
    /// It neither shoots nor hurts by touch.
    pub fn harmless(&self) -> bool {
        self.returning || self.fleeing
    }
}

impl Game {
    /// Moves a `Chase` character; returns false when it isn't hunting (it wanders / patrols).
    pub(super) fn chase(&mut self, i: usize, dt: f32) -> bool {
        let ts = self.ts();
        let e = &self.loaded.entities[i];
        let Behavior::Chase { sight, leash, flee_below, keep_distance } = e.behavior else { return false };
        if self.game_over.is_some() {
            self.loaded.entities[i].ai = Ai::default();
            return false;
        }
        let pos = e.body.pos;
        // Gave up: walk home first.
        if e.ai.returning {
            if self.walk_home(i, dt) {
                return true;
            }
            self.loaded.entities[i].ai = Ai::default();
            return false;
        }
        if leash > 0.0 && pos.distance(e.home) > leash * ts {
            self.loaded.entities[i].ai = Ai { returning: true, ..Default::default() };
            return self.walk_home(i, dt);
        }

        let eye = pos - vec2(0.0, e.body.hitbox.y / 2.0 + 2.0);
        // Noticed within sight; behind walls and trees only when closer than half of it. The
        // player and companions that can be hurt: it goes for the nearest it notices.
        let notices = |feet: Vec2, mid: Vec2| {
            let d = feet.distance(pos);
            d <= sight * ts && (d <= sight * ts * 0.5 || self.line_clear(eye, mid))
        };
        let mut seen: Vec<Vec2> = Vec::new();
        if notices(self.player.pos, self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0)) {
            seen.push(self.player.pos);
        }
        for (c, mid) in self.companion_targets() {
            let feet = self.loaded.entities[c].body.pos;
            if notices(feet, mid) {
                seen.push(feet);
            }
        }
        let nearest = seen.into_iter().min_by(|a, b| a.distance(pos).total_cmp(&b.distance(pos)));
        let sees = nearest.is_some();
        let target = nearest.unwrap_or(self.player.pos);
        let to = target - pos;
        let dist = to.length();
        let hurt = e.max_hp > 0 && flee_below > 0.0 && (e.hp as f32) < flee_below * e.max_hp as f32;
        let ranged = e.weapon.as_ref().and_then(|w| w.ranged.as_ref()).map(|r| r.range * ts);
        let e = &mut self.loaded.entities[i];
        if sees {
            e.ai.aggro = AGGRO_MEMORY;
            e.ai.last_seen = target;
        } else {
            e.ai.aggro -= dt;
        }
        if e.ai.aggro <= 0.0 {
            e.ai.fleeing = false;
            return false;
        }
        // Badly hurt: run away from the player (seen or remembered).
        e.ai.fleeing = hurt;
        if hurt {
            let from = if sees { target } else { e.ai.last_seen };
            let away = (pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
            self.step_towards_dir(i, away, dt, None);
            return true;
        }
        if !sees {
            // Hunt: walk to where the player was seen last; then give up.
            let goal = e.ai.last_seen;
            if pos.distance(goal) < ts * 0.3 {
                e.ai.aggro = 0.0;
                e.body.moving = false;
                return false;
            }
            let dir = (goal - pos).normalize_or_zero();
            self.step_towards_dir(i, dir, dt, None);
            return true;
        }
        let dir = to.try_normalize().unwrap_or(vec2(0.0, 1.0));
        // Shooters: hold position within range; with a kept distance, back off when too close.
        if let Some(range) = ranged {
            if keep_distance > 0.0 {
                let keep = keep_distance * ts;
                if dist < keep * 0.9 {
                    self.step_towards_dir(i, -dir, dt, Some(dir));
                    return true;
                }
                if dist <= range * 0.9 {
                    Self::hold(&mut self.loaded.entities[i].body, dir);
                    return true;
                }
            } else if dist < range * 0.6 {
                Self::hold(&mut self.loaded.entities[i].body, dir);
                return true;
            }
        }
        if dist < 1.0 {
            Self::hold(&mut self.loaded.entities[i].body, dir);
            return true;
        }
        self.step_towards_dir(i, dir, dt, None);
        true
    }

    /// Stands still facing `dir`.
    fn hold(body: &mut super::Body, dir: Vec2) {
        body.facing = Facing::from_vec(dir.x, dir.y).unwrap_or(body.facing);
        body.moving = false;
    }

    /// Moves character `i` along `dir` at its speed; when an obstacle stops most of that step it
    /// slides along it (the direction 45 or 90 degrees off that gets farthest along `dir`).
    /// `face`: look that way instead of where it walks (backing off while aiming).
    pub(super) fn step_towards_dir(&mut self, i: usize, dir: Vec2, dt: f32, face: Option<Vec2>) {
        let e = &self.loaded.entities[i];
        let (body, uid) = (e.body.clone(), e.uid);
        let full = body.speed * dt;
        let mut new_pos = self.try_move(&body, dir * full, Some(uid), false);
        let mut moved_dir = dir;
        if new_pos.distance(body.pos) < full * 0.5 {
            // Progress = how far a step gets along `dir` (sliding the wrong way scores below 0).
            let mut best = (new_pos - body.pos).dot(dir);
            for angle in [45.0f32, -45.0, 90.0, -90.0] {
                let d = Vec2::from_angle(angle.to_radians()).rotate(dir);
                let p = self.try_move(&body, d * full, Some(uid), false);
                let progress = (p - body.pos).dot(dir);
                if progress > best + 0.001 {
                    best = progress;
                    new_pos = p;
                    moved_dir = d;
                }
            }
        }
        let e = &mut self.loaded.entities[i];
        let look = face.unwrap_or(moved_dir);
        e.body.facing = Facing::from_vec(look.x, look.y).unwrap_or(e.body.facing);
        e.body.moving = new_pos != e.body.pos;
        e.body.pos = new_pos;
        e.body.anim += dt;
    }

    /// Walks character `i` home along an A* path; returns false once it is there (or can't get
    /// there).
    fn walk_home(&mut self, i: usize, dt: f32) -> bool {
        let e = &self.loaded.entities[i];
        let (pos, home) = (e.body.pos, e.home);
        if pos.distance(home) < 3.0 {
            self.loaded.entities[i].body.moving = false;
            return false;
        }
        if e.ai.path.is_empty() {
            let (here, goal) = (self.tile_of(pos), self.tile_of(home));
            let blocked = |x: i32, y: i32| !self.loaded.map.in_bounds(x, y) || self.tile_blocks_npc(x, y);
            let path = find_path(here, goal, |t| t == goal, blocked, MAX_NODES);
            let e = &mut self.loaded.entities[i];
            match path {
                Some(p) if !p.is_empty() => e.ai.path = p,
                // Same tile, or no way: the last step goes straight to the spot.
                _ => e.ai.path = vec![goal],
            }
        }
        let e = &self.loaded.entities[i];
        let next = *e.ai.path.first().unwrap();
        let spot = if e.ai.path.len() == 1 { home } else { self.tile_spot(next) };
        let step = (spot - pos).clamp_length_max(e.body.speed * dt);
        let (body, uid) = (e.body.clone(), e.uid);
        let new_pos = self.try_move(&body, step, Some(uid), false);
        let e = &mut self.loaded.entities[i];
        e.body.facing = Facing::from_vec(step.x, step.y).unwrap_or(e.body.facing);
        e.body.moving = new_pos != pos;
        e.body.pos = new_pos;
        e.body.anim += dt;
        if new_pos.distance(spot) < 1.0 {
            e.ai.path.remove(0);
            e.ai.stuck = 0.0;
        } else if new_pos.distance(pos) < 0.01 {
            e.ai.stuck += dt;
            if e.ai.stuck > HOME_STUCK_GIVE_UP {
                // Can't get home: this is home now.
                e.home = new_pos;
                return false;
            }
            if e.ai.stuck > 1.0 {
                e.ai.path.clear();
            }
        } else {
            e.ai.stuck = 0.0;
        }
        true
    }

    /// A character's HP dropped from `before` (it is still alive): fires its *HP below %*
    /// triggers whose threshold was crossed.
    pub(super) fn hp_dropped(&mut self, i: usize, before: i32) {
        let e = &self.loaded.entities[i];
        if e.max_hp <= 0 || e.hp <= 0 || e.hp >= before {
            return;
        }
        let (from, to) = (before as f32 / e.max_hp as f32, e.hp as f32 / e.max_hp as f32);
        let uid = e.uid;
        self.fire(crate::events::GameEvent::HpBelow { from, to }, super::Source::Entity(uid));
    }

    /// For `status()`: what the hunting characters are doing.
    pub(super) fn ai_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter(|e| e.active && (e.ai.aggro > 0.0 || e.ai.returning || e.ai.fleeing))
            .map(|e| {
                let what = if e.ai.returning {
                    "returning"
                } else if e.ai.fleeing {
                    "fleeing"
                } else {
                    "hunting"
                };
                let ts = self.ts();
                format!("{}:{what}@{},{}", e.def, (e.body.pos.x / ts).floor(), (e.body.pos.y / ts).floor())
            })
            .collect()
    }
}
