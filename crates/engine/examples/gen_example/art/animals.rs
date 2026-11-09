//! Village animals (chicken, dog, cats, sheep, cow) and children: whole-figure body sheets in
//! the character layout (rows = facing down, left, right, up; four walk frames each).

use super::*;

/// One animal: sheet id, frame size, how a frame is drawn.
pub struct Animal {
    pub id: &'static str,
    pub size: u32,
    draw: fn(&mut Canvas, Facing, u32),
}

pub const ANIMALS: [Animal; 6] = [
    Animal { id: "chicken", size: 32, draw: chicken },
    Animal { id: "dog", size: 32, draw: |c, f, k| quadruped(c, f, k, &DOG) },
    Animal { id: "cat", size: 32, draw: |c, f, k| quadruped(c, f, k, &CAT_GREY) },
    Animal { id: "cat_ginger", size: 32, draw: |c, f, k| quadruped(c, f, k, &CAT_GINGER) },
    Animal { id: "sheep", size: 32, draw: |c, f, k| quadruped(c, f, k, &SHEEP) },
    Animal { id: "cow", size: 48, draw: |c, f, k| quadruped(c, f, k, &COW) },
];

/// The animal's sheet.
pub fn sheet(a: &Animal) -> Canvas {
    let s = a.size;
    let mut c = Canvas::new(s * 4, s * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * s, f as u32 * s);
            (a.draw)(&mut c, f, frame);
            c.auto_outline(s as i32, s as i32, OUTLINE);
            c.shadow(s as f32 / 2.0, s as f32 - 2.5, s as f32 * 0.32, 2.0);
        }
    }
    c
}

/// Leg lift of walk frame `frame` for the front / hind pair.
fn steps(frame: u32) -> (i32, i32) {
    match frame {
        1 => (2, 0),
        3 => (0, 2),
        _ => (0, 0),
    }
}

fn chicken(c: &mut Canvas, f: Facing, frame: u32) {
    let white = [0xb8b0a0, 0xdcd4c4, 0xf2ece0, 0xffffff];
    let (a, b) = steps(frame);
    let bob = (frame % 2) as f32;
    let leg = |c: &mut Canvas, x: i32, lift: i32| {
        c.vline(x, 24 - lift, 6, hex(0xe0a020));
        c.hline(x - 1, 29 - lift, 3, hex(0xe0a020));
    };
    match f {
        Facing::Left | Facing::Right => {
            let d = if f == Facing::Right { 1.0 } else { -1.0 };
            let x = |v: f32| 16.0 + d * v;
            leg(c, x(-2.0) as i32, a);
            leg(c, x(2.0) as i32, b);
            c.shade_ellipse(x(-6.0), 14.0, 3.5, 5.0, white);
            c.shade_ellipse(16.0, 19.0 + bob * 0.5, 8.0, 6.0, white);
            c.shade_disc(x(6.0), 11.0 + bob, 3.8, white);
            c.rect(x(5.0) as i32 - 1, 6 + bob as i32, 3, 2, hex(0xd83030));
            c.rect(if d > 0.0 { x(9.0) as i32 } else { x(9.0) as i32 - 2 }, 11 + bob as i32, 3, 2, hex(0xf0b020));
            c.px(x(7.0) as i32, 10 + bob as i32, hex(0x111111));
            c.px(x(7.0) as i32, 13 + bob as i32, hex(0xd83030));
        }
        Facing::Down | Facing::Up => {
            leg(c, 13, a);
            leg(c, 19, b);
            c.shade_ellipse(16.0, 19.0, 7.5, 6.5, white);
            c.shade_disc(16.0, 11.0 + bob, 4.0, white);
            c.rect(15, 6 + bob as i32, 3, 2, hex(0xd83030));
            if f == Facing::Down {
                c.rect(15, 12 + bob as i32, 3, 2, hex(0xf0b020));
                c.px(14, 10 + bob as i32, hex(0x111111));
                c.px(18, 10 + bob as i32, hex(0x111111));
            } else {
                c.shade_ellipse(16.0, 15.0, 3.5, 3.0, white);
            }
        }
    }
}

/// How a four-legged animal looks.
struct Quad {
    /// Body: half length, half height (side view), colours.
    body: (f32, f32),
    coat: [u32; 4],
    /// Patches on the coat (side view offsets from the body centre, radius).
    patches: &'static [(f32, f32, f32)],
    patch: u32,
    head: f32,
    face: [u32; 4],
    legs: u32,
    leg_len: i32,
    ears: Ears,
    tail: Tail,
    /// Wool: the body is drawn as a cloud.
    woolly: bool,
    horns: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Ears {
    Pointy,
    Floppy,
    Side,
}

#[derive(Clone, Copy, PartialEq)]
enum Tail {
    Up,
    Curl,
    Short,
    Hang,
}

const DOG: Quad = Quad { body: (8.5, 5.0), coat: [0x5a3414, 0x8a5428, 0xb07a40, 0xd8a868], patches: &[], patch: 0, head: 5.0, face: [0x5a3414, 0x8a5428, 0xb07a40, 0xd8a868], legs: 0x8a5428, leg_len: 7, ears: Ears::Floppy, tail: Tail::Up, woolly: false, horns: false };
const CAT_GREY: Quad = Quad { body: (7.0, 4.0), coat: [0x3a3a44, 0x5a5a66, 0x80808c, 0xa8a8b4], patches: &[(-3.0, -1.0, 2.0), (2.0, -1.5, 1.6)], patch: 0x4a4a54, head: 4.2, face: [0x3a3a44, 0x5a5a66, 0x80808c, 0xa8a8b4], legs: 0x5a5a66, leg_len: 6, ears: Ears::Pointy, tail: Tail::Curl, woolly: false, horns: false };
const CAT_GINGER: Quad = Quad { body: (7.0, 4.0), coat: [0x8a3a08, 0xc86418, 0xe8903a, 0xffc078], patches: &[(-3.0, -1.0, 2.0), (2.0, -1.5, 1.6)], patch: 0xb05410, head: 4.2, face: [0x8a3a08, 0xc86418, 0xe8903a, 0xffc078], legs: 0xc86418, leg_len: 6, ears: Ears::Pointy, tail: Tail::Curl, woolly: false, horns: false };
const SHEEP: Quad = Quad { body: (8.5, 6.0), coat: [0xb8b0a0, 0xdcd4c4, 0xf2ece0, 0xffffff], patches: &[], patch: 0, head: 4.0, face: [0x1a1a1e, 0x2a2a30, 0x44444c, 0x6a6a74], legs: 0x2a2a30, leg_len: 6, ears: Ears::Side, tail: Tail::Short, woolly: true, horns: false };
const COW: Quad = Quad { body: (14.0, 8.0), coat: [0xb8b0a0, 0xdcd4c4, 0xf2ece0, 0xffffff], patches: &[(-5.0, -2.0, 4.5), (5.0, 1.0, 3.5), (9.0, -4.0, 2.5)], patch: 0x2a2a30, head: 6.0, face: [0x8a8478, 0xb8b0a0, 0xdcd4c4, 0xf2ece0], legs: 0xdcd4c4, leg_len: 10, ears: Ears::Side, tail: Tail::Hang, woolly: false, horns: true };

fn quadruped(c: &mut Canvas, f: Facing, frame: u32, q: &Quad) {
    let s = if q.body.0 > 10.0 { 48.0 } else { 32.0 };
    let ground = s - 3.0;
    let (a, b) = steps(frame);
    let (bl, bh) = q.body;
    let cy = ground - q.leg_len as f32 - bh * 0.6;
    let body = |c: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32| {
        if q.woolly {
            for (dx, dy) in [(-0.55, -0.3), (0.0, -0.45), (0.55, -0.3), (-0.6, 0.25), (0.0, 0.2), (0.6, 0.25)] {
                c.shade_ellipse(cx + dx * rx, cy + dy * ry, rx * 0.5, ry * 0.6, q.coat);
            }
        } else {
            c.shade_ellipse(cx, cy, rx, ry, q.coat);
        }
    };
    let leg = |c: &mut Canvas, x: f32, lift: i32| {
        let w = if s > 40.0 { 3 } else { 2 };
        c.rect(x as i32, (cy + bh * 0.4) as i32 - lift, w, q.leg_len + 1, hex(q.legs));
        c.rect(x as i32, ground as i32 - lift, w, 1, hex(0x2a2018));
    };
    match f {
        Facing::Left | Facing::Right => {
            let d = if f == Facing::Right { 1.0 } else { -1.0 };
            let cx = s / 2.0 - d * 1.0;
            let x = |v: f32| cx + d * v;
            // Far legs (darker), body, near legs.
            leg(c, x(bl * 0.55) + 1.0, b);
            leg(c, x(-bl * 0.55) + 1.0, a);
            match q.tail {
                Tail::Up => c.stroke(x(-bl), cy - 1.0, x(-bl - 4.0), cy - 7.0, 1.2, hex(q.coat[1])),
                Tail::Curl => {
                    c.stroke(x(-bl), cy, x(-bl - 4.0), cy - 4.0, 1.0, hex(q.coat[1]));
                    c.stroke(x(-bl - 4.0), cy - 4.0, x(-bl - 3.0), cy - 9.0, 1.0, hex(q.coat[1]));
                }
                Tail::Short => c.shade_disc(x(-bl - 1.0), cy - 1.0, 2.0, q.coat),
                Tail::Hang => {
                    c.stroke(x(-bl), cy - 3.0, x(-bl - 2.0), cy + 6.0, 0.9, hex(q.coat[0]));
                    c.disc(x(-bl - 2.0), cy + 7.0, 1.5, hex(0x2a2a30));
                }
            }
            body(c, cx, cy, bl, bh);
            for &(px, py, r) in q.patches {
                c.ellipse(x(px), cy + py, r, r * 0.8, hex(q.patch));
            }
            leg(c, x(bl * 0.55), a);
            leg(c, x(-bl * 0.55), b);
            if q.horns && s > 40.0 {
                c.ellipse(cx + d, cy + bh * 0.75, 3.0, 2.0, hex(0xf0a0a8));
            }
            // The head in front, a little higher.
            let (hx, hy) = (x(bl + q.head * 0.6), cy - bh * 0.6);
            c.shade_disc(hx, hy, q.head, q.face);
            match q.ears {
                Ears::Pointy => {
                    for e in [-0.4, 0.5] {
                        let ex = hx + d * e * q.head;
                        for k in 0..3 {
                            c.hline((ex - 1.0) as i32 + k / 2, (hy - q.head) as i32 - 3 + k, 3 - k / 2 * 2 + 1, hex(q.face[1]));
                        }
                    }
                }
                Ears::Floppy => c.shade_ellipse(hx - d * q.head * 0.5, hy, 2.0, 3.5, [q.coat[0], q.coat[0], q.coat[1], q.coat[1]]),
                Ears::Side => c.ellipse(hx - d * q.head * 0.7, hy - q.head * 0.5, 2.5, 1.5, hex(q.face[1])),
            }
            if q.horns {
                c.stroke(hx - d, hy - q.head, hx + d * 2.0, hy - q.head - 3.0, 0.8, hex(0xe8e0c8));
            }
            // Snout and eye.
            let sn = if q.horns { 0xf0a0a8 } else if q.woolly { q.face[2] } else { q.face[3] };
            c.ellipse(hx + d * q.head * 0.8, hy + q.head * 0.3, q.head * 0.45, q.head * 0.35, hex(sn));
            c.px((hx + d * q.head * 1.1) as i32, (hy + q.head * 0.2) as i32, hex(0x111111));
            c.px((hx + d * q.head * 0.3) as i32, (hy - q.head * 0.25) as i32, hex(if q.woolly { 0xffffff } else { 0x111111 }));
        }
        Facing::Down | Facing::Up => {
            let cx = s / 2.0;
            let (rx, ry) = (bh * 1.15, bl * 0.75);
            let by = ground - q.leg_len as f32 - ry * 0.4;
            for (k, lx) in [-0.55f32, 0.25].into_iter().enumerate() {
                leg(c, cx + lx * rx, if k == 0 { a } else { b });
            }
            if f == Facing::Up {
                body(c, cx, by - 1.0, rx, ry);
                match q.tail {
                    Tail::Up | Tail::Curl => c.stroke(cx, by + ry * 0.4, cx + 2.0, by - ry - 2.0, 1.1, hex(q.coat[1])),
                    Tail::Short => c.shade_disc(cx, by + ry * 0.3, 2.0, q.coat),
                    Tail::Hang => c.stroke(cx, by, cx, by + ry + 2.0, 0.8, hex(q.coat[0])),
                }
                let hy = by - ry - q.head * 0.3;
                c.shade_disc(cx, hy, q.head, q.face);
                if q.ears == Ears::Pointy {
                    for ex in [cx - q.head * 0.6, cx + q.head * 0.6] {
                        c.rect(ex as i32 - 1, (hy - q.head) as i32 - 2, 3, 3, hex(q.face[1]));
                    }
                } else {
                    c.ellipse(cx - q.head, hy, 2.0, 1.5, hex(q.face[1]));
                    c.ellipse(cx + q.head, hy, 2.0, 1.5, hex(q.face[1]));
                }
            } else {
                body(c, cx, by - ry * 0.5, rx, ry);
                for &(px, py, r) in q.patches.iter().take(1) {
                    c.ellipse(cx + px * 0.5, by - ry * 0.5 + py, r * 0.8, r * 0.7, hex(q.patch));
                }
                let hy = by + q.head * 0.1;
                if q.ears == Ears::Floppy {
                    for ex in [cx - q.head, cx + q.head] {
                        c.shade_ellipse(ex, hy, 2.0, 3.5, [q.coat[0], q.coat[0], q.coat[1], q.coat[1]]);
                    }
                }
                c.shade_disc(cx, hy, q.head, q.face);
                match q.ears {
                    Ears::Pointy => {
                        for ex in [cx - q.head * 0.6, cx + q.head * 0.6] {
                            for k in 0..3 {
                                c.hline(ex as i32 - 1 + k / 2, (hy - q.head) as i32 - 3 + k, 3 - k / 2 * 2 + 1, hex(q.face[1]));
                            }
                        }
                    }
                    Ears::Side => {
                        c.ellipse(cx - q.head - 1.5, hy - 1.0, 2.5, 1.5, hex(q.face[1]));
                        c.ellipse(cx + q.head + 1.5, hy - 1.0, 2.5, 1.5, hex(q.face[1]));
                    }
                    Ears::Floppy => {}
                }
                if q.horns {
                    c.stroke(cx - q.head * 0.6, hy - q.head, cx - q.head - 2.0, hy - q.head - 3.0, 0.8, hex(0xe8e0c8));
                    c.stroke(cx + q.head * 0.6, hy - q.head, cx + q.head + 2.0, hy - q.head - 3.0, 0.8, hex(0xe8e0c8));
                }
                let sn = if q.horns { 0xf0a0a8 } else if q.woolly { q.face[2] } else { q.face[3] };
                c.ellipse(cx, hy + q.head * 0.45, q.head * 0.5, q.head * 0.35, hex(sn));
                c.px(cx as i32, (hy + q.head * 0.3) as i32, hex(0x111111));
                let eye = if q.woolly { 0xffffff } else { 0x111111 };
                c.px((cx - q.head * 0.45) as i32, (hy - q.head * 0.2) as i32, hex(eye));
                c.px((cx + q.head * 0.45) as i32, (hy - q.head * 0.2) as i32, hex(eye));
            }
            for (k, lx) in [-0.25f32, 0.5].into_iter().enumerate() {
                if f == Facing::Down {
                    leg(c, cx + lx * rx - 1.0, if k == 0 { b } else { a });
                }
            }
        }
    }
}

// ------------------------------------------------------------------ children

/// How a child looks.
pub struct Child {
    pub id: &'static str,
    skin: [u32; 4],
    hair: [u32; 4],
    shirt: [u32; 4],
    legs: u32,
    /// A dress instead of a shirt and shorts, long hair with two braids.
    girl: bool,
}

pub const CHILDREN: [Child; 2] = [
    Child { id: "child_boy", skin: [0x9c6b48, 0xc98f63, 0xe8b88a, 0xf6d3b0], hair: [0x3a2010, 0x5a3418, 0x7a4a22, 0x9a6232], shirt: [0x1a3a7a, 0x2a5aa8, 0x4a80d0, 0x8ab0f0], legs: 0x5a4030, girl: false },
    Child { id: "child_girl", skin: [0x8a5a3b, 0xb07a52, 0xd09068, 0xe8b088], hair: [0x5a1a0a, 0x8a3014, 0xb04a20, 0xd87038], shirt: [0x8a6a10, 0xc8a020, 0xf0c840, 0xfff0a0], legs: 0xd09068, girl: true },
];

/// A child's sheet (32 x 48 frames like grown-ups; the figure is smaller, the head larger).
pub fn child_sheet(k: &Child) -> Canvas {
    let mut c = Canvas::new(FW * 4, FH * 4);
    for f in Facing::ALL {
        for frame in 0..4u32 {
            c.at(frame * FW, f as u32 * FH);
            draw_child(&mut c, k, f, frame);
            c.auto_outline(FW as i32, FH as i32, OUTLINE);
            c.shadow(16.0, 45.5, 8.0, 2.0);
        }
    }
    c
}

fn draw_child(c: &mut Canvas, k: &Child, f: Facing, frame: u32) {
    let (bob, l, r) = walk(frame);
    let top = 14 + bob;
    let side = matches!(f, Facing::Left | Facing::Right);
    let d = if f == Facing::Left { -1 } else { 1 };
    // Legs (or the dress hem and legs), feet at the bottom.
    let shoe = 0x3a2418;
    if side {
        for (x, lift) in [(14, l), (17, r)] {
            c.rect(x, 37 - lift, 3, 8, hex(k.legs));
            c.rect(x + if d > 0 { 0 } else { -1 }, 44 - lift, 4, 2, hex(shoe));
        }
    } else {
        for (x, lift) in [(12, l), (17, r)] {
            c.rect(x, 37 - lift, 3, 8, hex(k.legs));
            c.rect(x, 44 - lift, 3, 2, hex(shoe));
        }
    }
    // Torso: a shirt and shorts, or a dress.
    if k.girl {
        for y in 0..12 {
            let half = 5 + y / 3;
            c.hline(16 - half, top + 15 + y, 2 * half, hex(k.shirt[if y < 2 { 3 } else if y > 9 { 1 } else { 2 }]));
        }
        c.hline(10, top + 26, 12, hex(0xffffff));
    } else {
        c.shade_box(11, top + 15, 10, 9, k.shirt);
        c.shade_box(11, top + 23, 10, 4, [0x2a2018, 0x3a2e22, 0x5a4030, 0x7a5a44]);
    }
    // Arms swinging at the sides (front / back), one arm in front (sideways).
    let swing = match frame {
        1 => 1,
        3 => -1,
        _ => 0,
    };
    if side {
        c.rect(15 + swing * d, top + 16, 3, 8, hex(k.shirt[1]));
        c.rect(15 + swing * d, top + 24, 3, 2, hex(k.skin[2]));
    } else {
        for (x, s) in [(8, swing), (21, -swing)] {
            c.rect(x, top + 16 + s, 3, 7, hex(k.shirt[1]));
            c.rect(x, top + 23 + s, 3, 2, hex(k.skin[2]));
        }
    }
    // The head: big and round.
    let (hx, hy) = (16.0 + if side { d as f32 } else { 0.0 }, top as f32 + 7.0);
    if k.girl && f != Facing::Up {
        // Braids behind the head.
        for bx in [9.0, 23.0] {
            if !side || (bx < 16.0) == (d > 0) {
                c.shade_ellipse(bx, hy + 6.0, 2.5, 6.0, k.hair);
            }
        }
    }
    c.shade_disc(hx, hy, 7.5, k.skin);
    match f {
        Facing::Up => {
            c.shade_disc(hx, hy - 0.5, 7.8, k.hair);
            if k.girl {
                for bx in [9.0, 23.0] {
                    c.shade_ellipse(bx, hy + 6.0, 2.5, 6.0, k.hair);
                }
            }
        }
        Facing::Down => {
            c.shade_ellipse(hx, hy - 4.5, 8.0, 4.5, k.hair);
            c.rect(9, top + 2, 2, 7, hex(k.hair[1]));
            c.rect(21, top + 2, 2, 7, hex(k.hair[1]));
            for ex in [12, 18] {
                c.rect(ex, top + 7, 2, 3, hex(0x2a1a10));
                c.px(ex, top + 7, hex(0xffffff));
            }
            c.px(10, top + 11, hex(0xe8907a));
            c.px(21, top + 11, hex(0xe8907a));
            c.hline(15, top + 12, 3, hex(0xa04848));
        }
        _ => {
            c.shade_ellipse(hx - d as f32 * 1.5, hy - 4.0, 7.5, 5.0, k.hair);
            c.rect(if d > 0 { 9 } else { 20 }, top + 2, 3, 8, hex(k.hair[1]));
            let ex = if d > 0 { 19 } else { 12 };
            c.rect(ex, top + 7, 2, 3, hex(0x2a1a10));
            c.px(ex + if d > 0 { 3 } else { -2 }, top + 11, hex(0xa04848));
        }
    }
}
