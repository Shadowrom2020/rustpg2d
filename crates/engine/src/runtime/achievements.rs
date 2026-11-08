//! Unlocking achievements while playing: their conditions are checked twice a second (not on
//! the title screen), the *Unlock achievement* action / `unlock_achievement(id)` unlocks one
//! directly. An unlock is saved right away (the player's file, not the save game) and shown in
//! a toast of its own, drawn over the pause menu too.

use macroquad::prelude::*;

use super::Game;
use crate::events::runner::{conditions_met, ActionCtx};
use super::text::{draw_text};

/// Seconds between two checks of the conditions.
const CHECK_EVERY: f32 = 0.5;
/// Seconds an "Achievement unlocked" toast stays.
const TOAST_TIME: f32 = 4.0;

#[derive(Default)]
pub(super) struct Achievements {
    /// The player's unlocked achievements (loaded at start).
    pub progress: crate::achievements::Progress,
    timer: f32,
    /// Toasts: (achievement id, seconds shown).
    toasts: Vec<(String, f32)>,
}

impl Game {
    /// Loads the player's achievements of this game and tells the scripts.
    pub(super) fn load_achievements(&mut self) {
        self.ach.progress = crate::achievements::Progress::load(&self.project.config.name);
        self.sync_script_achievements();
    }

    fn sync_script_achievements(&mut self) {
        self.scripts.set_achievements(self.ach.progress.unlocked.keys().cloned().collect());
    }

    /// Checks the locked achievements' conditions (every half second) and ages the toasts.
    pub(super) fn update_achievements(&mut self, dt: f32) {
        for t in &mut self.ach.toasts {
            t.1 += dt;
        }
        self.ach.toasts.retain(|t| t.1 < TOAST_TIME);
        if self.on_title() || self.game_over.is_some() || self.project.db.achievements.is_empty() {
            return;
        }
        self.ach.timer -= dt;
        if self.ach.timer > 0.0 {
            return;
        }
        self.ach.timer = CHECK_EVERY;
        let due: Vec<(String, Vec<crate::events::Condition>)> = self
            .project
            .db
            .achievements
            .iter()
            .filter(|d| !d.conditions.is_empty() && !self.ach.progress.has(&d.id))
            .map(|d| (d.id.clone(), d.conditions.clone()))
            .collect();
        for (id, conds) in due {
            if conditions_met(self, &conds, &ActionCtx::default()) {
                self.unlock_achievement(&id);
            }
        }
    }

    /// Unlocks an achievement (no-op if it already is): saved at once, toast and sound.
    pub(super) fn unlock_achievement(&mut self, id: &str) {
        if self.project.db.achievement(id).is_none() {
            eprintln!("[game] Unlock achievement: there is no achievement '{id}'");
            return;
        }
        if !self.ach.progress.unlock(id, super::menu::now_secs()) {
            return;
        }
        if let Err(e) = self.ach.progress.save(&self.project.config.name) {
            eprintln!("[game] saving achievements: {e:#}");
        }
        self.sync_script_achievements();
        self.audio.play(&self.project.config.sfx.pickup);
        self.ach.toasts.push((id.to_string(), 0.0));
    }

    /// The menu's *Reset*: forgets every unlock.
    pub(super) fn reset_achievements(&mut self) {
        self.ach.progress.reset(&self.project.config.name);
        self.sync_script_achievements();
    }

    /// For `status()`: the unlocked achievements' ids.
    pub(super) fn achievements_status(&self) -> Vec<String> {
        self.ach.progress.unlocked.keys().cloned().collect()
    }

    /// "Achievement unlocked" toasts at the top of the screen (above the menu).
    pub(super) fn draw_achievement_toasts(&self) {
        let (w, h) = (380.0, 64.0);
        let x = ((screen_width() - w) / 2.0).round();
        for (k, (id, t)) in self.ach.toasts.iter().enumerate() {
            let Some(d) = self.project.db.achievement(id) else { continue };
            // Slides down, stays, fades.
            let slide = (t / 0.25).min(1.0);
            let alpha = ((TOAST_TIME - t) / 0.5).clamp(0.0, 1.0);
            let y = (-h + slide * (h + 16.0) + k as f32 * (h + 8.0)).round();
            draw_rectangle(x, y, w, h, Color::new(0.08, 0.07, 0.04, 0.92 * alpha));
            draw_rectangle_lines(x, y, w, h, 2.0, Color::new(0.95, 0.78, 0.3, alpha));
            if !d.icon.sheet.is_empty() {
                self.assets.draw_frame_in(&d.icon.sheet, d.icon.frame, Rect::new(x + 8.0, y + 8.0, 48.0, 48.0), Color::new(1.0, 1.0, 1.0, alpha));
            }
            draw_text("Achievement unlocked", x + 66.0, y + 24.0, 18.0, Color::new(0.85, 0.8, 0.6, alpha));
            draw_text(&d.name, x + 66.0, y + 48.0, 26.0, Color::new(1.0, 0.85, 0.35, alpha));
        }
    }
}
