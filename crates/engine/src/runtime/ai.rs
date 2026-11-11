//! Enemy behaviour for `Behavior::Chase` characters: they chase the player they notice (within
//! their sight range in a clear line, or within half of it through walls and trees),
//! remember them for a few seconds after losing sight (walking to where they saw them last),
//! give up beyond their leash and walk home, flee when badly hurt, and shooters keep their
//! distance. Group fighters (`group`, the default) take turns: at most [`MAX_ATTACKERS`] close
//! in on one target, the others circle around it until a place frees up; they spread out
//! instead of stacking, alert others of their kind (`alert` tiles), and the badly hurt run to an
//! ally, catch their breath and come back. Shooters without a clear line step sideways to find
//! one. The state lives in [`Ai`] on each character (not saved).

use macroquad::prelude::*;

use super::patrol::Walk;
use super::{Game, Mover};
use crate::defs::Behavior;
use crate::entity::Facing;

/// Seconds a character keeps hunting after it lost sight of the player.
const AGGRO_MEMORY: f32 = 4.0;
/// Group fighters closing in on one target at the same time.
pub(super) const MAX_ATTACKERS: usize = 2;
/// Waiting group fighters circle this many tiles from their target, at this share of their speed.
const CIRCLE: f32 = 2.5;
const CIRCLE_SPEED: f32 = 0.55;
/// Chasers closer than this (tiles) push each other apart.
const SPREAD: f32 = 1.1;
/// The badly hurt run to allies within this many tiles (not those fighting right at the
/// threat) and come back after this many seconds next to one.
const RALLY_RANGE: f32 = 8.0;
const REGROUP: f32 = 3.0;

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
    /// A group fighter without a place at its target: it circles and doesn't attack.
    pub circling: bool,
    /// Circling direction (1 or -1; 0 = not chosen yet) and the seconds until it may turn.
    orbit: f32,
    orbit_t: f32,
    /// Seconds next to an ally while badly hurt; `rallied` = back in the fight (until it calms down).
    regroup: f32,
    rallied: bool,
    /// It has seen the target itself (not only been alerted): it raised the alarm then.
    spotted: bool,
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
    /// The feet of a foe that is still there.
    fn foe_pos(&self, foe: Foe) -> Option<Vec2> {
        match foe {
            Foe::Player => Some(self.player.pos),
            Foe::Companion(uid) => self.loaded.entities.iter().find(|e| e.uid == uid && e.active && e.companion.is_some()).map(|e| e.body.pos),
            Foe::Npc(uid) => self.loaded.entities.iter().find(|e| e.uid == uid && e.active && e.hp > 0).map(|e| e.body.pos),
        }
    }

    /// Character `i` fights: it is hostile to the player, or goes for a character of a hostile
    /// faction.
    pub(super) fn fights(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        e.hostile || matches!(e.ai.target, Some(Foe::Npc(_)))
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

    /// A group fighter that fights up close (swings or lunges; shooters don't take turns).
    fn group_melee(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        chase_of(&e.behavior).is_some_and(|c| c.group) && e.weapon.as_ref().is_none_or(|w| w.ranged.is_none()) && self.attack_kind(i).is_some()
    }

    /// Hands out the places at each target (once a frame, before the characters move): the
    /// [`MAX_ATTACKERS`] nearest group fighters close in, the others circle. Those already
    /// attacking keep their place, and those that had one are preferred a little (no swapping
    /// back and forth when they stand at about the same distance).
    pub(super) fn assign_attack_slots(&mut self) {
        let ts = self.ts();
        let mut wanting: Vec<(Foe, usize, f32)> = Vec::new();
        for i in 0..self.loaded.entities.len() {
            let e = &self.loaded.entities[i];
            if !e.active || !self.fights(i) || e.ai.aggro <= 0.0 || e.ai.harmless() || !self.group_melee(i) {
                continue;
            }
            let Some(foe) = e.ai.target else { continue };
            let Some(at) = self.foe_pos(foe) else { continue };
            let mut key = e.body.pos.distance(at);
            if e.attack.is_some() {
                key -= 1000.0 * ts;
            } else if !e.ai.circling {
                key -= 1.5 * ts;
            }
            wanting.push((foe, i, key));
        }
        for e in &mut self.loaded.entities {
            e.ai.circling = false;
        }
        wanting.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut taken: Vec<(Foe, usize)> = Vec::new();
        for (foe, i, _) in wanting {
            match taken.iter_mut().find(|(f, _)| *f == foe) {
                Some((_, n)) if *n >= MAX_ATTACKERS => self.loaded.entities[i].ai.circling = true,
                Some((_, n)) => *n += 1,
                None => taken.push((foe, 1)),
            }
        }
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
        if self.game_over.is_some() {
            self.loaded.entities[i].ai = Ai::default();
            return false;
        }
        // Winding up, swinging, catching its breath: it stands (a lunge's hop is a push).
        if e.attack.is_some_and(|a| a.holds()) {
            self.loaded.entities[i].body.moving = false;
            return true;
        }
        let melee_reach = self.strike_reach(i);
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
        let notices = |feet: Vec2, mid: Vec2| {
            let d = feet.distance(pos);
            d <= sight * ts && (d <= sight * ts * 0.5 || self.line_clear(eye, mid))
        };
        let mut seen: Vec<(Foe, Vec2, Vec2)> = Vec::new();
        let player_mid = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        // The player and companions only when it is hostile to the player.
        if e.hostile {
            if notices(self.player.pos, player_mid) {
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
        let hurt = e.max_hp > 0 && flee_below > 0.0 && (e.hp as f32) < flee_below * e.max_hp as f32;
        let ranged = e.weapon.as_ref().and_then(|w| w.ranged.as_ref()).map(|r| r.range * ts);
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
            // Hunt: walk to where the player was seen last; then give up.
            let goal = e.ai.last_seen;
            if pos.distance(goal) < ts * 0.3 {
                e.ai.aggro = 0.0;
                e.body.moving = false;
                return false;
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
        // Waiting for a place at the target: circle around it at a distance, facing it.
        if self.loaded.entities[i].ai.circling && dist < (CIRCLE + 2.0) * ts {
            let e = &mut self.loaded.entities[i];
            e.ai.orbit_t -= dt;
            if e.ai.orbit == 0.0 || e.ai.orbit_t <= 0.0 {
                e.ai.orbit = if e.ai.orbit == 0.0 || rand::gen_range(0, 3) == 0 { if rand::gen_range(0, 2) == 0 { 1.0 } else { -1.0 } } else { e.ai.orbit };
                e.ai.orbit_t = rand::gen_range(2.5, 5.0);
            }
            let out = -dir;
            let tangent = vec2(-out.y, out.x) * e.ai.orbit;
            let radial = ((dist - CIRCLE * ts) / ts).clamp(-1.0, 1.0);
            let step = (tangent - out * radial + self.spread(i, pos) * 1.5).normalize_or_zero();
            if self.step_dir(i, step, dt, Some(dir), CIRCLE_SPEED) < 0.2 {
                // Blocked (a wall, another fighter): go round the other way.
                let e = &mut self.loaded.entities[i];
                e.ai.orbit = -e.ai.orbit;
                e.ai.orbit_t = rand::gen_range(1.5, 3.0);
            }
            return true;
        }
        // Armed: stop where the weapon reaches (the swing starts in `attacks.rs`).
        if dist < 1.0 || melee_reach.is_some_and(|r| dist <= r * 0.8) {
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

    /// For `status()`: what the hunting characters are doing (`hunting` = searching or shooting,
    /// `attack` = a group fighter closing in, `circle` = one waiting for its turn, `regroup` =
    /// badly hurt, next to an ally).
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
                } else if e.ai.circling {
                    "circle"
                } else if e.ai.target.is_some() && self.group_melee(i) {
                    "attack"
                } else {
                    "hunting"
                };
                format!("{}:{what}@{},{}", e.def, (e.body.pos.x / ts).floor(), (e.body.pos.y / ts).floor())
            })
            .collect()
    }
}
