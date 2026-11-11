//! Turns Meshy pictures into game art (the Rust form of `scripts/art/meshy/sprites.py`):
//!
//! - character grids (4 x 4 cells: rows front / left / right / back; walk cycle, or an attack)
//!   into a 16-row complete-figure sheet: walk 0-3, idle 4-7, attack 8-11, hurt 12-15;
//! - single pictures into an item icon, a prop, a portrait or a plain image.
//!
//! Each grid cell keeps its figure (the largest shape and the parts near it, not pieces of the
//! neighbouring cells), the figure is scaled to its size class with its feet on (w / 2, h - 3),
//! side rows face left (the right row is their mirror), faded or undersized frames are replaced,
//! idle frame 2 breathes, hurt leans back; one 48-colour palette and a soft shadow.

use std::collections::{BTreeMap, VecDeque};

use rustpg2d_engine::imageio::Rgba;

/// How big a character is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeClass {
    pub id: &'static str,
    pub label: &'static str,
    /// Standing height in sheet pixels.
    pub height: u32,
    pub frame: (u32, u32),
    /// Image pixels per world pixel (`SheetDef::scale`).
    pub scale: u32,
    /// Shadow radius (sheet pixels).
    pub shadow: f32,
}

pub const SIZES: [SizeClass; 7] = [
    SizeClass { id: "person", label: "Person", height: 82, frame: (64, 96), scale: 2, shadow: 13.0 },
    SizeClass { id: "small", label: "Small person (dwarf, child)", height: 68, frame: (64, 96), scale: 2, shadow: 13.0 },
    SizeClass { id: "big", label: "Big (orc, ogre)", height: 88, frame: (80, 96), scale: 2, shadow: 16.0 },
    SizeClass { id: "huge", label: "Huge (troll, giant)", height: 126, frame: (96, 144), scale: 2, shadow: 26.0 },
    SizeClass { id: "animal", label: "Small animal or monster", height: 44, frame: (64, 64), scale: 2, shadow: 13.0 },
    SizeClass { id: "beast", label: "Large animal", height: 70, frame: (96, 96), scale: 2, shadow: 26.0 },
    SizeClass { id: "flying", label: "Flying creature (no shadow)", height: 32, frame: (64, 64), scale: 2, shadow: 0.0 },
];

pub fn size_class(id: &str) -> Option<SizeClass> {
    SIZES.iter().find(|s| s.id == id).copied()
}

const NCOL: usize = 48;

/// 4-connected shapes of a mask: (labels, sizes) with sizes[0] = 0.
fn label(mask: &[bool], w: usize, h: usize) -> (Vec<usize>, Vec<usize>) {
    let mut lab = vec![0usize; w * h];
    let mut sizes = vec![0usize];
    for start in 0..w * h {
        if !mask[start] || lab[start] != 0 {
            continue;
        }
        let n = sizes.len();
        lab[start] = n;
        let mut q = VecDeque::from([start]);
        let mut cnt = 0;
        while let Some(i) = q.pop_front() {
            cnt += 1;
            let (x, y) = (i % w, i / w);
            let mut push = |j: usize| {
                if mask[j] && lab[j] == 0 {
                    lab[j] = n;
                    q.push_back(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        sizes.push(cnt);
    }
    (lab, sizes)
}

fn alpha(img: &Rgba, i: usize) -> u8 {
    img.px[i * 4 + 3]
}

/// Partly transparent pixels inside the figure (background removal eats into light areas)
/// become opaque with their opaque neighbours' colour; real gaps stay.
fn fill_holes(img: &mut Rgba) {
    let (w, h) = (img.w as usize, img.h as usize);
    let mut seen = vec![false; w * h];
    let mut q = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if (x == 0 || y == 0 || x + 1 == w || y + 1 == h) && alpha(img, y * w + x) <= 8 {
                seen[y * w + x] = true;
                q.push_back(y * w + x);
            }
        }
    }
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
            if nx < w && ny < h {
                let j = ny * w + nx;
                if !seen[j] && alpha(img, j) <= 128 {
                    seen[j] = true;
                    q.push_back(j);
                }
            }
        }
    }
    let src = img.clone();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let a = alpha(&src, i);
            if a > 8 && a <= 128 && !seen[i] {
                let (mut sum, mut n) = ([0u32; 3], 0u32);
                for yy in y.saturating_sub(2)..(y + 3).min(h) {
                    for xx in x.saturating_sub(2)..(x + 3).min(w) {
                        let c = src.get(xx as i32, yy as i32);
                        if c[3] > 128 {
                            for k in 0..3 {
                                sum[k] += c[k] as u32;
                            }
                            n += 1;
                        }
                    }
                }
                let mut c = src.get(x as i32, y as i32);
                for k in 0..3 {
                    if let Some(v) = sum[k].checked_div(n) {
                        c[k] = v as u8;
                    }
                }
                c[3] = 255;
                img.set(x as i32, y as i32, c);
            }
        }
    }
}

fn crop(img: &Rgba, x0: u32, y0: u32, x1: u32, y1: u32) -> Rgba {
    let mut out = Rgba::new(x1 - x0, y1 - y0);
    for y in y0..y1 {
        for x in x0..x1 {
            out.set((x - x0) as i32, (y - y0) as i32, img.get(x as i32, y as i32));
        }
    }
    out
}

/// The solid part's bounding box (alpha > `min`): (x0, y0, x1, y1), exclusive.
fn bbox(img: &Rgba, min: u8) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.get(x as i32, y as i32)[3] > min {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > 0).then_some((x0, y0, x1, y1))
}

/// A cell's figure, cropped to it (None: empty cell).
pub fn keep_figure(cell: &Rgba) -> Option<Rgba> {
    let (w, h) = (cell.w as usize, cell.h as usize);
    let mask: Vec<bool> = (0..w * h).map(|i| alpha(cell, i) > 64).collect();
    let (lab, sizes) = label(&mask, w, h);
    let main = (1..sizes.len()).max_by_key(|&l| sizes[l])?;
    let mut keep: Vec<bool> = sizes.iter().map(|&s| s * 50 >= sizes[main]).collect();
    keep[0] = false;
    for y in 0..h {
        for x in [0, w - 1] {
            let l = lab[y * w + x];
            if l != 0 && l != main {
                keep[l] = false;
            }
        }
    }
    // Each shape's extent.
    let mut ext = vec![(usize::MAX, usize::MAX, 0usize, 0usize); sizes.len()];
    for (i, &l) in lab.iter().enumerate() {
        if l != 0 {
            let (x, y) = (i % w, i / w);
            let e = &mut ext[l];
            *e = (e.0.min(x), e.1.min(y), e.2.max(x), e.3.max(y));
        }
    }
    let (mx0, top, mx1, bot) = ext[main];
    let margin = (mx1 - mx0) as f32 * 0.25;
    let (lx0, lx1) = (mx0 as f32 - margin, mx1 as f32 + margin);
    for l in 1..sizes.len() {
        if keep[l] && l != main {
            let (x0, y0, x1, y1) = ext[l];
            // Wholly above or below the figure: the feet / head of the neighbouring row.
            if y1 < top || y0 > bot + 2 || (x0 as f32) > lx1 || (x1 as f32) < lx0 {
                keep[l] = false;
            }
        }
    }
    let mut out = cell.clone();
    for (i, &l) in lab.iter().enumerate() {
        if !keep[l] {
            out.px[i * 4..i * 4 + 4].copy_from_slice(&[0; 4]);
        }
    }
    fill_holes(&mut out);
    let (x0, y0, x1, y1) = bbox(&out, 128)?;
    Some(crop(&out, x0, y0, x1, y1))
}

/// The model faded this figure out (its solid pixels are mostly see-through).
pub fn faded(fig: &Rgba) -> bool {
    let mut a: Vec<u8> = fig.px.chunks(4).map(|p| p[3]).filter(|&a| a > 64).collect();
    if a.is_empty() {
        return false;
    }
    a.sort_unstable();
    a[a.len() / 2] < 235
}

/// The 16 figures of a 4 x 4 grid.
pub fn cells(grid: &Rgba) -> BTreeMap<(usize, usize), Rgba> {
    let (w, h) = (grid.w as f32 / 4.0, grid.h as f32 / 4.0);
    let mut out = BTreeMap::new();
    for r in 0..4 {
        for c in 0..4 {
            let cell = crop(grid, (c as f32 * w).round() as u32, (r as f32 * h).round() as u32, ((c + 1) as f32 * w).round() as u32, ((r + 1) as f32 * h).round() as u32);
            if let Some(f) = keep_figure(&cell) {
                out.insert((r, c), f);
            }
        }
    }
    out
}

/// A figure scaled by `k` into a `fw` x `fh` frame, feet on (fw / 2, fh - 3), kept inside the
/// frame (8 x 8 samples per pixel, premultiplied alpha, solid where at least half covered).
pub fn place(fig: &Rgba, k: f32, fw: u32, fh: u32) -> Rgba {
    // Middle of the legs (the lowest quarter), so held weapons don't push the figure aside.
    let (mut lx0, mut lx1) = (u32::MAX, 0);
    for y in fig.h * 3 / 4..fig.h {
        for x in 0..fig.w {
            if fig.get(x as i32, y as i32)[3] > 128 {
                lx0 = lx0.min(x);
                lx1 = lx1.max(x);
            }
        }
    }
    let mid = if lx1 >= lx0 && lx0 != u32::MAX { (lx0 + lx1) as f32 / 2.0 + 0.5 } else { fig.w as f32 / 2.0 };
    let (tw, th) = (fig.w as f32 * k, fig.h as f32 * k);
    let mut left = fw as f32 / 2.0 - mid * k;
    if tw < fw as f32 - 2.0 {
        left = left.clamp(1.0, fw as f32 - 1.0 - tw);
    }
    let top = fh as f32 - 2.5 - th;
    let mut out = Rgba::new(fw, fh);
    const S: u32 = 8;
    for y in 0..fh {
        for x in 0..fw {
            let (mut acc, mut al) = ([0f32; 3], 0f32);
            for sy in 0..S {
                for sx in 0..S {
                    let u = x as f32 + (sx as f32 + 0.5) / S as f32;
                    let v = y as f32 + (sy as f32 + 0.5) / S as f32;
                    let (fx, fy) = ((u - left) / k, (v - top) / k);
                    if fx < 0.0 || fy < 0.0 {
                        continue;
                    }
                    let c = fig.get(fx as i32, fy as i32);
                    let a = c[3] as f32 / 255.0;
                    for i in 0..3 {
                        acc[i] += c[i] as f32 * a;
                    }
                    al += a;
                }
            }
            let n = (S * S) as f32;
            if al / n >= 0.5 {
                out.set(x as i32, y as i32, [(acc[0] / al) as u8, (acc[1] / al) as u8, (acc[2] / al) as u8, 255]);
            }
        }
    }
    out
}

fn solid(img: &Rgba) -> usize {
    img.px.chunks(4).filter(|p| p[3] == 255).count()
}

fn iou(a: &Rgba, b: &Rgba) -> f32 {
    let (mut and, mut or) = (0, 0);
    for (p, q) in a.px.chunks(4).zip(b.px.chunks(4)) {
        let (x, y) = (p[3] == 255, q[3] == 255);
        and += (x && y) as u32;
        or += (x || y) as u32;
    }
    and as f32 / or.max(1) as f32
}

fn mirrored(img: &Rgba) -> Rgba {
    let mut out = Rgba::new(img.w, img.h);
    for y in 0..img.h as i32 {
        for x in 0..img.w as i32 {
            out.set(img.w as i32 - 1 - x, y, img.get(x, y));
        }
    }
    out
}

/// Rows with solid pixels: (first, last).
fn rows(img: &Rgba) -> Option<(i32, i32)> {
    let ys: Vec<i32> = (0..img.h as i32).filter(|&y| (0..img.w as i32).any(|x| img.get(x, y)[3] > 0)).collect();
    Some((*ys.first()?, *ys.last()?))
}

/// Idle frame 2: everything above the hips one pixel lower.
fn breathe(img: &Rgba) -> Rgba {
    let Some((top, bot)) = rows(img) else { return img.clone() };
    let cut = top + ((bot - top) as f32 * 0.55) as i32;
    let mut out = img.clone();
    for y in (top + 1..=cut).rev() {
        for x in 0..img.w as i32 {
            out.set(x, y, img.get(x, y - 1));
        }
    }
    for x in 0..img.w as i32 {
        out.set(x, top, [0; 4]);
    }
    out
}

/// Hurt: the upper body leans back (front / back views: up).
fn lean(img: &Rgba, facing: usize) -> Rgba {
    let Some((top, bot)) = rows(img) else { return img.clone() };
    let dx = [0, 1, -1, 0][facing];
    let mut out = Rgba::new(img.w, img.h);
    for y in 0..img.h as i32 {
        let t = (((bot - y) as f32 / (bot - top).max(1) as f32 - 0.35).max(0.0)) / 0.65;
        let sx = (dx as f32 * 3.0 * t).round() as i32;
        let sy = if (facing == 0 || facing == 3) && t > 0.5 { -1 } else { 0 };
        for x in 0..img.w as i32 {
            let c = img.get(x - sx, y);
            if c[3] > 0 && (0..img.w as i32).contains(&(x - sx)) {
                out.set(x, y + sy, c);
                if sy != 0 {
                    out.set(x, y, c); // no gap where it left
                }
            }
        }
    }
    out
}

/// Median-cut palette of the colours (at most `n`).
fn palette(colors: &[[u8; 3]], n: usize) -> Vec<[u8; 3]> {
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![colors.to_vec()];
    while boxes.len() < n {
        // Split the box with the widest channel range.
        let (bi, ch, range) = boxes
            .iter()
            .enumerate()
            .flat_map(|(i, b)| {
                (0..3).map(move |c| {
                    let (lo, hi) = b.iter().fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[c]), hi.max(p[c])));
                    (i, c, hi.saturating_sub(lo))
                })
            })
            .max_by_key(|t| (t.2, boxes[t.0].len()))
            .unwrap_or((0, 0, 0));
        if range == 0 || boxes[bi].len() < 2 {
            break;
        }
        let mut b = boxes.swap_remove(bi);
        b.sort_unstable_by_key(|p| p[ch]);
        let other = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(other);
    }
    boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let s = b.iter().fold([0u64; 3], |mut s, p| {
                for i in 0..3 {
                    s[i] += p[i] as u64;
                }
                s
            });
            let n = b.len() as u64;
            [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]
        })
        .collect()
}

fn nearest(pal: &[[u8; 3]], c: [u8; 3]) -> [u8; 3] {
    *pal.iter().min_by_key(|p| (0..3).map(|i| (p[i] as i32 - c[i] as i32).pow(2)).sum::<i32>()).unwrap_or(&c)
}

/// A character's walk grid (and attack grid) as a 16-row sheet. `k`: scale (None = the size
/// class's height over the standing figure's).
pub fn character_sheet(walk_grid: &Rgba, attack_grid: Option<&Rgba>, size: &SizeClass, side_faces_right: bool) -> anyhow::Result<Rgba> {
    let mut walk = cells(walk_grid);
    let stand = walk.get(&(0, 0)).or_else(|| walk.values().next()).ok_or_else(|| anyhow::anyhow!("no figure in the walk picture"))?;
    let k = size.height as f32 / stand.h as f32;
    let (fw, fh) = size.frame;
    // Faded figures: the frame two steps on.
    for r in 0..4 {
        for c in 0..4 {
            if walk.get(&(r, c)).is_some_and(faded) {
                if let Some(alt) = walk.get(&(r, (c + 2) % 4)).filter(|a| !faded(a)).cloned() {
                    walk.insert((r, c), alt);
                }
            }
        }
    }
    let mut fr: BTreeMap<(usize, usize), Rgba> = walk.iter().map(|(&key, f)| (key, place(f, k, fw, fh))).collect();
    // Frames much smaller than the rest of their row.
    for r in 0..4 {
        let mut areas: Vec<(usize, usize)> = (0..4).filter_map(|c| fr.get(&(r, c)).map(|f| (c, solid(f)))).collect();
        if areas.len() < 2 {
            continue;
        }
        areas.sort_by_key(|a| a.1);
        let typical = areas[areas.len() / 2].1;
        for &(c, a) in &areas {
            let alt = (c + 2) % 4;
            if (a as f32) < 0.6 * typical as f32 && fr.get(&(r, alt)).is_some_and(|f| solid(f) as f32 >= 0.6 * typical as f32) {
                let f = fr[&(r, alt)].clone();
                fr.insert((r, c), f);
            }
        }
    }
    // The side row faces like its first frame.
    if let Some(first) = fr.get(&(1, 0)).cloned() {
        let back = mirrored(&first);
        for c in 1..4 {
            if let Some(x) = fr.get(&(1, c)).cloned() {
                if iou(&x, &back) > iou(&x, &first) + 0.05 {
                    fr.insert((1, c), mirrored(&x));
                }
            }
        }
    }
    let get = |r: usize, c: usize| fr.get(&(r, c)).or_else(|| fr.get(&(r, 0))).cloned().unwrap_or_else(|| Rgba::new(fw, fh));
    // walk[facing][col]: facings down, left, right, up.
    let mut walk_fr: Vec<Vec<Rgba>> = vec![Vec::new(); 4];
    for c in 0..4 {
        let side = get(1, c);
        let (left, right) = if side_faces_right { (mirrored(&side), side) } else { (side.clone(), mirrored(&side)) };
        walk_fr[0].push(get(0, c));
        walk_fr[1].push(left);
        walk_fr[2].push(right);
        walk_fr[3].push(get(3, c));
    }
    let mut attack: Vec<Vec<Option<Rgba>>> = vec![vec![None; 4]; 4];
    if let Some(grid) = attack_grid {
        let att = cells(grid);
        for (row, f) in [(0usize, 0usize), (1, 1), (3, 3)] {
            let standing = solid(&walk_fr[f][0]);
            for (c, slot) in attack[f].iter_mut().enumerate() {
                if let Some(fig) = att.get(&(row, c)).filter(|f| !faded(f)) {
                    let x = place(fig, k, fw, fh);
                    if solid(&x) * 2 >= standing {
                        *slot = Some(x);
                    }
                }
            }
        }
        let refl = walk_fr[1][0].clone();
        let back = mirrored(&refl);
        for x in attack[1].iter_mut().flatten() {
            if iou(x, &back) > iou(x, &refl) + 0.03 {
                *x = mirrored(x);
            }
        }
        attack[2] = attack[1].iter().map(|x| x.as_ref().map(mirrored)).collect();
    }
    let mut frames: BTreeMap<(usize, usize), Rgba> = BTreeMap::new();
    for f in 0..4 {
        let st = walk_fr[f][0].clone();
        for c in 0..4 {
            frames.insert((f, c), walk_fr[f][c].clone());
            frames.insert((8 + f, c), attack[f][c].clone().unwrap_or_else(|| st.clone()));
        }
        frames.insert((4 + f, 0), st.clone());
        frames.insert((4 + f, 1), breathe(&st));
        frames.insert((12 + f, 0), lean(&st, f));
    }
    // One palette, the shadow, the sheet.
    let colors: Vec<[u8; 3]> = frames.values().flat_map(|x| x.px.chunks(4).filter(|p| p[3] == 255).map(|p| [p[0], p[1], p[2]])).collect();
    let pal = palette(&colors, NCOL);
    let mut sheet = Rgba::new(fw * 4, fh * 16);
    for (&(r, c), x) in &frames {
        for y in 0..fh as i32 {
            for xx in 0..fw as i32 {
                let p = x.get(xx, y);
                let out = if p[3] == 255 {
                    let n = nearest(&pal, [p[0], p[1], p[2]]);
                    [n[0], n[1], n[2], 255]
                } else {
                    let (dx, dy) = ((xx as f32 + 0.5 - fw as f32 / 2.0) / size.shadow.max(0.01), (y as f32 + 0.5 - (fh as f32 - 3.0)) / 3.0);
                    if size.shadow > 0.0 && dx * dx + dy * dy <= 1.0 {
                        [0, 0, 0, 76]
                    } else {
                        [0; 4]
                    }
                };
                sheet.set(c as i32 * fw as i32 + xx, r as i32 * fh as i32 + y, out);
            }
        }
    }
    Ok(sheet)
}

/// Single objects: background removal leaves light areas (a white fang, a pale blade) partly
/// see-through. Everything visible that the clear background (alpha <= 8) can't reach from the
/// edges becomes solid, in its own (un-premultiplied) colour; real holes (fully clear, like the
/// inside of a loop) stay.
fn solidify(img: &mut Rgba) {
    let (w, h) = (img.w as usize, img.h as usize);
    let mut outside = vec![false; w * h];
    let mut q = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if (x == 0 || y == 0 || x + 1 == w || y + 1 == h) && alpha(img, y * w + x) <= 8 {
                outside[y * w + x] = true;
                q.push_back(y * w + x);
            }
        }
    }
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
            if nx < w && ny < h {
                let j = ny * w + nx;
                if !outside[j] && alpha(img, j) <= 8 {
                    outside[j] = true;
                    q.push_back(j);
                }
            }
        }
    }
    for i in 0..w * h {
        let a = alpha(img, i);
        // Not reached and visible: inside the object. (Pixels next to the clear outside keep
        // their softness: they are the object's rim.)
        if a > 8 && a < 255 && !outside[i] {
            let (x, y) = (i % w, i / w);
            let rim = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)].iter().any(|&(nx, ny)| nx < w && ny < h && outside[ny * w + nx]);
            if !rim {
                // Background removal stores these premultiplied (darkened by their alpha).
                for k in 0..3 {
                    img.px[i * 4 + k] = (img.px[i * 4 + k] as u32 * 255 / a as u32).min(255) as u8;
                }
                img.px[i * 4 + 3] = 255;
            }
        }
    }
}

/// A single picture's figure, scaled to fit `w` x `h` (keeping its proportions), bottom-centred
/// (props) or centred, with a margin of `pad` pixels.
pub fn fit(picture: &Rgba, w: u32, h: u32, pad: u32, bottom: bool) -> anyhow::Result<Rgba> {
    let mut picture = picture.clone();
    solidify(&mut picture);
    let fig = keep_figure(&picture).ok_or_else(|| anyhow::anyhow!("the picture is empty"))?;
    let k = ((w - 2 * pad) as f32 / fig.w as f32).min((h - 2 * pad) as f32 / fig.h as f32);
    let placed = place(&fig, k, w, h);
    if bottom {
        return Ok(placed);
    }
    // Centre vertically.
    let Some((top, bot)) = rows(&placed) else { return Ok(placed) };
    let shift = ((h as i32 - (bot - top + 1)) / 2) - top;
    let mut out = Rgba::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            out.set(x, y + shift, placed.get(x, y));
        }
    }
    Ok(out)
}

/// A large picture shrunk to fit `max` pixels on its longer side (painted images, portraits).
pub fn shrink(picture: &Rgba, max: u32) -> Rgba {
    let k = (max as f32 / picture.w.max(picture.h) as f32).min(1.0);
    if k >= 1.0 {
        return picture.clone();
    }
    let (w, h) = (((picture.w as f32 * k).round() as u32).max(1), ((picture.h as f32 * k).round() as u32).max(1));
    let mut out = Rgba::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (x0, x1) = ((x as f32 / k) as u32, (((x + 1) as f32 / k) as u32).min(picture.w).max((x as f32 / k) as u32 + 1));
            let (y0, y1) = ((y as f32 / k) as u32, (((y + 1) as f32 / k) as u32).min(picture.h).max((y as f32 / k) as u32 + 1));
            let (mut s, mut a, mut n) = ([0f32; 3], 0f32, 0f32);
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let c = picture.get(xx as i32, yy as i32);
                    let al = c[3] as f32;
                    for i in 0..3 {
                        s[i] += c[i] as f32 * al;
                    }
                    a += al;
                    n += 1.0;
                }
            }
            if a > 0.0 {
                out.set(x as i32, y as i32, [(s[0] / a) as u8, (s[1] / a) as u8, (s[2] / a) as u8, (a / n) as u8]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 x 4 grid of figures (filled ellipses); `faded` makes one see-through, `debris` puts a
    /// blob from the cell above near the top of a cell.
    fn grid(faded_cell: Option<(usize, usize)>, debris: Option<(usize, usize)>) -> Rgba {
        let (cw, ch) = (100, 150);
        let mut g = Rgba::new(cw * 4, ch * 4);
        for r in 0..4u32 {
            for c in 0..4u32 {
                let a = if faded_cell == Some((r as usize, c as usize)) { 150 } else { 255 };
                for y in 20..140u32 {
                    for x in 30..70u32 {
                        let (dx, dy) = ((x as f32 - 50.0) / 20.0, (y as f32 - 80.0) / 60.0);
                        if dx * dx + dy * dy <= 1.0 {
                            g.set((c * cw + x) as i32, (r * ch + y) as i32, [120, 80 + r as u8 * 20, 60, a]);
                        }
                    }
                }
                if debris == Some((r as usize, c as usize)) {
                    for y in 2..9u32 {
                        for x in 40..56u32 {
                            g.set((c * cw + x) as i32, (r * ch + y) as i32, [90, 60, 30, 255]);
                        }
                    }
                }
            }
        }
        g
    }

    #[test]
    fn figures_lose_pieces_of_the_neighbouring_row() {
        let cells = cells(&grid(None, Some((1, 2))));
        assert_eq!(cells.len(), 16);
        let clean = &cells[&(1, 1)];
        let with_debris = &cells[&(1, 2)];
        assert_eq!((clean.w, clean.h), (with_debris.w, with_debris.h), "the blob above the figure is dropped");
    }

    #[test]
    fn a_character_sheet_has_sixteen_rows_with_feet_on_the_line() {
        let size = size_class("person").unwrap();
        let sheet = character_sheet(&grid(Some((3, 1)), None), None, &size, false).unwrap();
        assert_eq!((sheet.w, sheet.h), (64 * 4, 96 * 16));
        for r in 0..16 {
            let frame = crop(&sheet, 0, r * 96, 64, r * 96 + 96);
            let (_, y0, _, y1) = bbox(&frame, 128).unwrap();
            assert!((92..=94).contains(&y1), "row {r}: feet at {y1}");
            assert!(y1 - y0 >= 78, "row {r}: {y0}..{y1}");
        }
        // The faded back frame was replaced: as solid as its neighbours.
        let solid_at = |r: u32, c: u32| solid(&crop(&sheet, c * 64, r * 96, c * 64 + 64, r * 96 + 96));
        assert!(solid_at(3, 1) as f32 > 0.9 * solid_at(3, 0) as f32);
        // Hurt leans without a gap row across the body.
        let hurt = crop(&sheet, 0, 12 * 96, 64, 13 * 96);
        let (_, y0, _, y1) = bbox(&hurt, 128).unwrap();
        assert!((y0..y1).all(|y| (0..64).any(|x| hurt.get(x, y as i32)[3] == 255)));
    }

    #[test]
    fn icons_fit_their_square() {
        let mut pic = Rgba::new(300, 200);
        for y in 50..150 {
            for x in 20..280 {
                pic.set(x, y, [200, 30, 30, 255]);
            }
        }
        let icon = fit(&pic, 32, 32, 1, false).unwrap();
        let (x0, y0, x1, y1) = bbox(&icon, 128).unwrap();
        assert!(x0 >= 1 && x1 <= 31 && x1 - x0 >= 28, "{x0}..{x1}");
        assert!(y0 > 4 && y1 < 28, "centred: {y0}..{y1}");
        let small = shrink(&pic, 100);
        assert_eq!((small.w, small.h), (100, 67));
    }

    #[test]
    fn see_through_insides_of_objects_become_solid_but_real_holes_stay() {
        // A dark ring (an outline) around a pale, half see-through inside, and a clear hole.
        let mut pic = Rgba::new(200, 200);
        for y in 20..180i32 {
            for x in 20..180i32 {
                let edge = !(24..176).contains(&x) || !(24..176).contains(&y);
                let hole = (80..120).contains(&x) && (80..120).contains(&y);
                let ring = (76..124).contains(&x) && (76..124).contains(&y) && !hole;
                let c = if edge || ring { [30, 20, 20, 255] } else if hole { [0, 0, 0, 0] } else { [230, 230, 220, 60] };
                pic.set(x, y, c);
            }
        }
        let icon = fit(&pic, 32, 32, 1, false).unwrap();
        let solid_inside = (0..32).flat_map(|y| (0..32).map(move |x| (x, y))).filter(|&(x, y)| icon.get(x, y)[3] == 255).count();
        assert!(solid_inside > 600, "the pale inside is solid: {solid_inside}");
        assert_eq!(icon.get(16, 16)[3], 0, "the hole stays clear");
    }
}
