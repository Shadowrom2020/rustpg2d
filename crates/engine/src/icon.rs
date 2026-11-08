//! Icons of games and of R(ust)PG2D: the player character drawn large ([`hero_icon`]), a
//! fallback emblem, and window icons made from them (the title bar and taskbar).

use crate::entity::Facing;
use crate::imageio::Rgba;
use crate::project::Project;
use crate::render::{character, recolor};

/// Size of the package icons (pixels, square).
pub const ICON: u32 = 256;

/// The player character facing the viewer (layers composed like in the game), enlarged with
/// sharp pixels onto a 256 x 256 transparent square.
pub fn hero_icon(project: &Project) -> Option<Rgba> {
    let db = &project.db;
    let d = db.character(&project.config.player)?;
    let mut canvas: Option<Rgba> = None;
    for l in character::layers(db, &d.appearance, &d.equipment, Facing::Down, 0, false) {
        let Some(sheet) = db.sheet(&l.sheet) else { continue };
        let Ok(img) = std::fs::read(project.path(&sheet.image)).map_err(anyhow::Error::from).and_then(|b| Rgba::decode(&b)) else { continue };
        let (fw, fh) = (sheet.frame_w.max(1), sheet.frame_h.max(1));
        let cols = (img.w / fw).max(1);
        let (fx, fy) = ((l.frame % cols * fw) as i32, (l.frame / cols * fh) as i32);
        let mut frame = Rgba::new(fw, fh);
        for y in 0..fh as i32 {
            for x in 0..fw as i32 {
                frame.set(x, y, img.get(fx + x, fy + y));
            }
        }
        recolor::recolor(&mut frame.px, l.skin, l.hair);
        let c = canvas.get_or_insert_with(|| Rgba::new(fw, fh));
        // Layers of another frame size stand on the same ground, centered.
        let (ox, oy) = ((c.w as i32 - fw as i32) / 2, c.h as i32 - fh as i32);
        for y in 0..fh as i32 {
            for x in 0..fw as i32 {
                over(c, ox + x, oy + y, frame.get(x, y));
            }
        }
    }
    let c = canvas?;
    // Trim to the drawn pixels.
    let opaque: Vec<(i32, i32)> = (0..c.h as i32).flat_map(|y| (0..c.w as i32).map(move |x| (x, y))).filter(|&(x, y)| c.get(x, y)[3] > 0).collect();
    let (x0, x1) = (opaque.iter().map(|p| p.0).min()?, opaque.iter().map(|p| p.0).max()?);
    let (y0, y1) = (opaque.iter().map(|p| p.1).min()?, opaque.iter().map(|p| p.1).max()?);
    let (w, h) = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
    let scale = ((ICON - 16) / w.max(h)).max(1);
    let mut out = Rgba::new(ICON, ICON);
    let (ox, oy) = ((ICON - w * scale) as i32 / 2, (ICON - h * scale) as i32 / 2);
    for y in 0..(h * scale) as i32 {
        for x in 0..(w * scale) as i32 {
            out.set(ox + x, oy + y, c.get(x0 + x / scale as i32, y0 + y / scale as i32));
        }
    }
    Some(out)
}

/// Fallback icon (no player character): a sword on a dark shield.
pub fn emblem() -> Rgba {
    let mut img = Rgba::new(ICON, ICON);
    let c = ICON as f32 / 2.0;
    for y in 0..ICON as i32 {
        for x in 0..ICON as i32 {
            let (dx, dy) = ((x as f32 - c) / c, (y as f32 - c) / c);
            // A shield: flat top, round bottom.
            let inside = dx.abs() < 0.8 && dy > -0.85 && (dy < 0.1 || dx * dx + (dy - 0.1) * (dy - 0.1) * 1.6 < 0.64);
            let edge = dx.abs() < 0.86 && dy > -0.92 && (dy < 0.1 || dx * dx + (dy - 0.1) * (dy - 0.1) * 1.6 < 0.74);
            if inside {
                img.set(x, y, [0x1b, 0x2a, 0x55, 255]);
            } else if edge {
                img.set(x, y, [0xe8, 0xc0, 0x5a, 255]);
            }
        }
    }
    let rect = |img: &mut Rgba, x: i32, y: i32, w: i32, h: i32, col: [u8; 4]| {
        for yy in y..y + h {
            for xx in x..x + w {
                img.set(xx, yy, col);
            }
        }
    };
    rect(&mut img, 120, 52, 16, 120, [0xd8, 0xdc, 0xe8, 255]); // blade
    rect(&mut img, 126, 52, 4, 120, [0xff, 0xff, 0xff, 255]);
    rect(&mut img, 92, 168, 72, 12, [0xe8, 0xc0, 0x5a, 255]); // guard
    rect(&mut img, 122, 180, 12, 28, [0x7a, 0x54, 0x10, 255]); // grip
    rect(&mut img, 116, 206, 24, 12, [0xe8, 0xc0, 0x5a, 255]); // pommel
    img
}

/// Alpha-composites `c` over the pixel at x, y.
fn over(img: &mut Rgba, x: i32, y: i32, c: [u8; 4]) {
    if c[3] == 0 {
        return;
    }
    let d = img.get(x, y);
    let a = c[3] as f32 / 255.0;
    let da = d[3] as f32 / 255.0 * (1.0 - a);
    let oa = a + da;
    let mix = |s: u8, t: u8| ((s as f32 * a + t as f32 * da) / oa).round() as u8;
    img.set(x, y, [mix(c[0], d[0]), mix(c[1], d[1]), mix(c[2], d[2]), (oa * 255.0).round() as u8]);
}

/// Windows: the game and the editor are GUI programs (no console window of their own); started
/// from a terminal, their output (logs, `--help`, automated runs' `status`) goes there. Output
/// that is already redirected (the editor reading `--engine-build`) stays where it is. Nothing
/// elsewhere.
pub fn attach_parent_console() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(process: u32) -> i32;
            fn GetStdHandle(which: u32) -> isize;
        }
        const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
        const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
        // SAFETY: plain Win32 calls without pointers.
        unsafe {
            let out = GetStdHandle(STD_OUTPUT_HANDLE);
            if out == 0 || out == -1 {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    }
}

/// A window icon (16, 32 and 64 px) from a square image: each pixel averages the area it covers
/// (alpha-weighted, so transparent pixels don't darken the edges).
pub fn window_icon(img: &Rgba) -> macroquad::miniquad::conf::Icon {
    fn scaled<const N: usize>(img: &Rgba, size: u32) -> [u8; N] {
        let mut out = [0u8; N];
        let (fx, fy) = (img.w as f32 / size as f32, img.h as f32 / size as f32);
        for y in 0..size {
            for x in 0..size {
                let (x0, x1) = ((x as f32 * fx) as i32, (((x + 1) as f32 * fx) as i32).max(x as i32 * fx as i32 + 1));
                let (y0, y1) = ((y as f32 * fy) as i32, (((y + 1) as f32 * fy) as i32).max(y as i32 * fy as i32 + 1));
                let mut acc = [0f32; 4];
                let mut n = 0.0f32;
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        let p = img.get(sx, sy);
                        let a = p[3] as f32 / 255.0;
                        for c in 0..3 {
                            acc[c] += p[c] as f32 * a;
                        }
                        acc[3] += a;
                        n += 1.0;
                    }
                }
                let i = ((y * size + x) * 4) as usize;
                if acc[3] > 0.0 {
                    for c in 0..3 {
                        out[i + c] = (acc[c] / acc[3]).round() as u8;
                    }
                }
                out[i + 3] = (acc[3] / n.max(1.0) * 255.0).round() as u8;
            }
        }
        out
    }
    macroquad::miniquad::conf::Icon { small: scaled::<{ 16 * 16 * 4 }>(img, 16), medium: scaled::<{ 32 * 32 * 4 }>(img, 32), big: scaled::<{ 64 * 64 * 4 }>(img, 64) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_icons_shrink_the_emblem() {
        let icon = window_icon(&emblem());
        // The middle of the 64 px icon is the shield / sword (opaque), a corner transparent.
        let at = |x: usize, y: usize| &icon.big[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(32, 32)[3], 255);
        assert_eq!(at(0, 0)[3], 0);
        assert!(icon.small.iter().any(|&v| v > 0) && icon.medium.iter().any(|&v| v > 0));
    }
}
