//! Status effects at runtime: applying and curing them on the player and on characters,
//! ticking (poison, burning and regeneration change HP), their influence on speed, damage and
//! defense, stun, and drawing (tints, the HUD badges, small markers over characters). The
//! effect kinds and the math are in [`crate::effects`].

use macroquad::prelude::*;

use super::Game;
use crate::effects::{self, ActiveEffect, EffectKind, EffectSpec};
use super::text::{draw_text, measure_text};

impl Game {
    /// Starts an effect on `target`: "player", "self" (the trigger's owner) or a character's name.
    pub(super) fn apply_effect(&mut self, target: &str, owner: Option<u32>, spec: &EffectSpec) {
        if target == "player" {
            self.apply_player_effect(spec);
        } else if let Some(i) = self.entity_index(target, owner) {
            effects::apply(&mut self.loaded.entities[i].effects, spec);
        } else {
            eprintln!("[events] Apply effect: no character '{target}' on this map");
        }
    }

    pub(super) fn apply_player_effect(&mut self, spec: &EffectSpec) {
        if self.game_over.is_none() {
            effects::apply(&mut self.world.effects, spec);
        }
    }

    /// Ends an effect (`None` = all) on `target`.
    pub(super) fn cure_effect(&mut self, target: &str, owner: Option<u32>, kind: Option<EffectKind>) {
        if target == "player" {
            effects::cure(&mut self.world.effects, kind);
        } else if let Some(i) = self.entity_index(target, owner) {
            effects::cure(&mut self.loaded.entities[i].effects, kind);
        } else {
            eprintln!("[events] Cure effect: no character '{target}' on this map");
        }
    }

    /// Once per frame (not while the game is frozen): effects run down; poison and burning
    /// hurt, regeneration heals.
    pub(super) fn tick_effects(&mut self, dt: f32) {
        if !self.world.effects.is_empty() {
            let hp = effects::tick(&mut self.world.effects, dt);
            if hp < 0 {
                self.effect_damage(-hp);
            } else if hp > 0 && self.combat.enabled() {
                let max = self.player_max_hp();
                self.world.player_hp = Some((self.hp() + hp).min(max));
            }
        }
        for i in 0..self.loaded.entities.len() {
            let e = &mut self.loaded.entities[i];
            if e.effects.is_empty() {
                continue;
            }
            let hp = effects::tick(&mut e.effects, dt);
            if !e.active || e.max_hp <= 0 || hp == 0 {
                continue;
            }
            if hp > 0 {
                e.hp = (e.hp + hp).min(e.max_hp);
            } else {
                let before = e.hp;
                e.hp += hp;
                e.flash = 0.15;
                if e.hp <= 0 {
                    self.defeat(i);
                } else {
                    self.hp_dropped(i, before);
                }
            }
        }
    }

    /// Damage from poison and burning: no defense, no knockback, no invulnerability.
    fn effect_damage(&mut self, n: i32) {
        if !self.combat.enabled() || self.game_over.is_some() {
            return;
        }
        let hp = (self.hp() - n).max(0);
        self.world.player_hp = Some(hp);
        if hp == 0 {
            self.player_defeated();
        }
    }

    pub(super) fn player_stunned(&self) -> bool {
        effects::stunned(&self.world.effects)
    }

    /// Movement speed factor of character `i` (0 = stunned).
    pub(super) fn npc_speed_factor(&self, i: usize) -> f32 {
        let fx = &self.loaded.entities[i].effects;
        if effects::stunned(fx) {
            0.0
        } else {
            effects::speed_factor(fx)
        }
    }

    /// The effect a hostile character's touch gives: its definition's, or its melee weapon's.
    pub(super) fn touch_effect(&self, i: usize) -> Option<EffectSpec> {
        let e = &self.loaded.entities[i];
        self.project
            .db
            .character(&e.def)
            .and_then(|d| d.touch_effect)
            .or_else(|| e.weapon.as_ref().filter(|w| w.ranged.is_none()).and_then(|w| w.effect))
    }

    /// The tint of someone with these effects (None = normal colors).
    pub(super) fn effect_tint(list: &[ActiveEffect]) -> Option<Color> {
        let k = [EffectKind::Stun, EffectKind::Burn, EffectKind::Poison].into_iter().find(|k| list.iter().any(|e| e.kind == *k))?;
        Some(match k {
            EffectKind::Poison => Color::new(0.6, 1.0, 0.55, 1.0),
            EffectKind::Burn => Color::new(1.0, 0.7, 0.45, 1.0),
            _ => Color::new(0.7, 0.7, 0.75, 1.0),
        })
    }

    /// The player's effects as a row of badges (label and seconds left) from `(x, y)`; returns
    /// the row's height (0 = no effects).
    pub(super) fn draw_effect_badges(&self, x: f32, y: f32) -> f32 {
        let mut list: Vec<&ActiveEffect> = self.world.effects.iter().collect();
        list.sort_by_key(|e| (!e.kind.harmful(), e.kind));
        let mut bx = x;
        for e in &list {
            let text = badge_text(e);
            let w = measure_text(&text, None, 14, 1.0).width + 8.0;
            let [r, g, b] = e.kind.color();
            let c = Color::from_rgba(r, g, b, 255);
            draw_rectangle(bx, y, w, 16.0, Color::new(c.r * 0.45, c.g * 0.45, c.b * 0.45, 0.95));
            draw_rectangle_lines(bx, y, w, 16.0, 1.0, c);
            draw_text(&text, bx + 4.0, y + 12.0, 14.0, WHITE);
            bx += w + 4.0;
        }
        if list.is_empty() {
            0.0
        } else {
            20.0
        }
    }

    /// Width of the badge row (for the status frame's size).
    pub(super) fn effect_badges_width(&self) -> f32 {
        self.world.effects.iter().map(|e| measure_text(&badge_text(e)[..], None, 14, 1.0).width + 12.0).sum()
    }

    /// Small colored squares above a character with effects (world space).
    pub(super) fn draw_npc_effects(&self, center_x: f32, bottom_y: f32, list: &[ActiveEffect]) {
        let u = self.u();
        let s = 3.0 * u;
        let total = list.len() as f32 * (s + u) - u;
        for (k, e) in list.iter().enumerate() {
            let [r, g, b] = e.kind.color();
            let x = center_x - total / 2.0 + k as f32 * (s + u);
            draw_rectangle(x - 0.5 * u, bottom_y - s - 0.5 * u, s + u, s + u, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_rectangle(x, bottom_y - s, s, s, Color::from_rgba(r, g, b, 255));
        }
    }

    /// For `status()`: the player's effects and the characters with any.
    pub(super) fn effects_status(&self) -> String {
        let npcs: Vec<String> = self
            .loaded
            .entities
            .iter()
            .filter(|e| e.active && !e.effects.is_empty())
            .map(|e| format!("{}:{}", if e.name.is_empty() { &e.def } else { &e.name }, e.effects.iter().map(|f| f.kind.id()).collect::<Vec<_>>().join("+")))
            .collect();
        format!("effects={:?} npc_effects={:?}", effects::describe(&self.world.effects), npcs)
    }
}

/// A badge's text: the effect's short name and the seconds left.
fn badge_text(e: &ActiveEffect) -> String {
    format!("{} {}", e.kind.badge(), e.remaining.ceil() as i32)
}
