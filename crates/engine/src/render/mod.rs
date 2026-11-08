//! Rendering helpers shared by the game and the editor.
//!
//! Tile layers are drawn tile-by-tile for the visible area only (macroquad batches
//! quads that share a texture). When zoomed far out, an [`Overview`] texture with one
//! pixel per tile replaces the per-tile drawing, so even a 1024x1024 map renders fast.

pub mod character;
pub mod fish;
pub mod lighting;
pub mod recolor;
pub mod parallax;
pub mod weather;

use std::cell::RefCell;
use std::collections::HashMap;

use macroquad::prelude::*;

use crate::defs::{SpriteRef, TileAnim};
use crate::map::{Map, TileLayer};
use crate::project::Project;

pub struct TilesetTex {
    pub texture: Texture2D,
    pub image: Image,
    pub columns: u32,
    pub count: u32,
    /// Average color of each tile (for the overview).
    pub colors: Vec<Color>,
    pub solid: Vec<bool>,
    /// Per tile index: animation (if any).
    anim_of: Vec<Option<usize>>,
    pub anims: Vec<TileAnim>,
}

impl TilesetTex {
    /// Index actually drawn for `index` at time `t` (animated tiles cycle).
    #[inline]
    pub fn display_index(&self, index: u32, t: f64) -> u32 {
        match self.anim_of.get(index as usize) {
            Some(Some(a)) => self.anims[*a].frame_at(t),
            _ => index,
        }
    }

    pub fn set_animations(&mut self, anims: &[TileAnim]) {
        self.anims = anims.to_vec();
        self.anim_of = vec![None; self.count as usize];
        for (i, a) in self.anims.iter().enumerate() {
            if let Some(slot) = self.anim_of.get_mut(a.tile as usize) {
                *slot = Some(i);
            }
        }
    }

    pub fn src_rect(&self, index: u32, ts: f32) -> Rect {
        let c = index % self.columns.max(1);
        let r = index / self.columns.max(1);
        Rect::new(c as f32 * ts, r as f32 * ts, ts, ts)
    }
    pub fn is_solid(&self, index: u32) -> bool {
        self.solid.get(index as usize).copied().unwrap_or(false)
    }
}

pub struct SheetTex {
    pub texture: Texture2D,
    pub image: Image,
    pub frame_w: f32,
    pub frame_h: f32,
    pub columns: u32,
    pub count: u32,
}

impl SheetTex {
    pub fn src_rect(&self, frame: u32) -> Rect {
        let c = frame % self.columns.max(1);
        let r = frame / self.columns.max(1);
        Rect::new(c as f32 * self.frame_w, r as f32 * self.frame_h, self.frame_w, self.frame_h)
    }

    /// Source rect of a `w × h` cell block whose top-left cell is `frame`.
    pub fn region(&self, frame: u32, w: u32, h: u32) -> Rect {
        let r = self.src_rect(frame);
        Rect::new(r.x, r.y, self.frame_w * w.max(1) as f32, self.frame_h * h.max(1) as f32)
    }
}

pub struct Assets {
    pub tile_size: f32,
    pub tilesets: HashMap<String, TilesetTex>,
    pub sheets: HashMap<String, SheetTex>,
    pub errors: Vec<String>,
    /// Recolored copies of body / hair sheets (layered characters), made on first use.
    recolored: RefCell<HashMap<ColorKey, Texture2D>>,
}

/// A sheet with optional skin / hair recoloring (cache key).
pub type ColorKey = (String, Option<[u8; 3]>, Option<[u8; 3]>);

/// Magenta/black checkerboard used when an image is missing.
fn placeholder(w: u16, h: u16) -> Image {
    let mut img = Image::gen_image_color(w, h, MAGENTA);
    for y in 0..h as u32 {
        for x in 0..w as u32 {
            if ((x / 4) + (y / 4)) % 2 == 0 {
                img.set_pixel(x, y, BLACK);
            }
        }
    }
    img
}

fn load_image(project: &Project, rel: &str, errors: &mut Vec<String>) -> Image {
    let path = project.path(rel);
    match crate::sys::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|b| Image::from_file_with_format(&b, None).map_err(|e| e.to_string()))
    {
        Ok(img) => img,
        Err(e) => {
            errors.push(format!("{}: {e}", path.display()));
            placeholder(64, 64)
        }
    }
}

fn texture(img: &Image) -> Texture2D {
    let t = Texture2D::from_image(img);
    t.set_filter(FilterMode::Nearest);
    t
}

fn average_color(img: &Image, r: Rect) -> Color {
    let (mut acc, mut n) = ([0f32; 3], 0f32);
    for y in r.y as u32..(r.y + r.h) as u32 {
        for x in r.x as u32..(r.x + r.w) as u32 {
            if x >= img.width as u32 || y >= img.height as u32 {
                continue;
            }
            let c = img.get_pixel(x, y);
            if c.a > 0.1 {
                acc[0] += c.r;
                acc[1] += c.g;
                acc[2] += c.b;
                n += 1.0;
            }
        }
    }
    if n == 0.0 {
        Color::new(0.0, 0.0, 0.0, 0.0)
    } else {
        Color::new(acc[0] / n, acc[1] / n, acc[2] / n, 1.0)
    }
}

impl Assets {
    /// No images (tests).
    pub fn empty(tile_size: f32) -> Assets {
        Assets { tile_size, tilesets: HashMap::new(), sheets: HashMap::new(), errors: Vec::new(), recolored: RefCell::default() }
    }

    /// Loads all tileset and sprite sheet images. Needs a macroquad window.
    pub fn load(project: &Project) -> Assets {
        let ts = crate::project::TILE_SIZE as f32;
        let mut errors = Vec::new();
        let mut tilesets = HashMap::new();
        for def in &project.db.tilesets {
            let image = load_image(project, &def.image, &mut errors);
            let columns = (image.width as u32 / ts as u32).max(1);
            let rows = (image.height as u32 / ts as u32).max(1);
            let count = columns * rows;
            let mut t = TilesetTex {
                texture: texture(&image),
                columns,
                count,
                colors: Vec::with_capacity(count as usize),
                solid: vec![false; count as usize],
                anim_of: Vec::new(),
                anims: Vec::new(),
                image,
            };
            t.set_animations(&def.animations);
            for i in 0..count {
                let c = average_color(&t.image, t.src_rect(i, ts));
                t.colors.push(c);
            }
            for &s in &def.solid {
                if let Some(v) = t.solid.get_mut(s as usize) {
                    *v = true;
                }
            }
            tilesets.insert(def.id.clone(), t);
        }
        let mut sheets = HashMap::new();
        for def in &project.db.sheets {
            let image = load_image(project, &def.image, &mut errors);
            let (fw, fh) = (def.frame_w.max(1), def.frame_h.max(1));
            let columns = (image.width as u32 / fw).max(1);
            let rows = (image.height as u32 / fh).max(1);
            sheets.insert(
                def.id.clone(),
                SheetTex {
                    texture: texture(&image),
                    image,
                    frame_w: fw as f32,
                    frame_h: fh as f32,
                    columns,
                    count: columns * rows,
                },
            );
        }
        for e in &errors {
            eprintln!("[assets] {e}");
        }
        Assets { tile_size: ts, tilesets, sheets, errors, recolored: RefCell::default() }
    }

    /// Tileset textures for each slot of a map.
    pub fn map_tilesets<'a>(&'a self, map: &Map) -> Vec<Option<&'a TilesetTex>> {
        map.tilesets.iter().map(|id| self.tilesets.get(id)).collect()
    }

    /// Solidity grid of a map using tileset flags and the map's overrides.
    pub fn compute_solid(&self, map: &Map) -> Vec<bool> {
        let sets = self.map_tilesets(map);
        map.compute_solid(|slot, i| sets.get(slot).copied().flatten().is_some_and(|t| t.is_solid(i)))
    }

    /// Draws a sheet frame with its bottom-center at `feet`.
    pub fn draw_frame(&self, sheet: &str, frame: u32, feet: Vec2, tint: Color) -> bool {
        let Some(s) = self.sheets.get(sheet) else { return false };
        let pos = vec2((feet.x - s.frame_w / 2.0).round(), (feet.y - s.frame_h).round());
        draw_texture_ex(
            &s.texture,
            pos.x,
            pos.y,
            tint,
            DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(s.src_rect(frame).size()), ..Default::default() },
        );
        true
    }

    /// A sheet's texture with skin / hair reference colors replaced (see [`recolor`]).
    fn colored(&self, sheet: &str, skin: Option<[u8; 3]>, hair: Option<[u8; 3]>) -> Option<(Texture2D, &SheetTex)> {
        let s = self.sheets.get(sheet)?;
        if skin.is_none() && hair.is_none() {
            return Some((s.texture.clone(), s));
        }
        let mut cache = self.recolored.borrow_mut();
        let tex = cache.entry((sheet.to_string(), skin, hair)).or_insert_with(|| {
            let mut img = s.image.clone();
            recolor::recolor(&mut img.bytes, skin, hair);
            texture(&img)
        });
        Some((tex.clone(), s))
    }

    /// Draws a frame of a body / hair sheet recolored with `skin` / `hair`, bottom-center at `feet`.
    pub fn draw_frame_colored(&self, sheet: &str, frame: u32, feet: Vec2, tint: Color, skin: Option<[u8; 3]>, hair: Option<[u8; 3]>) -> bool {
        let Some((tex, s)) = self.colored(sheet, skin, hair) else { return false };
        let pos = vec2((feet.x - s.frame_w / 2.0).round(), (feet.y - s.frame_h).round());
        draw_texture_ex(&tex, pos.x, pos.y, tint, DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(s.src_rect(frame).size()), ..Default::default() });
        true
    }

    /// Like [`draw_frame_colored`](Self::draw_frame_colored), scaled into `dest` (portraits).
    /// Like [`draw_frame_colored`](Self::draw_frame_colored), centered at `center` and rotated
    /// by `angle` radians (clockwise on screen): characters lying in a sideways bed.
    pub fn draw_frame_colored_rotated(&self, sheet: &str, frame: u32, center: Vec2, angle: f32, (skin, hair): (Option<[u8; 3]>, Option<[u8; 3]>), tint: Color) -> bool {
        let Some((tex, s)) = self.colored(sheet, skin, hair) else { return false };
        let pos = vec2((center.x - s.frame_w / 2.0).round(), (center.y - s.frame_h / 2.0).round());
        draw_texture_ex(&tex, pos.x, pos.y, tint, DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(s.src_rect(frame).size()), rotation: angle, ..Default::default() });
        true
    }

    /// Draws part of a frame (`part`: fractions of the frame, e.g. the top half) into `dest`
    /// (portraits: head and shoulders).
    pub fn draw_frame_part_in(&self, sheet: &str, frame: u32, part: Rect, dest: Rect, skin: Option<[u8; 3]>, hair: Option<[u8; 3]>) -> bool {
        let Some((tex, s)) = self.colored(sheet, skin, hair) else { return false };
        let r = s.src_rect(frame);
        let src = Rect::new(r.x + part.x * r.w, r.y + part.y * r.h, part.w * r.w, part.h * r.h);
        draw_texture_ex(&tex, dest.x, dest.y, WHITE, DrawTextureParams { source: Some(inset(src)), dest_size: Some(dest.size()), ..Default::default() });
        true
    }

    pub fn draw_frame_colored_in(&self, sheet: &str, frame: u32, dest: Rect, skin: Option<[u8; 3]>, hair: Option<[u8; 3]>) -> bool {
        let Some((tex, s)) = self.colored(sheet, skin, hair) else { return false };
        draw_texture_ex(&tex, dest.x, dest.y, WHITE, DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(dest.size()), ..Default::default() });
        true
    }

    /// Draws a (possibly multi-cell) sprite with its bottom-center at `feet`.
    /// `extra_frame` shifts the top-left cell (prop states such as an open gate).
    pub fn draw_sprite(&self, sprite: &SpriteRef, extra_frame: u32, feet: Vec2, tint: Color) -> bool {
        let Some(s) = self.sheets.get(&sprite.sheet) else { return false };
        let (w, h) = sprite.cells();
        let src = s.region(sprite.frame + extra_frame, w, h);
        let pos = vec2((feet.x - src.w / 2.0).round(), (feet.y - src.h).round());
        draw_texture_ex(&s.texture, pos.x, pos.y, tint, DrawTextureParams { source: Some(inset(src)), dest_size: Some(src.size()), ..Default::default() });
        true
    }

    /// Pixel size of a sprite (one frame if the sheet is unknown: `fallback`).
    pub fn sprite_size(&self, sprite: &SpriteRef, fallback: f32) -> Vec2 {
        let (w, h) = sprite.cells();
        self.sheets
            .get(&sprite.sheet)
            .map_or(vec2(fallback, fallback), |s| vec2(s.frame_w * w as f32, s.frame_h * h as f32))
    }

    /// Draws a frame centered at `center`, rotated by `angle` radians.
    pub fn draw_frame_rotated(&self, sheet: &str, frame: u32, center: Vec2, angle: f32, tint: Color) -> bool {
        let Some(s) = self.sheets.get(sheet) else { return false };
        draw_texture_ex(
            &s.texture,
            center.x - s.frame_w / 2.0,
            center.y - s.frame_h / 2.0,
            tint,
            DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(s.src_rect(frame).size()), rotation: angle, ..Default::default() },
        );
        true
    }

    /// Draws a frame centered at `center`, rotated by `angle` radians and scaled by `scale`.
    pub fn draw_frame_scaled(&self, sheet: &str, frame: u32, center: Vec2, angle: f32, scale: f32, tint: Color) -> bool {
        let Some(s) = self.sheets.get(sheet) else { return false };
        let size = vec2(s.frame_w, s.frame_h) * scale;
        draw_texture_ex(
            &s.texture,
            center.x - size.x / 2.0,
            center.y - size.y / 2.0,
            tint,
            DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(size), rotation: angle, ..Default::default() },
        );
        true
    }

    /// Draws a frame scaled into `dest` (screen UI, icons).
    /// Draws a sprite (its whole cell block) scaled into `r` keeping its shape: centered,
    /// standing on the bottom edge. Returns false if the sheet is unknown.
    pub fn draw_sprite_fit(&self, sprite: &crate::defs::SpriteRef, r: Rect) -> bool {
        let Some(s) = self.sheets.get(&sprite.sheet) else { return false };
        let (w, h) = sprite.cells();
        let src = s.region(sprite.frame, w, h);
        let scale = (r.w / src.w).min(r.h / src.h);
        let (dw, dh) = (src.w * scale, src.h * scale);
        draw_texture_ex(&s.texture, (r.x + (r.w - dw) / 2.0).round(), (r.bottom() - dh).round(), WHITE, DrawTextureParams { source: Some(inset(src)), dest_size: Some(vec2(dw, dh)), ..Default::default() });
        true
    }

    pub fn draw_frame_in(&self, sheet: &str, frame: u32, dest: Rect, tint: Color) -> bool {
        let Some(s) = self.sheets.get(sheet) else { return false };
        draw_texture_ex(
            &s.texture,
            dest.x,
            dest.y,
            tint,
            DrawTextureParams {
                source: Some(inset(s.src_rect(frame))),
                dest_size: Some(vec2(dest.w, dest.h)),
                ..Default::default()
            },
        );
        true
    }
}

/// A source rectangle shrunk by a sliver of a texel on every side, for drawing. With nearest
/// filtering at fractional scales (the web game, zoom levels) a screen pixel can fall exactly on
/// a frame's or tile's border in its sheet and pick the neighbour's texel: thin lines of other
/// tiles along edges. 1/64 texel is invisible, but keeps every sample inside the frame. (Callers
/// keep the drawn size: `dest_size` is the original rectangle's.)
pub fn inset(r: Rect) -> Rect {
    const E: f32 = 1.0 / 64.0;
    Rect::new(r.x + E, r.y + E, (r.w - 2.0 * E).max(0.0), (r.h - 2.0 * E).max(0.0))
}

/// Number of tiles visible in a view (used to choose between per-tile and overview drawing).
pub fn visible_tiles(view: Rect, ts: f32) -> f32 {
    (view.w / ts) * (view.h / ts)
}

pub const OVERVIEW_THRESHOLD: f32 = 60_000.0;

/// Draws the visible part of a tile layer. `view` is in world pixels.
pub fn draw_layer(map: &Map, layer: &TileLayer, sets: &[Option<&TilesetTex>], ts: f32, view: Rect, tint: Color) {
    let x0 = (view.x / ts).floor().max(0.0) as u32;
    let y0 = (view.y / ts).floor().max(0.0) as u32;
    let x1 = ((view.x + view.w) / ts).ceil().clamp(0.0, map.width as f32) as u32;
    let y1 = ((view.y + view.h) / ts).ceil().clamp(0.0, map.height as f32) as u32;
    let chunk = crate::map::CHUNK_SIZE;
    let time = get_time();
    for y in y0..y1 {
        let mut x = x0;
        while x < x1 {
            if !layer.chunk_allocated(x, y) {
                x = (x / chunk + 1) * chunk;
                continue;
            }
            if let Some((slot, idx)) = layer.get(x, y).parts() {
                if let Some(Some(t)) = sets.get(slot) {
                    let idx = t.display_index(idx, time);
                    draw_texture_ex(
                        &t.texture,
                        x as f32 * ts,
                        y as f32 * ts,
                        tint,
                        DrawTextureParams {
                            source: Some(inset(t.src_rect(idx, ts))),
                            dest_size: Some(vec2(ts, ts)),
                            ..Default::default()
                        },
                    );
                }
            }
            x += 1;
        }
    }
}

/// One pixel per tile: the top-most visible tile's average color.
pub struct Overview {
    pub image: Image,
    /// Created on first draw, so overviews can be built without a GL context.
    texture: Option<Texture2D>,
    dirty: bool,
}

impl Overview {
    pub fn build(map: &Map, assets: &Assets) -> Overview {
        let mut image = Image::gen_image_color(map.width as u16, map.height as u16, Color::new(0., 0., 0., 0.));
        let sets = assets.map_tilesets(map);
        for y in 0..map.height {
            for x in 0..map.width {
                image.set_pixel(x, y, Self::color_at(map, &sets, x, y));
            }
        }
        Overview { image, texture: None, dirty: false }
    }

    fn color_at(map: &Map, sets: &[Option<&TilesetTex>], x: u32, y: u32) -> Color {
        for l in map.layers.iter().rev().filter(|l| l.visible) {
            if let Some((slot, idx)) = l.get(x, y).parts() {
                if let Some(Some(t)) = sets.get(slot) {
                    if let Some(c) = t.colors.get(idx as usize) {
                        if c.a > 0.0 {
                            return *c;
                        }
                    }
                }
            }
        }
        Color::new(0., 0., 0., 0.)
    }

    pub fn update_tile(&mut self, map: &Map, assets: &Assets, x: u32, y: u32) {
        let sets = assets.map_tilesets(map);
        self.image.set_pixel(x, y, Self::color_at(map, &sets, x, y));
        self.dirty = true;
    }

    fn texture(&mut self) -> &Texture2D {
        if self.texture.is_none() {
            self.texture = Some(texture(&self.image));
            self.dirty = false;
        }
        let t = self.texture.as_ref().unwrap();
        if self.dirty {
            t.update(&self.image);
            self.dirty = false;
        }
        t
    }

    pub fn draw(&mut self, map: &Map, ts: f32) {
        let t = self.texture();
        draw_texture_ex(
            t,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(map.width as f32 * ts, map.height as f32 * ts)),
                ..Default::default()
            },
        );
    }

    /// Draws the overview into a screen rect (minimap).
    pub fn draw_in(&mut self, dest: Rect) {
        let t = self.texture();
        draw_texture_ex(
            t,
            dest.x,
            dest.y,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(dest.w, dest.h)), ..Default::default() },
        );
    }
}

/// Pixel-perfect camera centered on `target` with an integer-ish `scale`.
pub fn pixel_camera(target: Vec2, scale: f32) -> Camera2D {
    let (w, h) = (screen_width(), screen_height());
    let snap = |v: f32| (v * scale).round() / scale;
    Camera2D {
        target: vec2(snap(target.x), snap(target.y)),
        zoom: vec2(2.0 * scale / w, 2.0 * scale / h),
        ..Default::default()
    }
}

/// World-space rect visible through a camera made by [`pixel_camera`].
pub fn camera_view(cam: &Camera2D) -> Rect {
    let a = cam.screen_to_world(vec2(0.0, 0.0));
    let b = cam.screen_to_world(vec2(screen_width(), screen_height()));
    Rect::new(a.x.min(b.x), a.y.min(b.y), (b.x - a.x).abs(), (b.y - a.y).abs())
}

/// Word-wraps `text` to `max_w` pixels for `draw_text`.
pub fn wrap_text(text: &str, font_size: u16, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if measure_text(&candidate, None, font_size, 1.0).width > max_w && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_keeps_samples_inside_the_frame() {
        let r = inset(Rect::new(32.0, 64.0, 32.0, 48.0));
        assert!(r.x > 32.0 && r.right() < 64.0 && r.y > 64.0 && r.bottom() < 112.0);
        // By far less than a texel: nothing visibly shrinks.
        assert!(r.w > 31.9 && r.h > 47.9);
        assert_eq!(inset(Rect::new(0.0, 0.0, 0.0, 0.0)).w, 0.0);
    }
}
