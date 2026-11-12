//! Operations on the frames of a sprite sheet image: the sprite editor's buttons and the
//! `edit_frames` agent tool use them. Frames are numbered row by row. Directional sheets may use
//! the four-facing layout: 4 columns, blocks of 4 rows (down, left, right, up) for walk, idle,
//! attack and hurt.

use rustpg2d_engine::imageio::Rgba;

/// Rows per animation block of the four-facing layout (one per facing).
pub const FACINGS: u32 = 4;
/// The animation blocks of the four-facing layout: (name, frames).
pub const ANIMS: [(&str, u32); 4] = [("walk", 4), ("idle", 2), ("attack", 4), ("hurt", 1)];
pub const DIRS: [&str; 4] = ["down", "left", "right", "up"];

/// The first frame of animation `anim` (0 walk, 1 idle, 2 attack, 3 hurt) facing `dir` (0 down,
/// 1 left, 2 right, 3 up) in a sheet `cols` frames wide.
pub fn anim_frame(anim: u32, dir: u32, cols: u32) -> u32 {
    (anim * FACINGS + dir) * cols
}

pub fn anim_index(name: &str) -> Option<u32> {
    ANIMS.iter().position(|a| a.0 == name).map(|k| k as u32)
}

pub fn dir_index(name: &str) -> Option<u32> {
    DIRS.iter().position(|d| *d == name).map(|k| k as u32)
}

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

    pub fn is_empty(&self, f: u32) -> bool {
        self.get(f).px.as_chunks::<4>().0.iter().all(|p| p[3] == 0)
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

    /// The Right rows become the mirrored Left rows: of animation `anim`, or of every animation
    /// the sheet has (None). Returns the rows written.
    pub fn mirror_left_to_right(&mut self, anim: Option<u32>) -> Vec<u32> {
        let blocks: Vec<u32> = match anim {
            Some(a) => vec![a],
            None => (0..self.rows() / FACINGS).collect(),
        };
        let mut done = Vec::new();
        for b in blocks {
            let (left, right) = (b * FACINGS + 1, b * FACINGS + 2);
            if right >= self.rows() {
                continue;
            }
            let cols = self.cols();
            for c in 0..cols {
                let src = self.get(left * cols + c);
                self.put(right * cols + c, &src, true);
            }
            done.push(right);
        }
        done
    }

    /// Fills the empty idle (both frames) and hurt frames with the standing walk frame of their
    /// facing. Returns how many frames were filled.
    pub fn fill_standing(&mut self) -> u32 {
        let cols = self.cols();
        let mut n = 0;
        for dir in 0..FACINGS {
            let standing = self.get(anim_frame(0, dir, cols));
            for (anim, count) in [(1, 2), (3, 1)] {
                for c in 0..count.min(cols) {
                    let f = anim_frame(anim, dir, cols) + c;
                    if f / cols < self.rows() && self.is_empty(f) {
                        self.put(f, &standing, false);
                        n += 1;
                    }
                }
            }
        }
        n
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
    fn mirror_duplicate_nudge_and_fill() {
        let mut img = sheet();
        let mut f = Frames { img: &mut img, frame: (2, 2) };
        // Walk Left (row 1) frame 5 → Walk Right (row 2) frame 9, mirrored: its pixel moves right.
        assert_eq!(f.mirror_left_to_right(Some(0)), vec![2]);
        assert_eq!(f.get(9).get(1, 0), [5, 10, 20, 255]);
        assert_eq!(f.get(9).get(0, 0)[3], 0);
        assert_eq!(f.mirror_left_to_right(None), vec![2, 6, 10, 14]);
        // Duplicate frame 0 into 1; the last column can't.
        assert!(f.duplicate(0) && !f.duplicate(3));
        assert_eq!(f.get(1).get(0, 0)[0], 0);
        // Nudge frame 0 one pixel down.
        f.nudge(0, 0, 1);
        assert_eq!(f.get(0).get(0, 1)[0], 0);
        assert_eq!(f.get(0).get(0, 0)[3], 0);
        // Fill: only empty idle / hurt frames take the standing frame.
        f.clear_row(4);
        f.put(13 * 4, &Rgba::new(2, 2), false);
        let n = f.fill_standing();
        assert_eq!(n, 3, "idle down (2 frames) and hurt left");
        assert_eq!(f.get(16).get(0, 1)[0], 0, "idle down = the (nudged) standing frame 0");
        assert_eq!(f.get(13 * 4).get(0, 0)[0], 4, "hurt left = walk left standing");
        // Replace a colour in one frame, then in the whole sheet.
        assert_eq!(f.replace_color([7, 10, 20], [1, 2, 3, 255], Some(&[7])), 1);
        assert_eq!(f.replace_color([9, 10, 20], [1, 2, 3, 255], None), 0, "frame 9 was overwritten by the mirror");
        assert_eq!(anim_frame(2, 1, 4), 36);
        assert_eq!((anim_index("hurt"), dir_index("up")), (Some(3), Some(3)));
    }
}
