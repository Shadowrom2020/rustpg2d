//! Save game thumbnails: every save also writes `<slot>.png` next to `<slot>.ron`, a small
//! picture of the world at that moment (without HUD, dialogs and menus), shown on the Load
//! and Save pages. A save only asks for it ([`Game::request_thumbnail`]); the next frame's
//! drawing grabs the screen right after the world is drawn.

use macroquad::prelude::*;

use super::Game;
use crate::imageio::Rgba;

/// Thumbnail size (16:9).
pub(super) const THUMB_W: u32 = 192;
pub(super) const THUMB_H: u32 = 108;

/// What waits for the thumbnail before it happens (the world must still be on screen).
pub(super) enum AfterThumb {
    /// *Save and quit*: back to the title screen; the slot for the message.
    ToTitle(String),
}

impl Game {
    pub(super) fn request_thumbnail(&mut self, slot: &str) {
        self.thumb_request = Some(slot.to_string());
    }

    /// Called while drawing, after the world and before the HUD: writes the requested thumbnail.
    pub(super) fn capture_thumbnail(&mut self) {
        let Some(slot) = self.thumb_request.take() else { return };
        let shot = get_screen_data();
        let img = thumbnail(shot.width as u32, shot.height as u32, &shot.bytes);
        let path = self.project.saves_dir().join(format!("{slot}.png"));
        if let Err(e) = img.encode().and_then(|png| crate::sys::write(&path, &png).map_err(Into::into)) {
            eprintln!("[game] thumbnail {}: {e}", path.display());
        }
    }

    /// Runs what waited for the thumbnail (once it is written).
    pub(super) fn after_thumbnail(&mut self) {
        if self.thumb_request.is_some() {
            return;
        }
        if let Some(AfterThumb::ToTitle(slot)) = self.after_thumb.take() {
            self.quit_to_title();
            self.ui.toast(format!("Saved \"{}\"", super::menu::display_name(&slot)));
        }
    }
}

/// A screen grab (RGBA rows bottom-up, as OpenGL reads them) scaled to the thumbnail size:
/// the middle 16:9 part, each thumbnail pixel the average of its screen pixels.
pub(super) fn thumbnail(w: u32, h: u32, bytes: &[u8]) -> Rgba {
    let mut out = Rgba::new(THUMB_W, THUMB_H);
    if w == 0 || h == 0 || bytes.len() < (w * h * 4) as usize {
        return out;
    }
    // The widest 16:9 window in the middle of the screen.
    let (cw, ch) = if w * 9 >= h * 16 { (h * 16 / 9, h) } else { (w, w * 9 / 16) };
    let (x0, y0) = ((w - cw) / 2, (h - ch) / 2);
    for ty in 0..THUMB_H {
        let (sy0, sy1) = (y0 + ty * ch / THUMB_H, (y0 + (ty + 1) * ch / THUMB_H).max(y0 + ty * ch / THUMB_H + 1));
        for tx in 0..THUMB_W {
            let (sx0, sx1) = (x0 + tx * cw / THUMB_W, (x0 + (tx + 1) * cw / THUMB_W).max(x0 + tx * cw / THUMB_W + 1));
            let mut sum = [0u32; 3];
            let mut n = 0;
            for sy in sy0..sy1.min(h) {
                let row = (h - 1 - sy) as usize; // bottom-up rows
                for sx in sx0..sx1.min(w) {
                    let i = (row * w as usize + sx as usize) * 4;
                    for c in 0..3 {
                        sum[c] += bytes[i + c] as u32;
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            out.set(tx as i32, ty as i32, [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_flip_and_crop() {
        // A 32x9 "screen" (wider than 16:9): left half red, right half blue; the bottom-up rows
        // put the green row (stored first) at the bottom of the picture.
        let (w, h) = (32u32, 9u32);
        let mut bytes = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let c = if y == 0 { [0, 255, 0] } else if x < w / 2 { [255, 0, 0] } else { [0, 0, 255] };
                bytes[i..i + 3].copy_from_slice(&c);
                bytes[i + 3] = 255;
            }
        }
        let t = thumbnail(w, h, &bytes);
        assert_eq!((t.w, t.h), (THUMB_W, THUMB_H));
        assert_eq!(t.get(0, 0), [255, 0, 0, 255], "the cropped middle starts in the red half");
        assert_eq!(t.get(THUMB_W as i32 - 1, 0), [0, 0, 255, 255]);
        assert_eq!(t.get(0, THUMB_H as i32 - 1), [0, 255, 0, 255], "row 0 of the grab is the bottom");
        // Empty or short data gives a blank thumbnail instead of a panic.
        assert_eq!(thumbnail(0, 0, &[]).w, THUMB_W);
    }
}
