//! The village props (`sprites/village.png`): market stalls, the tavern's, bakery's, smithy's,
//! apothecary's and chapel's furnishings, farm and pond things, signs and night-blooming herbs.
//! Every sprite is listed once in `SPRITES` (name, size in tiles, drawing function); the sheet's
//! layout is packed from that list, so definitions look cells up by name (`cell`).

use super::*;

/// Sheet id and columns.
pub const SHEET: &str = "village";
const COLS: u32 = 8;

const CLOTH_RED: [u32; 4] = [0x6e1a1a, 0xa82a2a, 0xd04040, 0xf07a6a];
const CLOTH_GREEN: [u32; 4] = [0x1b4a24, 0x2e7a3a, 0x43a04f, 0x8ad08a];
const CLOTH_BLUE: [u32; 4] = [0x1a2e6e, 0x2a4aa8, 0x4070d0, 0x8aaef0];
const CREAM: [u32; 4] = [0xa89c80, 0xccc0a4, 0xe8dec4, 0xfffaf0];
const BREAD: [u32; 4] = [0x7a4a18, 0xb87a34, 0xd8a056, 0xf0c888];
const BRICK: [u32; 4] = [0x5a2418, 0x8a3a28, 0xb05a40, 0xd08060];
const STRAW: [u32; 4] = [0x8a6e2a, 0xa88a3c, 0xc4a44e, 0xe0c870];
const BURLAP: [u32; 4] = [0x6a5234, 0x9a805a, 0xbca47c, 0xdcc8a0];
const FISH: [u32; 4] = [0x3a4a5a, 0x6a7e92, 0x9cb0c0, 0xdce6ee];
const BRONZE: [u32; 4] = [0x5a3a10, 0x8a6020, 0xb88a34, 0xe8c070];
const CLAY: [u32; 4] = [0x6a3018, 0x9a4a2a, 0xc0643a, 0xe0905a];
const FLAME: [u32; 3] = [0xe8590c, 0xffa020, 0xffe680];

type Draw = fn(&mut Canvas);

/// Every sprite of the sheet: name, width and height in tiles, how it is drawn (at the
/// canvas origin, `w * 32` x `h * 32` pixels).
const SPRITES: &[(&str, u32, u32, Draw)] = &[
    ("stall_produce", 3, 3, |c| stall(c, CLOTH_RED, Goods::Produce)),
    ("stall_fish", 3, 3, |c| stall(c, CLOTH_BLUE, Goods::Fish)),
    ("stall_bread", 3, 3, |c| stall(c, CLOTH_GREEN, Goods::Bread)),
    ("windmill", 4, 5, windmill),
    ("watchtower", 2, 4, watchtower),
    ("apple_tree", 2, 3, |c| apple_tree(c, true)),
    ("apple_tree_picked", 2, 3, |c| apple_tree(c, false)),
    ("bell_tower", 2, 3, bell_tower),
    ("oven", 2, 2, |c| oven(c, true)),
    ("oven_out", 2, 2, |c| oven(c, false)),
    ("hearth", 2, 2, |c| fireplace(c, true, true)),
    ("hearth_out", 2, 2, |c| fireplace(c, false, true)),
    ("fireplace", 2, 2, |c| fireplace(c, true, false)),
    ("fireplace_out", 2, 2, |c| fireplace(c, false, false)),
    ("forge", 2, 2, |c| forge(c, true)),
    ("forge_out", 2, 2, |c| forge(c, false)),
    ("bread_shelf", 2, 2, bread_shelf),
    ("potion_shelf", 2, 2, potion_shelf),
    ("tool_rack", 2, 2, tool_rack),
    ("weapon_rack", 2, 2, weapon_rack),
    ("chicken_coop", 2, 2, chicken_coop),
    ("broken_cart", 2, 2, broken_cart),
    ("notice_board", 2, 2, notice_board),
    ("fish_rack", 2, 2, fish_rack),
    ("laundry", 2, 2, laundry),
    ("millstone", 2, 2, millstone),
    ("herb_rack", 1, 2, herb_rack),
    ("scarecrow", 1, 2, scarecrow),
    ("keg_rack", 2, 1, keg_rack),
    ("tavern_table", 2, 1, tavern_table),
    ("dough_table", 2, 1, dough_table),
    ("trough", 2, 1, trough),
    ("boat", 2, 1, boat),
    ("altar", 2, 1, altar),
    ("pew", 2, 1, |c| pew(c, false)),
    ("pew_back", 2, 1, |c| pew(c, true)),
    ("woodpile", 2, 1, woodpile),
    ("flour_sacks", 1, 1, flour_sacks),
    ("keg", 1, 1, keg),
    ("stool", 1, 1, stool),
    ("bellows", 1, 1, bellows),
    ("grindstone", 1, 1, grindstone),
    ("cauldron", 1, 1, cauldron),
    ("hay_bale", 1, 1, hay_bale),
    ("beehive", 1, 1, |c| beehive(c, true)),
    ("beehive_empty", 1, 1, |c| beehive(c, false)),
    ("fish_barrel", 1, 1, fish_barrel),
    ("candle_stand", 1, 1, |c| candle_stand(c, true)),
    ("candle_stand_out", 1, 1, |c| candle_stand(c, false)),
    ("bell_rope", 1, 1, bell_rope),
    ("grave_cross", 1, 1, grave_cross),
    ("brazier", 1, 1, |c| brazier(c, true)),
    ("brazier_out", 1, 1, |c| brazier(c, false)),
    ("flower_pot", 1, 1, flower_pot),
    ("flower_box", 1, 1, flower_box),
    ("cat_basket", 1, 1, cat_basket),
    ("sign_tavern", 1, 1, |c| hanging_sign(c, Emblem::Mug)),
    ("sign_bakery", 1, 1, |c| hanging_sign(c, Emblem::Loaf)),
    ("sign_smithy", 1, 1, |c| hanging_sign(c, Emblem::Anvil)),
    ("sign_apothecary", 1, 1, |c| hanging_sign(c, Emblem::Flask)),
    ("moonpetal", 1, 1, |c| moonpetal(c, true)),
    ("moonpetal_picked", 1, 1, |c| moonpetal(c, false)),
    ("feverleaf_bush", 1, 1, |c| feverleaf(c, true)),
    ("feverleaf_picked", 1, 1, |c| feverleaf(c, false)),
    ("bitterroot", 1, 1, |c| bitterroot(c, true)),
    ("bitterroot_picked", 1, 1, |c| bitterroot(c, false)),
];

/// The sheet's layout: (name, first cell, width, height), packed row by row into `COLS`
/// columns (first fit, in `SPRITES` order), and the number of rows.
fn layout() -> (Vec<(&'static str, u32, u32, u32)>, u32) {
    let mut used: Vec<[bool; COLS as usize]> = Vec::new();
    let mut out = Vec::new();
    for &(name, w, h, _) in SPRITES {
        let mut row = 0usize;
        let pos = loop {
            while used.len() < row + h as usize {
                used.push([false; COLS as usize]);
            }
            let fits = |col: usize| (row..row + h as usize).all(|r| (col..col + w as usize).all(|c| !used[r][c]));
            if let Some(col) = (0..=(COLS - w) as usize).find(|&col| fits(col)) {
                break (row, col);
            }
            row += 1;
        };
        for row in used.iter_mut().skip(pos.0).take(h as usize) {
            row[pos.1..pos.1 + w as usize].iter_mut().for_each(|c| *c = true);
        }
        out.push((name, pos.0 as u32 * COLS + pos.1 as u32, w, h));
    }
    let rows = used.len() as u32;
    (out, rows)
}

/// The sprite of `name` on the village sheet.
pub fn sprite(name: &str) -> rustpg2d_engine::defs::SpriteRef {
    let (cells, _) = layout();
    let &(_, cell, w, h) = cells.iter().find(|s| s.0 == name).unwrap_or_else(|| panic!("no village sprite {name}"));
    if w == 1 && h == 1 {
        rustpg2d_engine::defs::SpriteRef::new(SHEET, cell)
    } else {
        rustpg2d_engine::defs::SpriteRef::block(SHEET, cell, w, h)
    }
}

/// Draws the sheet.
pub fn sheet() -> Canvas {
    let (cells, rows) = layout();
    let mut c = Canvas::new(COLS * TS, rows * TS);
    for ((_, cell, _, _), (_, _, _, draw)) in cells.iter().zip(SPRITES) {
        c.at(cell % COLS * TS, cell / COLS * TS);
        draw(&mut c);
    }
    c
}

pub fn save(out: &Path) {
    sheet().save(&out.join("sprites/village.png"));
}

/// A warm glow around a light source (after the outline, so it isn't outlined).
fn glow(c: &mut Canvas, cx: f32, cy: f32, r: f32, rgb: (f32, f32, f32), a: f32) {
    c.ellipse(cx, cy, r, r * 0.85, Color::new(rgb.0, rgb.1, rgb.2, a));
    c.ellipse(cx, cy, r * 0.55, r * 0.5, Color::new(rgb.0, rgb.1, rgb.2, a));
}

/// Flames with their tip at (`cx`, `top`), `w` wide at the base `base`.
fn flames(c: &mut Canvas, cx: f32, base: f32, top: f32, w: f32) {
    let h = base - top;
    for (k, col) in FLAME.iter().enumerate() {
        let s = 1.0 - k as f32 * 0.3;
        c.ellipse(cx, base - h * 0.45 * s, w * 0.5 * s, h * 0.55 * s, hex(*col));
    }
}

// ------------------------------------------------------------------ market stalls

enum Goods {
    Produce,
    Fish,
    Bread,
}

/// A 3x3 market stall: a striped awning on two posts at the back, the seller's place behind the
/// counter (the middle row), the counter with goods in the bottom row.
fn stall(c: &mut Canvas, cloth: [u32; 4], goods: Goods) {
    let n = 96;
    for x in [8, 84] {
        c.shade_box(x, 18, 4, 56, WOOD);
    }
    // The awning: stripes, a darker ridge, a scalloped valance.
    c.rect(4, 4, 88, 3, hex(cloth[0]));
    for x in 2..94 {
        let stripe = ((x - 2) / 11) % 2 == 0;
        for y in 7..28 {
            let shade = if y < 10 { 1.1 } else if y > 24 { 0.85 } else { 1.0 };
            let col = if stripe { cloth[2] } else { 0xf2ead8 };
            c.px(x, y, hex(super::mul(col, shade)));
        }
    }
    for k in 0..8 {
        let cx = 2.0 + 5.75 + 11.5 * k as f32;
        let col = if k % 2 == 0 { cloth[1] } else { 0xd8ccb4 };
        c.ellipse(cx, 28.0, 5.75, 5.0, hex(col));
    }
    // The counter: a top board and a planked front.
    c.shade_box(0, 64, n, 6, [WOOD[1], WOOD[2], WOOD[3], 0xd0a060]);
    c.rect(2, 70, 92, 22, hex(WOOD[1]));
    for x in (2..94).step_by(12) {
        c.vline(x, 70, 22, hex(WOOD[0]));
        c.vline(x + 1, 71, 20, hex(WOOD[2]));
    }
    c.hline(2, 91, 92, hex(WOOD[0]));
    match goods {
        Goods::Produce => {
            // Baskets of apples and carrots, cabbages, a pumpkin.
            for bx in [6, 34] {
                c.shade_box(bx, 56, 22, 9, STRAW);
                c.hline(bx, 59, 22, hex(STRAW[0]));
            }
            for (x, y) in [(10.0, 54.0), (15.0, 53.0), (20.0, 54.0), (24.0, 55.0), (13.0, 57.0), (19.0, 57.0)] {
                c.shade_disc(x, y, 3.2, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
            }
            for k in 0..5 {
                let x = 37 + k * 4;
                c.stroke(x as f32, 50.0, x as f32 + 2.0, 58.0, 1.4, hex(0xf08a30));
                c.px(x, 49, hex(0x43a047));
                c.px(x + 1, 48, hex(0x7cc56f));
            }
            for x in [64.0, 74.0] {
                c.shade_disc(x, 58.0, 5.5, [0x2e6a2a, 0x4a9a3e, 0x7cc56f, 0xc8f0a8]);
            }
            c.shade_ellipse(86.0, 58.0, 7.0, 6.0, [0x8a3a08, 0xd06010, 0xf08a30, 0xffc078]);
            c.rect(85, 51, 2, 3, hex(0x43a047));
        }
        Goods::Fish => {
            // A crate of ice with fish laid out, a basket of crabs.
            c.shade_box(6, 54, 56, 11, [0x8ab0c8, 0xb8d8ec, 0xdcf0fa, 0xffffff]);
            for k in 0..4 {
                let x = 12.0 + k as f32 * 13.0;
                c.shade_ellipse(x, 58.0, 5.5, 2.5, FISH);
                c.ellipse(x + 6.0, 58.0, 2.0, 2.5, hex(FISH[1]));
                c.px(x as i32 - 3, 57, hex(0x111111));
            }
            c.shade_box(68, 55, 22, 10, STRAW);
            for x in [72.0, 79.0, 86.0] {
                c.shade_ellipse(x, 54.0, 3.5, 2.5, [0x7a1a08, 0xb83a18, 0xe0602a, 0xff9a6a]);
            }
        }
        Goods::Bread => {
            // Loaves, a basket of rolls, a sack of flour.
            for (x, y, rx) in [(12.0, 58.0, 7.0), (27.0, 58.0, 7.0), (19.0, 53.0, 6.0)] {
                c.shade_ellipse(x, y, rx, 4.5, BREAD);
                c.line(x as i32 - 3, y as i32 - 2, x as i32 + 1, y as i32 + 1, hex(0xf0d8a0));
            }
            c.shade_box(38, 56, 22, 9, STRAW);
            for x in [42.0, 48.0, 54.0] {
                c.shade_disc(x, 55.0, 3.5, BREAD);
            }
            c.shade_ellipse(78.0, 54.0, 9.0, 11.0, CREAM);
            c.rect(74, 43, 8, 3, hex(BURLAP[1]));
            c.hline(73, 54, 10, hex(0x5a7ab0));
        }
    }
    c.auto_outline(n, n, OUTLINE);
    c.shadow(48.0, 92.0, 46.0, 3.0);
}

// ------------------------------------------------------------------ big outdoor props

fn windmill(c: &mut Canvas) {
    let (w, h) = (128, 160);
    // The tower: whitewashed, tapering, on a stone foot.
    for y in 50..152 {
        let half = 21 + (y - 50) * 13 / 102;
        for x in 64 - half..64 + half {
            let t = (x - (64 - half)) as f32 / (2 * half) as f32;
            let l = 0.95 - 1.1 * (t - 0.3).abs() + (bayer(x, y) - 0.5) * 0.2;
            let pal = [0x9a9080, 0xc8bea8, 0xe8e0cc, 0xfaf6ea];
            let k = if l < 0.3 { 0 } else if l < 0.6 { 1 } else if l < 0.88 { 2 } else { 3 };
            c.px(x, y, hex(pal[k]));
        }
    }
    for y in [70, 92, 114, 136] {
        let half = 21 + (y - 50) * 13 / 102;
        c.hline(64 - half, y, 2 * half, hex(0xb8ae98));
    }
    c.shade_box(28, 146, 72, 12, STONE);
    c.rrect(56, 126, 16, 26, 6, hex(WOOD[0]));
    c.rrect(58, 128, 12, 24, 5, hex(WOOD[2]));
    c.vline(64, 130, 22, hex(WOOD[1]));
    for (x, y) in [(52, 84), (70, 106)] {
        c.rect(x, y, 8, 10, hex(WOOD[0]));
        c.rect(x + 1, y + 1, 6, 8, hex(0x2a3446));
    }
    // A thatched cap.
    for y in 26..54 {
        let half = (y - 26) * 26 / 28 + 3;
        for x in 64 - half..64 + half {
            let k = if (x + y / 3) % 7 == 0 { 0 } else if y % 6 == 0 { 1 } else if x < 64 - half / 3 { 3 } else { 2 };
            c.px(x, y, hex(STRAW[k]));
        }
    }
    c.rect(36, 52, 56, 3, hex(STRAW[0]));
    // Four sails in an X, a lattice of laths with canvas, on a hub.
    let hub = (64.0f32, 46.0f32);
    for a in [0.785f32, 2.356, 3.927, 5.498] {
        let (dx, dy) = (a.cos(), a.sin());
        let (px, py) = (-dy, dx);
        // Every pixel of the sail's rectangle (along the spar 12..58, across 0..12).
        for y in 0..h {
            for x in 0..w {
                let (rx, ry) = (x as f32 + 0.5 - hub.0, y as f32 + 0.5 - hub.1);
                let (r, s) = (rx * dx + ry * dy, rx * px + ry * py);
                if !(12.0..58.0).contains(&r) || !(0.0..12.0).contains(&s) {
                    continue;
                }
                let lath = (r as i32) % 7 == 0 || s < 1.0 || s >= 11.0;
                let col = if lath { WOOD[1] } else if s < 6.0 { 0xf2ead8 } else { 0xd8ccb4 };
                c.px(x, y, hex(col));
            }
        }
        c.stroke(hub.0, hub.1, hub.0 + dx * 58.0, hub.1 + dy * 58.0, 1.4, hex(WOOD[0]));
    }
    c.shade_disc(hub.0, hub.1, 5.0, WOOD);
    c.disc(hub.0, hub.1, 1.5, hex(IRON[1]));
    c.auto_outline(w, h, OUTLINE);
    c.shadow(64.0, 156.0, 44.0, 4.0);
}

fn watchtower(c: &mut Canvas) {
    let (w, h) = (64, 128);
    // Stilts with cross braces, a ladder in the middle.
    for x in [6, 54] {
        c.shade_box(x, 44, 5, 82, WOOD);
    }
    for (y0, y1) in [(54, 88), (88, 122)] {
        c.stroke(10.0, y0 as f32, 54.0, y1 as f32, 1.2, hex(WOOD[1]));
        c.stroke(54.0, y0 as f32, 10.0, y1 as f32, 1.2, hex(WOOD[1]));
    }
    for x in [26, 37] {
        c.vline(x, 50, 76, hex(WOOD[2]));
        c.vline(x + 1, 50, 76, hex(WOOD[1]));
    }
    for y in (54..126).step_by(8) {
        c.hline(27, y, 10, hex(WOOD[3]));
    }
    // The platform with a railing, the lookout's roof.
    c.shade_box(0, 38, 64, 9, [WOOD[0], WOOD[1], WOOD[2], 0xc89058]);
    for x in [2, 16, 31, 46, 59] {
        c.shade_box(x, 22, 3, 16, WOOD);
    }
    c.shade_box(0, 26, 64, 3, WOOD);
    for y in 0..20 {
        let half = 4 + y * 30 / 20;
        for x in 32 - half..32 + half {
            c.px(x, y + 2, hex(if (x + y) % 5 == 0 { 0x5e231e } else if y < 6 { 0xc65a50 } else { 0xa8433a }));
        }
    }
    c.rect(0, 21, 64, 2, hex(0x5e231e));
    c.auto_outline(w, h, OUTLINE);
    c.shadow(32.0, 125.0, 30.0, 3.0);
}

fn apple_tree(c: &mut Canvas, apples: bool) {
    let (w, h) = (64, 96);
    c.shade_cyl(26, 56, 12, 36, WOOD);
    c.rect(22, 88, 20, 3, hex(WOOD[1]));
    c.stroke(32.0, 62.0, 18.0, 44.0, 2.5, hex(WOOD[1]));
    c.stroke(32.0, 60.0, 46.0, 42.0, 2.5, hex(WOOD[1]));
    for (x, y, r) in [(32.0, 36.0, 22.0), (16.0, 42.0, 13.0), (48.0, 42.0, 13.0), (22.0, 24.0, 14.0), (42.0, 24.0, 14.0), (32.0, 16.0, 12.0), (32.0, 52.0, 12.0)] {
        c.shade_disc(x, y, r, LEAF);
    }
    if apples {
        for (x, y) in [(18.0, 38.0), (26.0, 28.0), (40.0, 33.0), (47.0, 45.0), (33.0, 47.0), (22.0, 50.0), (38.0, 18.0), (12.0, 46.0), (52.0, 34.0)] {
            c.shade_disc(x, y, 2.6, [0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80]);
            c.px(x as i32, y as i32 - 3, hex(0x3e2410));
        }
    }
    c.auto_outline(w, h, 0x10301a);
    c.shadow(32.0, 91.0, 18.0, 4.0);
}

fn bell_tower(c: &mut Canvas) {
    let (w, h) = (64, 96);
    // Two stone piers under a little roof, the bronze bell between them.
    for x in [4, 46] {
        c.shade_box(x, 24, 14, 68, STONE);
        for y in (30..92).step_by(9) {
            c.hline(x, y, 14, hex(STONE[0]));
        }
    }
    c.shade_box(2, 20, 60, 6, STONE);
    for y in 0..20 {
        let half = 3 + y * 30 / 20;
        for x in 32 - half..32 + half {
            c.px(x, y + 1, hex(if (x + 2 * y) % 6 == 0 { 0x2e3444 } else if y < 7 { 0x6a7898 } else { 0x546078 }));
        }
    }
    c.hline(18, 30, 28, hex(WOOD[1]));
    c.vline(32, 30, 6, hex(IRON[1]));
    c.shade_ellipse(32.0, 48.0, 11.0, 13.0, BRONZE);
    c.rect(18, 48, 28, 14, CLEAR);
    c.shade_box(19, 44, 26, 12, BRONZE);
    c.rect(17, 55, 30, 4, hex(BRONZE[1]));
    c.hline(17, 55, 30, hex(BRONZE[3]));
    c.disc(32.0, 61.0, 2.5, hex(IRON[1]));
    c.shade_box(0, 88, 64, 6, STONE);
    c.auto_outline(w, h, OUTLINE);
    c.shadow(32.0, 93.0, 30.0, 2.5);
}

// ------------------------------------------------------------------ fires and stations

fn oven(c: &mut Canvas, lit: bool) {
    let (w, h) = (64, 64);
    // A brick dome on a stone base: the dome first, the base in front of its foot.
    c.shade_ellipse(32.0, 44.0, 28.0, 30.0, BRICK);
    for y in (18..46).step_by(6) {
        for x in 4..60 {
            let (dx, dy) = ((x as f32 + 0.5 - 32.0) / 28.0, (y as f32 + 0.5 - 44.0) / 30.0);
            if dx * dx + dy * dy <= 0.9 {
                c.px(x, y, hex(BRICK[0]));
            }
        }
    }
    c.rect(0, 46, 64, 18, CLEAR);
    c.shade_box(2, 46, 60, 16, STONE);
    for x in (2..62).step_by(10) {
        c.vline(x, 47, 15, hex(STONE[0]));
    }
    c.shade_box(42, 4, 9, 18, BRICK);
    c.rect(41, 3, 11, 3, hex(BRICK[3]));
    c.rrect(19, 28, 26, 18, 9, hex(0x5a2418));
    c.rrect(21, 30, 22, 16, 8, hex(0x140a06));
    if lit {
        c.ellipse(32.0, 43.0, 10.0, 3.0, hex(0x8a2a08));
        flames(c, 26.0, 44.0, 33.0, 8.0);
        flames(c, 36.0, 44.0, 35.0, 7.0);
        c.shade_ellipse(31.0, 40.0, 4.0, 2.5, BREAD);
    } else {
        c.ellipse(32.0, 43.0, 9.0, 2.5, hex(0x3a3a40));
    }
    c.auto_outline(w, h, OUTLINE);
    if lit {
        glow(c, 32.0, 38.0, 22.0, (1.0, 0.6, 0.2), 0.12);
    }
    c.shadow(32.0, 62.0, 30.0, 2.0);
}

/// A stone fireplace; `pot`: a cooking pot hangs over the fire (the kitchen hearth).
fn fireplace(c: &mut Canvas, lit: bool, pot: bool) {
    let (w, h) = (64, 64);
    c.shade_box(2, 10, 60, 52, STONE);
    for (row, y) in (10..62).step_by(8).enumerate() {
        c.hline(2, y, 60, hex(STONE[0]));
        let off = if row % 2 == 0 { 0 } else { 10 };
        for x in (2 + off..62).step_by(20) {
            c.vline(x, y, 8, hex(STONE[0]));
        }
    }
    c.shade_box(0, 6, 64, 7, [WOOD[0], WOOD[1], WOOD[2], 0xc89058]);
    c.rrect(12, 22, 40, 40, 10, hex(0x2a2420));
    c.rrect(14, 24, 36, 38, 9, hex(0x120e0c));
    c.stroke(20.0, 58.0, 44.0, 54.0, 2.0, hex(WOOD[1]));
    c.stroke(20.0, 54.0, 44.0, 58.0, 2.0, hex(WOOD[2]));
    if lit {
        flames(c, 28.0, 56.0, 40.0, 10.0);
        flames(c, 37.0, 56.0, 43.0, 8.0);
    } else {
        c.ellipse(32.0, 58.0, 12.0, 2.5, hex(0x4a4a50));
    }
    if pot {
        c.vline(32, 24, 12, hex(IRON[1]));
        c.shade_ellipse(32.0, 42.0, 9.0, 7.0, IRON);
        c.ellipse(32.0, 36.0, 8.0, 2.0, hex(if lit { 0xb06a34 } else { 0x5a3a24 }));
        c.hline(23, 36, 18, hex(IRON[3]));
    }
    // Things on the mantel: a jug, a candle.
    c.shade_box(8, 1, 6, 6, CLAY);
    c.rect(48, 0, 3, 6, hex(0xf0ead8));
    c.auto_outline(w, h, OUTLINE);
    if lit {
        glow(c, 32.0, 48.0, 22.0, (1.0, 0.55, 0.2), 0.12);
    }
}

fn forge(c: &mut Canvas, lit: bool) {
    let (w, h) = (64, 64);
    // The hood and chimney over a stone hearth with a coal bed.
    c.shade_box(22, 0, 20, 12, STONE);
    for y in 10..26 {
        let half = 10 + (y - 10) * 18 / 16;
        c.hline(32 - half, y, 2 * half, hex(if y % 5 == 0 { STONE[0] } else if y < 14 { STONE[3] } else { STONE[2] }));
    }
    c.shade_box(2, 32, 60, 30, STONE);
    for y in [40, 48, 56] {
        c.hline(2, y, 60, hex(STONE[0]));
    }
    c.shade_box(0, 28, 64, 6, [0x2a2420, 0x3a3430, 0x5a5048, 0x7a6e64]);
    c.rect(6, 26, 52, 4, hex(0x1a1614));
    for k in 0..26 {
        let x = 7 + (k * 7) % 50;
        let y = 26 + (k * 3) % 4;
        let col = if !lit { 0x2a2a30 } else if k % 3 == 0 { 0xffe680 } else if k % 3 == 1 { 0xffa020 } else { 0xe8590c };
        c.px(x, y, hex(col));
        c.px(x + 1, y, hex(col));
    }
    if lit {
        flames(c, 24.0, 28.0, 18.0, 7.0);
        flames(c, 40.0, 28.0, 20.0, 6.0);
    }
    // Tongs leaning on the side.
    c.stroke(56.0, 32.0, 60.0, 60.0, 0.8, hex(IRON[2]));
    c.stroke(58.0, 32.0, 61.0, 60.0, 0.8, hex(IRON[1]));
    c.auto_outline(w, h, OUTLINE);
    if lit {
        glow(c, 32.0, 26.0, 24.0, (1.0, 0.5, 0.15), 0.13);
    }
    c.shadow(32.0, 62.0, 30.0, 2.0);
}

fn cauldron(c: &mut Canvas) {
    c.ellipse(16.0, 29.0, 7.0, 2.0, hex(0xe8590c));
    c.px(13, 27, hex(0xffa020));
    c.px(18, 27, hex(0xffe680));
    for x in [8, 23] {
        c.vline(x, 22, 8, hex(IRON[0]));
    }
    c.shade_ellipse(16.0, 19.0, 11.0, 9.0, IRON);
    c.ellipse(16.0, 12.0, 11.0, 3.0, hex(IRON[2]));
    c.ellipse(16.0, 12.5, 9.0, 2.0, hex(0x43a047));
    for (x, y) in [(13, 11), (19, 12), (16, 9)] {
        c.px(x, y, hex(0xa5d6a7));
    }
    c.disc(18.0, 7.0, 1.2, hex(0xa5d6a7));
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 12.0, 2.0);
}

fn bellows(c: &mut Canvas) {
    c.shade_box(4, 12, 18, 3, WOOD);
    c.shade_box(4, 23, 18, 3, WOOD);
    c.shade_ellipse(13.0, 19.0, 9.0, 5.0, [0x3a2414, 0x5a3a22, 0x7a5436, 0x9a7050]);
    for x in [8, 12, 16] {
        c.vline(x, 15, 8, hex(0x3a2414));
    }
    c.shade_box(22, 17, 8, 4, IRON);
    c.rect(1, 12, 4, 3, hex(WOOD[2]));
    c.rect(1, 23, 4, 3, hex(WOOD[2]));
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(15.0, 29.0, 12.0, 2.0);
}

fn grindstone(c: &mut Canvas) {
    c.shade_box(6, 22, 20, 4, WOOD);
    for x in [7, 23] {
        c.shade_box(x, 12, 3, 18, WOOD);
    }
    c.shade_disc(16.0, 15.0, 9.0, STONE);
    c.disc(16.0, 15.0, 2.0, hex(IRON[1]));
    c.stroke(16.0, 15.0, 28.0, 10.0, 0.9, hex(IRON[2]));
    c.rect(27, 6, 2, 5, hex(WOOD[2]));
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 12.0, 2.0);
}

fn brazier(c: &mut Canvas, lit: bool) {
    for (x0, x1) in [(9.0, 6.0), (23.0, 26.0), (16.0, 16.0)] {
        c.stroke(x0, 20.0, x1, 30.0, 0.9, hex(IRON[1]));
    }
    c.shade_ellipse(16.0, 18.0, 11.0, 5.0, IRON);
    c.ellipse(16.0, 15.0, 9.0, 2.5, hex(if lit { 0xe8590c } else { 0x2a2a30 }));
    if lit {
        flames(c, 13.0, 16.0, 3.0, 8.0);
        flames(c, 19.0, 16.0, 6.0, 6.0);
    }
    c.auto_outline(32, 32, OUTLINE);
    if lit {
        glow(c, 16.0, 10.0, 15.0, (1.0, 0.6, 0.2), 0.12);
    }
    c.shadow(16.0, 30.0, 10.0, 2.0);
}

fn candle_stand(c: &mut Canvas, lit: bool) {
    c.shade_ellipse(16.0, 28.0, 7.0, 2.5, BRONZE);
    c.shade_box(15, 10, 3, 18, BRONZE);
    c.shade_box(6, 13, 21, 3, BRONZE);
    for x in [7, 15, 24] {
        c.rect(x, 6, 3, 7, hex(0xf2ead8));
        c.vline(x, 6, 7, hex(0xffffff));
        if lit {
            c.ellipse(x as f32 + 1.5, 3.5, 1.5, 2.5, hex(0xffa020));
            c.px(x + 1, 3, hex(0xffe680));
        }
    }
    c.auto_outline(32, 32, OUTLINE);
    if lit {
        glow(c, 16.0, 5.0, 14.0, (1.0, 0.85, 0.5), 0.1);
    }
    c.shadow(16.0, 30.0, 8.0, 1.5);
}

// ------------------------------------------------------------------ furniture

/// A shelf frame (2x2) with a dark back and three boards.
fn shelf_frame(c: &mut Canvas) {
    c.shade_box(2, 2, 60, 60, WOOD);
    c.rect(6, 6, 52, 52, hex(0x3d2712));
    for y in [22, 40, 56] {
        c.shade_box(4, y, 56, 3, WOOD);
    }
}

fn bread_shelf(c: &mut Canvas) {
    shelf_frame(c);
    for (row, y) in [16.0f32, 34.0, 50.0].into_iter().enumerate() {
        match row {
            0 => {
                for x in [14.0, 30.0, 46.0] {
                    c.shade_ellipse(x, y, 7.0, 5.0, BREAD);
                    c.line(x as i32 - 3, y as i32 - 2, x as i32 + 2, y as i32 + 1, hex(0xf0d8a0));
                }
            }
            1 => {
                for k in 0..3 {
                    c.shade_ellipse(32.0, y - 2.0 + k as f32 * 2.0, 22.0, 2.2, BREAD);
                }
            }
            _ => {
                for k in 0..7 {
                    c.shade_disc(11.0 + k as f32 * 7.0, y, 3.4, BREAD);
                }
            }
        }
    }
    c.auto_outline(64, 64, OUTLINE);
}

fn potion_shelf(c: &mut Canvas) {
    shelf_frame(c);
    let liquids = [[0x7a1010, 0xb71c1c, 0xe53935, 0xff8a80], [0x1a237e, 0x283593, 0x3f51b5, 0x7986cb], [0x1b5e20, 0x2e7d32, 0x43a047, 0xa5d6a7], [0x6a1b9a, 0x8e24aa, 0xba68c8, 0xe1bee7]];
    for (row, y) in [22, 40, 56].into_iter().enumerate() {
        for k in 0..6 {
            let x = 8 + k * 9;
            let liq = liquids[(k as usize + row) % 4];
            if (k + row as i32) % 3 == 0 {
                c.shade_disc(x as f32 + 3.0, y as f32 - 5.0, 4.0, liq);
                c.rect(x + 2, y - 13, 3, 4, hex(0xcfe8ff));
            } else {
                c.shade_box(x, y - 10, 6, 10, liq);
                c.rect(x + 1, y - 13, 4, 3, hex(WOOD[2]));
            }
        }
    }
    c.auto_outline(64, 64, OUTLINE);
}

fn tool_rack(c: &mut Canvas) {
    for x in [4, 56] {
        c.shade_box(x, 4, 4, 58, WOOD);
    }
    c.shade_box(2, 8, 60, 36, [WOOD[0], WOOD[1], WOOD[1], WOOD[2]]);
    for x in (6..58).step_by(8) {
        c.vline(x, 9, 34, hex(WOOD[0]));
    }
    // A saw, hammers, tongs, horseshoes on pegs.
    for y in 22..30 {
        c.hline(9, y, (y - 20) * 2, hex(0xc5d0d6));
    }
    c.rect(8, 14, 4, 9, hex(WOOD[2]));
    c.rect(24, 12, 2, 18, hex(WOOD[3]));
    c.shade_box(21, 12, 8, 5, IRON);
    c.rect(34, 12, 2, 16, hex(WOOD[3]));
    c.shade_box(32, 10, 6, 4, IRON);
    c.stroke(44.0, 12.0, 41.0, 32.0, 0.8, hex(IRON[2]));
    c.stroke(46.0, 12.0, 49.0, 32.0, 0.8, hex(IRON[2]));
    // A horseshoe.
    c.stroke(49.0, 22.0, 49.0, 16.0, 0.9, hex(IRON[2]));
    c.stroke(55.0, 22.0, 55.0, 16.0, 0.9, hex(IRON[2]));
    c.stroke(49.0, 16.0, 55.0, 16.0, 0.9, hex(IRON[2]));
    c.shade_box(10, 36, 44, 8, WOOD);
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 62.0, 28.0, 2.0);
}

fn weapon_rack(c: &mut Canvas) {
    for x in [3, 57] {
        c.shade_box(x, 10, 4, 52, WOOD);
    }
    c.shade_box(2, 14, 60, 4, WOOD);
    c.shade_box(2, 48, 60, 4, WOOD);
    let steel = [0x5f6b73, 0x90a4ae, 0xc5d0d6, 0xf0f4f6];
    for x in [12, 22] {
        c.rect(x, 6, 3, 38, hex(steel[2]));
        c.vline(x, 6, 38, hex(steel[3]));
        c.rect(x - 3, 40, 9, 3, hex(GOLD[1]));
        c.rect(x, 43, 3, 8, hex(WOOD[1]));
    }
    c.rect(33, 0, 2, 58, hex(WOOD[2]));
    for k in 0..6 {
        c.hline(32 + k / 2, k, 4 - k / 2 * 2, hex(steel[2]));
    }
    c.shade_disc(48.0, 32.0, 10.0, WOOD);
    c.disc(48.0, 32.0, 3.0, hex(steel[2]));
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 62.0, 28.0, 2.0);
}

fn herb_rack(c: &mut Canvas) {
    for x in [3, 26] {
        c.shade_box(x, 4, 3, 58, WOOD);
    }
    for y in [8, 34] {
        c.shade_box(2, y, 28, 3, WOOD);
        for (k, x) in [8, 15, 22].into_iter().enumerate() {
            let col = [[0x1d5124, 0x2e7d32, 0x43a047, 0x7cc56f], [0x4a2a6a, 0x6a3a8e, 0x9a6ac0, 0xc8a8e8], [0x7a6a10, 0xa8941e, 0xd8c040, 0xf8e880]][(k + y as usize / 30) % 3];
            c.vline(x, y + 3, 4, hex(0xd8c8a0));
            c.shade_ellipse(x as f32 + 0.5, y as f32 + 13.0, 3.5, 7.0, col);
        }
    }
    c.auto_outline(32, 64, OUTLINE);
    c.shadow(16.0, 62.0, 14.0, 2.0);
}

fn keg_end(c: &mut Canvas, cx: f32, cy: f32, r: f32) {
    c.disc(cx, cy, r, hex(IRON[1]));
    c.shade_disc(cx, cy, r - 1.5, WOOD);
    c.disc(cx, cy, r - 5.0, hex(IRON[1]));
    c.shade_disc(cx, cy, r - 6.0, WOOD);
    for k in -2..=2 {
        c.vline(cx as i32 + k * 3, (cy - r + 3.0) as i32, (2.0 * r - 6.0) as i32, hex(WOOD[1]));
    }
    c.shade_box(cx as i32 - 1, (cy + r * 0.45) as i32, 3, 4, BRONZE);
}

fn keg(c: &mut Canvas) {
    c.shade_box(5, 24, 22, 5, WOOD);
    keg_end(c, 16.0, 14.0, 11.0);
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 12.0, 2.0);
}

fn keg_rack(c: &mut Canvas) {
    for x in [3, 29, 57] {
        c.shade_box(x, 14, 4, 16, WOOD);
    }
    c.shade_box(1, 24, 62, 5, WOOD);
    keg_end(c, 17.0, 13.0, 11.0);
    keg_end(c, 47.0, 13.0, 11.0);
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 30.0, 2.0);
}

/// A table top with legs (2x1).
fn table_top(c: &mut Canvas) {
    c.shade_box(2, 6, 60, 15, [WOOD[0], WOOD[1], WOOD[2], 0xc89058]);
    for y in [10, 15] {
        c.hline(3, y, 58, hex(WOOD[1]));
    }
    c.rect(2, 20, 60, 3, hex(WOOD[0]));
    for x in [5, 55] {
        c.shade_box(x, 22, 4, 8, WOOD);
    }
}

fn tavern_table(c: &mut Canvas) {
    table_top(c);
    for x in [12, 44] {
        c.shade_box(x, 4, 7, 8, [0x6a6a72, 0x8a8a94, 0xb0b0ba, 0xd8d8e0]);
        c.rect(x, 3, 7, 2, hex(0xfff8e8));
        c.vline(x + 7, 6, 3, hex(0x8a8a94));
    }
    c.ellipse(30.0, 12.0, 7.0, 3.0, hex(0xf2efe6));
    c.shade_ellipse(29.0, 11.0, 3.5, 2.0, BREAD);
    c.disc(33.0, 12.0, 1.5, hex(0xe53935));
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 30.0, 2.0);
}

fn dough_table(c: &mut Canvas) {
    table_top(c);
    c.ellipse(20.0, 13.0, 12.0, 4.0, hex(0xf0e8d8));
    c.shade_ellipse(20.0, 12.0, 6.0, 3.5, CREAM);
    c.shade_box(38, 10, 16, 3, [WOOD[1], WOOD[2], WOOD[3], 0xe0c090]);
    c.rect(35, 11, 3, 1, hex(WOOD[1]));
    c.rect(54, 11, 3, 1, hex(WOOD[1]));
    c.shade_box(48, 3, 8, 6, CLAY);
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 30.0, 2.0);
}

fn stool(c: &mut Canvas) {
    for (x0, x1) in [(10.0, 8.0), (22.0, 24.0), (16.0, 16.0)] {
        c.stroke(x0, 18.0, x1, 29.0, 1.1, hex(WOOD[1]));
    }
    c.shade_ellipse(16.0, 16.0, 9.0, 4.0, WOOD);
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 9.0, 2.0);
}

/// A pew seen from behind (people sit in it facing up, towards the altar); `back`: only the
/// backrest, drawn over a sitter.
fn pew(c: &mut Canvas, back: bool) {
    if !back {
        c.shade_box(2, 6, 60, 6, [WOOD[0], WOOD[1], WOOD[2], 0xc89058]);
    }
    c.shade_box(0, 12, 64, 13, WOOD);
    c.hline(1, 17, 62, hex(WOOD[1]));
    c.shade_box(0, 10, 4, 20, WOOD);
    c.shade_box(60, 10, 4, 20, WOOD);
    c.rect(4, 25, 56, 4, hex(WOOD[0]));
    c.auto_outline(64, 32, OUTLINE);
    if !back {
        c.shadow(32.0, 30.0, 30.0, 2.0);
    }
}

fn altar(c: &mut Canvas) {
    c.shade_box(4, 10, 56, 20, [0x8a8478, 0xb8b0a0, 0xdcd4c4, 0xf2ece0]);
    c.rect(4, 8, 56, 10, hex(0xf2efe6));
    c.rect(26, 8, 12, 20, hex(0xa82a2a));
    c.hline(26, 27, 12, hex(GOLD[2]));
    c.hline(4, 17, 56, hex(GOLD[2]));
    for x in [10, 51] {
        c.rect(x, 0, 3, 9, hex(0xf2ead8));
        c.px(x + 1, 0, hex(0xffa020));
    }
    c.rect(24, 5, 16, 5, hex(0xf8f4e8));
    c.vline(31, 5, 5, hex(0x8a6a4a));
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 30.0, 2.0);
}

fn bell_rope(c: &mut Canvas) {
    for y in 0..24 {
        let col = if (y / 2) % 2 == 0 { 0xd8c8a0 } else { 0xa89878 };
        c.rect(15, y, 3, 1, hex(col));
    }
    c.shade_box(13, 23, 7, 3, [0x6e1a1a, 0xa82a2a, 0xd04040, 0xf07a6a]);
    for x in 13..20 {
        c.vline(x, 26, 3 + (x % 2), hex(if x % 2 == 0 { 0xd04040 } else { 0xa82a2a }));
    }
    c.auto_outline(32, 32, OUTLINE);
}

fn cat_basket(c: &mut Canvas) {
    c.shade_ellipse(16.0, 22.0, 13.0, 7.0, STRAW);
    c.ellipse(16.0, 19.0, 10.0, 4.0, hex(0xa82a2a));
    c.ellipse(16.0, 19.0, 7.0, 2.5, hex(0xd04040));
    for x in (5..28).step_by(4) {
        c.vline(x, 21, 6, hex(STRAW[0]));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 29.0, 13.0, 2.0);
}

// ------------------------------------------------------------------ farm and yard

fn chicken_coop(c: &mut Canvas) {
    for x in [10, 50] {
        c.shade_box(x, 44, 4, 16, WOOD);
    }
    c.shade_box(6, 22, 52, 24, WOOD);
    for x in (8..56).step_by(6) {
        c.vline(x, 23, 22, hex(WOOD[1]));
    }
    for y in 0..20 {
        let half = 6 + y * 26 / 20;
        for x in 32 - half..32 + half {
            c.px(x, y + 4, hex(STRAW[if (x + y) % 6 == 0 { 0 } else if y < 6 { 3 } else { 2 }]));
        }
    }
    c.rect(2, 22, 60, 3, hex(STRAW[0]));
    c.rrect(26, 30, 12, 15, 5, hex(0x1a1208));
    c.stroke(32.0, 46.0, 42.0, 62.0, 2.0, hex(WOOD[2]));
    for k in 0..4 {
        c.hline(34 + k * 2, 49 + k * 3, 4, hex(WOOD[0]));
    }
    c.shade_ellipse(16.0, 58.0, 2.5, 3.0, CREAM);
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 61.0, 28.0, 2.5);
}

fn trough(c: &mut Canvas) {
    for x in [6, 54] {
        c.shade_box(x, 22, 4, 8, WOOD);
    }
    c.shade_box(2, 10, 60, 15, WOOD);
    c.rect(5, 12, 54, 6, hex(0x3a72c8));
    c.hline(5, 12, 54, hex(0x9ccbf5));
    c.hline(3, 20, 58, hex(WOOD[1]));
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 30.0, 2.0);
}

fn hay_bale(c: &mut Canvas) {
    c.shade_box(3, 10, 26, 19, STRAW);
    let mut rng = Rng(31);
    for _ in 0..30 {
        let (x, y) = (4 + rng.range(24) as i32, 11 + rng.range(17) as i32);
        c.hline(x, y, 3, hex(STRAW[(rng.range(2) * 3) as usize]));
    }
    for x in [9, 22] {
        c.vline(x, 10, 19, hex(0x7a5e22));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 14.0, 2.0);
}

fn beehive(c: &mut Canvas, full: bool) {
    c.shade_box(4, 26, 24, 4, WOOD);
    for k in 0..5 {
        let y = 24.0 - k as f32 * 4.0;
        let rx = 11.0 - k as f32 * 1.6;
        c.shade_ellipse(16.0, y, rx, 3.2, STRAW);
        c.hline((16.0 - rx) as i32 + 1, y as i32 + 2, (2.0 * rx) as i32 - 2, hex(STRAW[0]));
    }
    c.shade_disc(16.0, 5.0, 2.5, STRAW);
    c.ellipse(16.0, 24.0, 2.5, 2.0, hex(0x1a1208));
    if full {
        c.rect(20, 18, 2, 4, hex(GOLD[2]));
        c.px(20, 22, hex(GOLD[3]));
    }
    c.auto_outline(32, 32, OUTLINE);
    if full {
        for (x, y) in [(4, 8), (26, 12), (7, 16), (24, 4)] {
            c.px(x, y, hex(0xffd02a));
            c.px(x + 1, y, hex(0x1a1208));
        }
    }
    c.shadow(16.0, 30.0, 12.0, 2.0);
}

fn scarecrow(c: &mut Canvas) {
    c.shade_box(15, 14, 3, 48, WOOD);
    c.shade_box(1, 26, 30, 3, WOOD);
    c.shade_box(9, 25, 15, 20, [0x2a4a6a, 0x3a6a8e, 0x5a8ab0, 0x8ab0d0]);
    c.rect(12, 34, 4, 4, hex(0xa82a2a));
    for (x, y) in [(0, 30), (28, 30), (11, 45), (19, 45)] {
        for k in 0..3 {
            c.vline(x + k, y, 3 + k % 2, hex(STRAW[2 + k as usize % 2]));
        }
    }
    c.shade_disc(16.0, 17.0, 6.5, BURLAP);
    c.px(13, 16, hex(0x1a1208));
    c.px(19, 16, hex(0x1a1208));
    c.hline(13, 20, 7, hex(0x5a3a22));
    c.ellipse(16.0, 11.0, 11.0, 3.0, hex(STRAW[2]));
    c.shade_box(11, 4, 11, 7, STRAW);
    c.hline(11, 9, 11, hex(0xa82a2a));
    c.auto_outline(32, 64, OUTLINE);
    c.shadow(16.0, 62.0, 7.0, 1.5);
}

fn woodpile(c: &mut Canvas) {
    for (row, (n, y)) in [(5, 24.0f32), (4, 15.0), (3, 6.0)].into_iter().enumerate() {
        for k in 0..n {
            let x = 8.0 + k as f32 * 11.0 + row as f32 * 5.5;
            c.disc(x, y, 5.5, hex(WOOD[0]));
            c.shade_disc(x, y, 4.5, [0x8a6a3a, 0xb08a52, 0xd0aa70, 0xe8c890]);
            c.px(x as i32, y as i32, hex(0x8a6a3a));
        }
    }
    c.auto_outline(64, 32, OUTLINE);
    c.shadow(32.0, 30.0, 30.0, 2.0);
}

fn millstone(c: &mut Canvas) {
    c.shade_box(4, 40, 56, 20, WOOD);
    for x in (4..60).step_by(8) {
        c.vline(x, 41, 18, hex(WOOD[1]));
    }
    c.shade_ellipse(32.0, 40.0, 25.0, 9.0, STONE);
    c.shade_ellipse(32.0, 33.0, 21.0, 8.0, STONE);
    c.ellipse(32.0, 31.0, 3.0, 1.5, hex(0x2a2a30));
    c.ellipse(46.0, 44.0, 6.0, 2.0, hex(0xf0e8d8));
    // The hopper above, full of grain.
    for y in 4..20 {
        let half = 10 - (y - 4) / 2;
        c.hline(32 - half, y, 2 * half, hex(if y < 6 { WOOD[3] } else { WOOD[2] }));
    }
    c.ellipse(32.0, 5.0, 9.0, 2.0, hex(STRAW[2]));
    c.vline(24, 6, 22, hex(WOOD[1]));
    c.vline(40, 6, 22, hex(WOOD[1]));
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 61.0, 30.0, 2.0);
}

fn broken_cart(c: &mut Canvas) {
    // The cart bed tipped on its side, one wheel off, sacks spilled.
    for x in 6..50 {
        let y0 = 20 + (x - 6) / 6;
        for y in y0..y0 + 20 {
            let k = if y == y0 { 3 } else if (x - 6) % 9 == 0 { 0 } else { 2 };
            c.px(x, y, hex(WOOD[k]));
        }
    }
    c.shade_disc(16.0, 46.0, 9.0, WOOD);
    c.disc(16.0, 46.0, 6.5, hex(WOOD[0]));
    for a in 0..6 {
        let ang = a as f32 * 1.047;
        c.stroke(16.0, 46.0, 16.0 + ang.cos() * 7.0, 46.0 + ang.sin() * 7.0, 0.7, hex(WOOD[2]));
    }
    c.shade_ellipse(48.0, 56.0, 9.0, 3.5, WOOD);
    c.ellipse(48.0, 56.0, 5.0, 1.8, hex(WOOD[0]));
    c.stroke(50.0, 30.0, 62.0, 22.0, 1.5, hex(WOOD[1]));
    c.shade_ellipse(40.0, 46.0, 7.0, 6.0, BURLAP);
    c.shade_ellipse(55.0, 44.0, 6.0, 5.0, BURLAP);
    c.ellipse(46.0, 52.0, 8.0, 2.5, hex(0xf0e8d8));
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 58.0, 28.0, 4.0);
}

fn notice_board(c: &mut Canvas) {
    for x in [6, 54] {
        c.shade_box(x, 12, 4, 50, WOOD);
    }
    c.shade_box(2, 16, 60, 30, WOOD);
    c.rect(6, 19, 52, 24, hex(0xb08a5a));
    for (x, y, w, h, col) in [(9, 21, 12, 14, 0xf2ead8), (24, 22, 14, 10, 0xe8dcc0), (41, 20, 13, 16, 0xf8f4e8), (26, 34, 10, 8, 0xf0e0b0)] {
        c.rect(x, y, w, h, hex(col));
        for ly in (y + 3..y + h - 1).step_by(3) {
            c.hline(x + 2, ly, w - 4, hex(0x8a8070));
        }
        c.px(x + w / 2, y + 1, hex(0xc62828));
    }
    c.shade_box(0, 8, 64, 7, [0x5e231e, 0x8e3730, 0xa8433a, 0xc65a50]);
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 62.0, 28.0, 2.0);
}

fn fish_rack(c: &mut Canvas) {
    for x in [4, 56] {
        c.shade_box(x, 8, 4, 54, WOOD);
    }
    for y in [10, 32] {
        c.shade_box(2, y, 60, 3, WOOD);
        for k in 0..5 {
            let x = 12.0 + k as f32 * 10.0;
            c.vline(x as i32, y + 3, 2, hex(0xd8c8a0));
            c.shade_ellipse(x, y as f32 + 11.0, 2.5, 6.0, FISH);
            c.ellipse(x, y as f32 + 18.0, 3.0, 1.5, hex(FISH[1]));
        }
    }
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(32.0, 62.0, 28.0, 2.0);
}

fn fish_barrel(c: &mut Canvas) {
    c.shade_cyl(7, 10, 18, 20, WOOD);
    for y in [13, 25] {
        c.shade_box(7, y, 18, 2, IRON);
    }
    c.ellipse(16.0, 10.0, 9.0, 3.0, hex(WOOD[1]));
    for (x, y, a) in [(12.0, 6.0, 0.4f32), (18.0, 5.0, -0.3), (15.0, 8.0, 0.1)] {
        c.stroke(x, y + 4.0, x + a * 8.0, y - 3.0, 1.8, hex(FISH[2]));
        c.px((x + a * 8.0) as i32, (y - 4.0) as i32, hex(FISH[1]));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 11.0, 2.0);
}

fn laundry(c: &mut Canvas) {
    for x in [3, 58] {
        c.shade_box(x, 8, 3, 54, WOOD);
    }
    for x in 5..58 {
        let sag = 4.0 * (1.0 - ((x as f32 - 31.5) / 26.5).powi(2));
        c.px(x, 11 + sag as i32, hex(0xd8c8a0));
    }
    for (x, w, h, pal) in [(9, 14, 22, CREAM), (26, 12, 14, CLOTH_BLUE), (41, 12, 18, CLOTH_RED)] {
        let sag = 4.0 * (1.0 - ((x as f32 + w as f32 / 2.0 - 31.5) / 26.5).powi(2));
        c.shade_box(x, 12 + sag as i32, w, h, pal);
        c.px(x + 1, 11 + sag as i32, hex(0x8a6a4a));
        c.px(x + w - 2, 11 + sag as i32, hex(0x8a6a4a));
    }
    c.auto_outline(64, 64, OUTLINE);
    c.shadow(4.0, 62.0, 3.0, 1.0);
    c.shadow(59.0, 62.0, 3.0, 1.0);
}

fn boat(c: &mut Canvas) {
    c.shade_ellipse(32.0, 17.0, 30.0, 11.0, WOOD);
    c.ellipse(32.0, 15.0, 25.0, 6.5, hex(WOOD[0]));
    c.ellipse(32.0, 16.0, 23.0, 5.0, hex(0x5a3a1c));
    for x in [22, 40] {
        c.rect(x, 10, 5, 12, hex(WOOD[2]));
    }
    c.stroke(10.0, 26.0, 50.0, 8.0, 1.0, hex(WOOD[3]));
    c.auto_outline(64, 32, OUTLINE);
}

fn flour_sacks(c: &mut Canvas) {
    let sack = [0xb8ac94, 0xd8ccb0, 0xeee4cc, 0xfffaf0];
    for (x, y) in [(10.0, 22.0), (22.0, 22.0), (16.0, 12.0)] {
        c.shade_ellipse(x, y, 7.0, 8.0, sack);
        c.rect(x as i32 - 2, y as i32 - 9, 4, 2, hex(BURLAP[1]));
        c.hline(x as i32 - 4, y as i32 + 1, 8, hex(0x5a7ab0));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 14.0, 2.0);
}

fn grave_cross(c: &mut Canvas) {
    c.shade_ellipse(16.0, 27.0, 10.0, 3.5, [0x3e2616, 0x5a3a22, 0x7a5436, 0x8e6440]);
    c.shade_box(14, 4, 4, 23, WOOD);
    c.shade_box(8, 10, 16, 4, WOOD);
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 10.0, 1.5);
}

fn flower_pot(c: &mut Canvas) {
    for y in 18..30 {
        let half = 7 - (y - 18) / 4;
        c.hline(16 - half, y, 2 * half, hex(CLAY[if y < 20 { 3 } else { 2 }]));
    }
    c.rect(8, 17, 16, 3, hex(CLAY[1]));
    for (x, y, col) in [(11, 9, 0xf06292), (16, 6, 0xf4e04d), (21, 10, 0xf06292), (14, 12, 0xffffff)] {
        c.vline(x, y + 2, 17 - y, hex(0x2f6e2a));
        for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
            c.px(x + dx, y + dy, hex(col));
        }
        c.px(x, y, hex(0xfff3b0));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 8.0, 1.5);
}

fn flower_box(c: &mut Canvas) {
    c.shade_box(3, 18, 26, 11, WOOD);
    c.hline(3, 23, 26, hex(WOOD[1]));
    for k in 0..6 {
        let x = 6 + k * 4;
        let col = [0xf06292, 0xf4e04d, 0xffffff, 0x9fa8ff][k as usize % 4];
        c.vline(x, 12, 6, hex(0x2f6e2a));
        c.px(x - 1, 15, hex(0x43a047));
        c.disc(x as f32 + 0.5, 11.0, 1.8, hex(col));
    }
    c.auto_outline(32, 32, OUTLINE);
    c.shadow(16.0, 30.0, 13.0, 1.5);
}

// ------------------------------------------------------------------ signs

enum Emblem {
    Mug,
    Loaf,
    Anvil,
    Flask,
}

/// A sign hanging from an iron bracket (on a wall).
fn hanging_sign(c: &mut Canvas, e: Emblem) {
    c.shade_box(1, 3, 26, 3, IRON);
    c.stroke(2.0, 12.0, 12.0, 5.0, 0.8, hex(IRON[1]));
    for x in [7, 21] {
        c.vline(x, 6, 4, hex(IRON[2]));
    }
    c.shade_box(3, 10, 22, 17, WOOD);
    c.rect(5, 12, 18, 13, hex(WOOD[1]));
    match e {
        Emblem::Mug => {
            c.shade_box(9, 15, 8, 9, [0x8a5a10, 0xc08a20, 0xe8b040, 0xf8d878]);
            c.rect(8, 13, 10, 3, hex(0xfff8e8));
            c.rect(17, 17, 3, 4, hex(0xc08a20));
        }
        Emblem::Loaf => {
            c.shade_ellipse(14.0, 19.0, 7.0, 4.0, BREAD);
            c.line(11, 17, 13, 20, hex(0xf0d8a0));
            c.line(15, 17, 17, 20, hex(0xf0d8a0));
        }
        Emblem::Anvil => {
            c.rect(8, 15, 13, 3, hex(IRON[2]));
            c.rect(6, 15, 3, 2, hex(IRON[2]));
            c.rect(12, 18, 5, 3, hex(IRON[1]));
            c.rect(9, 21, 11, 2, hex(IRON[1]));
        }
        Emblem::Flask => {
            c.shade_disc(14.0, 20.0, 4.5, [0x1b5e20, 0x2e7d32, 0x43a047, 0xa5d6a7]);
            c.rect(13, 13, 3, 4, hex(0xcfe8ff));
        }
    }
    c.auto_outline(32, 32, OUTLINE);
}

// ------------------------------------------------------------------ herbs

fn leaf_tuft(c: &mut Canvas, cx: i32, base: i32, pal: [u32; 4], n: i32) {
    for k in 0..n {
        let a = -1.2 + 2.4 * k as f32 / (n - 1).max(1) as f32;
        let (tx, ty) = (cx as f32 + a.sin() * 9.0, base as f32 - a.cos() * 9.0);
        c.stroke(cx as f32, base as f32, tx, ty, 1.6, hex(pal[1 + (k % 2) as usize]));
        c.px(tx as i32, ty as i32, hex(pal[3]));
    }
}

fn moonpetal(c: &mut Canvas, bud: bool) {
    c.ellipse(16.0, 28.0, 7.0, 2.0, hex(0x3e2616));
    leaf_tuft(c, 16, 28, [0x1b3a2a, 0x2e5c44, 0x4a8a6a, 0x8ac8a8], 4);
    if bud {
        c.vline(16, 10, 18, hex(0x2e5c44));
        c.shade_ellipse(16.0, 9.0, 3.5, 5.0, [0x6a7ab8, 0x9aaee0, 0xc8d8f8, 0xf0f6ff]);
        c.px(16, 4, hex(0xffffff));
    }
    c.auto_outline(32, 32, 0x10202a);
    if bud {
        glow(c, 16.0, 9.0, 8.0, (0.6, 0.75, 1.0), 0.12);
    }
}

fn feverleaf(c: &mut Canvas, full: bool) {
    c.ellipse(16.0, 28.0, 8.0, 2.0, hex(0x3e2616));
    if full {
        for (x, h) in [(9, 14), (13, 19), (17, 22), (21, 17), (24, 12)] {
            for k in 0..h {
                let col = if k > h - 4 { 0xd04a2a } else if k % 3 == 0 { 0x7a8a2a } else { 0x5a7a2a };
                c.px(x + (k / 6) * if x < 16 { -1 } else { 1 }, 28 - k, hex(col));
                c.px(x + 1, 28 - k / 2, hex(0x4a6a24));
            }
        }
    } else {
        for x in [11, 16, 21] {
            c.vline(x, 24, 4, hex(0x5a7a2a));
        }
    }
    c.auto_outline(32, 32, 0x10301a);
    c.shadow(16.0, 30.0, 9.0, 1.5);
}

fn bitterroot(c: &mut Canvas, full: bool) {
    c.ellipse(16.0, 27.0, 9.0, 3.0, hex(0x4a2e1a));
    if full {
        for (x, y) in [(12.0, 27.0), (19.0, 28.0)] {
            c.shade_ellipse(x, y, 3.0, 2.0, [0x3a1a3a, 0x5a2a5a, 0x7a4a7a, 0xa87aa8]);
        }
        for (a, l) in [(-0.9f32, 11.0f32), (-0.3, 14.0), (0.3, 13.0), (0.9, 10.0)] {
            let (tx, ty) = (16.0 + a.sin() * l, 25.0 - a.cos() * l);
            c.stroke(16.0, 25.0, tx, ty, 2.6, hex(0x2e6a2a));
            c.stroke(16.0, 25.0, tx, ty, 1.0, hex(0x5a9a4a));
        }
    }
    c.auto_outline(32, 32, 0x10301a);
    c.shadow(16.0, 30.0, 10.0, 1.5);
}
