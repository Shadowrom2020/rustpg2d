//! Operations on the frames of a sprite sheet image, for animating character figures: the sprite
//! editor's buttons and the `edit_frames` agent tool use them. Frames are numbered row by row;
//! figures have 4 columns and 16 rows (walk, idle, attack, hurt; facings down, left, right, up).

use rustpg2d_engine::hands::{FrameHands, Grip};
use rustpg2d_engine::imageio::Rgba;

/// Rows per animation block of a figure (one per facing).
pub const FACINGS: u32 = 4;
/// The animations of a figure: (name, frames).
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

/// An image cut into frames, with the figure's hand grips (moved along with the frames).
pub struct Frames<'a> {
    pub img: &'a mut Rgba,
    pub frame: (u32, u32),
    pub hands: Option<&'a mut Vec<FrameHands>>,
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
        let cols = self.cols();
        for c in 0..cols {
            self.set_hands(r * cols + c, None);
        }
    }

    /// The hand grips of frame `f`.
    pub fn hands_of(&self, f: u32) -> Option<FrameHands> {
        self.hands.as_ref()?.iter().find(|h| h.frame == f).cloned()
    }

    /// Sets (or with None removes) the hand grips of frame `f`.
    pub fn set_hands(&mut self, f: u32, h: Option<FrameHands>) {
        let Some(list) = self.hands.as_mut() else { return };
        list.retain(|x| x.frame != f);
        if let Some(mut h) = h.filter(|h| h.right.is_some() || h.left.is_some()) {
            h.frame = f;
            list.push(h);
            list.sort_by_key(|h| h.frame);
        }
    }

    /// Frame `f` copied into the next column (the last column has none: false).
    pub fn duplicate(&mut self, f: u32) -> bool {
        if f % self.cols() + 1 >= self.cols() {
            return false;
        }
        let src = self.get(f);
        self.put(f + 1, &src, false);
        let h = self.hands_of(f);
        self.set_hands(f + 1, h);
        true
    }

    /// Shifts the contents of frame `f` by (dx, dy) pixels (what moves out is lost); its grips
    /// move along.
    pub fn nudge(&mut self, f: u32, dx: i32, dy: i32) {
        let src = self.get(f);
        let mut out = Rgba::new(self.frame.0, self.frame.1);
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                out.set(x + dx, y + dy, src.get(x, y));
            }
        }
        self.put(f, &out, false);
        if let Some(mut h) = self.hands_of(f) {
            for g in [&mut h.right, &mut h.left].into_iter().flatten() {
                g.x += dx as f32;
                g.y += dy as f32;
            }
            self.set_hands(f, Some(h));
        }
    }

    /// The grips of row `r`'s frames.
    pub fn row_hands(&self, r: u32) -> Vec<Option<FrameHands>> {
        (0..self.cols()).map(|c| self.hands_of(r * self.cols() + c)).collect()
    }

    pub fn put_row_hands(&mut self, r: u32, hands: &[Option<FrameHands>]) {
        let cols = self.cols();
        for (c, h) in hands.iter().enumerate().take(cols as usize) {
            self.set_hands(r * cols + c as u32, h.clone());
        }
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
            let w = self.frame.0 as f32;
            for c in 0..cols {
                let src = self.get(left * cols + c);
                self.put(right * cols + c, &src, true);
                // Turned the other way the figure shows its other hand in front: the hands swap.
                let h = self.hands_of(left * cols + c).map(|h| FrameHands { frame: 0, right: h.left.map(|g| g.mirrored(w)), left: h.right.map(|g| g.mirrored(w)) });
                self.set_hands(right * cols + c, h);
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
                        let h = self.hands_of(anim_frame(0, dir, cols));
                        self.set_hands(f, h);
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// Swaps the right and left grips of `frames` (archers' figures: the bow arm holds the left
    /// hand's item). Returns the frames changed.
    pub fn swap_hands(&mut self, frames: &[u32]) -> u32 {
        let mut n = 0;
        for &f in frames {
            if let Some(h) = self.hands_of(f) {
                self.set_hands(f, Some(FrameHands { right: h.left, left: h.right, ..h }));
                n += 1;
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
/// a fist (toward the viewer reads as down, away as up).
fn facing_vec(dir: u32) -> (f32, f32) {
    match dir {
        0 => (0.0, 1.0),
        1 => (-1.0, 0.0),
        2 => (1.0, 0.0),
        _ => (0.0, -1.0),
    }
}

/// The grip axis of a fist at the end of arm `v` (shoulder to fist) facing `dir`: square to the
/// arm (the handle runs across the fist) on the side toward the facing and a little up. Seen from
/// the front or back an arm along the screen's vertical hangs or reaches toward / away from the
/// viewer: the axis is the facing. Degrees.
pub fn grip_axis(v: (f32, f32), dir: u32) -> f32 {
    let f = facing_vec(dir);
    let len = v.0.hypot(v.1).max(1e-3);
    let (x, y) = if matches!(dir, 0 | 3) && (v.0 * f.0 + v.1 * f.1).abs() / len > 0.85 {
        f
    } else {
        let bias = (f.0, f.1 - 0.5);
        let (p, q) = ((-v.1, v.0), (v.1, -v.0));
        if p.0 * bias.0 + p.1 * bias.1 >= q.0 * bias.0 + q.1 * bias.1 { p } else { q }
    };
    y.atan2(x).to_degrees().round()
}

/// The hands of a front (`dir` 0) or back view at the outermost pixels of the hand band: facing
/// the viewer the right hand is on the image's left; facing away both are behind the body.
fn default_hands(extreme: &dyn Fn(bool, i32, i32) -> Option<(f32, f32)>, band: (i32, i32), dir: u32, ra: f32, la: f32) -> (Option<Grip>, Option<Grip>) {
    let back = dir == 3;
    let at = |left: bool, angle: f32| extreme(left, band.0, band.1).map(|(x, y)| Grip { front: !back, ..Grip::new(x, y, angle) });
    (at(!back, ra), at(back, la))
}

/// Skin-coloured (a warm hue, light): for snapping estimated hands.
fn skinlike(p: [u8; 4], skin: &[[u8; 3]]) -> bool {
    if p[3] < 200 {
        return false;
    }
    let c = [p[0], p[1], p[2]];
    if !skin.is_empty() {
        return skin.contains(&c);
    }
    let (r, g, b) = (c[0] as f32, c[1] as f32, c[2] as f32);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    max > 120.0 && r >= g && g >= b && (max - min) / max > 0.18 && (max - min) / max < 0.6
}

impl Frames<'_> {
    /// Estimates the hand grips of the frames that have none (all frames when `all`), from each
    /// frame's silhouette: front and back views at the outermost pixels at hand height, side views
    /// near the torso, attacks at the forward-most point; snapped to skin-coloured pixels (`skin`:
    /// the figure's skin shades, empty = skin-like hues). `sheet_scale`: image pixels per world
    /// pixel (the grip size follows the figure's height in the world). Returns frames estimated.
    pub fn estimate_hands(&mut self, all: bool, skin: &[[u8; 3]], sheet_scale: u32) -> u32 {
        let (cols, rows) = (self.cols(), self.rows().min(16));
        let (fw, _) = self.frame;
        let k = fw as f32 / 64.0;
        let mut n = 0;
        for f in 0..cols * rows {
            if !all && self.hands_of(f).is_some() {
                continue;
            }
            let px = self.get(f);
            let opaque = |x: i32, y: i32| px.get(x, y)[3] >= 200;
            let (mut x0, mut x1, mut y0, mut y1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
            for y in 0..px.h as i32 {
                for x in 0..px.w as i32 {
                    if opaque(x, y) {
                        (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
                    }
                }
            }
            if x0 > x1 {
                continue;
            }
            let h = (y1 - y0 + 1) as f32;
            let row = f / cols;
            let (anim, dir) = (row / FACINGS, row % FACINGS);
            // The outermost opaque pixel to one side within the rows `ya..yb`: (x, y).
            let extreme = |left: bool, ya: i32, yb: i32| -> Option<(f32, f32)> {
                let mut best: Option<(i32, Vec<i32>)> = None;
                for y in ya.max(y0)..=yb.min(y1) {
                    let xs: Vec<i32> = (x0..=x1).filter(|&x| opaque(x, y)).collect();
                    let Some(&x) = (if left { xs.first() } else { xs.last() }) else { continue };
                    match &mut best {
                        Some((bx, ys)) if *bx == x => ys.push(y),
                        Some((bx, _)) if (left && x > *bx) || (!left && x < *bx) => {}
                        _ => best = Some((x, vec![y])),
                    }
                }
                best.map(|(x, ys)| (x as f32 + if left { 3.0 * k } else { -3.0 * k }, ys.iter().sum::<i32>() as f32 / ys.len() as f32))
            };
            let band = (y0 + (0.42 * h) as i32, y0 + (0.68 * h) as i32);
            // Hands not swinging hold things forward through the fist.
            let (ra, la) = { let f = facing_vec(dir); let a = f.1.atan2(f.0).to_degrees().round(); (a, a) };
            let mid = (x0 + x1) as f32 / 2.0;
            // Attacks: a fist far from the shoulders (a raised or swung arm) holds the weapon and
            // points it along the arm. Front and back views give it to the hand on its side.
            // An attack frame that is the standing picture (a figure without an attack) has no swing.
            let still = anim == 2 && px == self.get(anim_frame(0, dir, cols));
            let swung = (anim == 2 && !still).then(|| {
                // The head: centred on the top rows' pixels (a raised arm widens the bounding box).
                let top: Vec<i32> = (y0..y0 + (0.1 * h) as i32).flat_map(|y| (x0..=x1).filter(move |&x| opaque(x, y))).collect();
                let head_x = if top.is_empty() { mid } else { top.iter().sum::<i32>() as f32 / top.len() as f32 };
                let shoulder = (head_x, y0 as f32 + 0.28 * h);
                let head = |x: i32, y: i32| (y as f32) < y0 as f32 + 0.34 * h && (x as f32 - head_x).abs() < 0.12 * h;
                let (chest, hips) = (y0 + (0.34 * h) as i32, y0 + (0.62 * h) as i32);
                let mut far: Option<(f32, i32, i32)> = None;
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if !opaque(x, y) || head(x, y) {
                            continue;
                        }
                        // A skin-coloured fist raised above the hands' usual height or stretched out to the
                        // side; between chest and hips any pixel stretched out (a fist drawn in shadow).
                        let wide = (x as f32 - shoulder.0).abs() >= 0.24 * h;
                        let skinned = skinlike(px.get(x, y), skin);
                        if !((skinned && (y < band.0 || wide)) || (wide && (chest..hips).contains(&y))) {
                            continue;
                        }
                        // Side views swing forward: a fist ahead of the body beats one stretched back.
                        let ahead = match dir {
                            1 => (x as f32) < shoulder.0,
                            2 => (x as f32) > shoulder.0,
                            _ => false,
                        };
                        let d = (x as f32 - shoulder.0).hypot(y as f32 - shoulder.1) + if ahead { 0.3 * h } else { 0.0 };
                        if far.is_none_or(|(b, _, _)| d > b) {
                            far = Some((d, x, y));
                        }
                    }
                }
                let (_, fx, fy) = far?;
                let (dx, dy) = (fx as f32 - shoulder.0, fy as f32 - shoulder.1);
                let len = dx.hypot(dy).max(1.0);
                // The grip sits in the fist, a little in from its outermost pixel.
                let (gx, gy) = (fx as f32 - dx / len * 2.0 * k, fy as f32 - dy / len * 2.0 * k);
                Some((Grip { front: dir != 3, ..Grip::new(gx, gy, grip_axis((dx, dy), dir)) }, gx >= shoulder.0))
            })
            .flatten();
            let (right, left) = match (anim, dir) {
                (2, 0) | (2, 3) if let Some((g, on_right)) = swung => {
                    // Facing the viewer the right hand is on the image's left; facing away on its right.
                    let (r, l) = default_hands(&extreme, band, dir, ra, la);
                    if on_right == (dir == 3) { (Some(g), l) } else { (r, Some(g)) }
                }
                // Attacks to the side: the swung fist holds the weapon, the other hand at the torso.
                (2, 1) | (2, 2) if let Some((g, _)) = swung => (Some(g), Some(Grip { front: dir == 1, ..Grip::new(mid, y0 as f32 + 0.6 * h, la) })),
                (_, 0) | (_, 3) => default_hands(&extreme, band, dir, ra, la),
                // Side views: the near hand in front of the torso, the far one behind it.
                (_, d) => {
                    let s = if d == 2 { 1.0 } else { -1.0 };
                    let near = Grip::new(mid + s * 2.0 * k, y0 as f32 + 0.6 * h, if d == 2 { ra } else { la });
                    let far = Grip { front: false, ..Grip::new(mid - s * 3.0 * k, y0 as f32 + 0.58 * h, if d == 2 { la } else { ra }) };
                    if d == 2 {
                        (Some(near), Some(far))
                    } else {
                        (Some(far), Some(near))
                    }
                }
            };
            // Snap to the skin-coloured pixels close by.
            let snap = |g: Grip| {
                let r = (5.0 * k) as i32;
                let (mut sx, mut sy, mut c) = (0.0, 0.0, 0.0);
                for y in g.y as i32 - r..=g.y as i32 + r {
                    for x in g.x as i32 - r..=g.x as i32 + r {
                        if skinlike(px.get(x, y), skin) {
                            (sx, sy, c) = (sx + x as f32, sy + y as f32, c + 1.0);
                        }
                    }
                }
                if c >= 3.0 { Grip { x: (sx / c).round(), y: (sy / c).round(), ..g } } else { g }
            };
            let scale = ((h / sheet_scale.max(1) as f32) / 41.0).clamp(0.6, 1.6);
            let fix = |g: Option<Grip>| g.map(|g| Grip { scale: (scale * 100.0).round() / 100.0, x: g.x.round(), y: g.y.round(), ..snap(g) });
            self.set_hands(f, Some(FrameHands { frame: f, right: fix(right), left: fix(left) }));
            n += 1;
        }
        n
    }
}

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
        let mut f = Frames { img: &mut img, frame: (2, 2), hands: None };
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

    #[test]
    fn grips_move_with_their_frames() {
        let mut img = sheet();
        let mut hands = vec![FrameHands { frame: 4, right: Some(Grip::new(0.0, 1.0, -45.0)), left: Some(Grip { front: false, ..Grip::new(1.0, 1.0, 100.0) }) }];
        let mut f = Frames { img: &mut img, frame: (2, 2), hands: Some(&mut hands) };
        // Mirror walk Left (frame 4) into walk Right (frame 8): mirrored, hands swapped.
        f.mirror_left_to_right(Some(0));
        let m = f.hands_of(8).unwrap();
        assert_eq!((m.right.unwrap().x, m.right.unwrap().angle, m.right.unwrap().front), (0.0, 80.0, false));
        assert_eq!((m.left.unwrap().x, m.left.unwrap().angle), (1.0, 225.0));
        // Duplicate and nudge carry them along; clearing a row drops them.
        f.duplicate(4);
        f.nudge(5, 1, 0);
        assert_eq!(f.hands_of(5).unwrap().right.unwrap().x, 1.0);
        f.clear_row(1);
        assert!(f.hands_of(4).is_none() && f.hands_of(5).is_none());
        assert!(f.hands_of(8).is_some());
    }

    #[test]
    fn hands_are_estimated_from_the_silhouette() {
        // A 16x24 frame figure: a body column and two "hands" (skin) at both sides, mid-height.
        let mut img = Rgba::new(16 * 4, 24);
        for c in 0..4 {
            let ox = c * 16;
            for y in 2..22 {
                for x in 6..10 {
                    img.set(ox + x, y, [40, 60, 120, 255]);
                }
            }
            for y in 12..14 {
                img.set(ox + 4, y, [230, 180, 140, 255]);
                img.set(ox + 5, y, [230, 180, 140, 255]);
                img.set(ox + 10, y, [230, 180, 140, 255]);
                img.set(ox + 11, y, [230, 180, 140, 255]);
            }
        }
        let mut hands = Vec::new();
        let mut f = Frames { img: &mut img, frame: (16, 24), hands: Some(&mut hands) };
        assert_eq!(f.estimate_hands(false, &[], 1), 4);
        let h = f.hands_of(0).unwrap();
        let (r, l) = (h.right.unwrap(), h.left.unwrap());
        // Facing the viewer: the right hand on the image's left, both on the skin pixels.
        assert!(r.x < 6.0 && l.x > 9.0, "{r:?} {l:?}");
        assert!((12.0..=14.0).contains(&r.y), "{r:?}");
        assert_eq!(f.estimate_hands(false, &[], 1), 0, "only frames without grips");
    }

    #[test]
    fn attack_frames_find_the_swung_fist() {
        // 64x96 frames of the figure layout; a body (head 10..30, torso 30..90) in two attack frames.
        let mut img = Rgba::new(64 * 4, 96 * 16);
        let (skin, cloth, shadow) = ([230, 180, 140, 255], [40, 60, 120, 255], [110, 90, 80, 255]);
        let fill = |img: &mut Rgba, f: u32, xs: std::ops::Range<i32>, ys: std::ops::Range<i32>, c: [u8; 4]| {
            let (ox, oy) = ((f % 4) as i32 * 64, (f / 4) as i32 * 96);
            for y in ys {
                for x in xs.clone() {
                    img.set(ox + x, oy + y, c);
                }
            }
        };
        for f in [32, 37] {
            fill(&mut img, f, 26..38, 10..30, skin);
            fill(&mut img, f, 24..40, 30..90, cloth);
        }
        // Attack down, wind-up: a fist raised beside the head on the image's left (the right hand).
        fill(&mut img, 32, 12..17, 14..19, skin);
        // Attack left, swing: a fist in shadow stretched forward (left), a skin one stretched back.
        fill(&mut img, 37, 2..8, 44..49, shadow);
        fill(&mut img, 37, 8..24, 45..47, cloth);
        fill(&mut img, 37, 54..60, 44..49, skin);
        fill(&mut img, 37, 40..54, 45..47, cloth);
        let mut hands = Vec::new();
        let mut f = Frames { img: &mut img, frame: (64, 96), hands: Some(&mut hands) };
        assert_eq!(f.estimate_hands(false, &[], 2), 2);
        let r = f.hands_of(32).unwrap().right.unwrap();
        assert!((12.0..=17.0).contains(&r.x) && (13.0..=20.0).contains(&r.y), "{r:?}");
        // The grip axis runs across the raised arm (shoulder about 31,36 to the fist).
        let arm = (r.y - 36.0).atan2(r.x - 31.0).to_degrees();
        assert!((r.angle - arm).to_radians().cos().abs() < 0.2, "square to the arm ({arm}): {r:?}");
        let r = f.hands_of(37).unwrap().right.unwrap();
        assert!(r.x < 10.0 && (43.0..=50.0).contains(&r.y), "the forward fist: {r:?}");
    }

    #[test]
    fn grip_axes_run_across_the_fist() {
        // Facing left: an arm hanging down holds things forward (left), a raised one forward and a
        // little up; an arm stretched forward holds them upright (a bow).
        assert_eq!(grip_axis((0.0, 20.0), 1), 180.0);
        assert!((-180.0..=-150.0).contains(&grip_axis((3.0, -20.0), 1)) || (150.0..=180.0).contains(&grip_axis((3.0, -20.0), 1)));
        assert!((-95.0..=-85.0).contains(&grip_axis((-20.0, 1.0), 1)));
        // Facing right, the arm forward and down: square to it, upward.
        assert!(grip_axis((15.0, 15.0), 2) < -30.0);
        // Facing the viewer, a hanging arm: toward the viewer (down on screen).
        assert_eq!(grip_axis((1.0, 20.0), 0), 90.0);
    }
}
