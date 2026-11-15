//! Draws a map's painted ground ([`crate::ground`]) with a shader: each chunk's cells go to the
//! GPU as a small texture (the cell value in the red channel), the materials' textures as one
//! atlas, and the shader shapes the edges per material every frame (crisp pixel steps made
//! ragged by noise, or soft fades; priority and rims where materials meet; flowing water and
//! animation frames). No
//! big pictures are baked: editing a cell re-uploads one 130 x 130 texture.
//!
//! GLSL ES 100 (the web build): no texture arrays, the material table is a texture (WebGL 1
//! can't index uniform arrays with values read from textures).

use std::collections::HashMap;

use macroquad::prelude::*;

use crate::defs::Database;
use crate::ground::{Edge, MaterialDef, CELL, CHUNK_CELLS, MAX_MATERIALS};
use crate::map::Map;

/// Side of a cell texture: a chunk's cells and a border of its neighbours' cells.
const SIDE: u32 = CHUNK_CELLS + 2;
/// A material's slot in the atlas (its texture, up to this size; animation frames side by side
/// and in rows as many as fit).
const SLOT: u32 = 256;
const ATLAS_COLS: u32 = 8;
/// Width of the material table texture (cell values 0..=MAX_MATERIALS).
const INFO_W: u32 = 128;
/// Cell textures kept (least recently drawn ones go first).
const MAX_CHUNKS: usize = 256;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying highp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

const FRAGMENT: &str = r#"#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
varying highp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
uniform sampler2D Atlas;
uniform sampler2D Info;
uniform sampler2D Noise;
uniform vec2 Origin;
uniform vec2 AtlasPx;
// macroquad's clock: (seconds, sin, cos, 0).
uniform vec4 _Time;

const float CELL = 8.0;
const float CELLS = 128.0;
const float SIDE = 130.0;

float cellAt(vec2 c) {
    c = clamp(c, vec2(-1.0), vec2(CELLS));
    return floor(texture2D(Texture, (c + 1.5) / SIDE).r * 255.0 + 0.5);
}

vec4 info(float v, float row) {
    return texture2D(Info, vec2((v + 0.5) / 128.0, (row + 0.5) / 4.0));
}

float byte(float x) {
    return floor(x * 255.0 + 0.5);
}

// Smooth value noise (two channels, -0.5..0.5), one noise texel per `CELL` pixels.
vec2 noise(vec2 p) {
    vec2 f = floor(p);
    vec2 t = p - f;
    t = t * t * (3.0 - 2.0 * t);
    vec2 a = texture2D(Noise, (mod(f, 64.0) + 0.5) / 64.0).rg;
    vec2 b = texture2D(Noise, (mod(f + vec2(1.0, 0.0), 64.0) + 0.5) / 64.0).rg;
    vec2 c = texture2D(Noise, (mod(f + vec2(0.0, 1.0), 64.0) + 0.5) / 64.0).rg;
    vec2 d = texture2D(Noise, (mod(f + vec2(1.0, 1.0), 64.0) + 0.5) / 64.0).rg;
    return mix(mix(a, b, t.x), mix(c, d, t.x), t.y) - 0.5;
}

// How much of the area around chunk-local point p (pixels) holds value v: bilinear over the
// cell centres (0.5 = the edge).
float cover(float v, vec2 p) {
    vec2 c = p / CELL - 0.5;
    vec2 f = floor(c);
    vec2 t = c - f;
    float a = cellAt(f) == v ? 1.0 : 0.0;
    float b = cellAt(f + vec2(1.0, 0.0)) == v ? 1.0 : 0.0;
    float d = cellAt(f + vec2(0.0, 1.0)) == v ? 1.0 : 0.0;
    float e = cellAt(f + vec2(1.0, 1.0)) == v ? 1.0 : 0.0;
    return mix(mix(a, b, t.x), mix(d, e, t.x), t.y);
}

vec4 texel(float v, vec2 world) {
    vec4 i0 = info(v, 0.0);
    vec4 i3 = info(v, 3.0);
    vec2 size = vec2(byte(i0.z), byte(i0.w)) * 8.0;
    vec2 flow = vec2(byte(i3.x), byte(i3.y)) - 128.0;
    vec2 q = floor(world - mod(flow * _Time.x, size));
    vec2 slot = vec2(byte(i0.x), byte(i0.y)) * 256.0;
    // Animation: the frame of the clock, laid out in rows of as many as fit in the slot.
    float frames = byte(i3.z);
    float k = frames > 1.0 ? mod(floor(_Time.x * byte(i3.w) / 10.0), frames) : 0.0;
    float cols = floor(256.0 / size.x);
    vec2 frame = vec2(mod(k, cols), floor(k / cols)) * size;
    return texture2D(Atlas, (slot + frame + mod(q, size) + 0.5) / AtlasPx);
}

void main() {
    vec2 local = uv * CELLS * CELL;
    vec2 world = Origin + local;
    vec2 c = floor(local / CELL - 0.5);
    float vs[4];
    vs[0] = cellAt(c);
    vs[1] = cellAt(c + vec2(1.0, 0.0));
    vs[2] = cellAt(c + vec2(0.0, 1.0));
    vs[3] = cellAt(c + vec2(1.0, 1.0));
    if (vs[0] == vs[1] && vs[0] == vs[2] && vs[0] == vs[3]) {
        if (vs[0] == 0.0) {
            discard;
        }
        gl_FragColor = texel(vs[0], world) * color;
        return;
    }
    // Materials of the four cells around, drawn from the lowest priority up.
    float pr[4];
    for (int i = 0; i < 4; i++) {
        pr[i] = vs[i] == 0.0 ? -1.0 : byte(info(vs[i], 1.0).z) + vs[i] / 256.0;
        for (int j = 0; j < 4; j++) {
            if (j < i && vs[j] == vs[i]) {
                pr[i] = -2.0;
            }
        }
    }
    vec4 col = vec4(0.0);
    float last = -1.5;
    bool first = true;
    for (int k = 0; k < 4; k++) {
        float best = 1000.0;
        float bv = -1.0;
        for (int i = 0; i < 4; i++) {
            if (pr[i] > last && pr[i] < best) {
                best = pr[i];
                bv = vs[i];
            }
        }
        if (bv < 0.0) {
            break;
        }
        last = best;
        vec4 tex = bv == 0.0 ? vec4(0.0) : texel(bv, world);
        if (first) {
            col = tex;
            first = false;
            continue;
        }
        vec4 e = info(bv, 1.0);
        float soft = e.x;
        float amp = byte(e.y) / 32.0;
        float pix = max(1.0, byte(e.w));
        float w;
        float a;
        if (soft > 0.75) {
            w = cover(bv, local + noise(world / CELL) * 2.0 * amp);
            a = smoothstep(0.1, 0.9, w);
        } else if (soft > 0.25) {
            // Straight: the cell under the pixel decides; the rim is a band of the neighbouring
            // cells along the material's cells.
            vec2 cl = floor(local / CELL);
            a = cellAt(cl) == bv ? 1.0 : 0.0;
            vec4 rim = info(bv, 2.0);
            if (rim.a > 0.0 && a < 0.5) {
                vec2 f = local - cl * CELL;
                float r = min(byte(rim.a), 4.0);
                float near = 0.0;
                if (f.x < r && cellAt(cl + vec2(-1.0, 0.0)) == bv) near = 1.0;
                if (CELL - f.x <= r && cellAt(cl + vec2(1.0, 0.0)) == bv) near = 1.0;
                if (f.y < r && cellAt(cl + vec2(0.0, -1.0)) == bv) near = 1.0;
                if (CELL - f.y <= r && cellAt(cl + vec2(0.0, 1.0)) == bv) near = 1.0;
                if (f.x < r && f.y < r && cellAt(cl + vec2(-1.0, -1.0)) == bv) near = 1.0;
                if (CELL - f.x <= r && f.y < r && cellAt(cl + vec2(1.0, -1.0)) == bv) near = 1.0;
                if (f.x < r && CELL - f.y <= r && cellAt(cl + vec2(-1.0, 1.0)) == bv) near = 1.0;
                if (CELL - f.x <= r && CELL - f.y <= r && cellAt(cl + vec2(1.0, 1.0)) == bv) near = 1.0;
                col = mix(col, vec4(rim.rgb, 1.0), near);
            }
        } else {
            // Crisp: the whole art pixel takes one value (pixel-art steps).
            vec2 ps = (floor(world / pix) + 0.5) * pix;
            w = cover(bv, ps - Origin + noise(ps / CELL) * 2.0 * amp);
            a = step(0.5, w);
            vec4 rim = info(bv, 2.0);
            if (rim.a > 0.0) {
                float ra = step(0.5 - byte(rim.a) / CELL * 0.5, w) * (1.0 - a);
                col = mix(col, vec4(rim.rgb, 1.0), ra);
            }
        }
        col = mix(col, tex, a);
    }
    if (col.a < 0.01) {
        discard;
    }
    gl_FragColor = col * color;
}"#;

/// The ground shader, the material atlas and table of the map being drawn, and the chunks'
/// cell textures.
#[derive(Default)]
pub struct GroundRenderer {
    material: Option<Material>,
    failed: bool,
    noise: Option<Texture2D>,
    /// What the atlas was built from: the map's material list, their definitions, the assets.
    built: Option<(Vec<String>, Vec<Option<MaterialDef>>, u64)>,
    atlas: Option<(Texture2D, Texture2D, Vec2)>,
    /// Map identity of the cell textures (name, size).
    map_key: (String, u32, u32),
    chunks: HashMap<(u32, u32), Chunk>,
    frame: u64,
}

struct Chunk {
    texture: Texture2D,
    /// Edit counters of the chunk and its eight neighbours when it was uploaded.
    revs: u64,
    used: u64,
}

impl GroundRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets the cell textures (another map, or the ground was replaced wholesale).
    pub fn reset(&mut self) {
        self.chunks.clear();
        self.built = None;
    }

    fn shader(&mut self) -> Option<Material> {
        if self.material.is_none() && !self.failed {
            match load_material(
                ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
                MaterialParams {
                    uniforms: vec![UniformDesc::new("Origin", UniformType::Float2), UniformDesc::new("AtlasPx", UniformType::Float2)],
                    textures: vec!["Atlas".into(), "Info".into(), "Noise".into()],
                    // See-through textures (an abyss over a parallax layer) blend like sprites.
                    pipeline_params: macroquad::miniquad::PipelineParams {
                        color_blend: Some(macroquad::miniquad::BlendState::new(
                            macroquad::miniquad::Equation::Add,
                            macroquad::miniquad::BlendFactor::Value(macroquad::miniquad::BlendValue::SourceAlpha),
                            macroquad::miniquad::BlendFactor::OneMinusValue(macroquad::miniquad::BlendValue::SourceAlpha),
                        )),
                        alpha_blend: Some(macroquad::miniquad::BlendState::new(macroquad::miniquad::Equation::Add, macroquad::miniquad::BlendFactor::Zero, macroquad::miniquad::BlendFactor::One)),
                        ..Default::default()
                    },
                },
            ) {
                Ok(m) => self.material = Some(m),
                Err(e) => {
                    eprintln!("[render] ground shader: {e}");
                    self.failed = true;
                }
            }
        }
        self.material.clone()
    }

    fn noise(&mut self) -> Texture2D {
        self.noise
            .get_or_insert_with(|| {
                // A fixed pattern (the same ragged edges every run).
                let mut seed: u32 = 0x9E37_79B9;
                let mut next = || {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    (seed >> 24) as u8
                };
                let mut bytes = Vec::with_capacity(64 * 64 * 4);
                for _ in 0..64 * 64 {
                    bytes.extend_from_slice(&[next(), next(), 0, 255]);
                }
                let t = Texture2D::from_rgba8(64, 64, &bytes);
                t.set_filter(FilterMode::Nearest);
                t
            })
            .clone()
    }

    /// Builds the atlas and the material table for the map's materials when they changed.
    fn prepare(&mut self, map: &Map, db: &Database, images: &HashMap<String, Image>, generation: u64) {
        let defs: Vec<Option<MaterialDef>> = map.materials.iter().map(|id| db.material(id).cloned()).collect();
        if self.built.as_ref().is_some_and(|(m, d, g)| *m == map.materials && *d == defs && *g == generation) && self.atlas.is_some() {
            return;
        }
        let n = map.materials.len().min(MAX_MATERIALS) as u32;
        let rows = n.div_ceil(ATLAS_COLS).max(1);
        let mut atlas = Image::gen_image_color((ATLAS_COLS * SLOT) as u16, (rows * SLOT) as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        let mut info = vec![0u8; (INFO_W * 4 * 4) as usize];
        let mut put = |v: u32, row: u32, px: [u8; 4]| {
            let i = ((row * INFO_W + v) * 4) as usize;
            info[i..i + 4].copy_from_slice(&px);
        };
        for k in 0..n {
            let id = &map.materials[k as usize];
            let def = defs[k as usize].clone().unwrap_or_else(|| MaterialDef::fresh(id));
            let img = images.get(id).or_else(|| images.get(&def.id));
            let fallback;
            let img = match img {
                Some(i) => i,
                None => {
                    fallback = super::placeholder(64, 64);
                    &fallback
                }
            };
            // One frame's size (frames side by side in the texture), in steps of 8 px; the frames
            // go into the slot in rows, as many as fit.
            let (fw, fh) = def.frame_size(img.width as u32, img.height as u32);
            let (w, h) = ((fw.min(SLOT) / 8 * 8).max(8), (fh.min(SLOT) / 8 * 8).max(8));
            let cols = SLOT / w;
            let frames = (def.frames.max(1) as u32).min(cols * (SLOT / h)).max(1);
            let (sx, sy) = (k % ATLAS_COLS, k / ATLAS_COLS);
            for f in 0..frames {
                let (ox, oy) = (f % cols * w, f / cols * h);
                for y in 0..h.min(img.height as u32) {
                    for x in 0..w.min(fw) {
                        let src = f * fw + x;
                        if src < img.width as u32 {
                            atlas.set_pixel(sx * SLOT + ox + x, sy * SLOT + oy + y, img.get_pixel(src, y));
                        }
                    }
                }
            }
            let v = k + 1;
            put(v, 0, [sx as u8, sy as u8, (w / 8) as u8, (h / 8) as u8]);
            let soft = match def.edge {
                Edge::Crisp => 0,
                Edge::Straight => 128,
                Edge::Soft => 255,
            };
            put(v, 1, [soft, (def.edge_noise.clamp(0.0, 4.0) * 32.0) as u8, def.priority, def.edge_pixel.clamp(1, 8)]);
            if let Some(r) = def.rim {
                put(v, 2, [r.color[0], r.color[1], r.color[2], r.width.clamp(1, 8)]);
            }
            let flow = |f: f32| (f.clamp(-127.0, 127.0) + 128.0).round() as u8;
            put(v, 3, [flow(def.scroll[0]), flow(def.scroll[1]), frames as u8, (def.fps.clamp(0.0, 25.5) * 10.0).round() as u8]);
        }
        let atlas_tex = Texture2D::from_image(&atlas);
        atlas_tex.set_filter(FilterMode::Nearest);
        let info_tex = Texture2D::from_rgba8(INFO_W as u16, 4, &info);
        info_tex.set_filter(FilterMode::Nearest);
        self.atlas = Some((atlas_tex, info_tex, vec2(atlas.width as f32, atlas.height as f32)));
        self.built = Some((map.materials.clone(), defs, generation));
    }

    /// Draws the ground of `map` inside `view` (world pixels) with the current camera; `tint`
    /// multiplies it (the editor dims inactive layers).
    pub fn draw(&mut self, map: &Map, db: &Database, assets: &super::Assets, view: Rect, tint: Color) {
        self.draw_with(map, db, &assets.materials, assets.generation, view, tint);
    }

    /// [`GroundRenderer::draw`] with the materials' texture images given (material id -> image;
    /// the editor's material preview passes a texture still being drawn); `generation` changes
    /// whenever an image does.
    pub fn draw_with(&mut self, map: &Map, db: &Database, images: &HashMap<String, Image>, generation: u64, view: Rect, tint: Color) {
        if map.ground.is_empty() {
            return;
        }
        let Some(mat) = self.shader() else { return };
        let key = (map.name.clone(), map.width, map.height);
        if self.map_key != key {
            self.chunks.clear();
            self.map_key = key;
        }
        self.prepare(map, db, images, generation);
        let noise = self.noise();
        let Some((atlas, info, atlas_px)) = self.atlas.clone() else { return };
        self.frame += 1;
        let span = (CHUNK_CELLS * CELL) as f32;
        let map_px = vec2(map.width as f32, map.height as f32) * crate::project::TILE_SIZE as f32;
        let (cw, ch) = map.ground.chunks_size();
        let cx0 = (view.x / span).floor().max(0.0) as u32;
        let cy0 = (view.y / span).floor().max(0.0) as u32;
        let cx1 = ((view.right() / span).ceil().max(0.0) as u32).min(cw);
        let cy1 = ((view.bottom() / span).ceil().max(0.0) as u32).min(ch);
        mat.set_texture("Atlas", atlas);
        mat.set_texture("Info", info);
        mat.set_texture("Noise", noise);
        mat.set_uniform("AtlasPx", atlas_px);
        gl_use_material(&mat);
        for cy in cy0..cy1 {
            for cx in cx0..cx1 {
                if self.blank(map, cx, cy) {
                    continue;
                }
                let tex = self.cells(map, cx, cy);
                mat.set_uniform("Origin", vec2(cx as f32 * span, cy as f32 * span));
                // The last chunks stop at the map's edge (the cells beyond only shape the edge):
                // a smaller quad over the same fraction of the texture keeps the coordinates.
                let size = vec2((map_px.x - cx as f32 * span).min(span), (map_px.y - cy as f32 * span).min(span));
                let source = Rect::new(0.0, 0.0, tex.width() * size.x / span, tex.height() * size.y / span);
                draw_texture_ex(&tex, cx as f32 * span, cy as f32 * span, tint, DrawTextureParams { dest_size: Some(size), source: Some(source), ..Default::default() });
            }
        }
        gl_use_default_material();
        if self.chunks.len() > MAX_CHUNKS {
            let mut ages: Vec<u64> = self.chunks.values().map(|c| c.used).collect();
            ages.sort_unstable();
            let cut = ages[self.chunks.len() - MAX_CHUNKS];
            self.chunks.retain(|_, c| c.used >= cut);
        }
    }

    /// No ground in the chunk nor at its border.
    fn blank(&self, map: &Map, cx: u32, cy: u32) -> bool {
        let g = &map.ground;
        let empty = |x: i64, y: i64| x < 0 || y < 0 || g.chunk(x as u32, y as u32).is_none_or(|c| *c == crate::ground::GroundChunk::Fill(0));
        (-1..=1).all(|dy| (-1..=1).all(|dx| empty(cx as i64 + dx, cy as i64 + dy)))
    }

    /// The cell texture of chunk (`cx`, `cy`), uploaded again when it or a neighbour changed.
    fn cells(&mut self, map: &Map, cx: u32, cy: u32) -> Texture2D {
        let g = &map.ground;
        let mut revs: u64 = 0;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (x, y) = (cx as i64 + dx, cy as i64 + dy);
                if x >= 0 && y >= 0 {
                    revs = revs.wrapping_mul(31).wrapping_add(g.rev(x as u32, y as u32) as u64 + 1);
                }
            }
        }
        let frame = self.frame;
        if let Some(c) = self.chunks.get_mut(&(cx, cy)) {
            if c.revs == revs {
                c.used = frame;
                return c.texture.clone();
            }
        }
        let (ox, oy) = ((cx * CHUNK_CELLS) as i32 - 1, (cy * CHUNK_CELLS) as i32 - 1);
        // Past the map's edge the edge cells go on (no ragged border against the background).
        let (gw, gh) = g.size();
        let mut bytes = vec![0u8; (SIDE * SIDE * 4) as usize];
        for y in 0..SIDE as i32 {
            for x in 0..SIDE as i32 {
                let i = ((y as u32 * SIDE + x as u32) * 4) as usize;
                bytes[i] = g.get((ox + x).clamp(0, gw as i32 - 1), (oy + y).clamp(0, gh as i32 - 1));
                bytes[i + 3] = 255;
            }
        }
        let texture = Texture2D::from_rgba8(SIDE as u16, SIDE as u16, &bytes);
        texture.set_filter(FilterMode::Nearest);
        self.chunks.insert((cx, cy), Chunk { texture: texture.clone(), revs, used: frame });
        texture
    }
}
