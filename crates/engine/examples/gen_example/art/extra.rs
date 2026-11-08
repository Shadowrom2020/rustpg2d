//! Extra assets for the default asset library (not used by the example maps): desert,
//! mountain, city and dungeon tilesets, more props (several with states) and characters.

use std::collections::BTreeMap;

use rustpg2d_engine::defs::*;
use rustpg2d_engine::entity::Facing;
use rustpg2d_engine::imageio::Rgba;

use super::*;

impl Canvas {
    /// The image as PNG bytes (no file).
    fn png(&self) -> Vec<u8> {
        Rgba { w: self.img.width as u32, h: self.img.height as u32, px: self.img.bytes.clone() }.encode().unwrap()
    }
}

/// Everything extra: definitions and the files they use (by project-relative path).
pub struct Extras {
    pub db: Database,
    pub files: BTreeMap<String, Vec<u8>>,
}

pub fn library_extras() -> Extras {
    let mut x = Extras { db: Database::default(), files: BTreeMap::new() };
    for (id, canvas, solid, anims, terrains) in [desert(), mountains(), city(), dungeon()] {
        let image = format!("tilesets/{id}.png");
        x.files.insert(image.clone(), canvas.png());
        // Wall torches (dungeon) and wall lanterns (city) light up dark maps.
        let lights = match id {
            "dungeon" => vec![TileLight { tile: 10, light: LightSpec::torch(4.5) }],
            "city" => vec![TileLight { tile: 15, light: LightSpec::new(4.0, [255, 226, 170], false) }],
            _ => vec![],
        };
        x.db.tilesets.push(TilesetDef { id: id.into(), image, solid, animations: anims, lights, terrains });
    }
    props(&mut x);
    // Props that give light in their default state ("lit"; their "off" / "out" states don't).
    for p in &mut x.db.props {
        p.light = match p.id.as_str() {
            "street_lamp" => Some(LightSpec::new(5.0, [255, 226, 170], false)),
            "campfire" => Some(LightSpec::torch(5.5)),
            "wall_torch" => Some(LightSpec::torch(4.5)),
            "crystal" => Some(LightSpec { intensity: 0.7, ..LightSpec::new(2.5, [150, 120, 255], false) }),
            _ => None,
        };
    }
    characters(&mut x);
    x
}

/// The dungeon tileset (for the example project's mine).
pub fn dungeon_tileset() -> (TilesetDef, Vec<u8>) {
    let (id, canvas, solid, animations, terrains) = dungeon();
    let lights = vec![TileLight { tile: 10, light: LightSpec::torch(4.5) }];
    (TilesetDef { id: id.into(), image: format!("tilesets/{id}.png"), solid, animations, lights, terrains }, canvas.png())
}

// ------------------------------------------------------------------ shared tile helpers

/// Rows of offset blocks (bricks, sandstone, big stones) that tile seamlessly.
fn blocks(c: &mut Canvas, base: u32, mortar: u32, bw: i32, bh: i32, rng: &mut Rng) {
    let n = TS as i32;
    c.rect(0, 0, n, n, hex(mortar));
    for (row, y) in (0..n).step_by(bh as usize).enumerate() {
        let off = if row % 2 == 0 { 0 } else { bw / 2 };
        for x in (-bw + off..n).step_by(bw as usize) {
            let pal = ramp(mul(base, 0.92 + rng.f() * 0.16));
            for yy in 0..bh - 1 {
                for xx in 0..bw - 1 {
                    let col = if yy == 0 || xx == 0 {
                        pal[3]
                    } else if yy == bh - 2 || xx == bw - 2 || rng.chance(0.06) {
                        pal[1]
                    } else {
                        pal[2]
                    };
                    c.wrap(x + xx, y + yy, hex(col));
                }
            }
        }
    }
}

/// Overlapping roof shingles in rows of 8 px.
fn shingles(c: &mut Canvas, base: u32, edge: bool, rng: &mut Rng) {
    let n = TS as i32;
    for (row, y) in (0..n).step_by(8).enumerate() {
        let off = if row % 2 == 0 { 0 } else { 4 };
        for x in (-8 + off..n).step_by(8) {
            let tone = mul(base, 0.9 + rng.f() * 0.16);
            for yy in 0..8 {
                for xx in 0..8 {
                    let col = if yy == 7 || (yy == 6 && (xx == 0 || xx == 7)) {
                        mul(base, 0.5)
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
    if edge {
        c.rect(0, 24, n, 6, hex(mul(base, 0.6)));
        c.hline(0, 24, n, hex(mul(base, 0.8)));
        c.rect(0, 30, n, 2, hex(mul(base, 0.35)));
    }
}

/// Water with drifting wave crests (`shift` animates).
fn water(c: &mut Canvas, base: u32, crest: u32, shift: i32) {
    let mut wrng = Rng(91);
    c.ground(base, mul(base, 1.1), mul(base, 0.9), &mut wrng, 0.12);
    for (k, x) in [(0, 3), (1, 18), (2, 9), (3, 25)] {
        let y = 4 + k * 8;
        for d in 0..7 {
            let lift = if d == 0 || d == 6 { 1 } else { 0 };
            c.wrap(x + shift + d, y + lift, hex(crest));
            c.wrap(x + shift + d, y + 1 + lift, hex(mul(base, 1.2)));
        }
    }
}

/// A wall tile with a window frame.
fn window(c: &mut Canvas, frame: u32, glass: u32) {
    c.rect(8, 6, 16, 17, hex(mul(frame, 0.6)));
    c.rect(10, 8, 12, 13, hex(glass));
    c.rect(10, 8, 12, 4, hex(mul(glass, 1.2)));
    c.line(12, 18, 16, 10, hex(0xe8f6ff));
    c.vline(15, 8, 13, hex(frame));
    c.hline(10, 14, 12, hex(frame));
    c.rect(7, 22, 18, 2, hex(frame));
}

/// A door filling the lower part of a wall tile.
fn door(c: &mut Canvas, wood: [u32; 4], arch: bool) {
    let r = if arch { 6 } else { 1 };
    c.rrect(6, 3, 20, 29, r, hex(wood[0]));
    c.rrect(8, 5, 16, 27, r - 1, hex(wood[2]));
    for x in [11, 15, 19] {
        c.vline(x, 6, 26, hex(wood[1]));
    }
    c.rect(20, 17, 2, 2, hex(GOLD[2]));
}

/// A tall object drawn on a 32x64 canvas, split into a bottom tile (layer 1) and a top tile
/// (overhead layer), like the example's trees.
fn pair(draw: impl Fn(&mut Canvas)) -> Canvas {
    let mut c = Canvas::new(TS, 2 * TS);
    draw(&mut c);
    c
}

/// Lays tiles out on an 8-column sheet.
fn sheet(rows: u32, tiles: &mut dyn FnMut(&mut Canvas, u32)) -> Canvas {
    let mut c = Canvas::new(8 * TS, rows * TS);
    for i in 0..8 * rows {
        c.at(i % 8 * TS, i / 8 * TS);
        tiles(&mut c, i);
    }
    c
}

type Tileset = (&'static str, Canvas, Vec<u32>, Vec<TileAnim>, Vec<rustpg2d_engine::autotile::Terrain>);

// ------------------------------------------------------------------ auto-tiling terrains

/// One auto-tiling terrain of a library tileset: name, the tile its inside uses (`None` = the
/// block's own full tile), what erasing leaves, terrains lying on it, whether its edge tiles
/// block movement, and how they look.
struct TerrainSpec<'a> {
    name: &'static str,
    inside: Option<u32>,
    base: u32,
    joins: &'static [&'static str],
    solid: bool,
    art: EdgeArt<'a>,
}

/// Appends the terrains' 4×4 edge blocks below a sheet of `rows` rows (two blocks per band of
/// four rows; the existing tiles keep their pixels and indices) and returns the grown sheet,
/// the terrains and their solid tiles.
fn add_terrains(c: Canvas, rows: u32, specs: &[TerrainSpec]) -> (Canvas, Vec<rustpg2d_engine::autotile::Terrain>, Vec<u32>) {
    use rustpg2d_engine::autotile::Terrain;
    let bands = (specs.len() as u32).div_ceil(2);
    let mut big = Canvas::new(8 * TS, (rows + bands * 4) * TS);
    for y in 0..c.img.height as u32 {
        for x in 0..c.img.width as u32 {
            big.img.set_pixel(x, y, c.img.get_pixel(x, y));
        }
    }
    let mut terrains = Vec::new();
    let mut solid = Vec::new();
    for (b, spec) in specs.iter().enumerate() {
        let first = block_start(rows, b as u32);
        edge_block(&mut big, first, 8, &spec.art);
        let mut t = Terrain { name: spec.name.into(), base: Some(spec.base), joins: spec.joins.iter().map(|j| j.to_string()).collect(), ..Default::default() };
        t.fill_from_block(first, 8);
        if let Some(i) = spec.inside {
            t.tiles[15] = Some(i);
        }
        if spec.solid {
            solid.extend((0..16).map(|k| Terrain::block_tile(first, 8, k)));
        }
        terrains.push(t);
    }
    (big, terrains, solid)
}

/// Edge-tile drawers matching tiles of the library tilesets (the sheets' own tile code keeps
/// its random sequence, so these draw look-alikes with their own seeds).
fn desert_sand(c: &mut Canvas, k: usize) {
    c.ground(0xe3c27e, 0xeed294, 0xcfad68, &mut Rng(1100 + k as u64), 0.15);
}
fn hardpan(c: &mut Canvas, k: usize) {
    c.ground(0xc49a62, 0xd2aa72, 0xb08854, &mut Rng(1120 + k as u64), 0.12);
    for (x0, y0, x1, y1) in [(2, 5, 12, 11), (12, 11, 10, 22), (12, 11, 24, 8), (24, 8, 30, 15), (10, 22, 20, 28), (20, 28, 31, 26)] {
        c.line(x0, y0, x1, y1, hex(0x8a6840));
    }
}
fn rock_ground(c: &mut Canvas, k: usize) {
    c.ground(0x7a7682, 0x8a8692, 0x6a6672, &mut Rng(1140 + k as u64), 0.25);
}
fn alpine_grass(c: &mut Canvas, k: usize) {
    let mut rng = Rng(1160 + k as u64);
    c.ground(0x7a9a52, 0x8aaa62, 0x6a8a46, &mut rng, 0.2);
    for _ in 0..8 {
        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
        c.wrap(x, y, hex(0x9a9688));
    }
}
fn mountain_snow(c: &mut Canvas, k: usize) {
    c.ground(0xeef2f8, 0xffffff, 0xd4dcea, &mut Rng(1180 + k as u64), 0.14);
}
fn mountain_dirt(c: &mut Canvas, k: usize) {
    c.ground(0x9a8060, 0xaa9070, 0x8a7050, &mut Rng(1200 + k as u64), 0.22);
}
fn ice(c: &mut Canvas, k: usize) {
    c.ground(0xa8d8ec, 0xc8ecf8, 0x90c4dc, &mut Rng(1220 + k as u64), 0.1);
    c.line(3, 6, 14, 12, hex(0xffffff));
    c.line(18, 20, 29, 26, hex(0xffffff));
    c.line(14, 12, 18, 20, hex(0x80b4cc));
}
fn cobbles(c: &mut Canvas, k: usize) {
    let mut rng = Rng(1240 + k as u64);
    let n = TS as i32;
    c.rect(0, 0, n, n, hex(0x4a4650));
    for y in (0..n).step_by(8) {
        let off = if (y / 8) % 2 == 0 { 0 } else { 4 };
        for x in (-8 + off..n).step_by(8) {
            let tone = mul(0x8a8692, 0.88 + rng.f() * 0.2);
            for yy in 1..7 {
                for xx in 1..7 {
                    let corner = (xx == 1 || xx == 6) && (yy == 1 || yy == 6);
                    if !corner {
                        c.wrap(x + xx, y + yy, hex(if yy == 1 { mul(tone, 1.15) } else if yy == 6 { mul(tone, 0.8) } else { tone }));
                    }
                }
            }
        }
    }
}
fn mosaic(c: &mut Canvas, _: usize) {
    let n = TS as i32;
    c.rect(0, 0, n, n, hex(0xd8d0c0));
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x - 16).abs(), (y - 16).abs());
            if (dx + dy) % 8 == 0 {
                c.px(x, y, hex(0x8a6a4a));
            } else if (dx + dy) % 8 == 4 {
                c.px(x, y, hex(0x3a6a9a));
            }
        }
    }
}
fn dungeon_floor(c: &mut Canvas, k: usize) {
    let mut rng = Rng(1260 + k as u64);
    let n = TS as i32;
    c.rect(0, 0, n, n, hex(0x1a181e));
    for (x, y, w, h) in [(0, 0, 15, 11), (16, 0, 16, 11), (0, 12, 10, 9), (11, 12, 21, 9), (0, 22, 17, 10), (18, 22, 14, 10)] {
        c.shade_box(x + 1, y + 1, w - 1, h - 1, ramp(mul(0x5a5662, 0.9 + rng.f() * 0.15)));
    }
}
fn earth(c: &mut Canvas, k: usize) {
    c.ground(0x4a3a2a, 0x5a4a38, 0x3a2c20, &mut Rng(1280 + k as u64), 0.3);
}
fn pit(c: &mut Canvas, k: usize) {
    let mut rng = Rng(1300 + k as u64);
    let n = TS as i32;
    c.rect(0, 0, n, n, hex(0x050406));
    for _ in 0..5 {
        c.px(rng.range(TS) as i32, rng.range(TS) as i32, hex(0x16141a));
    }
}

// ------------------------------------------------------------------ desert

fn desert() -> Tileset {
    let mut rng = Rng(101);
    let n = TS as i32;
    let sand = (0xe3c27e, 0xeed294, 0xcfad68);
    let palm = pair(|c| {
        // A curved trunk with leaves on top.
        for (k, y) in (14..60).rev().enumerate() {
            let x = 14 + ((k as f32 / 9.0).sin() * 2.0) as i32;
            c.hline(x, y, 5, hex(if y % 4 == 0 { 0x7a5a32 } else { 0x9a7442 }));
            c.px(x, y, hex(0xb8925a));
        }
        for (dx, dy) in [(-13, 6), (13, 6), (-10, -2), (10, -2), (0, -6)] {
            c.stroke(16.0, 14.0, 16.0 + dx as f32, 14.0 + dy as f32, 2.2, hex(0x3d8a38));
            c.stroke(16.0, 13.0, 16.0 + dx as f32 * 0.8, 13.0 + dy as f32 * 0.8, 1.0, hex(0x63ab55));
        }
        c.shade_disc(15.0, 16.0, 2.5, [0x4a2c14, 0x6b4423, 0x8a5a2e, 0xb07e4a]); // coconuts
        c.auto_outline(32, 64, 0x10301a);
        c.shadow(16.0, 60.0, 11.0, 3.0);
    });
    let mut t = |c: &mut Canvas, i: u32| match i {
        0 => c.ground(sand.0, sand.1, sand.2, &mut rng, 0.15),
        1 => {
            c.ground(sand.0, sand.1, sand.2, &mut rng, 0.1);
            for y in [6, 16, 26] {
                for x in 0..n {
                    let yy = y + ((x as f32 / 5.0).sin() * 1.5) as i32;
                    c.wrap(x, yy, hex(0xcfa962));
                    c.wrap(x, yy - 1, hex(0xf3dca4));
                }
            }
        }
        2 => c.ground(0xd2ad66, 0xdcb978, 0xbf9a56, &mut rng, 0.2),
        3 => {
            c.ground(sand.0, sand.1, sand.2, &mut rng, 0.12);
            for _ in 0..6 {
                let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                c.wrap(x, y, hex(0x9a8060));
                c.wrap(x + 1, y, hex(0x9a8060));
                c.wrap(x, y - 1, hex(0xc8b08a));
            }
        }
        4 => {
            c.ground(0xc49a62, 0xd2aa72, 0xb08854, &mut rng, 0.12);
            for (x0, y0, x1, y1) in [(2, 5, 12, 11), (12, 11, 10, 22), (12, 11, 24, 8), (24, 8, 30, 15), (10, 22, 20, 28), (20, 28, 31, 26)] {
                c.line(x0, y0, x1, y1, hex(0x8a6840));
            }
        }
        5 => {
            c.ground(sand.0, sand.1, sand.2, &mut rng, 0.12);
            for _ in 0..7 {
                let x = rng.range(28) as i32 + 2;
                let h = 3 + rng.range(5) as i32;
                for k in 0..h {
                    c.px(x + if k > h / 2 { 1 } else { 0 }, 30 - k, hex(if k > h - 2 { 0xc8c070 } else { 0x9a9050 }));
                }
            }
        }
        6 => {
            for (x, y) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
                c.shade_box(x, y, 16, 16, ramp(if (x + y) % 32 == 0 { 0xd8b880 } else { 0xccac74 }));
            }
        }
        7 => {
            c.ground(0xd8c08e, 0xe4cea0, 0xc8ae7c, &mut rng, 0.15);
            c.hline(0, 0, n, hex(0xb8986a));
            c.hline(0, 31, n, hex(0xb8986a));
        }
        8 => blocks(c, 0xd2a868, 0x9a7444, 16, 8, &mut rng),
        9 => {
            blocks(c, 0xd2a868, 0x9a7444, 16, 8, &mut rng);
            c.rect(0, 0, n, 7, hex(0xe0bc80));
            c.hline(0, 6, n, hex(0x8a6438));
            for x in (2..n).step_by(8) {
                c.rect(x, 0, 4, 3, hex(0xc89a5c)); // crenels
            }
        }
        10..=12 => {
            c.ground(0xdcb27a, 0xe8c28e, 0xc89e68, &mut rng, 0.1);
            c.line(4, 10, 8, 16, hex(0xb08454));
            c.line(24, 20, 27, 26, hex(0xb08454));
            c.rect(0, 29, n, 3, hex(0xb08454));
            if i == 11 {
                c.rrect(10, 7, 12, 14, 5, hex(0x3a2a1e));
                c.rrect(11, 8, 10, 12, 4, hex(0x14100c));
            }
            if i == 12 {
                c.rrect(7, 4, 18, 28, 8, hex(0x5a3a22));
                c.rrect(9, 6, 14, 26, 6, hex(0x8a5a2e));
                c.vline(16, 7, 25, hex(0x5a3a22));
            }
        }
        13 | 20 => {
            c.shade_cyl(12, 6, 8, 25, [0x1f5a2a, 0x2e7a3a, 0x43a04f, 0x7cc56f]);
            c.shade_cyl(4, 12, 6, 9, [0x1f5a2a, 0x2e7a3a, 0x43a04f, 0x7cc56f]);
            c.shade_cyl(22, 9, 6, 10, [0x1f5a2a, 0x2e7a3a, 0x43a04f, 0x7cc56f]);
            c.rect(9, 18, 4, 3, hex(0x2e7a3a));
            c.rect(19, 16, 4, 3, hex(0x2e7a3a));
            for (x, y) in [(13, 9), (17, 14), (14, 21), (6, 15), (24, 12)] {
                c.px(x, y, hex(0xf0f0d0));
            }
            if i == 20 {
                for (x, y, col) in [(15, 4, 0xf06292), (5, 10, 0xf4e04d), (24, 7, 0xf06292)] {
                    c.rect(x, y, 3, 3, hex(col));
                    c.px(x + 1, y + 1, hex(0xfff3b0));
                }
            }
            c.auto_outline(n, n, 0x10301a);
            c.shadow(16.0, 30.5, 10.0, 2.0);
        }
        14 => {
            for _ in 0..40 {
                let a = rng.f() * std::f32::consts::TAU;
                let r = 4.0 + rng.f() * 9.0;
                c.line(16, 22, 16 + (a.cos() * r) as i32, 22 + (a.sin() * r * 0.7) as i32, hex([0x8a6a3a, 0xa8844a, 0x6b5030][rng.range(3) as usize]));
            }
            c.auto_outline(n, n, 0x3a2a14);
            c.shadow(16.0, 29.0, 11.0, 2.5);
        }
        15 => {
            c.shade_ellipse(16.0, 19.0, 13.0, 10.0, [0x6a4a30, 0x9a6a44, 0xc08a58, 0xe0b080]);
            c.line(10, 14, 14, 22, hex(0x6a4a30));
            c.auto_outline(n, n, OUTLINE);
            c.shadow(16.0, 28.5, 14.0, 3.0);
        }
        16 => c.blit_tile(&palm, TS),
        17 => c.blit_tile(&palm, 0),
        18 | 24 | 25 => water(c, 0x2f8ab8, 0x9ad8f0, [0, 4, 8][[18, 24, 25].iter().position(|&k| k == i).unwrap()]),
        19 => {
            c.ground(sand.0, sand.1, sand.2, &mut rng, 0.1);
            let bone = [0xa89c84, 0xcfc4ac, 0xece4d0, 0xffffff];
            c.shade_disc(12.0, 14.0, 5.0, bone); // skull
            c.rect(9, 13, 2, 3, hex(0x3a3024));
            c.rect(14, 13, 2, 3, hex(0x3a3024));
            c.stroke(18.0, 22.0, 28.0, 18.0, 1.2, hex(bone[2]));
            c.stroke(6.0, 26.0, 16.0, 27.0, 1.2, hex(bone[2]));
        }
        21 => {
            c.rect(0, 0, n, n, hex(0x9a3a2a));
            for y in (2..n).step_by(6) {
                for x in 0..n {
                    let col = if (x / 3 + y / 6) % 2 == 0 { 0xe8b860 } else { 0x2a5a8a };
                    c.px(x, y, hex(col));
                    c.px(x, y + 1, hex(mul(col, 0.8)));
                }
            }
        }
        22 => {
            c.ground(0xc8a868, 0xd4b478, 0xb89858, &mut rng, 0.2);
            for k in 0..4 {
                let r = 4.0 + k as f32 * 3.0;
                c.ellipse(16.0, 16.0, r, r * 0.6, Color::new(0.3, 0.2, 0.1, 0.12));
            }
        }
        23 => {
            for (k, y) in (0..n).step_by(8).enumerate() {
                let tone = mul(0xd2a868, 1.0 - k as f32 * 0.06);
                c.shade_box(0, y, n, 8, ramp(tone));
            }
        }
        _ => {}
    };
    let c = sheet(4, &mut t);
    let mut solid = vec![8, 9, 10, 11, 13, 14, 15, 16, 18, 20];
    let oasis_water = |c: &mut Canvas, _: usize| water(c, 0x2f8ab8, 0x9ad8f0, 0);
    let specs = [
        TerrainSpec {
            name: "Oasis",
            inside: Some(18),
            base: 0,
            joins: &[],
            solid: true,
            art: EdgeArt { inside: &oasis_water, outside: &desert_sand, natural: true, band: 6.0, rim_in: Some(0x9ad8f0), rim_w: 1, rim_out: Some(0xb89858) },
        },
        TerrainSpec {
            name: "Hardpan",
            inside: Some(4),
            base: 0,
            joins: &[],
            solid: false,
            art: EdgeArt { inside: &hardpan, outside: &desert_sand, natural: true, band: 6.0, rim_in: None, rim_w: 1, rim_out: None },
        },
    ];
    let (c, terrains, more) = add_terrains(c, 4, &specs);
    solid.extend(more);
    ("desert", c, solid, vec![TileAnim { tile: 18, frames: vec![18, 24, 25, 24], fps: 3.0 }], terrains)
}

// ------------------------------------------------------------------ mountains

fn mountains() -> Tileset {
    let mut rng = Rng(202);
    let n = TS as i32;
    let rock = [0x4a4650, 0x6a6672, 0x8a8692, 0xb4b0bc];
    let snowy_pine = pair(|c| {
        c.shade_cyl(13, 42, 6, 18, WOOD);
        for (k, y) in [(5.0, 4.0), (8.0, 12.0), (11.0, 21.0), (13.5, 30.0), (15.0, 38.0)] {
            pine_tier(c, 16.0, y, k, 10.0);
            // Snow on each tier.
            for x in (16.0 - k) as i32..(16.0 + k) as i32 {
                if (x * 7) % 5 != 0 {
                    c.px(x, y as i32 + 9 - ((x - 16).abs() / 3), hex(0xf4f8ff));
                }
            }
        }
        c.auto_outline(32, 64, 0x082a18);
        c.shadow(16.0, 60.0, 11.0, 3.0);
    });
    let dead_tree = pair(|c| {
        c.shade_cyl(13, 26, 6, 34, [0x2a2018, 0x4a3a2a, 0x6a5440, 0x8a7458]);
        for (x0, y0, x1, y1) in [(16, 34, 5, 18), (16, 30, 27, 14), (8, 24, 4, 12), (24, 20, 29, 9), (16, 26, 16, 6)] {
            c.stroke(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.4, hex(0x5a4a38));
        }
        c.auto_outline(32, 64, 0x1a140e);
        c.shadow(16.0, 60.0, 9.0, 2.5);
    });
    let mut t = |c: &mut Canvas, i: u32| match i {
        0 => c.ground(0x7a7682, 0x8a8692, 0x6a6672, &mut rng, 0.25),
        1 => {
            c.ground(0x8a8478, 0x9a9488, 0x7a7468, &mut rng, 0.2);
            for _ in 0..14 {
                let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                c.wrap(x, y, hex(0x5a5650));
                c.wrap(x + 1, y, hex(0xb0aa9c));
            }
        }
        2 => {
            c.ground(0x7a9a52, 0x8aaa62, 0x6a8a46, &mut rng, 0.2);
            for _ in 0..8 {
                let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                c.wrap(x, y, hex(0x9a9688));
            }
        }
        3 => c.ground(0xeef2f8, 0xffffff, 0xd4dcea, &mut rng, 0.14),
        4 => {
            c.ground(0xeef2f8, 0xffffff, 0xd4dcea, &mut rng, 0.12);
            for (x, y) in [(8, 10), (22, 20), (14, 26)] {
                c.shade_ellipse(x as f32, y as f32, 4.0, 3.0, rock);
                c.hline(x - 3, y - 3, 6, hex(0xffffff));
            }
        }
        5 => {
            c.ground(0xa8d8ec, 0xc8ecf8, 0x90c4dc, &mut rng, 0.1);
            c.line(3, 6, 14, 12, hex(0xffffff));
            c.line(18, 20, 29, 26, hex(0xffffff));
            c.line(14, 12, 18, 20, hex(0x80b4cc));
        }
        6 => c.ground(0x9a8060, 0xaa9070, 0x8a7050, &mut rng, 0.22),
        7 | 23 => {
            for (k, y) in (0..n).step_by(8).enumerate() {
                let base = if i == 7 { mul(0x8a8692, 1.0 - k as f32 * 0.05) } else { 0x9a6b3c };
                c.shade_box(0, y, n, 8, ramp(base));
            }
            if i == 23 {
                c.rect(0, 0, 3, n, hex(WOOD[0]));
                c.rect(29, 0, 3, n, hex(WOOD[0]));
            }
        }
        8 | 10 => {
            // Cliff face: vertical strata.
            c.rect(0, 0, n, n, hex(rock[1]));
            for x in (0..n).step_by(6) {
                let w = 4 + rng.range(3) as i32;
                c.rect(x, 0, w, n, hex(rock[2]));
                c.vline(x, 0, n, hex(rock[3]));
                c.vline(x + w, 0, n, hex(rock[0]));
            }
            for _ in 0..4 {
                let (x, y) = (rng.range(28) as i32, rng.range(28) as i32);
                c.hline(x, y, 4, hex(rock[0]));
            }
            if i == 10 {
                for x in 0..n {
                    let h = 3 + ((x as f32 / 4.0).sin() * 2.0) as i32;
                    c.rect(x, 0, 1, h, hex(0xf4f8ff));
                }
            }
        }
        9 => {
            c.rect(0, 0, n, n, hex(rock[1]));
            for x in (0..n).step_by(6) {
                c.rect(x + 1, 10, 4, 22, hex(rock[2]));
                c.vline(x, 10, 22, hex(rock[0]));
            }
            c.rect(0, 0, n, 11, hex(0x6a8a46));
            for x in 0..n {
                c.px(x, 11 + (x % 3 == 0) as i32, hex(0x4a6a32));
            }
        }
        11 | 12 => {
            c.shade_ellipse(16.0, 18.0, 13.0, 11.0, rock);
            c.line(10, 12, 13, 22, hex(rock[0]));
            if i == 12 {
                c.ellipse(15.0, 10.0, 10.0, 4.0, hex(0xf4f8ff));
                c.hline(9, 9, 10, hex(0xffffff));
            }
            c.auto_outline(n, n, OUTLINE);
            c.shadow(16.0, 29.0, 14.0, 3.0);
        }
        13 => {
            c.rect(0, 0, n, n, hex(rock[1]));
            c.rrect(3, 5, 26, 27, 11, hex(rock[0]));
            c.rrect(5, 7, 22, 25, 10, hex(0x0e0c10));
            for x in [8, 13, 19, 24] {
                c.vline(x, 6, 3, hex(rock[3]));
            }
        }
        14 => {
            for (x, h, w) in [(9, 18, 6), (16, 24, 7), (22, 14, 5)] {
                for row in 0..h {
                    let half = (w as f32 * (1.0 - row as f32 / h as f32 * 0.7)) as i32 / 2;
                    let y = 29 - row;
                    c.hline(x - half, y, half * 2 + 1, hex(if row > h - 4 { 0xd8f4ff } else { 0x5ab4e8 }));
                    c.px(x - half, y, hex(0xa8e0ff));
                }
            }
            c.auto_outline(n, n, 0x123048);
            c.shadow(16.0, 30.0, 12.0, 2.0);
        }
        15 => {
            c.shade_ellipse(16.0, 18.0, 13.0, 11.0, rock);
            for (x, y) in [(10, 14), (18, 12), (14, 21), (21, 19)] {
                c.rect(x, y, 3, 2, hex(GOLD[2]));
                c.px(x, y, hex(GOLD[3]));
            }
            c.auto_outline(n, n, OUTLINE);
            c.shadow(16.0, 29.0, 14.0, 3.0);
        }
        16 => c.blit_tile(&snowy_pine, TS),
        17 => c.blit_tile(&snowy_pine, 0),
        18 => c.blit_tile(&dead_tree, TS),
        19 => c.blit_tile(&dead_tree, 0),
        20 | 24 | 25 => {
            // Waterfall: falling streaks moving down.
            let shift = [0, 5, 10][[20, 24, 25].iter().position(|&k| k == i).unwrap()];
            c.rect(0, 0, n, n, hex(0x3a8ad0));
            for x in (0..n).step_by(3) {
                for y in 0..n {
                    if (y + shift + x * 7) % 9 < 4 {
                        c.px(x, y, hex(0xa8dcff));
                    }
                }
            }
            c.vline(0, 0, n, hex(rock[0]));
            c.vline(31, 0, n, hex(rock[0]));
        }
        21 => {
            c.ground(0x4a6a8a, 0x5a7a9a, 0x3a5a7a, &mut rng, 0.15);
            for (x, y) in [(8, 9), (22, 18), (12, 24)] {
                c.shade_ellipse(x as f32, y as f32, 4.0, 3.0, rock);
            }
        }
        22 | 26 | 27 => {
            let shift = [0, 3, 6][[22, 26, 27].iter().position(|&k| k == i).unwrap()];
            c.rect(0, 0, n, n, hex(0xc8301c));
            for y in 0..n {
                for x in 0..n {
                    let v = ((x as f32 / 5.0 + shift as f32).sin() + (y as f32 / 4.0 - shift as f32 * 0.5).cos()) * 0.5;
                    if v > 0.55 {
                        c.px(x, y, hex(0xffd04a));
                    } else if v > 0.2 {
                        c.px(x, y, hex(0xf07020));
                    }
                }
            }
        }
        _ => {}
    };
    let c = sheet(4, &mut t);
    let mut solid = vec![8, 9, 10, 11, 12, 14, 15, 16, 18, 20, 22];
    let anims = vec![TileAnim { tile: 20, frames: vec![20, 24, 25], fps: 8.0 }, TileAnim { tile: 22, frames: vec![22, 26, 27, 26], fps: 3.0 }];
    let specs = [
        TerrainSpec {
            name: "Snowfield",
            inside: Some(3),
            base: 0,
            joins: &["Frozen lake"],
            solid: false,
            art: EdgeArt { inside: &mountain_snow, outside: &rock_ground, natural: true, band: 6.0, rim_in: Some(0xd4dcea), rim_w: 1, rim_out: Some(0x5a5662) },
        },
        TerrainSpec {
            name: "Mountain path",
            inside: Some(6),
            base: 2,
            joins: &[],
            solid: false,
            art: EdgeArt { inside: &mountain_dirt, outside: &alpine_grass, natural: true, band: 6.0, rim_in: None, rim_w: 1, rim_out: Some(0x5a7a3e) },
        },
        TerrainSpec {
            name: "Frozen lake",
            inside: Some(5),
            base: 3,
            joins: &[],
            solid: false,
            art: EdgeArt { inside: &ice, outside: &mountain_snow, natural: true, band: 5.0, rim_in: Some(0x80b4cc), rim_w: 1, rim_out: None },
        },
    ];
    let (c, terrains, more) = add_terrains(c, 4, &specs);
    solid.extend(more);
    ("mountains", c, solid, anims, terrains)
}

// ------------------------------------------------------------------ city

fn city() -> Tileset {
    let mut rng = Rng(303);
    let n = TS as i32;
    let mut t = |c: &mut Canvas, i: u32| match i {
        0 => {
            // Cobblestones.
            c.rect(0, 0, n, n, hex(0x4a4650));
            for y in (0..n).step_by(8) {
                let off = if (y / 8) % 2 == 0 { 0 } else { 4 };
                for x in (-8 + off..n).step_by(8) {
                    let tone = mul(0x8a8692, 0.88 + rng.f() * 0.2);
                    for yy in 1..7 {
                        for xx in 1..7 {
                            let corner = (xx == 1 || xx == 6) && (yy == 1 || yy == 6);
                            if !corner {
                                c.wrap(x + xx, y + yy, hex(if yy == 1 { mul(tone, 1.15) } else if yy == 6 { mul(tone, 0.8) } else { tone }));
                            }
                        }
                    }
                }
            }
        }
        1 | 4 | 30 => {
            c.ground(0x5a5a62, 0x66666e, 0x4e4e56, &mut rng, 0.2);
            if i == 4 {
                c.shade_disc(16.0, 16.0, 7.0, [0x2a2a30, 0x3a3a42, 0x4a4a54, 0x6a6a74]);
                for k in -4..=4 {
                    c.hline(12, 16 + k, 9, hex(if k % 2 == 0 { 0x2a2a30 } else { 0x4a4a54 }));
                }
            }
            if i == 30 {
                for x in (2..n).step_by(8) {
                    c.rect(x, 0, 5, n, hex(0xe8e8e0));
                }
            }
        }
        2 => {
            for (x, y) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
                c.shade_box(x, y, 16, 16, ramp(0xb8b0a4));
            }
        }
        3 => {
            c.rect(0, 0, n, n, hex(0xd8d0c0));
            for y in 0..n {
                for x in 0..n {
                    let (dx, dy) = ((x - 16).abs(), (y - 16).abs());
                    if (dx + dy) % 8 == 0 {
                        c.px(x, y, hex(0x8a6a4a));
                    } else if (dx + dy) % 8 == 4 {
                        c.px(x, y, hex(0x3a6a9a));
                    }
                }
            }
        }
        5 => {
            c.ground(0x4f9a43, 0x5fae52, 0x438a3a, &mut rng, 0.16);
        }
        6 => {
            c.ground(0x5a3a22, 0x6b4a2a, 0x4a2c18, &mut rng, 0.3);
            for _ in 0..7 {
                let (x, y) = (rng.range(26) as i32 + 3, rng.range(26) as i32 + 3);
                let col = [0xf06292, 0xf4e04d, 0xffffff, 0xb070e0][rng.range(4) as usize];
                c.rect(x - 1, y + 2, 3, 2, hex(0x2e7d32));
                for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                    c.px(x + dx, y + dy, hex(col));
                }
                c.px(x, y, hex(0xfff3b0));
            }
            c.outline_box(0, 0, n, n, 0x8a8692);
        }
        7 => {
            for (k, y) in (0..n).step_by(8).enumerate() {
                c.shade_box(0, y, n, 8, ramp(mul(0xa8a49c, 1.0 - k as f32 * 0.06)));
            }
        }
        8 => blocks(c, 0xa84a3a, 0x6a3a30, 8, 4, &mut rng),
        9 | 11 | 12 | 13 | 14 | 15 => {
            c.ground(0xb8c8d8, 0xc8d8e8, 0xa8b8c8, &mut rng, 0.08);
            c.rect(0, 0, n, 3, hex(0x7a8a9a));
            c.rect(0, 29, n, 3, hex(0x6a7a8a));
            match i {
                11 => {
                    c.rect(3, 6, 26, 20, hex(0x4a3a2a));
                    c.rect(5, 8, 22, 16, hex(0x8ec9ef));
                    c.rect(5, 8, 22, 5, hex(0xb8e2fa));
                    for (x, col) in [(7, 0xd84343), (12, 0xe8c05a), (18, 0x43a047), (23, 0x3a72c8)] {
                        c.rect(x, 18, 3, 6, hex(col)); // goods on display
                    }
                }
                12 => door(c, ramp(0x6a3a5a), true),
                13 => {
                    c.rrect(9, 5, 14, 20, 6, hex(0x5a6a7a));
                    c.rrect(11, 7, 10, 17, 5, hex(0x8ec9ef));
                    c.vline(16, 8, 15, hex(0x5a6a7a));
                }
                14 => {
                    window(c, 0x6a5040, 0x8ec9ef);
                    c.rect(6, 22, 20, 4, hex(0x8a5a2e));
                    for x in (7..25).step_by(3) {
                        c.rect(x, 19, 2, 3, hex([0xf06292, 0xd84343, 0xf4e04d][(x % 3) as usize]));
                        c.px(x, 21, hex(0x2e7d32));
                    }
                }
                15 => {
                    c.vline(16, 6, 6, hex(IRON[1]));
                    c.rrect(12, 11, 9, 11, 2, hex(IRON[1]));
                    c.rect(13, 13, 7, 7, hex(0xffd860));
                    c.px(14, 14, hex(0xffffff));
                    c.ellipse(16.5, 16.0, 9.0, 9.0, Color::new(1.0, 0.85, 0.4, 0.15));
                }
                _ => {}
            }
        }
        10 => blocks(c, 0x8a8a94, 0x55555e, 16, 16, &mut rng),
        16 | 17 => shingles(c, 0x5a6a80, i == 17, &mut rng),
        18 | 19 => shingles(c, 0xc8643a, i == 19, &mut rng),
        20 => {
            shingles(c, 0x5a6a80, false, &mut rng);
            c.shade_box(11, 4, 10, 18, ramp(0xa84a3a));
            c.rect(10, 2, 12, 3, hex(0x6a3a30));
            c.ellipse(16.0, 0.0, 4.0, 2.0, Color::new(0.8, 0.8, 0.8, 0.5));
        }
        21 => {
            c.rect(0, 26, n, 3, hex(IRON[1]));
            c.rect(0, 9, n, 2, hex(IRON[1]));
            for x in (2..n).step_by(5) {
                c.shade_box(x, 6, 2, 23, IRON);
                c.px(x, 4, hex(IRON[3]));
                c.px(x, 5, hex(IRON[2]));
            }
        }
        22 => {
            c.shade_box(0, 8, n, 22, [0x1d5124, 0x2e7d32, 0x43a047, 0x63ab55]);
            for _ in 0..30 {
                let (x, y) = (rng.range(TS) as i32, 9 + rng.range(20) as i32);
                c.px(x, y, hex(if rng.chance(0.5) { 0x5fae52 } else { 0x2e7d32 }));
            }
            c.hline(0, 8, n, hex(0x7cc56f));
        }
        23 | 26 | 27 => water(c, 0x3a6a8a, 0x8ab4d0, [0, 4, 8][[23, 26, 27].iter().position(|&k| k == i).unwrap()]),
        24 => {
            blocks(c, 0x9a9a9e, 0x5a5a60, 16, 8, &mut rng);
            c.rect(0, 26, n, 6, hex(0x3a6a8a));
            c.hline(0, 26, n, hex(0x2a4a60));
        }
        25 => {
            blocks(c, 0xa8a49c, 0x6a665e, 8, 32, &mut rng);
            c.rect(0, 0, n, 3, hex(0x6a665e));
            c.rect(0, 29, n, 3, hex(0x6a665e));
        }
        28 | 29 => {
            let col = if i == 28 { 0xc83a3a } else { 0x3a8a4a };
            for x in 0..n {
                let stripe = (x / 4) % 2 == 0;
                let base = if stripe { col } else { 0xf2efe6 };
                c.rect(x, 4, 1, 18, hex(base));
                c.px(x, 4, hex(mul(base, 1.1)));
                // Scalloped lower edge.
                let drop = 22 + ((x % 4) - 2).abs() / 2;
                c.rect(x, 22, 1, drop - 22 + 2, hex(mul(base, 0.85)));
            }
            c.hline(0, 3, n, hex(0x5a4a3a));
        }
        31 => {
            c.ground(0x5a5a62, 0x66666e, 0x4e4e56, &mut rng, 0.2);
            c.rect(8, 8, 16, 16, hex(0x2a2a30));
            for x in (9..23).step_by(3) {
                c.vline(x, 9, 14, hex(0x6a6a74));
            }
        }
        _ => {}
    };
    let c = sheet(4, &mut t);
    let mut solid = vec![8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24];
    let canal_water = |c: &mut Canvas, _: usize| water(c, 0x3a6a8a, 0x8ab4d0, 0);
    let specs = [
        TerrainSpec {
            name: "Canal",
            inside: Some(23),
            base: 0,
            joins: &[],
            solid: true,
            art: EdgeArt { inside: &canal_water, outside: &cobbles, natural: false, band: 4.0, rim_in: Some(0x24465e), rim_w: 2, rim_out: Some(0xb8b0a4) },
        },
        TerrainSpec {
            name: "Plaza",
            inside: Some(3),
            base: 0,
            joins: &[],
            solid: false,
            art: EdgeArt { inside: &mosaic, outside: &cobbles, natural: false, band: 3.0, rim_in: Some(0x8a6a4a), rim_w: 2, rim_out: Some(0x3a3640) },
        },
    ];
    let (c, terrains, more) = add_terrains(c, 4, &specs);
    solid.extend(more);
    ("city", c, solid, vec![TileAnim { tile: 23, frames: vec![23, 26, 27, 26], fps: 3.0 }], terrains)
}

// ------------------------------------------------------------------ dungeon

fn dungeon() -> Tileset {
    let mut rng = Rng(404);
    let n = TS as i32;
    let floor = |c: &mut Canvas, rng: &mut Rng, tone: u32| {
        c.rect(0, 0, n, n, hex(0x1a181e));
        for (x, y, w, h) in [(0, 0, 15, 11), (16, 0, 16, 11), (0, 12, 10, 9), (11, 12, 21, 9), (0, 22, 17, 10), (18, 22, 14, 10)] {
            c.shade_box(x + 1, y + 1, w - 1, h - 1, ramp(mul(tone, 0.9 + rng.f() * 0.15)));
        }
    };
    let mut t = |c: &mut Canvas, i: u32| match i {
        0 => floor(c, &mut rng, 0x5a5662),
        1 => {
            floor(c, &mut rng, 0x5a5662);
            c.line(4, 4, 12, 14, hex(0x1a181e));
            c.line(12, 14, 9, 26, hex(0x1a181e));
            c.line(12, 14, 24, 18, hex(0x1a181e));
        }
        2 => {
            floor(c, &mut rng, 0x5a5662);
            for _ in 0..40 {
                let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                c.px(x, y, hex(if rng.chance(0.5) { 0x4a7a3a } else { 0x3a6a2e }));
            }
        }
        3 | 23 => {
            floor(c, &mut rng, 0x5a5662);
            let (x, w) = if i == 3 { (0, n) } else { (6, 20) };
            c.rect(x, 0, w, n, hex(0x7a1c1c));
            c.vline(x + 2, 0, n, hex(GOLD[1]));
            c.vline(x + w - 3, 0, n, hex(GOLD[1]));
        }
        4 => c.ground(0x4a3a2a, 0x5a4a38, 0x3a2c20, &mut rng, 0.3),
        5 => {
            floor(c, &mut rng, 0x5a5662);
            c.rect(6, 6, 20, 20, hex(0x0a080c));
            for k in (7..26).step_by(3) {
                c.vline(k, 6, 20, hex(IRON[2]));
                c.hline(6, k, 20, hex(IRON[1]));
            }
        }
        6 => {
            floor(c, &mut rng, 0x5a5662);
            c.shade_box(5, 5, 22, 22, WOOD);
            for x in [10, 16, 22] {
                c.vline(x, 6, 20, hex(WOOD[0]));
            }
            c.rect(14, 14, 4, 4, hex(IRON[2]));
        }
        7 => {
            for (k, y) in (0..n).step_by(6).enumerate() {
                let tone = mul(0x6a6672, 1.0 - k as f32 * 0.15);
                c.shade_box(0, y, n, 6, ramp(tone));
            }
            c.rect(0, 30, n, 2, hex(0x0a080c));
        }
        8 | 10 | 11 | 12 => {
            blocks(c, 0x4a4652, 0x22202a, 16, 8, &mut rng);
            match i {
                10 => {
                    c.shade_box(14, 12, 4, 10, WOOD);
                    c.disc(16.0, 10.0, 3.5, hex(0xffa020));
                    c.disc(16.0, 9.0, 2.0, hex(0xffe060));
                    c.ellipse(16.0, 12.0, 11.0, 11.0, Color::new(1.0, 0.6, 0.2, 0.15));
                }
                11 => {
                    for x in [9, 22] {
                        for y in (4..24).step_by(3) {
                            c.rect(x, y, 2, 2, hex(IRON[2]));
                            c.px(x, y, hex(IRON[3]));
                        }
                        c.rrect(x - 2, 24, 6, 4, 2, hex(IRON[1]));
                    }
                }
                12 => {
                    for _ in 0..50 {
                        let (x, y) = (rng.range(TS) as i32, rng.range(TS) as i32);
                        c.px(x, y, hex(if rng.chance(0.5) { 0x3a6a2e } else { 0x4a7a3a }));
                    }
                }
                _ => {}
            }
        }
        9 => {
            c.rect(0, 0, n, n, hex(0x16141a));
            c.rect(0, 26, n, 6, hex(0x2a2832));
            c.hline(0, 26, n, hex(0x4a4652));
        }
        13 => {
            blocks(c, 0x4a4652, 0x22202a, 16, 8, &mut rng);
            c.rrect(6, 3, 20, 29, 6, hex(IRON[0]));
            c.rrect(8, 5, 16, 27, 5, hex(IRON[2]));
            for y in [9, 17, 25] {
                c.hline(8, y, 16, hex(IRON[1]));
            }
            for (x, y) in [(10, 7), (21, 7), (10, 28), (21, 28)] {
                c.px(x, y, hex(IRON[3]));
            }
            c.rect(19, 17, 3, 3, hex(GOLD[1]));
        }
        14 => {
            floor(c, &mut rng, 0x5a5662);
            c.rect(0, 2, n, 3, hex(IRON[1]));
            c.rect(0, 26, n, 3, hex(IRON[1]));
            for x in (2..n).step_by(5) {
                c.shade_box(x, 2, 3, 27, IRON);
            }
        }
        15 => {
            floor(c, &mut rng, 0x5a5662);
            c.shade_cyl(8, 0, 16, 28, [0x2a2832, 0x4a4652, 0x6a6672, 0x8a8692]);
            c.rect(6, 26, 20, 5, hex(0x4a4652));
            c.hline(6, 26, 20, hex(0x6a6672));
        }
        16 => {
            c.rect(0, 0, n, n, hex(0x050406));
            c.rect(0, 0, n, 4, hex(0x2a2832));
            c.hline(0, 4, n, hex(0x16141a));
        }
        17 | 19 => {
            floor(c, &mut rng, 0x5a5662);
            let bone = [0xa89c84, 0xcfc4ac, 0xece4d0, 0xffffff];
            c.shade_disc(if i == 19 { 16.0 } else { 11.0 }, 15.0, 5.5, bone);
            let sx = if i == 19 { 16 } else { 11 };
            c.rect(sx - 3, 14, 2, 3, hex(0x2a2018));
            c.rect(sx + 1, 14, 2, 3, hex(0x2a2018));
            c.hline(sx - 2, 19, 5, hex(0x2a2018));
            if i == 17 {
                for (x0, y0, x1, y1) in [(16, 24, 28, 20), (4, 27, 14, 25), (18, 10, 27, 12)] {
                    c.stroke(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.2, hex(bone[2]));
                }
            }
        }
        18 => {
            // Cobweb in the top-left corner (overhead decoration).
            let w = Color::new(0.9, 0.9, 0.95, 0.7);
            for a in [0.0f32, 0.4, 0.8, 1.2, 1.57] {
                c.line(0, 0, (a.cos() * 26.0) as i32, (a.sin() * 26.0) as i32, w);
            }
            for r in [7.0f32, 13.0, 19.0, 25.0] {
                let mut prev = (r as i32, 0);
                for k in 1..=8 {
                    let a = k as f32 / 8.0 * 1.57;
                    let p = ((a.cos() * r) as i32, (a.sin() * r) as i32);
                    c.line(prev.0, prev.1, p.0, p.1, w);
                    prev = p;
                }
            }
        }
        20 => {
            floor(c, &mut rng, 0x5a5662);
            for _ in 0..9 {
                let (x, y) = (4 + rng.range(24) as i32, 6 + rng.range(22) as i32);
                c.shade_disc(x as f32, y as f32, 2.0 + rng.f() * 2.5, [0x2a2832, 0x4a4652, 0x6a6672, 0x8a8692]);
            }
        }
        21 => {
            floor(c, &mut rng, 0x5a5662);
            c.ellipse(16.0, 18.0, 11.0, 6.0, Color::new(0.25, 0.35, 0.5, 0.7));
            c.hline(10, 16, 6, hex(0x8ab4d0));
        }
        22 => {
            floor(c, &mut rng, 0x5a5662);
            c.shade_box(3, 8, 26, 20, [0x2a2832, 0x4a4652, 0x6a6672, 0x8a8692]);
            c.rect(3, 8, 26, 4, hex(0x7a1c1c));
            c.rect(8, 4, 3, 5, hex(0xf2efe6));
            c.rect(21, 4, 3, 5, hex(0xf2efe6));
            c.px(9, 3, hex(0xffd860));
            c.px(22, 3, hex(0xffd860));
        }
        _ => {}
    };
    let c = sheet(3, &mut t);
    let mut solid = vec![8, 9, 10, 11, 12, 13, 14, 15, 16, 22];
    let specs = [
        TerrainSpec {
            name: "Earth",
            inside: Some(4),
            base: 0,
            joins: &[],
            solid: false,
            art: EdgeArt { inside: &earth, outside: &dungeon_floor, natural: true, band: 6.0, rim_in: Some(0x2e2318), rim_w: 1, rim_out: None },
        },
        TerrainSpec {
            name: "Chasm",
            inside: None,
            base: 0,
            joins: &[],
            solid: true,
            art: EdgeArt { inside: &pit, outside: &dungeon_floor, natural: true, band: 5.0, rim_in: Some(0x2a2832), rim_w: 2, rim_out: Some(0x6a6672) },
        },
        // A chasm whose depths are see-through: a map's parallax layer behind the tiles (or its
        // background colour) shows in it.
        TerrainSpec {
            name: "Abyss",
            inside: None,
            base: 0,
            joins: &[],
            solid: true,
            art: EdgeArt { inside: &|c: &mut Canvas, _| c.rect(0, 0, TS as i32, TS as i32, CLEAR), outside: &dungeon_floor, natural: true, band: 5.0, rim_in: Some(0x2a2832), rim_w: 2, rim_out: Some(0x6a6672) },
        },
    ];
    let (c, terrains, more) = add_terrains(c, 3, &specs);
    solid.extend(more);
    ("dungeon", c, solid, vec![], terrains)
}

impl Canvas {
    /// 1 px frame around a block.
    fn outline_box(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        self.hline(x, y, w, hex(col));
        self.hline(x, y + h - 1, w, hex(col));
        self.vline(x, y, h, hex(col));
        self.vline(x + w - 1, y, h, hex(col));
    }
}

// ------------------------------------------------------------------ props

/// Cells of the "props2" sheet (8 columns). Multi-cell props list their top-left cell.
mod q {
    pub const LAMP: u32 = 0; // 1x2, off at +1
    pub const CRATE: u32 = 2; // broken +1
    pub const POT: u32 = 4; // broken +1
    pub const CAMPFIRE: u32 = 6; // out +1
    pub const SIGNPOST: u32 = 10;
    pub const TOMBSTONE: u32 = 11;
    pub const LEVER: u32 = 12; // on +1
    pub const ANVIL: u32 = 14;
    pub const HAYSTACK: u32 = 15;
    pub const BONES: u32 = 16;
    pub const TREASURE: u32 = 17;
    pub const TORCH: u32 = 18; // out +1
    pub const CRYSTAL: u32 = 20;
    pub const BENCH: u32 = 21; // 2x1
    pub const SACK: u32 = 23;
    pub const CACTUS: u32 = 24; // 1x2
    pub const PALM: u32 = 25; // 2x3
    pub const STALL: u32 = 27; // 3x2
    pub const FOUNTAIN: u32 = 30; // 2x2
    pub const TENT: u32 = 43; // 2x2
    pub const CART: u32 = 45; // 2x2
}

fn props(x: &mut Extras) {
    let mut c = Canvas::new(TS * 8, TS * 7);
    let at = |c: &mut Canvas, cell: u32| {
        c.at(cell % 8 * TS, cell / 8 * TS);
    };
    // Street lamp, lit and off (1x2).
    for (k, lit) in [(0u32, true), (1, false)] {
        at(&mut c, q::LAMP + k);
        c.shade_box(14, 14, 4, 48, IRON);
        c.shade_box(10, 58, 12, 4, IRON);
        c.rrect(9, 4, 14, 13, 3, hex(IRON[1]));
        c.rect(11, 6, 10, 9, hex(if lit { 0xffe080 } else { 0x4a5058 }));
        if lit {
            c.rect(12, 7, 3, 3, hex(0xffffff));
        }
        c.rect(8, 2, 16, 3, hex(IRON[0]));
        c.auto_outline(32, 64, OUTLINE);
        if lit {
            c.ellipse(16.0, 10.0, 15.0, 12.0, Color::new(1.0, 0.85, 0.4, 0.14));
        }
        c.shadow(16.0, 62.0, 7.0, 1.5);
    }
    // Crate, intact and broken.
    for (k, broken) in [(0u32, false), (1, true)] {
        at(&mut c, q::CRATE + k);
        if broken {
            for (x0, y0, x1, y1) in [(4, 28, 14, 24), (16, 29, 28, 27), (6, 22, 10, 14), (20, 25, 26, 17)] {
                c.stroke(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.5, hex(WOOD[2]));
            }
            c.shade_box(9, 25, 14, 5, WOOD);
        } else {
            c.shade_box(4, 8, 24, 22, WOOD);
            c.outline_box(4, 8, 24, 22, WOOD[0]);
            c.line(5, 9, 26, 28, hex(WOOD[1]));
            c.line(26, 9, 5, 28, hex(WOOD[1]));
            c.hline(5, 18, 22, hex(WOOD[0]));
        }
        c.auto_outline(32, 32, OUTLINE);
        c.shadow(16.0, 30.0, 13.0, 2.0);
    }
    // Clay pot, intact and broken.
    let clay = [0x6a3018, 0x9a4a2a, 0xc0643a, 0xe0905a];
    for (k, broken) in [(0u32, false), (1, true)] {
        at(&mut c, q::POT + k);
        if broken {
            for (px, py, r) in [(10.0f32, 27.0f32, 4.0f32), (19.0, 26.0, 5.0), (24.0, 28.5, 2.5)] {
                c.shade_ellipse(px, py, r, r * 0.6, clay);
            }
        } else {
            c.shade_ellipse(16.0, 20.0, 10.0, 10.0, clay);
            c.shade_box(11, 7, 10, 5, clay);
            c.rect(10, 6, 12, 2, hex(clay[3]));
            c.hline(8, 20, 16, hex(clay[1]));
        }
        c.auto_outline(32, 32, OUTLINE);
        c.shadow(16.0, 30.0, 10.0, 2.0);
    }
    // Campfire, lit and out.
    for (k, lit) in [(0u32, true), (1, false)] {
        at(&mut c, q::CAMPFIRE + k);
        for a in 0..8 {
            let ang = a as f32 / 8.0 * std::f32::consts::TAU;
            c.shade_disc(16.0 + ang.cos() * 10.0, 24.0 + ang.sin() * 4.0, 3.0, [0x3a3a40, 0x5a5a62, 0x7a7a84, 0x9a9aa4]);
        }
        c.stroke(9.0, 26.0, 23.0, 21.0, 1.6, hex(WOOD[1]));
        c.stroke(9.0, 21.0, 23.0, 26.0, 1.6, hex(WOOD[2]));
        if lit {
            for (r, y, col) in [(6.0f32, 17.0f32, 0xf07020), (4.0, 15.0, 0xffa020), (2.5, 14.0, 0xffe060)] {
                c.ellipse(16.0, y, r, r * 1.5, hex(col));
            }
        } else {
            c.ellipse(16.0, 23.0, 5.0, 2.0, hex(0x2a2a2e));
            c.px(15, 20, Color::new(0.7, 0.7, 0.7, 0.5));
            c.px(17, 17, Color::new(0.7, 0.7, 0.7, 0.4));
        }
        c.auto_outline(32, 32, OUTLINE);
        if lit {
            c.ellipse(16.0, 18.0, 15.0, 13.0, Color::new(1.0, 0.6, 0.2, 0.12));
        }
    }
    // Signpost.
    at(&mut c, q::SIGNPOST);
    c.shade_box(14, 6, 4, 24, WOOD);
    for (y, dir) in [(8, 1), (16, -1)] {
        let x0 = if dir > 0 { 16 } else { 4 };
        c.shade_box(x0, y, 12, 6, WOOD);
        let tip = if dir > 0 { x0 + 12 } else { x0 - 1 };
        for k in 0..3 {
            c.vline(tip + k * dir, y + k, 6 - 2 * k, hex(WOOD[2]));
        }
        c.hline(x0 + 2, y + 3, 8, hex(WOOD[0]));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 6.0, 1.5);
    // Tombstone.
    at(&mut c, q::TOMBSTONE);
    c.rrect(8, 6, 16, 23, 7, hex(STONE[2]));
    c.vline(9, 9, 18, hex(STONE[3]));
    c.vline(22, 9, 18, hex(STONE[1]));
    c.rect(15, 11, 2, 9, hex(STONE[1]));
    c.rect(12, 14, 8, 2, hex(STONE[1]));
    c.rect(6, 27, 20, 3, hex(0x4a3a2a));
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 11.0, 2.0);
    // Lever, off and on.
    for (k, on) in [(0u32, false), (1, true)] {
        at(&mut c, q::LEVER + k);
        c.shade_box(8, 20, 16, 9, STONE);
        let (tx, ty) = if on { (24.0, 8.0) } else { (8.0, 8.0) };
        c.stroke(16.0, 22.0, tx, ty, 1.2, hex(IRON[2]));
        c.shade_disc(tx, ty, 2.5, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
        c.auto_outline(32, 32, OUTLINE);
        c.shadow(16.0, 30.0, 9.0, 1.5);
    }
    // Anvil.
    at(&mut c, q::ANVIL);
    c.shade_box(10, 22, 12, 7, WOOD);
    c.shade_box(12, 17, 8, 6, IRON);
    c.shade_box(5, 11, 22, 7, IRON);
    for k in 0..4 {
        c.vline(4 - k, 11 + k / 2, 7 - k, hex(IRON[2]));
    }
    c.hline(6, 11, 20, hex(IRON[3]));
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 12.0, 2.0);
    // Haystack.
    at(&mut c, q::HAYSTACK);
    c.shade_ellipse(16.0, 20.0, 14.0, 11.0, [0x8a6a1a, 0xc0962a, 0xe2bc4a, 0xf6dc80]);
    for k in 0..9 {
        let xx = 6 + k * 2;
        c.line(xx, 14 + k % 3, xx + 2, 26, hex(0xa8822a));
    }
    c.auto_outline(32, 32, 0x3a2a08);
    c.shadow(16.0, 30.0, 14.0, 2.5);
    // Bones.
    at(&mut c, q::BONES);
    let bone = [0xa89c84, 0xcfc4ac, 0xece4d0, 0xffffff];
    c.shade_disc(11.0, 22.0, 5.0, bone);
    c.rect(8, 21, 2, 3, hex(0x2a2018));
    c.rect(12, 21, 2, 3, hex(0x2a2018));
    for (x0, y0, x1, y1) in [(16, 27, 28, 23), (17, 20, 26, 18), (4, 29, 12, 29)] {
        c.stroke(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.2, hex(bone[2]));
    }
    c.auto_outline(32, 32, 0x3a3024);
    // Treasure pile.
    at(&mut c, q::TREASURE);
    c.shade_ellipse(16.0, 24.0, 13.0, 6.0, GOLD);
    for (xx, yy) in [(9, 20), (14, 18), (20, 19), (24, 22), (12, 23), (18, 23)] {
        c.shade_disc(xx as f32, yy as f32, 2.5, GOLD);
    }
    c.rect(18, 14, 4, 5, hex(0xd84343)); // ruby
    c.rect(9, 15, 3, 3, hex(0x43a0d8)); // sapphire
    c.px(19, 15, hex(0xffffff));
    c.auto_outline(32, 32, 0x4a3008);
    c.shadow(16.0, 30.0, 14.0, 2.0);
    // Wall torch, lit and out.
    for (k, lit) in [(0u32, true), (1, false)] {
        at(&mut c, q::TORCH + k);
        c.rect(13, 20, 6, 3, hex(IRON[1]));
        c.shade_box(14, 12, 4, 12, WOOD);
        if lit {
            c.ellipse(16.0, 9.0, 4.0, 6.0, hex(0xf07020));
            c.ellipse(16.0, 9.0, 2.5, 4.0, hex(0xffd040));
        } else {
            c.rect(14, 11, 4, 2, hex(0x2a2a2e));
        }
        c.auto_outline(32, 32, OUTLINE);
        if lit {
            c.ellipse(16.0, 10.0, 14.0, 14.0, Color::new(1.0, 0.6, 0.2, 0.12));
        }
    }
    // Crystal cluster.
    at(&mut c, q::CRYSTAL);
    for (cx, h, w, col) in [(10, 16, 6, 0x8a5ae8), (17, 22, 7, 0xb07af8), (24, 12, 5, 0x8a5ae8)] {
        for row in 0..h {
            let half = (w as f32 * (1.0 - row as f32 / h as f32 * 0.8)) as i32 / 2;
            c.hline(cx - half, 29 - row, half * 2 + 1, hex(if row > h - 4 { 0xe8d8ff } else { col }));
            c.px(cx - half, 29 - row, hex(0xd8c0ff));
        }
    }
    c.auto_outline(32, 32, 0x2a1048);
    c.ellipse(16.0, 20.0, 15.0, 12.0, Color::new(0.7, 0.5, 1.0, 0.1));
    c.shadow(16.0, 30.0, 12.0, 2.0);
    // Bench (2x1).
    at(&mut c, q::BENCH);
    c.shade_box(4, 14, 56, 5, WOOD);
    c.shade_box(4, 8, 56, 4, WOOD);
    for xx in [8, 52] {
        c.shade_box(xx, 19, 4, 10, IRON);
    }
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 28.0, 2.0);
    // Sack.
    at(&mut c, q::SACK);
    c.shade_ellipse(16.0, 21.0, 10.0, 9.0, [0x7a6440, 0xa88c5a, 0xc8ac78, 0xe4cc9c]);
    c.rect(13, 9, 6, 5, hex(0xa88c5a));
    c.hline(12, 12, 8, hex(0x6a5030));
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 10.0, 2.0);
    // Big cactus (1x2).
    at(&mut c, q::CACTUS);
    let green = [0x1f5a2a, 0x2e7a3a, 0x43a04f, 0x7cc56f];
    c.shade_cyl(12, 8, 9, 54, green);
    c.shade_cyl(3, 22, 6, 14, green);
    c.shade_box(6, 33, 7, 4, green);
    c.shade_cyl(23, 16, 6, 16, green);
    c.shade_box(19, 29, 6, 4, green);
    for yy in (12..60).step_by(6) {
        c.px(14, yy, hex(0xf0f0d0));
        c.px(18, yy + 3, hex(0xf0f0d0));
    }
    c.auto_outline(32, 64, 0x10301a);
    c.shadow(16.0, 62.0, 9.0, 2.0);
    // Palm tree (2x3).
    at(&mut c, q::PALM);
    for (k, yy) in (24..94).rev().enumerate() {
        let xx = 29 + ((k as f32 / 14.0).sin() * 4.0) as i32;
        c.hline(xx, yy, 7, hex(if yy % 5 == 0 { 0x7a5a32 } else { 0x9a7442 }));
        c.px(xx, yy, hex(0xb8925a));
    }
    for (dx, dy) in [(-26, 12), (26, 12), (-20, -4), (20, -4), (0, -14), (-12, 18), (12, 18)] {
        c.stroke(32.0, 26.0, 32.0 + dx as f32, 26.0 + dy as f32, 3.0, hex(0x2e7d32));
        c.stroke(32.0, 25.0, 32.0 + dx as f32 * 0.85, 25.0 + dy as f32 * 0.85, 1.4, hex(0x63ab55));
    }
    for (xx, yy) in [(29.0, 30.0), (35.0, 30.0), (32.0, 33.0)] {
        c.shade_disc(xx, yy, 3.0, [0x4a2c14, 0x6b4423, 0x8a5a2e, 0xb07e4a]);
    }
    c.auto_outline(64, 96, 0x10301a);
    c.shadow(32.0, 93.0, 14.0, 3.0);
    // Market stall (3x2).
    at(&mut c, q::STALL);
    c.shade_box(6, 30, 84, 28, WOOD); // counter
    c.hline(6, 40, 84, hex(WOOD[0]));
    for (xx, col) in [(14, 0xd84343), (24, 0xe8c05a), (34, 0x43a047), (50, 0xff9f43), (62, 0xd84343), (74, 0x8a5ae8)] {
        c.shade_disc(xx as f32, 28.0, 4.0, ramp(col)); // goods
    }
    for xx in [6, 86] {
        c.shade_box(xx, 8, 4, 50, WOOD);
    }
    for xx in 0..96 {
        let base = if (xx / 8) % 2 == 0 { 0xc83a3a } else { 0xf2efe6 };
        c.vline(xx, 2, 12, hex(base));
        c.px(xx, 2, hex(mul(base, 1.1)));
        c.vline(xx, 14, 2 + ((xx % 8) - 4).abs() / 2, hex(mul(base, 0.85)));
    }
    c.auto_outline(96, 64, OUTLINE);
    c.shadow(48.0, 60.0, 44.0, 3.0);
    // Fountain (2x2).
    at(&mut c, q::FOUNTAIN);
    c.shade_ellipse(32.0, 46.0, 28.0, 14.0, STONE);
    c.ellipse(32.0, 44.0, 22.0, 9.0, hex(0x3a72c8));
    c.hline(20, 42, 10, hex(0x9ccbf5));
    c.hline(36, 46, 8, hex(0x9ccbf5));
    c.shade_box(28, 18, 8, 26, STONE);
    c.shade_ellipse(32.0, 18.0, 10.0, 4.0, STONE);
    for (xx, yy) in [(26, 22), (38, 22), (24, 28), (40, 28), (32, 12)] {
        c.rect(xx, yy, 2, 4, hex(0x9ccbf5));
    }
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 60.0, 28.0, 3.0);
    // Tent (2x2).
    at(&mut c, q::TENT);
    for row in 0..44 {
        let half = 4 + row * 26 / 44;
        let yy = 14 + row;
        for xx in 32 - half..=32 + half {
            let col = if xx < 32 { 0xe8dcc0 } else { 0xc8b898 };
            c.px(xx, yy, hex(if (xx - 32).abs() < 1 { 0x8a7a5a } else { col }));
        }
    }
    for row in 20..44 {
        let half = (row - 20) * 8 / 24;
        c.hline(32 - half, 14 + row, half * 2 + 1, hex(0x3a2a1e)); // entrance
    }
    c.stroke(32.0, 14.0, 32.0, 4.0, 1.0, hex(WOOD[1]));
    c.rect(33, 4, 8, 5, hex(0xd84343)); // flag
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 59.0, 30.0, 3.0);
    // Cart (2x2).
    at(&mut c, q::CART);
    c.shade_box(6, 22, 48, 22, WOOD);
    for xx in (8..54).step_by(8) {
        c.vline(xx, 23, 20, hex(WOOD[1]));
    }
    c.stroke(54.0, 38.0, 63.0, 42.0, 1.5, hex(WOOD[2])); // shaft
    for xx in [16.0f32, 44.0] {
        c.shade_disc(xx, 48.0, 10.0, WOOD);
        c.disc(xx, 48.0, 7.0, hex(WOOD[1]));
        for a in 0..4 {
            let ang = a as f32 * 0.785;
            c.line((xx - ang.cos() * 7.0) as i32, (48.0 - ang.sin() * 7.0) as i32, (xx + ang.cos() * 7.0) as i32, (48.0 + ang.sin() * 7.0) as i32, hex(WOOD[0]));
        }
        c.shade_disc(xx, 48.0, 2.0, IRON);
    }
    c.shade_ellipse(30.0, 20.0, 18.0, 6.0, [0x8a6a1a, 0xc0962a, 0xe2bc4a, 0xf6dc80]); // hay load
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 60.0, 30.0, 3.0);

    let image = "sprites/props2.png".to_string();
    x.files.insert(image.clone(), c.png());
    x.db.sheets.push(SheetDef::new("props2", &image, TS, TS));

    let sp = |cell: u32| SpriteRef::new("props2", cell);
    let block = |cell: u32, w: u32, h: u32| SpriteRef::block("props2", cell, w, h);
    let st = |name: &str, sprite: SpriteRef, solid: bool| PropState { name: name.into(), sprite, solid, light: None };
    let prop = |id: &str, name: &str, sprite: SpriteRef, solid: bool, hitbox: [f32; 2]| PropDef {
        id: id.into(),
        name: name.into(),
        sprite,
        solid,
        hitbox,
        triggers: vec![],
        container: None,
        lock: None,
        default_state: default_state_name(),
        states: vec![],
        light: None,
        use_spot: None,
        gather: None,
        regrow: None,
        plot: false,
    };
    let with = |mut d: PropDef, default: &str, states: Vec<PropState>| {
        d.default_state = default.into();
        d.states = states;
        d
    };
    x.db.props.extend([
        with(prop("street_lamp", "Street Lamp", block(q::LAMP, 1, 2), true, [10.0, 8.0]), "lit", vec![st("off", block(q::LAMP + 1, 1, 2), true)]),
        with(prop("crate", "Crate", sp(q::CRATE), true, [26.0, 18.0]), "intact", vec![st("broken", sp(q::CRATE + 1), false)]),
        with(prop("pot", "Clay Pot", sp(q::POT), true, [18.0, 12.0]), "intact", vec![st("broken", sp(q::POT + 1), false)]),
        with(prop("campfire", "Campfire", sp(q::CAMPFIRE), true, [22.0, 12.0]), "lit", vec![st("out", sp(q::CAMPFIRE + 1), true)]),
        prop("signpost", "Signpost", sp(q::SIGNPOST), true, [10.0, 8.0]),
        prop("tombstone", "Tombstone", sp(q::TOMBSTONE), true, [20.0, 10.0]),
        with(prop("lever", "Lever", sp(q::LEVER), true, [14.0, 10.0]), "off", vec![st("on", sp(q::LEVER + 1), true)]),
        prop("anvil", "Anvil", sp(q::ANVIL), true, [24.0, 12.0]),
        prop("haystack", "Haystack", sp(q::HAYSTACK), true, [28.0, 14.0]),
        prop("bones", "Bones", sp(q::BONES), false, [20.0, 8.0]),
        prop("treasure", "Treasure Pile", sp(q::TREASURE), true, [24.0, 10.0]),
        with(prop("wall_torch", "Wall Torch", sp(q::TORCH), false, [12.0, 8.0]), "lit", vec![st("out", sp(q::TORCH + 1), false)]),
        prop("crystal", "Crystal Cluster", sp(q::CRYSTAL), true, [18.0, 10.0]),
        prop("bench", "Bench", block(q::BENCH, 2, 1), true, [56.0, 12.0]),
        prop("sack", "Sack", sp(q::SACK), true, [18.0, 10.0]),
        prop("cactus_big", "Big Cactus", block(q::CACTUS, 1, 2), true, [16.0, 10.0]),
        prop("palm_tree", "Palm Tree", block(q::PALM, 2, 3), true, [16.0, 12.0]),
        prop("market_stall", "Market Stall", block(q::STALL, 3, 2), true, [90.0, 20.0]),
        prop("fountain", "Fountain", block(q::FOUNTAIN, 2, 2), true, [56.0, 28.0]),
        prop("tent", "Tent", block(q::TENT, 2, 2), true, [56.0, 24.0]),
        prop("cart", "Cart", block(q::CART, 2, 2), true, [52.0, 20.0]),
    ]);
}

// ------------------------------------------------------------------ characters

fn person_sheet(look: &Look) -> Canvas {
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            draw_person(&mut c, look, f, frame);
            c.auto_outline(FW as i32, FH as i32, OUTLINE);
            c.shadow(16.0, 45.5, 10.0, 2.5);
        }
    }
    c
}

/// A flapping bat (32x32 frames).
fn bat_sheet() -> Canvas {
    let mut c = Canvas::new(32 * 4, 32 * 4);
    let body = [0x2a1e30, 0x46324e, 0x644870, 0x8a6a96];
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * 32, f as u32 * 32);
            let flap = [0, 4, 7, 4][frame as usize];
            let y = 12 + [0, -1, 0, 1][frame as usize];
            for side in [-1i32, 1] {
                for k in 0..11 {
                    let xx = 16 + side * (4 + k);
                    let top = y - 2 - flap + (k * flap) / 10 + (k / 3);
                    let bottom = y + 4 - (k % 3);
                    c.vline(xx, top.min(bottom), (bottom - top).abs() + 1, hex(if k % 3 == 0 { body[0] } else { body[1] }));
                }
            }
            c.shade_ellipse(16.0, y as f32 + 1.0, 5.0, 6.0, body);
            c.rect(13, y - 6, 2, 3, hex(body[1])); // ears
            c.rect(18, y - 6, 2, 3, hex(body[1]));
            if f != Facing::Up {
                let ex = match f {
                    Facing::Left => (13, 15),
                    Facing::Right => (17, 19),
                    _ => (14, 18),
                };
                c.px(ex.0, y - 1, hex(0xff5040));
                c.px(ex.1, y - 1, hex(0xff5040));
            }
            c.auto_outline(32, 32, 0x0e0812);
            c.shadow(16.0, 29.0, 6.0, 1.5);
        }
    }
    c
}

/// A floating ghost (32x48 frames, like people).
fn ghost_sheet() -> Canvas {
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            let bob = [0, -1, -2, -1][frame as usize];
            let top = 8 + bob;
            let col = Color::new(0.92, 0.95, 1.0, 0.85);
            c.ellipse(16.0, top as f32 + 11.0, 11.0, 11.0, col);
            c.rect(5, top + 11, 23, 18, col);
            for k in 0..5 {
                let xx = 5 + k * 5;
                let wave = ((k + frame as i32) % 2) * 2;
                c.rect(xx, top + 29, 4, 3 + wave, col);
            }
            c.rect(6, top + 6, 3, 14, Color::new(1.0, 1.0, 1.0, 0.5));
            if f != Facing::Up {
                let (ex1, ex2) = match f {
                    Facing::Left => (9, 15),
                    Facing::Right => (17, 23),
                    _ => (11, 19),
                };
                for ex in [ex1, ex2] {
                    c.rect(ex, top + 9, 3, 5, hex(0x1a1a2a));
                }
                if f == Facing::Down {
                    c.ellipse(16.0, top as f32 + 19.0, 3.0, 2.0, hex(0x1a1a2a));
                }
            }
            c.auto_outline(FW as i32, FH as i32, 0x6a7088);
            c.shadow(16.0, 45.5, 9.0, 2.0);
        }
    }
    c
}

fn characters(x: &mut Extras) {
    // Every humanoid type as a male and a female version. (The example game's hero, elder,
    // villager and goblin are male; their female versions are here.)
    let people: Vec<(&str, &str, Look)> = vec![
        ("hero_f", "Heroine", Look { skin: 0xf2c49b, hair: 0x7a4a22, shirt: 0x2e6fdb, pants: 0x3b3b4f, female: true, ..Default::default() }),
        ("elder_f", "Elder (female)", Look { skin: 0xe8b88a, hair: 0xdedede, shirt: 0x7b4fa0, pants: 0x4a3a5a, robe: Some(0x7b4fa0), female: true, ..Default::default() }),
        ("villager_f", "Villager (female)", Look { skin: 0xc68a5e, hair: 0xc0392b, shirt: 0x2e9d5b, pants: 0x6b4a2a, robe: Some(0x2e9d5b), female: true, ..Default::default() }),
        ("goblin_f", "Goblin Scout", Look { skin: 0x8bc34a, hair: 0x2a4a14, shirt: 0x5d4037, pants: 0x3e2723, ears: true, female: true, hair_style: HairStyle::Ponytail, ..Default::default() }),
        ("merchant", "Merchant", Look { skin: 0xf2c49b, hair: 0x6b4a2a, shirt: 0x7a3a8a, pants: 0x4a3a2a, apron: Some(0xe8dcc0), hat: Hat::Cap(0x2e6b4f), ..Default::default() }),
        ("merchant_f", "Merchant (female)", Look { skin: 0xe8b88a, hair: 0x3a2418, shirt: 0x7a3a8a, pants: 0x4a3a2a, robe: Some(0x6a3a7a), apron: Some(0xe8dcc0), female: true, ..Default::default() }),
        ("guard", "Town Guard", Look { skin: 0xe8b88a, hair: 0x3a2a1e, shirt: 0x2b4c8c, pants: 0x3a3a44, hat: Hat::Helmet, ..Default::default() }),
        ("guard_f", "Town Guard (female)", Look { skin: 0xd8a070, hair: 0xb06a2a, shirt: 0x2b4c8c, pants: 0x3a3a44, hat: Hat::Helmet, female: true, hair_style: HairStyle::Ponytail, ..Default::default() }),
        ("wizard", "Wizard", Look { skin: 0xe8c0a0, hair: 0xe8e8f0, shirt: 0x3b3b9e, pants: 0x2a2a6a, beard: true, robe: Some(0x3b3b9e), hat: Hat::Wizard(0x3b3b9e), ..Default::default() }),
        ("wizard_f", "Sorceress", Look { skin: 0xf2d0b8, hair: 0x2a1a3a, shirt: 0x6a2a8e, pants: 0x3a1a5a, robe: Some(0x6a2a8e), hat: Hat::Wizard(0x6a2a8e), female: true, ..Default::default() }),
        ("farmer", "Farmer", Look { skin: 0xd9a066, hair: 0x8a5a2e, shirt: 0xc9a25a, pants: 0x3f5f8f, hat: Hat::Straw(0xe2c46a), ..Default::default() }),
        ("farmer_f", "Farmer (female)", Look { skin: 0xe0a878, hair: 0xd8a040, shirt: 0xe8dcc0, pants: 0x3f5f8f, robe: Some(0x3f5f8f), apron: Some(0xe8dcc0), hat: Hat::Straw(0xe2c46a), female: true, hair_style: HairStyle::Ponytail, ..Default::default() }),
        ("nomad", "Desert Nomad", Look { skin: 0xb5794c, hair: 0x2a1e14, shirt: 0xe8d8b0, pants: 0xc9b28a, hat: Hat::Wrap(0xf2ead2), veil: Some(0xd8c49a), ..Default::default() }),
        ("nomad_f", "Desert Nomad (female)", Look { skin: 0xb5794c, hair: 0x1a120c, shirt: 0xc8643a, pants: 0xc9b28a, robe: Some(0xc8643a), hat: Hat::Wrap(0x2a6a8a), veil: Some(0x2a6a8a), female: true, ..Default::default() }),
        ("blacksmith", "Blacksmith", Look { skin: 0xc68a5e, hair: 0x2a2018, shirt: 0x6b5040, pants: 0x3a3020, beard: true, beard_style: BeardStyle::Goatee, apron: Some(0x4a3424), ..Default::default() }),
        ("blacksmith_f", "Blacksmith (female)", Look { skin: 0xd09068, hair: 0x5a2a14, shirt: 0x6b5040, pants: 0x3a3020, apron: Some(0x4a3424), female: true, ..Default::default() }),
        ("noble_m", "Nobleman", Look { skin: 0xf6d0b0, hair: 0x6a4020, shirt: 0xb0306a, pants: 0x3a2a4a, hat: Hat::Tiara, ..Default::default() }),
        ("noble", "Noble Lady", Look { skin: 0xf6d0b0, hair: 0xe8c060, shirt: 0xb0306a, pants: 0xb0306a, robe: Some(0xb0306a), hat: Hat::Tiara, female: true, ..Default::default() }),
        ("bandit", "Bandit", Look { skin: 0xd8a070, hair: 0x2a1e14, shirt: 0x4a4a42, pants: 0x2e2a26, hat: Hat::Hood(0x5a3a2a), veil: Some(0x2a2622), ..Default::default() }),
        ("bandit_f", "Bandit (female)", Look { skin: 0xe8b88a, hair: 0x8a2a1a, shirt: 0x3a4a3a, pants: 0x2e2a26, hat: Hat::Hood(0x3a4a2a), veil: Some(0x2a2622), female: true, ..Default::default() }),
        ("skeleton", "Skeleton", Look { skin: 0xe8e2d0, hair: 0xe8e2d0, shirt: 0x2b2620, pants: 0xd8d2c0, skeleton: true, ..Default::default() }),
        ("skeleton_f", "Skeleton (female)", Look { skin: 0xe8e2d0, hair: 0x6a6458, shirt: 0x2b2620, pants: 0xd8d2c0, skeleton: true, female: true, ..Default::default() }),
    ];
    // Skeletons are whole bodies (bones, no hair or clothes); everyone else is a layered model:
    // a shared body and hair in their colors, wearing an outfit (one clothing item per
    // character, in both cuts) and a helmet if any.
    let layered: Vec<&(&str, &str, Look)> = people.iter().filter(|p| !p.2.skeleton).collect();
    // (id, canvas, frame height, kind, female, playable): people's bodies are playable,
    // monsters' (skeletons, bat, ghost) are not.
    let mut sheets: Vec<(String, Canvas, u32, SheetKind, bool, bool)> = people.iter().filter(|p| p.2.skeleton).map(|(id, _, look)| (id.to_string(), person_sheet(look), FH, SheetKind::Body, look.female, false)).collect();
    sheets.push(("bat".into(), bat_sheet(), 32, SheetKind::Body, false, false));
    sheets.push(("ghost".into(), ghost_sheet(), FH, SheetKind::Body, false, false));
    sheets.extend(shared_layers().into_iter().map(|(id, c, k, f)| (id, c, FH, k, f, k == SheetKind::Body)));
    for (id, _, look) in &layered {
        for (sheet, canvas) in outfit_cuts(id, look) {
            sheets.push((sheet, canvas, FH, SheetKind::Outfit, false, false));
        }
    }
    let icons = outfit_icons(&layered.iter().map(|p| p.2.clone()).collect::<Vec<_>>());
    sheets.push(("library_outfit_icons".into(), icons, 32, SheetKind::Sprite, false, false));
    for (id, canvas, fh, kind, female, playable) in sheets {
        let image = format!("sprites/{id}.png");
        x.files.insert(image.clone(), canvas.png());
        x.db.sheets.push(SheetDef { kind, female, playable, ..SheetDef::new(&id, &image, FW, fh) });
    }
    for (k, (id, name, _)) in layered.iter().enumerate() {
        x.db.items.push(clothing_item(&format!("outfit_{id}"), &format!("{name} Outfit"), &format!("The clothes of the {name}."), "library_outfit_icons", k as u32, id, 10));
    }
    let ch = |id: &str, name: &str, speed: f32, behavior: Behavior| CharacterDef {
        id: id.into(),
        name: name.into(),
        sheet: String::new(),
        speed,
        hitbox: [20.0, 12.0],
        behavior,
        max_hp: 0,
        damage: 0,
        hostile: false,
        weapon: None,
        defense: 0,
        xp: 0,
        touch_effect: None,
        description: String::new(),
        triggers: vec![],
        shop: None,
        loot: vec![],
        light: None,
        appearance: Appearance { body: id.into(), ..Default::default() },
        equipment: Default::default(),
        inventory: Vec::new(),
        gear_locked: false,
        portrait: Default::default(),
        moods: Default::default(),
    };
    let foe = |mut d: CharacterDef, hp: i32, damage: i32, xp: u32| {
        d.description = match d.id.as_str() {
            "bandit" | "bandit_f" => "Highway robbers who chase travellers through the woods and over the roads.",
            "skeleton" | "skeleton_f" => "Restless bones from old graves. Their cold touch slows you down.",
            "goblin_f" => "Quicker than the archers, goblin scouts roam the wilds in small bands, bows ready.",
            "bat" => "Fast, erratic and always in a swarm. Weak alone, a nuisance in caves.",
            "ghost" => "A pale shape drifting through ruins at night. It passes through what you can't.",
            _ => "",
        }
        .into();
        d.max_hp = hp;
        d.damage = damage;
        d.hostile = true;
        d.xp = xp;
        d
    };
    for (id, name, _) in &people {
        let d = match *id {
            "bandit" | "bandit_f" => foe(ch(id, name, 90.0, Behavior::chase(6.0)), 4, 2, 20),
            "skeleton" | "skeleton_f" => {
                // A skeleton's cold touch slows for a few seconds.
                let mut d = foe(ch(id, name, 70.0, Behavior::chase(5.0)), 3, 2, 20);
                d.touch_effect = Some(rustpg2d_engine::effects::EffectSpec::new(rustpg2d_engine::effects::EffectKind::Slow, 40.0, 3.0));
                d
            }
            "goblin_f" => {
                // Like the example's goblin archer.
                let mut d = foe(ch(id, name, 76.0, Behavior::chase(7.0)), 2, 1, 12);
                d.weapon = Some(Weapon::ranged(2, 1.6, Ranged { projectile: SpriteRef::new("items", it::ARROW_SHOT), speed: 300.0, range: 6.0, ammo: String::new() }));
                d
            }
            "hero_f" => {
                let mut d = ch(id, name, 140.0, Behavior::Static);
                d.max_hp = 12;
                d.damage = 1;
                d
            }
            "farmer" | "farmer_f" | "noble" | "noble_m" | "nomad" | "nomad_f" | "villager_f" => ch(id, name, 70.0, Behavior::Wander { radius: 3.0 }),
            _ => ch(id, name, 70.0, Behavior::Static),
        };
        let mut d = d;
        if let Some((_, _, look)) = layered.iter().find(|p| p.0 == *id) {
            d.appearance = appearance_of(look);
            *d.equipment.slot_mut(EquipSlot::Clothing) = Some(rustpg2d_engine::inventory::ItemStack::new(&format!("outfit_{id}"), 1));
            if matches!(look.hat, Hat::Helmet) {
                *d.equipment.slot_mut(EquipSlot::Head) = Some(rustpg2d_engine::inventory::ItemStack::new("iron_helmet", 1));
            }
        }
        x.db.characters.push(d);
    }
    let mut bat = foe(ch("bat", "Bat", 130.0, Behavior::chase(7.0)), 1, 1, 6);
    bat.hitbox = [14.0, 8.0];
    x.db.characters.push(bat);
    x.db.characters.push(foe(ch("ghost", "Ghost", 60.0, Behavior::chase(6.0)), 3, 2, 15));
}
