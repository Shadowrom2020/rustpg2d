//! Screen feedback ("juice"): a short camera shake and a red flash when the player is hurt,
//! shakes from explosions (`effect(..., #{ shake: 0.3 })`) and from scripts / the *Shake the
//! screen* action, the hit-stop (the world pauses for a few hundredths of a second when the
//! player lands a blow), the white flash of a hit character and the damage numbers. The player
//! can switch shakes, flashes and hit-stop off (Options › Shake & flash) and the numbers too
//! (Options › Damage numbers).

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};
use macroquad::prelude::*;

use super::text::{draw_text, measure_text};
use super::Game;

/// The shake and flash of a hit on the player.
const HURT_SHAKE: f32 = 0.25;
const HURT_FLASH: f32 = 0.15;
/// Shake strength 1 moves the view up to this many art pixels.
const SHAKE_PIXELS: f32 = 3.0;
/// A hit character shows white this long.
pub(super) const HIT_FLASH: f32 = 0.1;
/// Damage numbers rise and fade over this many seconds.
const NUMBER_TIME: f32 = 0.8;
/// A hurt character's health bar shows this long after the hit, then fades.
pub(super) const BAR_TIME: f32 = 3.0;

/// What a damage number is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NumberKind {
    /// The player (or a spell, a companion) hurt a character.
    Hit,
    /// A critical hit of the player's.
    Crit,
    /// The player or a companion got hurt.
    Hurt,
}

/// A damage number floating up from a hit.
struct Number {
    pos: Vec2,
    kind: NumberKind,
    t: f32,
    drift: f32,
    /// What it shows, its colour (None = the kind's) and size (× the kind's): the
    /// `damage_number` script hook may change them.
    text: String,
    color: Option<Color>,
    scale: f32,
}

#[derive(Default)]
pub(super) struct Juice {
    /// Seconds of shaking left, and how long the current shake lasts in total.
    shake: f32,
    shake_total: f32,
    /// Strength of the current shake (1 = a hit).
    strength: f32,
    /// Seconds of the red hurt flash left.
    flash: f32,
    numbers: Vec<Number>,
    /// The last damage numbers, for `status`.
    recent: std::collections::VecDeque<String>,
    /// Draws a sprite as a white silhouette (the hit flash); made on first use.
    white: std::cell::OnceCell<Option<Material>>,
    /// Autoplay `crit`: the player's side's hits always / never critical (None = by the dice).
    pub crit_mode: Option<bool>,
    /// *Flash screen*: colour (alpha = strength), seconds left, seconds in all.
    screen_flash: Option<([f32; 4], f32, f32)>,
}

const WHITE_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

/// The texture's shape in white (the vertex color's alpha fades it).
const WHITE_FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
void main() {
    gl_FragColor = vec4(1.0, 1.0, 1.0, texture2D(Texture, uv).a * color.a);
}"#;

impl Game {
    /// Shakes the view for `seconds` (`strength` 1 = like a hit; a stronger shake replaces a
    /// weaker one, a longer one extends it).
    pub(super) fn shake_screen(&mut self, seconds: f32, strength: f32) {
        if seconds <= 0.0 || strength <= 0.0 {
            return;
        }
        let j = &mut self.juice;
        if j.shake <= 0.0 || strength >= j.strength {
            j.strength = strength;
        }
        j.shake = j.shake.max(seconds);
        j.shake_total = j.shake;
    }

    /// The player got hurt: a shake and a red flash.
    pub(super) fn hurt_feedback(&mut self) {
        self.shake_screen(HURT_SHAKE, 1.0);
        self.juice.flash = HURT_FLASH;
    }

    pub(super) fn update_juice(&mut self, dt: f32) {
        let j = &mut self.juice;
        j.shake = (j.shake - dt).max(0.0);
        j.flash = (j.flash - dt).max(0.0);
        if let Some((_, left, _)) = &mut j.screen_flash {
            *left -= dt;
            if *left <= 0.0 {
                j.screen_flash = None;
            }
        }
        self.world.screen.update(dt);
        for n in &mut j.numbers {
            n.t += dt;
        }
        j.numbers.retain(|n| n.t < NUMBER_TIME);
    }

    /// Autoplay `crit always|never|chance`.
    pub fn set_crit_mode(&mut self, mode: Option<bool>) {
        self.juice.crit_mode = mode;
    }

    /// A damage number rising from `pos` (world; above a character's head).
    pub(super) fn damage_number(&mut self, pos: Vec2, value: i32, kind: NumberKind) {
        let (mut text, mut color, mut scale) = (value.to_string(), None, 1.0);
        // Script hook: its text, colour and size (asked once, not every frame).
        if self.hooked("damage_number") {
            use crate::hooks::{map, num};
            let k = match kind {
                NumberKind::Hit => "hit",
                NumberKind::Crit => "crit",
                NumberKind::Hurt => "hurt",
            };
            let m = self.hook("damage_number", map([("value", (value as i64).into()), ("kind", k.into()), ("text", text.clone().into()), ("color", "".into()), ("size", 1.0_f64.into())]));
            if let Some(t) = m.get("text").map(|t| t.to_string()) {
                text = t;
            }
            color = m.get("color").and_then(|c| super::script_hud::color_of(&c.to_string()));
            scale = num(&m, "size", 1.0).clamp(0.2, 4.0) as f32;
        }
        let j = &mut self.juice;
        let drift = rand::gen_range(-1.0, 1.0);
        j.numbers.push(Number { pos, kind, t: 0.0, drift, text, color, scale });
        if j.numbers.len() > 40 {
            j.numbers.remove(0);
        }
        j.recent.push_back(match kind {
            // (A hook's own text, as shown.)
            _ if j.numbers.last().is_some_and(|n| n.text != value.to_string()) => j.numbers.last().map(|n| n.text.clone()).unwrap_or_default(),
            NumberKind::Hit => value.to_string(),
            NumberKind::Crit => format!("{value}!"),
            NumberKind::Hurt => format!("-{value}"),
        });
        if j.recent.len() > 6 {
            j.recent.pop_front();
        }
    }

    /// "miss" rising from `pos` (a fight's attack that missed).
    pub(super) fn miss_number(&mut self, pos: Vec2) {
        let text = crate::lang::tr_ui("miss").into_owned();
        let j = &mut self.juice;
        let drift = rand::gen_range(-1.0, 1.0);
        j.numbers.push(Number { pos, kind: NumberKind::Hit, t: 0.0, drift, text, color: Some(Color::new(0.8, 0.8, 0.85, 1.0)), scale: 0.85 });
        if j.numbers.len() > 40 {
            j.numbers.remove(0);
        }
        j.recent.push_back("miss".into());
        if j.recent.len() > 6 {
            j.recent.pop_front();
        }
    }

    /// The damage numbers (screen space, over the world; `cam` = the world's camera).
    pub(super) fn draw_damage_numbers(&self, cam: &Camera2D) {
        let _font = super::text::role(super::text::Role::Hud);
        if !self.settings.damage_numbers {
            return;
        }
        // Screen pixels per world pixel.
        let scale = (cam.world_to_screen(vec2(1.0, 0.0)) - cam.world_to_screen(Vec2::ZERO)).x.abs().max(0.1);
        let ts = self.ts();
        for n in &self.juice.numbers {
            let p = n.t / NUMBER_TIME;
            // Up fast, then slower; a pop at the start; fading out at the end.
            let rise = (1.0 - (1.0 - p) * (1.0 - p)) * ts * 0.7;
            let pop = if n.t < 0.1 { 1.0 + 0.4 * (1.0 - n.t / 0.1) } else { 1.0 };
            let alpha = ((1.0 - p) / 0.35).min(1.0);
            let (base, col) = match n.kind {
                NumberKind::Hit => (0.42, Color::new(1.0, 1.0, 1.0, alpha)),
                NumberKind::Crit => (0.62, Color::new(1.0, 0.86, 0.2, alpha)),
                NumberKind::Hurt => (0.42, Color::new(1.0, 0.32, 0.3, alpha)),
            };
            let col = n.color.map_or(col, |c| Color { a: c.a * alpha, ..c });
            let size = (base * n.scale * ts * scale * pop).round().max(8.0);
            let text = &n.text;
            let at = cam.world_to_screen(n.pos + vec2(n.drift * ts * 0.25 * p, -rise));
            let w = measure_text(text, None, size as u16, 1.0).width;
            let (x, y) = ((at.x - w / 2.0).round(), at.y.round());
            // (High contrast: a thicker outline all round.)
            let contrast = super::ui::contrast();
            let o = (size / if contrast { 7.0 } else { 12.0 }).max(1.0).round();
            let ring: &[(f32, f32)] = if contrast { &[(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (o, o), (-o, -o), (o, -o), (-o, o)] } else { &[(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (o, o)] };
            for (dx, dy) in ring {
                draw_text(text, x + dx, y + dy, size, Color::new(0.0, 0.0, 0.0, if contrast { alpha } else { alpha * 0.9 }));
            }
            draw_text(text, x, y, size, col);
        }
    }

    /// Runs `draw` with the white-silhouette material (the hit flash; nothing without shaders).
    pub(super) fn draw_white(&self, draw: impl FnOnce()) {
        let m = self.juice.white.get_or_init(|| {
            // Alpha blending like the default material (a material without it draws its quads opaque).
            let blend = BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha));
            let params = MaterialParams { pipeline_params: PipelineParams { color_blend: Some(blend), alpha_blend: Some(blend), ..Default::default() }, ..Default::default() };
            load_material(ShaderSource::Glsl { vertex: WHITE_VERTEX, fragment: WHITE_FRAGMENT }, params)
                .map_err(|e| eprintln!("[render] hit flash shader: {e}"))
                .ok()
        });
        if let Some(m) = m {
            gl_use_material(m);
            draw();
            gl_use_default_material();
        }
    }

    /// `numbers=` (the last six; `!` = critical, `-` = the player or a companion hurt) for `status`.
    pub(super) fn hits_status(&self) -> String {
        format!("numbers={:?}", self.juice.recent.iter().collect::<Vec<_>>())
    }

    /// The camera offset of the shake this frame (world pixels; zero when switched off).
    pub(super) fn shake_offset(&self) -> Vec2 {
        let j = &self.juice;
        if j.shake <= 0.0 || !self.settings.screen_effects || j.shake_total <= 0.0 {
            return Vec2::ZERO;
        }
        // Fades out over the shake; a new random direction every frame.
        let amount = SHAKE_PIXELS * self.u() * j.strength * (j.shake / j.shake_total);
        vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * amount
    }

    /// The red hurt flash over the whole screen (screen space).
    pub(super) fn draw_hurt_flash(&self) {
        if self.juice.flash > 0.0 && self.settings.screen_effects {
            let a = 0.32 * self.juice.flash / HURT_FLASH;
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.85, 0.05, 0.05, a));
        }
    }

    /// *Flash screen* (left out when the player turned off screen effects).
    pub(super) fn flash_screen(&mut self, rgba: [u8; 4], seconds: f32) {
        if self.settings.screen_effects && seconds > 0.0 {
            self.juice.screen_flash = Some((rgba.map(|v| v as f32 / 255.0), seconds, seconds));
        }
    }

    /// The tint (over the world and the pictures).
    pub(super) fn draw_screen_tint(&self) {
        if let Some(t) = &self.world.screen.tint {
            let [r, g, b, a] = t.now();
            if a > 0.0 {
                draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(r, g, b, a));
            }
        }
    }

    /// The flash and the fade cover (over the tint, under dialogs and the HUD).
    pub(super) fn draw_screen_cover(&self) {
        if let Some(([r, g, b, a], left, total)) = self.juice.screen_flash {
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(r, g, b, a * (left / total).clamp(0.0, 1.0)));
        }
        if let Some(c) = &self.world.screen.cover {
            let [r, g, b, a] = c.now();
            if a > 0.0 {
                draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(r, g, b, a));
            }
        }
    }

    /// For `status()`: `tint:r,g,b,a;flash:a;cover:a` of what is showing, `-` for nothing.
    pub(super) fn screen_status(&self) -> String {
        let f = |c: [f32; 4]| format!("{:.0},{:.0},{:.0},{:.2}", c[0] * 255.0, c[1] * 255.0, c[2] * 255.0, c[3]);
        let mut parts = Vec::new();
        if let Some(t) = &self.world.screen.tint {
            parts.push(format!("tint:{}", f(t.now())));
        }
        if let Some((c, left, total)) = self.juice.screen_flash {
            parts.push(format!("flash:{:.2}", c[3] * left / total));
        }
        if let Some(c) = &self.world.screen.cover {
            parts.push(format!("cover:{:.2}", c.now()[3]));
        }
        if parts.is_empty() {
            "-".into()
        } else {
            parts.join(";")
        }
    }

    /// For `status()`: seconds of shake left (`off` when switched off).
    pub(super) fn juice_status(&self) -> String {
        if self.settings.screen_effects {
            format!("{:.2}", self.juice.shake)
        } else {
            "off".into()
        }
    }
}
