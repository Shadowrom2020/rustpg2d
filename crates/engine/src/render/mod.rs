//! Rendering helpers shared by the game and the editor.
//!
//! The painted ground is drawn by [`ground::GroundRenderer`], sprites by [`Assets`]. When zoomed
//! far out, an [`Overview`] texture with one pixel per tile stands in for the map (minimaps,
//! the editor), so even a 1024x1024 map renders fast.

pub mod iso;
pub mod character;
pub mod fish;
pub mod ground;
pub mod lighting;
pub mod model;
pub mod recolor;
pub mod parallax;
pub mod weather;

use std::collections::HashMap;

use macroquad::prelude::*;

use crate::defs::SpriteRef;
use crate::map::Map;
use crate::project::Project;

pub struct SheetTex {
    pub texture: Texture2D,
    pub image: Image,
    /// Frame size in image pixels (where frames are cut).
    pub frame_w: f32,
    pub frame_h: f32,
    pub columns: u32,
    pub count: u32,
    /// Image pixels per world pixel (`SheetDef::scale`): frames are drawn at their size divided by it.
    pub scale: f32,
}

impl SheetTex {
    /// A frame's size in the world (pixels at the 32-px tile size).
    pub fn world_size(&self) -> Vec2 {
        vec2(self.frame_w, self.frame_h) / self.scale
    }

    /// The world size of a source rectangle of the image.
    pub fn world_of(&self, src: Rect) -> Vec2 {
        src.size() / self.scale
    }

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
    pub sheets: HashMap<String, SheetTex>,
    /// Ground materials' textures (CPU images: the ground renderer packs them into an atlas)
    /// and their average colours (overviews).
    pub materials: HashMap<String, Image>,
    pub material_colors: HashMap<String, Color>,
    /// Animation frames of each material's texture (side by side; 1 = still): pictures of a
    /// material show the first.
    pub material_frames: HashMap<String, u32>,
    /// Changes with every load (renderers rebuild what they made from the images).
    pub generation: u64,
    pub errors: Vec<String>,
    /// 3D character models and their rendered poses.
    pub models: model::Models,
}

/// The colours chosen for a sheet's colour features, in feature order (`None` = as drawn).
pub type Colors = [Option<[u8; 3]>];

/// A sheet with its features' colours (cache key).
pub type ColorKey = (String, Vec<Option<[u8; 3]>>);

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

/// How dark a picture's ground shadow is (black, this clear).
const PICTURE_SHADOW: f32 = 0.5;

impl Assets {
    /// No images (tests).
    pub fn empty(tile_size: f32) -> Assets {
        Assets { tile_size, sheets: HashMap::new(), materials: HashMap::new(), material_colors: HashMap::new(), material_frames: HashMap::new(), generation: 0, errors: Vec::new(), models: model::Models::default() }
    }

    /// Loads all sprite sheet and material images. Needs a macroquad window.
    pub fn load(project: &Project) -> Assets {
        let ts = crate::project::TILE_SIZE as f32;
        let mut errors = Vec::new();
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
                    scale: def.scale.clamp(1, 4) as f32,
                },
            );
        }
        let mut materials = HashMap::new();
        let mut material_colors = HashMap::new();
        let mut material_frames = HashMap::new();
        for def in &project.db.materials {
            if def.texture.is_empty() {
                continue;
            }
            let image = load_image(project, &def.texture, &mut errors);
            let (fw, fh) = def.frame_size(image.width as u32, image.height as u32);
            material_colors.insert(def.id.clone(), average_color(&image, Rect::new(0.0, 0.0, fw.max(1) as f32, fh.max(1) as f32)));
            material_frames.insert(def.id.clone(), def.frames.max(1) as u32);
            materials.insert(def.id.clone(), image);
        }
        for e in &errors {
            eprintln!("[assets] {e}");
        }
        static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let models = model::Models::load(&project.db, &|rel| crate::sys::read(&project.path(rel)));
        models.set_atlas_mb(project.config.model_atlas_mb);
        errors.extend(models.errors.iter().cloned());
        Assets { tile_size: ts, sheets, materials, material_colors, material_frames, generation, errors, models }
    }

    /// The picture sprite `sp` shows and its size in world pixels (buildings' textures).
    pub fn sprite_picture(&self, sp: &SpriteRef) -> Option<crate::building::Picture> {
        let s = self.sheets.get(&sp.sheet)?;
        let (cw, ch) = sp.cells();
        let r = s.region(sp.frame, cw, ch);
        let (x0, y0, w, h) = (r.x as u32, r.y as u32, r.w as u32, r.h as u32);
        if w == 0 || h == 0 || x0 + w > s.image.width as u32 || y0 + h > s.image.height as u32 {
            return None;
        }
        let mut out = Image::gen_image_color(w as u16, h as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        for y in 0..h {
            for x in 0..w {
                out.set_pixel(x, y, s.image.get_pixel(x0 + x, y0 + y));
            }
        }
        Some((out, vec2(w as f32, h as f32) / s.scale.max(1.0)))
    }

    /// Builds the building props' models again from `db` (a shape changed in the editor).
    pub fn rebuild_buildings(&mut self, db: &crate::defs::Database) {
        let mut models = std::mem::take(&mut self.models);
        models.rebuild_buildings(db, &|sp| self.sprite_picture(sp));
        self.models = models;
    }

    /// Builds building prop `p`'s model again (its shape is being edited), and those joined to it.
    pub fn rebuild_building(&mut self, p: &crate::defs::PropDef, db: &crate::defs::Database) {
        let mut models = std::mem::take(&mut self.models);
        models.rebuild_building(p, db, &|sp| self.sprite_picture(sp));
        self.models = models;
    }

    /// Overview colours of a map's ground by cell value (0 = none: transparent).
    pub fn map_material_colors(&self, map: &Map) -> Vec<Color> {
        std::iter::once(Color::new(0., 0., 0., 0.)).chain(map.materials.iter().map(|id| self.material_colors.get(id).copied().unwrap_or(MAGENTA))).collect()
    }

    /// The average colour of a sprite's picture (transparent when its sheet isn't loaded).
    pub fn sprite_color(&self, sprite: &SpriteRef) -> Color {
        let Some(s) = self.sheets.get(&sprite.sheet) else { return Color::new(0., 0., 0., 0.) };
        let (w, h) = sprite.cells();
        average_color(&s.image, s.region(sprite.frame, w, h))
    }

    /// Draws a sheet frame with its bottom-center at `feet`.
    pub fn draw_frame(&self, sheet: &str, frame: u32, feet: Vec2, tint: Color) -> bool {
        let Some(s) = self.sheets.get(sheet) else { return false };
        draw_at_feet(&s.texture, s, s.src_rect(frame), feet, tint);
        true
    }

    /// Draws a (possibly multi-cell) sprite with its bottom-center at `feet`.
    /// `extra_frame` shifts the top-left cell (prop states such as an open gate).
    pub fn draw_sprite(&self, sprite: &SpriteRef, extra_frame: u32, feet: Vec2, tint: Color) -> bool {
        let Some(s) = self.sheets.get(&sprite.sheet) else { return false };
        let (w, h) = sprite.cells();
        draw_at_feet(&s.texture, s, s.region(sprite.frame + extra_frame, w, h), feet, tint);
        true
    }

    /// [`Assets::draw_sprite`] mirrored left to right when `flip` (decorations).
    pub fn draw_sprite_flipped(&self, sprite: &SpriteRef, feet: Vec2, tint: Color, flip: bool) -> bool {
        let Some(s) = self.sheets.get(&sprite.sheet) else { return false };
        let (w, h) = sprite.cells();
        let src = s.region(sprite.frame, w, h);
        let size = s.world_of(src);
        let pos = vec2((feet.x - size.x / 2.0).round(), (feet.y - size.y).round());
        draw_texture_ex(&s.texture, pos.x, pos.y, tint, DrawTextureParams { source: Some(inset(src)), dest_size: Some(size), flip_x: flip, ..Default::default() });
        true
    }

    /// Draws prop `p` in state `state` facing `facing` (props that turn: a 3D model or pictures
    /// from four sides) standing at `feet` (the hitbox's bottom middle), mirrored when `flip`;
    /// `frame` frames further into its sheet (pictures). A model stands on the middle of its
    /// hitbox: `iso` (drawn through an upright camera at `feet`) puts that ground offset where
    /// the isometric view shows it. False when nothing could be drawn (a model not rendered yet,
    /// a missing sheet).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_prop(&self, db: &crate::defs::Database, p: &crate::defs::PropDef, state: usize, facing: crate::entity::Facing, frame: u32, feet: Vec2, flip: bool, tint: Color, time: f64, iso: bool) -> bool {
        let model = p.model_at(state);
        if !model.is_empty() && self.models.has(&model) {
            let look = crate::defs::Appearance { model: model.to_string(), ..Default::default() };
            let gear = model::Gear::default();
            let req = model::PoseReq { look: &look, gear: &gear, anim: "idle", time: 0.0, once: false, facing, turn: 0, bust: false };
            let Some(cell) = self.models.pose(db, &self.sheets, &req) else { return false };
            let off = vec2(0.0, -p.hitbox_facing(facing)[1] / 2.0);
            let at = feet + if iso { iso::lay(off) } else { off };
            self.models.draw_flipped(&cell, at, 1.0, tint, flip);
            return true;
        }
        let sp = p.picture_facing(state, facing, time);
        if frame == 0 {
            self.draw_sprite_flipped(&sp, feet, tint, flip)
        } else {
            self.draw_sprite_flipped(&SpriteRef { frame: sp.frame + frame, ..sp.into_owned() }, feet, tint, flip)
        }
    }

    /// Draws the ground shadow of prop `p` (state `state`, facing `facing`) standing at `feet`,
    /// when it is drawn as a picture (models and blocks have their own; flat ones and those
    /// without [`crate::defs::PropDef::shadow`] cast none): its picture sheared along the sun
    /// like a 3D prop's shadow, dark and half clear. In the coordinates of its upright drawing
    /// (the caller sets that camera); before the things standing there are drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_prop_shadow(&self, p: &crate::defs::PropDef, state: usize, facing: crate::entity::Facing, feet: Vec2, flip: bool, time: f64, alpha: f32) {
        if !p.shadow || p.flat || p.block > 0.0 || p.building.is_some() || (!p.model_at(state).is_empty() && self.models.has(&p.model_at(state))) {
            return;
        }
        let sp = p.picture_facing(state, facing, time);
        let Some(s) = self.sheets.get(&sp.sheet) else { return };
        let (cw, ch) = sp.cells();
        let src = inset(s.region(sp.frame, cw, ch));
        let size = s.world_of(src);
        let (tw, th) = (s.texture.width().max(1.0), s.texture.height().max(1.0));
        let (u0, u1) = if flip { ((src.x + src.w) / tw, src.x / tw) } else { (src.x / tw, (src.x + src.w) / tw) };
        let (v0, v1) = (src.y / th, (src.y + src.h) / th);
        // A point `h` above the feet in the picture lands `h * SHEAR` to the side on the ground.
        let (w, h) = (size.x, size.y);
        let top = feet + vec2(model::SHADOW_SHEAR.x * h, model::SHADOW_SHEAR.y * h);
        let c = Color::new(0.0, 0.0, 0.0, alpha * PICTURE_SHADOW);
        let v = |pos: Vec2, u: f32, vv: f32| Vertex::new(pos.x, pos.y, 0.0, u, vv, c);
        let mesh = Mesh {
            vertices: vec![v(feet + vec2(-w / 2.0, 0.0), u0, v1), v(feet + vec2(w / 2.0, 0.0), u1, v1), v(top + vec2(w / 2.0, 0.0), u1, v0), v(top + vec2(-w / 2.0, 0.0), u0, v0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(s.texture.clone()),
        };
        draw_mesh(&mesh);
    }

    /// Draws prop `p` (state `state`, facing `facing`) standing at `feet` as a block `height`
    /// world pixels high on its hitbox ([`crate::defs::PropDef::block`]), its picture on the
    /// faces, through view `v` in window pixels (the caller sets the default camera).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_prop_block(&self, v: &iso::View, p: &crate::defs::PropDef, state: usize, facing: crate::entity::Facing, feet: Vec2, height: f32, tint: Color) {
        let [w, h] = p.hitbox_facing(facing);
        let r = Rect::new(feet.x - w / 2.0, feet.y - h, w, h);
        let sp = p.picture_facing(state, facing, 0.0);
        let tex = self.sheets.get(&sp.sheet).map(|s| {
            let (cw, ch) = sp.cells();
            (&s.texture, inset(s.region(sp.frame, cw, ch)))
        });
        iso::draw_block(v, r, height, tex, tint);
    }

    /// Where prop `p` (state `state`, facing `facing`) standing at `feet` is drawn, in the
    /// coordinates of its upright drawing (a picture: its frame above the feet; a model in the
    /// isometric view: the box its footprint and height take on the screen): for clicking on it.
    pub fn prop_rect(&self, p: &crate::defs::PropDef, state: usize, facing: crate::entity::Facing, feet: Vec2, ts: f32, iso: bool) -> Rect {
        let model = p.model_at(state);
        if iso && !model.is_empty() && self.models.has(&model) {
            let [w, h] = p.hitbox_facing(facing);
            let height = self.models.def(&model).map_or(ts, |d| d.height);
            let c = feet + iso::lay(vec2(0.0, -h / 2.0));
            let (half, below) = ((w + h) / (2.0 * iso::UPRIGHT), (w + h) / (4.0 * iso::UPRIGHT));
            let above = height * 30f32.to_radians().cos() + below;
            return Rect::new(c.x - half, c.y - above, 2.0 * half, above + below);
        }
        let size = self.sprite_size(&p.picture_facing(state, facing, 0.0), ts);
        Rect::new(feet.x - size.x / 2.0, feet.y - size.y, size.x, size.y)
    }

    /// Draws something held in a hand (`l`: its sprite, its grip point put at `at`, turned by its
    /// rotation, mirrored top to bottom first when `flip`); `scale` times its world size.
    pub fn draw_held_sprite(&self, l: &crate::hands::HeldLayer, at: Vec2, scale: f32, tint: Color) -> bool {
        let Some(s) = self.sheets.get(&l.sprite.sheet) else { return false };
        let (w, h) = l.sprite.cells();
        let src = s.region(l.sprite.frame, w, h);
        let size = s.world_of(src) * scale;
        let pos = at - vec2(l.grip[0] * size.x, l.grip[1] * size.y);
        let params = DrawTextureParams { source: Some(inset(src)), dest_size: Some(size), rotation: l.rotation.to_radians(), pivot: Some(at), flip_y: l.flip, ..Default::default() };
        draw_texture_ex(&s.texture, pos.x, pos.y, tint, params);
        true
    }

    /// World size of a sprite (one frame if the sheet is unknown: `fallback`).
    pub fn sprite_size(&self, sprite: &SpriteRef, fallback: f32) -> Vec2 {
        let (w, h) = sprite.cells();
        self.sheets.get(&sprite.sheet).map_or(vec2(fallback, fallback), |s| s.world_size() * vec2(w as f32, h as f32))
    }

    /// Draws a frame centered at `center`, rotated by `angle` radians.
    pub fn draw_frame_rotated(&self, sheet: &str, frame: u32, center: Vec2, angle: f32, tint: Color) -> bool {
        self.draw_frame_scaled(sheet, frame, center, angle, 1.0, tint)
    }

    /// Draws a frame centered at `center`, rotated by `angle` radians and scaled by `scale`.
    pub fn draw_frame_scaled(&self, sheet: &str, frame: u32, center: Vec2, angle: f32, scale: f32, tint: Color) -> bool {
        let Some(s) = self.sheets.get(sheet) else { return false };
        let size = s.world_size() * scale;
        draw_texture_ex(
            &s.texture,
            center.x - size.x / 2.0,
            center.y - size.y / 2.0,
            tint,
            DrawTextureParams { source: Some(inset(s.src_rect(frame))), dest_size: Some(size), rotation: angle, ..Default::default() },
        );
        true
    }

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

    /// Draws a frame scaled into `dest` (screen UI, icons).
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

/// Draws `src` of a sheet's texture at its world size, bottom-center at `feet` (pixel-snapped).
fn draw_at_feet(tex: &Texture2D, s: &SheetTex, src: Rect, feet: Vec2, tint: Color) {
    let size = s.world_of(src);
    let pos = vec2((feet.x - size.x / 2.0).round(), (feet.y - size.y).round());
    draw_texture_ex(tex, pos.x, pos.y, tint, DrawTextureParams { source: Some(inset(src)), dest_size: Some(size), ..Default::default() });
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

/// Draws the frame `src` of `tex` stretched over `dest` as a nine-slice: corners of `corner`
/// picture pixels at `scale` screen pixels each, edges and middle stretched (or repeated with
/// `tile`). Positions snap to whole pixels (UI skins, `runtime/skin.rs`).
pub fn draw_nine_slice(tex: &Texture2D, src: Rect, dest: Rect, corner: f32, scale: f32, tile: bool, tint: Color) {
    let dest = Rect::new(dest.x.round(), dest.y.round(), dest.w.round(), dest.h.round());
    let scale = scale.max(1.0);
    let piece = |s: [f32; 4], d: [f32; 4]| {
        draw_texture_ex(tex, d[0], d[1], tint, DrawTextureParams { source: Some(inset(Rect::new(s[0], s[1], s[2], s[3]))), dest_size: Some(vec2(d[2], d[3])), ..Default::default() });
    };
    for (s, d) in crate::skin::nine_parts([src.x, src.y, src.w, src.h], [dest.x, dest.y, dest.w, dest.h], corner, scale) {
        // Repeat along a side that is longer on screen than the piece at its scale.
        let rep_x = tile && d[2] > s[2] * scale + 0.5;
        let rep_y = tile && d[3] > s[3] * scale + 0.5;
        let xs = if rep_x { crate::skin::repeats(s[2], d[2], scale) } else { vec![(s[2], 0.0, d[2])] };
        let ys = if rep_y { crate::skin::repeats(s[3], d[3], scale) } else { vec![(s[3], 0.0, d[3])] };
        for (sh, oy, dh) in &ys {
            for (sw, ox, dw) in &xs {
                piece([s[0], s[1], *sw, *sh], [d[0] + ox, d[1] + oy, *dw, *dh]);
            }
        }
    }
}

/// Number of tiles visible in a view (used to choose between per-tile and overview drawing).
pub fn visible_tiles(view: Rect, ts: f32) -> f32 {
    (view.w / ts) * (view.h / ts)
}

pub const OVERVIEW_THRESHOLD: f32 = 60_000.0;

/// One pixel per tile: the ground's colour, with the average colour of the decorations
/// standing on the tile (their feet) over it.
pub struct Overview {
    pub image: Image,
    /// Created on first draw, so overviews can be built without a GL context.
    texture: Option<Texture2D>,
    dirty: bool,
    /// Tiles with decorations: their colour (tile index -> colour).
    decor: HashMap<u32, Color>,
    /// The decorations changed since their colours were taken: [`Overview::refresh_decor`].
    pub decor_stale: bool,
}

impl Overview {
    /// The overview of the ground; the decorations come with [`Overview::refresh_decor`].
    pub fn build(map: &Map, assets: &Assets) -> Overview {
        let mut image = Image::gen_image_color(map.width as u16, map.height as u16, Color::new(0., 0., 0., 0.));
        let mats = assets.map_material_colors(map);
        let decor = HashMap::new();
        for y in 0..map.height {
            for x in 0..map.width {
                image.set_pixel(x, y, Self::color_at(map, &mats, &decor, x, y));
            }
        }
        Overview { image, texture: None, dirty: false, decor, decor_stale: !map.decor.is_empty() }
    }

    /// The overview with its decorations.
    pub fn with_decor(map: &Map, assets: &Assets, db: &crate::defs::Database) -> Overview {
        let mut o = Self::build(map, assets);
        o.refresh_decor(map, assets, db);
        o
    }

    /// The colour of the decorations on each tile (by their feet; the last one drawn wins).
    fn decor_colors(map: &Map, assets: &Assets, db: &crate::defs::Database) -> HashMap<u32, Color> {
        let looks: Vec<Color> = map.decor_defs.iter().map(|id| db.prop(id).map_or(Color::new(0., 0., 0., 0.), |p| assets.sprite_color(p.state_look(0).0))).collect();
        let ts = crate::project::TILE_SIZE as i32;
        let mut out = HashMap::new();
        let mut order: Vec<&crate::decor::Decor> = map.decor.iter().collect();
        order.sort_by_key(|d| (d.layer as u8, d.y));
        for d in order {
            let (tx, ty) = (d.x.div_euclid(ts), (d.y - 1).div_euclid(ts));
            if let Some(c) = looks.get(d.def as usize).filter(|c| c.a > 0.0) {
                if map.in_bounds(tx, ty) {
                    out.insert(ty as u32 * map.width + tx as u32, *c);
                }
            }
        }
        out
    }

    fn color_at(map: &Map, mats: &[Color], decor: &HashMap<u32, Color>, x: u32, y: u32) -> Color {
        if let Some(c) = decor.get(&(y * map.width + x)) {
            return *c;
        }
        // The painted ground: the material in the middle of the tile.
        let mid = (crate::ground::CELLS / 2) as i32;
        let v = map.ground.get((x * crate::ground::CELLS) as i32 + mid, (y * crate::ground::CELLS) as i32 + mid);
        mats.get(v as usize).copied().unwrap_or(Color::new(0., 0., 0., 0.))
    }

    /// The ground of tile (x, y) changed.
    pub fn update_tile(&mut self, map: &Map, assets: &Assets, x: u32, y: u32) {
        let mats = assets.map_material_colors(map);
        self.image.set_pixel(x, y, Self::color_at(map, &mats, &self.decor, x, y));
        self.dirty = true;
    }

    /// Takes the decorations' colours again (when [`Overview::decor_stale`]): the tiles they
    /// stood or stand on change.
    pub fn refresh_decor(&mut self, map: &Map, assets: &Assets, db: &crate::defs::Database) {
        let decor = Self::decor_colors(map, assets, db);
        let mats = assets.map_material_colors(map);
        let old = std::mem::take(&mut self.decor);
        self.decor = decor;
        for &i in old.keys().chain(self.decor.keys()) {
            let (x, y) = (i % map.width.max(1), i / map.width.max(1));
            if y < map.height && x < self.image.width as u32 && y < self.image.height as u32 {
                self.image.set_pixel(x, y, Self::color_at(map, &mats, &self.decor, x, y));
            }
        }
        self.decor_stale = false;
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
    // (All four corners: the isometric camera is turned.)
    let (w, h) = (screen_width(), screen_height());
    let ps = [vec2(0.0, 0.0), vec2(w, 0.0), vec2(0.0, h), vec2(w, h)].map(|p| cam.screen_to_world(p));
    let (mut lo, mut hi) = (ps[0], ps[0]);
    for p in ps {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
}

/// Wraps `text` to `max_w` pixels for `draw_text` (measured in the current font role's font; at
/// spaces, and between Chinese and Japanese characters: `runtime::text::wrap`).
pub fn wrap_text(text: &str, font_size: u16, max_w: f32) -> Vec<String> {
    crate::runtime::text::wrap(text, font_size as f32, max_w)
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
