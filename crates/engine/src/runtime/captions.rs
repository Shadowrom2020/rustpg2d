//! Captions (Options › Accessibility › Captions): every sound effect that has a caption
//! (`SoundDef::caption`, translated like dialogs) shows it at the bottom left for a few seconds
//! while it plays, a `<` / `>` marking the side it came from when it was in the world more than
//! a few tiles away; a repeated one counts up ("Hit (x3)"). A new music track shows its caption
//! as "Music: …". The audio notes every sound started (`Audio::take_heard`); world sounds tell
//! where they came from with [`Game::sound_from`].

use std::collections::HashMap;

use gfx::prelude::*;

use super::text::{draw_text, measure_text};
use super::Game;

/// Seconds a caption shows (a repeat starts it over).
const SECS: f32 = 3.0;
/// At most this many lines (the oldest goes).
const LINES: usize = 5;
/// Farther than this (tiles) to the side, a sound's caption names the side.
const SIDE: f32 = 3.0;

#[derive(Default)]
pub(super) struct Captions {
    lines: Vec<Line>,
    /// Where sounds started this frame came from (-1 left, 0 near, 1 right).
    sides: HashMap<String, i8>,
    /// The music playing when last looked (a new track gets a caption).
    music: Option<String>,
}

struct Line {
    text: String,
    side: i8,
    count: u32,
    t: f32,
}

impl Game {
    /// Plays sound effect `id` heard from `from` (world pixels): its caption names the side.
    pub(super) fn sound_from(&mut self, id: &str, from: Vec2) {
        let dx = (from.x - self.player.pos.x) / self.ts();
        let side = if dx < -SIDE { -1 } else if dx > SIDE { 1 } else { 0 };
        self.captions.sides.insert(id.to_string(), side);
        self.audio.play(id);
    }

    /// Turns the sounds started since the last frame into captions and ages the shown ones.
    pub(super) fn update_captions(&mut self, dt: f32) {
        let heard = self.audio.take_heard();
        let sides = std::mem::take(&mut self.captions.sides);
        let music = self.audio.current_music().map(str::to_string);
        let new_track = music != self.captions.music;
        self.captions.music = music.clone();
        let c = &mut self.captions;
        c.lines.iter_mut().for_each(|l| l.t -= dt);
        c.lines.retain(|l| l.t > 0.0);
        if !self.settings.captions {
            c.lines.clear();
            return;
        }
        let mut add = |text: String, side: i8, secs: f32| {
            match c.lines.iter_mut().find(|l| l.text == text && l.side == side) {
                Some(l) => {
                    l.count += 1;
                    l.t = secs;
                }
                None => {
                    if c.lines.len() >= LINES {
                        c.lines.remove(0);
                    }
                    c.lines.push(Line { text, side, count: 1, t: secs });
                }
            }
        };
        for id in heard {
            if let Some(cap) = self.project.db.sound(&id).map(|d| d.caption.trim()).filter(|t| !t.is_empty()) {
                add(cap.to_string(), sides.get(&id).copied().unwrap_or(0), SECS);
            }
        }
        if let Some(cap) = music.filter(|_| new_track).and_then(|id| self.project.db.sound(&id)).map(|d| d.caption.trim()).filter(|t| !t.is_empty()) {
            add(format!("Music: {cap}"), 0, SECS + 1.0);
        }
    }

    /// The captions at the bottom left (over the world, under menus); while a window is open
    /// (its details line is down there) at the top right, under the clock.
    pub(super) fn draw_captions(&self) {
        let _font = super::text::role(super::text::Role::Dialog);
        if self.captions.lines.is_empty() {
            return;
        }
        let s = self.settings.text_scale();
        let size = (20.0 * s).round();
        let (row, pad) = ((26.0 * s).round(), 8.0);
        let contrast = super::ui::contrast();
        let window = self.inventory_open();
        let mut y = if window { 48.0 + row } else { screen_height() - 72.0 };
        let lines: Vec<&Line> = if window { self.captions.lines.iter().collect() } else { self.captions.lines.iter().rev().collect() };
        for l in lines {
            let text = match (l.side, l.count) {
                (-1, 1) => format!("< {}", l.text),
                (1, 1) => format!("{} >", l.text),
                (-1, n) => format!("< {} (x{n})", l.text),
                (1, n) => format!("{} (x{n}) >", l.text),
                (_, 1) => l.text.clone(),
                (_, n) => format!("{} (x{n})", l.text),
            };
            let a = (l.t / 0.4).min(1.0);
            let w = measure_text(&text, None, size as u16, 1.0).width + pad * 2.0;
            let (x, top) = (if window { screen_width() - w - 16.0 } else { 16.0 }, y - row);
            if contrast {
                draw_rectangle(x, top, w, row, Color::new(0.0, 0.0, 0.0, a));
                draw_rectangle_lines(x, top, w, row, 2.0, Color::new(1.0, 0.85, 0.2, a));
            } else {
                draw_rectangle(x, top, w, row, Color::new(0.0, 0.0, 0.0, 0.6 * a));
            }
            draw_text(&text, x + pad, top + row * 0.72, size, Color::new(1.0, 1.0, 1.0, a));
            y += if window { row + 4.0 } else { -(row + 4.0) };
        }
    }

    /// `captions=` for `status`: the captions showing.
    pub(super) fn captions_status(&self) -> Vec<String> {
        self.captions.lines.iter().map(|l| format!("{}{}{}", if l.side < 0 { "<" } else { "" }, l.text, if l.side > 0 { ">" } else { "" })).collect()
    }
}
