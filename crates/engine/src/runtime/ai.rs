//! Enemy behaviour for `Behavior::Chase` characters outside fights: they notice the player and
//! companions (within their sight range in a clear line, or within half of it through walls and
//! trees; sneaking and darkness shorten it) and a fight starts (`turns/`). Characters going for a
//! character of a hostile faction close in on it; they remember a foe for a few seconds after
//! losing sight (walking to where they saw it last), give up beyond their leash and walk home,
//! spread out instead of stacking, alert others of their kind (`alert` tiles), and the badly
//! hurt run to an ally, catch their breath and come back. The state lives in [`Ai`] on each
//! character (not saved).

use macroquad::prelude::*;

use super::patrol::Walk;
use super::{Game, Mover};
use crate::defs::Behavior;
use crate::entity::Facing;

/// Seconds a character keeps hunting after it lost sight of the player.
const AGGRO_MEMORY: f32 = 4.0;
/// Chasers closer than this (tiles) push each other apart.
const SPREAD: f32 = 1.1;
/// The badly hurt run to allies within this many tiles (not those fighting right at the
/// threat) and come back after this many seconds next to one.
const RALLY_RANGE: f32 = 8.0;
const REGROUP: f32 = 3.0;
/// How long a character sent by *Rush to* follows where it was sent (seconds).
const RUSH_TIME: f32 = 15.0;

/// Whom a chaser is after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Foe {
    Player,
    /// A companion (its uid).
    Companion(u32),
    /// A character of a hostile faction (its uid).
    Npc(u32),
}

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
    /// Whom it sees and goes for right now (None while it searches).
    pub target: Option<Foe>,
    /// Which way a shooter steps to find a clear line (1 or -1; 0 = not chosen yet).
    orbit: f32,
    /// Seconds next to an ally while badly hurt; `rallied` = back in the fight (until it calms down).
    regroup: f32,
    rallied: bool,
    /// It has seen the target itself (not only been alerted): it raised the alarm then.
    spotted: bool,
    /// Sent to someone (`Action::RushTo`): whom it runs to (None = the player, else a uid) and the
    /// seconds left; over once it sees a foe, gets there or the time is up.
    pub rush: Option<(Option<u32>, f32)>,
    /// The way home (`walk_home`), shared with patrols and schedules.
    walker: super::patrol::Walker,
}

impl Ai {
    /// It neither shoots nor hurts by touch.
    pub fn harmless(&self) -> bool {
        self.returning || self.fleeing
    }
}

/// The `Chase` settings of a behaviour.
struct Chase {
    sight: f32,
    leash: f32,
    flee_below: f32,
    keep_distance: f32,
    group: bool,
    alert: f32,
}

fn chase_of(b: &Behavior) -> Option<Chase> {
    match *b {
        Behavior::Chase { sight, leash, flee_below, keep_distance, group, alert } => Some(Chase { sight, leash, flee_below, keep_distance, group, alert }),
        _ => None,
    }
}

impl Game {
    /// Whether hostile character `i` knows of the party: it is hunting someone, or it notices
    /// the hero or a companion now (within its sight; behind walls and trees only when closer
    /// than half of it; the hero later while sneaking or in the dark). Only those take part when
    /// a fight starts near them.
    pub(super) fn aware_of_party(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        if e.ai.aggro > 0.0 {
            return true;
        }
        let ts = self.ts();
        let sight = chase_of(&e.behavior).map_or(8.0, |c| c.sight);
        let pos = e.body.pos;
        let eye = pos - vec2(0.0, e.body.hitbox.y / 2.0 + 2.0);
        let notices = |feet: Vec2, mid: Vec2, range: f32| {
            let d = feet.distance(pos);
            d <= range && (d <= range * 0.5 || self.line_clear(eye, mid))
        };
        let player_mid = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        // (Blindsight, tremorsense, truesight: noticed within their range, dark or sneaking.)
        let near = self.player.pos.distance(pos) <= super::stealth::NEAR_NOTICE.min(sight).max(self.sense_tiles(i).min(sight)) * ts;
        near || notices(self.player.pos, player_mid, self.player_sight(sight)) || self.companion_targets().into_iter().any(|(c, mid)| notices(self.loaded.entities[c].body.pos, mid, sight * ts))
    }

    /// Characters `i` and `j` are on the same side (the same kind or the same faction).
    fn kin(&self, i: usize, j: usize) -> bool {
        let (a, b) = (&self.loaded.entities[i], &self.loaded.entities[j]);
        a.def == b.def || (a.faction.is_some() && a.faction == b.faction) || (a.faction.is_none() && b.faction.is_none() && a.hostile && b.hostile)
    }

    /// Living characters of factions hostile to `i`'s (not companions: they are on the player's
    /// side), with their feet and the middle of their body.
    pub(super) fn npc_foes(&self, i: usize) -> Vec<(usize, Vec2, Vec2)> {
        if self.loaded.entities[i].faction.is_none() {
            return Vec::new();
        }
        (0..self.loaded.entities.len())
            .filter(|&j| {
                let o = &self.loaded.entities[j];
                j != i && o.active && o.kind == crate::entity::EntityKind::Character && o.companion.is_none() && o.max_hp > 0 && o.hp > 0 && self.foes(i, j)
            })
            .map(|j| {
                let o = &self.loaded.entities[j];
                (j, o.body.pos, o.body.pos - vec2(0.0, o.body.hitbox.y / 2.0))
            })
            .collect()
    }

    /// The push away from other chasers standing too close to character `i` (0 when alone).
    fn spread(&self, i: usize, pos: Vec2) -> Vec2 {
        let r = SPREAD * self.ts();
        let mut push = Vec2::ZERO;
        for (j, o) in self.loaded.entities.iter().enumerate() {
            if j == i || !o.active || o.ai.aggro <= 0.0 || chase_of(&o.behavior).is_none() {
                continue;
            }
            let d = pos - o.body.pos;
            let len = d.length();
            if len < r {
                // Exactly on top of each other: the lower index steps aside.
                let away = d.try_normalize().unwrap_or(if i < j { vec2(1.0, 0.0) } else { vec2(-1.0, 0.0) });
                push += away * (1.0 - len / r);
            }
        }
        push
    }

    /// Hostile character `i` was hurt by the player's side from `from` (the attacker's feet): out
    /// of sight or not, it hunts there (long enough to walk the way), surprised and alerting its
    /// kind like on sighting the player. Its leash still sends it home.
    pub(super) fn notice_attack(&mut self, i: usize, from: Vec2) {
        let e = &self.loaded.entities[i];
        let Some(Chase { alert, .. }) = chase_of(&e.behavior) else { return };
        if !e.hostile || e.companion.is_some() || self.game_over.is_some() {
            return;
        }
        let (was_hunting, spotted) = (e.ai.aggro > 0.0, e.ai.spotted);
        let walk = from.distance(e.body.pos) / e.body.speed.max(1.0);
        let e = &mut self.loaded.entities[i];
        e.ai.returning = false;
        e.ai.aggro = e.ai.aggro.max(AGGRO_MEMORY).max(walk * 1.5 + 1.0);
        e.ai.last_seen = from;
        e.ai.spotted = true;
        if !was_hunting {
            self.auto_emote(i, "surprise");
        }
        if !spotted && alert > 0.0 {
            self.alert_kind(i, from, alert);
        }
    }

    /// `Action::RushTo`: fighting character `i` runs to `to` (the player or a character) and
    /// follows it for [`RUSH_TIME`], fighting whoever it sees on the way (`chase`).
    pub(super) fn rush_to(&mut self, i: usize, to: super::emotions::Who) {
        if chase_of(&self.loaded.entities[i].behavior).is_none() {
            eprintln!("[events] Rush to: {} doesn't fight (no Chase behaviour)", self.loaded.entities[i].name);
            return;
        }
        let (uid, goal) = match to {
            super::emotions::Who::Player => (None, self.player.pos),
            super::emotions::Who::Entity(j) if j == i => return,
            super::emotions::Who::Entity(j) => (Some(self.loaded.entities[j].uid), self.loaded.entities[j].body.pos),
        };
        let e = &mut self.loaded.entities[i];
        e.ai.returning = false;
        e.ai.rush = Some((uid, RUSH_TIME));
        e.ai.last_seen = goal;
        e.ai.aggro = e.ai.aggro.max(0.5);
        self.auto_emote(i, "anger");
    }

    /// Character `i` noticed `target`: others of its kind within `tiles` come too.
    fn alert_kind(&mut self, i: usize, target: Vec2, tiles: f32) {
        let ts = self.ts();
        let pos = self.loaded.entities[i].body.pos;
        let near: Vec<usize> = (0..self.loaded.entities.len())
            .filter(|&j| {
                let o = &self.loaded.entities[j];
                j != i && o.active && self.kin(i, j) && o.companion.is_none() && o.ai.aggro <= 0.0 && !o.ai.returning && chase_of(&o.behavior).is_some() && o.body.pos.distance(pos) <= tiles * ts
            })
            .collect();
        for j in near {
            let o = &mut self.loaded.entities[j];
            o.ai.aggro = AGGRO_MEMORY;
            o.ai.last_seen = target;
            self.auto_emote(j, "surprise");
        }
    }

    /// The ally a badly hurt character `i` runs to: the nearest other hostile chaser within
    /// [`RALLY_RANGE`] that isn't fleeing itself nor fighting right at the threat.
    fn rally_point(&self, i: usize, pos: Vec2, threat: Vec2) -> Option<usize> {
        let ts = self.ts();
        let ents = &self.loaded.entities;
        (0..ents.len())
            .filter(|&j| {
                let o = &ents[j];
                j != i && o.active && self.kin(i, j) && o.companion.is_none() && !o.ai.harmless() && chase_of(&o.behavior).is_some() && o.hp > 0 && o.body.pos.distance(pos) <= RALLY_RANGE * ts && o.body.pos.distance(threat) > 3.0 * ts
            })
            .min_by(|&a, &b| ents[a].body.pos.distance(pos).total_cmp(&ents[b].body.pos.distance(pos)))
    }

    /// Moves a `Chase` character; returns false when it isn't hunting (it wanders / patrols).
    pub(super) fn chase(&mut self, i: usize, dt: f32) -> bool {
        let ts = self.ts();
        let e = &self.loaded.entities[i];
        let Some(Chase { sight, leash, flee_below, keep_distance, group, alert }) = chase_of(&e.behavior) else { return false };
        // Its conditions: blinded it sees a tile, charmed it leaves the player alone, frightened
        // it runs from them.
        let rules = (!e.effects.is_empty()).then(|| crate::srd::conditions::rules(&e.effects));
        let sight = if rules.as_ref().is_some_and(|r| r.blinded) { sight.min(1.0) } else { sight };
        let (charmed, frightened) = rules.as_ref().map_or((false, false), |r| (r.charmed, r.frightened));
        if self.game_over.is_some() {
            self.loaded.entities[i].ai = Ai::default();
            return false;
        }
        let e = &self.loaded.entities[i];
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
        let notices_within = |feet: Vec2, mid: Vec2, range: f32| {
            let d = feet.distance(pos);
            d <= range && (d <= range * 0.5 || self.line_clear(eye, mid))
        };
        let notices = |feet: Vec2, mid: Vec2| notices_within(feet, mid, sight * ts);
        let mut seen: Vec<(Foe, Vec2, Vec2)> = Vec::new();
        let player_mid = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        // The player and companions only when it is hostile to the player. The player is
        // noticed later while sneaking and in the dark (`stealth.rs`).
        if e.hostile && !charmed {
            // (Blindsight, tremorsense, truesight: noticed within their range, dark or sneaking.)
            let near = self.player.pos.distance(pos) <= super::stealth::NEAR_NOTICE.min(sight).max(self.sense_tiles(i).min(sight)) * ts;
            if near || notices_within(self.player.pos, player_mid, self.player_sight(sight)) {
                seen.push((Foe::Player, self.player.pos, player_mid));
            }
            for (c, mid) in self.companion_targets() {
                let feet = self.loaded.entities[c].body.pos;
                if notices(feet, mid) {
                    seen.push((Foe::Companion(self.loaded.entities[c].uid), feet, mid));
                }
            }
        }
        // Characters of hostile factions.
        for (j, feet, mid) in self.npc_foes(i) {
            if notices(feet, mid) {
                seen.push((Foe::Npc(self.loaded.entities[j].uid), feet, mid));
            }
        }
        let nearest = seen.into_iter().min_by(|a, b| a.1.distance(pos).total_cmp(&b.1.distance(pos)));
        let sees = nearest.is_some();
        let (foe, target, target_mid) = nearest.map_or((None, self.player.pos, player_mid), |(f, t, m)| (Some(f), t, m));
        let to = target - pos;
        let dist = to.length();
        let hurt = frightened || (e.max_hp > 0 && flee_below > 0.0 && (e.hp as f32) < flee_below * e.max_hp as f32);
        let ranged = e.weapon.as_ref().and_then(|w| w.ranged.as_ref()).map(|r| r.range * ts);
        // Sent somewhere (*Rush to*): where that one is now.
        let rush = self.loaded.entities[i].ai.rush.map(|(to, left)| {
            let goal = match to {
                None => Some(self.player.pos),
                Some(uid) => self.loaded.entities.iter().find(|o| o.uid == uid && o.active).map(|o| o.body.pos),
            };
            (to, left, goal)
        });
        let e = &mut self.loaded.entities[i];
        let was_hunting = e.ai.aggro > 0.0;
        let was_fleeing = e.ai.fleeing;
        e.ai.target = foe;
        if sees {
            e.ai.aggro = AGGRO_MEMORY;
            e.ai.last_seen = target;
        } else {
            e.ai.aggro -= dt;
        }
        e.ai.rush = None;
        if let (false, Some((to, left, Some(goal)))) = (sees, rush) {
            // Running there until it sees a foe, arrives (next to a character) or gives up.
            let arrived = to.is_some() && pos.distance(goal) < 1.5 * ts;
            if left > dt && !arrived {
                e.ai.rush = Some((to, left - dt));
                e.ai.last_seen = goal;
                e.ai.aggro = e.ai.aggro.max(0.5);
            }
        }
        let hurt = hurt && !(group && e.ai.rallied);
        // Emotions (automatic): noticing someone, losing them, running away.
        // The first time it sees the target itself (alerted ones too), it alerts its kind.
        let ai = &mut self.loaded.entities[i].ai;
        let spotted = ai.spotted;
        ai.spotted |= sees;
        if sees && !spotted && alert > 0.0 {
            self.alert_kind(i, target, alert);
        }
        if sees && !was_hunting {
            self.auto_emote(i, "surprise");
        } else if !sees && was_hunting && self.loaded.entities[i].ai.aggro <= 0.0 {
            self.auto_emote(i, "confused");
        } else if hurt && !was_fleeing && (sees || was_hunting) {
            self.auto_emote(i, "fear");
        }
        // The player or a companion noticed (and it doesn't run): a fight starts.
        if matches!(foe, Some(Foe::Player | Foe::Companion(_))) && !hurt && self.start_combat(false, true) {
            return true;
        }
        let e = &mut self.loaded.entities[i];
        if e.ai.aggro <= 0.0 {
            e.ai = Ai { walker: std::mem::take(&mut e.ai.walker), ..Default::default() };
            return false;
        }
        // Badly hurt: run away from the player (seen or remembered); group fighters run to an
        // ally instead, catch their breath there and come back.
        e.ai.fleeing = hurt;
        if hurt {
            let from = if sees { target } else { e.ai.last_seen };
            let away = (pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
            // Once it reached an ally, it catches its breath there (the ally may have left to fight).
            let resting = e.ai.regroup > 0.0;
            let ally = if group && !resting { self.rally_point(i, pos, from) } else { None };
            if resting || ally.is_some() {
                let d = ally.map_or(Vec2::ZERO, |a| self.loaded.entities[a].body.pos - pos);
                if let (Some(_), true) = (ally, d.length() > 1.5 * ts) {
                    let dir = (d.normalize() + away * 0.4).normalize_or_zero();
                    self.step_towards_dir(i, dir, dt, None);
                } else {
                    // Next to the ally: it tells it (whatever its kind) and keeps the threat in mind.
                    if let Some(ally) = ally {
                        let a = &mut self.loaded.entities[ally].ai;
                        if a.aggro <= 0.0 {
                            a.aggro = AGGRO_MEMORY;
                            a.last_seen = from;
                            self.auto_emote(ally, "surprise");
                        }
                    }
                    let e = &mut self.loaded.entities[i];
                    Self::hold(&mut e.body, -away);
                    e.ai.aggro = e.ai.aggro.max(AGGRO_MEMORY);
                    e.ai.last_seen = from;
                    e.ai.regroup += dt.max(1e-4);
                    if e.ai.regroup >= REGROUP {
                        e.ai.rallied = true;
                        e.ai.fleeing = false;
                        e.ai.regroup = 0.0;
                        self.auto_emote(i, "anger");
                    }
                }
                return true;
            }
            self.step_towards_dir(i, away, dt, None);
            return true;
        }
        if !sees {
            // Hunt: walk to where the player was seen last (or where it was sent); then give up.
            let goal = e.ai.last_seen;
            if pos.distance(goal) < ts * 0.3 {
                e.ai.aggro = 0.0;
                e.body.moving = false;
                return false;
            }
            // Around trees and walls (A*, planned again when the goal moves to another tile); the
            // last stretch, and a goal it can't reach, straight.
            let tile = self.tile_of(goal);
            let mut walker = std::mem::take(&mut self.loaded.entities[i].ai.walker);
            if walker.path.last().is_some_and(|t| *t != tile) {
                walker.path.clear();
            }
            let walked = self.walk_to_tile(i, tile, &mut walker, 0.0, dt);
            self.loaded.entities[i].ai.walker = walker;
            if walked == Walk::Walking {
                return true;
            }
            let mut dir = (goal - pos).normalize_or_zero();
            if group {
                dir = (dir + self.spread(i, pos)).normalize_or_zero();
            }
            self.step_towards_dir(i, dir, dt, None);
            return true;
        }
        let dir = to.try_normalize().unwrap_or(vec2(0.0, 1.0));
        // Shooters: hold position within range; with a kept distance, back off when too close.
        if let Some(range) = ranged {
            // No clear shot (a wall or a tall prop between): step sideways to find one.
            if dist <= range && !self.line_clear(eye, target_mid) {
                let side = vec2(-dir.y, dir.x);
                let (left, right) = (self.line_clear(eye + side * 1.5 * ts, target_mid), self.line_clear(eye - side * 1.5 * ts, target_mid));
                let clear = |s: f32| if s > 0.0 { left } else { right };
                let e = &mut self.loaded.entities[i];
                if e.ai.orbit == 0.0 {
                    e.ai.orbit = if rand::gen_range(0, 2) == 0 { 1.0 } else { -1.0 };
                }
                let orbit = e.ai.orbit;
                let s = if !clear(orbit) && clear(-orbit) { -orbit } else { orbit };
                e.ai.orbit = s;
                // Sideways (on along the wall until a line opens); stuck: the other way.
                if self.step_dir(i, side * s, dt, Some(dir), 1.0) < 0.2 {
                    self.loaded.entities[i].ai.orbit = -s;
                }
                return true;
            }
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
        // Next to its foe: it stands (the fight starts when the player is near).
        if dist < 1.2 * ts {
            Self::hold(&mut self.loaded.entities[i].body, dir);
            return true;
        }
        let dir = if group { (dir + self.spread(i, pos)).normalize_or_zero() } else { dir };
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
        self.step_dir(i, dir, dt, face, 1.0);
    }

    /// [`Self::step_towards_dir`] at `scale` of the character's speed; returns how much of the
    /// step it got along `dir` (1 = all of it, 0 = stuck).
    fn step_dir(&mut self, i: usize, dir: Vec2, dt: f32, face: Option<Vec2>, scale: f32) -> f32 {
        let e = &self.loaded.entities[i];
        let (body, uid) = (e.body.clone(), e.uid);
        let full = body.speed * dt * scale;
        let mut new_pos = self.try_move(&body, dir * full, Mover::Npc(uid), false);
        let mut moved_dir = dir;
        if new_pos.distance(body.pos) < full * 0.5 {
            // Progress = how far a step gets along `dir` (sliding the wrong way scores below 0).
            let mut best = (new_pos - body.pos).dot(dir);
            for angle in [45.0f32, -45.0, 90.0, -90.0] {
                let d = Vec2::from_angle(angle.to_radians()).rotate(dir);
                let p = self.try_move(&body, d * full, Mover::Npc(uid), false);
                let progress = (p - body.pos).dot(dir);
                if progress > best + 0.001 {
                    best = progress;
                    new_pos = p;
                    moved_dir = d;
                }
            }
        }
        let progress = if full > 0.0 { ((new_pos - body.pos).dot(dir) / full).max(0.0) } else { 1.0 };
        let e = &mut self.loaded.entities[i];
        let look = face.unwrap_or(moved_dir);
        e.body.facing = Facing::from_vec(look.x, look.y).unwrap_or(e.body.facing);
        e.body.moving = new_pos != e.body.pos;
        e.body.pos = new_pos;
        e.body.animate(dt);
        progress
    }

    /// Walks character `i` home the way patrols walk (round props and, when blocked, people);
    /// returns false once it is there, or once it gave up (this is home now).
    fn walk_home(&mut self, i: usize, dt: f32) -> bool {
        let e = &self.loaded.entities[i];
        let (pos, home) = (e.body.pos, e.home);
        if pos.distance(home) < 3.0 {
            self.loaded.entities[i].body.moving = false;
            return false;
        }
        let goal = self.tile_of(home);
        let mut walker = std::mem::take(&mut self.loaded.entities[i].ai.walker);
        // A way planned while hunting leads elsewhere.
        if walker.path.last().is_some_and(|t| *t != goal) {
            walker.path.clear();
        }
        let walked = if self.tile_of(pos) == goal {
            // On the home tile: the last step goes straight to the spot.
            let e = &self.loaded.entities[i];
            let step = (home - pos).clamp_length_max(e.body.speed * dt);
            let (body, uid) = (e.body.clone(), e.uid);
            let new_pos = self.try_move(&body, step, Mover::Npc(uid), false);
            let e = &mut self.loaded.entities[i];
            e.body.facing = Facing::from_vec(step.x, step.y).unwrap_or(e.body.facing);
            e.body.moving = new_pos != pos;
            e.body.pos = new_pos;
            e.body.animate(dt);
            if new_pos.distance(pos) < 0.01 {
                if walker.blocked(dt) { Walk::GaveUp } else { Walk::Walking }
            } else {
                walker.moved();
                Walk::Walking
            }
        } else {
            self.walk_to_tile(i, goal, &mut walker, 0.0, dt)
        };
        self.loaded.entities[i].ai.walker = walker;
        match walked {
            Walk::Walking | Walk::Arrived => true,
            Walk::Unreachable | Walk::GaveUp => {
                // Can't get home: this is home now.
                let e = &mut self.loaded.entities[i];
                e.home = e.body.pos;
                e.body.moving = false;
                false
            }
        }
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

    /// For `status()`: what the hunting characters are doing (`hunting` = searching or closing
    /// in, `regroup` = badly hurt, next to an ally).
    pub(super) fn ai_status(&self) -> Vec<String> {
        let ts = self.ts();
        (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && (e.ai.aggro > 0.0 || e.ai.returning || e.ai.fleeing)
            })
            .map(|i| {
                let e = &self.loaded.entities[i];
                let what = if e.ai.returning {
                    "returning"
                } else if e.ai.fleeing && e.ai.regroup > 0.0 {
                    "regroup"
                } else if e.ai.fleeing {
                    "fleeing"
                } else {
                    "hunting"
                };
                format!("{}:{what}@{},{}", e.def, (e.body.pos.x / ts).floor(), (e.body.pos.y / ts).floor())
            })
            .collect()
    }
}
