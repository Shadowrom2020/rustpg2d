//! Operations on the frames of a sprite sheet image: the sprite editor's buttons and the
//! `edit_frames` agent tool use them. Frames are numbered row by row.

use rustpg2d_engine::imageio::Rgba;

/// An image cut into frames.
pub struct Frames<'a> {
    pub img: &'a mut Rgba,
    pub frame: (u32, u32),
}

impl Frames<'_> {
    pub fn cols(&self) -> u32 {
        (self.img.w / self.frame.0.max(1)).max(1)
    }
    pub fn rows(&self) -> u32 {
        (self.img.h / self.frame.1.max(1)).max(1)
    }
    fn origin(&self, f: u32) -> (i32, i32) {
        ((f % self.cols() * self.frame.0) as i32, (f / self.cols() * self.frame.1) as i32)
    }

    /// A copy of frame `f`.
    pub fn get(&self, f: u32) -> Rgba {
        let (ox, oy) = self.origin(f);
        let mut out = Rgba::new(self.frame.0, self.frame.1);
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                out.set(x, y, self.img.get(ox + x, oy + y));
            }
        }
        out
    }

    /// Replaces frame `f` by `src` (mirrored when `flip`); transparent pixels of `src` clear.
    pub fn put(&mut self, f: u32, src: &Rgba, flip: bool) {
        let (ox, oy) = self.origin(f);
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                let sx = if flip { src.w as i32 - 1 - x } else { x };
                self.img.set(ox + x, oy + y, src.get(sx, y));
            }
        }
    }

    /// The frames of row `r`.
    pub fn row(&self, r: u32) -> Vec<Rgba> {
        (0..self.cols()).map(|c| self.get(r * self.cols() + c)).collect()
    }

    /// Puts `frames` into row `r` (as many as fit).
    pub fn put_row(&mut self, r: u32, frames: &[Rgba]) {
        let cols = self.cols();
        for (c, f) in frames.iter().enumerate().take(cols as usize) {
            self.put(r * cols + c as u32, f, false);
        }
    }

    pub fn clear_row(&mut self, r: u32) {
        let empty = vec![Rgba::new(self.frame.0, self.frame.1); self.cols() as usize];
        self.put_row(r, &empty);
    }

    /// Frame `f` copied into the next column (the last column has none: false).
    pub fn duplicate(&mut self, f: u32) -> bool {
        if f % self.cols() + 1 >= self.cols() {
            return false;
        }
        let src = self.get(f);
        self.put(f + 1, &src, false);
        true
    }

    /// Shifts the contents of frame `f` by (dx, dy) pixels (what moves out is lost).
    pub fn nudge(&mut self, f: u32, dx: i32, dy: i32) {
        let src = self.get(f);
        let mut out = Rgba::new(self.frame.0, self.frame.1);
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                out.set(x + dx, y + dy, src.get(x, y));
            }
        }
        self.put(f, &out, false);
    }

    /// Replaces colour `from` (RGB, opaque pixels) by `to` in `frames` (None = the whole image).
    /// Returns the pixels changed.
    pub fn replace_color(&mut self, from: [u8; 3], to: [u8; 4], frames: Option<&[u32]>) -> u32 {
        let mut n = 0;
        let all: Vec<u32>;
        let list = match frames {
            Some(l) => l,
            None => {
                all = (0..self.cols() * self.rows()).collect();
                &all
            }
        };
        for &f in list {
            let (ox, oy) = self.origin(f);
            for y in 0..self.frame.1 as i32 {
                for x in 0..self.frame.0 as i32 {
                    let p = self.img.get(ox + x, oy + y);
                    if p[3] > 0 && p[..3] == from {
                        self.img.set(ox + x, oy + y, to);
                        n += 1;
                    }
                }
            }
        }
        n
    }
}

/// The facing as a direction on screen (`dir`: 0 down, 1 left, 2 right, 3 up): forward through
#[cfg(test)]
mod tests {
    use super::*;

    /// A 4x16 figure of 2x2 frames; frame k's top-left pixel has red = k.
    fn sheet() -> Rgba {
        let mut img = Rgba::new(8, 32);
        for k in 0..64 {
            img.set((k % 4) * 2, (k / 4) * 2, [k as u8, 10, 20, 255]);
        }
        img
    }

    #[test]
    fn duplicate_nudge_rows_and_replace() {
        let mut img = sheet();
        let mut f = Frames { img: &mut img, frame: (2, 2) };
        // Duplicate frame 0 into 1; the last column can't.
        assert!(f.duplicate(0) && !f.duplicate(3));
        assert_eq!(f.get(1).get(0, 0)[0], 0);
        // Nudge frame 0 one pixel down.
        f.nudge(0, 0, 1);
        assert_eq!(f.get(0).get(0, 1)[0], 0);
        assert_eq!(f.get(0).get(0, 0)[3], 0);
        // Copy row 2 into row 5, clear row 4.
        let row = f.row(2);
        f.put_row(5, &row);
        assert_eq!(f.get(20).get(0, 0)[0], 8);
        f.clear_row(4);
        assert_eq!(f.get(16).get(0, 0)[3], 0);
        // Replace a colour in one frame, then in the whole sheet.
        assert_eq!(f.replace_color([7, 10, 20], [1, 2, 3, 255], Some(&[7])), 1);
        assert_eq!(f.replace_color([9, 10, 20], [1, 2, 3, 255], None), 2, "frame 9 and its copy in row 5");
    }
}
