//! *Input number* / *Input text*: a window that asks the player for a number or a word and puts
//! it into a variable (a text variable for words). The event waits until it is confirmed.
//! Numbers: typed digits, Up / Down (or the gamepad) change it by one, Left / Right by ten.
//! Words: typed letters (a keyboard; the window starts with the variable's value, so the gamepad
//! can confirm it as it is). Enter / E / A confirm; Backspace deletes.

use gfx::prelude::*;

use super::input::{self, Act, Pad};
use super::skin;
use super::text::{draw_plain, measure_plain, role, Role};
use super::Game;

/// What the window asks for.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Ask {
    Number { min: i64, max: i64, digits: u32 },
    Text { max_len: u32 },
}

pub(super) struct InputBox {
    ask: Ask,
    /// The prompt (translated, placeholders filled in).
    prompt: String,
    var: String,
    /// What is typed so far.
    value: String,
}

const W: f32 = 520.0;

impl Game {
    /// Opens the window for *Input number* / *Input text*.
    pub(super) fn open_input(&mut self, ask: Ask, prompt: &str, var: &str) {
        let value = match &ask {
            Ask::Number { min, max, .. } => self.world.var(var).clamp(*min, (*max).max(*min)).to_string(),
            Ask::Text { max_len } => self.world.text(var).chars().take(*max_len as usize).collect(),
        };
        // Keys typed before the window opened don't count.
        while input::next_char().is_some() {}
        self.input_box = Some(InputBox { ask, prompt: self.world.expand_text(prompt), var: var.to_string(), value });
    }

    pub(super) fn input_open(&self) -> bool {
        self.input_box.is_some()
    }

    /// The window's input; true while it is open (it pauses the game).
    pub(super) fn update_input_box(&mut self) -> bool {
        let Some(b) = &mut self.input_box else { return false };
        match b.ask.clone() {
            Ask::Number { min, max, digits } => {
                let max = max.max(min);
                let mut n: i64 = b.value.parse().unwrap_or(min);
                while let Some(c) = input::next_char() {
                    if c.is_ascii_digit() {
                        let mut s = if b.value == "0" { String::new() } else { b.value.clone() };
                        if digits == 0 || s.trim_start_matches('-').len() < digits as usize {
                            s.push(c);
                        }
                        n = s.parse().unwrap_or(n);
                        b.value = s;
                    } else if c == '-' && min < 0 && !b.value.starts_with('-') {
                        b.value = format!("-{}", b.value);
                        n = b.value.parse().unwrap_or(n);
                    }
                }
                let step = |n: i64, d: i64| (n + d).clamp(min, max);
                if input::act_pressed(Act::Up) {
                    n = step(n, 1);
                    b.value = n.to_string();
                } else if input::act_pressed(Act::Down) {
                    n = step(n, -1);
                    b.value = n.to_string();
                } else if input::act_pressed(Act::Right) {
                    n = step(n, 10);
                    b.value = n.to_string();
                } else if input::act_pressed(Act::Left) {
                    n = step(n, -10);
                    b.value = n.to_string();
                }
                if input::pressed(KeyCode::Backspace) {
                    b.value.pop();
                }
                if Self::confirm_pressed() {
                    let v = b.value.parse::<i64>().unwrap_or(min).clamp(min, max);
                    let var = b.var.clone();
                    self.world.set_var(&var, v);
                    self.input_box = None;
                }
            }
            Ask::Text { max_len } => {
                while let Some(c) = input::next_char() {
                    if !c.is_control() && b.value.chars().count() < max_len as usize {
                        b.value.push(c);
                    }
                }
                if input::pressed(KeyCode::Backspace) {
                    b.value.pop();
                }
                // (E types an "e" here: only Enter and the gamepad confirm.)
                if input::pressed(KeyCode::Enter) || input::pressed(KeyCode::KpEnter) || input::pad_pressed(Pad::South) {
                    let (var, v) = (b.var.clone(), b.value.trim().to_string());
                    self.world.texts.insert(var, v);
                    self.input_box = None;
                }
            }
        }
        true
    }

    fn confirm_pressed() -> bool {
        input::pressed(KeyCode::Enter) || input::pressed(KeyCode::KpEnter) || input::act_pressed(Act::Confirm)
    }

    /// The window.
    pub(super) fn draw_input_box(&self) {
        let Some(b) = &self.input_box else { return };
        let _font = role(Role::Dialog);
        let (sw, sh) = (screen_width(), screen_height());
        let lines = crate::render::wrap_text(&b.prompt, 22, W - 48.0);
        let h = 70.0 + lines.len() as f32 * 26.0 + 60.0;
        let p = Rect::new(((sw - W) / 2.0).round(), ((sh - h) / 2.0).round(), W, h);
        skin::dialog_box(p.x, p.y, p.w, p.h);
        let mut y = p.y + 34.0;
        for l in &lines {
            draw_plain(l, p.x + 24.0, y, 22.0, skin::normal());
            y += 26.0;
        }
        // The field, with a blinking caret.
        let field = Rect::new(p.x + 24.0, y, W - 48.0, 40.0);
        draw_rectangle(field.x, field.y, field.w, field.h, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_rectangle_lines(field.x, field.y, field.w, field.h, 2.0, skin::highlight());
        let caret = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
        let shown = format!("{}{caret}", b.value);
        let x = match b.ask {
            Ask::Number { .. } => field.x + (field.w - measure_plain(&shown, 28.0).width) / 2.0,
            Ask::Text { .. } => field.x + 12.0,
        };
        draw_plain(&shown, x, field.y + 29.0, 28.0, skin::highlight());
        let hint = match b.ask {
            Ask::Number { .. } => "Type a number · {key:up}/{key:down}: one more / less · Enter or {key:confirm}: done",
            Ask::Text { .. } => "Type · Backspace: delete · Enter: done",
        };
        super::glyphs::draw_hint_in(hint, p.x + 24.0, skin::hint_baseline(p.bottom()), 16.0, W - 48.0, skin::disabled());
    }

    /// For `status()`: `number:VAR=value` / `text:VAR="value"`, `-` when closed.
    pub(super) fn input_status(&self) -> String {
        match &self.input_box {
            None => "-".into(),
            Some(b) => match b.ask {
                Ask::Number { .. } => format!("number:{}={}", b.var, b.value),
                Ask::Text { .. } => format!("text:{}={:?}", b.var, b.value),
            },
        }
    }
}
