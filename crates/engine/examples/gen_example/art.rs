//! Procedural pixel art for the example project (32x32 tiles).
//!
//! Everything is drawn with a few helpers: shaded shapes lit from the top-left, an automatic
//! dark outline around sprites, and soft drop shadows underneath.

use std::path::Path;

use macroquad::prelude::{Color, Image};
use rustpg2d_engine::defs::{Armor, EquipSlot, ItemDef, SpriteRef};
use rustpg2d_engine::entity::Facing;

/// Tile size in pixels.
pub const TS: u32 = rustpg2d_engine::project::TILE_SIZE;
/// Scale of the art relative to the classic 16px grid (hitboxes and speeds are given in
/// 16px units and multiplied by this).
pub const R: f32 = TS as f32 / 16.0;

// ------------------------------------------------------------------ colors

pub fn hex(v: u32) -> Color {
    Color::from_rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}
pub const CLEAR: Color = Color::new(0.0, 0.0, 0.0, 0.0);

/// `v` with its brightness scaled by `f`.
fn mul(v: u32, f: f32) -> u32 {
    let ch = |s: u32| (((v >> s) & 0xff) as f32 * f).round().clamp(0.0, 255.0) as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}
/// Dark, mid-dark, base and light shade of a color.
fn ramp(v: u32) -> [u32; 4] {
    [mul(v, 0.55), mul(v, 0.78), v, mul(v, 1.22)]
}

const OUTLINE: u32 = 0x1a1622;
const WOOD: [u32; 4] = [0x3e2410, 0x6b4423, 0x8a5a2e, 0xb07e4a];
const STONE: [u32; 4] = [0x45454e, 0x6c6c76, 0x8f8f9a, 0xbcbcc8];
const IRON: [u32; 4] = [0x1c1c22, 0x32323a, 0x4c4c56, 0x777784];
const LEAF: [u32; 4] = [0x1d5124, 0x2e7d32, 0x43a047, 0x7cc56f];
const PINE: [u32; 4] = [0x0d3a22, 0x1b5e3a, 0x2e7d4f, 0x5aa872];
const GOLD: [u32; 4] = [0x7a5410, 0xc0901e, 0xe8c05a, 0xfff2a8];

// ------------------------------------------------------------------ random numbers

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    pub fn range(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    pub fn chance(&mut self, p: f32) -> bool {
        (self.next() % 10000) as f32 / 10000.0 < p
    }
    fn f(&mut self) -> f32 {
        (self.next() % 10000) as f32 / 10000.0
    }
}

/// 4x4 ordered dither threshold in 0..1.
fn bayer(x: i32, y: i32) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f32 + 0.5) / 16.0
}

// ------------------------------------------------------------------ canvas

pub struct Canvas {
    img: Image,
    ox: u32,
    oy: u32,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Canvas { img: Image::gen_image_color(w as u16, h as u16, CLEAR), ox: 0, oy: 0 }
    }
    pub fn at(&mut self, ox: u32, oy: u32) -> &mut Self {
        self.ox = ox;
        self.oy = oy;
        self
    }
    fn abs(&self, x: i32, y: i32) -> Option<(u32, u32)> {
        let (x, y) = (self.ox as i32 + x, self.oy as i32 + y);
        (x >= 0 && y >= 0 && x < self.img.width as i32 && y < self.img.height as i32).then_some((x as u32, y as u32))
    }
    fn get(&self, x: i32, y: i32) -> Color {
        self.abs(x, y).map_or(CLEAR, |(x, y)| self.img.get_pixel(x, y))
    }
    /// Sets a pixel; translucent colors are blended over what is there.
    pub fn px(&mut self, x: i32, y: i32, c: Color) {
        // Never spill into the cell above / to the left of the current origin.
        if x < 0 || y < 0 {
            return;
        }
        let Some((ax, ay)) = self.abs(x, y) else { return };
        // Fully transparent (CLEAR) erases: cut-outs like holes, flat bottoms, neck openings.
        if c.a <= 0.0 {
            self.img.set_pixel(ax, ay, c);
            return;
        }
        if c.a >= 0.999 {
            self.img.set_pixel(ax, ay, c);
            return;
        }
        let d = self.img.get_pixel(ax, ay);
        let a = c.a + d.a * (1.0 - c.a);
        if a <= 0.0 {
            return;
        }
        let mix = |s: f32, t: f32| (s * c.a + t * d.a * (1.0 - c.a)) / a;
        self.img.set_pixel(ax, ay, Color::new(mix(c.r, d.r), mix(c.g, d.g), mix(c.b, d.b), a));
    }
    /// Sets a pixel only where nothing opaque is drawn yet (shadows go under sprites).
    fn under(&mut self, x: i32, y: i32, c: Color) {
        if self.get(x, y).a < 0.5 {
            self.px(x, y, c);
        }
    }
    /// Pixel on a seamlessly tiling TS x TS tile.
    fn wrap(&mut self, x: i32, y: i32, c: Color) {
        self.px(x.rem_euclid(TS as i32), y.rem_euclid(TS as i32), c);
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
    }
    fn hline(&mut self, x: i32, y: i32, w: i32, c: Color) {
        self.rect(x, y, w, 1, c);
    }
    fn vline(&mut self, x: i32, y: i32, h: i32, c: Color) {
        self.rect(x, y, 1, h, c);
    }
    /// Rectangle with rounded corners of radius `r`.
    fn rrect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: Color) {
        for yy in 0..h {
            for xx in 0..w {
                let dx = (r - xx).max(xx - (w - 1 - r)).max(0);
                let dy = (r - yy).max(yy - (h - 1 - r)).max(0);
                if dx * dx + dy * dy <= r * r + r / 2 {
                    self.px(x + xx, y + yy, c);
                }
            }
        }
    }
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        self.ellipse(cx, cy, r, r, c);
    }
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.px(x, y, c);
                }
            }
        }
    }
    /// Ellipse shaded like a lit sphere (light from the top-left), dithered between 4 tones.
    fn shade_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, pal: [u32; 4]) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (nx, ny) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                let d = nx * nx + ny * ny;
                if d > 1.0 {
                    continue;
                }
                let l = 0.55 - 0.42 * nx - 0.55 * ny - 0.3 * d + (bayer(x, y) - 0.5) * 0.18;
                let k = if l < 0.12 {
                    0
                } else if l < 0.42 {
                    1
                } else if l < 0.78 {
                    2
                } else {
                    3
                };
                self.px(x, y, hex(pal[k]));
            }
        }
    }
    fn shade_disc(&mut self, cx: f32, cy: f32, r: f32, pal: [u32; 4]) {
        self.shade_ellipse(cx, cy, r, r, pal);
    }
    /// Box lit from the top-left: light top/left edges, dark bottom/right edges.
    fn shade_box(&mut self, x: i32, y: i32, w: i32, h: i32, pal: [u32; 4]) {
        self.rect(x, y, w, h, hex(pal[2]));
        self.hline(x, y, w, hex(pal[3]));
        self.vline(x, y, h, hex(pal[3]));
        self.hline(x, y + h - 1, w, hex(pal[1]));
        self.vline(x + w - 1, y, h, hex(pal[1]));
    }
    /// Vertical cylinder shading (left lit, right in shadow), e.g. barrels and trunks.
    fn shade_cyl(&mut self, x: i32, y: i32, w: i32, h: i32, pal: [u32; 4]) {
        for xx in 0..w {
            let t = (xx as f32 + 0.5) / w as f32;
            for yy in 0..h {
                let l = 0.95 - 1.1 * (t - 0.3).abs() + (bayer(x + xx, y + yy) - 0.5) * 0.25;
                let k = if l < 0.3 {
                    0
                } else if l < 0.6 {
                    1
                } else if l < 0.88 {
                    2
                } else {
                    3
                };
                self.px(x + xx, y + yy, hex(pal[k]));
            }
        }
    }
    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.px(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    /// Thick line made of discs.
    fn stroke(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, c: Color) {
        let n = ((x1 - x0).hypot(y1 - y0) * 2.0).ceil().max(1.0) as i32;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            self.disc(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, r, c);
        }
    }
    /// Dark outline around everything opaque in the `w` x `h` cell at the origin.
    fn auto_outline(&mut self, w: i32, h: i32, c: u32) {
        let mut edge = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if self.get(x, y).a >= 0.5 {
                    continue;
                }
                let solid = |dx: i32, dy: i32| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < w && ny < h && self.get(nx, ny).a >= 0.5
                };
                if solid(1, 0) || solid(-1, 0) || solid(0, 1) || solid(0, -1) {
                    edge.push((x, y));
                }
            }
        }
        for (x, y) in edge {
            self.px(x, y, hex(c));
        }
    }
    /// Outline around what is drawn, but only where `mask` (same layout) has nothing: an
    /// overlay gets its outline where it stands out from the body below.
    fn outline_outside(&mut self, mask: &Canvas, w: i32, h: i32, c: u32) {
        let mut edge = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if self.get(x, y).a >= 0.5 || mask.get(x, y).a >= 0.5 {
                    continue;
                }
                let solid = |dx: i32, dy: i32| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < w && ny < h && self.get(nx, ny).a >= 0.5
                };
                if solid(1, 0) || solid(-1, 0) || solid(0, 1) || solid(0, -1) {
                    edge.push((x, y));
                }
            }
        }
        for (x, y) in edge {
            self.px(x, y, hex(c));
        }
    }
    /// Soft elliptical shadow under what is already drawn.
    fn shadow(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                let d = dx * dx + dy * dy;
                if d <= 1.0 {
                    self.under(x, y, Color::new(0.0, 0.0, 0.0, if d < 0.55 { 0.3 } else { 0.18 }));
                }
            }
        }
    }
    /// Copies the TS x TS tile at (0, `sy`) of another canvas to the current origin.
    fn blit_tile(&mut self, src: &Canvas, sy: u32) {
        for y in 0..TS {
            for x in 0..TS {
                let c = src.img.get_pixel(x, sy + y);
                self.img.set_pixel(self.ox + x, self.oy + y, c);
            }
        }
    }
    /// Base color with speckles and small blobs of a lighter / darker shade (tiles seamlessly).
    fn ground(&mut self, base: u32, light: u32, dark: u32, rng: &mut Rng, density: f32) {
        let n = TS as i32;
        self.rect(0, 0, n, n, hex(base));
        for y in 0..n {
            for x in 0..n {
                if rng.chance(density) {
                    let c = hex(if rng.chance(0.5) { light } else { dark });
                    self.px(x, y, c);
                    if rng.chance(0.3) {
                        self.wrap(x + 1, y, c);
                    }
                }
            }
        }
    }
    pub fn save(&self, path: &Path) {
        // `export_png` flips vertically (it's meant for GPU readback), so pre-flip.
        let mut img = self.img.clone();
        let row = img.width as usize * 4;
        let h = img.height as usize;
        for y in 0..h / 2 {
            let (a, b) = img.bytes.split_at_mut((h - 1 - y) * row);
            a[y * row..y * row + row].swap_with_slice(&mut b[..row]);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        img.export_png(path.to_str().unwrap());
    }
}

// ------------------------------------------------------------------ terrain tileset

pub mod extra;

pub mod t {
    pub const GRASS: u32 = 0;
    pub const FLOWERS: u32 = 1;
    pub const PATH: u32 = 2;
    pub const WATER: u32 = 3;
    pub const SAND: u32 = 4;
    pub const STONE_FLOOR: u32 = 5;
    pub const WOOD_FLOOR: u32 = 6;
    pub const VOID: u32 = 7;
    pub const STONE_WALL: u32 = 8;
    pub const HOUSE_WALL: u32 = 9;
    pub const ROOF: u32 = 10;
    pub const DOOR: u32 = 11;
    pub const TRUNK: u32 = 12;
    pub const CANOPY: u32 = 13;
    pub const BUSH: u32 = 14;
    pub const ROCK: u32 = 15;
    pub const FENCE: u32 = 16;
    pub const TALL_GRASS: u32 = 17;
    pub const BRIDGE: u32 = 18;
    pub const WINDOW: u32 = 19;
    pub const RUG: u32 = 20;
    pub const TABLE: u32 = 21;
    pub const BED: u32 = 22;
    pub const SHELF: u32 = 23;
    pub const FOREST_FLOOR: u32 = 24;
    pub const MUSHROOM: u32 = 25;
    pub const CAVE: u32 = 26;
    pub const CLIFF: u32 = 27;
    pub const ROOF_EDGE: u32 = 28;
    pub const PINE_TRUNK: u32 = 29;
    pub const PINE_TOP: u32 = 30;
    pub const SNOW: u32 = 31;
    // Animation frames for water (row 4).
    pub const WATER_2: u32 = 32;
    pub const WATER_3: u32 = 33;
    /// Lower half of the bed (blanket, no pillow).
    pub const BED_FOOT: u32 = 34;
}

pub const SOLID: [u32; 18] = [
    t::WATER, t::VOID, t::STONE_WALL, t::HOUSE_WALL, t::ROOF, t::TRUNK, t::BUSH, t::ROCK, t::FENCE,
    t::WINDOW, t::TABLE, t::BED, t::SHELF, t::CLIFF, t::ROOF_EDGE, t::PINE_TRUNK, t::MUSHROOM, t::BED_FOOT,
];

const GRASS: (u32, u32, u32) = (0x4f9a43, 0x5fae52, 0x438a3a);

fn grass(c: &mut Canvas, rng: &mut Rng) {
    c.ground(GRASS.0, GRASS.1, GRASS.2, rng, 0.16);
    // Little tufts of blades.
    for _ in 0..9 {
        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
        for (dx, h) in [(0, 3), (1, 4), (2, 2)] {
            for k in 0..h {
                c.wrap(x + dx, y - k, hex(if k == h - 1 { 0x7cc56f } else { 0x3d8034 }));
            }
        }
    }
}

/// A tree drawn into a 32x64 canvas: the top half becomes the canopy tile (overhead layer),
/// the bottom half the trunk tile.
fn tree_pair(pine: bool) -> Canvas {
    let mut c = Canvas::new(TS, 2 * TS);
    if pine {
        c.shade_cyl(13, 40, 6, 20, WOOD);
        for (k, y) in [(5.0, 4.0), (8.0, 12.0), (11.0, 21.0), (13.5, 30.0), (15.0, 38.0)] {
            pine_tier(&mut c, 16.0, y, k, 10.0);
        }
    } else {
        c.shade_cyl(12, 30, 8, 30, WOOD);
        c.rect(10, 56, 12, 3, hex(WOOD[1])); // roots
        c.px(9, 58, hex(WOOD[1]));
        c.px(22, 58, hex(WOOD[1]));
        for (x, y, r) in [(16.0, 22.0, 14.0), (8.0, 24.0, 8.0), (24.0, 24.0, 8.0), (13.0, 14.0, 10.0), (21.0, 15.0, 9.0), (16.0, 9.0, 8.0)] {
            c.shade_disc(x, y, r, LEAF);
        }
    }
    c.auto_outline(TS as i32, 2 * TS as i32, 0x10301a);
    c.shadow(16.0, 59.5, 12.0, 3.5);
    c
}

/// One triangular tier of a pine, apex at (`cx`, `y`), half-width `k`, height `h`.
fn pine_tier(c: &mut Canvas, cx: f32, y: f32, k: f32, h: f32) {
    for row in 0..h as i32 {
        let half = k * (row as f32 + 1.0) / h;
        for x in (cx - half).floor() as i32..=(cx + half).ceil() as i32 {
            let nx = (x as f32 + 0.5 - cx) / half.max(1.0);
            let l = 0.6 - 0.5 * nx - 0.04 * row as f32 + (bayer(x, row) - 0.5) * 0.25;
            let k = if l < 0.15 {
                0
            } else if l < 0.45 {
                1
            } else if l < 0.8 {
                2
            } else {
                3
            };
            c.px(x, y as i32 + row, hex(PINE[k]));
        }
    }
}

pub fn terrain(out: &Path) {
    let cols = 8;
    let mut c = Canvas::new(cols * TS, (25 + 7 * BLOB_TERRAINS.len() as u32) * TS);
    let mut rng = Rng(7);
    let n = TS as i32;
    let oak = tree_pair(false);
    let pine = tree_pair(true);
    for i in 0..35u32 {
        c.at(i % cols * TS, i / cols * TS);
        match i {
            t::GRASS => grass(&mut c, &mut rng),
            t::FLOWERS => {
                grass(&mut c, &mut rng);
                for _ in 0..5 {
                    let (x, y) = (rng.range(26) as i32 + 3, rng.range(26) as i32 + 3);
                    let col = [0xf4e04d, 0xf06292, 0xffffff, 0x9fa8ff, 0xff9f43][rng.range(5) as usize];
                    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                        c.px(x + dx, y + dy, hex(col));
                    }
                    c.px(x, y, hex(0xfff3b0));
                    c.px(x + 1, y + 2, hex(0x2f6e2a));
                }
            }
            t::PATH => path(&mut c, &mut rng),
            t::WATER | t::WATER_2 | t::WATER_3 => water(
                &mut c,
                match i {
                    t::WATER => 0,
                    t::WATER_2 => 4,
                    _ => 8,
                },
            ),
            t::SAND => sand(&mut c, &mut rng),
            t::STONE_FLOOR => stone_floor(&mut c, &mut rng),
            t::WOOD_FLOOR => wood_floor(&mut c, &mut rng),
            t::VOID => c.rect(0, 0, n, n, hex(0x0d0d14)),
            t::STONE_WALL | t::CLIFF => {
                let base = if i == t::CLIFF { 0x6b5d52 } else { 0x8a8a94 };
                let mortar = if i == t::CLIFF { 0x3e342d } else { 0x4c4c55 };
                c.rect(0, 0, n, n, hex(mortar));
                for (row, y) in (0..n).step_by(8).enumerate() {
                    let off = if row % 2 == 0 { 0 } else { 8 };
                    for x in (-16 + off..n).step_by(16) {
                        let tone = mul(base, 0.92 + rng.f() * 0.16);
                        let pal = ramp(tone);
                        // Bricks wrap around the tile edge so walls tile seamlessly.
                        for yy in 0..7 {
                            for xx in 0..15 {
                                let col = if yy == 0 || xx == 0 {
                                    pal[3]
                                } else if yy == 6 || xx == 14 || rng.chance(0.08) {
                                    pal[1]
                                } else {
                                    pal[2]
                                };
                                c.wrap(x + xx, y + yy, hex(col));
                            }
                        }
                    }
                }
                if i == t::CLIFF {
                    for _ in 0..3 {
                        let (x, y) = (rng.range(28) as i32 + 2, rng.range(28) as i32 + 2);
                        c.line(x, y, x + 2, y + 3, hex(0x3e342d));
                    }
                }
            }
            t::HOUSE_WALL | t::WINDOW | t::DOOR => {
                c.ground(0xe0caa4, 0xeadab8, 0xd2b88f, &mut rng, 0.1);
                // Timber frame: a beam along the top and the left edge (walls stack and tile
                // seamlessly), plaster slightly shaded towards the bottom.
                c.rect(0, 0, n, 3, hex(WOOD[1]));
                c.hline(0, 0, n, hex(WOOD[2]));
                c.rect(0, 0, 3, n, hex(WOOD[1]));
                c.vline(0, 0, n, hex(WOOD[2]));
                for x in 3..n {
                    c.px(x, 3, hex(0xb89c74));
                    if (x + 31) % 4 < 2 {
                        c.px(x, 30, hex(0xd2b88f));
                    }
                    c.px(x, 31, hex(0xc8ac82));
                }
                if i == t::WINDOW {
                    c.rect(7, 6, 18, 17, hex(WOOD[0]));
                    c.rect(9, 8, 14, 13, hex(0x8ec9ef));
                    c.rect(9, 8, 14, 4, hex(0xb8e2fa));
                    c.line(11, 18, 15, 10, hex(0xe8f6ff));
                    c.vline(15, 8, 13, hex(WOOD[1]));
                    c.hline(9, 14, 14, hex(WOOD[1]));
                    c.rect(6, 22, 20, 2, hex(WOOD[2])); // sill
                    c.px(9, 21, hex(0xf06292)); // flowers on the sill
                    c.px(12, 21, hex(0xf4e04d));
                    c.px(20, 21, hex(0xf06292));
                }
                if i == t::DOOR {
                    c.rrect(6, 3, 20, 29, 6, hex(WOOD[0]));
                    c.rrect(8, 5, 16, 27, 5, hex(WOOD[2]));
                    for x in [11, 15, 19] {
                        c.vline(x, 6, 26, hex(WOOD[1]));
                    }
                    c.rect(8, 10, 16, 2, hex(IRON[1])); // hinges
                    c.rect(8, 23, 16, 2, hex(IRON[1]));
                    c.rect(20, 17, 2, 2, hex(GOLD[2]));
                    c.px(20, 17, hex(GOLD[3]));
                }
            }
            t::ROOF | t::ROOF_EDGE => {
                for (row, y) in (0..n).step_by(8).enumerate() {
                    let off = if row % 2 == 0 { 0 } else { 4 };
                    for x in (-8 + off..n).step_by(8) {
                        let tone = mul(0xa8433a, 0.92 + rng.f() * 0.14);
                        for yy in 0..8 {
                            for xx in 0..8 {
                                // Rounded shingle bottoms.
                                let bottom = yy == 7 || (yy == 6 && (xx == 0 || xx == 7));
                                let col = if bottom {
                                    0x5e231e
                                } else if xx == 0 {
                                    mul(tone, 0.8)
                                } else if yy < 2 {
                                    mul(tone, 1.12)
                                } else {
                                    tone
                                };
                                c.wrap(x + xx, y + yy, hex(col));
                            }
                        }
                    }
                }
                if i == t::ROOF_EDGE {
                    c.rect(0, 24, n, 6, hex(0x6e2a24));
                    c.hline(0, 24, n, hex(0x8e3730));
                    c.rect(0, 30, n, 2, hex(0x3a1512));
                }
            }
            t::TRUNK => c.blit_tile(&oak, TS),
            t::CANOPY => c.blit_tile(&oak, 0),
            t::PINE_TRUNK => c.blit_tile(&pine, TS),
            t::PINE_TOP => c.blit_tile(&pine, 0),
            t::BUSH => {
                for (x, y, r) in [(16.0, 18.0, 12.0), (9.0, 21.0, 7.0), (23.0, 21.0, 7.0), (14.0, 13.0, 7.0)] {
                    c.shade_disc(x, y, r, [0x1f5a22, 0x2c6e2a, 0x3d8a38, 0x63ab55]);
                }
                for (x, y) in [(10, 14), (20, 20), (14, 22), (22, 12)] {
                    c.rect(x, y, 2, 2, hex(0xd84343));
                    c.px(x, y, hex(0xff9a9a));
                }
                c.auto_outline(n, n, 0x10301a);
                c.shadow(16.0, 28.0, 13.0, 3.5);
            }
            t::ROCK => {
                c.shade_ellipse(16.0, 19.0, 12.0, 10.0, STONE);
                c.line(12, 14, 15, 19, hex(STONE[1]));
                c.line(15, 19, 14, 23, hex(STONE[1]));
                c.rect(19, 12, 3, 2, hex(0x5f8f4a)); // moss
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 28.0, 13.0, 3.5);
            }
            t::FENCE => {
                for y in [10, 20] {
                    c.rect(0, y, n, 4, hex(WOOD[2]));
                    c.hline(0, y, n, hex(WOOD[3]));
                    c.hline(0, y + 3, n, hex(WOOD[1]));
                }
                for x in [5, 23] {
                    c.shade_box(x, 4, 5, 25, WOOD);
                    c.px(x + 2, 3, hex(WOOD[2]));
                    c.rect(x, 28, 5, 2, Color::new(0.0, 0.0, 0.0, 0.25));
                }
            }
            t::TALL_GRASS => {
                grass(&mut c, &mut rng);
                for _ in 0..16 {
                    let x = rng.range(30) as i32 + 1;
                    let h = 7 + rng.range(10) as i32;
                    let lean = rng.range(3) as i32 - 1;
                    for k in 0..h {
                        let xx = x + if k > h / 2 { lean } else { 0 };
                        c.px(xx, 31 - k, hex(if k > h - 3 { 0x8fd17c } else if k < 3 { 0x2a6a25 } else { 0x3b8a33 }));
                    }
                }
            }
            t::BRIDGE => {
                for x in (0..n).step_by(8) {
                    let tone = mul(0x9a6b3c, 0.92 + rng.f() * 0.14);
                    c.shade_box(x, 0, 8, n, ramp(tone));
                    c.px(x + 3, 6, hex(WOOD[0]));
                    c.px(x + 3, 25, hex(WOOD[0]));
                }
                c.rect(0, 0, n, 3, hex(WOOD[0]));
                c.rect(0, 29, n, 3, hex(WOOD[0]));
            }
            t::RUG => rug(&mut c),
            t::TABLE => {
                c.shadow(16.0, 28.0, 14.0, 3.0);
                c.shade_box(2, 6, 28, 16, WOOD);
                c.rect(4, 22, 3, 8, hex(WOOD[0]));
                c.rect(25, 22, 3, 8, hex(WOOD[0]));
                c.rect(8, 8, 16, 11, hex(0xf2efe6)); // cloth
                c.hline(8, 18, 16, hex(0xcfc9b8));
                c.rect(11, 10, 4, 5, hex(0xd9d9e0)); // cup
                c.px(11, 10, hex(0xffffff));
                c.disc(20.0, 13.0, 3.0, hex(0xd84343)); // apple
                c.px(19, 11, hex(0xff9a9a));
                c.auto_outline(n, n, OUTLINE);
            }
            t::BED => {
                // Headboard, pillow and the top of the blanket; continues in BED_FOOT.
                c.rect(2, 0, 28, n, hex(WOOD[2]));
                c.vline(2, 0, n, hex(WOOD[3]));
                c.vline(29, 0, n, hex(WOOD[1]));
                c.shade_box(2, 0, 28, 4, WOOD);
                c.rrect(5, 5, 22, 9, 3, hex(0xf5f5f5));
                c.hline(7, 13, 18, hex(0xcfcfd8));
                c.rect(4, 16, 24, 16, hex(0x3f6fb5));
                c.rect(4, 16, 24, 4, hex(0xe8e8f0)); // folded sheet
                c.hline(4, 20, 24, hex(0x2f5590));
            }
            t::BED_FOOT => {
                c.rect(2, 0, 28, n, hex(WOOD[2]));
                c.vline(2, 0, n, hex(WOOD[3]));
                c.vline(29, 0, n, hex(WOOD[1]));
                c.rect(4, 0, 24, 26, hex(0x3f6fb5));
                for y in [8, 17] {
                    c.hline(4, y, 24, hex(0x2f5590));
                }
                c.hline(4, 25, 24, hex(0x2f5590));
                c.shade_box(2, 26, 28, 6, WOOD); // footboard
            }
            t::SHELF => {
                c.shade_box(0, 0, n, n, WOOD);
                c.rect(2, 2, 28, 28, hex(0x3d2712));
                for (row, y) in [(0u32, 3), (1, 17)] {
                    let mut x = 3;
                    let mut k = row;
                    while x < 28 {
                        let w = 2 + (k * 7 % 3) as i32;
                        let h = 9 + (k * 5 % 4) as i32;
                        let col = [0xc0392b, 0x2980b9, 0x27ae60, 0xf1c40f, 0x8e44ad, 0xd35400][(k % 6) as usize];
                        c.rect(x, y + 12 - h, w, h, hex(col));
                        c.vline(x, y + 12 - h, h, hex(mul(col, 1.25)));
                        x += w + 1;
                        k += 1;
                    }
                    c.rect(1, y + 12, 30, 2, hex(WOOD[2]));
                }
            }
            t::FOREST_FLOOR => forest_floor(&mut c, &mut rng),
            t::MUSHROOM => {
                for (cx, cy, r, stem) in [(12.0f32, 14.0f32, 9.0f32, 7), (24.0, 22.0, 5.0, 4)] {
                    let sx = cx as i32;
                    c.shade_box(sx - stem / 2, cy as i32, stem, (30.0 - cy) as i32, [0xb8b0a0, 0xd8d0c0, 0xeeeeee, 0xffffff]);
                    for y in (cy - r) as i32..=cy as i32 {
                        for x in (cx - r) as i32..=(cx + r) as i32 {
                            let (dx, dy) = ((x as f32 + 0.5 - cx) / r, (y as f32 + 0.5 - cy) / (r * 0.8));
                            if dx * dx + dy * dy <= 1.0 {
                                let l = 0.6 - 0.5 * dx - 0.5 * dy + (bayer(x, y) - 0.5) * 0.2;
                                c.px(x, y, hex(if l < 0.3 { 0x8e1c1c } else if l < 0.8 { 0xc62828 } else { 0xef5350 }));
                            }
                        }
                    }
                }
                for (x, y) in [(8, 9), (13, 7), (16, 11), (23, 18)] {
                    c.rect(x, y, 2, 2, hex(0xffffff));
                }
                c.auto_outline(n, n, 0x2a0e0e);
                c.shadow(16.0, 30.0, 13.0, 2.5);
            }
            t::CAVE => {
                c.ground(0x6b5d52, 0x7a6b5f, 0x5a4d43, &mut rng, 0.2);
                c.rrect(3, 6, 26, 26, 11, hex(0x3e342d));
                c.rrect(5, 8, 22, 24, 10, hex(0x120d0a));
                c.rect(8, 26, 16, 6, hex(0x1e1712));
            }
            t::SNOW => snow(&mut c, &mut rng),
            _ => {}
        }
    }
    terrain_edges(&mut c);
    c.save(&out.join("tilesets/terrain.png"));
}

fn sand(c: &mut Canvas, rng: &mut Rng) {
    let n = TS as i32;
    c.ground(0xe2cf92, 0xf0e0aa, 0xcdb878, rng, 0.18);
    for y in [7, 22] {
        for x in 0..n {
            if (x / 4) % 3 != 0 {
                c.px(x, y + ((x / 4) % 2), hex(0xd2bd80));
            }
        }
    }
}

fn stone_floor(c: &mut Canvas, rng: &mut Rng) {
    for (x, y) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
        let tone = if (x + y) % 32 == 0 { 0x8d8d96 } else { 0x85858e };
        c.shade_box(x, y, 16, 16, ramp(tone));
        for _ in 0..6 {
            c.px(x + 2 + rng.range(12) as i32, y + 2 + rng.range(12) as i32, hex(mul(tone, 0.88)));
        }
    }
}

fn wood_floor(c: &mut Canvas, rng: &mut Rng) {
    let n = TS as i32;
    for (row, y) in (0..n).step_by(8).enumerate() {
        let tone = [0xa0703f, 0x98693a, 0xa87644, 0x9c6c3c][row % 4];
        c.rect(0, y, n, 8, hex(tone));
        c.hline(0, y, n, hex(mul(tone, 1.15)));
        c.hline(0, y + 7, n, hex(0x5e3d1e));
        let seam = [5, 21, 13, 27][row % 4];
        c.vline(seam, y, 8, hex(0x5e3d1e));
        c.px(seam - 2, y + 3, hex(0x4a2e14));
        c.px(seam + 2, y + 3, hex(0x4a2e14));
        for _ in 0..3 {
            let (x, gy) = (rng.range(TS) as i32, y + 2 + rng.range(4) as i32);
            c.hline(x, gy, 4, hex(mul(tone, 0.88)));
        }
    }
}

fn rug(c: &mut Canvas) {
    let n = TS as i32;
    // Seamless woven pattern: a gold diamond lattice with blue centres.
    c.rect(0, 0, n, n, hex(0xa83a3a));
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x - 16).abs(), (y - 16).abs());
            let d = dx + dy;
            if d == 15 || d == 16 {
                c.px(x, y, hex(0xe8c05a));
            } else if d == 13 || d == 18 {
                c.px(x, y, hex(0x7e2626));
            } else if d < 4 {
                c.px(x, y, hex(if d < 2 { 0x6f97d6 } else { 0x3f6fb5 }));
            } else if (x + y) % 4 == 0 && d > 6 && d < 11 {
                c.px(x, y, hex(0xc04848));
            }
        }
    }
}

fn forest_floor(c: &mut Canvas, rng: &mut Rng) {
    c.ground(0x356b33, 0x427d3d, 0x2b5a29, rng, 0.25);
    for _ in 0..4 {
        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
        let col = [0x8a6d2e, 0x6b8a2e, 0xa0522d][rng.range(3) as usize];
        c.wrap(x, y, hex(col));
        c.wrap(x + 1, y, hex(col));
        c.wrap(x + 1, y + 1, hex(mul(col, 0.8)));
    }
    for _ in 0..2 {
        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
        for k in 0..4 {
            c.wrap(x + k, y + k / 2, hex(0x5a4020));
        }
    }
}

fn snow(c: &mut Canvas, rng: &mut Rng) {
    c.ground(0xeef2f7, 0xffffff, 0xd8e0ec, rng, 0.14);
    for _ in 0..4 {
        c.px(rng.range(TS) as i32, rng.range(TS) as i32, hex(0xb8c8e0));
    }
}

fn path(c: &mut Canvas, rng: &mut Rng) {
    c.ground(0xc2a46e, 0xd4b886, 0xab8d5a, rng, 0.22);
    for _ in 0..5 {
        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
        c.wrap(x, y, hex(0x9c8158));
        c.wrap(x + 1, y, hex(0x9c8158));
        c.wrap(x, y - 1, hex(0xe6d2a6));
    }
}

/// A water frame: the same speckles each frame; the wave crests drift right by `shift`.
fn water(c: &mut Canvas, shift: i32) {
    let mut wrng = Rng(77);
    c.ground(0x3a72c8, 0x4682d6, 0x3268ba, &mut wrng, 0.12);
    for (k, x) in [(0, 3), (1, 18), (2, 9), (3, 25)] {
        let y = 4 + k * 8;
        for d in 0..7 {
            let lift = if d == 0 || d == 6 { 1 } else { 0 };
            c.wrap(x + shift + d, y + lift, hex(0x9ccbf5));
            c.wrap(x + shift + d, y + 1 + lift, hex(0x5a92dc));
        }
    }
    c.wrap(12 + shift * 2, 15, hex(0xffffff));
}

/// How the edge tiles of an auto-tiling terrain look (`edge_block`).
pub struct EdgeArt<'a> {
    /// Draws the terrain filling a tile at the canvas origin (argument: the neighbour mask).
    pub inside: &'a dyn Fn(&mut Canvas, usize),
    /// Draws what surrounds the terrain, filling a tile at the canvas origin.
    pub outside: &'a dyn Fn(&mut Canvas, usize),
    /// Wobbly sides with round corners (ground, water); false: straight sides (rugs, paving).
    pub natural: bool,
    /// Width of the surroundings' band on the open sides, in px.
    pub band: f32,
    /// Colour of the terrain's rim along its border (None: the terrain a little darker).
    pub rim_in: Option<u32>,
    /// Width of that rim, in px.
    pub rim_w: i32,
    /// Colour of the surroundings' pixels touching the terrain (None: unchanged).
    pub rim_out: Option<u32>,
}

impl EdgeArt<'_> {
    /// Whether pixel `(x, y)` of the tile for neighbour mask `mask` shows the surroundings: a
    /// band along every open side (whose neighbour isn't the terrain), with rounded corners. The
    /// wobble repeats every 32 px, so the bands of neighbouring tiles join up.
    fn outside_at(&self, mask: usize, x: i32, y: i32) -> bool {
        use rustpg2d_engine::autotile::{EAST, NORTH, SOUTH, WEST};
        let natural = self.natural;
        let wob = |p: i32| {
            if !natural {
                return 0.0;
            }
            let a = p as f32 * std::f32::consts::TAU / 32.0;
            1.2 * (2.0 * a).sin() + 0.7 * (3.0 * a + 1.0).sin()
        };
        let open = |bit: usize| mask & bit == 0;
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let w = self.band;
        let sides = [(NORTH, fy, w + wob(x)), (SOUTH, 32.0 - fy, w + wob(x + 16)), (WEST, fx, w + wob(y + 8)), (EAST, 32.0 - fx, w + wob(y + 24))];
        if sides.iter().any(|&(bit, d, b)| open(bit) && d < b) {
            return true;
        }
        let r = if natural { 6.0 } else { 2.5 };
        for (a, b) in [(0, 2), (0, 3), (1, 2), (1, 3)] {
            let ((ba, da, wa), (bb, db, wb)) = (sides[a], sides[b]);
            if open(ba) && open(bb) && da < wa + r && db < wb + r {
                let (u, v) = (wa + r - da, wb + r - db);
                if u * u + v * v > r * r {
                    return true;
                }
            }
        }
        // Inner corners (blob sets): both sides are the terrain but the diagonal isn't, so the
        // bands of the two neighbours meet in this corner (a notch, rounded towards the middle).
        use rustpg2d_engine::autotile::{NORTH_EAST, NORTH_WEST, SOUTH_EAST, SOUTH_WEST};
        let ri = if natural { w * 0.7 } else { 1.5 };
        for (diag, a, b) in [(NORTH_EAST, 0, 3), (SOUTH_EAST, 1, 3), (SOUTH_WEST, 1, 2), (NORTH_WEST, 0, 2)] {
            let ((ba, da, wa), (bb, db, wb)) = (sides[a], sides[b]);
            if open(diag) && !open(ba) && !open(bb) && da < wa && db < wb {
                let (u, v) = (da - (wa - ri), db - (wb - ri));
                if u <= 0.0 || v <= 0.0 || u * u + v * v < ri * ri {
                    return true;
                }
            }
        }
        false
    }
}

/// A 4×4 block of a terrain's edge tiles in neighbour-mask order (row by row), its top-left
/// tile at index `first` of a sheet `cols` tiles wide.
pub fn edge_block(c: &mut Canvas, first: u32, cols: u32, art: &EdgeArt) {
    for k in 0..16usize {
        // The 16-tile set assumes the diagonals are the terrain (no inner corners).
        edge_tile(c, rustpg2d_engine::autotile::Terrain::block_tile(first, cols, k), cols, art, k | 0xF0, k);
    }
}

/// A 7×7 block of a terrain's 47 blob tiles (inner corners too) in slot order
/// (`autotile::BLOB_MASKS`), its top-left tile at index `first` of a sheet `cols` tiles wide.
pub fn blob_block(c: &mut Canvas, first: u32, cols: u32, art: &EdgeArt) {
    use rustpg2d_engine::autotile::{Terrain, BLOB_MASKS};
    for (s, &m) in BLOB_MASKS.iter().enumerate() {
        edge_tile(c, Terrain::blob_block_tile(first, cols, s), cols, art, m as usize, 16 + s);
    }
}

/// One edge tile at sheet index `i` for an 8-neighbour `mask`; `seed` varies the textures.
fn edge_tile(c: &mut Canvas, i: u32, cols: u32, art: &EdgeArt, mask: usize, seed: usize) {
    let n = TS as i32;
    let k = mask;
    {
        c.at(i % cols * TS, i / cols * TS);
        (art.inside)(c, seed);
        let mut g = Canvas::new(TS, TS);
        (art.outside)(&mut g, seed);
        let out = |x: i32, y: i32| (0..n).contains(&x) && (0..n).contains(&y) && art.outside_at(k, x, y);
        // Distance (in px, up to the rim width) from an inside pixel to the surroundings.
        let near = |x: i32, y: i32, d: i32| (-d..=d).any(|dy| (-d..=d).any(|dx| (dx != 0 || dy != 0) && dx.abs() + dy.abs() <= d && out(x + dx, y + dy)));
        for y in 0..n {
            for x in 0..n {
                if out(x, y) {
                    let touching = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| (0..n).contains(&(x + dx)) && (0..n).contains(&(y + dy)) && !out(x + dx, y + dy));
                    c.px(x, y, match art.rim_out {
                        Some(col) if touching => hex(col),
                        _ => g.get(x, y),
                    });
                } else if near(x, y, art.rim_w) {
                    let col = c.get(x, y);
                    c.px(x, y, art.rim_in.map_or(Color::new(col.r * 0.8, col.g * 0.8, col.b * 0.8, 1.0), hex));
                }
            }
        }
    }
}

/// Where a terrain's 4×4 block starts: blocks go two per band of four rows, from row `row0`
/// of a sheet `cols` (8) tiles wide.
pub fn block_start(row0: u32, block: u32) -> u32 {
    (row0 + block / 2 * 4) * 8 + block % 2 * 4
}

/// The terrain tileset's auto-tiling terrains, in block order (rows 5–24 of terrain.png), with
/// the tile each terrain's inside uses (the ordinary tile, so painted areas match the maps;
/// `None` = the block's own full tile), what erasing leaves, and the terrains lying on it.
const TERRAINS: [(&str, Option<u32>, u32, &[&str]); 10] = [
    ("Dirt path", Some(t::PATH), t::GRASS, &["Forest path"]),
    ("Pond", Some(t::WATER), t::GRASS, &[]),
    ("Sand", Some(t::SAND), t::GRASS, &["Shore"]),
    // Its own inside tile (animated like water): a tile belongs to one terrain only.
    ("Shore", None, t::SAND, &[]),
    ("Forest floor", Some(t::FOREST_FLOOR), t::GRASS, &["Snowfield", "Forest path", "Stream"]),
    ("Snowfield", Some(t::SNOW), t::FOREST_FLOOR, &[]),
    ("Forest path", None, t::FOREST_FLOOR, &["Dirt path"]),
    ("Stream", None, t::FOREST_FLOOR, &[]),
    ("Flagstones", Some(t::STONE_FLOOR), t::GRASS, &[]),
    ("Rug", Some(t::RUG), t::WOOD_FLOOR, &[]),
];

/// Index of a terrain in `terrains()` by name.
pub fn terrain_index(name: &str) -> usize {
    TERRAINS.iter().position(|t| t.0 == name).unwrap()
}

/// The terrain tileset's auto-tiling terrains.
pub fn terrains() -> Vec<rustpg2d_engine::autotile::Terrain> {
    TERRAINS
        .iter()
        .enumerate()
        .map(|(b, &(name, inside, base, joins))| {
            let mut tr = rustpg2d_engine::autotile::Terrain { name: name.into(), base: Some(base), joins: joins.iter().map(|j| j.to_string()).collect(), ..Default::default() };
            tr.fill_from_block(block_start(5, b as u32), 8);
            if let Some(i) = inside {
                tr.tiles[15] = Some(i);
            }
            if let Some(j) = BLOB_TERRAINS.iter().position(|n| *n == name) {
                tr.fill_blob_from_block(blob_start(j), 8);
                // The inside stays the ordinary tile, like the 16-tile set's.
                if let Some(i) = inside {
                    tr.blob[rustpg2d_engine::autotile::BLOB_SLOTS - 1] = Some(i);
                }
            }
            tr
        })
        .collect()
}

/// Animations of the terrain tileset: the water, and the insides of the water terrains with
/// their own tiles (Shore, Stream) cycling through the same frames.
pub fn terrain_anims() -> Vec<rustpg2d_engine::defs::TileAnim> {
    let frames = vec![t::WATER, t::WATER_2, t::WATER_3, t::WATER_2];
    let mut v = vec![rustpg2d_engine::defs::TileAnim { tile: t::WATER, frames: frames.clone(), fps: 3.0 }];
    for name in ["Shore", "Stream"] {
        let inside = rustpg2d_engine::autotile::Terrain::block_tile(block_start(5, terrain_index(name) as u32), 8, 15);
        v.push(rustpg2d_engine::defs::TileAnim { tile: inside, frames: frames.clone(), fps: 3.0 });
    }
    v
}

/// Solid tiles of the terrain tileset: `SOLID` plus the edge tiles of the water terrains
/// (water you can't walk into, with a bank drawn on it).
pub fn terrain_solid() -> Vec<u32> {
    let water = ["Pond", "Shore", "Stream"].map(terrain_index);
    let mut v: Vec<u32> = SOLID.to_vec();
    for b in water {
        v.extend((0..16).map(|k| rustpg2d_engine::autotile::Terrain::block_tile(block_start(5, b as u32), 8, k)));
    }
    v
}

/// Draws the edge blocks of `TERRAINS` into terrain.png.
fn terrain_edges(c: &mut Canvas) {
    let grass_out = |g: &mut Canvas, k: usize| grass(g, &mut Rng(900 + k as u64));
    let forest_out = |g: &mut Canvas, k: usize| forest_floor(g, &mut Rng(930 + k as u64));
    let sand_out = |g: &mut Canvas, k: usize| sand(g, &mut Rng(960 + k as u64));
    let wood_out = |g: &mut Canvas, k: usize| wood_floor(g, &mut Rng(990 + k as u64));
    let path_in = |c: &mut Canvas, k: usize| path(c, &mut Rng(500 + k as u64));
    let water_in = |c: &mut Canvas, _: usize| water(c, 0);
    let arts: [EdgeArt; 10] = [
        EdgeArt { inside: &path_in, outside: &grass_out, natural: true, band: 6.0, rim_in: None, rim_w: 1, rim_out: Some(0x3d7a33) },
        EdgeArt { inside: &water_in, outside: &grass_out, natural: true, band: 6.0, rim_in: Some(0x2c5aa8), rim_w: 1, rim_out: Some(0x3d7a33) },
        EdgeArt { inside: &|c, k| sand(c, &mut Rng(600 + k as u64)), outside: &grass_out, natural: true, band: 6.0, rim_in: Some(0xcdb878), rim_w: 1, rim_out: Some(0x3d7a33) },
        EdgeArt { inside: &water_in, outside: &sand_out, natural: true, band: 6.0, rim_in: Some(0xbfe3f7), rim_w: 1, rim_out: Some(0xc9b47a) },
        EdgeArt { inside: &|c, k| forest_floor(c, &mut Rng(700 + k as u64)), outside: &grass_out, natural: true, band: 7.0, rim_in: Some(0x2b5a29), rim_w: 1, rim_out: None },
        EdgeArt { inside: &|c, k| snow(c, &mut Rng(800 + k as u64)), outside: &forest_out, natural: true, band: 6.0, rim_in: Some(0xc8d4e6), rim_w: 1, rim_out: Some(0x2b5a29) },
        EdgeArt { inside: &path_in, outside: &forest_out, natural: true, band: 6.0, rim_in: None, rim_w: 1, rim_out: Some(0x2b5a29) },
        EdgeArt { inside: &water_in, outside: &forest_out, natural: true, band: 5.0, rim_in: Some(0x2c5aa8), rim_w: 1, rim_out: Some(0x2b5a29) },
        EdgeArt { inside: &|c, k| stone_floor(c, &mut Rng(1000 + k as u64)), outside: &grass_out, natural: false, band: 3.0, rim_in: Some(0x5d5d66), rim_w: 2, rim_out: Some(0x3d7a33) },
        EdgeArt { inside: &|c, _| rug(c), outside: &wood_out, natural: false, band: 3.0, rim_in: Some(0xe8c05a), rim_w: 2, rim_out: Some(0x5e3d1e) },
    ];
    for (b, art) in arts.iter().enumerate() {
        edge_block(c, block_start(5, b as u32), 8, art);
    }
    for (j, name) in BLOB_TERRAINS.iter().enumerate() {
        blob_block(c, blob_start(j), 8, &arts[terrain_index(name)]);
    }
}

/// Terrains that also have a 47-tile blob set (inner corners), in block order below the 4×4
/// blocks (rows 25.. of terrain.png, one 7×7 block each).
const BLOB_TERRAINS: [&str; 2] = ["Dirt path", "Sand"];

/// Where blob block `j` starts.
fn blob_start(j: usize) -> u32 {
    (25 + 7 * j as u32) * 8
}

// ------------------------------------------------------------------ characters

#[derive(Clone, Default)]
pub struct Look {
    pub skin: u32,
    pub hair: u32,
    pub shirt: u32,
    pub pants: u32,
    pub beard: bool,
    /// Pointy ears (goblins).
    pub ears: bool,
    pub hat: Hat,
    /// A long robe or dress (covers the legs).
    pub robe: Option<u32>,
    pub apron: Option<u32>,
    /// Cloth over the lower face (nomads, bandits).
    pub veil: Option<u32>,
    /// Bones instead of skin and clothes.
    pub skeleton: bool,
    /// Female look: a feminine body (narrow waist, bust, slim limbs, rounder chin), lashes
    /// and lips, long hair by default, outfits in their fitted cut.
    pub female: bool,
    pub hair_style: HairStyle,
    pub beard_style: BeardStyle,
}

impl Look {
    fn hair_style(&self) -> HairStyle {
        match self.hair_style {
            HairStyle::Auto if self.female => HairStyle::Long,
            HairStyle::Auto => HairStyle::Short,
            s => s,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum HairStyle {
    /// Long for women, short for men.
    #[default]
    Auto,
    Short,
    Long,
    /// Short on top with a tail at the back.
    Ponytail,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum BeardStyle {
    #[default]
    Full,
    /// A small chin beard.
    Goatee,
}

#[derive(Clone, Copy, Default)]
pub enum Hat {
    #[default]
    None,
    Straw(u32),
    Wizard(u32),
    /// Head wrap / turban.
    Wrap(u32),
    Helmet,
    Cap(u32),
    Tiara,
    Hood(u32),
}

#[derive(Clone, Copy)]
pub enum Gear {
    Cap,
    Helmet,
    Leather,
    Chain,
}

/// Character frames are 32x48; feet at the bottom.
const FW: u32 = 32;
const FH: u32 = 48;

/// Head bob and leg lift of walk frame `frame`.
fn walk(frame: u32) -> (i32, i32, i32) {
    let bob = if frame % 2 == 1 { 1 } else { 0 };
    let (l, r) = match frame {
        1 => (2, 0),
        3 => (0, 2),
        _ => (0, 0),
    };
    (bob, l, r)
}

/// Top of the head and of the torso for a walk frame.
fn body_y(frame: u32) -> (i32, i32) {
    let (bob, ..) = walk(frame);
    (5 + bob, 22 + bob)
}

/// Which part of a person to draw: everything (pre-drawn sheets), or one layer of a layered
/// model - the body in underwear (skin in the magenta reference ramp), hair or beard (cyan
/// reference ramp), or the outfit (clothes, shoes, hats).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Full,
    Body,
    Hair,
    Beard,
    Outfit,
}

/// Skin / hair reference shades the engine recolors (`render::recolor`).
pub const SKIN_REF: [u32; 4] = [0x6e006e, 0x9c009c, 0xc800c8, 0xf000f0];
pub const HAIR_REF: [u32; 4] = [0x006e6e, 0x009c9c, 0x00c8c8, 0x00f0f0];
/// Underwear of the bodies.
const UNDERWEAR: [u32; 4] = [0x8a8478, 0xb8b0a0, 0xdcd4c4, 0xf2ece0];

fn draw_person(c: &mut Canvas, look: &Look, f: Facing, frame: u32) {
    draw_part(c, look, f, frame, Part::Full);
}

fn draw_part(c: &mut Canvas, look: &Look, f: Facing, frame: u32, part: Part) {
    let full = part == Part::Full;
    let (body, outfit, hairs) = (full || part == Part::Body, full || part == Part::Outfit, full || part == Part::Hair);
    let (_, l_up, r_up) = walk(frame);
    let (hy, by) = body_y(frame);
    let skin = if full { ramp(look.skin) } else { SKIN_REF };
    let hair = if full { ramp(look.hair) } else { HAIR_REF };
    let shirt = ramp(look.shirt);
    let pants = ramp(look.pants);
    let boots = [0x24180e, 0x3a2a1e, 0x4e3a28, 0x6a5038];
    let side = matches!(f, Facing::Left | Facing::Right);
    let dir = if f == Facing::Left { -1 } else { 1 };
    let swing = match frame {
        1 => 2,
        3 => -2,
        _ => 0,
    };

    // Legs (or a robe reaching the ground); the body has bare legs and feet.
    // Women: slimmer limbs (3 / 4 px instead of 4 / 5), a narrow waist and a fitted cut.
    let fem = look.female;
    let (leg_w, arm_w) = if fem { (4, 3) } else { (5, 4) };
    let (leg_l, leg_r) = if fem { (12, 17) } else { (11, 17) };
    let (arm_l, arm_r) = if fem { (7, 23) } else { (5, 24) };
    if part == Part::Body {
        let bare = |c: &mut Canvas, x: i32, lift: i32, pal: [u32; 4]| c.shade_box(x, 34 - lift, leg_w, 11, pal);
        if side {
            bare(c, 14 - swing * dir, r_up, [skin[0], skin[0], skin[1], skin[2]]);
            bare(c, 14 + swing * dir, l_up, skin);
        } else {
            bare(c, leg_l, l_up, skin);
            bare(c, leg_r, r_up, skin);
        }
    } else if outfit {
        if let Some(col) = look.robe {
            let pal = ramp(col);
            for (k, y) in (by + 9..45).enumerate() {
                // A woman's dress flares from the waist into a skirt.
                let half = if fem { 6 + (k as i32) * 9 / 35 } else { 7 + (k as i32) / 4 };
                c.hline(16 - half, y, 2 * half + 1, hex(pal[2]));
                c.px(16 - half, y, hex(pal[3]));
                c.px(16 + half, y, hex(pal[1]));
            }
            c.hline(9, 44, 15, hex(pal[0]));
            // Shoes peeking out while walking.
            if l_up > 0 || r_up > 0 {
                c.rect(if l_up > 0 { 11 } else { 18 }, 44, 4, 2, hex(boots[1]));
            }
        } else if side {
            let leg = |c: &mut Canvas, x: i32, lift: i32, pal: [u32; 4]| {
                c.shade_box(x, 34 - lift, leg_w, 9, pal);
                c.shade_box(x, 41 - lift, leg_w, 4, boots);
            };
            leg(c, 14 - swing * dir, r_up, [pants[0], pants[0], pants[1], pants[2]]);
            leg(c, 14 + swing * dir, l_up, pants);
        } else {
            let leg = |c: &mut Canvas, x: i32, lift: i32, pal: [u32; 4]| {
                c.shade_box(x, 34 - lift, leg_w, 9, pal);
                c.shade_box(x, 41 - lift, leg_w, 4, boots);
            };
            leg(c, leg_l, l_up, pants);
            leg(c, leg_r, r_up, pants);
        }
    }

    // Hood mass behind the head.
    if let (Hat::Hood(col), true) = (look.hat, outfit) {
        c.rrect(6, hy - 3, 21, 22, 6, hex(ramp(col)[1]));
    }
    // Back arm (side view) goes behind the torso.
    if side && (body || outfit) {
        let pal = if part == Part::Body { [skin[0], skin[0], skin[1], skin[1]] } else { [shirt[0], shirt[0], shirt[1], shirt[1]] };
        c.shade_box(14 + swing * dir, by + 2, arm_w, 9, pal);
    }

    // Torso: bare with underwear, or the shirt with a belt. Women: shoulders, a narrow waist
    // and hips (the same shape under the clothes and in their cut).
    let torso = |c: &mut Canvas, pal: [u32; 4]| {
        if fem {
            c.rrect(10, by, 13, 5, 2, hex(pal[2]));
            c.rect(11, by + 4, 11, 5, hex(pal[2]));
            c.rrect(10, by + 8, 13, 5, 2, hex(pal[2]));
            c.vline(11, by + 1, 3, hex(pal[3]));
            c.vline(12, by + 4, 5, hex(pal[3]));
            c.vline(11, by + 9, 3, hex(pal[3]));
            c.vline(21, by + 1, 3, hex(pal[1]));
            c.vline(20, by + 4, 5, hex(pal[1]));
            c.vline(21, by + 9, 3, hex(pal[1]));
        } else {
            c.rrect(9, by, 15, 13, 2, hex(pal[2]));
            c.vline(10, by + 1, 11, hex(pal[3]));
            c.vline(22, by + 1, 11, hex(pal[1]));
        }
    };
    if part == Part::Body {
        torso(c, skin);
        if fem {
            if f == Facing::Down {
                // Bust: a shadow under it, a highlight on top.
                c.hline(12, by + 3, 4, hex(skin[1]));
                c.hline(17, by + 3, 4, hex(skin[1]));
                c.px(13, by + 1, hex(skin[3]));
                c.px(18, by + 1, hex(skin[3]));
            }
            c.shade_box(10, by + 9, 13, 4, UNDERWEAR);
            if f == Facing::Up {
                c.hline(11, by + 2, 11, hex(UNDERWEAR[1]));
            } else {
                c.shade_box(10, by + 1, 13, 3, UNDERWEAR);
            }
        } else {
            c.shade_box(9, by + 9, 15, 4, UNDERWEAR);
        }
    } else if outfit {
        torso(c, shirt);
        if fem {
            c.hline(11, by + 12, 11, hex(shirt[1]));
            c.rect(10, by + 10, 13, 2, hex(0x4a2c14)); // belt
        } else {
            c.hline(10, by + 12, 13, hex(shirt[1]));
            c.rect(9, by + 10, 15, 2, hex(0x4a2c14)); // belt
        }
    }
    if look.skeleton && full {
        // Rib cage over a dark torso.
        let bone = ramp(look.skin);
        c.rrect(9, by, 15, 13, 2, hex(0x2b2620));
        for y in [by + 2, by + 5, by + 8] {
            c.hline(11, y, 11, hex(bone[2]));
        }
        c.vline(16, by + 1, 11, hex(bone[3]));
    }
    if let (Some(col), true) = (look.apron, outfit) {
        let pal = ramp(col);
        let (ax, aw) = if fem { (12, 9) } else { (11, 11) };
        match f {
            Facing::Down => {
                c.shade_box(ax, by + 3, aw, 17, pal);
                c.hline(ax, by + 3, aw, hex(pal[3]));
            }
            Facing::Left => c.shade_box(if fem { 10 } else { 9 }, by + 4, if fem { 5 } else { 6 }, 15, pal),
            Facing::Right => c.shade_box(18, by + 4, if fem { 5 } else { 6 }, 15, pal),
            Facing::Up => c.hline(ax - 1, by + 4, aw + 2, hex(pal[1])),
        }
    }
    if f == Facing::Down && !look.skeleton && outfit {
        c.rect(15, by + 10, 3, 2, hex(GOLD[2]));
        // The neck opening shows the skin (an outfit leaves it open for the body below).
        let neck = if full { hex(skin[2]) } else { CLEAR };
        for (x, y) in [(16, by), (15, by), (17, by), (16, by + 1)] {
            c.px(x, y, neck);
        }
    }

    // Arms: sleeves and hands (the body: bare arms).
    if part == Part::Body {
        if side {
            c.shade_box(14 - swing * dir, by + 2, arm_w, 10, skin);
        } else {
            for (x, s) in [(arm_l, swing), (arm_r, -swing)] {
                c.shade_box(x, by + 1 + s / 2, arm_w, 10, skin);
            }
        }
    } else if outfit {
        if side {
            let x = 14 - swing * dir;
            c.shade_box(x, by + 2, arm_w, 7, shirt);
            if full {
                c.shade_box(x, by + 9, arm_w, 3, skin);
            }
        } else {
            for (x, s) in [(arm_l, swing), (arm_r, -swing)] {
                c.shade_box(x, by + 1 + s / 2, arm_w, 7, shirt);
                if full {
                    c.shade_box(x, by + 8 + s / 2, arm_w, 3, skin);
                }
            }
        }
    }

    // Head.
    let (hx, hw, hh) = (8, 17, 16);
    if body {
        if fem {
            // A rounder, narrower chin.
            c.rrect(hx, hy, hw, hh - 3, 4, hex(skin[2]));
            c.rrect(hx + 1, hy + hh - 5, hw - 2, 5, 3, hex(skin[2]));
            c.hline(hx + 5, hy + hh - 1, hw - 10, hex(skin[1]));
            c.vline(hx + hw - 1, hy + 4, hh - 9, hex(skin[1]));
        } else {
            c.rrect(hx, hy, hw, hh, 4, hex(skin[2]));
            c.hline(hx + 3, hy + hh - 1, hw - 6, hex(skin[1]));
            c.vline(hx + hw - 1, hy + 4, hh - 8, hex(skin[1]));
        }
    }
    if look.ears && body {
        let ear = |c: &mut Canvas, x: i32, d: i32| {
            for k in 0..4 {
                c.vline(x + d * k, hy + 6 + k / 2, 4 - k / 2, hex(skin[if k == 0 { 2 } else { 1 }]));
            }
        };
        match f {
            Facing::Down | Facing::Up => {
                ear(c, hx - 1, -1);
                ear(c, hx + hw, 1);
            }
            Facing::Left => ear(c, hx + hw, 1),
            Facing::Right => ear(c, hx - 1, -1),
        }
    }
    let skeleton = look.skeleton && full;
    let eye = |c: &mut Canvas, x: i32| {
        if skeleton {
            c.rect(x - 1, hy + 7, 4, 4, hex(0x1a1612)); // dark sockets
            c.px(x, hy + 8, hex(0xd84343));
        } else {
            c.rect(x, hy + 8, 2, 3, hex(0x1a1a22));
            c.px(x, hy + 8, hex(0xffffff));
        }
    };
    // Hair cap (lit from the top-left).
    let cap = |c: &mut Canvas, x: i32, w: i32, h: i32| {
        c.rrect(x, hy - 1, w, h, 4, hex(hair[2]));
        c.hline(x + 4, hy, w - 8, hex(hair[3]));
        c.px(x + 3, hy + 1, hex(hair[3]));
    };
    match f {
        Facing::Down => {
            if hairs {
                cap(c, hx, hw, 7);
                for x in (hx + 1..hx + hw - 1).step_by(3) {
                    c.px(x + 1, hy + 6, hex(hair[1])); // fringe
                }
                c.rect(hx, hy + 4, 2, 7, hex(hair[1]));
                c.rect(hx + hw - 2, hy + 4, 2, 7, hex(hair[1]));
            }
            if body {
                eye(c, 11);
                eye(c, 20);
                if !skeleton {
                    c.px(10, hy + 12, hex(0xe8907a)); // cheeks
                    c.px(22, hy + 12, hex(0xe8907a));
                }
            }
            if skeleton {
                for x in (12..21).step_by(2) {
                    c.px(x, hy + 13, hex(0x1a1612)); // teeth
                }
            } else if look.beard && full || part == Part::Beard {
                match look.beard_style {
                    BeardStyle::Full => {
                        c.rrect(10, hy + 10, 13, 9, 3, hex(hair[2]));
                        c.hline(12, hy + 10, 9, hex(hair[3]));
                        c.hline(14, hy + 13, 5, hex(hair[0]));
                    }
                    BeardStyle::Goatee => {
                        c.rrect(13, hy + 12, 7, 6, 2, hex(hair[2]));
                        c.hline(14, hy + 12, 5, hex(hair[3]));
                        c.hline(14, hy + 14, 5, hex(hair[0]));
                        c.hline(13, hy + 11, 7, hex(hair[1])); // mustache
                    }
                }
            } else if look.female && body {
                c.hline(15, hy + 13, 3, hex(0xc0505a)); // lips
                c.px(16, hy + 14, hex(0xa03a46));
            } else if body {
                c.hline(15, hy + 13, 3, hex(skin[0]));
            }
            if hairs && look.hair_style() == HairStyle::Long {
                // Long hair falling past the face over the shoulders.
                c.rect(hx - 1, hy + 4, 3, 17, hex(hair[2]));
                c.vline(hx - 1, hy + 5, 15, hex(hair[3]));
                c.rect(hx + hw - 2, hy + 4, 3, 17, hex(hair[1]));
                c.px(hx, hy + 21, hex(hair[1]));
                c.px(hx + hw - 1, hy + 21, hex(hair[0]));
            }
            if hairs && look.hair_style() == HairStyle::Ponytail {
                // The tail's tip shows beside the neck.
                c.rect(hx + hw, hy + 12, 2, 7, hex(hair[1]));
            }
            if look.female && body {
                // Eyelashes.
                for x in [11, 20] {
                    c.hline(x - 1, hy + 7, 3, hex(0x1a1a22));
                }
            }
        }
        Facing::Up => {
            if hairs {
                cap(c, hx, hw, hh);
                c.vline(hx + hw - 2, hy + 3, hh - 6, hex(hair[1]));
                c.hline(hx + 3, hy + hh - 2, hw - 6, hex(hair[1]));
                match look.hair_style() {
                    HairStyle::Long => {
                        // Falls over the shoulders, narrowing to a rounded tip.
                        c.rrect(hx + 1, hy + 8, hw - 2, 12, 4, hex(hair[2]));
                        c.rrect(hx + 3, hy + 17, hw - 6, 7, 4, hex(hair[2]));
                        c.vline(hx + hw - 3, hy + 9, 12, hex(hair[1]));
                        c.vline(hx + 3, hy + 9, 11, hex(hair[3]));
                        c.hline(hx + 5, hy + 22, hw - 10, hex(hair[1]));
                    }
                    HairStyle::Ponytail => {
                        c.rect(14, hy + 10, 5, 15, hex(hair[2]));
                        c.vline(14, hy + 11, 13, hex(hair[3]));
                        c.vline(18, hy + 11, 13, hex(hair[1]));
                        c.hline(14, hy + 10, 5, hex(hair[0])); // tie
                        c.hline(15, hy + 24, 3, hex(hair[1]));
                    }
                    _ => {}
                }
            }
        }
        Facing::Left | Facing::Right => {
            let mirror = |x: i32| if f == Facing::Left { x } else { 2 * 16 - x };
            if hairs {
                cap(c, hx, hw, 7);
                // Hair at the back of the head.
                let style = look.hair_style();
                let (bx, bw) = if f == Facing::Left { (hx + 8, 9) } else { (hx, 9) };
                c.rrect(bx, hy + 2, bw, if style == HairStyle::Long { 21 } else { 12 }, 3, hex(hair[2]));
                c.vline(if f == Facing::Left { hx + hw - 2 } else { hx + 1 }, hy + 4, 9, hex(hair[1]));
                if style == HairStyle::Ponytail {
                    // A tail hanging from the back of the head.
                    let tx = if f == Facing::Left { hx + hw } else { hx - 3 };
                    c.rect(tx, hy + 7, 3, 4, hex(hair[2]));
                    c.rect(tx + if f == Facing::Left { 1 } else { -1 }, hy + 10, 3, 11, hex(hair[2]));
                    c.vline(tx + 1, hy + 11, 9, hex(hair[1]));
                    c.px(tx + 1, hy + 7, hex(hair[0])); // tie
                }
            }
            let ex = if f == Facing::Left { 10 } else { 21 };
            if body {
                eye(c, ex);
                if look.female && !skeleton {
                    c.hline(ex - 1, hy + 7, 3, hex(0x1a1a22));
                    c.px(if f == Facing::Left { 9 } else { 23 }, hy + 13, hex(0xc0505a));
                }
                c.px(mirror(8) - if f == Facing::Left { 1 } else { 0 }, hy + 11, hex(skin[2])); // nose
                c.px(mirror(8) - if f == Facing::Left { 1 } else { 0 }, hy + 12, hex(skin[1]));
                if !look.ears {
                    let ear_x = if f == Facing::Left { hx + 6 } else { hx + 9 };
                    c.rect(ear_x, hy + 8, 2, 3, hex(skin[1]));
                }
            }
            if look.beard && full || part == Part::Beard {
                match look.beard_style {
                    BeardStyle::Full => {
                        let x = if f == Facing::Left { hx } else { hx + 7 };
                        c.rrect(x, hy + 10, 10, 9, 3, hex(hair[2]));
                    }
                    BeardStyle::Goatee => {
                        let x = if f == Facing::Left { hx } else { hx + hw - 5 };
                        c.rrect(x, hy + 12, 5, 6, 2, hex(hair[2]));
                        c.hline(x, hy + 11, 5, hex(hair[1]));
                    }
                }
            }
        }
    }
    if outfit {
        // In a layered model a helmet is armor (an item), not part of the outfit.
        if full || !matches!(look.hat, Hat::Helmet) {
            draw_headwear(c, look, f, hy);
        }
    }
}

/// A sheet of one layer of a layered model: the body with outline and shadow; hair, beard and
/// outfit outlined only where they stand out from the (outlined) body, like the full sprites.
pub fn layer_canvas(look: &Look, part: Part) -> Canvas {
    let mut c = Canvas::new(FW * 4, FH * 4);
    let mut body = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            body.at(frame * FW, f as u32 * FH);
            draw_part(&mut body, look, f, frame, Part::Body);
            body.auto_outline(FW as i32, FH as i32, OUTLINE);
            draw_part(&mut c, look, f, frame, part);
            if part == Part::Body {
                c.auto_outline(FW as i32, FH as i32, OUTLINE);
                c.shadow(16.0, 45.5, 10.0, 2.5);
            } else {
                c.outline_outside(&body, FW as i32, FH as i32, OUTLINE);
            }
        }
    }
    c
}

/// Items held in a hand (layered models show them).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Held {
    Sword,
    Axe,
    Spear,
    Bow,
    Crossbow,
    Shield,
    Lantern,
    Torch,
    /// A fishing rod (in the hand while fishing): its tip is where the engine starts the line.
    Rod,
}

impl Held {
    /// In the left hand (shield, lights) or the right one (weapons).
    fn left(self) -> bool {
        matches!(self, Held::Shield | Held::Lantern | Held::Torch)
    }
}

/// Where the right (`left` = false) or left hand is in a frame, and whether it is on the near
/// side (sideways: the far hand is behind the body).
fn hand(f: Facing, frame: u32, left: bool) -> (i32, i32) {
    let (_, by) = body_y(frame);
    let swing = match frame {
        1 => 2,
        3 => -2,
        _ => 0,
    };
    let dir = if f == Facing::Left { -1 } else { 1 };
    match f {
        // Facing down the right hand is on the viewer's left.
        Facing::Down => if left { (26, by + 9 - swing / 2) } else { (7, by + 9 + swing / 2) },
        Facing::Up => if left { (7, by + 9 + swing / 2) } else { (26, by + 9 - swing / 2) },
        _ => {
            let near = (f == Facing::Right) != left;
            let x = if near { 14 - swing * dir } else { 14 + swing * dir };
            (x + 2, by + 10)
        }
    }
}

/// A held-item sheet (character layout).
pub fn held_canvas(kind: Held) -> Canvas {
    let steel = [0x5f6b73, 0x90a4ae, 0xc5d0d6, 0xf0f4f6];
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            let (x, y) = hand(f, frame, kind.left());
            let side = matches!(f, Facing::Left | Facing::Right);
            let fwd = if f == Facing::Left { -1 } else { 1 };
            // Front views: things point away from the body (outward).
            let out = if x < 16 { -1 } else { 1 };
            match kind {
                Held::Sword => {
                    if side {
                        for k in 0..12 {
                            c.rect(x + fwd * (k / 2 + 1), y - 2 - k, 2, 2, hex(steel[if k > 9 { 3 } else { 2 }]));
                        }
                        c.rect(x - 2, y - 2, 5, 2, hex(GOLD[1])); // guard
                        c.rect(x, y, 2, 3, hex(WOOD[1]));
                    } else {
                        c.rect(x - 1, y - 15, 2, 13, hex(steel[2]));
                        c.vline(x - 1, y - 15, 13, hex(steel[3]));
                        c.px(x - 1, y - 16, hex(steel[3]));
                        c.rect(x - 3, y - 2, 6, 2, hex(GOLD[1]));
                        c.rect(x - 1, y, 2, 3, hex(WOOD[1]));
                    }
                }
                Held::Axe => {
                    c.rect(x - 1, y - 12, 2, 15, hex(WOOD[1]));
                    c.vline(x - 1, y - 12, 15, hex(WOOD[2]));
                    let d = if side { fwd } else { out };
                    let hx = if d > 0 { x + 1 } else { x - 6 };
                    c.shade_box(hx, y - 13, 5, 6, steel);
                    c.vline(if d > 0 { hx + 5 } else { hx - 1 }, y - 12, 4, hex(steel[3]));
                }
                Held::Spear => {
                    c.rect(x - 1, y - 24, 2, 34, hex(WOOD[1]));
                    c.vline(x - 1, y - 24, 34, hex(WOOD[2]));
                    for k in 0..5 {
                        c.hline(x - 2 + k / 2, y - 25 - k, 4 - k / 2 * 2 + 1, hex(steel[if k < 2 { 1 } else { 3 }]));
                    }
                }
                Held::Bow => {
                    let d = if side { fwd } else { out };
                    for k in -10..=10i32 {
                        let bend = 3 - (k * k) / 30;
                        c.px(x + d * bend, y + k, hex(WOOD[if k < 0 { 2 } else { 1 }]));
                        c.px(x + d * bend + d, y + k, hex(WOOD[1]));
                    }
                    c.vline(x, y - 9, 19, hex(0xe8e0c8)); // string
                }
                Held::Crossbow => {
                    if side {
                        c.shade_box(if fwd > 0 { x } else { x - 9 }, y - 1, 10, 3, WOOD);
                        c.vline(if fwd > 0 { x + 9 } else { x - 9 }, y - 5, 10, hex(steel[2]));
                    } else {
                        c.shade_box(x - 1, y - 2, 3, 8, WOOD);
                        c.hline(x - 5, y - 2, 11, hex(steel[2]));
                        c.hline(x - 4, y - 3, 9, hex(steel[3]));
                    }
                }
                Held::Shield => {
                    if side {
                        c.shade_box(x - 2, y - 7, 4, 13, WOOD);
                        c.vline(x - 2, y - 7, 13, hex(steel[2]));
                    } else if f == Facing::Down {
                        c.ellipse(x as f32, y as f32 - 1.0, 6.0, 7.0, hex(WOOD[2]));
                        c.ellipse(x as f32, y as f32 - 1.0, 4.5, 5.5, hex(WOOD[1]));
                        c.disc(x as f32, y as f32 - 1.0, 2.0, hex(steel[2]));
                        c.px(x - 1, y - 2, hex(steel[3]));
                    } else {
                        c.ellipse(x as f32, y as f32 - 1.0, 6.0, 7.0, hex(WOOD[0]));
                        c.vline(x, y - 7, 13, hex(WOOD[1]));
                    }
                }
                Held::Lantern => {
                    c.vline(x, y + 1, 2, hex(IRON[2]));
                    c.shade_box(x - 2, y + 3, 5, 7, IRON);
                    c.rect(x - 1, y + 4, 3, 5, hex(0xffd166));
                    c.px(x - 1, y + 4, hex(0xfff2b0));
                }
                Held::Rod => {
                    // From the hand out to the tip (idle frame: the tip at the engine's
                    // `rod_tip` offsets: down (3, 48), up (30, 6), right (29, 12), left (3, 12)).
                    let tip = match f {
                        Facing::Down => (x - 4, y + 16),
                        Facing::Up => (x + 4, y - 25),
                        Facing::Right => (x + 13, y - 20),
                        Facing::Left => (x - 13, y - 20),
                    };
                    let (hx, hy) = (x as f32, y as f32);
                    let (tx, ty) = (tip.0 as f32, tip.1 as f32);
                    let mid = (hx + (tx - hx) * 0.4, hy + (ty - hy) * 0.4);
                    c.stroke(hx, hy, mid.0, mid.1, 1.6, hex(WOOD[1]));
                    c.stroke(mid.0, mid.1, tx, ty, 1.0, hex(WOOD[2]));
                    c.disc(hx + (tx - hx) * 0.12, hy + (ty - hy) * 0.12 + 1.0, 1.6, hex(IRON[2]));
                    c.px(tip.0, tip.1, hex(0xe53935));
                }
                Held::Torch => {
                    c.rect(x - 1, y - 8, 2, 11, hex(WOOD[1]));
                    c.disc(x as f32, y as f32 - 10.0, 2.5, hex(0xe8590c));
                    c.disc(x as f32, y as f32 - 10.5, 1.5, hex(0xffc430));
                    c.px(x, y - 13, hex(0xffc430));
                }
            }
            c.auto_outline(FW as i32, FH as i32, OUTLINE);
        }
    }
    c
}

/// Look of the shared body sheets: male / female, with pointy ears (goblins) or not.
pub fn body_look(female: bool, ears: bool) -> Look {
    Look { female, ears, ..Default::default() }
}

/// Id of the body sheet for a look.
pub fn body_id(look: &Look) -> &'static str {
    match (look.female, look.ears) {
        (false, false) => "body_male",
        (true, false) => "body_female",
        (false, true) => "body_goblin",
        (true, true) => "body_goblin_female",
    }
}

/// Id of the hair sheet for a look ("" = none).
pub fn hair_id(look: &Look) -> &'static str {
    match look.hair_style() {
        HairStyle::Short | HairStyle::Auto => "hair_short",
        HairStyle::Long => "hair_long",
        HairStyle::Ponytail => "hair_ponytail",
    }
}

/// Id of the beard sheet for a look ("" = none).
pub fn beard_id(look: &Look) -> &'static str {
    match (look.beard, look.beard_style) {
        (false, _) => "",
        (true, BeardStyle::Full) => "beard_full",
        (true, BeardStyle::Goatee) => "beard_goatee",
    }
}

fn rgb(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// The layered model of a look: body, skin, hair style, beard.
pub fn appearance_of(look: &Look) -> rustpg2d_engine::defs::Appearance {
    rustpg2d_engine::defs::Appearance { body: body_id(look).into(), skin: rgb(look.skin), hair: hair_id(look).into(), hair_color: rgb(look.hair), beard: beard_id(look).into() }
}

/// The shared layer sheets of layered characters: (sheet id, canvas, kind, female body).
pub fn shared_layers() -> Vec<(String, Canvas, rustpg2d_engine::defs::SheetKind, bool)> {
    use rustpg2d_engine::defs::SheetKind as K;
    let hair = |s: HairStyle| Look { hair_style: s, ..Default::default() };
    let beard = |s: BeardStyle| Look { beard: true, beard_style: s, ..Default::default() };
    let mut v = vec![
        ("body_male".to_string(), layer_canvas(&body_look(false, false), Part::Body), K::Body, false),
        ("body_female".to_string(), layer_canvas(&body_look(true, false), Part::Body), K::Body, true),
        ("body_goblin".to_string(), layer_canvas(&body_look(false, true), Part::Body), K::Body, false),
        ("body_goblin_female".to_string(), layer_canvas(&body_look(true, true), Part::Body), K::Body, true),
        ("hair_short".to_string(), layer_canvas(&hair(HairStyle::Short), Part::Hair), K::Hair, false),
        ("hair_long".to_string(), layer_canvas(&hair(HairStyle::Long), Part::Hair), K::Hair, false),
        ("hair_ponytail".to_string(), layer_canvas(&hair(HairStyle::Ponytail), Part::Hair), K::Hair, false),
        ("beard_full".to_string(), layer_canvas(&beard(BeardStyle::Full), Part::Beard), K::Beard, false),
        ("beard_goatee".to_string(), layer_canvas(&beard(BeardStyle::Goatee), Part::Beard), K::Beard, false),
    ];
    for (id, kind) in [
        ("sword", Held::Sword),
        ("axe", Held::Axe),
        ("spear", Held::Spear),
        ("bow", Held::Bow),
        ("crossbow", Held::Crossbow),
        ("shield", Held::Shield),
        ("lantern", Held::Lantern),
        ("torch", Held::Torch),
        ("rod", Held::Rod),
    ] {
        v.push((format!("held_{id}"), held_canvas(kind), K::Held, false));
    }
    v
}

/// Both cuts of an outfit: (sheet id, canvas) of the regular and the female cut.
pub fn outfit_cuts(id: &str, look: &Look) -> [(String, Canvas); 2] {
    let f = Look { female: true, ..look.clone() };
    let m = Look { female: false, ..look.clone() };
    [(format!("outfit_{id}"), layer_canvas(&m, Part::Outfit)), (format!("outfit_{id}_f"), layer_canvas(&f, Part::Outfit))]
}

/// 32 x 32 inventory icons of outfits (the front view from the shoulders down), one per look.
pub fn outfit_icons(looks: &[Look]) -> Canvas {
    let mut c = Canvas::new(TS * looks.len().max(1) as u32, TS);
    for (k, look) in looks.iter().enumerate() {
        let src = layer_canvas(look, Part::Outfit);
        c.at(k as u32 * TS, 0);
        // Front idle frame, rows 13..45 (shoulders to feet).
        for y in 0..TS as i32 {
            for x in 0..TS as i32 {
                let p = src.get(x, y + 13);
                if p.a > 0.0 {
                    c.px(x, y, p);
                }
            }
        }
    }
    c
}

/// A clothing item wearing outfit sheet `outfit_<overlay>` (`_f` = the female cut), with icon
/// `icon_frame` of `icon_sheet`.
pub fn clothing_item(id: &str, name: &str, description: &str, icon_sheet: &str, icon_frame: u32, overlay: &str, value: u32) -> ItemDef {
    ItemDef {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        icon: SpriteRef::new(icon_sheet, icon_frame),
        consumable: false,
        on_use: vec![],
        weapon: None,
        armor: Some(Armor { slot: EquipSlot::Clothing, defense: 0, speed: 0.0, overlay: format!("outfit_{overlay}"), overlay_f: format!("outfit_{overlay}_f") }),
        max_stack: 1,
        value,
        light: None,
        teaches: String::new(),
        held: String::new(),
        tool: String::new(),
        use_on: Default::default(),
        crop: None,
        food: false,
    }
}


/// Hats, hoods and veils (drawn over the finished head).
fn draw_headwear(c: &mut Canvas, look: &Look, f: Facing, hy: i32) {
    let side = matches!(f, Facing::Left | Facing::Right);
    let back = if f == Facing::Left { 1 } else { -1 }; // direction of the back of the head
    match look.hat {
        Hat::None => {}
        Hat::Straw(col) => {
            let p = ramp(col);
            c.ellipse(16.0, hy as f32 + 2.5, 15.0, 3.5, hex(p[1]));
            c.rrect(10, hy - 5, 13, 8, 3, hex(p[2]));
            c.hline(12, hy - 4, 8, hex(p[3]));
            c.hline(10, hy + 1, 13, hex(0xb03a2e)); // band
        }
        Hat::Wizard(col) => {
            let p = ramp(col);
            c.ellipse(16.0, hy as f32 + 1.5, 13.0, 3.0, hex(p[1]));
            // A cone whose tip flops over to the side (there is only ~5 px above the head).
            let tip = if side { -back } else { 1 };
            for row in 0..6 {
                let half = 8.0 - row as f32 * 1.2;
                let cx = 16.0 + (row * row) as f32 / 8.0 * tip as f32;
                let y = hy + 1 - row;
                for x in (cx - half).round() as i32..=(cx + half).round() as i32 {
                    let shade = if (x as f32) < cx - half / 3.0 { p[3] } else if (x as f32) > cx + half / 2.0 { p[1] } else { p[2] };
                    c.px(x, y, hex(shade));
                }
            }
            for k in 0..6 {
                c.px(16 + tip * (5 + k), hy - 4 + k / 2, hex(p[1])); // drooping tip
            }
            c.px(16 + tip * 11, hy - 1, hex(GOLD[3]));
            c.px(13, hy - 1, hex(GOLD[3]));
            c.px(18, hy - 3, hex(GOLD[3]));
        }
        Hat::Wrap(col) => {
            let p = ramp(col);
            c.rrect(7, hy - 3, 19, 9, 4, hex(p[2]));
            c.hline(9, hy, 15, hex(p[1]));
            c.hline(10, hy + 3, 13, hex(p[1]));
            c.hline(10, hy - 2, 10, hex(p[3]));
            if side {
                let x = if back > 0 { 23 } else { 7 };
                c.rect(x, hy + 3, 3, 10, hex(p[1])); // loose end
            }
        }
        Hat::Helmet => {
            let p = ramp(0x9ea7b0);
            c.rrect(7, hy - 3, 19, 11, 5, hex(p[2]));
            c.hline(11, hy - 2, 10, hex(p[3]));
            c.hline(7, hy + 6, 19, hex(p[1]));
            match f {
                Facing::Down => {
                    c.shade_box(7, hy + 6, 4, 8, p);
                    c.shade_box(22, hy + 6, 4, 8, p);
                    c.rect(16, hy + 6, 2, 5, hex(p[1]));
                }
                Facing::Up => c.rrect(7, hy - 3, 19, 18, 5, hex(p[2])),
                _ => {
                    let x = if back > 0 { 16 } else { 8 };
                    c.shade_box(x, hy + 6, 9, 9, p);
                }
            }
            c.rect(14, hy - 6, 5, 4, hex(0xb03a2e)); // plume
        }
        Hat::Cap(col) => {
            let p = ramp(col);
            c.rrect(8, hy - 2, 17, 7, 3, hex(p[2]));
            c.hline(11, hy - 1, 10, hex(p[3]));
            match f {
                Facing::Down => c.rect(7, hy + 4, 19, 2, hex(p[0])),
                Facing::Left => c.rect(4, hy + 4, 12, 2, hex(p[0])),
                Facing::Right => c.rect(17, hy + 4, 12, 2, hex(p[0])),
                Facing::Up => {}
            }
        }
        Hat::Tiara => {
            if f != Facing::Up {
                c.hline(10, hy + 1, 13, hex(GOLD[2]));
                c.px(12, hy, hex(GOLD[3]));
                c.px(20, hy, hex(GOLD[3]));
                if f == Facing::Down {
                    c.rect(15, hy - 1, 3, 2, hex(0xd84343));
                }
            } else {
                c.hline(10, hy + 1, 13, hex(GOLD[1]));
            }
        }
        Hat::Hood(col) => {
            let p = ramp(col);
            match f {
                Facing::Down => {
                    c.rrect(7, hy - 3, 19, 8, 4, hex(p[2]));
                    c.rect(7, hy + 3, 3, 12, hex(p[2]));
                    c.rect(23, hy + 3, 3, 12, hex(p[1]));
                    c.hline(11, hy - 2, 10, hex(p[3]));
                }
                Facing::Up => c.rrect(7, hy - 3, 19, 20, 6, hex(p[2])),
                _ => {
                    let x = if back > 0 { 15 } else { 7 };
                    c.rrect(x, hy - 3, 11, 19, 5, hex(p[2]));
                    c.rrect(7, hy - 3, 19, 7, 4, hex(p[2]));
                }
            }
        }
    }
    if let Some(col) = look.veil {
        let p = ramp(col);
        match f {
            Facing::Down => {
                c.rrect(9, hy + 10, 15, 6, 2, hex(p[2]));
                c.hline(10, hy + 12, 13, hex(p[1]));
            }
            Facing::Left => c.rrect(7, hy + 10, 10, 6, 2, hex(p[2])),
            Facing::Right => c.rrect(16, hy + 10, 10, 6, 2, hex(p[2])),
            Facing::Up => {}
        }
    }
}


/// Armor drawn over the character sheet layout (same frames, same head bob).
pub fn overlay(out: &Path, name: &str, gear: Gear) {
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            let (hy, by) = body_y(frame);
            let side = matches!(f, Facing::Left | Facing::Right);
            let outline = match gear {
                Gear::Cap => {
                    let pal = ramp(0x8d5a2b);
                    c.rrect(7, hy - 2, 19, 8, 4, hex(pal[2]));
                    c.hline(11, hy - 1, 10, hex(pal[3]));
                    c.hline(8, hy + 4, 17, hex(pal[1]));
                    match f {
                        Facing::Down => c.rect(6, hy + 5, 21, 2, hex(pal[0])),
                        Facing::Left => c.rect(4, hy + 5, 13, 2, hex(pal[0])),
                        Facing::Right => c.rect(16, hy + 5, 13, 2, hex(pal[0])),
                        Facing::Up => c.rrect(7, hy - 2, 19, 11, 4, hex(pal[2])),
                    }
                    0x3a2410
                }
                Gear::Helmet => {
                    let pal = ramp(0x9ea7b0);
                    c.rrect(7, hy - 3, 19, 11, 5, hex(pal[2]));
                    c.hline(11, hy - 2, 10, hex(pal[3]));
                    c.rect(10, hy - 1, 3, 2, hex(0xffffff));
                    c.hline(7, hy + 6, 19, hex(pal[1]));
                    match f {
                        Facing::Down => {
                            c.shade_box(7, hy + 6, 4, 9, pal); // cheek guards
                            c.shade_box(22, hy + 6, 4, 9, pal);
                            c.rect(16, hy + 6, 2, 6, hex(pal[1])); // nose guard
                        }
                        Facing::Left => c.shade_box(16, hy + 6, 9, 9, pal),
                        Facing::Right => c.shade_box(8, hy + 6, 9, 9, pal),
                        Facing::Up => c.rrect(7, hy - 3, 19, 19, 5, hex(pal[2])),
                    }
                    0x2f353b
                }
                Gear::Leather | Gear::Chain => {
                    let pal = match gear {
                        Gear::Leather => ramp(0x8d5a2b),
                        _ => ramp(0xa7b1b9),
                    };
                    c.rrect(9, by, 15, 11, 2, hex(pal[2]));
                    c.vline(10, by + 1, 9, hex(pal[3]));
                    c.vline(22, by + 1, 9, hex(pal[1]));
                    c.rect(9, by + 9, 15, 2, hex(pal[0])); // belt
                    if !side {
                        c.shade_box(4, by, 6, 4, pal); // shoulder pads
                        c.shade_box(23, by, 6, 4, pal);
                    }
                    if matches!(gear, Gear::Chain) {
                        for y in (by + 2..by + 9).step_by(2) {
                            for x in (11 + (y % 2)..22).step_by(2) {
                                c.px(x, y, hex(pal[1]));
                            }
                        }
                    } else {
                        c.vline(16, by + 1, 8, hex(pal[1])); // lacing
                        c.px(15, by + 3, hex(pal[1]));
                        c.px(17, by + 5, hex(pal[1]));
                    }
                    mul(pal[0], 0.6)
                }
            };
            c.auto_outline(FW as i32, FH as i32, outline);
        }
    }
    c.save(&out.join(format!("sprites/{name}.png")));
}

pub fn slime(out: &Path) {
    let (fw, fh) = (32u32, 32u32);
    let mut c = Canvas::new(fw * 4, fh * 4);
    let pal = [0x1f7a63, 0x2fa784, 0x4fc3a1, 0x9ff0d8];
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * fw, f as u32 * fh);
            let squish = [0, 2, 0, -2][frame as usize] as f32;
            let (w, h) = (24.0 + squish, 18.0 - squish);
            let cy = 29.0 - h / 2.0;
            c.shade_ellipse(16.0, cy + 2.0, w / 2.0, h / 2.0 + 2.0, pal);
            // Flat bottom: cut the body at row 30 — also what the ellipse drew below the frame
            // (rows 32-33 belong to the next facing row, which is drawn after this one).
            for x in 0..32 {
                for y in 30..36 {
                    c.px(x, y, CLEAR);
                }
            }
            c.rect(9, (cy - h / 2.0 + 5.0) as i32, 3, 2, hex(0xe0fff6)); // shine
            c.px(8, (cy - h / 2.0 + 7.0) as i32, hex(0xe0fff6));
            let eye = |c: &mut Canvas, x: i32| {
                let y = (cy + 1.0) as i32;
                c.rect(x, y, 3, 4, hex(0x10302a));
                c.px(x, y, hex(0xffffff));
            };
            match f {
                Facing::Down => {
                    eye(&mut c, 10);
                    eye(&mut c, 19);
                    c.hline(14, (cy + 6.0) as i32, 4, hex(0x10302a));
                }
                Facing::Left => eye(&mut c, 7),
                Facing::Right => eye(&mut c, 22),
                Facing::Up => {}
            }
            c.auto_outline(fw as i32, fh as i32, 0x0d3a30);
            c.shadow(16.0, 30.5, 13.0, 2.0);
        }
    }
    c.save(&out.join("sprites/slime.png"));
}

// ------------------------------------------------------------------ props

pub mod p {
    pub const CHEST: u32 = 0; // +1 open
    pub const SIGN: u32 = 2;
    pub const GATE: u32 = 3;
    /// The small gate, opened (a free cell; open frame offset = GATE_OPEN - GATE).
    pub const GATE_OPEN: u32 = 15;
    /// The small gate, destroyed (bent bars, rubble).
    pub const GATE_BROKEN: u32 = 23;
    pub const BARREL: u32 = 4;
    // Cells 5 / 6: the small 1x1 shrine of older projects (dormant / glowing), still drawn.
    pub const WELL: u32 = 7;
    // Multi-cell props (sheet is 8 cells wide).
    pub const DOUBLE_GATE: u32 = 8; // 2x2, open state at +2
    pub const BOULDER: u32 = 12; // 2x2
    pub const STATUE: u32 = 14; // 1x2
    pub const BIG_OAK: u32 = 24; // 3x4
    pub const BIG_PINE: u32 = 27; // 2x4
    pub const VGATE: u32 = 29; // 1x4 vertical double gate, open state at +1
    /// The forest shrine (3x2): a monolith with a crystal between two lamp pillars on a
    /// stepped dais; glowing state at +3.
    pub const GRAND_SHRINE: u32 = 56;
    /// An alchemy table (crafting station), on row 9 (added below the older rows so existing
    /// libraries take the larger sheet: their pixels stay the same).
    pub const ALCHEMY: u32 = 72;
    /// Furniture (row 9, the bed's foot on row 10): a chair, a bench (2x1), a bed (1x2) and
    /// the bed's blanket alone (1x2: drawn over a sleeper).
    pub const CHAIR: u32 = 73;
    pub const BENCH: u32 = 74;
    pub const BED: u32 = 76;
    pub const BLANKET: u32 = 77;
    /// A chair facing up (seen from behind) and its backrest alone (drawn over a sitter).
    pub const CHAIR_UP: u32 = 78;
    pub const CHAIR_UP_BACK: u32 = 79;
    /// A sideways bed (2x1, the head at the left) and its quilt alone.
    pub const BED_SIDE: u32 = 80;
    pub const BLANKET_SIDE: u32 = 82;
    /// Chairs facing left / right (side views).
    pub const CHAIR_LEFT: u32 = 86;
    pub const CHAIR_RIGHT: u32 = 87;
    /// Ore veins (row 11): rocks with copper, iron or silver ore, a gem vein, and the
    /// broken rock a vein leaves when it is mined out.
    pub const VEIN_COPPER: u32 = 88;
    pub const VEIN_IRON: u32 = 89;
    pub const VEIN_SILVER: u32 = 90;
    pub const VEIN_GEM: u32 = 91;
    pub const VEIN_EMPTY: u32 = 92;
    /// Coal for the smelter (row 11), and plants picked by hand (row 11-12): a herb bush and a
    /// mushroom patch, each also picked bare.
    pub const VEIN_COAL: u32 = 93;
    pub const HERB_BUSH: u32 = 94;
    pub const HERB_BUSH_PICKED: u32 = 95;
    pub const MUSHROOMS: u32 = 96;
    pub const MUSHROOMS_PICKED: u32 = 97;
    /// A smelter (crafting station: ore and coal into ingots), 1x2.
    pub const SMELTER: u32 = 98;
    /// An anvil on a tree-stump block (crafting station: ingots into weapons, armor, tools).
    pub const ANVIL: u32 = 99;
    /// Farming: a tilled soil plot (row 12) and three crops in three stages each - a sprout,
    /// growing, ripe (wheat on row 12, carrots and pumpkins on row 14).
    pub const SOIL: u32 = 100;
    pub const WHEAT: u32 = 101;
    pub const CARROT: u32 = 112;
    pub const PUMPKIN: u32 = 115;
    /// A cooking fire (row 14): burnt out, and lit.
    pub const CAMPFIRE: u32 = 118;
    pub const CAMPFIRE_LIT: u32 = 119;
}

/// The campfire: a ring of stones around logs, cold and burning.
fn campfire(c: &mut Canvas) {
    let n = TS as i32;
    for (cell_id, lit) in [(p::CAMPFIRE, false), (p::CAMPFIRE_LIT, true)] {
        let (x, y) = cell(cell_id);
        c.at(x, y);
        for k in 0..9 {
            let a = k as f32 / 9.0 * std::f32::consts::TAU;
            c.shade_disc(16.0 + a.cos() * 11.0, 24.0 + a.sin() * 5.0, 2.6, STONE);
        }
        c.stroke(9.0, 26.0, 23.0, 21.0, 1.8, hex(if lit { WOOD[1] } else { 0x2a2420 }));
        c.stroke(9.0, 21.0, 23.0, 26.0, 1.8, hex(if lit { WOOD[2] } else { 0x3a322c }));
        if lit {
            for (k, col) in FIRE.iter().enumerate() {
                let r = 6.5 - k as f32 * 1.2;
                c.ellipse(16.0, 21.0 - k as f32 * 2.2, r * 0.8, r + 1.0, hex(*col));
            }
            c.px(13, 9, hex(FIRE[2]));
            c.px(19, 7, hex(FIRE[3]));
        } else {
            c.rect(14, 22, 4, 2, hex(0x606060));
        }
        c.auto_outline(n, n, OUTLINE);
    }
}

/// The soil plot and the crop stages (drawn over the plot, standing in its furrows).
fn farming(c: &mut Canvas) {
    let n = TS as i32;
    let soil = [0x3a2414, 0x553620, 0x6e4a2c, 0x8a6240];
    let (x, y) = cell(p::SOIL);
    c.at(x, y);
    c.shade_box(2, 14, 28, 16, soil);
    for fy in [17, 22, 27] {
        c.hline(3, fy, 26, hex(soil[0]));
        c.hline(3, fy - 1, 26, hex(soil[3]));
    }
    c.auto_outline(n, n, OUTLINE);
    let green = LEAF;
    for stage in 0..3u32 {
        // Wheat: sprouts, green stalks, golden ears.
        let (x, y) = cell(p::WHEAT + stage);
        c.at(x, y);
        for (k, sx) in [6, 11, 16, 21, 26].into_iter().enumerate() {
            let h = [4, 11, 16][stage as usize] + (k % 2) as i32;
            let col = if stage == 2 { hex(0xc8a040) } else { hex(green[2]) };
            c.vline(sx, 27 - h, h, col);
            if stage == 2 {
                c.shade_box(sx - 1, 27 - h - 4, 3, 5, [0x8a6a20, 0xc8a040, 0xe8c860, 0xfff0a0]);
            } else if stage == 1 {
                c.line(sx, 27 - h / 2, sx + 2, 27 - h / 2 - 3, hex(green[1]));
            }
        }
        // Carrots: leaf tufts that grow, orange shoulders when ripe.
        let (x, y) = cell(p::CARROT + stage);
        c.at(x, y);
        for sx in [8, 16, 24] {
            let r = [2.0, 3.5, 4.5][stage as usize];
            c.shade_disc(sx as f32, 24.0 - r, r, green);
            c.shade_disc(sx as f32 - r * 0.6, 25.0 - r * 0.5, r * 0.6, green);
            if stage == 2 {
                c.shade_box(sx - 2, 24, 5, 3, [0x8a3a0a, 0xd86a1a, 0xf08a30, 0xffb870]);
            }
        }
        // Pumpkins: a sprout, a vine with a small green fruit, a big orange one.
        let (x, y) = cell(p::PUMPKIN + stage);
        c.at(x, y);
        match stage {
            0 => {
                c.vline(16, 21, 6, hex(green[2]));
                c.shade_disc(14.0, 21.0, 2.5, green);
                c.shade_disc(18.5, 20.0, 2.5, green);
            }
            1 => {
                c.stroke(4.0, 25.0, 28.0, 23.0, 1.0, hex(green[1]));
                for lx in [7.0, 15.0, 24.0] {
                    c.shade_disc(lx, 21.0, 3.5, green);
                }
                c.shade_ellipse(17.0, 25.0, 4.0, 3.0, [0x2e5a24, 0x4a8a34, 0x6aaa4a, 0x9ad070]);
            }
            _ => {
                c.stroke(3.0, 26.0, 29.0, 24.0, 1.0, hex(green[1]));
                c.shade_disc(6.0, 21.0, 3.5, green);
                c.shade_disc(27.0, 20.0, 3.5, green);
                c.shade_ellipse(16.0, 22.0, 9.0, 7.0, [0x8a3a06, 0xd8661a, 0xf08a2a, 0xffbc6a]);
                for rx in [12, 16, 20] {
                    c.vline(rx, 16, 12, hex(0xb85010));
                }
                c.shade_box(15, 13, 3, 3, WOOD);
            }
        }
    }
    for k in 0..3 {
        for id in [p::WHEAT, p::CARROT, p::PUMPKIN] {
            let (x, y) = cell(id + k);
            c.at(x, y);
            c.auto_outline(n, n, OUTLINE);
        }
    }
}

/// The coal vein, the herb bush and the mushroom patch (full and picked), the smelter.
fn gathering_props(c: &mut Canvas) {
    let n = TS as i32;
    // Coal: a rock with black glossy lumps.
    let (x, y) = cell(p::VEIN_COAL);
    c.at(x, y);
    c.shade_ellipse(16.0, 19.0, 13.0, 11.0, STONE);
    c.shade_ellipse(9.0, 24.0, 6.0, 5.0, STONE);
    c.shade_ellipse(24.0, 24.0, 6.0, 5.0, STONE);
    for (fx, fy, r) in [(11.0, 15.0, 3.0), (18.0, 19.0, 3.4), (21.0, 13.0, 2.4), (13.0, 23.0, 2.2)] {
        c.shade_disc(fx, fy, r, [0x0c0c10, 0x1c1c24, 0x30303c, 0x6a6a7a]);
    }
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.0, 13.0, 1.5);
    // Herb bush: leafy, with pale sprigs when full; bare twigs when picked.
    for (cell_id, full) in [(p::HERB_BUSH, true), (p::HERB_BUSH_PICKED, false)] {
        let (x, y) = cell(cell_id);
        c.at(x, y);
        if full {
            for (bx, by, r) in [(10.0, 20.0, 7.0), (22.0, 20.0, 7.0), (16.0, 15.0, 8.0), (16.0, 23.0, 7.0)] {
                c.shade_disc(bx, by, r, LEAF);
            }
            for (sx, sy) in [(9, 14), (15, 10), (22, 13), (19, 20), (12, 22)] {
                c.rect(sx, sy, 2, 3, hex(0xb8e08a));
                c.px(sx, sy - 1, hex(0xe8f8c8));
            }
        } else {
            for (x0, y0, x1, y1) in [(16, 29, 16, 18), (16, 22, 10, 16), (16, 22, 22, 15), (16, 25, 21, 21)] {
                c.line(x0, y0, x1, y1, hex(WOOD[1]));
            }
            c.shade_disc(10.0, 16.0, 2.0, LEAF);
            c.shade_disc(22.0, 15.0, 2.0, LEAF);
        }
        c.auto_outline(n, n, OUTLINE);
        c.shadow(16.0, 29.5, 10.0, 1.5);
    }
    // Mushroom patch: three red caps in moss; picked: the moss and stubs.
    for (cell_id, full) in [(p::MUSHROOMS, true), (p::MUSHROOMS_PICKED, false)] {
        let (x, y) = cell(cell_id);
        c.at(x, y);
        c.shade_ellipse(16.0, 26.0, 12.0, 4.0, [0x1d3a18, 0x2e5a24, 0x447a34, 0x6a9a50]);
        for (mx, my, s) in [(10.0, 22.0, 1.0), (19.0, 20.0, 1.3), (24.0, 24.0, 0.8)] {
            c.shade_box((mx - 1.0) as i32, (my + 1.0) as i32, 3, (5.0 * s) as i32, [0xbcaaa4, 0xd7ccc8, 0xefebe9, 0xffffff]);
            if full {
                c.shade_ellipse(mx + 0.5, my, 5.0 * s, 3.5 * s, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
                c.px(mx as i32 - 1, (my - 1.0) as i32, hex(0xffffff));
                c.px(mx as i32 + 2, my as i32, hex(0xffffff));
            }
        }
        c.auto_outline(n, n, OUTLINE);
    }
    // Smelter (1x2): a stone furnace with a glowing mouth and a chimney.
    let (x, y) = cell(p::SMELTER);
    c.at(x, y);
    c.shade_box(12, 2, 8, 22, STONE); // chimney
    c.rect(11, 1, 10, 3, hex(STONE[0]));
    c.shade_box(4, 22, 24, 40, STONE); // body
    c.rrect(4, 20, 24, 6, 2, hex(STONE[2]));
    for (bx, by) in [(6, 30), (18, 30), (10, 40), (22, 44), (6, 52), (16, 54)] {
        c.hline(bx, by, 6, hex(STONE[0]));
    }
    // The fire in its mouth.
    c.rrect(9, 40, 14, 14, 4, hex(0x1a0c06));
    for (k, col) in FIRE.iter().enumerate().skip(1) {
        let r = 6.0 - k as f32 * 1.1;
        c.ellipse(16.0, 50.0 - k as f32, r + 1.0, r, hex(*col));
    }
    c.auto_outline(n, 2 * n, OUTLINE);
    c.shadow(16.0, 62.5, 13.0, 1.5);
    // Anvil: an iron horn-and-face block on a wooden stump.
    let (x, y) = cell(p::ANVIL);
    c.at(x, y);
    c.shade_box(9, 20, 14, 10, WOOD); // stump
    c.hline(9, 20, 14, hex(WOOD[3]));
    c.shade_box(12, 15, 8, 5, IRON); // waist
    c.shade_box(5, 9, 22, 6, [0x24242c, 0x3c3c48, 0x5c5c6a, 0x8a8a98]); // face
    c.hline(5, 9, 22, hex(0xb0b0bc));
    c.stroke(5.0, 11.0, 1.5, 12.5, 2.2, hex(0x3c3c48)); // horn
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.0, 11.0, 1.5);
}

/// Ore veins: a rock with ore flecks in the vein's color, and the mined-out rubble.
fn veins(c: &mut Canvas) {
    let n = TS as i32;
    let ores: [(u32, [u32; 4]); 4] = [
        (p::VEIN_COPPER, [0x7a3e1a, 0xb8642c, 0xe0904a, 0xffc890]),
        (p::VEIN_IRON, [0x4a3a36, 0x7a6058, 0xa88a7e, 0xd8c4b8]),
        (p::VEIN_SILVER, [0x6c7480, 0xa8b2be, 0xd8e0ea, 0xffffff]),
        (p::VEIN_GEM, [0x6a0e2a, 0xb01e48, 0xe8406e, 0xffa0c0]),
    ];
    for (cell_id, ore) in ores {
        let (x, y) = cell(cell_id);
        c.at(x, y);
        c.shade_ellipse(16.0, 19.0, 13.0, 11.0, STONE);
        c.shade_ellipse(9.0, 24.0, 6.0, 5.0, STONE);
        c.shade_ellipse(24.0, 24.0, 6.0, 5.0, STONE);
        // Ore: flecks and a few bright nuggets along a crack.
        c.line(7, 15, 24, 25, hex(STONE[0]));
        for (fx, fy, r) in [(10.0, 15.0, 2.2), (15.0, 19.0, 2.6), (20.0, 22.0, 2.0), (21.0, 13.0, 1.8), (12.0, 23.0, 1.6)] {
            if cell_id == p::VEIN_GEM {
                // Faceted crystals.
                c.shade_box(fx as i32 - 1, fy as i32 - 2, 3, 4, ore);
                c.px(fx as i32, fy as i32 - 2, hex(ore[3]));
            } else {
                c.shade_disc(fx, fy, r, ore);
            }
        }
        c.auto_outline(n, n, OUTLINE);
        c.shadow(16.0, 30.0, 13.0, 1.5);
    }
    // Mined out: low broken rubble.
    let (x, y) = cell(p::VEIN_EMPTY);
    c.at(x, y);
    for (rx, ry, r) in [(10.0, 25.0, 5.0), (19.0, 26.0, 6.0), (25.0, 24.0, 3.5), (14.0, 21.0, 3.0)] {
        c.shade_disc(rx, ry, r, STONE);
    }
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.5, 12.0, 1.5);
}

/// Chair, bench, bed and the bed's blanket (furniture the player can use).
fn furniture(c: &mut Canvas) {
    let n = TS as i32;
    // Chair (seen from the front): back with two slats, seat, legs.
    let (x, y) = cell(p::CHAIR);
    c.at(x, y);
    // Wide enough that a sitter (drawn over it) leaves the posts and legs in sight.
    for px in [3, 26] {
        c.shade_box(px, 1, 3, 29, WOOD); // back posts, down to the floor
    }
    for sy in [3, 8] {
        c.shade_box(6, sy, 20, 3, WOOD); // slats
    }
    c.shade_box(2, 17, 28, 5, WOOD); // seat
    c.hline(2, 17, 28, hex(WOOD[3]));
    for lx in [3, 26] {
        c.shade_box(lx, 22, 3, 8, WOOD);
    }
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.5, 10.0, 1.5);
    // Bench (2x1): a plank seat with a backrest on stone feet.
    let (x, y) = cell(p::BENCH);
    c.at(x, y);
    for sy in [6, 11] {
        c.shade_box(4, sy, 56, 3, WOOD);
    }
    for px in [6, 55] {
        c.shade_box(px, 4, 3, 14, WOOD);
    }
    c.shade_box(2, 16, 60, 5, WOOD);
    c.hline(2, 16, 60, hex(WOOD[3]));
    for lx in [5, 53] {
        c.shade_box(lx, 21, 6, 9, STONE);
    }
    c.auto_outline(2 * n, n, OUTLINE);
    c.shadow(32.0, 30.5, 26.0, 1.5);
    // Bed (1x2, 32 x 64): headboard, pillow, sheet and a quilt; the quilt alone is the cover.
    let quilt = |c: &mut Canvas| {
        c.shade_box(4, 24, 24, 34, [0x7a1f1f, 0xa83232, 0xc94a4a, 0xe07a6a]);
        c.hline(4, 24, 24, hex(0xf2e8d8)); // the folded sheet's edge
        c.hline(4, 25, 24, hex(0xf2e8d8));
        for k in 0..3 {
            c.hline(5, 34 + k * 9, 22, hex(0x8a2626)); // quilt seams
        }
    };
    let (x, y) = cell(p::BED);
    c.at(x, y);
    c.shade_box(2, 2, 28, 10, WOOD); // headboard
    c.hline(2, 2, 28, hex(WOOD[3]));
    c.shade_box(3, 10, 26, 50, [0xc8c0b0, 0xe0d8c8, 0xf2ecdf, 0xffffff]); // mattress + sheet
    c.shade_box(7, 12, 18, 9, [0xc8c8d0, 0xe0e0e8, 0xf4f4f8, 0xffffff]); // pillow
    quilt(c);
    c.shade_box(2, 57, 28, 6, WOOD); // footboard
    c.auto_outline(n, 2 * n, OUTLINE);
    // The cover: the same quilt, outlined on its own, drawn over a sleeper.
    let (x, y) = cell(p::BLANKET);
    c.at(x, y);
    quilt(c);
    c.shade_box(2, 57, 28, 6, WOOD); // the footboard stays in front of the feet
    c.auto_outline(n, 2 * n, OUTLINE);

    // A chair facing up, seen from behind: the seat beyond, the backrest on the near side
    // (the backrest alone is the cover: it hides the sitter's lower back).
    let backrest = |c: &mut Canvas| {
        for px in [3, 26] {
            c.shade_box(px, 11, 3, 19, WOOD); // posts down to the floor
        }
        for sy in [12, 17] {
            c.shade_box(6, sy, 20, 3, WOOD); // slats
        }
    };
    let (x, y) = cell(p::CHAIR_UP);
    c.at(x, y);
    c.shade_box(4, 6, 24, 7, WOOD); // seat
    c.hline(4, 6, 24, hex(WOOD[3]));
    for lx in [5, 24] {
        c.shade_box(lx, 3, 3, 4, WOOD); // the far legs' tops behind the seat
    }
    backrest(c);
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.5, 10.0, 1.5);
    let (x, y) = cell(p::CHAIR_UP_BACK);
    c.at(x, y);
    backrest(c);
    c.auto_outline(n, n, OUTLINE);

    // Chairs seen from the side: the backrest on the side behind the sitter.
    for (cellno, back_left) in [(p::CHAIR_LEFT, false), (p::CHAIR_RIGHT, true)] {
        let (x, y) = cell(cellno);
        c.at(x, y);
        let bx = if back_left { 6 } else { 23 };
        c.shade_box(bx, 3, 3, 27, WOOD); // backrest post, down to the floor
        c.shade_box(if back_left { 6 } else { 9 }, 6, 17, 3, WOOD); // top rail
        c.shade_box(5, 17, 22, 4, WOOD); // seat
        c.hline(5, 17, 22, hex(WOOD[3]));
        let fx = if back_left { 23 } else { 6 };
        c.shade_box(fx, 21, 3, 9, WOOD); // front leg
        c.auto_outline(n, n, OUTLINE);
        c.shadow(16.0, 30.5, 10.0, 1.5);
    }

    // A sideways bed (64 x 32, the head at the left) and its quilt alone.
    let quilt_side = |c: &mut Canvas| {
        c.shade_box(22, 4, 36, 25, [0x7a1f1f, 0xa83232, 0xc94a4a, 0xe07a6a]);
        c.vline(22, 4, 25, hex(0xf2e8d8)); // the folded sheet's edge
        c.vline(23, 4, 25, hex(0xf2e8d8));
        for k in 0..3 {
            c.vline(32 + k * 9, 5, 23, hex(0x8a2626));
        }
        c.shade_box(58, 2, 5, 28, WOOD); // the footboard
    };
    let (x, y) = cell(p::BED_SIDE);
    c.at(x, y);
    c.shade_box(1, 0, 6, 31, WOOD); // headboard
    c.vline(1, 0, 31, hex(WOOD[3]));
    c.shade_box(6, 3, 54, 27, [0xc8c0b0, 0xe0d8c8, 0xf2ecdf, 0xffffff]); // mattress + sheet
    c.shade_box(8, 7, 11, 18, [0xc8c8d0, 0xe0e0e8, 0xf4f4f8, 0xffffff]); // pillow
    quilt_side(c);
    c.auto_outline(2 * n, n, OUTLINE);
    let (x, y) = cell(p::BLANKET_SIDE);
    c.at(x, y);
    quilt_side(c);
    c.auto_outline(2 * n, n, OUTLINE);
}

/// The forest shrine, 96 x 64: a stepped dais, two lamp pillars and a rune-carved monolith with
/// a crystal on top. Glowing: blue flames in the lamps, lit runes, a bright crystal and a halo.
fn grand_shrine(c: &mut Canvas, glow: bool) {
    let dark = [0x2c2c38, 0x45455a, 0x5c5c74, 0x80809a];
    let rune = if glow { [0x2a8fb0, 0x55d4ff, 0x9ff0ff, 0xe8ffff] } else { [0x1e1e28, 0x2c2c38, 0x38384a, 0x4a4a5e] };
    if glow {
        // A soft halo behind everything.
        c.ellipse(48.0, 32.0, 46.0, 30.0, Color::new(0.45, 0.85, 1.0, 0.10));
        c.ellipse(48.0, 24.0, 30.0, 22.0, Color::new(0.55, 0.9, 1.0, 0.12));
    }
    // Dais: two broad steps.
    c.shade_box(4, 52, 88, 10, STONE);
    c.hline(4, 52, 88, hex(STONE[3]));
    c.shade_box(12, 44, 72, 9, STONE);
    c.hline(12, 44, 72, hex(STONE[3]));
    for x in (10..90).step_by(12) {
        c.vline(x, 54, 7, hex(STONE[1])); // joints
    }
    // Lamp pillars with bowls on top.
    // (Everything stays inside the 96 x 64 block: the sheet cells above belong to the big oak.)
    for px in [14, 72] {
        c.shade_box(px, 23, 10, 22, STONE);
        c.shade_box(px - 2, 19, 14, 5, STONE); // capital
        c.shade_box(px - 1, 41, 12, 4, STONE); // foot
        c.shade_box(px - 3, 13, 16, 6, IRON); // bowl
        c.hline(px - 3, 13, 16, hex(IRON[3]));
        if glow {
            // Blue flames.
            let cx = px as f32 + 5.0;
            c.ellipse(cx, 8.0, 6.0, 6.0, hex(0x2a8fb0));
            c.ellipse(cx, 8.5, 4.0, 4.5, hex(0x55d4ff));
            c.ellipse(cx, 9.5, 2.0, 2.5, hex(0xe8ffff));
            c.px(px + 5, 2, hex(0x9ff0ff));
        } else {
            c.hline(px - 1, 14, 12, hex(0x14141c)); // cold ashes
        }
        // A rune on each pillar.
        c.rect(px + 4, 28, 2, 8, hex(rune[2]));
        c.hline(px + 2, 31, 6, hex(rune[1]));
    }
    // The monolith: tapering towards a pointed top, carved runes down its face.
    for (k, yy) in (8..45).enumerate() {
        let half = 8 + (k as i32) / 9;
        let x0 = 48 - half;
        c.hline(x0, yy, 2 * half, hex(dark[2]));
        c.px(x0, yy, hex(dark[3]));
        c.px(x0 + 1, yy, hex(dark[3]));
        c.px(x0 + 2 * half - 1, yy, hex(dark[0]));
        c.px(x0 + 2 * half - 2, yy, hex(dark[1]));
    }
    c.rect(41, 6, 14, 3, hex(dark[2]));
    // Runes: a channel with glyphs branching off.
    c.rect(47, 12, 2, 30, hex(rune[1]));
    c.vline(47, 13, 28, hex(rune[2]));
    for (gy, gx, gw) in [(15, 42, 5), (21, 49, 6), (27, 41, 6), (33, 49, 5), (38, 43, 4)] {
        c.hline(gx, gy, gw, hex(rune[1]));
        c.px(gx + if gx < 47 { 0 } else { gw - 1 }, gy - 1, hex(rune[2]));
    }
    if glow {
        c.vline(48, 14, 26, hex(rune[3]));
    }
    // The crystal on top: a diamond.
    let gem = if glow { [0x2a8fb0, 0x55d4ff, 0x9ff0ff, 0xffffff] } else { [0x3a3048, 0x504464, 0x66597e, 0x8478a0] };
    for k in 0..7i32 {
        let w = 2 * (k + 1);
        c.hline(48 - w / 2, k, w, hex(gem[2]));
        c.hline(48 - w / 2, 12 - k, w, hex(gem[1]));
    }
    c.vline(46, 2, 5, hex(gem[3]));
    c.px(45, 4, hex(gem[3]));
    c.auto_outline(96, 64, OUTLINE);
    if glow {
        // Sparkles around it (after the outline: they float free).
        for (gx, gy) in [(30, 6), (66, 10), (36, 26), (62, 30), (8, 30), (88, 24), (54, 3), (24, 40), (74, 40)] {
            c.px(gx, gy, hex(0xe8ffff));
            c.px(gx - 1, gy, Color::new(0.6, 0.95, 1.0, 0.6));
            c.px(gx + 1, gy, Color::new(0.6, 0.95, 1.0, 0.6));
            c.px(gx, gy - 1, Color::new(0.6, 0.95, 1.0, 0.6));
            c.px(gx, gy + 1, Color::new(0.6, 0.95, 1.0, 0.6));
        }
    }
    c.shadow(48.0, 61.0, 44.0, 2.5);
}

/// Origin of sheet cell `cell` (8 cells per row).
fn cell(cell: u32) -> (u32, u32) {
    (cell % 8 * TS, cell / 8 * TS)
}

/// Iron bars with spear tips between `x0` and `x1`, from `top` to `bottom`.
fn bars(c: &mut Canvas, x0: i32, x1: i32, top: i32, bottom: i32, step: usize) {
    for x in (x0..x1).step_by(step) {
        c.shade_box(x, top, 3, bottom - top, IRON);
        c.px(x + 1, top - 1, hex(IRON[3]));
        c.px(x + 1, top - 2, hex(IRON[2]));
    }
}

fn big_props(c: &mut Canvas) {
    // Double gate, closed (2x2) and open (+2).
    for (k, open) in [(0u32, false), (2, true)] {
        let (x, y) = cell(p::DOUBLE_GATE + k);
        c.at(x, y);
        for px in [0, 53] {
            c.shade_box(px, 6, 11, 58, STONE);
            // The cap overhangs the post by a pixel on the inner side only: on the outer side
            // it would spill into the neighbouring cells of the sheet (the boulder's).
            c.shade_box(if px == 0 { 0 } else { px - 1 }, 2, 12, 6, STONE);
            for yy in (14..60).step_by(10) {
                c.hline(px + 1, yy, 9, hex(STONE[1]));
            }
        }
        if open {
            // Leaves swung inward, seen edge-on next to the posts.
            for px in [11, 47] {
                c.shade_box(px, 16, 6, 46, IRON);
            }
        } else {
            bars(c, 12, 52, 12, 62, 5);
            for yy in [18, 44] {
                c.shade_box(11, yy, 42, 4, IRON);
            }
            c.shade_box(29, 27, 6, 8, GOLD); // latch
            c.px(31, 30, hex(GOLD[0]));
        }
        c.auto_outline(64, 64, OUTLINE);
        c.shadow(32.0, 62.0, 30.0, 3.0);
    }
    // Boulder (2x2).
    let (x, y) = cell(p::BOULDER);
    c.at(x, y);
    c.shade_ellipse(32.0, 42.0, 27.0, 20.0, STONE);
    c.line(20, 34, 26, 44, hex(STONE[1]));
    c.line(26, 44, 24, 52, hex(STONE[1]));
    c.line(40, 30, 44, 38, hex(STONE[1]));
    for (mx, my) in [(30, 23), (34, 24), (38, 26), (27, 25)] {
        c.rect(mx, my, 4, 2, hex(0x5f8f4a));
        c.px(mx + 1, my, hex(0x86b86a));
    }
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 61.0, 28.0, 4.0);
    // The forest shrine (3x2), dormant and glowing.
    for (k, glow) in [(0u32, false), (3, true)] {
        let (x, y) = cell(p::GRAND_SHRINE + k);
        c.at(x, y);
        grand_shrine(c, glow);
    }
    // Statue (1x2): a knight on a pedestal.
    let (x, y) = cell(p::STATUE);
    c.at(x, y);
    c.shade_box(3, 48, 26, 14, STONE);
    c.shade_box(5, 44, 22, 5, STONE);
    c.shade_box(10, 20, 12, 24, STONE); // body
    c.shade_disc(16.0, 14.0, 7.0, STONE); // head
    c.shade_box(6, 22, 4, 14, STONE); // arms
    c.shade_box(22, 22, 4, 14, STONE);
    c.shade_box(24, 6, 3, 30, [STONE[1], STONE[2], STONE[3], 0xe0e0ea]); // sword
    c.rect(21, 30, 9, 2, hex(STONE[1]));
    c.auto_outline(32, 64, OUTLINE);
    c.shadow(16.0, 62.0, 15.0, 2.5);
    // Big oak (3x4).
    let (x, y) = cell(p::BIG_OAK);
    c.at(x, y);
    c.shade_cyl(40, 66, 16, 58, WOOD);
    for (rx, w) in [(34, 6), (54, 6)] {
        c.shade_box(rx, 118, w, 6, WOOD);
    }
    for (bx, by, ex, ey) in [(44, 80, 26, 62), (52, 78, 70, 60)] {
        c.stroke(bx as f32, by as f32, ex as f32, ey as f32, 2.5, hex(WOOD[1]));
    }
    for (cx, cy, r) in [
        (48.0, 44.0, 34.0),
        (22.0, 56.0, 20.0),
        (74.0, 56.0, 20.0),
        (36.0, 30.0, 22.0),
        (62.0, 32.0, 20.0),
        (48.0, 20.0, 18.0),
        (30.0, 62.0, 13.0),
        (64.0, 64.0, 13.0),
    ] {
        c.shade_disc(cx, cy, r, LEAF);
    }
    for (ax, ay) in [(30, 40), (60, 50), (45, 26), (70, 38), (24, 58)] {
        c.rect(ax, ay, 3, 3, hex(0xd84343)); // apples
        c.px(ax, ay, hex(0xff9a9a));
    }
    c.auto_outline(96, 128, 0x10301a);
    c.shadow(48.0, 123.0, 30.0, 5.0);
    // Big pine (2x4).
    let (x, y) = cell(p::BIG_PINE);
    c.at(x, y);
    c.shade_cyl(28, 96, 9, 28, WOOD);
    for (k, ty) in [(9.0, 4.0), (14.0, 18.0), (19.0, 34.0), (24.0, 50.0), (28.0, 66.0), (30.0, 80.0)] {
        pine_tier(c, 32.0, ty, k, 20.0);
    }
    c.auto_outline(64, 128, 0x082a18);
    c.shadow(32.0, 123.0, 22.0, 4.0);
    // Vertical double gate (1x4, open at +1): seen from the side, a post at the top and the
    // bottom, the two leaves edge-on between them.
    for (k, open) in [(0u32, false), (1, true)] {
        let (x, y) = cell(p::VGATE + k);
        c.at(x, y);
        for py in [6, 102] {
            c.shade_box(7, py, 18, 24, STONE);
            c.shade_box(6, py - 3, 20, 5, STONE);
            c.hline(8, py + 12, 16, hex(STONE[1]));
        }
        if open {
            // Leaves folded back against the posts.
            c.shade_box(12, 30, 8, 12, IRON);
            c.shade_box(12, 90, 8, 12, IRON);
        } else {
            c.shade_box(10, 30, 12, 72, IRON);
            for yy in (34..100).step_by(6) {
                c.hline(12, yy, 8, hex(IRON[3]));
            }
            c.shade_box(8, 44, 16, 4, IRON);
            c.shade_box(8, 86, 16, 4, IRON);
            c.shade_box(18, 63, 6, 6, GOLD); // latch
        }
        c.auto_outline(32, 128, OUTLINE);
        for yy in 30..124 {
            c.under(4, yy, Color::new(0.0, 0.0, 0.0, 0.18));
            c.under(27, yy, Color::new(0.0, 0.0, 0.0, 0.12));
        }
    }
}

pub fn props(out: &Path) {
    // 11 rows: rows 7-8 (the grand shrine), 9 (the alchemy table, furniture) and 10 (the bed's
    // foot) were added later, below the older cells.
    let mut c = Canvas::new(TS * 8, TS * 15);
    let n = TS as i32;
    for i in 0..8u32 {
        let (x, y) = cell(i);
        c.at(x, y);
        match i {
            0 | 1 => {
                c.shade_box(4, 14, 24, 16, WOOD);
                for bx in [7, 22] {
                    c.shade_box(bx, 14, 3, 16, IRON);
                }
                if i == 0 {
                    c.rrect(4, 6, 24, 10, 3, hex(WOOD[2]));
                    c.hline(6, 7, 20, hex(WOOD[3]));
                    c.hline(4, 15, 24, hex(WOOD[0]));
                    for bx in [7, 22] {
                        c.rect(bx, 6, 3, 10, hex(IRON[2]));
                    }
                    c.shade_box(13, 12, 6, 7, GOLD);
                    c.rect(15, 15, 2, 3, hex(GOLD[0]));
                } else {
                    c.rrect(4, 1, 24, 9, 3, hex(WOOD[1])); // lid, swung back
                    c.hline(6, 2, 20, hex(WOOD[2]));
                    c.rect(5, 10, 22, 5, hex(0x1a1208));
                    for (gx, gy) in [(9, 12), (14, 11), (19, 12), (23, 11)] {
                        c.rect(gx, gy, 3, 2, hex(GOLD[2]));
                        c.px(gx, gy, hex(GOLD[3]));
                    }
                }
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.0, 14.0, 2.5);
            }
            2 => {
                c.shade_box(14, 18, 4, 13, WOOD);
                c.shade_box(3, 4, 26, 16, WOOD);
                c.hline(4, 11, 24, hex(WOOD[1]));
                for (ly, lw) in [(7, 18), (14, 14)] {
                    c.hline(7, ly, lw, hex(WOOD[0]));
                }
                for nx in [5, 26] {
                    c.px(nx, 6, hex(IRON[3]));
                    c.px(nx, 17, hex(IRON[3]));
                }
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.5, 8.0, 2.0);
            }
            3 => {
                for px in [0, 26] {
                    c.shade_box(px, 2, 6, 29, STONE);
                }
                bars(&mut c, 7, 26, 5, 31, 4);
                c.shade_box(6, 10, 20, 3, IRON);
                c.shade_box(6, 22, 20, 3, IRON);
                c.shade_box(14, 15, 4, 6, GOLD);
                c.auto_outline(n, n, OUTLINE);
            }
            4 => {
                c.shade_cyl(7, 7, 18, 23, WOOD);
                for hy in [10, 24] {
                    c.shade_box(7, hy, 18, 3, IRON);
                }
                c.ellipse(16.0, 7.0, 9.0, 3.5, hex(WOOD[2]));
                c.ellipse(16.0, 7.0, 7.0, 2.0, hex(WOOD[1]));
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.0, 11.0, 2.5);
            }
            5 | 6 => {
                c.shade_box(5, 26, 22, 5, STONE);
                c.shade_box(9, 7, 14, 20, STONE);
                c.shade_box(7, 4, 18, 4, STONE);
                let glow = i == 6;
                c.rect(14, 11, 4, 11, hex(if glow { 0x7df9ff } else { STONE[0] }));
                if glow {
                    c.vline(15, 12, 9, hex(0xe0ffff));
                }
                c.auto_outline(n, n, OUTLINE);
                if glow {
                    for (gx, gy) in [(4, 8), (27, 13), (11, 1), (24, 3), (3, 20)] {
                        c.px(gx, gy, hex(0x7df9ff));
                        c.px(gx + 1, gy, Color::new(0.5, 0.97, 1.0, 0.5));
                    }
                    c.ellipse(16.0, 16.0, 15.0, 15.0, Color::new(0.5, 0.97, 1.0, 0.08));
                }
                c.shadow(16.0, 30.5, 12.0, 2.0);
            }
            7 => {
                c.shade_ellipse(16.0, 22.0, 13.0, 8.0, STONE);
                c.ellipse(16.0, 20.0, 9.0, 4.5, hex(0x1d3557));
                c.hline(12, 19, 5, hex(0x4a7ab0));
                for px in [4, 25] {
                    c.shade_box(px, 4, 3, 19, WOOD);
                }
                for (k, w) in [(0, 30), (1, 26), (2, 22)] {
                    c.rect(16 - w / 2, k * 2, w, 2, hex(if k == 0 { 0xc65a50 } else { 0xa8433a }));
                }
                c.vline(16, 6, 9, hex(0xd8c8a0)); // rope
                c.shade_box(14, 14, 5, 4, WOOD); // bucket
                c.auto_outline(n, n, OUTLINE);
                c.shadow(16.0, 30.0, 14.0, 2.5);
            }
            _ => {}
        }
    }
    // The small iron gate, opened: both leaves swung back against the posts.
    let (x, y) = cell(p::GATE_OPEN);
    c.at(x, y);
    for px in [0, 26] {
        c.shade_box(px, 2, 6, 29, STONE);
    }
    for lx in [6, 22] {
        c.shade_box(lx, 6, 4, 25, IRON);
        c.px(lx + 1, 5, hex(IRON[3]));
        c.px(lx + 2, 4, hex(IRON[2]));
    }
    c.auto_outline(n, n, OUTLINE);
    for yy in 26..31 {
        c.under(10, yy, Color::new(0.0, 0.0, 0.0, 0.15));
        c.under(21, yy, Color::new(0.0, 0.0, 0.0, 0.15));
    }
    // The small iron gate, destroyed: a cracked post, bent and broken bars, rubble.
    let (x, y) = cell(p::GATE_BROKEN);
    c.at(x, y);
    c.shade_box(0, 2, 6, 29, STONE);
    c.shade_box(26, 12, 6, 19, STONE); // the right post broke off
    c.line(27, 14, 30, 18, hex(STONE[0]));
    for (x0, y0, x1, y1) in [(8, 31, 9, 8), (12, 31, 14, 14), (17, 31, 21, 20), (22, 31, 25, 25)] {
        c.stroke(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.0, hex(IRON[1]));
        c.px(x1, y1, hex(IRON[3]));
    }
    c.stroke(6.0, 22.0, 15.0, 26.0, 1.0, hex(IRON[1])); // fallen crossbar
    for (rx, ry, r) in [(19.0f32, 29.0f32, 2.5f32), (25.0, 29.5, 2.0), (13.0, 30.0, 1.5), (29.0, 30.0, 1.5)] {
        c.shade_disc(rx, ry, r, STONE);
    }
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.5, 14.0, 1.5);
    // The alchemy table: a small workbench with a flask over a burner, bottles and a mortar.
    let (x, y) = cell(p::ALCHEMY);
    c.at(x, y);
    c.shade_box(3, 16, 26, 5, WOOD); // table top
    c.hline(3, 16, 26, hex(WOOD[3]));
    for lx in [4, 25] {
        c.shade_box(lx, 21, 3, 10, WOOD); // legs
    }
    c.hline(6, 26, 20, hex(WOOD[0])); // shelf
    c.shade_box(8, 24, 4, 3, [0x5d4037, 0x795548, 0x8d6e63, 0xa1887f]); // a jar on the shelf
    c.shade_box(19, 24, 3, 3, [0x33691e, 0x558b2f, 0x7cb342, 0xaed581]);
    // A round flask with green brew over a small burner.
    c.shade_box(9, 13, 6, 3, IRON);
    c.px(12, 12, hex(0xffb300));
    c.px(11, 12, hex(0xff6f00));
    c.shade_disc(12.0, 8.0, 4.5, [0x7fa8c8, 0xa8cbe8, 0xcfe8ff, 0xffffff]);
    c.shade_disc(12.0, 9.0, 3.2, [0x1b5e20, 0x2e7d32, 0x43a047, 0xa5d6a7]);
    c.rect(11, 1, 2, 4, hex(0xcfe8ff));
    c.px(14, 6, hex(0xffffff));
    // Two bottles and a mortar.
    c.shade_box(18, 8, 3, 8, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
    c.rect(18, 6, 3, 2, hex(WOOD[2]));
    c.shade_box(22, 10, 3, 6, [0x1a237e, 0x283593, 0x3f51b5, 0x7986cb]);
    c.rect(22, 8, 3, 2, hex(WOOD[2]));
    c.shade_box(25, 12, 3, 4, STONE);
    c.auto_outline(n, n, OUTLINE);
    c.shadow(16.0, 30.5, 13.0, 1.5);
    furniture(&mut c);
    veins(&mut c);
    gathering_props(&mut c);
    farming(&mut c);
    campfire(&mut c);
    big_props(&mut c);
    c.save(&out.join("sprites/props.png"));
}

// ------------------------------------------------------------------ items

pub mod it {
    pub const KEY: u32 = 0;
    pub const POTION: u32 = 1;
    pub const COIN: u32 = 2;
    pub const APPLE: u32 = 3;
    pub const LETTER: u32 = 4;
    pub const BOW: u32 = 5;
    pub const CROSSBOW: u32 = 6;
    pub const ARROWS: u32 = 7;
    pub const BOLTS: u32 = 8;
    /// Projectiles (drawn pointing right).
    pub const ARROW_SHOT: u32 = 9;
    pub const BOLT_SHOT: u32 = 10;
    pub const SWORD: u32 = 11;
    pub const SPEAR: u32 = 12;
    pub const AXE: u32 = 13;
    pub const CAP: u32 = 14;
    pub const HELMET: u32 = 15;
    pub const LEATHER: u32 = 16;
    pub const CHAIN: u32 = 17;
    pub const BRONZE_KEY: u32 = 18;
    /// Left-hand items: lights and a shield.
    pub const LANTERN: u32 = 19;
    pub const TORCH: u32 = 20;
    pub const SHIELD: u32 = 21;
    /// Magic: scroll, spell book, mana potion, spell icons, the spark projectile, a star
    /// (arcana skill icon) and a lockpick (lockpicking skill icon).
    pub const SCROLL: u32 = 22;
    pub const BOOK: u32 = 23;
    pub const MANA_POTION: u32 = 24;
    pub const SPELL_HEAL: u32 = 25;
    pub const SPELL_SPARK: u32 = 26;
    pub const SPELL_LIGHT: u32 = 27;
    pub const SPELL_BLAST: u32 = 28;
    pub const SPARK_SHOT: u32 = 29;
    pub const STAR: u32 = 30;
    pub const LOCKPICK: u32 = 31;
    /// Status effect potions: antidote (green), elixir of strength (orange), swiftness (cyan).
    pub const ANTIDOTE: u32 = 32;
    pub const ELIXIR: u32 = 33;
    pub const SWIFTNESS: u32 = 34;
    /// Crafting ingredients: a healing herb and a red-capped mushroom.
    pub const HERB: u32 = 35;
    pub const MUSHROOM: u32 = 36;
    /// Fishing: the rod and two catches.
    pub const ROD: u32 = 37;
    pub const TROUT: u32 = 38;
    pub const BOOT: u32 = 39;
    /// A rolled linen bandage (heals the player or a companion).
    pub const BANDAGE: u32 = 40;
    /// Mining: the pickaxe (a tool) and what veins give.
    pub const PICKAXE: u32 = 41;
    pub const COPPER_ORE: u32 = 42;
    pub const IRON_ORE: u32 = 43;
    pub const SILVER_ORE: u32 = 44;
    pub const GEM: u32 = 45;
    /// Crafting tools and smelting: mortar and pestle (alchemy), smith's tongs (smelter), a
    /// lump of coal and three ingots.
    pub const MORTAR: u32 = 46;
    pub const TONGS: u32 = 47;
    pub const COAL: u32 = 48;
    pub const COPPER_INGOT: u32 = 49;
    pub const IRON_INGOT: u32 = 50;
    pub const SILVER_INGOT: u32 = 51;
    /// Blacksmithing: the smith's hammer, an iron sword and a fine one (a better result of the
    /// same recipe at a higher Smithing level).
    pub const HAMMER: u32 = 52;
    pub const IRON_SWORD: u32 = 53;
    pub const FINE_SWORD: u32 = 54;
    /// Farming: seed pouches and what they grow.
    pub const WHEAT_SEEDS: u32 = 55;
    pub const CARROT_SEEDS: u32 = 56;
    pub const PUMPKIN_SEEDS: u32 = 57;
    pub const WHEAT: u32 = 58;
    pub const CARROT: u32 = 59;
    pub const PUMPKIN: u32 = 60;
    /// Number of icons in the strip (it can't grow much wider: textures are at most 2048 px,
    /// new icons go on their own sheets such as `food`).
    pub const COUNT: u32 = 61;
}

/// An arrow pointing right along row `y`, from `x0`, `len` pixels long.
fn arrow(c: &mut Canvas, x0: i32, y: i32, len: i32, feathers: bool) {
    c.hline(x0, y, len, hex(WOOD[2]));
    c.hline(x0, y + 1, len, hex(WOOD[1]));
    let tip = x0 + len;
    for k in 0..4 {
        c.vline(tip + k, y - 2 + k / 2, 6 - k, hex(if k == 0 { 0x8a96a0 } else { 0xcfd8dc }));
    }
    if feathers {
        for k in 0..4 {
            c.px(x0 + k, y - 1 - (k % 2), hex(0xe53935));
            c.px(x0 + k, y + 2 + (k % 2), hex(0xb71c1c));
        }
    }
}

pub fn items(out: &Path) {
    let mut c = Canvas::new(TS * it::COUNT, TS);
    let n = TS as i32;
    let steel = [0x5f6b73, 0x90a4ae, 0xc5d0d6, 0xf0f4f6];
    let string = hex(0xe8e0c8);
    for i in 0..it::COUNT {
        c.at(i * TS, 0);
        match i {
            it::KEY | it::BRONZE_KEY => {
                let pal = if i == it::KEY { GOLD } else { ramp(0xb5652b) };
                c.shade_disc(10.0, 12.0, 7.0, pal);
                c.disc(10.0, 12.0, 3.0, CLEAR);
                c.shade_box(16, 10, 13, 4, pal);
                c.shade_box(22, 14, 3, 5, pal);
                c.shade_box(26, 14, 3, 7, pal);
            }
            it::POTION => {
                c.shade_box(13, 3, 6, 4, WOOD); // cork
                c.rect(14, 7, 4, 5, hex(0xcfe8ff));
                c.shade_disc(16.0, 20.0, 10.0, [0x7fa8c8, 0xa8cbe8, 0xcfe8ff, 0xffffff]);
                c.shade_disc(16.0, 21.0, 8.0, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
                c.hline(9, 17, 14, hex(0xff8a80));
                c.rect(11, 14, 2, 4, hex(0xffffff));
            }
            it::COIN => {
                c.shade_disc(16.0, 16.0, 11.0, GOLD);
                c.disc(16.0, 16.0, 7.5, hex(GOLD[1]));
                c.disc(15.5, 15.5, 6.5, hex(GOLD[2]));
                c.rect(14, 10, 3, 12, hex(GOLD[1]));
                c.px(10, 9, hex(0xffffff));
            }
            it::APPLE => {
                c.shade_disc(16.0, 19.0, 10.0, [0x7a1010, 0xb71c1c, 0xd32f2f, 0xff6e60]);
                c.rect(11, 13, 2, 3, hex(0xffcdd2));
                c.shade_box(15, 4, 3, 7, WOOD);
                c.disc(21.0, 7.0, 3.5, hex(LEAF[2]));
                c.px(20, 6, hex(LEAF[3]));
            }
            it::LETTER => {
                c.shade_box(4, 8, 24, 17, [0x9e9580, 0xd8d0bc, 0xf5f0e1, 0xffffff]);
                for k in 0..10 {
                    c.px(5 + k, 9 + k * 6 / 10, hex(0x9e9580));
                    c.px(26 - k, 9 + k * 6 / 10, hex(0x9e9580));
                }
                c.shade_disc(16.0, 16.0, 3.0, [0x6a0e0e, 0x9a1a1a, 0xc62828, 0xef5350]);
            }
            it::BOW => {
                // Curved limb from the top-right to the bottom-left, string straight.
                let pts: Vec<(f32, f32)> = (0..=20).map(|k| {
                    let t = k as f32 / 20.0;
                    let (x, y) = (27.0 - 22.0 * t, 4.0 + 23.0 * t);
                    let bend = (t * std::f32::consts::PI).sin() * 7.0;
                    (x - bend * 0.72, y - bend * 0.69)
                }).collect();
                c.line(27, 4, 5, 27, string);
                for w in pts.windows(2) {
                    c.stroke(w[0].0, w[0].1, w[1].0, w[1].1, 1.3, hex(WOOD[2]));
                }
                c.shade_box(11, 10, 5, 5, [0x2a1a0a, 0x4a2c14, 0x6b4423, 0x8a5a2e]); // grip
            }
            it::CROSSBOW => {
                c.shade_box(3, 14, 22, 5, WOOD); // stock
                c.shade_box(3, 18, 6, 6, WOOD);
                c.shade_box(21, 5, 4, 23, steel); // bow
                c.line(21, 5, 15, 16, string);
                c.line(21, 27, 15, 17, string);
                c.rect(25, 15, 4, 3, hex(steel[2]));
            }
            it::ARROWS | it::BOLTS => {
                for (k, y) in [(0, 8), (1, 15), (2, 22)] {
                    if i == it::ARROWS {
                        arrow(&mut c, 3 + k * 2, y, 20, true);
                    } else {
                        c.shade_box(6 + k * 2, y, 15, 3, WOOD);
                        c.shade_box(21 + k * 2, y - 1, 5, 5, steel);
                        c.rect(6 + k * 2, y - 1, 2, 5, hex(WOOD[0]));
                    }
                }
            }
            it::ARROW_SHOT => arrow(&mut c, 4, 15, 20, true),
            it::BOLT_SHOT => {
                c.shade_box(7, 14, 15, 4, WOOD);
                c.shade_box(22, 12, 6, 8, steel);
                c.rect(7, 12, 2, 8, hex(WOOD[0]));
            }
            it::SWORD => {
                for k in 0..17 {
                    c.rect(8 + k, 21 - k, 3, 3, hex(steel[1]));
                    c.px(9 + k, 21 - k, hex(steel[3]));
                }
                c.rect(25, 3, 3, 3, hex(steel[2]));
                c.stroke(4.0, 18.0, 13.0, 27.0, 1.5, hex(GOLD[1])); // guard
                c.stroke(4.0, 27.0, 7.0, 24.0, 1.8, hex(WOOD[1])); // grip
                c.disc(3.5, 28.5, 2.0, hex(GOLD[2]));
            }
            it::SPEAR => {
                c.stroke(4.0, 28.0, 22.0, 10.0, 1.2, hex(WOOD[2]));
                for k in 0..7 {
                    c.rect(22 + k, 4 + (6 - k), 3, 3, hex(steel[if k < 3 { 1 } else { 2 }]));
                }
                c.px(29, 3, hex(steel[3]));
                c.rect(20, 10, 3, 3, hex(0xb71c1c)); // tassel
            }
            it::AXE => {
                c.stroke(7.0, 28.0, 19.0, 6.0, 1.4, hex(WOOD[2]));
                // Crescent blade on the right of the haft, a small spike on the left.
                for y in 1..22 {
                    for x in 18..31 {
                        let (dx, dy) = ((x as f32 + 0.5 - 17.0) / 13.0, (y as f32 + 0.5 - 11.0) / 10.0);
                        let inner = ((x as f32 + 0.5 - 14.0) / 8.0).powi(2) + dy * dy;
                        if dx * dx + dy * dy <= 1.0 && (inner > 1.0 || x > 26) && (y as f32 - 11.0).abs() < 9.5 {
                            let edge = dx * dx + dy * dy > 0.72;
                            c.px(x, y, hex(if edge { steel[3] } else if dy < 0.0 { steel[2] } else { steel[1] }));
                        }
                    }
                }
                c.shade_box(16, 7, 5, 9, IRON); // socket
                for k in 0..4 {
                    c.vline(15 - k, 10 + k / 2, 3 - k / 2 * 2, hex(steel[1]));
                }
            }
            it::CAP | it::HELMET => {
                let pal = if i == it::CAP { ramp(0x8d5a2b) } else { ramp(0x9ea7b0) };
                c.shade_ellipse(16.0, 17.0, 11.0, 10.0, pal);
                c.rect(4, 17, 24, 10, CLEAR);
                c.shade_box(3, 17, 26, 4, pal);
                if i == it::HELMET {
                    c.shade_box(5, 20, 5, 8, pal);
                    c.shade_box(22, 20, 5, 8, pal);
                    c.shade_box(15, 7, 3, 18, pal);
                }
            }
            it::LEATHER | it::CHAIN => {
                let pal = if i == it::LEATHER { ramp(0x8d5a2b) } else { ramp(0xa7b1b9) };
                c.shade_box(8, 6, 16, 22, pal);
                c.shade_box(3, 6, 7, 9, pal);
                c.shade_box(22, 6, 7, 9, pal);
                c.rrect(12, 4, 8, 6, 3, CLEAR); // neck
                c.rect(8, 21, 16, 3, hex(pal[0]));
                c.rect(15, 21, 3, 3, hex(GOLD[2]));
                if i == it::CHAIN {
                    for y in (9..20).step_by(2) {
                        for x in (9 + (y % 4) / 2..23).step_by(2) {
                            c.px(x, y, hex(pal[1]));
                        }
                    }
                } else {
                    c.vline(16, 10, 10, hex(pal[1]));
                }
            }
            it::LANTERN => {
                // Iron frame with a glowing glass body and a ring handle.
                c.stroke(12.0, 7.0, 16.0, 3.0, 1.2, hex(IRON[2]));
                c.stroke(16.0, 3.0, 20.0, 7.0, 1.2, hex(IRON[2]));
                c.shade_box(10, 7, 12, 3, IRON);
                c.shade_box(11, 10, 10, 13, [0xb07a20, 0xf0b030, 0xffd860, 0xfff4c0]);
                c.rect(15, 13, 2, 6, hex(0xffffff));
                c.vline(11, 10, 13, hex(IRON[1]));
                c.vline(20, 10, 13, hex(IRON[1]));
                c.vline(15, 10, 3, hex(IRON[1]));
                c.shade_box(9, 23, 14, 4, IRON);
            }
            it::TORCH => {
                // Wooden handle with a wrapped head and a flame.
                c.stroke(9.0, 29.0, 18.0, 13.0, 1.6, hex(WOOD[2]));
                c.stroke(9.5, 29.0, 18.5, 13.0, 0.8, hex(WOOD[1]));
                c.shade_box(16, 10, 6, 5, [0x3a2a1a, 0x5a4028, 0x7a5a38, 0x9a7a50]);
                c.shade_disc(20.0, 8.0, 5.5, [0xc62828, 0xef6c00, 0xffa726, 0xffe082]);
                c.disc(20.5, 9.0, 2.5, hex(0xfff8e1));
                c.px(21, 2, hex(0xffa726));
                c.px(18, 3, hex(0xef6c00));
            }
            it::SCROLL => {
                // Rolled parchment with a red ribbon.
                c.shade_box(6, 9, 20, 15, [0x9e9580, 0xd8d0bc, 0xf5f0e1, 0xffffff]);
                c.shade_cyl(4, 6, 24, 5, [0x8a8070, 0xc8c0a8, 0xe8e0cc, 0xfff8e8]);
                c.shade_cyl(4, 22, 24, 5, [0x8a8070, 0xc8c0a8, 0xe8e0cc, 0xfff8e8]);
                for k in 0..4 {
                    c.hline(9, 12 + k * 3, 14 - k * 2, hex(0x6a6060));
                }
                c.rect(14, 8, 4, 18, hex(0xb71c1c));
            }
            it::BOOK => {
                // Closed book with a clasp and a glowing rune.
                c.shade_box(5, 5, 22, 24, [0x2a1a4a, 0x4a2a7a, 0x6a3aa0, 0x8a5ac0]);
                c.rect(5, 5, 4, 24, hex(0x2a1a4a));
                c.rect(24, 10, 3, 14, hex(GOLD[1]));
                c.shade_disc(16.0, 17.0, 5.0, [0x4a8aa0, 0x6ab0c8, 0x9adcf0, 0xe0fbff]);
                c.px(16, 14, hex(0xffffff));
            }
            it::ANTIDOTE | it::ELIXIR | it::SWIFTNESS => {
                let liquid = match i {
                    it::ANTIDOTE => [0x1b5e20, 0x2e7d32, 0x43a047, 0xa5d6a7],
                    it::ELIXIR => [0x8a3a00, 0xc25e00, 0xf57c00, 0xffcc80],
                    _ => [0x006064, 0x00838f, 0x26c6da, 0xb2ebf2],
                };
                c.shade_box(13, 3, 6, 4, WOOD); // cork
                c.rect(14, 7, 4, 5, hex(0xcfe8ff));
                c.shade_disc(16.0, 20.0, 10.0, [0x7fa8c8, 0xa8cbe8, 0xcfe8ff, 0xffffff]);
                c.shade_disc(16.0, 21.0, 8.0, liquid);
                c.hline(9, 17, 14, hex(liquid[3]));
                c.rect(11, 14, 2, 4, hex(0xffffff));
                // A small mark per potion: a leaf, a fist-like block, a streak.
                match i {
                    it::ANTIDOTE => {
                        c.rect(15, 20, 2, 5, hex(0xe8f5e9));
                        c.rect(13, 21, 6, 2, hex(0xe8f5e9));
                    }
                    it::ELIXIR => c.rect(14, 19, 5, 5, hex(0xfff3e0)),
                    _ => {
                        c.hline(12, 20, 8, hex(0xe0f7fa));
                        c.hline(14, 23, 7, hex(0xe0f7fa));
                    }
                }
            }
            it::MANA_POTION => {
                c.shade_box(13, 3, 6, 4, WOOD); // cork
                c.rect(14, 7, 4, 5, hex(0xcfe8ff));
                c.shade_disc(16.0, 20.0, 10.0, [0x7fa8c8, 0xa8cbe8, 0xcfe8ff, 0xffffff]);
                c.shade_disc(16.0, 21.0, 8.0, [0x1a237e, 0x283593, 0x3f51b5, 0x7986cb]);
                c.hline(9, 17, 14, hex(0x7986cb));
                c.rect(11, 14, 2, 4, hex(0xffffff));
            }
            it::SPELL_HEAL => {
                // Soft green glow with a cross.
                c.shade_disc(16.0, 16.0, 12.0, [0x1d5124, 0x2e7d32, 0x43a047, 0x7cc56f]);
                c.rect(13, 7, 6, 18, hex(0xe8ffe8));
                c.rect(7, 13, 18, 6, hex(0xe8ffe8));
            }
            it::SPELL_SPARK => {
                // Yellow lightning bolt on a blue disc.
                c.shade_disc(16.0, 16.0, 12.0, [0x0d2a5a, 0x1a4a8a, 0x2a6ab0, 0x5a9ad8]);
                for (x0, y0, x1, y1) in [(18.0, 5.0, 12.0, 16.0), (12.0, 16.0, 18.0, 16.0), (18.0, 16.0, 12.0, 27.0)] {
                    c.stroke(x0, y0, x1, y1, 2.2, hex(0xffe84a));
                }
                c.px(17, 6, hex(0xffffff));
            }
            it::SPELL_LIGHT => {
                // A sun.
                c.shade_disc(16.0, 16.0, 12.0, [0x3a2a0a, 0x6a4a10, 0x9a7a20, 0xc0a040]);
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    c.stroke(16.0 + a.cos() * 7.0, 16.0 + a.sin() * 7.0, 16.0 + a.cos() * 11.5, 16.0 + a.sin() * 11.5, 1.6, hex(0xffe8a0));
                }
                c.shade_disc(16.0, 16.0, 5.5, [0xe0a020, 0xffd040, 0xffe88a, 0xfffff0]);
            }
            it::SPELL_BLAST => {
                // A ring of fire.
                c.shade_disc(16.0, 16.0, 12.0, [0x3a0a0a, 0x6a1a10, 0x9a2a10, 0xc04a20]);
                for k in 0..10 {
                    let a = k as f32 * std::f32::consts::TAU / 10.0;
                    let (x, y) = (16.0 + a.cos() * 8.0, 16.0 + a.sin() * 8.0);
                    c.shade_disc(x, y, 3.2, [0xc62828, 0xef6c00, 0xffa726, 0xffe082]);
                }
                c.shade_disc(16.0, 16.0, 4.0, [0x3a0a0a, 0x6a1a10, 0x9a2a10, 0xc04a20]);
            }
            it::SPARK_SHOT => {
                // Small bolt flying right.
                c.stroke(4.0, 16.0, 24.0, 16.0, 2.5, hex(0xffe84a));
                c.stroke(4.0, 16.0, 24.0, 16.0, 1.0, hex(0xffffff));
                c.shade_disc(25.0, 16.0, 4.0, [0xc08a10, 0xffd040, 0xffe88a, 0xffffff]);
            }
            it::STAR => {
                // Five-pointed star (arcana).
                let pts: Vec<(f32, f32)> = (0..10).map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    let r = if k % 2 == 0 { 13.0 } else { 5.5 };
                    (16.0 + a.cos() * r, 17.0 + a.sin() * r)
                }).collect();
                for k in 0..10 {
                    let (a, b) = (pts[k], pts[(k + 1) % 10]);
                    c.stroke(a.0, a.1, b.0, b.1, 2.0, hex(GOLD[2]));
                }
                c.disc(16.0, 17.0, 5.0, hex(GOLD[2]));
                c.disc(16.0, 17.0, 2.5, hex(GOLD[3]));
            }
            it::LOCKPICK => {
                // Two bent wires.
                c.stroke(5.0, 26.0, 20.0, 11.0, 1.6, hex(0xc5d0d6));
                c.stroke(20.0, 11.0, 26.0, 8.0, 1.6, hex(0xc5d0d6));
                c.stroke(26.0, 8.0, 27.0, 12.0, 1.6, hex(0xc5d0d6));
                c.stroke(4.0, 20.0, 22.0, 20.0, 1.6, hex(0x90a4ae));
                c.stroke(22.0, 20.0, 26.0, 24.0, 1.6, hex(0x90a4ae));
            }
            it::SHIELD => {
                // Round wooden shield: planks, iron rim and boss.
                c.shade_disc(16.0, 16.0, 13.0, IRON);
                c.shade_disc(16.0, 16.0, 11.0, WOOD);
                for x in [11, 16, 21] {
                    c.vline(x, 6, 21, hex(WOOD[0]));
                }
                c.shade_disc(16.0, 16.0, 3.5, [0x5f6b73, 0x90a4ae, 0xc5d0d6, 0xf0f4f6]);
            }
            it::ROD => {
                // A long wooden rod with a reel and the line hanging down.
                c.stroke(5.0, 28.0, 27.0, 4.0, 2.0, hex(WOOD[2]));
                c.stroke(5.0, 28.0, 12.0, 20.5, 2.6, hex(WOOD[0]));
                c.shade_disc(11.0, 23.0, 3.2, IRON);
                c.disc(11.0, 23.0, 1.2, hex(0xc5d0d6));
                c.vline(27, 5, 16, hex(0xe0e0e0));
                c.rect(26, 20, 3, 3, hex(0xe53935));
            }
            it::TROUT => {
                // A speckled fish swimming right.
                let body = [0x4e6a3a, 0x7a9a5a, 0xa8c08a, 0xe0ecd0];
                c.shade_disc(15.0, 16.0, 8.0, body);
                c.rect(4, 10, 24, 3, CLEAR);
                c.rect(4, 20, 24, 3, CLEAR);
                c.shade_disc(15.0, 16.0, 6.0, body);
                c.shade_disc(21.0, 16.0, 4.5, body);
                for (fx, fy) in [(3, 11), (3, 21), (5, 13), (5, 19), (6, 15), (6, 17)] {
                    c.px(fx, fy, hex(body[1]));
                }
                c.rect(4, 12, 3, 9, hex(body[1])); // tail
                c.hline(9, 17, 16, hex(0xe57373)); // pink stripe
                for (sx, sy) in [(11, 13), (14, 12), (17, 14), (12, 19)] {
                    c.px(sx, sy, hex(0x2e3b22));
                }
                c.px(23, 15, hex(0x111111));
            }
            it::BOOT => {
                // A soggy old boot.
                let leather = [0x3e2a1a, 0x5d4030, 0x7b5a44, 0x9c7a62];
                c.shade_box(9, 5, 9, 17, leather);
                c.shade_box(9, 19, 18, 7, leather);
                c.hline(9, 26, 18, hex(0x2a1c10));
                c.hline(9, 8, 9, hex(leather[3]));
                c.px(22, 17, hex(0x6fa8dc)); // a drip
                c.px(22, 18, hex(0x6fa8dc));
            }
            it::PICKAXE => {
                // A double-ended iron head on a wooden haft.
                c.stroke(9.0, 28.0, 22.0, 8.0, 1.8, hex(WOOD[1]));
                c.stroke(9.5, 27.5, 21.5, 8.5, 0.8, hex(WOOD[3]));
                c.stroke(8.0, 10.0, 22.0, 6.0, 2.2, hex(IRON[1]));
                c.stroke(22.0, 6.0, 28.0, 13.0, 2.0, hex(IRON[1]));
                c.stroke(8.0, 10.0, 4.0, 17.0, 1.8, hex(IRON[1]));
                c.stroke(9.0, 9.0, 21.0, 6.0, 0.8, hex(IRON[3]));
            }
            it::COPPER_ORE | it::IRON_ORE | it::SILVER_ORE => {
                let ore = match i {
                    it::COPPER_ORE => [0x7a3e1a, 0xb8642c, 0xe0904a, 0xffc890],
                    it::IRON_ORE => [0x4a3a36, 0x7a6058, 0xa88a7e, 0xd8c4b8],
                    _ => [0x6c7480, 0xa8b2be, 0xd8e0ea, 0xffffff],
                };
                c.shade_ellipse(15.0, 18.0, 10.0, 8.0, STONE);
                c.shade_ellipse(20.0, 14.0, 6.0, 5.0, STONE);
                for (fx, fy, r) in [(12.0, 17.0, 2.6), (18.0, 13.0, 2.2), (19.0, 20.0, 2.0), (10.0, 21.0, 1.6)] {
                    c.shade_disc(fx, fy, r, ore);
                }
            }
            it::WHEAT_SEEDS | it::CARROT_SEEDS | it::PUMPKIN_SEEDS => {
                // A tied paper pouch with a picture of its crop.
                let mark = match i {
                    it::WHEAT_SEEDS => 0xe8c860,
                    it::CARROT_SEEDS => 0xf08a30,
                    _ => 0xd8661a,
                };
                c.shade_box(8, 10, 16, 18, [0x8a7a5a, 0xc8b48a, 0xe2d2aa, 0xf6ecd0]);
                c.shade_box(10, 6, 12, 5, [0x8a7a5a, 0xc8b48a, 0xe2d2aa, 0xf6ecd0]);
                c.hline(9, 10, 14, hex(0x6a4a2a));
                c.disc(16.0, 19.0, 4.0, hex(mark));
                c.vline(16, 13, 3, hex(LEAF[2]));
            }
            it::WHEAT => {
                // A sheaf of golden stalks, tied in the middle.
                for k in -2..=2 {
                    c.stroke(16.0 + k as f32 * 1.2, 28.0, 16.0 + k as f32 * 3.0, 9.0, 0.7, hex(0xc8a040));
                    c.shade_box(15 + k * 3, 4, 3, 6, [0x8a6a20, 0xc8a040, 0xe8c860, 0xfff0a0]);
                }
                c.hline(12, 20, 9, hex(0x8a4a20));
            }
            it::CARROT => {
                c.stroke(10.0, 26.0, 21.0, 11.0, 3.2, hex(0xd86a1a));
                c.stroke(10.5, 25.0, 20.0, 12.0, 1.2, hex(0xffb870));
                c.shade_disc(23.0, 8.0, 3.0, LEAF);
                c.shade_disc(20.0, 6.5, 2.5, LEAF);
                c.shade_disc(25.5, 10.5, 2.5, LEAF);
            }
            it::PUMPKIN => {
                c.shade_ellipse(16.0, 19.0, 12.0, 9.0, [0x8a3a06, 0xd8661a, 0xf08a2a, 0xffbc6a]);
                for rx in [10, 16, 22] {
                    c.vline(rx, 12, 15, hex(0xb85010));
                }
                c.shade_box(15, 7, 3, 4, WOOD);
                c.shade_disc(20.0, 8.0, 2.5, LEAF);
            }
            it::HAMMER => {
                // A heavy iron head on a short haft.
                c.stroke(8.0, 28.0, 19.0, 12.0, 1.8, hex(WOOD[1]));
                c.stroke(8.5, 27.5, 18.5, 12.5, 0.7, hex(WOOD[3]));
                c.shade_box(13, 4, 14, 9, IRON);
                c.hline(13, 4, 14, hex(IRON[3]));
                c.rect(24, 5, 3, 7, hex(IRON[2]));
            }
            it::IRON_SWORD | it::FINE_SWORD => {
                // A longer blade than the short sword; the fine one gleams with a gold guard and
                // a blue gem in the pommel.
                let fine = i == it::FINE_SWORD;
                let blade = if fine { [0x5f6b73, 0xb8c8d4, 0xe2ecf4, 0xffffff] } else { [0x4a4a52, 0x7c8088, 0xa8acb4, 0xd8dce0] };
                for k in 0..20 {
                    c.rect(6 + k, 24 - k, 3, 3, hex(blade[1]));
                    c.px(7 + k, 24 - k, hex(blade[3]));
                }
                c.rect(26, 2, 3, 3, hex(blade[2]));
                let guard = if fine { GOLD[2] } else { IRON[2] };
                c.stroke(2.0, 19.0, 12.0, 29.0, 1.6, hex(guard));
                c.stroke(2.0, 29.0, 5.0, 26.0, 1.8, hex(WOOD[1]));
                c.disc(1.8, 30.2, 2.0, hex(if fine { 0x4fc3f7 } else { IRON[3] }));
            }
            it::MORTAR => {
                // A stone bowl with a wooden pestle leaning in it.
                c.stroke(14.0, 18.0, 24.0, 5.0, 2.0, hex(WOOD[2]));
                c.disc(24.0, 5.0, 2.4, hex(WOOD[3]));
                c.shade_ellipse(16.0, 22.0, 11.0, 7.0, STONE);
                c.rect(5, 15, 23, 6, CLEAR);
                c.shade_ellipse(16.0, 18.0, 11.0, 3.0, [0x2a2a30, 0x45454e, 0x5a5a64, 0x6c6c76]);
                c.shade_box(10, 27, 12, 2, STONE);
                c.ellipse(16.0, 18.0, 6.0, 1.5, hex(0x7cc56f));
            }
            it::TONGS => {
                // Two long iron arms joined by a rivet.
                c.stroke(6.0, 28.0, 20.0, 9.0, 1.4, hex(IRON[2]));
                c.stroke(10.0, 28.0, 18.0, 8.0, 1.4, hex(IRON[3]));
                c.stroke(20.0, 9.0, 25.0, 4.0, 1.6, hex(IRON[2]));
                c.stroke(18.0, 8.0, 21.0, 3.0, 1.6, hex(IRON[3]));
                c.disc(17.0, 13.0, 1.8, hex(GOLD[2]));
            }
            it::COAL => {
                c.shade_ellipse(15.0, 19.0, 9.0, 7.0, [0x0c0c10, 0x1c1c24, 0x30303c, 0x6a6a7a]);
                c.shade_ellipse(20.0, 14.0, 5.0, 4.0, [0x0c0c10, 0x1c1c24, 0x30303c, 0x6a6a7a]);
                c.px(12, 15, hex(0x9a9aaa));
                c.px(19, 11, hex(0x9a9aaa));
            }
            it::COPPER_INGOT | it::IRON_INGOT | it::SILVER_INGOT => {
                let metal = match i {
                    it::COPPER_INGOT => [0x6a2e10, 0xa85a28, 0xd8864a, 0xffc28a],
                    it::IRON_INGOT => [0x3a3a44, 0x60606c, 0x8a8a96, 0xc0c0cc],
                    _ => [0x7c8490, 0xb4bcc8, 0xdde3ec, 0xffffff],
                };
                // A trapezoid bar seen from the side, with a bright top face.
                c.shade_box(6, 16, 20, 9, metal);
                c.shade_box(9, 11, 14, 5, metal);
                c.hline(9, 11, 14, hex(metal[3]));
                c.hline(6, 16, 20, hex(metal[2]));
            }
            it::GEM => {
                // A cut red gem.
                let red = [0x6a0e2a, 0xb01e48, 0xe8406e, 0xffa0c0];
                c.shade_box(9, 10, 14, 6, red);
                c.shade_box(11, 16, 10, 4, red);
                c.shade_box(13, 20, 6, 3, red);
                c.shade_box(15, 23, 2, 2, red);
                c.hline(9, 10, 14, hex(red[3]));
                c.px(12, 12, hex(0xffffff));
            }
            it::BANDAGE => {
                // A roll of linen with its loose end trailing off, marked with a red cross.
                let linen = [0x9e9a90, 0xcfcac0, 0xece8de, 0xffffff];
                c.shade_box(14, 18, 14, 5, linen); // the loose end
                c.shade_disc(12.0, 15.0, 8.5, linen);
                c.shade_disc(12.0, 15.0, 3.0, [0x8a857a, 0xb0aa9e, 0xcfcac0, 0xe6e2d8]);
                for k in [7, 10] {
                    c.px(k, 9, hex(linen[0]));
                    c.px(k + 9, 20, hex(linen[1]));
                }
                c.rect(23, 18, 2, 5, hex(0xd32f2f));
                c.rect(21, 19, 6, 2, hex(0xd32f2f));
                c.hline(14, 23, 14, hex(linen[0]));
            }
            it::HERB => {
                // A sprig with three pairs of leaves and a tied stem.
                let leaf = [0x1b5e20, 0x2e7d32, 0x43a047, 0x81c784];
                c.stroke(16.0, 28.0, 16.0, 6.0, 1.4, hex(0x33691e));
                for (k, y) in [9.0f32, 15.0, 21.0].into_iter().enumerate() {
                    let w = 6.5 - k as f32;
                    c.shade_disc(16.0 - w * 0.8, y, w * 0.6, leaf);
                    c.shade_disc(16.0 + w * 0.8, y + 1.5, w * 0.6, leaf);
                }
                c.shade_disc(16.0, 5.5, 2.5, leaf);
                c.rect(14, 24, 5, 2, hex(0xd7ccc8)); // string
            }
            it::MUSHROOM => {
                // A red cap with white spots on a pale stem.
                c.shade_disc(16.0, 16.0, 11.0, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
                c.rect(4, 16, 24, 12, CLEAR); // a dome: only the upper half
                c.shade_box(13, 15, 6, 13, [0xbcaaa4, 0xd7ccc8, 0xefebe9, 0xffffff]);
                c.hline(6, 15, 20, hex(0x7a1010));
                for (sx, sy) in [(11, 9), (18, 7), (21, 12), (14, 13)] {
                    c.rect(sx, sy, 2, 2, hex(0xffffff));
                }
            }
            _ => {}
        }
        c.auto_outline(n, n, OUTLINE);
    }
    c.save(&out.join("sprites/items.png"));
}

/// Frames of `sprites/food.png`: cooked dishes (32 x 32 each).
pub mod food {
    pub const GRILLED_TROUT: u32 = 0;
    pub const BREAD: u32 = 1;
    pub const CARROT_SOUP: u32 = 2;
    pub const STEW: u32 = 3;
    pub const PIE: u32 = 4;
    pub const COUNT: u32 = 5;
}

/// The cooked dishes' icons (their own sheet: `items.png` is near the texture size limit).
pub fn food_icons(out: &Path) {
    let mut c = Canvas::new(TS * food::COUNT, TS);
    let n = TS as i32;
    for i in 0..food::COUNT {
        c.at(i * TS, 0);
        match i {
            food::GRILLED_TROUT => {
                // A browned fish with grill stripes on a stick.
                c.stroke(2.0, 28.0, 28.0, 5.0, 1.0, hex(WOOD[2]));
                c.shade_ellipse(15.0, 16.0, 10.0, 5.0, [0x5a3014, 0x8a5024, 0xb8783a, 0xe0a868]);
                c.shade_box(23, 12, 4, 8, [0x5a3014, 0x8a5024, 0xb8783a, 0xe0a868]);
                for gx in [9, 13, 17] {
                    c.line(gx, 12, gx + 3, 20, hex(0x3a1c0a));
                }
                c.px(9, 15, hex(0x111111));
            }
            food::BREAD => {
                c.shade_ellipse(16.0, 19.0, 12.0, 7.0, [0x7a4a18, 0xb87a34, 0xd8a056, 0xf0c888]);
                for sx in [10, 15, 20] {
                    c.line(sx, 15, sx + 3, 20, hex(0xf0d8a0));
                }
            }
            food::CARROT_SOUP | food::STEW => {
                // A wooden bowl with soup and bits in it.
                let soup = if i == food::CARROT_SOUP { [0xa04a10, 0xd8701e, 0xf0923a, 0xffc078] } else { [0x5a2e14, 0x8a4a22, 0xb06a34, 0xd8945a] };
                c.shade_ellipse(16.0, 21.0, 12.0, 7.0, WOOD);
                c.rect(3, 13, 26, 8, CLEAR);
                c.shade_ellipse(16.0, 17.0, 12.0, 4.0, soup);
                for (bx, by, col) in [(11, 16, 0xf08a30), (17, 18, 0x7cc56f), (21, 16, 0xffd8a0)] {
                    c.rect(bx, by, 2, 2, hex(col));
                }
                c.shade_box(10, 27, 12, 2, WOOD);
                c.px(14, 9, hex(0xe0e0e0));
                c.px(18, 8, hex(0xe0e0e0));
            }
            food::PIE => {
                c.shade_ellipse(16.0, 20.0, 13.0, 7.0, [0x7a4a18, 0xb87a34, 0xd8a056, 0xf0c888]);
                c.shade_ellipse(16.0, 18.0, 11.0, 4.5, [0x6a1010, 0xa02020, 0xc84040, 0xe87070]);
                for k in 0..4 {
                    c.hline(7, 16 + k * 2, 18, hex(0xe8c080));
                    c.vline(9 + k * 4, 14, 8, hex(0xe8c080));
                }
            }
            _ => {}
        }
        c.auto_outline(n, n, OUTLINE);
    }
    c.save(&out.join("sprites/food.png"));
}

// ------------------------------------------------------------------ spell effects

/// Fire colors from dark red to white-hot.
const FIRE: [u32; 5] = [0x7a1606, 0xc8380c, 0xf07818, 0xffc430, 0xfff4c0];

/// A stable noise value 0..1 per pixel (so flames don't jump around between frames).
fn noise(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263) ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) % 1000) as f32 / 1000.0
}

/// `sprites/fireball.png`: 4 frames (32 x 32) of a fireball flying to the right, with a
/// flickering tail (the game rotates it into its direction).
pub fn fireball(out: &Path) {
    let mut c = Canvas::new(32 * 4, 32);
    for f in 0..4u32 {
        c.at(f * 32, 0);
        // Tail: puffs trailing to the left, shrinking and cooling.
        for k in 0..7 {
            let k = k as f32;
            let wob = ((f as f32 * 1.7 + k * 1.3).sin()) * (0.6 + k * 0.35);
            let (x, y, r) = (18.0 - k * 2.6, 16.0 + wob, 5.2 - k * 0.6);
            if r <= 0.5 {
                break;
            }
            c.disc(x, y, r, hex(FIRE[if k < 2.0 { 1 } else { 0 }]));
            c.disc(x + 0.5, y, r * 0.55, hex(FIRE[if k < 3.0 { 2 } else { 1 }]));
        }
        // Flickering sparks behind it.
        for s in 0..5 {
            let n = noise(s, f as i32, 7);
            let x = 2 + (n * 10.0) as i32;
            let y = 10 + (noise(s, f as i32, 9) * 12.0) as i32;
            c.px(x, y, hex(FIRE[3]));
        }
        // Head.
        let pulse = [0.0, 0.5, 0.2, 0.7][f as usize];
        c.disc(21.0, 16.0, 7.0 + pulse, hex(FIRE[1]));
        c.disc(21.5, 16.0, 5.4 + pulse * 0.6, hex(FIRE[2]));
        c.disc(22.0, 15.5, 3.6, hex(FIRE[3]));
        c.disc(22.5, 15.0, 1.8, hex(FIRE[4]));
    }
    c.save(&out.join("sprites/fireball.png"));
}

/// `sprites/explosion.png`: 8 frames (64 x 64): a burst of fire that swells, cools to smoke
/// and dissolves.
/// A burst of blue magic (64x64 frames x 8): a bright core, an expanding ring of light and
/// sparkles flying outwards, fading.
pub fn magic_burst(out: &Path) {
    let mut c = Canvas::new(64 * 8, 64);
    let mut rng = Rng(77);
    let sparks: Vec<(f32, f32)> = (0..18).map(|_| (rng.range(628) as f32 / 100.0, 0.6 + rng.range(40) as f32 / 100.0)).collect();
    for f in 0..8i32 {
        c.at(f as u32 * 64, 0);
        let t = f as f32 / 7.0;
        let fade = 1.0 - t * 0.85;
        // Core.
        let core = 10.0 * (1.0 - t).max(0.0);
        if core > 0.5 {
            c.disc(32.0, 32.0, core + 4.0, Color::new(0.33, 0.83, 1.0, 0.45 * fade));
            c.disc(32.0, 32.0, core, Color::new(0.62, 0.94, 1.0, 0.9 * fade));
            c.disc(32.0, 32.0, core * 0.5, Color::new(1.0, 1.0, 1.0, fade));
        }
        // The ring.
        let r = 6.0 + t * 24.0;
        for y in 0..64 {
            for x in 0..64 {
                let d = ((x as f32 + 0.5 - 32.0).powi(2) + (y as f32 + 0.5 - 32.0).powi(2)).sqrt();
                let band = 1.0 - ((d - r).abs() / (2.5 + t * 2.0));
                if band > 0.0 {
                    let a = band * fade;
                    c.px(x, y, Color::new(0.55 + 0.4 * band, 0.9 + 0.1 * band, 1.0, a.min(1.0)));
                }
            }
        }
        // Sparkles flying outwards.
        for (a, sp) in &sparks {
            let d = 4.0 + t * 28.0 * sp;
            let (x, y) = ((32.0 + a.cos() * d) as i32, (32.0 + a.sin() * d) as i32);
            let col = Color::new(0.85, 0.98, 1.0, fade);
            c.px(x, y, col);
            if f < 5 {
                c.px(x + 1, y, Color::new(0.55, 0.9, 1.0, fade * 0.6));
                c.px(x, y + 1, Color::new(0.55, 0.9, 1.0, fade * 0.6));
            }
        }
    }
    c.save(&out.join("sprites/magic_burst.png"));
}

pub fn explosion(out: &Path) {
    let mut c = Canvas::new(64 * 8, 64);
    const RADIUS: [f32; 8] = [8.0, 15.0, 21.0, 25.0, 28.0, 29.5, 30.5, 31.0];
    for f in 0..8i32 {
        c.at(f as u32 * 64, 0);
        let r = RADIUS[f as usize];
        let cool = (f as f32 - 2.0).max(0.0) * 0.17;
        for y in 0..64 {
            for x in 0..64 {
                let (dx, dy) = (x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 32.0);
                // Ragged edge: noise in the radius.
                let n = noise(x / 3, y / 3, 1) * 0.35 + noise(x, y, 2) * 0.15;
                let d = (dx * dx + dy * dy).sqrt() / (r * (0.8 + n));
                if d > 1.0 {
                    continue;
                }
                // Later frames dissolve from the inside out (an expanding smoke ring).
                if f >= 5 && bayer(x, y) < (f - 4) as f32 * 0.28 * (1.2 - d) {
                    continue;
                }
                let heat = (1.0 - d) * 1.25 + noise(x, y, f as u32 + 3) * 0.2 - cool;
                let col = if heat > 0.95 {
                    hex(FIRE[4])
                } else if heat > 0.7 {
                    hex(FIRE[3])
                } else if heat > 0.45 {
                    hex(FIRE[2])
                } else if heat > 0.22 {
                    hex(FIRE[1])
                } else if heat > 0.0 || f < 3 {
                    hex(FIRE[0])
                } else {
                    // Smoke.
                    let g = 0.22 + noise(x / 2, y / 2, 5) * 0.12;
                    Color::new(g, g * 0.95, g * 0.92, 0.9)
                };
                c.px(x, y, col);
            }
        }
    }
    c.save(&out.join("sprites/explosion.png"));
}

// ------------------------------------------------------------------ portraits

/// Frames of `sprites/portraits.png` (64 x 64 each).
pub mod portrait {
    pub const ELDER: u32 = 0;
    pub const ELDER_WORRIED: u32 = 1;
    pub const TILDA: u32 = 2;
    pub const COUNT: u32 = 3;
    pub const SIZE: u32 = 64;
}

/// Drawn dialog portraits: the elder (calm and worried) and Tilda the merchant, head and
/// shoulders on a dusky backdrop.
pub fn portraits(out: &Path) {
    const S: i32 = portrait::SIZE as i32;
    let mut c = Canvas::new(portrait::SIZE * portrait::COUNT, portrait::SIZE);
    let skin = [0x9c6b48, 0xc98f63, 0xe8b88a, 0xf6d3b0];
    for k in 0..portrait::COUNT {
        let c = c.at(k * portrait::SIZE, 0);
        // Backdrop: a soft vignette.
        let bg = if k == portrait::TILDA { [0x1f2a22, 0x2c3b30, 0x3b5040, 0x4a6250] } else { [0x1c1f2e, 0x282c40, 0x363b55, 0x444a68] };
        for y in 0..S {
            for x in 0..S {
                let (dx, dy) = ((x as f32 - 32.0) / 40.0, (y as f32 - 26.0) / 40.0);
                let l = 1.0 - (dx * dx + dy * dy).sqrt() + (bayer(x, y) - 0.5) * 0.12;
                let i = if l < 0.25 { 0 } else if l < 0.5 { 1 } else if l < 0.75 { 2 } else { 3 };
                c.px(x, y, hex(bg[i]));
            }
        }
        match k {
            portrait::ELDER | portrait::ELDER_WORRIED => {
                let worried = k == portrait::ELDER_WORRIED;
                let robe = [0x2a1638, 0x45245c, 0x613883, 0x8456a8];
                let white = [0x8f8f96, 0xbdbdc4, 0xdedee4, 0xffffff];
                // Shoulders in the purple robe with a golden trim.
                c.shade_ellipse(32.0, 70.0, 30.0, 22.0, robe);
                c.stroke(22.0, 50.0, 32.0, 64.0, 1.2, hex(0xd4a838));
                c.stroke(42.0, 50.0, 32.0, 64.0, 1.2, hex(0xd4a838));
                // Neck and head.
                c.rect(27, 40, 10, 8, hex(skin[1]));
                c.shade_ellipse(32.0, 28.0, 12.5, 15.0, skin);
                // Ears.
                c.shade_ellipse(19.5, 29.0, 2.5, 4.0, skin);
                c.shade_ellipse(44.5, 29.0, 2.5, 4.0, skin);
                // White hair: a receding crown and full sides.
                c.shade_ellipse(32.0, 15.0, 12.0, 5.0, white);
                c.shade_ellipse(21.0, 22.0, 4.0, 8.0, white);
                c.shade_ellipse(43.0, 22.0, 4.0, 8.0, white);
                // A long beard and moustache.
                c.shade_ellipse(32.0, 45.0, 11.0, 12.0, white);
                c.shade_ellipse(32.0, 36.0, 9.0, 3.5, white);
                c.rect(29, 37, 6, 1, hex(0x7a5040)); // the mouth under the moustache
                // Eyes, brows, nose and wrinkles.
                for ex in [26, 36] {
                    c.rect(ex, 28, 3, 2, hex(0xffffff));
                    c.px(ex + 1, 28, hex(0x3a2a20));
                    c.px(ex + 1, 29, hex(0x3a2a20));
                }
                if worried {
                    // Brows raised towards the middle, a crease, a bead of sweat.
                    c.line(24, 25, 29, 23, hex(0xdedee4));
                    c.line(35, 23, 40, 25, hex(0xdedee4));
                    c.line(24, 26, 29, 24, hex(0xbdbdc4));
                    c.line(35, 24, 40, 26, hex(0xbdbdc4));
                    c.line(31, 21, 31, 23, hex(skin[0]));
                    c.line(33, 21, 33, 23, hex(skin[0]));
                    c.rect(43, 22, 2, 3, hex(0xa8d8ff));
                    c.px(43, 21, hex(0xa8d8ff));
                } else {
                    c.rect(24, 25, 6, 2, hex(0xdedee4));
                    c.rect(34, 25, 6, 2, hex(0xdedee4));
                    c.hline(27, 20, 10, hex(skin[1]));
                }
                c.line(32, 29, 31, 33, hex(skin[1]));
                c.px(32, 33, hex(skin[0]));
            }
            _ => {
                let hair = [0x1e120c, 0x3a2418, 0x5a3a26, 0x7a5236];
                let dress = [0x1b3a24, 0x2e5c3a, 0x43804f, 0x63a56c];
                // Long hair behind the shoulders.
                c.shade_ellipse(32.0, 34.0, 17.0, 24.0, hair);
                // Shoulders: a green dress with a white apron strap and collar.
                c.shade_ellipse(32.0, 70.0, 28.0, 22.0, dress);
                c.stroke(26.0, 50.0, 24.0, 64.0, 1.5, hex(0xece6d6));
                c.stroke(38.0, 50.0, 40.0, 64.0, 1.5, hex(0xece6d6));
                c.rect(27, 41, 10, 8, hex(skin[1]));
                c.shade_ellipse(32.0, 29.0, 11.5, 14.0, skin);
                // Fringe and side locks.
                c.shade_ellipse(32.0, 17.0, 12.5, 6.0, hair);
                c.shade_ellipse(21.5, 27.0, 3.5, 11.0, hair);
                c.shade_ellipse(42.5, 27.0, 3.5, 11.0, hair);
                // Eyes with lashes, brows, rosy cheeks, a smile.
                for ex in [26, 36] {
                    c.rect(ex, 29, 3, 2, hex(0xffffff));
                    c.px(ex + 1, 29, hex(0x3d6b3a));
                    c.px(ex + 1, 30, hex(0x1e2a1a));
                    c.hline(ex, 28, 3, hex(0x1e120c));
                }
                c.line(25, 26, 29, 25, hex(0x3a2418));
                c.line(35, 25, 39, 26, hex(0x3a2418));
                c.rect(24, 33, 3, 2, hex(0xe8907a));
                c.rect(37, 33, 3, 2, hex(0xe8907a));
                c.line(32, 30, 31, 34, hex(skin[1]));
                c.line(29, 37, 31, 38, hex(0xa04848));
                c.line(31, 38, 33, 38, hex(0xa04848));
                c.line(33, 38, 35, 37, hex(0xa04848));
                // A small earring.
                c.px(20, 34, hex(0xd4a838));
            }
        }
    }
    c.save(&out.join("sprites/portraits.png"));
}
