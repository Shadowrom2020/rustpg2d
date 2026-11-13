//! Icons of games and of R(ust)PG2D: the game's icon file or the player's portrait drawn large
//! ([`hero_icon`]), a fallback emblem, and window icons made from them (the title bar and
//! taskbar).

use crate::imageio::Rgba;
use crate::project::Project;

/// Size of the package icons (pixels, square).
pub const ICON: u32 = 256;

/// The game's own icon file: exports write the icon their packages got (a custom one, the
/// player's portrait or the player model's head and shoulders) as `icon.png` beside
/// `project.ron`, so the game window shows the same.
pub const ICON_FILE: &str = "icon.png";

/// A game's icon: its `icon.png` (any size), else the player's portrait picture
/// (`ProjectConfig::player_portrait`) enlarged with sharp pixels onto a 256 x 256 transparent
/// square; None without either.
pub fn hero_icon(project: &Project) -> Option<Rgba> {
    if let Some(i) = crate::sys::read(&project.path(ICON_FILE)).ok().and_then(|b| Rgba::decode(&b).ok()).filter(|i| i.w > 0 && i.h > 0) {
        return Some(i);
    }
    let db = &project.db;
    let sprite = &project.config.player_portrait;
    let sheet = db.sheet(&sprite.sheet)?;
    let img = crate::sys::read(&project.path(&sheet.image)).ok().and_then(|b| Rgba::decode(&b).ok())?;
    let (fw, fh) = (sheet.frame_w.max(1), sheet.frame_h.max(1));
    let (cw, ch) = sprite.cells();
    let cols = (img.w / fw).max(1);
    let (fx, fy) = ((sprite.frame % cols * fw) as i32, (sprite.frame / cols * fh) as i32);
    let mut canvas: Option<Rgba> = None;
    {
        let (w, h) = (fw * cw, fh * ch);
        let c = canvas.get_or_insert_with(|| Rgba::new(w, h));
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                c.set(x, y, img.get(fx + x, fy + y));
            }
        }
    }
    fit_icon(&canvas?)
}

/// A picture trimmed to its drawn pixels and enlarged with sharp pixels (a whole factor) onto a
/// 256 x 256 transparent square; None when nothing is drawn.
pub fn fit_icon(c: &Rgba) -> Option<Rgba> {
    // Trim to the drawn pixels.
    let opaque: Vec<(i32, i32)> = (0..c.h as i32).flat_map(|y| (0..c.w as i32).map(move |x| (x, y))).filter(|&(x, y)| c.get(x, y)[3] > 0).collect();
    let (x0, x1) = (opaque.iter().map(|p| p.0).min()?, opaque.iter().map(|p| p.0).max()?);
    let (y0, y1) = (opaque.iter().map(|p| p.1).min()?, opaque.iter().map(|p| p.1).max()?);
    let (w, h) = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
    if w.max(h) > ICON - 16 {
        // Too big to enlarge: the whole picture shrunk onto the square instead.
        let k = (ICON - 16) as f32 / w.max(h) as f32;
        let (sw, sh) = (((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1));
        let mut crop = Rgba::new(w, h);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                crop.set(x, y, c.get(x0 + x, y0 + y));
            }
        }
        let small = crop.resized(sw, sh);
        let mut out = Rgba::new(ICON, ICON);
        let (ox, oy) = ((ICON - sw) as i32 / 2, (ICON - sh) as i32 / 2);
        for y in 0..sh as i32 {
            for x in 0..sw as i32 {
                out.set(ox + x, oy + y, small.get(x, y));
            }
        }
        return Some(out);
    }
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
