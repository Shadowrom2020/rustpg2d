//! Turns Meshy pictures into game art: a single picture into an item icon, a prop, a portrait or a
//! plain image. The picture's figure is kept (the largest shape and the parts near it), see-through
//! insides become solid, and it is scaled into its square or rectangle.

use std::collections::VecDeque;

use rustpg2d_engine::imageio::Rgba;

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

/// Rows with solid pixels: (first, last).
fn rows(img: &Rgba) -> Option<(i32, i32)> {
    let ys: Vec<i32> = (0..img.h as i32).filter(|&y| (0..img.w as i32).any(|x| img.get(x, y)[3] > 0)).collect();
    Some((*ys.first()?, *ys.last()?))
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
