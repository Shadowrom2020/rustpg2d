//! *Scroll text*: credits or a story intro rolling up the screen over a background colour. The
//! event waits until the last line has gone, or the player skips it (confirm / cancel). Lines
//! starting with `# ` are headings. It runs over the title screen too (the Credits of a custom
//! title entry).

use gfx::prelude::*;

use super::input::{self, Act};
use super::skin;
use super::text::{draw_plain, measure_plain, role, Role};
use super::Game;

pub(super) struct ScrollText {
    /// (line, heading), wrapped.
    lines: Vec<(String, bool)>,
    /// Pixels per second.
    speed: f32,
    bg: Color,
    /// How far it has rolled (pixels).
    y: f32,
}

const SIZE: f32 = 24.0;
const LINE: f32 = 32.0;
const HEAD_SIZE: f32 = 32.0;
const HEAD_LINE: f32 = 48.0;

impl ScrollText {
    fn height(&self) -> f32 {
        self.lines.iter().map(|(_, h)| if *h { HEAD_LINE } else { LINE }).sum()
    }
}

impl Game {
    /// Opens it (the text translated, placeholders filled in).
    pub(super) fn open_scroll_text(&mut self, text: &str, speed: f32, bg: [u8; 4]) {
        let text = self.world.expand_text(text);
        let _font = role(Role::Dialog);
        let width = (screen_width() * 0.7).min(900.0);
        let mut lines = Vec::new();
        for raw in text.lines() {
            let (body, heading) = match raw.strip_prefix("# ") {
                Some(h) => (h, true),
                None => (raw, false),
            };
            if body.trim().is_empty() {
                lines.push((String::new(), false));
                continue;
            }
            for l in crate::render::wrap_text(body, if heading { HEAD_SIZE } else { SIZE } as u16, width) {
                lines.push((l, heading));
            }
        }
        self.scroll = Some(ScrollText { lines, speed: if speed > 0.0 { speed } else { 40.0 }, bg: Color::from_rgba(bg[0], bg[1], bg[2], bg[3]), y: 0.0 });
    }

    pub(super) fn scroll_open(&self) -> bool {
        self.scroll.is_some()
    }

    /// Rolls it on; true while it is open (it pauses the game).
    pub(super) fn update_scroll_text(&mut self, dt: f32) -> bool {
        let Some(s) = &mut self.scroll else { return false };
        // Holding confirm speeds it up; cancel skips it.
        let fast = input::act_down(Act::Confirm);
        s.y += s.speed * dt * if fast { 4.0 } else { 1.0 };
        if s.y > s.height() + screen_height() || input::act_pressed(Act::Cancel) {
            self.scroll = None;
        }
        true
    }

    pub(super) fn draw_scroll_text(&self) {
        let Some(s) = &self.scroll else { return };
        let _font = role(Role::Dialog);
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, s.bg);
        let mut y = sh - s.y;
        for (l, heading) in &s.lines {
            let (size, step) = if *heading { (HEAD_SIZE, HEAD_LINE) } else { (SIZE, LINE) };
            if y + step > 0.0 && y < sh + step && !l.is_empty() {
                let w = measure_plain(l, size).width;
                let color = if *heading { skin::highlight() } else { skin::normal() };
                draw_plain(l, ((sw - w) / 2.0).round(), (y + size).round(), size, color);
            }
            y += step;
        }
        super::glyphs::draw_hint_in("Hold {key:confirm}: faster · {key:cancel}: skip", 16.0, sh - 14.0, 16.0, sw - 32.0, skin::disabled());
    }

    /// For `status()`: `-`, or how far it rolled `rolled/height` (pixels).
    pub(super) fn scroll_status(&self) -> String {
        match &self.scroll {
            None => "-".into(),
            Some(s) => format!("{:.0}/{:.0}", s.y, s.height() + screen_height()),
        }
    }
}
