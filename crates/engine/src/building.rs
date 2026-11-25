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
    /// Faces inside a joined building, made last: they cast no shadow (that one's covers it).
    late: Vec<(Vec<Vec3>, Vec3, usize)>,
}

impl Builder<'_> {
    /// [`Builder::tiled`], or kept for the end when `hidden` (inside a joined building).
    fn face(&mut self, poly: &[Vec3], origin: Vec3, cell: usize, hidden: bool) {
        if hidden {
            self.late.push((poly.to_vec(), origin, cell));
        } else {
            self.tiled(poly, origin, cell);
        }
    }

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
/// pictures through `picture`. Without the buildings it is joined to: see [`build_joined`].
pub fn build(shape: &BuildingShape, hitbox: [f32; 2], picture: &dyn Fn(&SpriteRef) -> Option<Picture>) -> Built {
    build_joined(shape, hitbox, picture, &|_| None)
}

/// A building prop's shape and footprint, by id (for [`BuildingShape::joins`]).
pub type Neighbours<'a> = dyn Fn(&str) -> Option<(BuildingShape, [f32; 2])> + 'a;

/// [`build`] with the buildings it is joined to, found through `others`: its roof runs on into
/// theirs where they meet, and their shapes go into the model as hidden geometry
/// ([`Model::occluder`]), so whichever of the two is drawn later leaves out what the other
/// covers.
pub fn build_joined(shape: &BuildingShape, hitbox: [f32; 2], picture: &dyn Fn(&SpriteRef) -> Option<Picture>, others: &Neighbours) -> Built {
    let ends = roof_ends(shape, hitbox, others);
    let (mesh, image, height, shadowless) = geometry(shape, hitbox, picture, ends);
    let mut occluder = Vec::new();
    for j in &shape.joins {
        let Some((mut other, size)) = others(&j.prop) else { continue };
        let their_ends = roof_ends(&other, size, others);
        other.openings.clear();
        let (m, ..) = geometry(&other, size, &|_| None, their_ends);
        // Their footprint's middle from ours (units).
        let off = vec3((j.at[0] + size[0] / 2.0 - hitbox[0] / 2.0) / U, 0.0, (j.at[1] + size[1] / 2.0 - hitbox[1] / 2.0) / U);
        occluder.extend(m.indices.iter().map(|&i| m.vertices[i as usize].pos + off));
    }
    let Mesh { vertices, indices } = mesh;
    let mut model = Model::still(vertices, indices, Some(image));
    model.occluder = occluder;
    model.shadowless = shadowless;
    Built { model, height }
}

/// How far a roof reaches past the walls at one end of its ridge (world pixels), and across
/// what span of its footprint's middle line (into a joined building: no wider than that one).
#[derive(Clone, Copy, Debug)]
struct End {
    reach: f32,
    joined: bool,
    across: (f32, f32),
}

/// The roof's ends at the low and high end of its ridge: the overhang, or on into a joined
/// building that end touches until it meets that one's roof.
fn roof_ends(shape: &BuildingShape, hitbox: [f32; 2], others: &Neighbours) -> [End; 2] {
    let o = shape.overhang.max(0.0);
    let mut ends = [End { reach: o, joined: false, across: (f32::MIN, f32::MAX) }; 2];
    let (w, d) = (hitbox[0], hitbox[1]);
    let along_x = ridge_along_x(shape, w, d);
    let (len, across) = if along_x { (w, d) } else { (d, w) };
    let ridge = shape.height + shape.pitch.max(0.0) * across / 2.0;
    for j in &shape.joins {
        let Some((other, size)) = others(&j.prop) else { continue };
        // Their footprint along our ridge (a0..a1) and across it (b0..b1), ours from 0.
        let (a0, b0, la, lb) = if along_x { (j.at[0], j.at[1], size[0], size[1]) } else { (j.at[1], j.at[0], size[1], size[0]) };
        if !(b0 < across / 2.0 && across / 2.0 < b0 + lb) {
            continue;
        }
        // (The ridge's high end is the east one, or the north one: footprint y grows southward.)
        let end = if (a0 - len).abs() < 1.0 {
            along_x as usize
        } else if (a0 + la).abs() < 1.0 {
            !along_x as usize
        } else {
            continue;
        };
        // Their roof rises away from the wall we touch when their ridge crosses ours.
        let theirs_along = ridge_along_x(&other, size[0], size[1]);
        let k = other.pitch.max(0.0);
        let reach = if theirs_along != along_x && k > 0.01 && other.roof != RoofKind::Flat { (ridge - other.height) / k } else { la / 2.0 };
        // (Across: footprint y grows southward = +z, x eastward = +x: both as the roof's b.)
        ends[end] = End { reach: reach.clamp(o, la), joined: true, across: (b0 - across / 2.0, b0 + lb - across / 2.0) };
    }
    ends
}

fn ridge_along_x(shape: &BuildingShape, w: f32, d: f32) -> bool {
    match shape.ridge {
        Ridge::Auto => w >= d,
        Ridge::EastWest => true,
        Ridge::NorthSouth => false,
    }
}

/// The building's faces, its atlas, its height (world pixels) and how many of the last indices
/// cast no shadow (inside a joined building); the roof reaching past the walls as `ends` say at
/// the low and high end of its ridge.
fn geometry(shape: &BuildingShape, hitbox: [f32; 2], picture: &dyn Fn(&SpriteRef) -> Option<Picture>, ends: [End; 2]) -> (Mesh, Image, f32, usize) {
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
    let mut b = Builder { mesh: Mesh::default(), atlas: &atlas, late: Vec::new() };

    let (w, d) = (hitbox[0].max(8.0) / U, hitbox[1].max(8.0) / U);
    let h = shape.height.max(8.0) / U;
    let (hw, hd) = (w / 2.0, d / 2.0);
    let o = shape.overhang.max(0.0) / U;
    let k = shape.pitch.max(0.0);
    let bh = (shape.base_height / U).clamp(0.0, h);
    let along_x = ridge_along_x(shape, hitbox[0], hitbox[1]);

    let roof_dims = Roof { kind: shape.roof, along_x, w, d, h, k };
    // The walls: south, east, north, west, each from its left end as seen from outside.
    let walls: [(WallSide, Vec3, Vec3, f32); 4] = [
        (WallSide::South, vec3(-hw, 0.0, hd), vec3(1.0, 0.0, 0.0), w),
        (WallSide::East, vec3(hw, 0.0, hd), vec3(0.0, 0.0, -1.0), d),
        (WallSide::North, vec3(hw, 0.0, -hd), vec3(-1.0, 0.0, 0.0), w),
        (WallSide::West, vec3(-hw, 0.0, -hd), vec3(0.0, 0.0, 1.0), d),
    ];
    let joined = [ends[0].joined, ends[1].joined];
    // The wall at a joined end of the ridge stands inside the other building.
    let inside = |side: WallSide| match (along_x, side) {
        (true, WallSide::East) | (false, WallSide::North) => joined[1],
        (true, WallSide::West) | (false, WallSide::South) => joined[0],
        _ => false,
    };
    for (side, start, dir, len) in walls {
        let end = start + dir * len;
        let hidden = inside(side);
        if bh > 0.0 && shape.walls {
            b.face(&[start, end, end + Vec3::Y * bh, start + Vec3::Y * bh], start, c_base, hidden);
        }
        let s = start + Vec3::Y * bh;
        if shape.walls {
            b.face(&[s, end + Vec3::Y * bh, end + Vec3::Y * h, start + Vec3::Y * h], s, c_wall, hidden);
        }
        // What rises above the eaves at this wall: a gable, or a shed roof's back wall.
        let across_side = if along_x { matches!(side, WallSide::East | WallSide::West) } else { matches!(side, WallSide::North | WallSide::South) };
        let top = |p: Vec3| roof_y(&roof_dims, p.x, p.z);
        match shape.roof {
            RoofKind::Gable if across_side => {
                let mid = (start + end) / 2.0;
                let apex = vec3(mid.x, top(mid), mid.z);
                b.face(&[start + Vec3::Y * h, end + Vec3::Y * h, apex], s, c_gable, hidden);
            }
            RoofKind::Shed => {
                let (ys, ye) = (top(start), top(end));
                if ys > h + 1e-3 || ye > h + 1e-3 {
                    b.face(&[start + Vec3::Y * h, end + Vec3::Y * h, vec3(end.x, ye, end.z), vec3(start.x, ys, start.z)], s, c_gable, hidden);
                }
            }
            _ => {}
        }
        // Its doors and windows.
        let n = dir.cross(Vec3::Y);
        for (i, op) in shape.openings.iter().enumerate().filter(|(_, op)| op.side == side && shape.walls) {
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
    // The roof's reach past the walls at the ridge's low (-a) and high (+a) end: the overhang, or
    // on into a joined building. A joined end has no hip and no boards, and what reaches into the
    // other building is no wider than the walls (it would stick out where both are flush).
    let (lo_end, hi_end) = (la + ends[0].reach / U, la + ends[1].reach / U);
    let (m0, m1) = (if joined[0] { la } else { lo_end }, if joined[1] { la } else { hi_end });
    let rb = lb + o;
    // The pieces reaching into joined buildings: their spans along the ridge and across it (to
    // each side of the ridge: no further than the walls or the other building).
    let reaches: Vec<(f32, f32, [f32; 2])> = [(ends[0], -lo_end, -la), (ends[1], la, hi_end)]
        .into_iter()
        .filter(|r| r.0.joined)
        .map(|(e, a0, a1)| (a0, a1, [(lb.min(e.across.1 / U) - 0.02).max(0.0), (lb.min(-e.across.0 / U) - 0.02).max(0.0)]))
        .collect();
    let side_of = |s: f32, span: [f32; 2]| if s > 0.0 { span[0] } else { span[1] };
    let board = |b: &mut Builder, p0: Vec3, p1: Vec3, t: f32| b.tiled(&[p0, p1, p1 - Vec3::Y * t, p0 - Vec3::Y * t], p0 - Vec3::Y * t, c_trim);
    match shape.roof {
        RoofKind::Gable | RoofKind::Hip => {
            let eave = h - k * o;
            let ridge = h + k * lb;
            let hip = |reach: f32, joined: bool| if shape.roof == RoofKind::Hip && !joined { (reach - rb).max(0.0) } else { reach };
            let (rl0, rl1) = (hip(m0, joined[0]), hip(m1, joined[1]));
            for s in [1.0f32, -1.0] {
                let slope = [to_xz(-m0, eave, s * rb), to_xz(m1, eave, s * rb), to_xz(rl1, ridge, 0.0), to_xz(-rl0, ridge, 0.0)];
                let poly: Vec<Vec3> = slope.iter().copied().fold(Vec::new(), |mut v, p| {
                    if v.last().is_none_or(|q: &Vec3| (*q - p).length() > 1e-4) {
                        v.push(p);
                    }
                    v
                });
                b.tiled(&poly, slope[0], c_roof);
                for &(a0, a1, span) in &reaches {
                    let rbx = side_of(s, span);
                    let y = h + k * (lb - rbx);
                    if rbx > 0.0 {
                        b.face(&[to_xz(a0, y, s * rbx), to_xz(a1, y, s * rbx), to_xz(a1, ridge, 0.0), to_xz(a0, ridge, 0.0)], slope[0], c_roof, true);
                    }
                }
                // The board along the eave.
                board(&mut b, to_xz(-m0, eave, s * rb), to_xz(m1, eave, s * rb), BOARD);
                // The ends: a hip, else the boards along the gable's edges.
                for (sign, reach, rl, joined) in [(-1.0f32, m0, rl0, joined[0]), (1.0, m1, rl1, joined[1])] {
                    if joined || s < 0.0 && shape.roof == RoofKind::Hip {
                        continue;
                    }
                    if shape.roof == RoofKind::Hip {
                        let end = [to_xz(sign * reach, eave, -rb), to_xz(sign * reach, eave, rb), to_xz(sign * rl, ridge, 0.0)];
                        b.tiled(&end, end[0], c_roof);
                        board(&mut b, to_xz(sign * reach, eave, -rb), to_xz(sign * reach, eave, rb), BOARD);
                    } else {
                        board(&mut b, to_xz(sign * reach, eave, s * rb), to_xz(sign * reach, ridge, 0.0), BOARD);
                    }
                }
            }
        }
        RoofKind::Shed => {
            let y_at = |bb: f32| h + k * (lb - bb);
            let slope = [to_xz(-m0, y_at(rb), rb), to_xz(m1, y_at(rb), rb), to_xz(m1, y_at(-rb), -rb), to_xz(-m0, y_at(-rb), -rb)];
            b.tiled(&slope, slope[0], c_roof);
            for &(a0, a1, [p, n]) in &reaches {
                b.face(&[to_xz(a0, y_at(p), p), to_xz(a1, y_at(p), p), to_xz(a1, y_at(-n), -n), to_xz(a0, y_at(-n), -n)], slope[0], c_roof, true);
            }
            for (i, (p0, p1)) in [(slope[0], slope[1]), (slope[1], slope[2]), (slope[2], slope[3]), (slope[3], slope[0])].into_iter().enumerate() {
                if !(i == 1 && joined[1] || i == 3 && joined[0]) {
                    board(&mut b, p0, p1, BOARD);
                }
            }
        }
        RoofKind::Flat => {
            let y = h + BOARD;
            let top = [to_xz(-m0, y, -rb), to_xz(m1, y, -rb), to_xz(m1, y, rb), to_xz(-m0, y, rb)];
            b.tiled(&top, top[0], c_roof);
            for &(a0, a1, [p, n]) in &reaches {
                b.face(&[to_xz(a0, y, -n), to_xz(a1, y, -n), to_xz(a1, y, p), to_xz(a0, y, p)], top[0], c_roof, true);
            }
            for i in 0..4 {
                if !(i == 1 && joined[1] || i == 3 && joined[0]) {
                    board(&mut b, top[i], top[(i + 1) % 4], BOARD * 2.0);
                }
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
    let cast = b.mesh.indices.len();
    for (poly, origin, cell) in std::mem::take(&mut b.late) {
        b.tiled(&poly, origin, cell);
    }
    let shadowless = b.mesh.indices.len() - cast;
    (b.mesh, atlas.image, height, shadowless)
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
    fn a_roof_alone_has_no_walls() {
        let b = build(&BuildingShape { walls: false, height: 48.0, ..shape(RoofKind::Gable) }, [160.0, 128.0], &plain);
        let (lo, _) = bounds(&b.model);
        // Nothing below the eaves (the roof's edge hangs a little lower than the walls' top).
        assert!(lo.y > 48.0 / 32.0 - 0.3, "{lo}");
        assert!((b.height - (48.0 + 0.75 * 64.0)).abs() < 0.01, "{}", b.height);
    }

    #[test]
    fn joined_buildings_hide_each_other_and_share_a_roof() {
        use crate::defs::Join;
        // A 224 x 160 hall (ridge east-west) and a 128 x 160 wing south of its east end (ridge
        // north-south, into the hall).
        let hall = BuildingShape { joins: vec![Join { prop: "wing".into(), at: [96.0, 160.0] }], ..shape(RoofKind::Gable) };
        let wing = BuildingShape { joins: vec![Join { prop: "hall".into(), at: [-96.0, -160.0] }], ridge: Ridge::NorthSouth, ..shape(RoofKind::Gable) };
        let (h2, w2) = (hall.clone(), wing.clone());
        let others = move |id: &str| match id {
            "hall" => Some((h2.clone(), [224.0, 160.0])),
            "wing" => Some((w2.clone(), [128.0, 160.0])),
            _ => None,
        };
        let w = build_joined(&wing, [128.0, 160.0], &plain, &others);
        // The hall is the wing's hidden geometry, north of it.
        assert!(!w.model.occluder.is_empty() && w.model.occluder.iter().all(|p| p.z < -2.5 + 0.3));
        // The wing's roof runs north past its wall into the hall (as far as its ridge needs to meet
        // the hall's roof: 64 px), no wider than its walls there; that part casts no shadow.
        let (lo, _) = bounds(&w.model);
        assert!((lo.z - (-2.5 - 64.0 / 32.0)).abs() < 0.05, "{lo}");
        assert!(w.model.shadowless > 0 && w.model.shadowless < w.model.indices.len());
        // The hall's ridge runs along the wing's wall: it doesn't reach into the wing.
        let h = build_joined(&hall, [224.0, 160.0], &plain, &others);
        let (_, hi) = bounds(&h.model);
        assert!((hi.z - (2.5 + 6.0 / 32.0)).abs() < 1e-3, "{hi}");
        assert!(!h.model.occluder.is_empty() && h.model.shadowless == 0);
        // Alone, neither has hidden geometry.
        assert!(build(&wing, [128.0, 160.0], &plain).model.occluder.is_empty());
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
