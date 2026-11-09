//! Lighting: darkens the world by the map's ambient light (time of day outdoors, constant in
//! dungeons) and adds soft circles of light around light sources. A light map (ambient color
//! plus the lights) is drawn into a render target with the world camera, then multiplied over
//! the screen.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};
use macroquad::prelude::*;

use crate::defs::LightSpec;
use crate::map::{LightMode, MapLighting};

/// 0..1 daylight at `hour`: 1 from 7 to 18, 0 from 21 to 5, ramping at dawn and dusk.
pub fn daylight(hour: f32) -> f32 {
    let h = hour.rem_euclid(24.0);
    let ramp = |a: f32, b: f32, x: f32| ((x - a) / (b - a)).clamp(0.0, 1.0);
    if h < 12.0 {
        ramp(5.0, 7.0, h)
    } else {
        1.0 - ramp(18.0, 21.0, h)
    }
}

/// Night by the clock (used by scripts and the HUD).
pub fn is_night(hour: f32) -> bool {
    daylight(hour) < 0.5
}

/// The color the unlit world is multiplied with; `None` = no darkness at all.
pub fn ambient(l: MapLighting, hour: f32) -> Option<Color> {
    let darkness = l.darkness.clamp(0.0, 1.0);
    let (k, night, warm) = match l.mode {
        LightMode::Bright => return None,
        LightMode::Outdoor => {
            let d = daylight(hour);
            // A warm tint while the light changes at dawn and dusk.
            (darkness * (1.0 - d), vec3(0.10, 0.13, 0.28), d * (1.0 - d) * 4.0 * darkness)
        }
        LightMode::Dark => (darkness, vec3(0.0, 0.0, 0.0), 0.0),
    };
    if k <= 0.002 && warm <= 0.002 {
        return None;
    }
    let c = vec3(1.0, 1.0, 1.0).lerp(night, k) * vec3(1.0, 1.0 - warm * 0.12, 1.0 - warm * 0.3);
    Some(Color::new(c.x, c.y, c.z, 1.0))
}

/// A light source in world pixels.
#[derive(Clone, Debug)]
pub struct Light {
    pub pos: Vec2,
    pub spec: LightSpec,
    /// Varies the flicker between sources.
    pub seed: f32,
}

impl Light {
    pub fn new(pos: Vec2, spec: &LightSpec) -> Self {
        Light { pos, spec: spec.clone(), seed: (pos.x * 0.37 + pos.y * 0.71).fract() * 100.0 }
    }
}

/// Lights of the light-giving tiles placed on `map` (wall torches, lamps; see
/// `TilesetDef::lights`), centered on their tiles.
pub fn tile_lights(map: &crate::map::Map, db: &crate::defs::Database, ts: f32) -> Vec<Light> {
    let sets: Vec<Option<&crate::defs::TilesetDef>> = map.tilesets.iter().map(|id| db.tileset(id).filter(|t| !t.lights.is_empty())).collect();
    if sets.iter().all(Option::is_none) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for layer in &map.layers {
        for (x, y, t) in layer.tiles() {
            let Some((slot, index)) = t.parts() else { continue };
            if let Some(spec) = sets.get(slot).copied().flatten().and_then(|d| d.light_of(index)) {
                out.push(Light::new(vec2((x as f32 + 0.5) * ts, (y as f32 + 0.5) * ts), spec));
            }
        }
    }
    out
}

const VERTEX: &str = r#"#version 100
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

const FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
void main() {
    gl_FragColor = color * texture2D(Texture, uv);
}"#;

/// Draws darkness and lights; keeps its render target and textures between frames.
pub struct LightPass {
    rt: Option<(RenderTarget, u32, u32)>,
    glow: Texture2D,
    multiply: Option<Material>,
}

impl Default for LightPass {
    fn default() -> Self {
        Self::new()
    }
}

impl LightPass {
    pub fn new() -> Self {
        // Soft round glow: opaque white in the middle fading out to the edge.
        const N: u16 = 128;
        let mut img = Image::gen_image_color(N, N, Color::new(1.0, 1.0, 1.0, 0.0));
        let c = N as f32 / 2.0;
        for y in 0..N {
            for x in 0..N {
                let d = (vec2(x as f32 + 0.5, y as f32 + 0.5) - vec2(c, c)).length() / c;
                // Full light over the inner part, then a smooth fade to the edge.
                let a = ((1.0 - d) * 1.8).clamp(0.0, 1.0);
                let a = a * a * (3.0 - 2.0 * a);
                img.set_pixel(x as u32, y as u32, Color::new(1.0, 1.0, 1.0, a));
            }
        }
        let glow = Texture2D::from_image(&img);
        glow.set_filter(FilterMode::Linear);
        // Multiplies the screen with the light map: result = screen color × light color.
        let multiply = load_material(
            ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::DestinationColor), BlendFactor::Zero)),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .map_err(|e| eprintln!("[render] lighting shader: {e}"))
        .ok();
        LightPass { rt: None, glow, multiply }
    }

    /// Darkens what was drawn with `cam` by `ambient` and lights `lights` (call after the world,
    /// before the UI). `ts` = tile size in pixels (light radii are in tiles), `t` = time.
    pub fn draw(&mut self, cam: &Camera2D, ambient: Color, lights: &[Light], ts: f32, t: f64) {
        // Half resolution: lights are soft anyway.
        let (w, h) = ((screen_width() / 2.0).max(1.0) as u32, (screen_height() / 2.0).max(1.0) as u32);
        if self.rt.as_ref().is_none_or(|r| (r.1, r.2) != (w, h)) {
            let rt = render_target(w, h);
            rt.texture.set_filter(FilterMode::Linear);
            self.rt = Some((rt, w, h));
        }
        let rt = self.rt.as_ref().unwrap().0.clone();
        let lc = Camera2D { target: cam.target, zoom: cam.zoom, rotation: cam.rotation, offset: cam.offset, render_target: Some(rt.clone()), viewport: None };
        set_camera(&lc);
        clear_background(ambient);
        let view = super::camera_view(cam);
        for l in lights {
            let mut r = l.spec.radius.max(0.1) * ts;
            let mut i = l.spec.intensity.clamp(0.0, 1.0);
            if l.spec.flicker {
                let t = t as f32 + l.seed;
                r *= 1.0 + 0.05 * (t * 9.1).sin() + 0.03 * (t * 23.7).sin();
                i *= 0.92 + 0.08 * (t * 15.3).sin();
            }
            if l.pos.x + r < view.x || l.pos.x - r > view.right() || l.pos.y + r < view.y || l.pos.y - r > view.bottom() {
                continue;
            }
            let [cr, cg, cb] = l.spec.color;
            let col = Color::new(cr as f32 / 255.0, cg as f32 / 255.0, cb as f32 / 255.0, i);
            let params = DrawTextureParams { dest_size: Some(vec2(r * 2.0, r * 2.0)), ..Default::default() };
            draw_texture_ex(&self.glow, l.pos.x - r, l.pos.y - r, col, params);
        }
        set_default_camera();
        let full = DrawTextureParams { dest_size: Some(vec2(screen_width(), screen_height())), ..Default::default() };
        static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *DEBUG.get_or_init(|| std::env::var("RUSTPG2D_LIGHTMAP_DEBUG").is_ok()) {
            draw_texture_ex(&rt.texture, 0.0, 0.0, WHITE, full);
            return;
        }
        match &self.multiply {
            Some(m) => {
                gl_use_material(m);
                draw_texture_ex(&rt.texture, 0.0, 0.0, WHITE, full);
                gl_use_default_material();
            }
            // Without the shader: a plain dark overlay (no light circles).
            None => draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 1.0 - (ambient.r + ambient.g + ambient.b) / 3.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_night_curve() {
        assert_eq!(daylight(12.0), 1.0);
        assert_eq!(daylight(0.0), 0.0);
        assert!(daylight(6.0) > 0.4 && daylight(6.0) < 0.6);
        assert!(daylight(19.5) > 0.4 && daylight(19.5) < 0.6);
        assert!(is_night(23.0) && !is_night(9.0));
        let out = MapLighting { mode: LightMode::Outdoor, darkness: 0.7 };
        assert!(ambient(out, 12.0).is_none());
        let night = ambient(out, 0.0).unwrap();
        assert!(night.r < 0.5 && night.b > night.r);
        let dark = MapLighting { mode: LightMode::Dark, darkness: 1.0 };
        assert_eq!(ambient(dark, 12.0).map(|c| (c.r, c.g, c.b)), Some((0.0, 0.0, 0.0)));
        assert!(ambient(MapLighting::default(), 0.0).is_none());
    }
}
