//! Sneaking: the player toggles it (Act::Sneak: Left Ctrl / V, the left stick) and walks at half
//! speed (a model's "sneak" clip, else the walk slowed down). Starting to sneak rolls the hero's
//! Stealth once (the SRD's hiding). Enemies (`ai.rs`) notice the player within [`Game::player_sight`]
//! of their sight, less in the dark (the map's light by the clock and the light sources near the
//! player, the player's own lantern included); a sneaking hero only when their passive Perception
//! reaches the Stealth roll. Within [`NEAR_NOTICE`] they always do (bumping into them). A hit on an enemy that hasn't noticed the player is a sneak
//! attack: double damage, more with *Sneak attack* skills, shown like a critical hit.

use macroquad::prelude::*;

use super::input::{self, Act};
use super::text::draw_text;
use super::Game;

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
        if !key_consumed && input::act_pressed(Act::Sneak) && !self.player_stunned() {
            self.combat.sneak = !self.combat.sneak;
            self.combat.sneak_roll = 0;
            // (Hiding: one Stealth roll for as long as the sneaking lasts; a plain roll, no DC.)
            if self.combat.sneak {
                self.combat.sneak_roll = self.who_check(super::turns::Who::Player, "stealth", 0, false).map_or(0, |r| r.total as i32);
                if let Some(r) = self.world.rolls.last_mut() {
                    (r.success, r.dc) = (None, 0);
                }
            }
        }
    }

    /// Whether enemy `i`, with `sight` tiles of sight, notices the hero now: within its sight
    /// (shorter in the dark; behind walls and trees within half of it) and, while the hero
    /// sneaks, when its passive Perception reaches the hero's Stealth roll; always within
    /// [`NEAR_NOTICE`] tiles or its blindsight, tremorsense or truesight.
    pub(super) fn notices_hero(&self, i: usize, sight: f32) -> bool {
        let e = &self.loaded.entities[i];
        let ts = self.ts();
        let pos = e.body.pos;
        let d = self.player.pos.distance(pos);
        // Beyond its sight nothing notices (its senses reach no farther either): most of a big
        // map's creatures stop here, every frame, so the cheap answers come first.
        if d > sight * ts {
            return false;
        }
        if d <= NEAR_NOTICE.min(sight) * ts {
            return true;
        }
        let eye = pos - vec2(0.0, e.body.hitbox.y / 2.0 + 2.0);
        let mid = self.player.pos - vec2(0.0, self.player.hitbox.y / 2.0);
        let range = self.player_sight(sight);
        let in_sight = d <= range && (d <= range * 0.5 || self.line_clear(eye, mid));
        if in_sight && (!self.sneaking() || self.passive_of(super::turns::Who::Npc(e.uid)) >= self.combat.sneak_roll) {
            return true;
        }
        d <= self.sense_tiles(i).min(sight) * ts
    }

    /// How lit the player is: 0 (dark) to 1 (daylight, a lamp right beside them). The map's
    /// ambient light by the clock plus every light source in reach (as drawn: full at its
    /// middle, nothing at its radius).
    pub(super) fn light_at_player(&self) -> f32 {
        // Once a frame: every enemy with the hero in sight asks, and the answer adds up every
        // light source on the map.
        let key = (self.player.pos, self.world.hour);
        if let Some((_, light)) = self.light_here.get().filter(|(k, _)| *k == key) {
            return light;
        }
        let light = self.light_level(self.player.pos - vec2(0.0, self.ts() * 0.5));
        self.light_here.set(Some((key, light)));
        light
    }

    /// How lit a spot is: 0 (dark) to 1 (daylight): the map's ambient light by the clock plus
    /// every light source in reach.
    pub(super) fn light_level(&self, at: Vec2) -> f32 {
        let ambient = crate::render::lighting::ambient(self.loaded.map.lighting, self.world.hour).map_or(1.0, |c| 0.299 * c.r + 0.587 * c.g + 0.114 * c.b);
        let ts = self.ts();
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

    /// How far (pixels) an enemy with `sight` tiles of sight notices the player: less in the
    /// dark, never under [`NEAR_NOTICE`] tiles (sneaking adds the Stealth roll: [`Game::notices_hero`]).
    pub(super) fn player_sight(&self, sight: f32) -> f32 {
        // Invisible: only noticed up close.
        if crate::srd::conditions::has(&self.world.effects, crate::srd::Condition::Invisible) {
            return 0.0;
        }
        let light = DARK_SIGHT + (1.0 - DARK_SIGHT) * self.light_at_player();
        (sight * light).max(NEAR_NOTICE.min(sight)) * self.ts()
    }

    /// How far (tiles) character `i` notices the hero whatever the light, sneaking or
    /// invisibility: its blindsight, tremorsense or truesight (a bat's 60 ft. = 12 tiles).
    pub(super) fn sense_tiles(&self, i: usize) -> f32 {
        let who = super::turns::Who::Npc(self.loaded.entities[i].uid);
        let feet = [crate::srd::conditions::Sense::Blindsight, crate::srd::conditions::Sense::Tremorsense, crate::srd::conditions::Sense::Truesight].into_iter().map(|s| self.sense_of(who, s)).max().unwrap_or(0);
        if feet == u32::MAX {
            f32::MAX
        } else {
            feet as f32 / 5.0
        }
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

    /// `sneak=` for `status`: on / off, the light at the player, how far a 10-tile enemy would
    /// notice them (tiles) and, sneaking, the Stealth roll its passive Perception has to reach.
    pub(super) fn sneak_status(&self) -> String {
        // (As it is now: a script command may have changed a light since the frame.)
        self.light_here.set(None);
        let roll = if self.sneaking() { format!(":stealth={}", self.combat.sneak_roll) } else { String::new() };
        format!("{}:light{:.2}:sight10={:.1}{roll}", if self.sneaking() { "on" } else { "off" }, self.light_at_player(), self.player_sight(10.0) / self.ts())
    }
}
