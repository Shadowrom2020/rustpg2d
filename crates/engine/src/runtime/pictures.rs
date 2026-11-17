//! Picture layers (`crate::pictures`): *Show picture*, *Move picture*, *Erase picture*; the moves
//! advance with the game, the pictures are drawn above the map (and its lighting) and below the
//! HUD, in layer order.

use std::cell::RefCell;

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::prelude::*;

use super::Game;
use crate::pictures::{Blend, Picture, PictureLook, PictureTargets, MAX_LAYER};

thread_local! {
    /// The additive blend (`Blend::Additive`), made once.
    static ADD: RefCell<Option<Option<Material>>> = const { RefCell::new(None) };
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

fn additive() -> Option<Material> {
    ADD.with(|m| {
        m.borrow_mut()
            .get_or_insert_with(|| {
                load_material(
                    ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
                    MaterialParams {
                        pipeline_params: PipelineParams {
                            color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::One)),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .map_err(|e| eprintln!("[pictures] additive shader: {e}"))
                .ok()
            })
            .clone()
    })
}

impl Game {
    /// *Show picture*: puts a picture on `layer` (replacing what was there).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn show_picture(&mut self, layer: u32, sheet: &str, frame: u32, anchor: crate::script_ui::Anchor, on_map: bool, blend: Blend, look: PictureLook) {
        if layer == 0 || layer > MAX_LAYER {
            eprintln!("[pictures] Show picture: layer {layer} is not 1 to {MAX_LAYER}");
            return;
        }
        if !self.assets.sheets.contains_key(sheet) {
            eprintln!("[pictures] Show picture: there is no sprite sheet '{sheet}'");
        }
        self.world.pictures.insert(layer, Picture { sheet: sheet.to_string(), frame, anchor, on_map, blend, look, moving: None });
    }

    /// *Move picture*; false when the layer is empty.
    pub(super) fn move_picture(&mut self, layer: u32, to: &PictureTargets, seconds: f32, easing: crate::pictures::Easing) -> bool {
        let Some(p) = self.world.pictures.get_mut(&layer) else {
            eprintln!("[pictures] Move picture: layer {layer} shows no picture");
            return false;
        };
        let target = to.applied(&p.look);
        p.move_to(target, seconds, easing);
        true
    }

    /// *Erase picture* (0 = all).
    pub(super) fn erase_picture(&mut self, layer: u32) {
        if layer == 0 {
            self.world.pictures.clear();
        } else {
            self.world.pictures.remove(&layer);
        }
    }

    /// Advances the pictures' moves.
    pub(super) fn update_pictures(&mut self, dt: f32) {
        for p in self.world.pictures.values_mut() {
            p.update(dt);
        }
    }

    /// Draws the pictures (screen coordinates; map pictures through `cam`).
    pub(super) fn draw_pictures(&self, cam: &Camera2D) {
        if self.world.pictures.is_empty() {
            return;
        }
        let (sw, sh) = (screen_width(), screen_height());
        let ts = self.assets.tile_size;
        // Screen pixels per world pixel.
        let zoom = (cam.world_to_screen(vec2(1.0, 0.0)) - cam.world_to_screen(vec2(0.0, 0.0))).x.abs();
        for p in self.world.pictures.values() {
            let Some(s) = self.assets.sheets.get(&p.sheet) else { continue };
            let l = &p.look;
            if l.opacity <= 0.0 {
                continue;
            }
            let src = s.src_rect(p.frame);
            let base = vec2(src.w, src.h) / s.scale;
            let (size, top_left) = if p.on_map {
                let size = base * l.scale * zoom;
                let c = cam.world_to_screen(vec2((l.x + 0.5) * ts, (l.y + 0.5) * ts));
                (size, c - size / 2.0)
            } else {
                let size = base * l.scale;
                let (x, y) = p.anchor.place(sw, sh, size.x, size.y, l.x, l.y);
                (size, vec2(x, y))
            };
            let tint = Color::from_rgba(l.tint[0], l.tint[1], l.tint[2], (l.opacity.clamp(0.0, 1.0) * 255.0) as u8);
            let mat = (p.blend == Blend::Additive).then(additive).flatten();
            if let Some(m) = &mat {
                gl_use_material(m);
            }
            draw_texture_ex(
                &s.texture,
                top_left.x.round(),
                top_left.y.round(),
                tint,
                DrawTextureParams { source: Some(crate::render::inset(src)), dest_size: Some(size), rotation: l.rotation.to_radians(), ..Default::default() },
            );
            if mat.is_some() {
                gl_use_default_material();
            }
        }
    }

    /// For `Game::status()`: `layer:sheet@x,y:alpha` (`>` while moving).
    pub(super) fn pictures_status(&self) -> Vec<String> {
        self.world
            .pictures
            .iter()
            .map(|(k, p)| format!("{k}:{}@{:.0},{:.0}:{:.2}{}", p.sheet, p.look.x, p.look.y, p.look.opacity, if p.moving.is_some() { ">" } else { "" }))
            .collect()
    }
}
