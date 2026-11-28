//! A picture of the project painted pixel by pixel: one frame of a sprite sheet at a time
//! (what is drawn stays inside the frame), with its own undo while it is open. Nothing here
//! draws: the editor's pixel canvas shows it, tests and scripts use it as it is.

use std::path::Path;

use rustpg2d_engine::imageio::Rgba;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PxTool {
    Pencil,
    Eraser,
    Fill,
    Line,
    Rect,
    Picker,
}

impl PxTool {
    /// (tool, name, key, what it does)
    pub const ALL: [(PxTool, &'static str, &'static str, &'static str); 6] = [
        (PxTool::Pencil, "Pencil", "B", "Click or drag to draw single pixels: the left button with the first colour, the right with the second"),
        (PxTool::Eraser, "Eraser", "E", "Click or drag to make pixels see-through"),
        (PxTool::Fill, "Fill", "G", "Click to fill the connected area of one colour"),
        (PxTool::Line, "Line", "L", "Drag a straight line"),
        (PxTool::Rect, "Rectangle", "R", "Drag a rectangle's outline"),
        (PxTool::Picker, "Pick colour", "I", "Click a pixel to make its colour the first (right button: the second) colour"),
    ];

    pub fn named(name: &str) -> Option<PxTool> {
        let name = name.to_lowercase();
        PxTool::ALL.iter().find(|t| t.1.to_lowercase() == name || t.1.to_lowercase().starts_with(&name)).map(|t| t.0)
    }

    pub fn label(self) -> &'static str {
        PxTool::ALL.iter().find(|t| t.0 == self).map_or("", |t| t.1)
    }
}

const CLEAR: [u8; 4] = [0, 0, 0, 0];
/// The widest and tallest picture the editor can show (egui's limit for a texture).
pub const MAX_SIDE: u32 = 2048;

/// A picture being painted.
pub struct PixelDoc {
    /// Where it is in the project (`sprites/items.png`).
    pub rel: String,
    pub img: Rgba,
    /// The size of a frame (the whole picture when it isn't cut).
    pub frame: (u32, u32),
    /// The frame painted in.
    pub cur: u32,
    /// A texture laid side by side with itself (a ground material's): what is drawn across
    /// an edge of the frame comes back in on the other side, so its seams can be painted away.
    pub seamless: bool,
    undo: Vec<Rgba>,
    redo: Vec<Rgba>,
    /// The picture as its file has it.
    saved: Rgba,
    /// Counts the changes (whoever shows the picture follows it).
    pub version: u64,
    /// A frame copied (pasted into another).
    pub clip: Option<Rgba>,
}

impl PixelDoc {
    /// Reads picture `rel` of the project at `root`, cut into frames of `frame` pixels.
    pub fn open(root: &Path, rel: &str, frame: (u32, u32)) -> Result<PixelDoc, String> {
        let bytes = std::fs::read(root.join(rel)).map_err(|e| format!("reading {rel}: {e}"))?;
        let img = Rgba::decode(&bytes).map_err(|e| format!("{rel}: {e:#}"))?;
        Ok(PixelDoc::of(rel, img, frame))
    }

    pub fn of(rel: &str, img: Rgba, frame: (u32, u32)) -> PixelDoc {
        let frame = (frame.0.clamp(1, img.w.max(1)), frame.1.clamp(1, img.h.max(1)));
        PixelDoc { rel: rel.to_string(), saved: img.clone(), img, frame, cur: 0, seamless: false, undo: Vec::new(), redo: Vec::new(), version: 0, clip: None }
    }

    /// Writes the picture to its file.
    pub fn save(&mut self, root: &Path) -> Result<(), String> {
        let bytes = self.img.encode().map_err(|e| format!("{}: {e:#}", self.rel))?;
        std::fs::write(root.join(&self.rel), bytes).map_err(|e| format!("writing {}: {e}", self.rel))?;
        self.saved = self.img.clone();
        Ok(())
    }

    /// The frames are cut another way (the sheet's form changed): the picture stays.
    pub fn set_frame(&mut self, frame: (u32, u32)) {
        self.frame = (frame.0.clamp(1, self.img.w.max(1)), frame.1.clamp(1, self.img.h.max(1)));
        self.cur = self.cur.min(self.count() - 1);
    }

    pub fn cols(&self) -> u32 {
        (self.img.w / self.frame.0.max(1)).max(1)
    }

    pub fn rows(&self) -> u32 {
        (self.img.h / self.frame.1.max(1)).max(1)
    }

    pub fn count(&self) -> u32 {
        self.cols() * self.rows()
    }

    fn origin(&self) -> (i32, i32) {
        let f = self.cur.min(self.count() - 1);
        ((f % self.cols() * self.frame.0) as i32, (f / self.cols() * self.frame.1) as i32)
    }

    /// Whether `p` (a pixel of the frame painted in) is inside it.
    pub fn inside(&self, p: (i32, i32)) -> bool {
        p.0 >= 0 && p.1 >= 0 && p.0 < self.frame.0 as i32 && p.1 < self.frame.1 as i32
    }

    /// The pixel of the frame that `p` stands for: itself inside the frame; outside it the
    /// one it comes back in at when the picture is seamless, else none.
    pub fn wrapped(&self, p: (i32, i32)) -> Option<(i32, i32)> {
        if self.inside(p) {
            Some(p)
        } else if self.seamless {
            Some((p.0.rem_euclid(self.frame.0 as i32), p.1.rem_euclid(self.frame.1 as i32)))
        } else {
            None
        }
    }

    /// The colour of pixel `p` of the frame painted in (see-through outside it).
    pub fn get(&self, p: (i32, i32)) -> [u8; 4] {
        let Some(p) = self.wrapped(p) else { return CLEAR };
        let (ox, oy) = self.origin();
        self.img.get(ox + p.0, oy + p.1)
    }

    fn plot(&mut self, p: (i32, i32), c: [u8; 4]) {
        if let Some(p) = self.wrapped(p) {
            let (ox, oy) = self.origin();
            self.img.set(ox + p.0, oy + p.1, c);
        }
    }

    /// The frame painted in, as a picture of its own.
    pub fn frame_pixels(&self) -> Rgba {
        let mut out = Rgba::new(self.frame.0, self.frame.1);
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                out.set(x, y, self.get((x, y)));
            }
        }
        out
    }

    // ---------------------------------------------------------------- frames

    /// One more frame, painted in next: a sheet of one row grows by a frame to the right, one
    /// of several rows by a row (its first frame is the new one, the others are empty too).
    /// `copy`: the new frame starts as a copy of the one painted in (*Duplicate*). One step of
    /// the picture's undo. Not for a ground's texture: its frames are its animation, counted
    /// by its material.
    pub fn add_frame(&mut self, copy: bool) -> Result<String, String> {
        if self.seamless {
            return Err("a ground's texture has the frames its material's Frames says: change that there".into());
        }
        let (fw, fh) = self.frame;
        let (cols, rows) = (self.cols(), self.rows());
        let (w, h, at, more) = if rows == 1 { (((cols + 1) * fw).max(self.img.w), self.img.h, cols, 1) } else { (self.img.w, ((rows + 1) * fh).max(self.img.h), rows * cols, cols) };
        if w > MAX_SIDE || h > MAX_SIDE {
            return Err(format!("the picture would be {w} x {h} pixels: pictures wider or taller than {MAX_SIDE} can't be shown in the editor"));
        }
        let pixels = self.frame_pixels();
        self.snapshot();
        self.img = self.img.with_canvas(w, h);
        self.cur = at;
        if copy {
            for y in 0..fh as i32 {
                for x in 0..fw as i32 {
                    self.plot((x, y), pixels.get(x, y));
                }
            }
        }
        self.changed();
        let what = if copy { "A copy of the frame" } else { "An empty frame" };
        Ok(if more == 1 { format!("{what} added: frame {} of {}", at + 1, self.count()) } else { format!("{what} added in a new row: frame {} of {} ({} more empty ones in the row)", at + 1, self.count(), more - 1) })
    }

    // ---------------------------------------------------------------- undo

    /// Before a change: what the picture is now is a step to come back to.
    pub fn snapshot(&mut self) {
        self.undo.push(self.img.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// After a change (a snapshot that changed nothing is dropped again).
    pub fn changed(&mut self) {
        if self.undo.last() == Some(&self.img) {
            self.undo.pop();
            return;
        }
        self.version += 1;
    }

    /// Whether the picture differs from its file (undone back to it, it doesn't).
    pub fn is_dirty(&self) -> bool {
        self.img != self.saved
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(img) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.img, img));
        self.version += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(img) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.img, img));
        self.version += 1;
        true
    }

    // ---------------------------------------------------------------- painting

    /// Part of a stroke with `tool` from pixel `a` to `b` in colour `c`, without a step of its
    /// own (the caller takes a [`Self::snapshot`] when the stroke starts). The picker and the
    /// shapes drawn on release aren't strokes: see [`Self::shape`].
    pub fn drag(&mut self, tool: PxTool, a: (i32, i32), b: (i32, i32), c: [u8; 4]) {
        let c = if tool == PxTool::Eraser { CLEAR } else { c };
        if matches!(tool, PxTool::Pencil | PxTool::Eraser) {
            for p in line_points(a, b) {
                self.plot(p, c);
            }
        }
    }

    /// A line or a rectangle's outline from `a` to `b`, or the area around `a` filled: one step.
    pub fn shape(&mut self, tool: PxTool, a: (i32, i32), b: (i32, i32), c: [u8; 4]) {
        self.snapshot();
        match tool {
            PxTool::Line => line_points(a, b).into_iter().for_each(|p| self.plot(p, c)),
            PxTool::Rect => rect_points(a, b).into_iter().for_each(|p| self.plot(p, c)),
            PxTool::Fill => self.fill(a, c),
            _ => {}
        }
        self.changed();
    }

    /// One pixel set, as a step of its own (a click of the pencil or the eraser; scripts).
    pub fn dot(&mut self, tool: PxTool, p: (i32, i32), c: [u8; 4]) {
        self.snapshot();
        self.drag(tool, p, p, c);
        self.changed();
    }

    fn fill(&mut self, at: (i32, i32), c: [u8; 4]) {
        let Some(at) = self.wrapped(at) else { return };
        let target = self.get(at);
        if target == c {
            return;
        }
        // (Across the edges of a seamless picture: neighbours come back in on the other side.)
        let mut stack = vec![at];
        while let Some(p) = stack.pop() {
            let Some(p) = self.wrapped(p) else { continue };
            if self.get(p) != target {
                continue;
            }
            self.plot(p, c);
            stack.extend([(p.0 + 1, p.1), (p.0 - 1, p.1), (p.0, p.1 + 1), (p.0, p.1 - 1)]);
        }
    }

    // ---------------------------------------------------------------- the frame as a whole

    fn put(&mut self, src: &Rgba, flip: bool) {
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                let sx = if flip { self.frame.0 as i32 - 1 - x } else { x };
                self.plot((x, y), if sx < src.w as i32 && y < src.h as i32 { src.get(sx, y) } else { CLEAR });
            }
        }
    }

    /// Copies the frame painted in.
    pub fn copy(&mut self) {
        self.clip = Some(self.frame_pixels());
    }

    /// The frame copied replaces the one painted in. False without one.
    pub fn paste(&mut self) -> bool {
        let Some(src) = self.clip.clone() else { return false };
        self.snapshot();
        self.put(&src, false);
        self.changed();
        true
    }

    /// Mirrors the frame left to right (a figure facing the other way).
    pub fn flip(&mut self) {
        let src = self.frame_pixels();
        self.snapshot();
        self.put(&src, true);
        self.changed();
    }

    /// Empties the frame.
    pub fn clear(&mut self) {
        self.snapshot();
        self.put(&Rgba::new(self.frame.0, self.frame.1), false);
        self.changed();
    }

    /// Moves what is painted in the frame by `d` pixels (what leaves it is gone; in a seamless
    /// picture it comes back in on the other side: the seam moves into view).
    pub fn nudge(&mut self, d: (i32, i32)) {
        let src = self.frame_pixels();
        self.snapshot();
        for y in 0..self.frame.1 as i32 {
            for x in 0..self.frame.0 as i32 {
                let (mut sx, mut sy) = (x - d.0, y - d.1);
                if self.seamless {
                    (sx, sy) = (sx.rem_euclid(src.w as i32), sy.rem_euclid(src.h as i32));
                }
                let c = if sx >= 0 && sy >= 0 && sx < src.w as i32 && sy < src.h as i32 { src.get(sx, sy) } else { CLEAR };
                self.plot((x, y), c);
            }
        }
        self.changed();
    }

    /// Every pixel of colour `from` becomes `to`, in the frame or (`everywhere`) the whole
    /// picture. Returns how many.
    pub fn replace(&mut self, from: [u8; 4], to: [u8; 4], everywhere: bool) -> u32 {
        self.snapshot();
        let mut n = 0;
        if everywhere {
            for px in self.img.px.as_chunks_mut::<4>().0.iter_mut().filter(|px| **px == from) {
                *px = to;
                n += 1;
            }
        } else {
            for y in 0..self.frame.1 as i32 {
                for x in 0..self.frame.0 as i32 {
                    if self.get((x, y)) == from {
                        self.plot((x, y), to);
                        n += 1;
                    }
                }
            }
        }
        self.changed();
        n
    }

    /// The colours the picture uses, the most used first (see-through pixels don't count).
    pub fn colors(&self, most: usize) -> Vec<[u8; 4]> {
        let mut count: std::collections::HashMap<[u8; 4], u32> = std::collections::HashMap::new();
        for px in self.img.px.as_chunks::<4>().0.iter().filter(|px| px[3] > 0) {
            *count.entry(*px).or_default() += 1;
        }
        let mut list: Vec<([u8; 4], u32)> = count.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        list.into_iter().take(most).map(|c| c.0).collect()
    }
}

/// The pixels on the line between two points (Bresenham).
pub fn line_points(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    let mut out = Vec::new();
    loop {
        out.push((x, y));
        if (x, y) == b {
            return out;
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

/// The pixels of a rectangle's outline between two corners.
pub fn rect_points(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    let mut out = Vec::new();
    for x in x0..=x1 {
        out.extend([(x, y0), (x, y1)]);
    }
    for y in y0..=y1 {
        out.extend([(x0, y), (x1, y)]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [u8; 4] = [200, 30, 30, 255];
    const BLUE: [u8; 4] = [30, 60, 200, 255];

    /// A sheet of 2 x 2 frames of 8 x 8 pixels.
    fn doc() -> PixelDoc {
        PixelDoc::of("sprites/test.png", Rgba::new(16, 16), (8, 8))
    }

    #[test]
    fn painting_stays_inside_the_frame_and_is_undone_step_by_step() {
        let mut d = doc();
        assert_eq!((d.cols(), d.rows(), d.count()), (2, 2, 4));
        d.cur = 1;
        // A pencil stroke across the frame and out of it: only the frame's pixels are set.
        d.snapshot();
        d.drag(PxTool::Pencil, (0, 3), (12, 3), RED);
        d.changed();
        assert!((0..8).all(|x| d.get((x, 3)) == RED) && d.get((8, 3)) == CLEAR);
        assert_eq!(d.img.get(8, 3), RED, "frame 1 starts at x = 8 of the picture");
        assert_eq!(d.img.get(7, 3), CLEAR, "the frame beside it is untouched");
        // A rectangle and a fill inside it, each a step.
        d.shape(PxTool::Rect, (1, 5), (5, 7), BLUE);
        d.shape(PxTool::Fill, (0, 0), (0, 0), BLUE);
        assert_eq!((d.get((1, 5)), d.get((5, 7)), d.get((0, 0)), d.get((7, 2))), (BLUE, BLUE, BLUE, BLUE));
        assert_eq!(d.get((3, 6)), CLEAR, "the fill stops at the stroke and the rectangle");
        assert!(d.is_dirty());
        // Undone one by one, back to the file's picture: nothing to save then.
        assert!(d.undo() && d.get((0, 0)) == CLEAR && d.get((1, 5)) == BLUE);
        assert!(d.undo() && d.get((1, 5)) == CLEAR && d.get((0, 3)) == RED);
        assert!(d.undo() && !d.is_dirty() && !d.undo());
        assert!(d.redo() && d.get((0, 3)) == RED && d.is_dirty());
        // The eraser, and a click that changes nothing is no step.
        d.dot(PxTool::Eraser, (0, 3), RED);
        assert_eq!(d.get((0, 3)), CLEAR);
        let steps = d.undo.len();
        d.dot(PxTool::Eraser, (0, 3), RED);
        assert_eq!(d.undo.len(), steps);
    }

    #[test]
    fn a_frame_is_copied_mirrored_moved_recoloured_and_saved() {
        let mut d = doc();
        d.dot(PxTool::Pencil, (1, 2), RED);
        d.dot(PxTool::Pencil, (2, 2), BLUE);
        // Copied into the next frame, mirrored there.
        d.copy();
        d.cur = 3;
        assert!(d.paste());
        assert_eq!((d.get((1, 2)), d.get((2, 2))), (RED, BLUE));
        d.flip();
        assert_eq!((d.get((6, 2)), d.get((5, 2)), d.get((1, 2))), (RED, BLUE, CLEAR));
        // Moved down and left: what leaves the frame is gone, the frame beside it untouched.
        d.nudge((-5, 1));
        assert_eq!((d.get((1, 3)), d.get((0, 3))), (RED, BLUE));
        d.nudge((-1, 0));
        assert_eq!((d.get((0, 3)), d.colors(8)), (RED, vec![RED, BLUE]));
        // Red becomes blue in this frame only, then everywhere.
        assert_eq!(d.replace(RED, BLUE, false), 1);
        assert_eq!(d.replace(RED, BLUE, true), 1);
        assert_eq!(d.colors(8), vec![BLUE]);
        d.clear();
        assert!(d.frame_pixels().px.iter().all(|b| *b == 0));
        // Saved, the file holds the picture and nothing waits any more.
        let dir = std::env::temp_dir().join(format!("rustpg2d-pixels-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sprites")).unwrap();
        d.save(&dir).unwrap();
        assert!(!d.is_dirty());
        let again = PixelDoc::open(&dir, "sprites/test.png", (8, 8)).unwrap();
        // (Opened, its first frame is painted in: the one that was red and blue, all blue now.)
        assert!(again.img == d.img && again.get((1, 2)) == BLUE && again.get((2, 2)) == BLUE);
        assert_eq!(PxTool::named("fill"), Some(PxTool::Fill));
        assert_eq!(PxTool::named("pick"), Some(PxTool::Picker));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn a_seamless_texture_is_painted_across_its_edges() {
        // A material's texture of two frames of 8 x 8 side by side, the second painted in.
        let mut d = PixelDoc::of("materials/test.png", Rgba::new(16, 8), (8, 8));
        d.seamless = true;
        d.cur = 1;
        // A stroke out over the right edge comes back in on the left, in the same frame.
        d.snapshot();
        d.drag(PxTool::Pencil, (6, 2), (9, 2), RED);
        d.changed();
        assert_eq!([d.get((6, 2)), d.get((7, 2)), d.get((0, 2)), d.get((1, 2)), d.get((2, 2))], [RED, RED, RED, RED, CLEAR]);
        assert_eq!((d.get((8, 2)), d.get((-2, 2)), d.get((6, 10))), (RED, RED, RED), "asked for beside the frame, it answers as its copy there");
        assert!((0..8).all(|x| d.img.get(x, 2) == CLEAR), "the first frame is untouched");
        // A line down the frame and one across it: on a picture that ends at its edges they
        // would cut it into four areas; here the four are one (each goes on through the edge
        // opposite), and a fill started in one corner reaches all of them.
        d.shape(PxTool::Line, (4, 0), (4, 7), BLUE);
        d.shape(PxTool::Line, (0, 5), (7, 5), BLUE);
        d.shape(PxTool::Fill, (5, 7), (5, 7), RED);
        assert!((0..8).all(|y| (0..8).all(|x| d.get((x, y)) == if x == 4 || y == 5 { BLUE } else { RED })));
        // Moved half a frame, what leaves on one side comes in on the other: the seam is in the middle.
        let before = d.frame_pixels();
        d.nudge((4, 0));
        assert!((0..8).all(|y| (0..8).all(|x| d.get((x, y)) == before.get((x + 4) % 8, y))));
        // A picture that isn't seamless keeps cutting off at the frame.
        let mut plain = doc();
        plain.dot(PxTool::Pencil, (9, 2), RED);
        assert!(plain.img.px.iter().all(|b| *b == 0));
    }


    #[test]
    fn a_frame_is_added_or_duplicated_and_undone_in_one_step() {
        // A sheet of 2 frames of 4 x 4 in one row, the second with a red dot.
        let mut d = PixelDoc::of("sprites/s.png", Rgba::new(8, 4), (4, 4));
        d.cur = 1;
        d.dot(PxTool::Pencil, (1, 2), RED);
        let red_frame = d.frame_pixels();
        // Duplicated: a third frame to the right, a copy of the second, painted in next.
        let said = d.add_frame(true).unwrap();
        assert_eq!((d.img.w, d.img.h, d.count(), d.cur), (12, 4, 3, 2), "{said}");
        assert_eq!(d.frame_pixels(), red_frame);
        assert_eq!(said, "A copy of the frame added: frame 3 of 3");
        // An empty one: a fourth.
        d.add_frame(false).unwrap();
        assert_eq!((d.img.w, d.count(), d.cur), (16, 4, 3));
        assert_eq!(d.frame_pixels(), Rgba::new(4, 4), "empty");
        // One undo each.
        assert!(d.undo() && d.img.w == 12 && d.count() == 3);
        assert!(d.undo() && d.img.w == 8 && d.count() == 2);
        // A sheet of two rows grows by a row.
        let mut g = PixelDoc::of("sprites/g.png", Rgba::new(8, 8), (4, 4));
        let said = g.add_frame(false).unwrap();
        assert_eq!((g.img.w, g.img.h, g.count(), g.cur), (8, 12, 6, 4), "{said}");
        assert!(said.contains("new row") && said.contains("1 more empty"), "{said}");
        // Too big for the editor to show: refused, nothing changed.
        let mut wide = PixelDoc::of("sprites/w.png", Rgba::new(2048, 16), (16, 16));
        assert!(wide.add_frame(false).unwrap_err().contains("2064 x 16"));
        assert_eq!((wide.img.w, wide.can_undo()), (2048, false));
        // A ground's texture: not here.
        let mut ground = PixelDoc::of("materials/m.png", Rgba::new(16, 16), (16, 16));
        ground.seamless = true;
        assert!(ground.add_frame(false).is_err());
    }
}
