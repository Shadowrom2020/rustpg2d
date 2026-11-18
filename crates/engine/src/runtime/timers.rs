//! Timers (`crate::timers`): counted down while no menu or dialog is open, drawn at the top of
//! the screen when shown, and what they run when they run out.

use macroquad::prelude::*;

use super::skin;
use super::text::{draw_plain, measure_plain, role, Role};
use super::Game;
use crate::events::runner::ActionCtx;
use crate::events::Action;

impl Game {
    /// *Start timer*.
    pub(super) fn start_timer(&mut self, name: &str, seconds: f32, shown: bool, function: &str, event: &str) {
        self.world.timers.insert(name.to_string(), crate::timers::Timer::new(seconds, shown, function, event));
    }

    /// Counts the timers down (not while a dialog is open: the menus pause the whole update) and
    /// queues what the ones that ran out run.
    pub(super) fn update_timers(&mut self, dt: f32) {
        if self.world.timers.is_empty() || self.ui.dialog_open() {
            return;
        }
        for (name, t) in crate::timers::tick(&mut self.world.timers, dt) {
            let mut run = Vec::new();
            if !t.function.is_empty() {
                run.push(Action::RunScript { function: t.function.clone() });
            }
            if !t.event.is_empty() {
                run.push(Action::Emit { name: t.event.clone() });
            }
            if run.is_empty() {
                eprintln!("[timers] timer '{name}' ran out (it runs nothing)");
            }
            self.runner.push(&run, &ActionCtx::default());
        }
    }

    /// The shown timers, at the top middle of the screen (with the HUD).
    pub(super) fn draw_timers(&self) {
        let shown: Vec<&crate::timers::Timer> = self.world.timers.values().filter(|t| t.shown).collect();
        if shown.is_empty() {
            return;
        }
        let _font = role(Role::Hud);
        let size = 26.0;
        let mut y = 12.0;
        for t in shown {
            let text = crate::timers::clock(t.left);
            let w = measure_plain(&text, size).width + 28.0;
            let r = Rect::new(((screen_width() - w) / 2.0).round(), y, w.round(), 38.0);
            skin::toast_box(r, 1.0);
            // The last ten seconds in the skin's "bad news" colour.
            let col = if t.left <= 10.0 { skin::text(skin::Text::Negative, Color::new(1.0, 0.45, 0.4, 1.0)) } else { skin::toast_text(1.0) };
            draw_plain(&text, r.x + 14.0, r.y + 28.0, size, col);
            y += 44.0;
        }
    }

    /// For `status()`: `name:seconds` (`+shown`).
    pub(super) fn timers_status(&self) -> Vec<String> {
        self.world.timers.iter().map(|(n, t)| format!("{n}:{:.1}{}", t.left, if t.shown { "+shown" } else { "" })).collect()
    }
}
