//! The SRD's movement outside fights: falling into a *Drop* zone (a pit, a ledge's edge: 1d6
//! bludgeoning per 10 ft, at most 20d6, prone), long jumps over one (a running hero clears STR
//! feet, a standing one half), and climbing walls and ropes that need an Athletics check
//! (`PropDef::climb_dc`; a failure finds no hold for two seconds). In fights nobody steps into a drop
//! but fliers; walkers long-jump one as part of their move (`turns/` `reachable`).

use macroquad::prelude::*;

use super::occupancy::OCC_DROP;
use super::Game;
use crate::map::ZoneKind;
use crate::routes::RouteStep;

/// Feet of walking before a jump counts as running (the SRD's 10-ft run-up).
const RUN_UP_FT: f32 = 10.0;
/// Seconds before another try at a climb that found no hold.
const CLIMB_RETRY: f32 = 2.0;
/// Seconds a fall leaves the hero prone.
const FALL_PRONE: f32 = 1.0;

/// The hero's movement state.
#[derive(Default)]
pub(super) struct Moves {
    /// Pixels walked without stopping (the run-up of a jump).
    run_up: f32,
    /// The last direction walked.
    dir: Vec2,
    /// The last fall: feet and damage (`fall=`).
    pub last_fall: Option<(u32, i32)>,
    /// Seconds before the hero tries a climb with a check again.
    climb_wait: f32,
    /// The climb the hero has a hold on (its check passed) until they leave it.
    holding: bool,
    /// Pounds carried and the load they make (once a frame: `survival.rs`).
    pub carried: f32,
    pub load: crate::srd::survival::Load,
    /// Seconds before "You carry too much to move." shows again.
    pub over_wait: f32,
}

/// The dice of a fall from `feet` (1d6 per 10 ft, at most 20d6; "" = too low to hurt).
pub fn fall_dice(feet: u32) -> String {
    match (feet / 10).min(20) {
        0 => String::new(),
        n => format!("{n}d6"),
    }
}

/// How far a long jump goes: STR feet with a run-up, half from standing.
pub fn long_jump_ft(strength: i32, run_up: bool) -> u32 {
    let s = strength.max(0) as u32;
    if run_up {
        s
    } else {
        s / 2
    }
}

/// The main direction of `dir` as a tile step (the larger axis).
fn axis(dir: Vec2) -> (i32, i32) {
    if dir.x.abs() >= dir.y.abs() {
        (dir.x.signum() as i32, 0)
    } else {
        (0, dir.y.signum() as i32)
    }
}

/// Tiles of a drop from `from` along `step` (`from` included) until the first tile that isn't one
/// (`drop(t)`), at most `limit`: (how many, the tile past them).
pub fn gap_along(from: (i32, i32), step: (i32, i32), limit: i32, drop: impl Fn((i32, i32)) -> bool) -> (i32, (i32, i32)) {
    let mut t = from;
    let mut n = 0;
    while n < limit && drop(t) {
        n += 1;
        t = (t.0 + step.0, t.1 + step.1);
    }
    (n, t)
}

impl Game {
    /// Called every frame: the run-up of a jump (pixels walked without stopping or turning).
    pub(super) fn walk_step(&mut self, dir: Vec2, moved: f32, dt: f32) {
        self.moves.climb_wait = (self.moves.climb_wait - dt).max(0.0);
        self.moves.over_wait = (self.moves.over_wait - dt).max(0.0);
        if dir != Vec2::ZERO && self.moves.load == crate::srd::survival::Load::Over && self.moves.over_wait <= 0.0 {
            self.ui.toast(crate::lang::tr_ui("You carry too much to move.").into_owned());
            self.moves.over_wait = 3.0;
        }
        if dir == Vec2::ZERO || moved <= 0.0 {
            self.moves.run_up = 0.0;
            return;
        }
        if self.moves.dir != Vec2::ZERO && self.moves.dir.dot(dir) < 0.5 {
            self.moves.run_up = 0.0;
        }
        self.moves.dir = dir;
        self.moves.run_up += moved;
        self.march_step(dt);
    }

    /// Whether the hero, about to step from `before` onto a climbable wall with a DC, gets a hold:
    /// an Athletics check (none with a hold already). False = the step is refused.
    pub(super) fn climb_hold(&mut self, before: Vec2, after: Vec2) -> bool {
        let ts = self.ts();
        let (from, to) = (crate::grid::feet_tile(before, ts), crate::grid::feet_tile(after, ts));
        let on_dc = |g: &Game, t: (i32, i32)| g.loaded.climb_dc.get(&t).copied();
        if on_dc(self, from).is_none() {
            self.moves.holding = false;
        }
        let Some(dc) = on_dc(self, to) else { return true };
        // (Spider Climb: no check.)
        if self.hero_buff(|b| matches!(b, crate::srd::conditions::Buff::SpiderClimb)) {
            return true;
        }
        if self.moves.holding || from == to {
            return true;
        }
        if self.moves.climb_wait > 0.0 {
            return false;
        }
        let ok = self.world.check("athletics", dc as i32, crate::srd::Mode::Normal, false).is_some_and(|r| r.success == Some(true));
        if ok {
            self.moves.holding = true;
        } else {
            self.moves.climb_wait = CLIMB_RETRY;
            self.ui.toast(crate::lang::tr_ui("You find no hold.").into_owned());
        }
        ok
    }

    /// The hero walked into Drop zone `uid`: a long jump over it when it is narrow enough and the
    /// hero ran (or has the run-up), else a fall. True = the hero left the spot (fell or jumps).
    pub(super) fn enter_drop(&mut self, uid: u32, running: bool) -> bool {
        let Some(z) = self.loaded.map.zones.iter().find(|z| z.uid == uid) else { return false };
        let ZoneKind::Drop { height_ft, target_map, target_spawn } = z.kind.clone() else { return false };
        if self.hero_jumping() {
            return false;
        }
        // (Flying: over it.)
        if self.hero_buff(|b| matches!(b, crate::srd::conditions::Buff::Fly { .. })) {
            return false;
        }
        let ts = self.ts();
        let here = self.player_tile();
        let step = axis(if self.moves.dir == Vec2::ZERO { vec2(0.0, 1.0) } else { self.moves.dir });
        let limit = (self.loaded.map.width.max(self.loaded.map.height)) as i32;
        // (The zone fires when the hitbox touches it: the feet may still stand a tile before it.)
        let is_drop = |t: (i32, i32)| self.loaded.occupancy.has(t.0, t.1, OCC_DROP);
        let edge = (0..3).map(|k| (here.0 + step.0 * k, here.1 + step.1 * k)).find(|t| is_drop(*t)).unwrap_or(here);
        let (n, past) = gap_along(edge, step, limit, is_drop);
        let run_up = running || self.moves.run_up >= RUN_UP_FT / 5.0 * ts;
        // (Jump: three times as far.)
        let times = crate::srd::conditions::buffs(&self.world.effects).find_map(|b| if let crate::srd::conditions::Buff::Jump { times } = b { Some(*times) } else { None }).unwrap_or(1.0);
        let jump = (long_jump_ft(self.world.abilities.score(crate::srd::Ability::Str), run_up) as f32 * times.max(1.0)) as u32;
        // The landing: the tile past the gap, else one beside it (room for the hero's feet: open
        // ground, nothing standing there).
        let perp = (step.1, step.0);
        let open = |t: (i32, i32)| self.loaded.map.in_bounds(t.0, t.1) && !self.tile_solid(t.0, t.1) && !self.loaded.occupancy.has(t.0, t.1, OCC_DROP) && !self.rect_blocked(self.player.rect_at(crate::grid::feet_spot(t, ts)), super::Mover::Player, false);
        let landing = [past, (past.0 + perp.0, past.1 + perp.1), (past.0 - perp.0, past.1 - perp.1)].into_iter().find(|t| open(*t));
        if let (true, Some(land)) = (n > 0 && (n as u32) * 5 <= jump, landing) {
            // The hop: a short route over the gap (through what lies between).
            let (dx, dy) = (land.0 - here.0, land.1 - here.1);
            self.start_route("player", None, &[RouteStep::Through { on: true }, RouteStep::Jump { dx, dy }], false, false, false);
            self.moves.run_up = 0.0;
            return true;
        }
        let beyond = landing.filter(|_| n > 0);
        self.fall(height_ft, &target_map, &target_spawn, beyond);
        true
    }

    /// Whether one of the hero's boons is like `f` (Feather Fall, Fly, Spider Climb).
    pub(super) fn hero_buff(&self, f: impl Fn(&crate::srd::conditions::Buff) -> bool) -> bool {
        crate::srd::conditions::buffs(&self.world.effects).any(f)
    }

    /// Whether the hero is in the air (a jump of a route).
    pub(super) fn hero_jumping(&self) -> bool {
        self.route_jumping(super::routes::Walker::Player)
    }

    /// A fall of `feet`: the damage (rolled into the roll log), prone for a moment, and the
    /// landing: the nearest spawn named `spawn` on `map` ("" = this map), else `beyond`, else
    /// where the hero is.
    pub(super) fn fall(&mut self, feet: u32, map: &str, spawn: &str, beyond: Option<(i32, i32)>) {
        // (Feather Fall: down slowly, unhurt.)
        let feather = self.hero_buff(|b| matches!(b, crate::srd::conditions::Buff::FeatherFall));
        let dice = if feather { String::new() } else { fall_dice(feet) };
        let dmg = if dice.is_empty() { 0 } else { self.world.roll_dice(&dice).map_or(0, |r| r.total as i32) };
        self.moves.last_fall = Some((feet, dmg));
        self.moves.run_up = 0.0;
        self.ui.toast(crate::lang::tr_ui("You fall {} ft.").replacen("{}", &feet.to_string(), 1));
        let ts = self.ts();
        if !map.is_empty() && map != self.loaded.map.name {
            self.transition(map, super::Arrival::Spawn(spawn.to_string()));
        } else if !spawn.is_empty() {
            let at = self.player.pos;
            let spot = self.loaded.map.zones.iter().filter(|z| matches!(z.kind, ZoneKind::Spawn) && z.name == spawn).map(|z| z.arrival_point(ts)).min_by(|a, b| a.distance_squared(at).total_cmp(&b.distance_squared(at)));
            match spot {
                Some(p) => self.land_at(p),
                None => eprintln!("[game] Drop: map '{}' has no spawn '{spawn}'", self.loaded.map.name),
            }
        } else if let Some(t) = beyond {
            self.land_at(crate::grid::feet_spot(t, ts));
        }
        if dmg > 0 {
            let from = self.player.pos;
            self.hurt_player_by(dmg, from, Some(crate::srd::DamageType::Bludgeoning));
        }
        if self.game_over.is_none() && !feather {
            self.apply_player_effect(&crate::srd::conditions::ConditionSpec::new(crate::srd::Condition::Prone, FALL_PRONE));
        }
    }

    /// Puts the hero down at `pos` on this map (a free spot near it; the zones there don't fire).
    fn land_at(&mut self, pos: Vec2) {
        self.player.pos = self.free_spot(&self.player, pos, super::Mover::Player);
        self.player.move_target = None;
        self.order = None;
        self.zones_inside = self.zones_at(self.player.pos);
        self.place_companions();
    }

    /// `fall=` for status: the last fall's height and damage (`20ft:7`, `-`); `:jump` while in the
    /// air.
    pub(super) fn fall_status(&self) -> String {
        let last = self.moves.last_fall.map_or("-".to_string(), |(f, d)| format!("{f}ft:{d}"));
        if self.hero_jumping() {
            format!("{last}:jump")
        } else {
            last
        }
    }

    /// `climbing=` for status: `true`, `true:dc12` on a wall with a check, `false`.
    pub(super) fn climbing_status(&self) -> String {
        if !self.climbing() {
            return "false".into();
        }
        match self.loaded.climb_dc.get(&self.player_tile()) {
            Some(dc) => format!("true:dc{dc}"),
            None => "true".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_and_jumps_by_the_srd() {
        assert_eq!((fall_dice(5), fall_dice(10), fall_dice(25), fall_dice(250)), (String::new(), "1d6".into(), "2d6".into(), "20d6".into()));
        assert_eq!((long_jump_ft(15, true), long_jump_ft(15, false)), (15, 7));
        // A gap of three drop tiles going right from (2, 5): the landing is (5, 5).
        let drop = |t: (i32, i32)| t.1 == 5 && (2..5).contains(&t.0);
        assert_eq!(gap_along((2, 5), (1, 0), 50, drop), (3, (5, 5)));
        assert_eq!(gap_along((2, 5), (0, 1), 50, drop), (1, (2, 6)));
        assert_eq!(axis(vec2(0.3, -0.9)), (0, -1));
    }
}
