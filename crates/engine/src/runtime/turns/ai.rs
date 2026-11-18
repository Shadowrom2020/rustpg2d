//! The turns of enemies and of allies left to the AI: pick a foe, walk into reach (melee) or keep
//! a shot (ranged), attack, flee when badly hurt, drink a potion when critical. One step per
//! call, at a readable pace ([`super::Combat::ai_t`]).

use super::{AttackSpec, Side, Who};
use crate::grid::TilePos;
use crate::runtime::Game;
use crate::srd::combat::Cover;
use crate::srd::conditions;

fn dist(a: TilePos, b: TilePos) -> i32 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1)) as i32
}

impl Game {
    /// The living foes of `who` that can be attacked (not at 0 HP unless the attacker focuses).
    fn ai_foes(&self, who: Who, side: Side) -> Vec<Who> {
        let Some(c) = &self.fight else { return Vec::new() };
        let _ = who;
        c.order.iter().filter(|f| f.side != side && !f.out && !(f.hidden && side == Side::Enemy)).map(|f| f.who).collect()
    }

    /// Whether `who` runs (morale): frightened, or hurt below its *flee below* share.
    fn ai_flees(&self, who: Who) -> bool {
        let Some(i) = self.who_index(who) else { return false };
        let e = &self.loaded.entities[i];
        if conditions::rules(&e.effects).frightened {
            return true;
        }
        let flee_below = match e.behavior {
            crate::defs::Behavior::Chase { flee_below, .. } => flee_below,
            _ => 0.0,
        };
        e.companion.is_none() && e.max_hp > 0 && flee_below > 0.0 && (e.hp as f32) < flee_below * e.max_hp as f32
    }

    /// One step of an AI turn: attack, walk, act, or end the turn.
    pub(super) fn ai_step(&mut self) {
        let Some(c) = &self.fight else { return };
        let me = c.current().clone();
        let turn = c.turn.clone();
        let here = self.who_tile(me.who);
        // Running away: Dash once, as far from the foes as it gets.
        if self.ai_flees(me.who) {
            if !turn.action {
                let speed = self.who_speed_now(me.who);
                let c = self.fight.as_mut().unwrap();
                c.turn.action = true;
                c.turn.speed += speed;
                let name = self.who_name(me.who);
                self.fight_log(format!("{name}: {}", crate::lang::tr_ui("flees")));
            }
            if !self.fight.as_ref().unwrap().turn.ai_walked {
                let foes: Vec<TilePos> = self.ai_foes(me.who, me.side).into_iter().map(|w| self.who_tile(w)).collect();
                let reach = self.reachable(me.who);
                let best = reach.keys().copied().max_by_key(|t| (foes.iter().map(|f| dist(*t, *f)).min().unwrap_or(0), -(reach[t].0 as i32)));
                if let Some(goal) = best.filter(|g| *g != here) {
                    let path = Self::path_to(&reach, here, goal);
                    self.fight.as_mut().unwrap().turn.ai_walked = true;
                    self.walk(path);
                    return;
                }
            }
            // Far enough from everyone: it has left the fight.
            let foes: Vec<TilePos> = self.ai_foes(me.who, me.side).into_iter().map(|w| self.who_tile(w)).collect();
            if foes.iter().all(|f| dist(here, *f) as f32 >= super::FLED_TILES) {
                self.leaves_fight(me.who);
            }
            self.next_turn();
            return;
        }
        // A companion told to stay out of fights (order "passive") only dodges.
        let passive = self.who_index(me.who).and_then(|i| self.loaded.entities[i].companion.as_ref()).is_some_and(|l| l.order == crate::state::Order::Passive);
        if passive {
            if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(me.who)) {
                f.dodging = true;
            }
            self.next_turn();
            return;
        }
        // A potion when critical (the Use an Object action).
        if !turn.action {
            if let Some(i) = self.who_index(me.who) {
                if self.npc_drink_now(i) {
                    self.fight.as_mut().unwrap().turn.action = true;
                    return;
                }
            }
        }
        let specs = self.attack_specs(me.who);
        let foes = self.ai_foes(me.who, me.side);
        if foes.is_empty() || specs.is_empty() {
            self.next_turn();
            return;
        }
        // Attack now if a foe is in reach (the one most hurt for multiattackers, else the nearest).
        if !turn.action && turn.attacks_left > 0 {
            // (Foes down at 0 HP are left alone, except by those with Multiattack: they focus.)
            let focus = turn.attacks_left > 1 || self.who_block(me.who).is_some_and(|b| b.multiattack > 1);
            let mut options: Vec<(Who, AttackSpec, i32)> = Vec::new();
            for f in &foes {
                // Down party members are left alone unless the attacker focuses.
                let down = self.fight.as_ref().and_then(|c| c.fighter(*f)).is_some_and(|x| x.down);
                if down && !focus {
                    continue;
                }
                let d = dist(here, self.who_tile(*f));
                // Melee first when next to the foe; a shot otherwise.
                let mut best: Option<AttackSpec> = None;
                for s in &specs {
                    if self.can_attack(me.who, *f, s).is_ok() && (best.is_none() || (d <= 1 && !s.ranged)) {
                        best = Some(s.clone());
                    }
                }
                if let Some(s) = best {
                    let hp = self.who_hp(*f).0;
                    options.push((*f, s, if focus { hp } else { d }));
                }
            }
            if let Some((target, spec, _)) = options.into_iter().min_by_key(|o| o.2) {
                self.make_attack(me.who, target, &spec);
                if let Some(c) = &mut self.fight {
                    c.turn.attacks_left = c.turn.attacks_left.saturating_sub(1);
                    if c.turn.attacks_left == 0 {
                        c.turn.action = true;
                    }
                }
                return;
            }
        }
        // Walk: to a tile with a foe in reach, else as near the nearest foe as it gets.
        if !turn.ai_walked && turn.moved < turn.speed && !turn.action {
            let reach = self.reachable(me.who);
            let ranged = specs.iter().all(|s| s.ranged);
            let foe_tiles: Vec<TilePos> = foes.iter().map(|w| self.who_tile(*w)).collect();
            let nearest = foes.iter().copied().min_by_key(|w| dist(here, self.who_tile(*w)));
            let mut best: Option<(i32, TilePos)> = None;
            for (t, (cost, _)) in &reach {
                let near = foe_tiles.iter().map(|f| dist(*t, *f)).min().unwrap_or(99);
                let score = if ranged {
                    // Shooters: within their range with a line to the nearest foe, not next to a
                    // foe, the shortest walk.
                    let r = specs.iter().map(|s| s.range).max().unwrap_or(1);
                    let line = nearest.is_some_and(|f| near <= r && self.cover_from(*t, me.who, f) != Cover::Total);
                    if near <= r && near > 1 && line {
                        *cost as i32
                    } else {
                        1000 + (near - r).abs() * 10 + if line { 0 } else { 50 } + *cost as i32
                    }
                } else {
                    let r = specs.iter().filter(|s| !s.ranged).map(|s| s.reach).max().unwrap_or(1);
                    if near <= r {
                        *cost as i32
                    } else {
                        1000 + near * 10 + *cost as i32 / 5
                    }
                };
                if best.is_none_or(|b| score < b.0) {
                    best = Some((score, *t));
                }
            }
            self.fight.as_mut().unwrap().turn.ai_walked = true;
            if let Some((_, goal)) = best.filter(|b| b.1 != here) {
                let path = Self::path_to(&reach, here, goal);
                if !path.is_empty() {
                    self.walk(path);
                    return;
                }
            }
        }
        // Nothing in reach after walking: Dodge (the action) rather than nothing.
        if !turn.action && turn.ai_walked {
            if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(me.who)) {
                f.dodging = true;
            }
        }
        self.next_turn();
    }

    /// Speed (feet) for Dash: the creature's speed.
    pub(super) fn who_speed_now(&self, who: Who) -> u32 {
        self.who_speed(who)
    }

    /// A fleeing enemy got away: out of this fight (it walks home later).
    pub(super) fn leaves_fight(&mut self, who: Who) {
        let name = self.who_name(who);
        if let Some(f) = self.fight.as_mut().and_then(|c| c.fighter_mut(who)) {
            f.out = true;
        }
        self.fight_log(format!("{name} {}", crate::lang::tr_ui("got away")));
    }

    pub(super) fn fight_log(&mut self, line: String) {
        if let Some(c) = &mut self.fight {
            c.log(line);
        }
    }
}
