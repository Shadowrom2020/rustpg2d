//! The player's defence. **Block** (held, with a shield in the off hand): walking at half
//! speed and without attacking, hits from the front (swings, arrows, lunges, touches) do no
//! damage, only a small push; blocking right before a swing lands (the first 0.2 s) **parries**
//! it and stuns the attacker for a second. **Dodge**: a quick roll of about three tiles in the
//! direction the player moves (or faces), untouchable while it lasts, then a short cooldown.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::Game;

/// The roll: its time and speed (tiles per second); the pause before the next one.
const ROLL_TIME: f32 = 0.3;
const ROLL_SPEED: f32 = 10.0;
const DODGE_COOLDOWN: f32 = 1.0;
/// A block this young parries a swing.
const PARRY: f32 = 0.2;
/// Hits from within this angle (degrees) of the facing direction are in front.
const FRONT: f32 = 70.0;
/// A parried attacker is stunned this long.
const PARRY_STUN: f32 = 1.0;

/// How a hit on the player ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Guard {
    /// It lands.
    Open,
    /// Blocked by the shield (no damage).
    Blocked,
    /// Blocked right as it came: the attacker is stunned.
    Parried,
}

impl Game {
    /// A shield is in the player's off hand (an off-hand item with armor; lanterns don't count).
    pub(super) fn has_shield(&self) -> bool {
        self.world.equipment.item(crate::defs::EquipSlot::Offhand).and_then(|id| self.project.db.item(id)).is_some_and(|d| d.armor.as_ref().is_some_and(|a| a.slot == crate::defs::EquipSlot::Offhand))
    }

    pub(super) fn blocking(&self) -> bool {
        self.combat.block.is_some()
    }

    pub(super) fn rolling(&self) -> bool {
        self.combat.roll.is_some()
    }

    /// Block and dodge input (call where the player attacks; not while frozen).
    pub(super) fn update_defense(&mut self, dt: f32, key_consumed: bool) {
        let c = &mut self.combat;
        c.dodge_cd = (c.dodge_cd - dt).max(0.0);
        let can = !self.player_stunned() && !self.climbing() && self.seat.is_none();
        // Block: held with a shield.
        let held = !key_consumed && can && input::act_down(Act::Block) && !self.rolling();
        if held && !self.has_shield() {
            if input::act_pressed(Act::Block) {
                self.ui.toast("You need a shield to block.".into());
            }
            self.combat.block = None;
        } else if held {
            self.combat.block = Some(self.combat.block.map_or(0.0, |t| t + dt));
        } else {
            self.combat.block = None;
        }
        // Dodge: a roll where the player is heading (or facing). B also cancels aiming: that first.
        if !key_consumed && can && !self.aiming() && self.combat.dodge_cd <= 0.0 && !self.rolling() && input::act_pressed(Act::Dodge) {
            let mut dir = Vec2::ZERO;
            for (a, d) in [(Act::Left, vec2(-1.0, 0.0)), (Act::Right, vec2(1.0, 0.0)), (Act::Up, vec2(0.0, -1.0)), (Act::Down, vec2(0.0, 1.0))] {
                if input::act_down(a) {
                    dir += d;
                }
            }
            let dir = dir.try_normalize().unwrap_or_else(|| Vec2::from(self.player.facing.dir()));
            self.combat.roll = Some((dir, 0.0));
            self.combat.block = None;
            self.combat.dodge_cd = DODGE_COOLDOWN + ROLL_TIME;
            self.order = None;
            self.player.move_target = None;
            self.audio.play(&self.project.config.sfx.attack);
        }
    }

    /// Moves a rolling player (in `update_player`); false when not rolling.
    pub(super) fn update_roll(&mut self, dt: f32) -> bool {
        let Some((dir, t)) = self.combat.roll else { return false };
        let ts = self.ts();
        self.player.pos = self.try_move(&self.player, dir * ROLL_SPEED * ts * dt, super::Mover::Player, false);
        if let Some(f) = crate::entity::Facing::from_vec(dir.x, dir.y) {
            self.player.facing = f;
        }
        self.player.moving = true;
        self.player.animate(dt * 2.0);
        let t = t + dt;
        self.combat.roll = (t < ROLL_TIME).then_some((dir, t));
        true
    }

    /// How a hit coming from `from` (the attacker's or the projectile's position) ends.
    pub(super) fn guard_against(&self, from: Vec2) -> Guard {
        let Some(held) = self.combat.block else { return Guard::Open };
        let to = from - self.player.pos;
        let facing = Vec2::from(self.player.facing.dir());
        if to.length() > 0.5 && to.angle_between(facing).abs() > FRONT.to_radians() {
            return Guard::Open;
        }
        if held <= PARRY {
            Guard::Parried
        } else {
            Guard::Blocked
        }
    }

    /// A hit was blocked: a small push, the clang; a parry stuns `attacker`.
    pub(super) fn blocked(&mut self, guard: Guard, from: Vec2, attacker: Option<usize>) {
        let away = (self.player.pos - from).try_normalize().unwrap_or(vec2(0.0, 1.0));
        self.combat.knock = away * super::combat::KNOCKBACK * 0.35 * self.u();
        let at = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0) - away * 8.0 * self.u();
        self.add_impact(at);
        self.audio.play(&self.project.config.sfx.hit);
        if guard == Guard::Parried {
            if let Some(i) = attacker {
                let stun = crate::effects::EffectSpec { kind: crate::effects::EffectKind::Stun, strength: 0.0, seconds: PARRY_STUN };
                crate::effects::apply(&mut self.loaded.entities[i].effects, &stun);
                self.loaded.entities[i].attack = None;
            }
            self.ui.toast("Parried!".into());
        }
    }

    /// The shield raised in front of the player, and the dust of a roll.
    pub(super) fn draw_defense(&self) {
        let u = self.u();
        let center = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        if self.blocking() {
            let dir = Vec2::from(self.player.facing.dir());
            let base = dir.y.atan2(dir.x);
            let r = 14.0 * u;
            let arc = FRONT.to_radians();
            for k in 0..8 {
                let t0 = base - arc + 2.0 * arc * k as f32 / 8.0;
                let t1 = base - arc + 2.0 * arc * (k + 1) as f32 / 8.0;
                draw_line(center.x + t0.cos() * r, center.y + t0.sin() * r, center.x + t1.cos() * r, center.y + t1.sin() * r, 1.5 * u, Color::new(0.6, 0.85, 1.0, 0.8));
            }
            let icon = self.world.equipment.item(crate::defs::EquipSlot::Offhand).and_then(|id| self.project.db.item(id)).map(|d| d.icon.clone());
            if let Some(icon) = icon {
                self.assets.draw_frame_scaled(&icon.sheet, icon.frame, center + dir * 9.0 * u, 0.0, 0.8, WHITE);
            }
        }
        if let Some((dir, t)) = self.combat.roll {
            for k in 1..=3 {
                let p = self.player.pos - dir * (k as f32 * 7.0 * u);
                let a = (0.45 - k as f32 * 0.12) * (1.0 - t / ROLL_TIME);
                draw_circle(p.x, p.y - 2.0 * u, (5.0 - k as f32) * u, Color::new(0.85, 0.8, 0.7, a.max(0.0)));
            }
        }
    }

    /// `block=` / `dodge=` for `status`.
    pub(super) fn defense_status(&self) -> String {
        let block = match (self.combat.block, self.combat.roll) {
            (_, Some(_)) => "roll".to_string(),
            (Some(t), _) if t <= PARRY => "parry".to_string(),
            (Some(_), _) => "on".to_string(),
            _ => "off".to_string(),
        };
        let dodge = if self.combat.dodge_cd <= 0.0 { "ready".to_string() } else { format!("{:.1}", self.combat.dodge_cd) };
        format!("block={block} dodge={dodge}")
    }
}
