//! Buildings from their measurements ([`crate::defs::PropDef::building`]): walls around the prop's
//! hitbox, a roof, doors, windows and chimneys, made into one still 3D model that the figure
//! renderer draws like any prop model ([`crate::defs::PropDef::model`]). The model's units are
//! tiles (32 world pixels): x east, y up, z south, the origin in the middle of the footprint on
//! the ground. Pictures are tiled over walls and roofs: each face is cut along the picture's
//! size, every piece showing the whole picture, so one texture atlas serves the whole building.

use crate::defs::{BuildingShape, Database, Ridge, RoofKind, SpriteRef, WallSide};
use crate::model::{Model, SkinVertex};
use macroquad::color::Color;
use macroquad::math::{vec2, vec3, Vec2, Vec3};
use macroquad::texture::Image;
use std::cell::RefCell;
use std::collections::HashMap;

/// The id a building prop's generated model has among the models.
pub const PREFIX: &str = "@building:";

/// The id of prop `prop`'s building model.
pub fn model_id(prop: &str) -> String {
    format!("{PREFIX}{prop}")
}

/// The prop whose building model `model` is.
pub fn prop_of(model: &str) -> Option<&str> {
    model.strip_prefix(PREFIX)
}

/// World pixels per model unit.
const U: f32 = 32.0;
/// How far doors and windows stand out of the wall (units), so they draw over it.
const PROUD: f32 = 0.03;
/// The thickness of the boards along the roof's edges (units).
const BOARD: f32 = 0.12;

/// A picture and its size in world pixels.
pub type Picture = (Image, Vec2);

/// A generated building: its model and its height in world pixels.
pub struct Built {
    pub model: Model,
    pub height: f32,
}

/// Reads the pictures sprites show from the project's sheets (files through `read`), each sheet
/// decoded once.
pub fn picture_reader<'a>(db: &'a Database, read: &'a dyn Fn(&str) -> std::io::Result<Vec<u8>>) -> impl Fn(&SpriteRef) -> Option<Picture> + 'a {
    let sheets: RefCell<HashMap<String, Option<Image>>> = RefCell::new(HashMap::new());
    move |sp: &SpriteRef| {
        let def = db.sheets.iter().find(|s| s.id == sp.sheet)?;
        let mut cache = sheets.borrow_mut();
        let img = cache.entry(def.image.clone()).or_insert_with(|| read(&def.image).ok().and_then(|b| Image::from_file_with_format(&b, None).ok())).as_ref()?;
        let (fw, fh) = (def.frame_w.max(1), def.frame_h.max(1));
        let cols = (img.width as u32 / fw).max(1);
        let (cw, ch) = (sp.w.max(1), sp.h.max(1));
        let (x0, y0) = ((sp.frame % cols) * fw, (sp.frame / cols) * fh);
        let (w, h) = (cw * fw, ch * fh);
        if x0 + w > img.width as u32 || y0 + h > img.height as u32 {
            return None;
        }
        let mut out = Image::gen_image_color(w as u16, h as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        for y in 0..h {
            for x in 0..w {
                out.set_pixel(x, y, img.get_pixel(x0 + x, y0 + y));
            }
        }
        let scale = def.scale.max(1) as f32;
        Some((out, vec2(w as f32 / scale, h as f32 / scale)))
    }
}

/// The pictures a building uses, packed into one texture.
struct Atlas {
    /// Per entry: its rectangle in the atlas (pixels) and its world size (units).
    cells: Vec<(macroquad::math::Rect, Vec2)>,
    image: Image,
}

impl Atlas {
    /// Packs `pics` (each a picture or a plain colour) in rows, each with a border of its own
    /// edge pixels so tiles don't bleed.
    fn pack(pics: &[(Option<Picture>, [u8; 4], f32)]) -> Atlas {
        let imgs: Vec<(Image, Vec2)> = pics
            .iter()
            .map(|(p, fallback, darken)| {
                let (mut img, size) = p.clone().unwrap_or_else(|| {
                    let c = Color::from_rgba(fallback[0], fallback[1], fallback[2], fallback[3]);
                    (Image::gen_image_color(4, 4, c), vec2(U, U))
                });
                if *darken < 1.0 {
                    for px in img.bytes.as_chunks_mut::<4>().0 {
                        for c in &mut px[..3] {
                            *c = (*c as f32 * darken) as u8;
                        }
                    }
                }
                (img, size / U)
            })
            .collect();
        let width = imgs.iter().map(|(i, _)| i.width as u32 + 2).max().unwrap_or(4).max(256).next_power_of_two();
        let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
        let mut places = Vec::new();
        for (img, _) in &imgs {
            let (w, h) = (img.width as u32 + 2, img.height as u32 + 2);
            if x + w > width {
                (x, y, row) = (0, y + row, 0);
            }
            places.push((x, y));
            x += w;
            row = row.max(h);
        }
        let height = (y + row).max(4).next_power_of_two();
        let mut image = Image::gen_image_color(width as u16, height as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        let mut cells = Vec::new();
        for ((img, size), (px, py)) in imgs.iter().zip(places) {
            let (w, h) = (img.width as i32, img.height as i32);
            for yy in -1..=h {
                for xx in -1..=w {
                    let c = img.get_pixel(xx.clamp(0, w - 1) as u32, yy.clamp(0, h - 1) as u32);
                    image.set_pixel((px as i32 + 1 + xx) as u32, (py as i32 + 1 + yy) as u32, c);
                }
            }
            cells.push((macroquad::math::Rect::new(px as f32 + 1.0, py as f32 + 1.0, w as f32, h as f32), *size));
        }
        Atlas { cells, image }
    }

    /// Texture coordinates of point `f` (0..1 across, 0..1 up) of cell `cell`.
    fn uv(&self, cell: usize, f: Vec2) -> Vec2 {
        let (r, _) = self.cells[cell];
        let (aw, ah) = (self.image.width as f32, self.image.height as f32);
        // (A hair inside the cell: nearest sampling right on its edge could pick the border.)
        let f = f.clamp(Vec2::splat(0.002), Vec2::splat(0.998));
        vec2((r.x + f.x * r.w) / aw, (r.y + (1.0 - f.y) * r.h) / ah)
    }
}

/// The triangles made so far.
#[derive(Default)]
struct Mesh {
    vertices: Vec<SkinVertex>,
    indices: Vec<u16>,
}

impl Mesh {
    fn push(&mut self, pts: &[(Vec3, Vec2)], n: Vec3) {
        if pts.len() < 3 || self.vertices.len() + pts.len() > u16::MAX as usize {
            return;
        }
        let base = self.vertices.len() as u16;
        for (p, uv) in pts {
            self.vertices.push(SkinVertex { pos: *p, normal: n, uv: *uv, joints: [0; 4], weights: [1.0, 0.0, 0.0, 0.0] });
        }
        for k in 1..pts.len() as u16 - 1 {
            self.indices.extend([base, base + k, base + k + 1]);
        }
    }
}

/// The polygon `poly` (2D, convex) cut to the rectangle `lo`..`hi` (Sutherland-Hodgman).
fn clip(poly: &[Vec2], lo: Vec2, hi: Vec2) -> Vec<Vec2> {
    let mut out = poly.to_vec();
    let planes: [(Vec2, f32); 4] = [(vec2(1.0, 0.0), lo.x), (vec2(-1.0, 0.0), -hi.x), (vec2(0.0, 1.0), lo.y), (vec2(0.0, -1.0), -hi.y)];
    for (n, d) in planes {
        let inp = std::mem::take(&mut out);
        for i in 0..inp.len() {
            let (a, b) = (inp[i], inp[(i + 1) % inp.len()]);
            let (da, db) = (a.dot(n) - d, b.dot(n) - d);
            if da >= 0.0 {
                out.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                out.push(a + (b - a) * (da / (da - db)));
            }
        }
        if out.len() < 3 {
            return vec![];
        }
    }
    out
}

/// A face's normal from its first three corners, turned outward: up for roofs, away from the
/// building's middle line for walls.
fn outward(poly: &[Vec3]) -> Vec3 {
    let mut n = (poly[1] - poly[0]).cross(poly[2] - poly[0]).normalize_or_zero();
    if n.y.abs() > 0.05 {
        if n.y < 0.0 {
            n = -n;
        }
    } else {
        let mid = poly.iter().fold(Vec3::ZERO, |a, p| a + *p) / poly.len() as f32;
        if n.dot(vec3(mid.x, 0.0, mid.z)) < 0.0 {
            n = -n;
        }
    }
    n
}

struct Builder<'a> {
    mesh: Mesh,
    atlas: &'a Atlas,
}

impl Builder<'_> {
    /// Face `poly` (convex, flat) tiled with cell `cell`: the picture upright along the face (up
    /// the slope on roofs, north on flat ones), tiles counted from `origin`.
    fn tiled(&mut self, poly: &[Vec3], origin: Vec3, cell: usize) {
        let n = outward(poly);
        let up = if n.y.abs() > 0.999 { vec3(0.0, 0.0, -1.0) } else { (Vec3::Y - n * n.y).normalize() };
        let right = up.cross(n);
        let size = self.atlas.cells[cell].1;
        let flat: Vec<Vec2> = poly.iter().map(|p| vec2((*p - origin).dot(right), (*p - origin).dot(up))).collect();
        let (lo, hi) = flat.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let (i0, i1) = ((lo.x / size.x).floor() as i32, (hi.x / size.x).ceil() as i32);
        let (j0, j1) = ((lo.y / size.y).floor() as i32, (hi.y / size.y).ceil() as i32);
        for j in j0..j1 {
            for i in i0..i1 {
                let (a, b) = (vec2(i as f32 * size.x, j as f32 * size.y), vec2((i + 1) as f32 * size.x, (j + 1) as f32 * size.y));
                let piece = clip(&flat, a, b);
                let pts: Vec<(Vec3, Vec2)> = piece.iter().map(|q| (origin + right * q.x + up * q.y, self.atlas.uv(cell, (*q - a) / size))).collect();
                self.mesh.push(&pts, n);
            }
        }
    }

    /// Picture `cell` stretched over the rectangle `corner` .. `corner + right * w + up * h`.
    fn stretched(&mut self, corner: Vec3, right: Vec3, up: Vec3, size: Vec2, n: Vec3, cell: usize) {
        let at = |x: f32, y: f32| (corner + right * (x * size.x) + up * (y * size.y), self.atlas.uv(cell, vec2(x, y)));
        self.mesh.push(&[at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)], n);
    }
}

/// A roof's measures (units): its kind, whether the ridge runs along x, the footprint, the eaves'
/// height and the pitch.
struct Roof {
    kind: RoofKind,
    along_x: bool,
    w: f32,
    d: f32,
    h: f32,
    k: f32,
}

/// The roof's height (units) over point (x, z), for chimneys and the walls under it.
fn roof_y(r: &Roof, x: f32, z: f32) -> f32 {
    let (kind, h, k) = (r.kind, r.h, r.k);
    let (a, b, la, lb) = if r.along_x { (x, z, r.w / 2.0, r.d / 2.0) } else { (z, x, r.d / 2.0, r.w / 2.0) };
    match kind {
        RoofKind::Gable => h + k * (lb - b.abs()),
        RoofKind::Hip => h + k * (lb - b.abs()).min(la - a.abs()),
        RoofKind::Shed => h + k * (lb - b),
        RoofKind::Flat => h,
    }
}

/// Builds the model of a building with footprint `hitbox` (world pixels: width, depth), its
/// pictures through `picture`.
pub fn build(shape: &BuildingShape, hitbox: [f32; 2], picture: &dyn Fn(&SpriteRef) -> Option<Picture>) -> Built {
    let get = |sp: &SpriteRef| if sp.is_empty() { None } else { picture(sp) };
    // The atlas: wall, roof, gable, base, trim, chimney top, then each opening and chimney.
    let wall = get(&shape.wall);
    let roof = get(&shape.roof_picture);
    let mut pics: Vec<(Option<Picture>, [u8; 4], f32)> = vec![
        (wall.clone(), [205, 192, 168, 255], 1.0),
        (roof.clone(), [150, 72, 52, 255], 1.0),
        (get(&shape.gable).or(wall), [205, 192, 168, 255], 1.0),
        (get(&shape.base), [128, 124, 118, 255], 1.0),
        (get(&shape.trim).or_else(|| roof.clone()), [110, 60, 45, 255], if shape.trim.is_empty() { 0.62 } else { 1.0 }),
        (None, [46, 40, 36, 255], 1.0),
    ];
    let (c_wall, c_roof, c_gable, c_base, c_trim, c_top) = (0, 1, 2, 3, 4, 5);
    let first_opening = pics.len();
    pics.extend(shape.openings.iter().map(|o| (get(&o.picture), [72, 52, 36, 255], 1.0)));
    let first_chimney = pics.len();
    pics.extend(shape.chimneys.iter().map(|c| (get(&c.picture), [140, 130, 122, 255], 1.0)));
    let atlas = Atlas::pack(&pics);
    let mut b = Builder { mesh: Mesh::default(), atlas: &atlas };

    let (w, d) = (hitbox[0].max(8.0) / U, hitbox[1].max(8.0) / U);
    let h = shape.height.max(8.0) / U;
    let (hw, hd) = (w / 2.0, d / 2.0);
    let o = shape.overhang.max(0.0) / U;
    let k = shape.pitch.max(0.0);
    let bh = (shape.base_height / U).clamp(0.0, h);
    let along_x = match shape.ridge {
        Ridge::Auto => w >= d,
        Ridge::EastWest => true,
        Ridge::NorthSouth => false,
    };

    let roof_dims = Roof { kind: shape.roof, along_x, w, d, h, k };
    // The walls: south, east, north, west, each from its left end as seen from outside.
    let walls: [(WallSide, Vec3, Vec3, f32); 4] = [
        (WallSide::South, vec3(-hw, 0.0, hd), vec3(1.0, 0.0, 0.0), w),
        (WallSide::East, vec3(hw, 0.0, hd), vec3(0.0, 0.0, -1.0), d),
        (WallSide::North, vec3(hw, 0.0, -hd), vec3(-1.0, 0.0, 0.0), w),
        (WallSide::West, vec3(-hw, 0.0, -hd), vec3(0.0, 0.0, 1.0), d),
    ];
    for (side, start, dir, len) in walls {
        let end = start + dir * len;
        if bh > 0.0 {
            b.tiled(&[start, end, end + Vec3::Y * bh, start + Vec3::Y * bh], start, c_base);
        }
        let s = start + Vec3::Y * bh;
        b.tiled(&[s, end + Vec3::Y * bh, end + Vec3::Y * h, start + Vec3::Y * h], s, c_wall);
        // What rises above the eaves at this wall: a gable, or a shed roof's back wall.
        let across_side = if along_x { matches!(side, WallSide::East | WallSide::West) } else { matches!(side, WallSide::North | WallSide::South) };
        let top = |p: Vec3| roof_y(&roof_dims, p.x, p.z);
        match shape.roof {
            RoofKind::Gable if across_side => {
                let mid = (start + end) / 2.0;
                let apex = vec3(mid.x, top(mid), mid.z);
                b.tiled(&[start + Vec3::Y * h, end + Vec3::Y * h, apex], s, c_gable);
            }
            RoofKind::Shed => {
                let (ys, ye) = (top(start), top(end));
                if ys > h + 1e-3 || ye > h + 1e-3 {
                    b.tiled(&[start + Vec3::Y * h, end + Vec3::Y * h, vec3(end.x, ye, end.z), vec3(start.x, ys, start.z)], s, c_gable);
                }
            }
            _ => {}
        }
        // Its doors and windows.
        let n = dir.cross(Vec3::Y);
        for (i, op) in shape.openings.iter().enumerate().filter(|(_, op)| op.side == side) {
            let from_left = match side {
                WallSide::South | WallSide::West => op.at / U,
                WallSide::East | WallSide::North => len - op.at / U,
            };
            let (ow, oh) = (op.size[0].max(1.0) / U, op.size[1].max(1.0) / U);
            let corner = start + dir * (from_left - ow / 2.0) + Vec3::Y * (op.sill.max(0.0) / U) + n * PROUD;
            b.stretched(corner, dir, Vec3::Y, vec2(ow, oh), n, first_opening + i);
        }
    }

    // The roof, built with the ridge along `a` and mapped onto x / z.
    let (la, lb) = if along_x { (hw, hd) } else { (hd, hw) };
    let to_xz = |a: f32, y: f32, bb: f32| if along_x { vec3(a, y, bb) } else { vec3(bb, y, -a) };
    let (ra, rb) = (la + o, lb + o);
    match shape.roof {
        RoofKind::Gable | RoofKind::Hip => {
            let eave = h - k * o;
            let ridge = h + k * lb;
            let rl = if shape.roof == RoofKind::Hip { (ra - rb).max(0.0) } else { ra };
            for s in [1.0f32, -1.0] {
                let slope = [to_xz(-ra, eave, s * rb), to_xz(ra, eave, s * rb), to_xz(rl, ridge, 0.0), to_xz(-rl, ridge, 0.0)];
                let poly: Vec<Vec3> = slope.iter().copied().fold(Vec::new(), |mut v, p| {
                    if v.last().is_none_or(|q: &Vec3| (*q - p).length() > 1e-4) {
                        v.push(p);
                    }
                    v
                });
                b.tiled(&poly, slope[0], c_roof);
                // The board along the eave.
                let (e0, e1) = (to_xz(-ra, eave, s * rb), to_xz(ra, eave, s * rb));
                b.tiled(&[e0, e1, e1 - Vec3::Y * BOARD, e0 - Vec3::Y * BOARD], e0 - Vec3::Y * BOARD, c_trim);
                if shape.roof == RoofKind::Hip {
                    let end = [to_xz(s * ra, eave, -rb), to_xz(s * ra, eave, rb), to_xz(s * rl, ridge, 0.0)];
                    b.tiled(&end, end[0], c_roof);
                    let (f0, f1) = (to_xz(s * ra, eave, -rb), to_xz(s * ra, eave, rb));
                    b.tiled(&[f0, f1, f1 - Vec3::Y * BOARD, f0 - Vec3::Y * BOARD], f0 - Vec3::Y * BOARD, c_trim);
                } else {
                    // The boards along the gable's edges.
                    for side in [1.0f32, -1.0] {
                        let (p0, p1) = (to_xz(side * ra, eave, s * rb), to_xz(side * ra, ridge, 0.0));
                        b.tiled(&[p0, p1, p1 - Vec3::Y * BOARD, p0 - Vec3::Y * BOARD], p0 - Vec3::Y * BOARD, c_trim);
                    }
                }
            }
        }
        RoofKind::Shed => {
            let (front, back) = (h - k * o, h + k * (2.0 * lb + o));
            let slope = [to_xz(-ra, front, rb), to_xz(ra, front, rb), to_xz(ra, back, -rb), to_xz(-ra, back, -rb)];
            b.tiled(&slope, slope[0], c_roof);
            for (p0, p1) in [(slope[0], slope[1]), (slope[3], slope[0]), (slope[1], slope[2]), (slope[2], slope[3])] {
                b.tiled(&[p0, p1, p1 - Vec3::Y * BOARD, p0 - Vec3::Y * BOARD], p0 - Vec3::Y * BOARD, c_trim);
            }
        }
        RoofKind::Flat => {
            let y = h + BOARD;
            let top = [to_xz(-ra, y, -rb), to_xz(ra, y, -rb), to_xz(ra, y, rb), to_xz(-ra, y, rb)];
            b.tiled(&top, top[0], c_roof);
            for i in 0..4 {
                let (p0, p1) = (top[i], top[(i + 1) % 4]);
                b.tiled(&[p0, p1, p1 - Vec3::Y * BOARD * 2.0, p0 - Vec3::Y * BOARD * 2.0], p0 - Vec3::Y * BOARD * 2.0, c_trim);
            }
        }
    }

    // Chimneys: from inside the roof to `rise` above its highest corner.
    for (i, c) in shape.chimneys.iter().enumerate() {
        let (cx, cz) = (c.at[0] / U - hw, c.at[1] / U - hd);
        let (sx, sz) = (c.size[0].max(2.0) / U / 2.0, c.size[1].max(2.0) / U / 2.0);
        let corners = [vec3(cx - sx, 0.0, cz + sz), vec3(cx + sx, 0.0, cz + sz), vec3(cx + sx, 0.0, cz - sz), vec3(cx - sx, 0.0, cz - sz)];
        let high = corners.iter().map(|p| roof_y(&roof_dims, p.x, p.z)).fold(h, f32::max);
        let (y0, y1) = (h - 0.05, high + c.rise.max(0.0) / U);
        for j in 0..4 {
            let (p, q) = (corners[j], corners[(j + 1) % 4]);
            let s = p + Vec3::Y * y0;
            b.tiled(&[s, q + Vec3::Y * y0, q + Vec3::Y * y1, p + Vec3::Y * y1], s, first_chimney + i);
        }
        let top: Vec<Vec3> = corners.iter().map(|p| *p + Vec3::Y * y1).collect();
        b.tiled(&top, top[0], c_top);
    }

    let height = b.mesh.vertices.iter().map(|v| v.pos.y).fold(h, f32::max) * U;
    let Mesh { vertices, indices } = b.mesh;
    Built { model: Model::still(vertices, indices, Some(atlas.image)), height }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Opening;

    fn plain(_: &SpriteRef) -> Option<Picture> {
        None
    }

    fn shape(roof: RoofKind) -> BuildingShape {
        BuildingShape { roof, wall: SpriteRef::new("w", 0), roof_picture: SpriteRef::new("r", 0), ..Default::default() }
    }

    fn bounds(m: &Model) -> (Vec3, Vec3) {
        m.vertices.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), v| (lo.min(v.pos), hi.max(v.pos)))
    }

    #[test]
    fn a_gable_house_stands_on_its_footprint_under_its_roof() {
        let b = build(&shape(RoofKind::Gable), [224.0, 160.0], &plain);
        let (lo, hi) = bounds(&b.model);
        // Walls on the footprint, the roof reaching 6 px past them.
        assert!((lo.x + 7.0 / 2.0 + 6.0 / 32.0).abs() < 1e-3 && (hi.z - (5.0 / 2.0 + 6.0 / 32.0)).abs() < 1e-3, "{lo} {hi}");
        assert!(lo.y.abs() < 1e-4);
        // The ridge: 64 px of wall and 3/4 of half the depth (80 px) above it.
        assert!((b.height - (64.0 + 0.75 * 80.0)).abs() < 0.01, "{}", b.height);
        assert!(b.model.vertices.len() < crate::render::model::DRAW_VERTICES, "{}", b.model.vertices.len());
        // Every corner's picture coordinates lie in the atlas.
        assert!(b.model.vertices.iter().all(|v| (0.0..=1.0).contains(&v.uv.x) && (0.0..=1.0).contains(&v.uv.y)));
    }

    #[test]
    fn the_ridge_follows_the_longer_side_unless_told() {
        let deep = build(&shape(RoofKind::Gable), [96.0, 192.0], &plain);
        assert!((deep.height - (64.0 + 0.75 * 48.0)).abs() < 0.01, "{}", deep.height);
        let across = build(&BuildingShape { ridge: Ridge::EastWest, ..shape(RoofKind::Gable) }, [96.0, 192.0], &plain);
        assert!((across.height - (64.0 + 0.75 * 96.0)).abs() < 0.01, "{}", across.height);
    }

    #[test]
    fn every_roof_kind_and_its_parts_build() {
        for roof in [RoofKind::Gable, RoofKind::Hip, RoofKind::Shed, RoofKind::Flat] {
            let mut s = shape(roof);
            s.base = SpriteRef::new("b", 0);
            s.base_height = 12.0;
            s.openings = vec![Opening { side: WallSide::South, at: 50.0, picture: SpriteRef::new("door", 0), size: [24.0, 36.0], sill: 0.0 }, Opening { side: WallSide::East, at: 40.0, picture: SpriteRef::new("win", 0), size: [20.0, 20.0], sill: 24.0 }];
            s.chimneys = vec![crate::defs::Chimney { at: [40.0, 60.0], picture: SpriteRef::default(), size: [12.0, 12.0], rise: 14.0 }];
            let b = build(&s, [160.0, 128.0], &plain);
            assert!(!b.model.indices.is_empty() && b.model.indices.iter().all(|&i| (i as usize) < b.model.vertices.len()), "{roof:?}");
            // The chimney rises over the roof.
            let roof_top = match roof {
                RoofKind::Gable | RoofKind::Hip => 64.0 + 0.75 * 64.0,
                RoofKind::Shed => 64.0 + 0.75 * 128.0,
                RoofKind::Flat => 64.0,
            };
            assert!(b.height > roof_top - 1.0, "{roof:?} {}", b.height);
        }
    }

    #[test]
    fn a_door_sits_on_its_wall_where_it_says() {
        let mut s = shape(RoofKind::Gable);
        s.openings = vec![Opening { side: WallSide::South, at: 32.0, picture: SpriteRef::new("door", 0), size: [24.0, 36.0], sill: 0.0 }];
        let b = build(&s, [160.0, 128.0], &plain);
        // The door's quad: x from 20 to 44 px from the west end, just in front of the south wall.
        let door: Vec<&SkinVertex> = b.model.vertices.iter().filter(|v| (v.pos.z - (2.0 + PROUD)).abs() < 1e-4).collect();
        assert_eq!(door.len(), 4);
        let xs: Vec<f32> = door.iter().map(|v| v.pos.x * 32.0 + 80.0).collect();
        assert!(xs.iter().all(|x| (*x - 20.0).abs() < 0.01 || (*x - 44.0).abs() < 0.01), "{xs:?}");
        assert!(door.iter().all(|v| (v.pos.z - (2.0 + PROUD)).abs() < 1e-4 && v.normal.z > 0.99));
    }
}
