//! Attacks of hostile characters you can see coming. An armed enemy (a melee weapon in its
//! equipment or definition) stops, **winds up** (its weapon raised, the swing's reach shown on
//! the ground), **swings** in the direction it locked when the wind-up began, hitting the player
//! or companions inside the weapon's arc, and **recovers** (the weapon's cooldown + a pause). An
//! unarmed chaser (a slime) crouches for a moment and then **lunges** at its target, hurting by
//! touch only during the hop. Stepping out of the way during the wind-up avoids the hit.
//! Companions' swings are drawn the same way. The state lives on each character (not saved).

use macroquad::prelude::*;

use super::Game;
use crate::defs::Behavior;
use crate::entity::Facing;

/// Wind-up of a weapon swing and of a lunge unless the definition sets one (`CharacterDef::windup`).
pub(super) const STRIKE_WINDUP: f32 = 0.4;
const LUNGE_WINDUP: f32 = 0.3;
/// How long a swing is drawn.
const SWING_TIME: f32 = 0.18;
/// The hop of a lunge: its time and speed (tiles per second).
const LUNGE_TIME: f32 = 0.25;
const LUNGE_SPEED: f32 = 7.5;
/// Pause after an attack on top of the weapon's cooldown (a lunge: all of it).
const RECOVER_EXTRA: f32 = 0.5;
const LUNGE_RECOVER: f32 = 0.9;
/// A lunge starts from this close (tiles).
const LUNGE_RANGE: f32 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Windup,
    Swing,
    Recover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// A weapon swing (enemies).
    Strike,
    /// A hop at the target, hurting by touch (unarmed chasers).
    Lunge,
    /// A companion's swing: drawn only (its hit is dealt where it attacks).
    Shown,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Attack {
    pub kind: Kind,
    pub phase: Phase,
    /// Seconds in the current phase.
    pub t: f32,
    /// The direction it attacks in, locked when the wind-up began.
    pub dir: Vec2,
    /// A lunge hurt someone already (once per hop).
    pub hit: bool,
}

impl Attack {
    /// The animation frame it shows: wind-up, the swing in two halves, then the first moment of
    /// recovery (a lunge's hop too); afterwards the character stands as usual.
    pub fn anim(&self) -> Option<super::paperdoll::Anim> {
        use super::paperdoll::Anim;
        match self.phase {
            Phase::Windup => Some(Anim::Attack(0)),
            Phase::Swing => Some(Anim::Attack(if self.t < SWING_TIME / 2.0 { 1 } else { 2 })),
            Phase::Recover if self.t < 0.2 => Some(Anim::Attack(3)),
            Phase::Recover => None,
        }
    }

    /// It stands still (winding up, swinging, catching its breath).
    pub fn holds(&self) -> bool {
        self.kind != Kind::Shown
    }

    fn name(&self) -> &'static str {
        match (self.kind, self.phase) {
            (Kind::Lunge, Phase::Swing) => "lunge",
            (_, Phase::Windup) => "windup",
            (_, Phase::Swing) => "swing",
            (_, Phase::Recover) => "recover",
        }
    }
}

/// What kind of attack a character makes: a weapon swing with a melee weapon, a lunge for an
/// unarmed chaser with contact damage, none otherwise (archers shoot, others touch).
pub(super) fn kind_of(weapon: Option<&crate::defs::Weapon>, damage: i32, behavior: &Behavior) -> Option<Kind> {
    match weapon {
        Some(w) if w.ranged.is_none() => Some(Kind::Strike),
        Some(_) => None,
        None if damage > 0 && matches!(behavior, Behavior::Chase { .. }) => Some(Kind::Lunge),
        None => None,
    }
}

impl Game {
    /// The kind of attack of character `i` (see [`kind_of`]).
    pub(super) fn attack_kind(&self, i: usize) -> Option<Kind> {
        let e = &self.loaded.entities[i];
        kind_of(e.weapon.as_ref(), e.damage, &e.behavior)
    }

    /// Melee reach of character `i` in pixels (its weapon's), for closing in.
    pub(super) fn strike_reach(&self, i: usize) -> Option<f32> {
        let e = &self.loaded.entities[i];
        e.weapon.as_ref().filter(|w| w.ranged.is_none()).map(|w| w.reach.max(0.5) * self.ts())
    }

    /// Whom character `i` can hit: the player and the companions that can be hurt (when it is
    /// hostile to the player), and the character of a hostile faction it goes for: (None = player /
    /// Some(character), feet, middle of the body, radius).
    fn attack_targets(&self, i: usize) -> Vec<(Option<usize>, Vec2, Vec2, f32)> {
        let mut out = Vec::new();
        let me = &self.loaded.entities[i];
        if me.hostile {
            let p = &self.player;
            out.push((None, p.pos, p.pos - vec2(0.0, p.hitbox.y / 2.0), p.hitbox.x.max(p.hitbox.y) / 2.0));
            for (c, mid) in self.companion_targets() {
                let e = &self.loaded.entities[c];
                out.push((Some(c), e.body.pos, mid, e.body.hitbox.x.max(e.body.hitbox.y) / 2.0));
            }
        }
        if let Some(super::ai::Foe::Npc(uid)) = me.ai.target {
            if let Some(j) = self.loaded.entities.iter().position(|e| e.uid == uid && e.active && e.hp > 0) {
                let e = &self.loaded.entities[j];
                out.push((Some(j), e.body.pos, e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0), e.body.hitbox.x.max(e.body.hitbox.y) / 2.0));
            }
        }
        out
    }

    /// Runs the attacks of hostile characters (and times the companions' shown swings).
    pub(super) fn update_attacks(&mut self, dt: f32) {
        let ts = self.ts();
        for i in 0..self.loaded.entities.len() {
            let e = &mut self.loaded.entities[i];
            if let Some(a) = &mut e.attack {
                a.t += dt;
            }
            // Shown swings (companions) just fade.
            if e.attack.is_some_and(|a| a.kind == Kind::Shown) {
                if e.attack.is_some_and(|a| a.t >= SWING_TIME) {
                    e.attack = None;
                }
                continue;
            }
            let ready = e.active && !e.ai.harmless() && !crate::effects::stunned(&e.effects) && self.game_over.is_none();
            let ready = ready && self.fights(i);
            let e = &mut self.loaded.entities[i];
            if !ready {
                e.attack = None;
                continue;
            }
            let Some(kind) = self.attack_kind(i) else { continue };
            let windup = {
                let w = self.loaded.entities[i].windup;
                if w > 0.0 {
                    w
                } else if kind == Kind::Lunge {
                    LUNGE_WINDUP
                } else {
                    STRIKE_WINDUP
                }
            };
            match self.loaded.entities[i].attack {
                None => self.maybe_start_attack(i, kind),
                Some(a) => match (a.kind, a.phase) {
                    (_, Phase::Windup) if a.t >= windup => {
                        self.set_phase(i, Phase::Swing);
                        if kind == Kind::Strike {
                            self.strike(i, a.dir);
                        }
                    }
                    (Kind::Lunge, Phase::Swing) => {
                        // The hop: straight at the locked direction, stopped by walls.
                        let e = &self.loaded.entities[i];
                        let (body, uid) = (e.body.clone(), e.uid);
                        let p = self.try_move(&body, a.dir * LUNGE_SPEED * ts * dt, super::Mover::Npc(uid), false);
                        self.loaded.entities[i].body.pos = p;
                        if !a.hit {
                            self.lunge_hit(i);
                        }
                        if a.t >= LUNGE_TIME {
                            self.set_phase(i, Phase::Recover);
                        }
                    }
                    (_, Phase::Swing) if a.t >= SWING_TIME => self.set_phase(i, Phase::Recover),
                    (_, Phase::Recover) => {
                        let e = &self.loaded.entities[i];
                        let rest = if kind == Kind::Lunge { LUNGE_RECOVER } else { e.weapon.as_ref().map_or(0.5, |w| w.cooldown.max(0.2)) + RECOVER_EXTRA };
                        if a.t >= rest {
                            self.loaded.entities[i].attack = None;
                        }
                    }
                    _ => {}
                },
            }
        }
    }

    fn set_phase(&mut self, i: usize, phase: Phase) {
        if let Some(a) = &mut self.loaded.entities[i].attack {
            a.phase = phase;
            a.t = 0.0;
        }
    }

    /// Starts a wind-up when a target is in reach (in sight, while hunting).
    fn maybe_start_attack(&mut self, i: usize, kind: Kind) {
        let ts = self.ts();
        let e = &self.loaded.entities[i];
        // Group fighters waiting for their turn don't attack.
        if e.ai.aggro <= 0.0 && matches!(e.behavior, Behavior::Chase { .. }) || e.knock != Vec2::ZERO || e.ai.circling {
            return;
        }
        let center = e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0);
        let reach = match kind {
            Kind::Strike => self.strike_reach(i).unwrap_or(ts),
            _ => LUNGE_RANGE * ts,
        };
        let target = self
            .attack_targets(i)
            .into_iter()
            .map(|(_, _, mid, r)| (mid, (mid - center).length() - r))
            .filter(|(mid, d)| *d <= reach && self.line_clear(center, *mid))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((mid, _)) = target else { return };
        let dir = (mid - center).try_normalize().unwrap_or(vec2(0.0, 1.0));
        let e = &mut self.loaded.entities[i];
        e.body.moving = false;
        e.body.facing = Facing::from_vec(dir.x, dir.y).unwrap_or(e.body.facing);
        e.attack = Some(Attack { kind, phase: Phase::Windup, t: 0.0, dir, hit: false });
    }

    /// The swing of character `i` in direction `dir`: whoever is inside its weapon's arc now.
    fn strike(&mut self, i: usize, dir: Vec2) {
        let ts = self.ts();
        let e = &self.loaded.entities[i];
        let Some(w) = e.weapon.clone() else { return };
        let center = e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0);
        let reach = w.reach.max(0.5) * ts;
        let arc = w.arc.clamp(5.0, 180.0).to_radians();
        let damage = crate::effects::scale_damage(&e.effects, w.damage.max(1));
        let from = e.body.pos;
        self.audio.play(&self.project.config.sfx.attack);
        for (who, _, mid, r) in self.attack_targets(i) {
            let to = mid - center;
            if !(to.length() < ts * 0.5 || (to.length() - r <= reach && to.angle_between(dir).abs() <= arc)) {
                continue;
            }
            match who {
                None if self.combat.invuln <= 0.0 => match self.guard_against(from) {
                    super::defense::Guard::Open => {
                        if let Some(fx) = w.effect {
                            self.apply_player_effect(&fx);
                        }
                        self.hurt_player(damage, from);
                    }
                    g => self.blocked(g, from, Some(i)),
                },
                None => {}
                Some(c) => {
                    if let Some(fx) = &w.effect {
                        crate::effects::apply(&mut self.loaded.entities[c].effects, fx);
                    }
                    self.npc_hits(c, damage, from, w.knockback);
                }
            }
        }
    }

    /// During a lunge's hop: the first target it touches is hurt (contact damage, touch effect).
    fn lunge_hit(&mut self, i: usize) {
        let e = &self.loaded.entities[i];
        let me = e.body.rect();
        let (dmg, from) = (crate::effects::scale_damage(&e.effects, e.damage), e.body.pos);
        let player_hit = self.combat.invuln <= 0.0 && me.overlaps(&Rect::new(self.player.rect().x - 2.0, self.player.rect().y - 2.0, self.player.rect().w + 4.0, self.player.rect().h + 4.0));
        if player_hit && self.rolling() {
            return;
        }
        if player_hit {
            match self.guard_against(from) {
                super::defense::Guard::Open => {
                    if let Some(fx) = self.touch_effect(i) {
                        self.apply_player_effect(&fx);
                    }
                    self.hurt_player(dmg, from);
                }
                g => self.blocked(g, from, Some(i)),
            }
        } else if let Some(c) = self.attack_targets(i).into_iter().filter_map(|t| t.0).find(|&c| self.loaded.entities[c].invuln <= 0.0 && self.loaded.entities[c].body.rect().overlaps(&me)) {
            self.npc_hits(c, dmg, from, 1.0);
        } else {
            return;
        }
        if let Some(a) = &mut self.loaded.entities[i].attack {
            a.hit = true;
        }
    }

    /// A companion swung its weapon at `dir` (the hit is dealt by its own code): drawn like the
    /// player's swing.
    pub(super) fn show_swing(&mut self, i: usize, dir: Vec2) {
        self.loaded.entities[i].attack = Some(Attack { kind: Kind::Shown, phase: Phase::Swing, t: 0.0, dir, hit: true });
    }

    /// `attacks=` for `status`: name:phase of characters attacking now.
    pub(super) fn attacks_status(&self) -> Vec<String> {
        self.loaded
            .entities
            .iter()
            .filter_map(|e| e.attack.filter(|a| a.kind != Kind::Shown).map(|a| format!("{}:{}", if e.name.is_empty() { &e.def } else { &e.name }, a.name())))
            .collect()
    }

    /// The tells and swings in the world (over the characters).
    pub(super) fn draw_attacks(&self) {
        let ts = self.ts();
        let u = self.u();
        for e in self.loaded.entities.iter().filter(|e| e.active) {
            let Some(a) = e.attack else { continue };
            let center = e.body.pos - vec2(0.0, e.body.hitbox.y / 2.0);
            let base = a.dir.y.atan2(a.dir.x);
            // (A weapon whose held sheet has attack rows swings in the hand instead.)
            let icon = if self.weapon_animates(&e.equipment) || self.figure_animates(&e.appearance) { None } else { e.equipment.item(crate::defs::EquipSlot::Weapon).and_then(|id| self.project.db.item(id)).map(|d| d.icon.clone()) };
            match (a.kind, a.phase) {
                (Kind::Strike, Phase::Windup) => {
                    // Where the swing will land, filling up; the weapon raised behind the head.
                    let w = e.weapon.as_ref();
                    let reach = w.map_or(ts, |w| w.reach.max(0.5) * ts);
                    let arc = w.map_or(1.0, |w| w.arc.clamp(5.0, 180.0).to_radians()).min(1.4);
                    let windup = if e.windup > 0.0 { e.windup } else { STRIKE_WINDUP };
                    let p = (a.t / windup).clamp(0.0, 1.0);
                    let steps = 10;
                    for k in 0..steps {
                        let t0 = base - arc + 2.0 * arc * k as f32 / steps as f32;
                        let t1 = base - arc + 2.0 * arc * (k + 1) as f32 / steps as f32;
                        let (a0, a1) = (center + vec2(t0.cos(), t0.sin()) * reach, center + vec2(t1.cos(), t1.sin()) * reach);
                        draw_triangle(center, a0, a1, Color::new(1.0, 0.15, 0.1, 0.18 + 0.3 * p));
                        // The edge of the danger zone.
                        draw_line(a0.x, a0.y, a1.x, a1.y, 1.5 * u, Color::new(1.0, 0.3, 0.2, 0.5 + 0.5 * p));
                    }
                    if let Some(icon) = &icon {
                        let at = e.body.pos - vec2(0.0, e.body.hitbox.y + ts * 1.05) - a.dir * 4.0 * u;
                        self.assets.draw_frame_scaled(&icon.sheet, icon.frame, at, base + std::f32::consts::PI + std::f32::consts::FRAC_PI_4, 1.0, WHITE);
                    }
                }
                (Kind::Strike | Kind::Shown, Phase::Swing) => {
                    let w = e.weapon.as_ref();
                    let reach = w.map_or(ts, |w| w.reach.max(0.5) * ts).max(ts * 0.6);
                    let arc = w.map_or(1.0, |w| w.arc.clamp(5.0, 180.0).to_radians()).min(1.4);
                    let p = (a.t / SWING_TIME).clamp(0.0, 1.0);
                    let (a0, a1) = (base - arc, base - arc + 2.0 * arc * p);
                    let steps = 8;
                    let col = if a.kind == Kind::Shown { Color::new(0.85, 0.95, 1.0, 1.0) } else { Color::new(1.0, 0.75, 0.6, 1.0) };
                    for k in 0..steps {
                        let t0 = a0 + (a1 - a0) * k as f32 / steps as f32;
                        let t1 = a0 + (a1 - a0) * (k + 1) as f32 / steps as f32;
                        draw_line(center.x + t0.cos() * reach, center.y + t0.sin() * reach, center.x + t1.cos() * reach, center.y + t1.sin() * reach, 2.0 * u, Color { a: 0.3 + 0.7 * (k as f32 / steps as f32), ..col });
                    }
                    if let Some(icon) = &icon {
                        let at = center + vec2(a1.cos(), a1.sin()) * (reach * 0.5).max(ts * 0.45);
                        self.assets.draw_frame_scaled(&icon.sheet, icon.frame, at, a1 + std::f32::consts::FRAC_PI_4, 1.0, WHITE);
                    }
                }
                (Kind::Lunge, Phase::Windup) => {
                    // A crouch: a dark ring tightening around it and an arrow where it will jump.
                    let windup = if e.windup > 0.0 { e.windup } else { LUNGE_WINDUP };
                    let p = (a.t / windup).clamp(0.0, 1.0);
                    let r = (1.0 - p) * ts * 0.7 + ts * 0.25;
                    draw_circle_lines(e.body.pos.x, e.body.pos.y - 2.0 * u, r, 1.5 * u, Color::new(1.0, 0.3, 0.2, 0.4 + 0.5 * p));
                    let tip = center + a.dir * ts * (0.6 + 0.6 * p);
                    draw_line(center.x, center.y, tip.x, tip.y, 1.5 * u, Color::new(1.0, 0.3, 0.2, 0.5 * p));
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{Ranged, SpriteRef, Weapon};

    fn weapon(ranged: bool) -> Weapon {
        Weapon {
            damage: 2,
            cooldown: 0.4,
            ranged: ranged.then(|| Ranged { projectile: SpriteRef::new("items", 9), speed: 300.0, range: 6.0, ammo: String::new() }),
            reach: 1.0,
            arc: 60.0,
            knockback: 1.0,
            two_handed: false,
            effect: None,
            crit: 0.05,
        }
    }

    #[test]
    fn who_attacks_how() {
        let chase = Behavior::chase(5.0);
        assert_eq!(kind_of(Some(&weapon(false)), 0, &Behavior::Static), Some(Kind::Strike), "armed: a swing, whatever its behaviour");
        assert_eq!(kind_of(Some(&weapon(true)), 2, &chase), None, "archers shoot");
        assert_eq!(kind_of(None, 2, &chase), Some(Kind::Lunge), "an unarmed chaser lunges");
        assert_eq!(kind_of(None, 2, &Behavior::Static), None, "an unarmed trap hurts by touch");
        assert_eq!(kind_of(None, 0, &chase), None, "harmless");
    }
}
