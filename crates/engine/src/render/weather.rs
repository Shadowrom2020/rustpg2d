//! Weather particles: rain (streaks and splashes), snow (drifting flakes) and fog (slow soft
//! clouds over a grey veil). They live in world coordinates inside the camera view and wrap
//! around its edges, so the layer moves with the world and always fills the screen. Drawn on
//! top of the lit scene (the game: after the light pass); `brightness` dims them at night.

use gfx::prelude::*;

use crate::map::{Weather, WeatherKind};

/// Particles per 100 000 square world pixels at full intensity.
const RAIN_DENSITY: f32 = 22.0;
const SNOW_DENSITY: f32 = 14.0;
const FOG_CLOUDS: usize = 9;

#[derive(Clone, Copy, Debug)]
struct Particle {
    pos: Vec2,
    /// Speed (world px / s).
    vel: Vec2,
    /// Size factor / phase for variety.
    size: f32,
    phase: f32,
}

#[derive(Clone, Copy, Debug)]
struct Splash {
    pos: Vec2,
    age: f32,
}

/// The weather layer of a view (game or editor preview).
pub struct WeatherFx {
    kind: WeatherKind,
    parts: Vec<Particle>,
    splashes: Vec<Splash>,
    seed: u32,
    time: f32,
    /// A soft round blob (fog clouds), made on first use.
    blob: Option<Texture2D>,
}

impl Default for WeatherFx {
    fn default() -> Self {
        WeatherFx { kind: WeatherKind::None, parts: Vec::new(), splashes: Vec::new(), seed: 0x9e37_79b9, time: 0.0, blob: None }
    }
}

/// How the weather tints the scene's ambient light (multiplied in): rain and fog darken it a
/// little, rain a bit towards blue.
pub fn ambient_factor(w: Weather) -> Option<Vec3> {
    if !w.active() {
        return None;
    }
    let i = w.intensity.clamp(0.0, 1.0);
    match w.kind {
        WeatherKind::Rain => Some(vec3(1.0 - 0.32 * i, 1.0 - 0.28 * i, 1.0 - 0.2 * i)),
        WeatherKind::Fog => Some(Vec3::splat(1.0 - 0.12 * i)),
        WeatherKind::Snow => Some(vec3(1.0 - 0.06 * i, 1.0 - 0.05 * i, 1.0 - 0.02 * i)),
        WeatherKind::None => None,
    }
}

/// Applies the weather to an ambient light color (`None` = full daylight).
pub fn tint_ambient(ambient: Option<Color>, w: Weather) -> Option<Color> {
    let Some(f) = ambient_factor(w) else { return ambient };
    let base = ambient.map_or(Vec3::ONE, |c| vec3(c.r, c.g, c.b));
    let c = base * f;
    Some(Color::new(c.x, c.y, c.z, 1.0))
}

impl WeatherFx {
    fn rand(&mut self) -> f32 {
        // xorshift32
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn spawn(&mut self, kind: WeatherKind, view: Rect, scale: f32) -> Particle {
        let pos = vec2(view.x + self.rand() * view.w, view.y + self.rand() * view.h);
        let (size, phase) = (0.6 + self.rand() * 0.8, self.rand() * std::f32::consts::TAU);
        let vel = match kind {
            WeatherKind::Rain => vec2(-90.0, 820.0 + self.rand() * 260.0) * scale,
            WeatherKind::Snow => vec2(-12.0 + self.rand() * 24.0, 38.0 + self.rand() * 40.0) * scale,
            _ => vec2(10.0 + self.rand() * 14.0, -2.0 + self.rand() * 4.0) * scale,
        };
        Particle { pos, vel, size, phase }
    }

    /// Advances the particles for weather `w` in camera view `view` (world pixels); `scale` =
    /// world pixels per tile / 32 (speeds and sizes follow the tile size).
    pub fn update(&mut self, dt: f32, w: Weather, view: Rect, scale: f32) {
        self.time += dt;
        if w.kind != self.kind {
            self.kind = w.kind;
            self.parts.clear();
            self.splashes.clear();
        }
        if !w.active() || view.w <= 0.0 || view.h <= 0.0 {
            self.parts.clear();
            self.splashes.clear();
            return;
        }
        let area = view.w * view.h / 100_000.0;
        let want = match w.kind {
            WeatherKind::Rain => (area * RAIN_DENSITY * w.intensity) as usize,
            WeatherKind::Snow => (area * SNOW_DENSITY * w.intensity) as usize,
            WeatherKind::Fog => FOG_CLOUDS,
            WeatherKind::None => 0,
        }
        .min(6000);
        while self.parts.len() < want {
            let p = self.spawn(w.kind, view, scale);
            self.parts.push(p);
        }
        self.parts.truncate(want);
        let t = self.time;
        for p in &mut self.parts {
            let sway = if w.kind == WeatherKind::Snow { vec2((t * 1.3 + p.phase).sin() * 14.0 * scale, 0.0) } else { Vec2::ZERO };
            p.pos += (p.vel + sway) * dt;
            // Wrap into the view: the layer always covers the screen, also while it moves.
            p.pos.x = view.x + (p.pos.x - view.x).rem_euclid(view.w);
            p.pos.y = view.y + (p.pos.y - view.y).rem_euclid(view.h);
        }
        // Rain: splashes where drops land (a few per frame).
        if w.kind == WeatherKind::Rain {
            let n = (area * 60.0 * w.intensity * dt) as usize + usize::from(self.rand() < (area * 60.0 * w.intensity * dt).fract());
            for _ in 0..n {
                let pos = vec2(view.x + self.rand() * view.w, view.y + self.rand() * view.h);
                self.splashes.push(Splash { pos, age: 0.0 });
            }
        }
        for s in &mut self.splashes {
            s.age += dt;
        }
        self.splashes.retain(|s| s.age < 0.25);
    }

    fn blob(&mut self) -> Texture2D {
        if let Some(t) = &self.blob {
            return t.clone();
        }
        let n = 64u16;
        let mut img = Image::gen_image_color(n, n, Color::new(1.0, 1.0, 1.0, 0.0));
        for y in 0..n as u32 {
            for x in 0..n as u32 {
                let d = vec2(x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 32.0).length() / 32.0;
                let a = (1.0 - d).clamp(0.0, 1.0);
                img.set_pixel(x, y, Color::new(1.0, 1.0, 1.0, a * a * (3.0 - 2.0 * a)));
            }
        }
        let t = Texture2D::from_image(&img);
        t.set_filter(FilterMode::Linear);
        self.blob = Some(t.clone());
        t
    }

    /// Draws the layer for weather `w` (camera already set to the world). `brightness` (0..1)
    /// is how lit the scene is (night: dim rain and snow).
    pub fn draw(&mut self, w: Weather, view: Rect, scale: f32, brightness: f32) {
        if !w.active() {
            return;
        }
        let i = w.intensity.clamp(0.0, 1.0);
        let lum = brightness.clamp(0.35, 1.0);
        match w.kind {
            WeatherKind::Rain => {
                // A light veil, then the streaks and splashes.
                draw_rectangle(view.x, view.y, view.w, view.h, Color::new(0.35, 0.42, 0.55, 0.10 * i));
                for p in &self.parts {
                    let tail = p.vel * 0.022 * p.size;
                    let c = Color::new(0.78 * lum + 0.1, 0.84 * lum + 0.1, 1.0 * lum + 0.1, 0.38 + 0.2 * p.size - 0.25);
                    draw_line(p.pos.x, p.pos.y, p.pos.x - tail.x, p.pos.y - tail.y, 1.2 * scale, c);
                }
                for s in &self.splashes {
                    let k = s.age / 0.25;
                    let r = (2.0 + 5.0 * k) * scale;
                    draw_ellipse_lines(s.pos.x, s.pos.y, r, r * 0.45, 0.0, 1.0 * scale, Color::new(0.85 * lum, 0.9 * lum, 1.0 * lum, 0.45 * (1.0 - k)));
                }
            }
            WeatherKind::Snow => {
                draw_rectangle(view.x, view.y, view.w, view.h, Color::new(0.9, 0.93, 1.0, 0.05 * i));
                for p in &self.parts {
                    let c = Color::new(lum, lum, lum + 0.03, 0.85);
                    draw_circle(p.pos.x, p.pos.y, (0.9 + 1.3 * p.size) * scale, c);
                }
            }
            WeatherKind::Fog => {
                let tex = self.blob();
                let g = 0.82 * lum + 0.1;
                draw_rectangle(view.x, view.y, view.w, view.h, Color::new(g, g, g + 0.02, 0.34 * i));
                let cloud = view.w.max(view.h) * 0.55;
                for p in &self.parts {
                    let size = cloud * (0.7 + 0.5 * p.size);
                    let a = 0.42 * i * (0.75 + 0.25 * (self.time * 0.2 + p.phase).sin());
                    draw_texture_ex(&tex, p.pos.x - size / 2.0, p.pos.y - size * 0.3, Color::new(g, g, g + 0.02, a), DrawTextureParams { dest_size: Some(vec2(size, size * 0.6)), ..Default::default() });
                }
            }
            WeatherKind::None => {}
        }
    }

    /// Particles alive (status / tests).
    pub fn count(&self) -> usize {
        self.parts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_tint() {
        let rain = Weather::new(WeatherKind::Rain, 1.0);
        let c = tint_ambient(None, rain).unwrap();
        assert!(c.r < 0.8 && c.b > c.r, "darker, a bit blue: {c:?}");
        assert_eq!(tint_ambient(None, Weather::default()), None);
        let night = Color::new(0.5, 0.5, 0.5, 1.0);
        assert!(tint_ambient(Some(night), rain).unwrap().r < 0.5);
    }

    #[test]
    fn particles_fill_the_view_and_wrap() {
        let mut fx = WeatherFx::default();
        let view = Rect::new(100.0, 100.0, 1000.0, 500.0);
        fx.update(0.016, Weather::new(WeatherKind::Rain, 1.0), view, 1.0);
        assert_eq!(fx.count(), (5.0 * RAIN_DENSITY) as usize);
        for _ in 0..200 {
            fx.update(0.05, Weather::new(WeatherKind::Rain, 1.0), view, 1.0);
        }
        assert!(fx.parts.iter().all(|p| view.contains(p.pos)), "drops stay in the view");
        fx.update(0.016, Weather::new(WeatherKind::Snow, 0.5), view, 1.0);
        assert_eq!(fx.count(), (5.0 * SNOW_DENSITY * 0.5) as usize, "a new kind starts over");
        fx.update(0.016, Weather::default(), view, 1.0);
        assert_eq!(fx.count(), 0);
    }
}
