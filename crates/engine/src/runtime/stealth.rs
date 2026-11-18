//! Sneaking: the player toggles it (Act::Sneak: Left Ctrl / V, the left stick) and walks at half
//! speed (a model's "sneak" clip, else the walk slowed down). Enemies (`ai.rs`) notice the player
//! within [`Game::player_sight`] of their sight: half while sneaking, and less in the dark (the map's
//! light by the clock and the light sources near the player, the player's own lantern included),
//! but always within [`NEAR_NOTICE`]. A hit on an enemy that hasn't noticed the player is a sneak
//! attack: double damage, more with *Sneak attack* skills, shown like a critical hit.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::text::draw_text;
use super::Game;

/// Sight range while sneaking (times the enemy's sight).
const SNEAK_SIGHT: f32 = 0.5;
/// Sight range in full darkness (times the enemy's sight; 1 in full light).
const DARK_SIGHT: f32 = 0.4;
/// Enemies notice the player within this many tiles however careful they are (also behind walls):
/// bumping into them. Less than a melee strike's reach, so a careful sneak can strike first.
pub(super) const NEAR_NOTICE: f32 = 0.75;
/// Walking speed while sneaking.
pub(super) const SNEAK_SPEED: f32 = 0.5;
/// Damage of a sneak attack (times the hit's), before skills.
const SNEAK_ATTACK: f32 = 2.0;

impl Game {
    pub(super) fn sneaking(&self) -> bool {
        self.combat.sneak
    }

    /// The sneak key toggles sneaking (call with the other combat input).
    pub(super) fn update_sneak(&mut self, key_consumed: bool) {
        if !key_consumed && input::act_pressed(Act::Sneak) && !self.player_stunned() && !self.rolling() {
            self.combat.sneak = !self.combat.sneak;
        }
    }

    /// How lit the player is: 0 (dark) to 1 (daylight, a lamp right beside them). The map's
    /// ambient light by the clock plus every light source in reach (as drawn: full at its
    /// middle, nothing at its radius).
    pub(super) fn light_at_player(&self) -> f32 {
        let ambient = crate::render::lighting::ambient(self.loaded.map.lighting, self.world.hour).map_or(1.0, |c| 0.299 * c.r + 0.587 * c.g + 0.114 * c.b);
        let ts = self.ts();
        let at = self.player.pos - vec2(0.0, ts * 0.5);
        let lit: f32 = self
            .lights()
            .iter()
            .map(|l| {
                let r = l.spec.radius * ts;
                if r <= 0.0 {
                    return 0.0;
                }
                (1.0 - l.pos.distance(at) / r).max(0.0) * l.spec.intensity.clamp(0.0, 1.0)
            })
            .sum();
        (ambient + lit).clamp(0.0, 1.0)
    }

    /// How far (pixels) an enemy with `sight` tiles of sight notices the player: less while
    /// sneaking and in the dark, never under [`NEAR_NOTICE`] tiles.
    pub(super) fn player_sight(&self, sight: f32) -> f32 {
        // Invisible: only noticed up close.
        if crate::srd::conditions::has(&self.world.effects, crate::srd::Condition::Invisible) {
            return 0.0;
        }
        let light = DARK_SIGHT + (1.0 - DARK_SIGHT) * self.light_at_player();
        let sneak = if self.sneaking() { SNEAK_SIGHT } else { 1.0 };
        (sight * light * sneak).max(NEAR_NOTICE.min(sight)) * self.ts()
    }

    /// Whether character `i` hasn't noticed the player (an enemy that isn't hunting anyone).
    pub(super) fn unaware(&self, i: usize) -> bool {
        let e = &self.loaded.entities[i];
        matches!(e.behavior, super::Behavior::Chase { .. }) && e.ai.aggro <= 0.0 && !e.ai.fleeing
    }

    /// The damage of the player's hit `dmg` on character `i`, and whether it was a sneak attack.
    pub(super) fn sneak_attack(&self, i: usize, dmg: i32) -> (i32, bool) {
        if !self.unaware(i) {
            return (dmg, false);
        }
        (((dmg as f32) * SNEAK_ATTACK).round() as i32, true)
    }

    /// "Sneaking" under the HUD's panel while the player sneaks.
    pub(super) fn draw_sneak_hint(&self, x: f32, y: f32) {
        if self.sneaking() {
            draw_text("Sneaking", x, y, 18.0, Color::new(0.75, 0.85, 1.0, 0.9));
        }
    }

    /// `sneak=` for `status`: on / off, the light at the player and how far a 10-tile enemy
    /// would notice them (tiles).
    pub(super) fn sneak_status(&self) -> String {
        format!("{}:light{:.2}:sight10={:.1}", if self.sneaking() { "on" } else { "off" }, self.light_at_player(), self.player_sight(10.0) / self.ts())
    }
}
