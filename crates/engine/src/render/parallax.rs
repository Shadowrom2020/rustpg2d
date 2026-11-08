//! Parallax layers of a map (`Map::parallax`): images moving at their own speed behind the
//! tiles or over the map. Shared by the game and the editor's preview; drawn in world space
//! with the map's camera set.

use macroquad::prelude::*;

use super::Assets;
use crate::map::ParallaxLayer;

/// Draws the layers that are `above` the map (or behind it) for a camera view, `time` seconds
/// in (drift). Layers whose sheet is missing are skipped.
pub fn draw(assets: &Assets, layers: &[ParallaxLayer], above: bool, view: Rect, time: f32) {
    for l in layers.iter().filter(|l| l.above == above && l.alpha > 0.0) {
        let Some(sheet) = assets.sheets.get(&l.sheet) else { continue };
        let tex = &sheet.texture;
        let scale = l.scale.max(0.01);
        let (w, h) = (tex.width() * scale, tex.height() * scale);
        let o = l.origin([view.x, view.y], time);
        let (x0, nx) = ParallaxLayer::copies(o[0], w, l.repeat[0], view.x, view.right());
        let (y0, ny) = ParallaxLayer::copies(o[1], h, l.repeat[1], view.y, view.bottom());
        let tint = Color::new(1.0, 1.0, 1.0, l.alpha.min(1.0));
        for j in 0..ny {
            for i in 0..nx {
                let (x, y) = (x0 + i as f32 * w, y0 + j as f32 * h);
                if Rect::new(x, y, w, h).overlaps(&view) {
                    draw_texture_ex(tex, x, y, tint, DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() });
                }
            }
        }
    }
}

/// How many of the layers are drawn on that side (`status()`).
pub fn count(assets: &Assets, layers: &[ParallaxLayer], above: bool) -> usize {
    layers.iter().filter(|l| l.above == above && l.alpha > 0.0 && assets.sheets.contains_key(&l.sheet)).count()
}
