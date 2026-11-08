//! In-game UI: dialog box, choices and toast messages (inventories live in `inventory_ui`).

use macroquad::prelude::*;

use super::input;
use crate::render::wrap_text;
use crate::state::WorldState;
use super::text::{draw_text, measure_text};

struct Dialog {
    /// Room for the speaker's portrait at the left (the game draws it into `portrait_rect`).
    portrait: bool,
    speaker: String,
    text: String,
    revealed: f32,
    choice: Option<(Vec<String>, String, usize)>,
}

#[derive(Default)]
pub struct Ui {
    dialog: Option<Dialog>,
    toasts: Vec<(String, f32)>,
    /// Choice option areas, recorded while drawing the previous frame.
    choice_rects: Vec<Rect>,
    /// Where the open dialog's portrait goes (set while drawing).
    pub portrait_rect: Option<Rect>,
    /// Dialog text speed from the player's options (set every frame; 0 = instant).
    pub text_speed: f32,
}

pub struct UiInput {
    /// A key press was used by the UI (don't also treat it as gameplay input).
    pub consumed: bool,
    /// A mouse click was used by the UI.
    pub mouse_consumed: bool,
}

const CHARS_PER_SEC: f32 = 60.0;
const FONT: u16 = 26;

fn confirm_pressed() -> bool {
    input::act_pressed(input::Act::Confirm)
}

impl Ui {
    pub fn toast(&mut self, msg: String) {
        self.toasts.push((msg, 2.5));
    }

    pub fn show_dialog(&mut self, speaker: &str, text: &str) {
        self.dialog = Some(Dialog { portrait: false, speaker: speaker.into(), text: text.into(), revealed: 0.0, choice: None });
    }

    /// The open dialog gets a portrait (call right after `show_dialog` / `show_choice`).
    pub fn with_portrait(&mut self) {
        if let Some(d) = &mut self.dialog {
            d.portrait = true;
        }
    }

    pub fn show_choice(&mut self, speaker: &str, text: &str, options: &[String], var: &str) {
        self.dialog = Some(Dialog {
            portrait: false,
            speaker: speaker.into(),
            text: text.into(),
            revealed: 0.0,
            choice: Some((options.to_vec(), var.to_string(), 0)),
        });
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn update(&mut self, dt: f32, world: &mut WorldState) -> UiInput {
        for t in &mut self.toasts {
            t.1 -= dt;
        }
        self.toasts.retain(|t| t.1 > 0.0);
        let mut out = UiInput { consumed: false, mouse_consumed: false };
        let mouse = input::mouse_pos();
        let click = input::mouse_pressed(MouseButton::Left);

        if let Some(d) = &mut self.dialog {
            out.consumed = true;
            out.mouse_consumed = true;
            let len = d.text.chars().count() as f32;
            d.revealed = if self.text_speed <= 0.0 { len } else { (d.revealed + dt * CHARS_PER_SEC * self.text_speed).min(len) };
            if let Some((opts, _, sel)) = &mut d.choice {
                let n = opts.len().max(1);
                if input::act_pressed(input::Act::Up) {
                    *sel = (*sel + n - 1) % n;
                }
                if input::act_pressed(input::Act::Down) {
                    *sel = (*sel + 1) % n;
                }
            }
            let done = d.revealed >= len;
            let mut confirm = confirm_pressed();
            if let Some((_, _, sel)) = &mut d.choice {
                if let Some(k) = self.choice_rects.iter().position(|r| r.contains(mouse)) {
                    if done {
                        *sel = k; // hover selects
                    }
                    confirm |= click;
                }
            } else {
                confirm |= click;
            }
            if confirm {
                if d.revealed < len {
                    d.revealed = len;
                } else {
                    if let Some((opts, var, sel)) = &d.choice {
                        world.set_var(var, *sel as i64);
                        // The answer goes into the dialog log under the player's name.
                        let me = if world.player_name.trim().is_empty() { "You".to_string() } else { world.player_name.clone() };
                        if let Some(o) = opts.get(*sel) {
                            world.log_dialog(&me, o, true);
                        }
                    }
                    self.dialog = None;
                }
            }
            return out;
        }

        out
    }

    pub fn draw_toasts(&self) {
        let sw = screen_width();
        for (i, (msg, t)) in self.toasts.iter().enumerate() {
            let a = t.min(1.0);
            let w = measure_text(msg, None, FONT, 1.0).width + 24.0;
            let y = 16.0 + i as f32 * 40.0;
            draw_rectangle((sw - w) / 2.0, y, w, 34.0, Color::new(0.05, 0.05, 0.1, 0.8 * a));
            draw_text(msg, (sw - w) / 2.0 + 12.0, y + 24.0, FONT as f32, Color::new(1., 1., 1., a));
        }
    }

    pub fn draw_dialog(&mut self) {
        let (sw, sh) = (screen_width(), screen_height());
        self.choice_rects.clear();
        self.portrait_rect = None;

        if let Some(d) = &self.dialog {
            let margin = 16.0;
            let w = (sw - margin * 2.0).min(900.0);
            let x = (sw - w) / 2.0;
            // A portrait takes a square at the left; the text moves right of it.
            const PORTRAIT: f32 = 120.0;
            let inset = if d.portrait { PORTRAIT + 18.0 } else { 0.0 };
            let choice_rows = d.choice.as_ref().map_or(0, |c| c.0.len()) as f32;
            let lines = wrap_text(&d.text, FONT, w - 40.0 - inset);
            let mut h = 56.0 + lines.len().max(2) as f32 * 30.0 + choice_rows * 30.0;
            if d.portrait {
                h = h.max(PORTRAIT + 28.0);
            }
            let y = sh - h - margin;
            panel(x, y, w, h);
            if d.portrait {
                self.portrait_rect = Some(Rect::new(x + 14.0, y + 14.0, PORTRAIT, PORTRAIT));
            }
            let x = x + inset;
            let w = w - inset;
            let mut ty = y + 34.0;
            if !d.speaker.is_empty() {
                // (Dialog content is translated already: macroquad's draw_text, no UI lookup.)
                macroquad::text::draw_text(&d.speaker, x + 20.0, ty, 28.0, GOLD);
                ty += 32.0;
            }
            let mut budget = d.revealed as usize;
            for line in &lines {
                let shown: String = line.chars().take(budget).collect();
                budget = budget.saturating_sub(line.chars().count() + 1);
                macroquad::text::draw_text(&shown, x + 20.0, ty, FONT as f32, WHITE);
                ty += 30.0;
            }
            let done = d.revealed as usize >= d.text.chars().count();
            if let (Some((opts, _, sel)), true) = (&d.choice, done) {
                for (i, o) in opts.iter().enumerate() {
                    self.choice_rects.push(Rect::new(x + 20.0, ty - 22.0, w - 40.0, 30.0));
                    let c = if i == *sel { GOLD } else { LIGHTGRAY };
                    let marker = if i == *sel { ">" } else { " " };
                    macroquad::text::draw_text(format!("{marker} {o}"), x + 36.0, ty, FONT as f32, c);
                    ty += 30.0;
                }
            } else if done && (get_time() * 2.0) as i64 % 2 == 0 {
                let (cx, cy) = (x + w - 28.0, y + h - 20.0);
                draw_triangle(vec2(cx - 7.0, cy - 5.0), vec2(cx + 7.0, cy - 5.0), vec2(cx, cy + 4.0), WHITE);
            }
        }
    }
}

pub(super) fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.06, 0.06, 0.12, 0.92));
    draw_rectangle_lines(x, y, w, h, 3.0, Color::new(0.85, 0.8, 0.6, 1.0));
}
