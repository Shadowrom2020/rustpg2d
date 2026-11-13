//! Barks: short lines in a speech bubble over a character's head ("Fresh bread!", "Cold
//! tonight...") that don't stop play, unlike dialogs. Said by the *Bark* action and `bark()`;
//! when, is up to the triggers (the hour, a zone near a stall, the weather). The bubble follows
//! the character, pops up and fades out; a character says one line at a time, and from a list of
//! lines never the one it said last. Lines are translated and expanded like dialog text. Not saved.

use std::collections::HashMap;

use macroquad::prelude::*;

use super::emotions::Who;
use super::text::{draw_text, measure_text};
use super::Game;

/// Characters farther from the player than this (tiles) don't bark.
const HEARD_TILES: f32 = 12.0;

/// The pop at the start and the fade at the end (seconds).
const POP: f32 = 0.15;
const FADE: f32 = 0.4;

/// A line being said.
#[derive(Clone, Debug)]
pub(super) struct Bark {
    pub text: String,
    t: f32,
    seconds: f32,
}

impl Bark {
    fn alpha(&self) -> f32 {
        ((self.seconds - self.t) / FADE).clamp(0.0, 1.0)
    }

    fn scale(&self) -> f32 {
        if self.t >= POP {
            1.0
        } else {
            0.6 + 0.4 * self.t / POP
        }
    }
}

/// How long a line stays: `seconds`, or by its length (2 to 7 s).
pub(super) fn bark_seconds(text: &str, seconds: f32) -> f32 {
    if seconds > 0.0 {
        seconds
    } else {
        (1.6 + 0.06 * text.chars().count() as f32).clamp(2.0, 7.0)
    }
}

/// One of `lines` (empty ones don't count), the `r`-th of those that aren't `last` (all of
/// them when every line is `last`).
pub(super) fn pick_line<'a>(lines: &'a [String], last: Option<&str>, r: usize) -> Option<&'a str> {
    let lines: Vec<&str> = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let fresh: Vec<&str> = lines.iter().copied().filter(|l| Some(*l) != last).collect();
    let pool = if fresh.is_empty() { &lines } else { &fresh };
    (!pool.is_empty()).then(|| pool[r % pool.len()])
}

/// Words of `text` in lines no wider than `max` (pixels at `size`); a word longer than that
/// gets a line of its own.
fn wrap(text: &str, size: f32, max: f32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match out.last_mut() {
            Some(line) if measure_text(&format!("{line} {word}"), None, size as u16, 1.0).width <= max => {
                line.push(' ');
                line.push_str(word);
            }
            _ => out.push(word.to_string()),
        }
    }
    out
}

/// Who said what last (uid; the player is `u32::MAX`), so a list of lines doesn't repeat.
pub(super) type LastBarks = HashMap<u32, String>;

impl Game {
    fn bark_key(&self, who: Who) -> u32 {
        match who {
            Who::Player => u32::MAX,
            Who::Entity(i) => self.loaded.entities[i].uid,
        }
    }

    /// `who` says one of `lines` (see [`pick_line`]); false without a line to say, or when the
    /// character is too far from the player to be seen (a bark is for passing by).
    pub(super) fn start_bark(&mut self, who: Who, lines: &[String], seconds: f32) -> bool {
        if let Who::Entity(i) = who {
            if self.loaded.entities[i].body.pos.distance(self.player.pos) > HEARD_TILES * self.ts() {
                return false;
            }
        }
        let key = self.bark_key(who);
        let Some(raw) = pick_line(lines, self.last_barks.get(&key).map(String::as_str), rand::gen_range(0, 1000)).map(str::to_string) else { return false };
        let text = self.world.expand_text(&raw);
        let b = Bark { seconds: bark_seconds(&text, seconds), text, t: 0.0 };
        match who {
            Who::Player => self.player_bark = Some(b),
            Who::Entity(i) => self.loaded.entities[i].bark = Some(Box::new(b)),
        }
        self.last_barks.insert(key, raw);
        true
    }

    pub(super) fn update_barks(&mut self, dt: f32) {
        let tick = |b: &mut Bark| {
            b.t += dt;
            b.t < b.seconds
        };
        if self.player_bark.as_mut().is_some_and(|b| !tick(b)) {
            self.player_bark = None;
        }
        for e in &mut self.loaded.entities {
            if e.bark.as_mut().is_some_and(|b| !tick(b)) {
                e.bark = None;
            }
        }
    }

    /// The speech bubbles (screen space, over the world; `cam` = the world's camera).
    pub(super) fn draw_barks(&self, cam: &Camera2D) {
        let ts = self.ts();
        // Screen pixels per world pixel.
        let scale = (cam.world_to_screen(vec2(1.0, 0.0)) - cam.world_to_screen(Vec2::ZERO)).x.abs().max(0.1);
        let draw = |b: &Bark, feet: Vec2, height: f32, emote: bool| {
            let alpha = b.alpha();
            let size = (0.36 * ts * scale * b.scale()).round().max(10.0);
            let lines = wrap(&b.text, size, 5.5 * ts * scale);
            let widths: Vec<f32> = lines.iter().map(|l| measure_text(l, None, size as u16, 1.0).width).collect();
            let w = widths.iter().copied().fold(0.0, f32::max);
            let (pad, line_h) = ((size * 0.35).round(), (size * 0.95).round());
            let (bw, bh) = (w + pad * 2.0, line_h * lines.len() as f32 + pad * 1.4);
            // Above the head, and above an emotion icon when one shows.
            let head = cam.world_to_screen(feet - vec2(0.0, height + if emote { 22.0 } else { 6.0 } * ts / 32.0));
            let tail = (size * 0.4).round();
            let (x, y) = ((head.x - bw / 2.0).round(), (head.y - tail - bh).round());
            let paper = Color::new(0.98, 0.95, 0.86, 0.95 * alpha);
            let ink = Color::new(0.22, 0.15, 0.1, alpha);
            draw_rectangle(x - 1.0, y - 1.0, bw + 2.0, bh + 2.0, ink);
            draw_rectangle(x, y, bw, bh, paper);
            draw_triangle(vec2(head.x - tail, y + bh), vec2(head.x + tail, y + bh), vec2(head.x, y + bh + tail), paper);
            draw_line(head.x - tail, y + bh, head.x, y + bh + tail, 1.0, ink);
            draw_line(head.x + tail, y + bh, head.x, y + bh + tail, 1.0, ink);
            for (k, (line, lw)) in lines.iter().zip(&widths).enumerate() {
                draw_text(line, (head.x - lw / 2.0).round(), (y + pad * 0.7 + line_h * (k as f32 + 0.8)).round(), size, ink);
            }
        };
        if let Some(b) = &self.player_bark {
            let h = self.player_appearance().map_or(48.0, |a| self.character_frame_size(&a).1);
            draw(b, self.player.pos, h, self.player_emote.is_some());
        }
        for e in self.loaded.entities.iter().filter(|e| e.active) {
            if let Some(b) = &e.bark {
                let h = if !e.appearance.has_look() { 32.0 } else { self.character_frame_size(&e.appearance).1 };
                draw(b, e.body.pos, h, e.emote.is_some());
            }
        }
    }

    /// `barks=` for `status`: who:text.
    pub(super) fn barks_status(&self) -> Vec<String> {
        let mut out: Vec<String> = self.player_bark.iter().map(|b| format!("player:{}", b.text)).collect();
        out.extend(self.loaded.entities.iter().filter_map(|e| e.bark.as_ref().map(|b| format!("{}:{}", if e.name.is_empty() { &e.def } else { &e.name }, b.text))));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barks_pick_another_line_and_last_by_their_length() {
        let lines: Vec<String> = ["Fresh bread!", "", "Warm rolls!"].map(String::from).to_vec();
        for r in 0..6 {
            assert_eq!(pick_line(&lines, Some("Fresh bread!"), r), Some("Warm rolls!"));
        }
        assert_eq!(pick_line(&lines[..1], Some("Fresh bread!"), 3), Some("Fresh bread!"), "a single line repeats");
        assert_eq!(pick_line(&[String::new()], None, 0), None);
        assert_eq!(bark_seconds("Hi", 0.0), 2.0);
        assert_eq!(bark_seconds(&"a".repeat(500), 0.0), 7.0);
        assert_eq!(bark_seconds("Hi", 4.5), 4.5);
    }
}
