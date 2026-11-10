//! Emotions over characters' heads: a small animated icon (a tag of a sprite sheet of the kind
//! *Emotions*: "surprise", "joy", "sleepy", …) that pops up above a character (or the player),
//! follows it while it walks and fades after a while (or stays until cleared). Shown by the
//! *Show emotion* action, `emote()` and `say(…, #{ emote })`; characters also show some by
//! themselves (`ProjectConfig::auto_emotions`): enemies noticing, losing and fleeing from the
//! player, sleepers, companions learning a spell. Not saved.

use macroquad::prelude::*;

use super::Game;
use crate::defs::SheetKind;

/// How long an emotion stays unless told otherwise.
pub(super) const DEFAULT_SECONDS: f32 = 2.5;
/// Automatic emotions are shorter.
const AUTO_SECONDS: f32 = 1.4;
/// The pop at the start and the fade at the end (seconds).
const POP: f32 = 0.18;
const FADE: f32 = 0.3;

/// An emotion being shown.
#[derive(Clone, Debug)]
pub(super) struct Emote {
    pub name: String,
    sheet: String,
    first: u32,
    last: u32,
    fps: f32,
    t: f32,
    /// Seconds in all (None = until cleared).
    seconds: Option<f32>,
}

impl Emote {
    fn alpha(&self) -> f32 {
        match self.seconds {
            Some(s) if s - self.t < FADE => ((s - self.t) / FADE).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    /// The pop: a quick grow with a small overshoot.
    fn scale(&self) -> f32 {
        if self.t >= POP {
            return 1.0;
        }
        let p = self.t / POP;
        0.3 + 0.95 * p - 0.25 * (p * std::f32::consts::PI).sin().max(0.0) * (1.0 - p) + 0.3 * (p * std::f32::consts::PI).sin()
    }
}

/// Who shows an emotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Who {
    Player,
    Entity(usize),
}

impl Game {
    /// The animation of emotion `name` (a tag of an *Emotions* sheet; case doesn't matter).
    fn find_emotion(&self, name: &str) -> Option<(String, u32, u32, f32)> {
        self.project.db.sheets.iter().filter(|s| s.kind == SheetKind::Emotions).find_map(|s| {
            s.tags.iter().find(|t| t.name.eq_ignore_ascii_case(name.trim())).map(|t| (s.id.clone(), t.first, t.last.max(t.first), if t.fps > 0.0 { t.fps } else { 6.0 }))
        })
    }

    /// The character a target names: `player` (or the player's name), `self`, a placed
    /// character's name, `{uid:N}` (a dialog speaker), or the display name of a character here.
    pub(super) fn emote_target(&self, target: &str, owner: Option<u32>) -> Option<Who> {
        let t = target.trim();
        if t.eq_ignore_ascii_case("player") || (!self.world.player_name.is_empty() && t == self.world.player_name) {
            return Some(Who::Player);
        }
        if let Some(uid) = t.strip_prefix("{uid:").and_then(|s| s.strip_suffix('}')).and_then(|s| s.parse::<u32>().ok()) {
            return self.loaded.entities.iter().position(|e| e.uid == uid).map(Who::Entity);
        }
        if let Some(i) = self.entity_index(t, owner) {
            return Some(Who::Entity(i));
        }
        let db = &self.project.db;
        let near = |i: &usize| self.loaded.entities[*i].body.pos.distance(self.player.pos);
        (0..self.loaded.entities.len())
            .filter(|&i| {
                let e = &self.loaded.entities[i];
                e.active && db.character(&e.def).is_some_and(|d| d.name.eq_ignore_ascii_case(t))
            })
            .min_by(|a, b| near(a).total_cmp(&near(b)))
            .map(Who::Entity)
    }

    /// Shows emotion `name` over `who`: `seconds` 0 = the default, below 0 = until cleared.
    /// False when the project has no such emotion.
    pub(super) fn start_emote(&mut self, who: Who, name: &str, seconds: f32) -> bool {
        let Some((sheet, first, last, fps)) = self.find_emotion(name) else {
            eprintln!("[game] no emotion '{name}' (a tag of an Emotions sprite sheet)");
            return false;
        };
        let seconds = if seconds < 0.0 { None } else if seconds == 0.0 { Some(DEFAULT_SECONDS) } else { Some(seconds) };
        let e = Emote { name: name.trim().to_lowercase(), sheet, first, last, fps, t: 0.0, seconds };
        match who {
            Who::Player => self.player_emote = Some(e),
            Who::Entity(i) => self.loaded.entities[i].emote = Some(Box::new(e)),
        }
        true
    }

    pub(super) fn clear_emote(&mut self, who: Who) {
        match who {
            Who::Player => self.player_emote = None,
            Who::Entity(i) => self.loaded.entities[i].emote = None,
        }
    }

    /// A character shows an emotion by itself (if the project wants that and has the emotion);
    /// it doesn't cut short one it already shows.
    pub(super) fn auto_emote(&mut self, i: usize, name: &str) {
        if !self.project.config.auto_emotions || self.loaded.entities[i].emote.is_some() || self.find_emotion(name).is_none() {
            return;
        }
        self.start_emote(Who::Entity(i), name, AUTO_SECONDS);
    }

    pub(super) fn update_emotes(&mut self, dt: f32) {
        let tick = |e: &mut Option<Emote>| {
            if let Some(m) = e {
                m.t += dt;
                if m.seconds.is_some_and(|s| m.t >= s) {
                    *e = None;
                }
            }
        };
        tick(&mut self.player_emote);
        for ent in &mut self.loaded.entities {
            if let Some(m) = &mut ent.emote {
                m.t += dt;
                if m.seconds.is_some_and(|s| m.t >= s) {
                    ent.emote = None;
                }
            }
        }
    }

    /// The emotion icons over the heads (after the characters, before the overhead tiles).
    pub(super) fn draw_emotes(&self) {
        let u = self.u();
        let draw = |m: &Emote, feet: Vec2, height: f32| {
            let n = m.last - m.first + 1;
            let frame = m.first + (m.t * m.fps) as u32 % n;
            let bob = (m.t * 3.0).sin() * 0.8 * u;
            let at = feet - vec2(0.0, height + 9.0 * u + bob);
            self.assets.draw_frame_scaled(&m.sheet, frame, at, 0.0, m.scale(), Color::new(1.0, 1.0, 1.0, m.alpha()));
        };
        if let Some(m) = &self.player_emote {
            if !self.on_title() {
                let h = self.player_appearance().map_or(48.0, |a| self.character_frame_size(&a).1);
                draw(m, self.player.pos, h);
            }
        }
        // Sleepers (lying on furniture at night; a sick child resting by day doesn't snore) snore by themselves.
        let night = self.world.hour >= 21.0 || self.world.hour < 6.0;
        let sleepy = if self.project.config.auto_emotions && night { self.find_emotion("sleepy") } else { None };
        for e in self.loaded.entities.iter().filter(|e| e.active) {
            let h = if e.appearance.body.is_empty() { 32.0 } else { self.character_frame_size(&e.appearance).1 };
            if let Some(m) = &e.emote {
                draw(m, e.body.pos, h);
            } else if let (Some((sheet, first, last, fps)), Some(seat)) = (&sleepy, &e.npc_seat) {
                if seat.lying() {
                    let m = Emote { name: "sleepy".into(), sheet: sheet.clone(), first: *first, last: *last, fps: *fps, t: macroquad::time::get_time() as f32 + e.uid as f32, seconds: None };
                    draw(&m, e.body.pos + vec2(0.0, -4.0 * u), h * 0.6);
                }
            }
        }
    }

    /// `emotes=` for `status`: who:emotion.
    pub(super) fn emotes_status(&self) -> Vec<String> {
        let mut out: Vec<String> = self.player_emote.iter().map(|m| format!("player:{}", m.name)).collect();
        out.extend(self.loaded.entities.iter().filter_map(|e| e.emote.as_ref().map(|m| format!("{}:{}", if e.name.is_empty() { &e.def } else { &e.name }, m.name))));
        out
    }
}
