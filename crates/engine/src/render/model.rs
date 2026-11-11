//! Drawing 3D character models as 2D images. A pose (model, colours, gear, animation frame,
//! facing) is rendered once off screen: the skinned model with cel shading (a few light bands, a
//! colour step), the held items and a carried prop as pictures at its hands, from an orthographic
//! camera looking down 20°, at the figures' pixel density (2 image pixels per world pixel); then a
//! dark outline is added while it is copied into an atlas page. The world draws that image like a
//! sprite, so depth sorting, lights, tints and the UI work as for any picture. Pages fill lazily
//! and are reused oldest first.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::prelude::*;

use crate::defs::{Appearance, Database, ModelDef};
use crate::entity::Facing;
use crate::hands::{Grip, Hand, HeldLayer};
use crate::model::Model;

/// Image pixels per world pixel of rendered characters (as the 2x figures).
pub const PX: f32 = 2.0;
/// Camera elevation (degrees): the figures' slightly-from-above view.
pub const ELEVATION: f32 = 20.0;
/// Feet this many image pixels above a frame's bottom (room for the shadow).
const FEET_MARGIN: f32 = 4.0;
const PAGE: u32 = 1024;
const MAX_PAGES: usize = 8;
/// Draw-call capacity the game and editor windows ask for: a model is one draw (up to about
/// 32k vertices and 30k triangles; generated models are kept near 5k triangles).
pub const DRAW_VERTICES: usize = 32_768;
pub const DRAW_INDICES: usize = 98_304;
/// New poses rendered per frame at most (the rest wait a frame, drawn from a nearby pose).
pub const RENDERS_PER_FRAME: u32 = 24;

/// What a character holds: the weapon (or the tool it uses), its left-hand item, a carried prop.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Gear {
    pub weapon: Option<String>,
    pub offhand: Option<String>,
    pub carrying: String,
}

impl Gear {
    /// From equipment, a carried prop and the tool in use (it replaces the weapon).
    pub fn of(eq: &crate::state::Equipment, carrying: &str, tool: Option<&str>) -> Gear {
        use crate::defs::EquipSlot;
        Gear { weapon: tool.or(eq.item(EquipSlot::Weapon)).map(str::to_string), offhand: eq.item(EquipSlot::Offhand).map(str::to_string), carrying: carrying.to_string() }
    }

    pub fn equipment(&self) -> crate::state::Equipment {
        use crate::inventory::ItemStack;
        crate::state::Equipment { weapon: self.weapon.as_ref().map(|i| ItemStack::new(i, 1)), offhand: self.offhand.as_ref().map(|i| ItemStack::new(i, 1)), ..Default::default() }
    }
}

/// A pose to draw: which frame of which animation, facing where, holding what.
#[derive(Clone, Debug)]
pub struct PoseReq<'a> {
    pub look: &'a Appearance,
    pub gear: &'a Gear,
    /// "walk", "idle", "attack", "hurt" or an extra animation ("sit", "fish"…).
    pub anim: &'a str,
    /// Seconds into the animation (looping), or for one-shots the fraction 0..1 when `once`.
    pub time: f32,
    pub once: bool,
    pub facing: Facing,
    /// A portrait: the head and shoulders close up ([`BUST`] pixels square, the rest pose).
    pub bust: bool,
}

/// The model characters without one of their own are drawn as (a project may have its own).
pub const MANNEQUIN: &str = "mannequin";
const BUILTIN: &str = "<engine>/mannequin.glb";
const BUILTIN_CLIPS: &str = "<engine>/mannequin_clips.glb";
const MANNEQUIN_GLB: &[u8] = include_bytes!("../../models/mannequin.glb");
const MANNEQUIN_CLIPS: &[u8] = include_bytes!("../../models/mannequin_clips.glb");

/// Size of a bust render (image pixels, square).
pub const BUST: u32 = 64;

/// What a model plays for a figure animation: (animation, time, once). `now`: the clock in
/// seconds (idle and extra animations loop on it).
pub fn anim_req(anim: super::character::Anim, now: f32) -> (&'static str, f32, bool) {
    use super::character::{Anim, LAYOUT_FRAMES};
    match anim {
        Anim::Walk(c) => ("walk", (c as f32 + 0.5) / LAYOUT_FRAMES as f32, true),
        Anim::Walking(t) => ("walk", t as f32 / 100.0, false),
        Anim::Idle(_) => ("idle", now, false),
        // Wind-up, swing, follow-through, recover: points of the clip (strikes sit in its
        // middle; the player's swing shows the last three).
        Anim::Attack(k) => ("attack", [0.22, 0.42, 0.58, 0.8][k.min(3) as usize], true),
        Anim::Hurt => ("hurt", 0.4, true),
        Anim::Extra(n, _) => (n, now, false),
    }
}

/// Where a rendered pose is: its atlas page and rectangle (image pixels), the feet point in it,
/// the tips of held items (fishing lines start there).
#[derive(Clone, Debug)]
pub struct Cell {
    page: usize,
    rect: Rect,
    pub feet: Vec2,
    /// The head bone (the base of the head) in the cell, for portraits.
    pub head: Option<Vec2>,
    pub tips: Vec<(String, Vec2)>,
    used: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PoseKey {
    model: String,
    colors: u64,
    gear: u64,
    anim: String,
    frame: u32,
    facing: u8,
}

/// A hand bone and the local axis of its thumb side (the grip axis).
#[derive(Clone, Copy, Debug)]
struct HandBone {
    node: usize,
    parent: Option<usize>,
    thumb: Vec3,
}

struct Loaded {
    def: ModelDef,
    model: Model,
    texture: Option<Image>,
    /// Turn per clip that makes it face the front (radians).
    align: HashMap<String, f32>,
    hands: [Option<HandBone>; 2],
    /// Frame size in image pixels, pixels per model unit, the camera target's height, the
    /// model's standing height (model units).
    frame: (u32, u32),
    height: f32,
    ppu: f32,
    target_y: f32,
    textures: RefCell<HashMap<u64, Texture2D>>,
    /// Rig templates: the unfitted model, the template and the template's clip map (refitting
    /// when the fit is edited).
    fitted: Option<Box<Fitted>>,
}

struct Fitted {
    base: Model,
    template: Model,
    clips: Vec<crate::defs::ClipMap>,
}

/// A model's definition as it plays: without clips of its own, a fitted model plays the
/// template's.
fn effective(def: &ModelDef, fitted: Option<&Fitted>) -> ModelDef {
    match fitted {
        Some(f) if def.clips.is_empty() => ModelDef { clips: f.clips.clone(), ..def.clone() },
        _ => def.clone(),
    }
}

struct Page {
    rt: RenderTarget,
    /// Shelf packing: next x, shelf top, shelf height.
    x: u32,
    y: u32,
    row: u32,
    stamp: u64,
}

#[derive(Default)]
struct Cache {
    pages: Vec<Page>,
    current: usize,
    poses: HashMap<PoseKey, Cell>,
    scratch: HashMap<(u32, u32), RenderTarget>,
    material: Option<Material>,
    frame: u64,
    renders: u32,
    renders_total: u64,
}

/// A held item's 3D model: its mesh as it stands in the file, its texture and its length.
struct ItemMesh {
    points: Vec<(Vec3, Vec3)>,
    uvs: Vec<Vec2>,
    indices: Vec<u16>,
    image: Option<Image>,
    texture: RefCell<Option<Texture2D>>,
    /// The longest side of its bounds (model units).
    len: f32,
}

/// The project's models and their rendered poses.
#[derive(Default)]
pub struct Models {
    loaded: HashMap<String, Loaded>,
    /// Held items' models by file.
    items: HashMap<String, ItemMesh>,
    cache: RefCell<Cache>,
    pub errors: Vec<String>,
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;
varying lowp vec2 uv;
varying lowp vec4 color;
varying mediump vec3 n;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
    color = color0 / 255.0;
    n = normal.xyz;
}
"#;

const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying lowp vec2 uv;
varying lowp vec4 color;
varying mediump vec3 n;
uniform sampler2D Texture;
uniform vec3 LightDir;
uniform float Shade;
void main() {
    vec4 c = texture2D(Texture, uv) * color;
    if (c.a < 0.5) discard;
    float d = dot(normalize(n), LightDir);
    float k = d > 0.35 ? 1.0 : (d > -0.2 ? 0.8 : 0.62);
    vec3 col = c.rgb * mix(1.0, k, Shade);
    col = floor(col * 15.0 + 0.5) / 15.0;
    gl_FragColor = vec4(col, 1.0);
}
"#;

impl Models {
    /// Loads the models of `db` (files through `read`, relative to the project).
    pub fn load(db: &Database, read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>) -> Models {
        let mut m = Models::default();
        // Characters without a model are drawn as the mannequin: the project's, else the engine's.
        if db.model(MANNEQUIN).is_none() {
            let def = ModelDef { clips: crate::model::guess_clips(&crate::model::clip_names(MANNEQUIN_CLIPS).unwrap_or_default()), clip_files: vec![BUILTIN_CLIPS.into()], ..ModelDef::new(MANNEQUIN, BUILTIN) };
            let builtin = |p: &str| -> std::io::Result<Vec<u8>> {
                Ok(match p {
                    BUILTIN => MANNEQUIN_GLB.to_vec(),
                    _ => MANNEQUIN_CLIPS.to_vec(),
                })
            };
            match load_one(&def, db, &builtin) {
                Ok(l) => {
                    m.loaded.insert(MANNEQUIN.into(), l);
                }
                Err(e) => eprintln!("[models] the built-in mannequin: {e:#}"),
            }
        }
        for def in &db.models {
            match load_one(def, db, read) {
                Ok(l) => {
                    m.loaded.insert(def.id.clone(), l);
                }
                Err(e) => {
                    eprintln!("[models] {}: {e:#}", def.id);
                    m.errors.push(format!("model {}: {e:#}", def.id));
                }
            }
        }
        for it in &db.items {
            let Some(im) = it.model.as_ref().filter(|im| !m.items.contains_key(&im.file)) else { continue };
            match read(&im.file).map_err(anyhow::Error::from).and_then(|b| Model::load(&b)) {
                Ok(model) => {
                    let mut points = Vec::new();
                    model.skin(&model.pose(None, 0.0), &mut points);
                    let (lo, hi) = points.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)));
                    let len = (hi - lo).max_element().max(1e-4);
                    let uvs = model.vertices.iter().map(|v| v.uv).collect();
                    m.items.insert(im.file.clone(), ItemMesh { points, uvs, indices: model.indices.clone(), image: model.texture.clone(), texture: RefCell::new(None), len });
                }
                Err(e) => {
                    eprintln!("[models] item {}: {e:#}", it.id);
                    m.errors.push(format!("item model {} ({}): {e:#}", it.id, im.file));
                }
            }
        }
        m
    }

    /// Forgets every rendered pose (an item model's marks changed).
    pub fn clear_poses(&self) {
        self.cache.borrow_mut().poses.clear();
    }

    /// An inventory icon of an item's model (`size` pixels square): the model alone, its handle
    /// pointing up-right like the drawn icons, cel shaded and outlined. Needs a window (GPU).
    pub fn render_item_icon(&self, spec: &crate::defs::ItemModel, size: u32) -> Option<Image> {
        let mesh = self.items.get(&spec.file)?;
        let rot = Quat::from_rotation_arc(Vec3::from(spec.axis).normalize_or(Vec3::Y), vec3(1.0, 1.0, 0.0).normalize());
        // A quarter turn about the handle shows the blade's face rather than its edge.
        let rot = Quat::from_axis_angle(vec3(1.0, 1.0, 0.0).normalize(), 0.5) * rot;
        let pts: Vec<(Vec3, Vec3)> = mesh.points.iter().map(|(p, n)| (rot * (*p - Vec3::from(spec.grip)), rot * *n)).collect();
        let (lo, hi) = pts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)));
        let center = (lo + hi) / 2.0;
        let span = (hi.x - lo.x).max(hi.y - lo.y) * 1.12;
        let vertices: Vec<Vertex> = pts.iter().zip(&mesh.uvs).map(|((p, n), uv)| Vertex { position: *p - center, uv: *uv, color: [255; 4], normal: vec4(n.x, n.y, n.z, 0.0) }).collect();
        let texture = mesh.texture.borrow_mut().get_or_insert_with(|| {
            let t = mesh.image.as_ref().map_or_else(|| Texture2D::from_rgba8(1, 1, &[200, 200, 200, 255]), Texture2D::from_image);
            t.set_filter(FilterMode::Nearest);
            t
        }).clone();
        let mut c = self.cache.borrow_mut();
        let mat = material(&mut c);
        let scratch = render_target_ex(size, size, RenderTargetParams { sample_count: 1, depth: true });
        scratch.texture.set_filter(FilterMode::Nearest);
        let out = render_target_ex(size, size, RenderTargetParams { sample_count: 1, depth: false });
        let cam = Camera3D {
            position: vec3(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fovy: span,
            aspect: Some(1.0),
            projection: Projection::Orthographics,
            render_target: Some(scratch.clone()),
            viewport: None,
            z_near: 0.1,
            z_far: 50.0,
        };
        push_camera_state();
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        gl_use_material(&mat);
        mat.set_uniform("LightDir", vec3(-0.45, 0.75, 0.55).normalize());
        mat.set_uniform("Shade", 1.0f32);
        draw_mesh(&Mesh { vertices, indices: mesh.indices.clone(), texture: Some(texture) });
        gl_use_default_material();
        let mut cam2 = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size as f32, size as f32));
        cam2.zoom.y = -cam2.zoom.y;
        cam2.render_target = Some(out.clone());
        set_camera(&cam2);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        let dark = Color::new(0.15, 0.10, 0.09, 1.0);
        let sz = Some(vec2(size as f32, size as f32));
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            draw_texture_ex(&scratch.texture, dx, dy, dark, DrawTextureParams { dest_size: sz, flip_y: true, ..Default::default() });
        }
        draw_texture_ex(&scratch.texture, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: sz, flip_y: true, ..Default::default() });
        pop_camera_state();
        Some(out.texture.get_texture_data())
    }

    /// Whether an item model file is loaded.
    pub fn has_item_model(&self, file: &str) -> bool {
        self.items.contains_key(file)
    }

    /// An item model's triangles (indices into its points).
    pub fn item_indices(&self, file: &str) -> Vec<u16> {
        self.items.get(file).map(|m| m.indices.clone()).unwrap_or_default()
    }

    /// An item model's points (as in the file) and longest side, for the editor's estimates.
    pub fn item_points(&self, file: &str) -> Option<(Vec<Vec3>, f32)> {
        self.items.get(file).map(|m| (m.points.iter().map(|p| p.0).collect(), m.len))
    }

    pub fn has(&self, id: &str) -> bool {
        self.loaded.contains_key(id)
    }

    /// A model's frame size in world pixels (as a figure's frame).
    pub fn frame_size(&self, id: &str) -> Option<(f32, f32)> {
        self.loaded.get(id).map(|l| (l.frame.0 as f32 / PX, l.frame.1 as f32 / PX))
    }

    /// Cache numbers for `status`: poses, pages, renders this frame, renders in all.
    pub fn status(&self) -> String {
        let c = self.cache.borrow();
        format!("{}/{}p/{}r/{}", c.poses.len(), c.pages.len(), c.renders, c.renders_total)
    }

    /// Call once per frame (resets the per-frame render budget).
    pub fn begin_frame(&self) {
        let mut c = self.cache.borrow_mut();
        c.frame += 1;
        c.renders = 0;
    }

    /// The rendered pose (rendering it now when needed and the frame's budget allows).
    pub fn pose(&self, db: &Database, sheets: &HashMap<String, super::SheetTex>, req: &PoseReq) -> Option<Cell> {
        let l = self.loaded.get(&req.look.model)?;
        // Busts use the rest pose: the head straight, facing the front.
        let (clip_name, frame, time) = if req.bust { (None, 0, 0.0) } else { clip_frame(l, req) };
        let anim = if req.bust { "@bust" } else { req.anim };
        let key = PoseKey { model: req.look.model.clone(), colors: hash(&req.look.colors), gear: hash(req.gear), anim: anim.to_string(), frame, facing: req.facing as u8 };
        let mut c = self.cache.borrow_mut();
        let now = c.frame;
        if let Some(cell) = c.poses.get_mut(&key) {
            cell.used = now;
            return Some(cell.clone());
        }
        if c.renders >= RENDERS_PER_FRAME {
            // Over budget: any frame of the same model, colours and facing, to draw meanwhile.
            return c.poses.iter().find(|(k, _)| k.model == key.model && k.colors == key.colors && k.facing == key.facing).map(|(_, v)| v.clone());
        }
        c.renders += 1;
        c.renders_total += 1;
        let cell = render(&mut c, l, &self.items, db, sheets, req, clip_name.as_deref(), time);
        c.poses.insert(key, cell.clone());
        Some(cell)
    }

    /// Draws a rendered pose with its feet at `feet` (world pixels), `size` times its size.
    pub fn draw(&self, cell: &Cell, feet: Vec2, size: f32, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        let dest = vec2(cell.rect.w, cell.rect.h) / PX * size;
        let top_left = feet - cell.feet / PX * size;
        draw_texture_ex(&page.rt.texture, top_left.x, top_left.y, tint, DrawTextureParams { source: Some(cell.rect), dest_size: Some(dest), ..Default::default() });
    }

    /// Draws a rendered pose into `dest` (screen pixels; UI portraits, windows).
    pub fn draw_in(&self, cell: &Cell, dest: Rect, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        draw_texture_ex(&page.rt.texture, dest.x, dest.y, tint, DrawTextureParams { source: Some(cell.rect), dest_size: Some(dest.size()), ..Default::default() });
    }

    /// Draws part `part` (fractions of the frame: x, y, w, h) of a rendered pose into `dest`.
    pub fn draw_part(&self, cell: &Cell, part: Rect, dest: Rect, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        let r = cell.rect;
        let src = Rect::new(r.x + part.x * r.w, r.y + part.y * r.h, part.w * r.w, part.h * r.h);
        draw_texture_ex(&page.rt.texture, dest.x, dest.y, tint, DrawTextureParams { source: Some(src), dest_size: Some(dest.size()), ..Default::default() });
    }

    /// Draws a rendered pose turned by `angle` (radians) about `center` (world pixels), as big
    /// as it is drawn standing (a character lying in a bed).
    pub fn draw_rotated(&self, cell: &Cell, center: Vec2, angle: f32, size: f32, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        let dest = vec2(cell.rect.w, cell.rect.h) / PX * size;
        let params = DrawTextureParams { source: Some(cell.rect), dest_size: Some(dest), rotation: angle, ..Default::default() };
        draw_texture_ex(&page.rt.texture, center.x - dest.x / 2.0, center.y - dest.y / 2.0, tint, params);
    }

    /// What model `id` is made of.
    pub fn info(&self, id: &str) -> Option<ModelInfo> {
        let l = self.loaded.get(id)?;
        let m = &l.model;
        Some(ModelInfo {
            vertices: m.vertices.len(),
            triangles: m.indices.len() / 3,
            bones: m.joints.iter().map(|&j| m.nodes[j].name.clone()).collect(),
            clips: m.clips.iter().map(|c| (c.name.clone(), c.duration)).collect(),
            hands: l.hands.iter().all(Option::is_some),
            texture: l.texture.as_ref().map(|t| (t.width, t.height)),
            rigged: l.fitted.is_none() && m.joints.len() >= 2,
        })
    }

    /// Where each colour of model `id`'s texture is on the model at rest: colour -> (average
    /// height, 0 = the feet .. 1 = the top; the number of triangles showing it). Telling the
    /// shades of hair from the same browns of boots, for colour features.
    pub fn texture_heights(&self, id: &str) -> HashMap<[u8; 3], (f32, u32)> {
        let mut out: HashMap<[u8; 3], (f32, u32)> = HashMap::new();
        let Some(l) = self.loaded.get(id) else { return out };
        let (Some(t), m) = (&l.texture, &l.model) else { return out };
        let mut pts = Vec::new();
        m.skin(&m.pose(None, 0.0), &mut pts);
        let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (p, _)| (lo.min(p.y), hi.max(p.y)));
        let h = (hi - lo).max(1e-5);
        let (w, th) = (t.width as f32, t.height as f32);
        for tri in m.indices.as_chunks::<3>().0 {
            let [Some(a), Some(b), Some(c)] = tri.map(|i| m.vertices.get(i as usize)) else { continue };
            let uv = (a.uv + b.uv + c.uv) / 3.0;
            let (x, y) = (((uv.x.rem_euclid(1.0)) * w) as u32, ((uv.y.rem_euclid(1.0)) * th) as u32);
            let k = ((y.min(t.height as u32 - 1) * t.width as u32 + x.min(t.width as u32 - 1)) * 4) as usize;
            let Some(px) = t.bytes.get(k..k + 4) else { continue };
            if px[3] == 0 {
                continue;
            }
            let at = tri.iter().filter_map(|&i| pts.get(i as usize)).map(|(p, _)| (p.y - lo) / h).sum::<f32>() / 3.0;
            let e = out.entry([px[0], px[1], px[2]]).or_default();
            e.0 += at;
            e.1 += 1;
        }
        for e in out.values_mut() {
            e.0 /= e.1.max(1) as f32;
        }
        out
    }

    /// Model `id` at rest with its joints (a fitted rig template's: the editor drags them).
    pub fn rig_view(&self, id: &str) -> Option<RigView> {
        let m = &self.loaded.get(id)?.model;
        let mut pts = Vec::new();
        m.skin(&m.pose(None, 0.0), &mut pts);
        let (lo, hi) = pts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)));
        // Corners and the middles of the triangles (a silhouette also for models of few, big faces).
        let mut all: Vec<Vec3> = pts.iter().map(|(p, _)| *p).collect();
        all.extend(m.indices.as_chunks::<3>().0.iter().filter_map(|t| Some((pts.get(t[0] as usize)?.0 + pts.get(t[1] as usize)?.0 + pts.get(t[2] as usize)?.0) / 3.0)));
        let step = (all.len() / 3000).max(1);
        let points = all.into_iter().step_by(step).collect();
        let rest = m.joints_at_rest();
        let joints = rest.iter().map(|(_, name, p, parent)| (name.clone(), *p, parent.and_then(|pn| rest.iter().position(|r| r.0 == pn)))).collect();
        Some(RigView { points, joints, lo, hi })
    }

    /// Model `id`'s texture as loaded (downsized), for picking colour features.
    pub fn texture_image(&self, id: &str) -> Option<&Image> {
        self.loaded.get(id)?.texture.as_ref()
    }

    /// Takes an edited definition (height, clips, colour features…) without reading the files
    /// again; its poses render anew. False when the model isn't loaded (a new file: reload).
    pub fn set_def(&mut self, def: &ModelDef) -> bool {
        let Some(l) = self.loaded.get_mut(&def.id) else { return false };
        if l.def == effective(def, l.fitted.as_deref()) {
            return true;
        }
        if l.def.file != def.file || l.def.clip_files != def.clip_files {
            return false;
        }
        if l.def.rig != def.rig {
            // A new fit of the same template is computed from the kept parts; anything else reads
            // the files again.
            let (Some(f), Some(rig)) = (&l.fitted, &def.rig) else { return false };
            if l.def.rig.as_ref().map(|r| &r.template) != Some(&rig.template) {
                return false;
            }
            match f.base.fit_rig(&f.template, rig.turn, &def.nudges()) {
                Ok(m) => l.model = m,
                Err(_) => return false,
            }
            let (lo, hi) = l.model.bounds();
            l.height = (hi.y - lo.y).max(1e-3);
        }
        (l.frame, l.ppu, l.target_y) = layout(def, l.height);
        l.def = effective(def, l.fitted.as_deref());
        l.textures.borrow_mut().clear();
        self.cache.borrow_mut().poses.retain(|k, _| k.model != def.id);
        true
    }

    /// The atlas texture of a rendered pose and the pose's place in it (texture coordinates
    /// 0..1): the editor shows it in egui without copying.
    pub fn cell_texture(&self, cell: &Cell) -> Option<(Texture2D, Rect)> {
        let c = self.cache.borrow();
        let page = c.pages.get(cell.page)?;
        let r = cell.rect;
        let p = PAGE as f32;
        Some((page.rt.texture.clone(), Rect::new(r.x / p, r.y / p, r.w / p, r.h / p)))
    }

    /// Whether model `id` has a clip for animation `anim`.
    pub fn has_anim(&self, id: &str, anim: &str) -> bool {
        self.loaded.get(id).is_some_and(|l| l.def.clip_of(anim).is_some_and(|c| l.model.clip(&c.clip).is_some()))
    }

    /// `RUSTPG2D_MODEL_DEBUG=1`: the first atlas page in the screen's corner (screen camera).
    pub fn debug_draw(&self) {
        if std::env::var_os("RUSTPG2D_MODEL_DEBUG").is_none() {
            return;
        }
        let c = self.cache.borrow();
        if let Some(p) = c.pages.first() {
            draw_rectangle(0.0, 0.0, 512.0, 512.0, Color::new(0.2, 0.3, 0.2, 1.0));
            draw_texture_ex(&p.rt.texture, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(512.0, 512.0)), ..Default::default() });
            for cell in c.poses.values().filter(|v| v.page == 0) {
                let k = 512.0 / PAGE as f32;
                draw_rectangle_lines(cell.rect.x * k, cell.rect.y * k, cell.rect.w * k, cell.rect.h * k, 1.0, YELLOW);
                draw_circle((cell.rect.x + cell.feet.x) * k, (cell.rect.y + cell.feet.y) * k, 2.0, RED);
            }
        }
    }

    /// The model's colour features (for the creator and the editor).
    pub fn def(&self, id: &str) -> Option<&ModelDef> {
        self.loaded.get(id).map(|l| &l.def)
    }
}

fn hash<T: Hash>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn load_one(def: &ModelDef, db: &Database, read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>) -> anyhow::Result<Loaded> {
    use anyhow::Context;
    let load = |def: &ModelDef| -> anyhow::Result<Model> {
        let mut model = Model::load(&read(&def.file).with_context(|| format!("reading {}", def.file))?)?;
        for f in &def.clip_files {
            model.add_clips(&read(f).with_context(|| format!("reading {f}"))?)?;
        }
        Ok(model)
    };
    let mut model = load(def)?;
    let mut fitted = None;
    if let Some(rig) = &def.rig {
        let tdef = db.model(&rig.template).filter(|t| t.id != def.id).ok_or_else(|| anyhow::anyhow!("no rig template \"{}\"", rig.template))?;
        let template = load(tdef).with_context(|| format!("rig template {}", tdef.id))?;
        let fit = model.fit_rig(&template, rig.turn, &def.nudges())?;
        fitted = Some(Box::new(Fitted { base: std::mem::replace(&mut model, fit), template, clips: tdef.clips.clone() }));
    }
    let mut align = HashMap::new();
    for c in &model.clips {
        if let Some(yaw) = model.facing_of(Some(c)) {
            align.insert(c.name.clone(), -yaw);
        }
    }
    let rest = model.pose(None, 0.0);
    let hand = |side: &str| -> Option<HandBone> {
        let node = model.node(&format!("{side}hand"))?;
        let parent = model.nodes[node].parent;
        let rot = Mat3::from_mat4(rest[node]);
        // The local axis that points most to the front (+Z) at rest: the thumb side.
        let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let thumb = axes.into_iter().max_by(|a, b| (rot * *a).normalize_or_zero().z.total_cmp(&(rot * *b).normalize_or_zero().z))?;
        Some(HandBone { node, parent, thumb })
    };
    let hands = [hand("right"), hand("left")];
    let (lo, hi) = model.bounds();
    let height = (hi.y - lo.y).max(1e-3);
    let (frame, ppu, target_y) = layout(def, height);
    let texture = model.texture.clone();
    Ok(Loaded { def: effective(def, fitted.as_deref()), model, texture, align, hands, frame, height, ppu, target_y, textures: RefCell::new(HashMap::new()), fitted })
}

/// A model's frame (image pixels), pixels per model unit and camera target height: the standing
/// figure 2 x `def.height` image pixels tall in a frame with room for raised arms and swings.
fn layout(def: &ModelDef, height: f32) -> ((u32, u32), f32, f32) {
    let fig = def.height.max(4.0) * PX;
    let ppu = fig / height;
    let frame = (((fig * 1.1) as u32 + 1) & !1, ((fig * 1.3) as u32 + 1) & !1);
    let target_y = (frame.1 as f32 / 2.0 - FEET_MARGIN) / (ppu * ELEVATION.to_radians().cos());
    (frame, ppu, target_y)
}

/// What a loaded model is made of (the editor's Models tab).
#[derive(Clone, Debug, Default)]
pub struct ModelInfo {
    pub vertices: usize,
    pub triangles: usize,
    pub bones: Vec<String>,
    /// Name and seconds of every clip (its own file's and the clip files').
    pub clips: Vec<(String, f32)>,
    /// Hand bones found (gear can be held).
    pub hands: bool,
    pub texture: Option<(u16, u16)>,
    /// Its own skeleton: false for an unrigged model (with or without a rig template fitted).
    pub rigged: bool,
}

/// A model at rest for editing a fitted rig template: a sample of its mesh's points and the
/// joints (name, position, parent joint index), in model space; its height.
pub struct RigView {
    pub points: Vec<Vec3>,
    pub joints: Vec<(String, Vec3, Option<usize>)>,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// The clip of a request, its frame number and the time in it.
fn clip_frame(l: &Loaded, req: &PoseReq) -> (Option<String>, u32, f32) {
    let map = l.def.clip_of(req.anim).or_else(|| l.def.clip_of("idle"));
    let clip = map.and_then(|m| l.model.clip(&m.clip)).or_else(|| l.model.clips.first());
    let Some(clip) = clip else { return (None, 0, 0.0) };
    let fps = map.map_or(12.0, |m| m.fps).max(1.0);
    let n = ((clip.duration * fps).round() as u32).max(1);
    let frame = if req.once { ((req.time.clamp(0.0, 0.999) * n as f32) as u32).min(n - 1) } else { (req.time.max(0.0) * fps) as u32 % n };
    (Some(clip.name.clone()), frame, frame as f32 / fps)
}

fn facing_yaw(f: Facing) -> f32 {
    match f {
        Facing::Down => 0.0,
        Facing::Left => -std::f32::consts::FRAC_PI_2,
        Facing::Right => std::f32::consts::FRAC_PI_2,
        Facing::Up => std::f32::consts::PI,
    }
}

fn material(c: &mut Cache) -> Material {
    if c.material.is_none() {
        let pipeline = PipelineParams {
            depth_write: true,
            depth_test: Comparison::LessOrEqual,
            color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
            ..Default::default()
        };
        let m = load_material(
            ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
            MaterialParams { pipeline_params: pipeline, uniforms: vec![UniformDesc::new("LightDir", UniformType::Float3), UniformDesc::new("Shade", UniformType::Float1)], textures: vec![] },
        )
        .expect("model shader");
        c.material = Some(m);
    }
    c.material.clone().unwrap()
}

/// A page cell of `w x h` pixels (a new page or the oldest one reused when full).
fn alloc(c: &mut Cache, w: u32, h: u32) -> (usize, Rect) {
    loop {
        if let Some(p) = c.pages.get_mut(c.current) {
            if p.x + w > PAGE {
                p.x = 0;
                p.y += p.row;
                p.row = 0;
            }
            if p.y + h <= PAGE {
                let r = Rect::new(p.x as f32, p.y as f32, w as f32, h as f32);
                p.x += w;
                p.row = p.row.max(h);
                p.stamp = c.frame;
                return (c.current, r);
            }
        }
        if c.pages.len() < MAX_PAGES {
            let rt = render_target_ex(PAGE, PAGE, RenderTargetParams { sample_count: 1, depth: false });
            rt.texture.set_filter(FilterMode::Nearest);
            push_camera_state();
            set_camera(&Camera2D { render_target: Some(rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, PAGE as f32, PAGE as f32)) });
            clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
            pop_camera_state();
            c.pages.push(Page { rt, x: 0, y: 0, row: 0, stamp: c.frame });
            c.current = c.pages.len() - 1;
        } else {
            // Reuse the page filled longest ago: its poses render again when needed.
            let oldest = (0..c.pages.len()).min_by_key(|&i| c.pages[i].stamp).unwrap_or(0);
            c.poses.retain(|_, cell| cell.page != oldest);
            let p = &mut c.pages[oldest];
            (p.x, p.y, p.row) = (0, 0, 0);
            push_camera_state();
            set_camera(&Camera2D { render_target: Some(p.rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, PAGE as f32, PAGE as f32)) });
            clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
            pop_camera_state();
            c.current = oldest;
        }
    }
}

/// The texture of `l` in the colours of `look` (its colour features recoloured).
fn texture_for(l: &Loaded, look: &Appearance) -> Option<Texture2D> {
    let key = hash(&look.colors);
    if let Some(t) = l.textures.borrow().get(&key) {
        return Some(t.clone());
    }
    let mut img = l.texture.clone()?;
    let features: Vec<(&[[u8; 3]], [u8; 3])> = l.def.features.iter().filter_map(|f| look.colors.get(&f.name).map(|c| (f.shades.as_slice(), *c))).collect();
    if !features.is_empty() {
        super::recolor::recolor(&mut img.bytes, &features);
    }
    let t = Texture2D::from_image(&img);
    t.set_filter(FilterMode::Nearest);
    l.textures.borrow_mut().insert(key, t.clone());
    Some(t)
}

/// Renders one pose into the atlas.
#[allow(clippy::too_many_arguments)]
fn render(c: &mut Cache, l: &Loaded, items: &HashMap<String, ItemMesh>, db: &Database, sheets: &HashMap<String, super::SheetTex>, req: &PoseReq, clip: Option<&str>, time: f32) -> Cell {
    let (w, h) = if req.bust { (BUST, BUST) } else { l.frame };
    let clip_ref = clip.and_then(|n| l.model.clip(n));
    let world = l.model.pose(clip_ref, time);
    let mut skinned = Vec::new();
    l.model.skin(&world, &mut skinned);
    let yaw = facing_yaw(req.facing) + clip.and_then(|n| l.align.get(n)).copied().unwrap_or(0.0);
    let turn = Mat4::from_rotation_y(yaw);
    let vertices: Vec<Vertex> = skinned
        .iter()
        .zip(&l.model.vertices)
        .map(|((p, n), v)| {
            let n = turn.transform_vector3(*n);
            Vertex { position: turn.transform_point3(*p), uv: v.uv, color: [255; 4], normal: vec4(n.x, n.y, n.z, 0.0) }
        })
        .collect();
    let e = ELEVATION.to_radians();
    // A bust looks at the head (its bone is at the head's base); a pose at the whole figure.
    let head_bone = l.model.node("head").map(|n| turn.transform_point3(world[n].w_axis.truncate()));
    let target = match (req.bust, head_bone) {
        (true, Some(hb)) => hb + vec3(0.0, l.height * 0.02, 0.0),
        (true, None) => vec3(0.0, l.height * 0.85, 0.0),
        _ => vec3(0.0, l.target_y, 0.0),
    };
    let span = if req.bust { l.height * 0.42 } else { h as f32 / l.ppu };
    let back = vec3(0.0, e.sin(), e.cos());
    let scratch = c
        .scratch
        .entry((w, h))
        .or_insert_with(|| {
            let rt = render_target_ex(w, h, RenderTargetParams { sample_count: 1, depth: true });
            rt.texture.set_filter(FilterMode::Nearest);
            rt
        })
        .clone();
    let cam = Camera3D {
        position: target + back * 50.0,
        target,
        up: Vec3::Y,
        fovy: span,
        aspect: Some(w as f32 / h as f32),
        projection: Projection::Orthographics,
        render_target: Some(scratch.clone()),
        viewport: None,
        z_near: 0.1,
        z_far: 100.0,
    };
    let view = cam.matrix();
    // Model point -> frame pixel (y down).
    let to_px = |p: Vec3| {
        let q = view.project_point3(p);
        vec2((q.x + 1.0) / 2.0 * w as f32, (1.0 - q.y) / 2.0 * h as f32)
    };
    let screen_right = Vec3::X;
    let screen_down = -vec3(0.0, e.cos(), -e.sin());
    // The hands: grip point (a little past the wrist), grip axis, in model space after the turn.
    let hand = |hb: &Option<HandBone>| -> Option<(Vec3, Vec3)> {
        let hb = (*hb)?;
        let m = world[hb.node];
        let wrist = m.w_axis.truncate();
        let elbow = hb.parent.map_or(wrist - Vec3::Y, |p| world[p].w_axis.truncate());
        let along = (wrist - elbow).normalize_or_zero();
        let grip = wrist + along * (0.07 * l.height / 1.7);
        let axis = Mat3::from_mat4(m).mul_vec3(hb.thumb).normalize_or_zero();
        Some((turn.transform_point3(grip), turn.transform_vector3(axis)))
    };
    let hands3 = [hand(&l.hands[0]), hand(&l.hands[1])];
    let grip2 = |h: Option<(Vec3, Vec3)>| {
        h.map(|(p, a)| {
            let s = to_px(p);
            let d = to_px(p + a * 0.1) - s;
            Grip { x: s.x, y: s.y, angle: d.y.atan2(d.x).to_degrees(), scale: 1.0, front: true }
        })
    };
    let (gr, gl) = (grip2(hands3[0]), grip2(hands3[1]));
    let eq = req.gear.equipment();
    let layers = if req.bust { Vec::new() } else { crate::hands::held_with(db, (gr, gl), &eq, &req.gear.carrying, None, req.facing) };
    let mat = material(c);
    push_camera_state();
    set_camera(&cam);
    clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
    gl_use_material(&mat);
    let light = vec3(-0.45, 0.75, 0.55).normalize();
    mat.set_uniform("LightDir", light);
    mat.set_uniform("Shade", 1.0f32);
    draw_mesh(&Mesh { vertices, indices: l.model.indices.clone(), texture: texture_for(l, req.look) });
    // Held items and a carried prop: their pictures at the hands, facing the camera, unshaded.
    mat.set_uniform("Shade", 0.0f32);
    let mut tips = Vec::new();
    for layer in &layers {
        let anchor = match layer.hand {
            Hand::Right => hands3[0].map(|h| h.0),
            Hand::Left => hands3[1].map(|h| h.0),
            Hand::Both => match (hands3[0], hands3[1]) {
                (Some(a), Some(b)) => Some((a.0 + b.0) / 2.0),
                (a, b) => a.or(b).map(|h| h.0),
            },
        };
        let Some(mut anchor) = anchor else { continue };
        // An item with its own 3D model: its mesh on the hand, the handle along the grip axis.
        let im = db.item(&layer.id).and_then(|d| d.model.as_ref()).and_then(|m| items.get(&m.file).map(|mesh| (m, mesh)));
        if let Some((spec, mesh)) = im {
            let axis = match layer.hand {
                Hand::Right => hands3[0].map(|h| h.1),
                Hand::Left => hands3[1].map(|h| h.1),
                Hand::Both => match (hands3[0], hands3[1]) {
                    (Some(r), Some(lh)) if (r.0 - lh.0).length() > l.height * 0.08 => {
                        let d = (r.0 - lh.0).normalize_or_zero();
                        Some(if d.dot(r.1) >= 0.0 { d } else { -d })
                    }
                    (r, lh) => r.or(lh).map(|h| h.1),
                },
            };
            let Some(axis) = axis else { continue };
            let k = spec.size * PX / l.ppu / mesh.len;
            let rot = Quat::from_rotation_arc(Vec3::from(spec.axis).normalize_or(Vec3::Y), axis.normalize_or(Vec3::Y));
            let grip = Vec3::from(spec.grip);
            let place = |p: Vec3| anchor + rot * ((p - grip) * k);
            let vertices: Vec<Vertex> = mesh.points.iter().zip(&mesh.uvs).map(|((p, n), uv)| {
                let n = rot * *n;
                Vertex { position: place(*p), uv: *uv, color: [255; 4], normal: vec4(n.x, n.y, n.z, 0.0) }
            }).collect();
            let texture = mesh.texture.borrow_mut().get_or_insert_with(|| {
                let t = mesh.image.as_ref().map_or_else(|| Texture2D::from_rgba8(1, 1, &[200, 200, 200, 255]), Texture2D::from_image);
                t.set_filter(FilterMode::Nearest);
                t
            }).clone();
            mat.set_uniform("Shade", 1.0f32);
            draw_mesh(&Mesh { vertices, indices: mesh.indices.clone(), texture: Some(texture) });
            mat.set_uniform("Shade", 0.0f32);
            let far = mesh.points.iter().map(|p| p.0).max_by(|a, b| (*a - grip).length_squared().total_cmp(&(*b - grip).length_squared())).unwrap_or(grip);
            tips.push((layer.id.clone(), to_px(place(far))));
            continue;
        }
        let mut layer = layer.clone();
        if layer.id == req.gear.carrying {
            // A carried prop is held level in front of the chest, not between the hanging hands.
            anchor += turn.transform_vector3(Vec3::Z) * l.height * 0.14 + Vec3::Y * l.height * 0.08;
            (layer.rotation, layer.flip) = (0.0, false);
        }
        if let Some((mesh, tip)) = held_quad(&layer, anchor, sheets, l.ppu, screen_right, screen_down) {
            draw_mesh(&mesh);
            tips.push((layer.id.clone(), to_px(tip)));
        }
    }
    gl_use_default_material();
    // Into the atlas, with a dark outline: the picture four times shifted by a pixel in a dark
    // tint, then itself.
    let (page, rect) = alloc(c, w + 2, h + 2);
    set_camera(&page_camera(&c.pages[page].rt));
    // (A tint multiplies: this darkens every colour to the outline's dark brown or below.)
    let dark = Color::new(0.15, 0.10, 0.09, 1.0);
    let (x, y) = (rect.x + 1.0, rect.y + 1.0);
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        draw_texture_ex(&scratch.texture, x + dx, y + dy, dark, DrawTextureParams { dest_size: Some(vec2(w as f32, h as f32)), flip_y: true, ..Default::default() });
    }
    draw_texture_ex(&scratch.texture, x, y, WHITE, DrawTextureParams { dest_size: Some(vec2(w as f32, h as f32)), flip_y: true, ..Default::default() });
    pop_camera_state();
    let feet = to_px(Vec3::ZERO) + vec2(1.0, 1.0);
    let head = l.model.node("head").map(|n| to_px(turn.transform_point3(world[n].w_axis.truncate())) + vec2(1.0, 1.0));
    Cell { page, rect, feet, head, tips: tips.into_iter().map(|(id, p)| (id, p + vec2(1.0, 1.0))).collect(), used: c.frame }
}

/// A camera drawing into an atlas page in its texture's own pixels (y down as sampled).
fn page_camera(rt: &RenderTarget) -> Camera2D {
    let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, PAGE as f32, PAGE as f32));
    cam.zoom.y = -cam.zoom.y;
    cam.render_target = Some(rt.clone());
    cam
}

/// A held picture as a quad facing the camera at `anchor` (the hold point), turned and mirrored
/// as `layer` says; with the picture's far tip. `ppu`: frame pixels per model unit.
fn held_quad(layer: &HeldLayer, anchor: Vec3, sheets: &HashMap<String, super::SheetTex>, ppu: f32, right: Vec3, down: Vec3) -> Option<(Mesh, Vec3)> {
    let s = sheets.get(&layer.sprite.sheet)?;
    let (cw, ch) = layer.sprite.cells();
    let src = s.region(layer.sprite.frame, cw, ch);
    // Frame pixels: its world size times the layer's scale, at the frame's pixel density.
    let size = s.world_of(src) * layer.scale * PX;
    let g = vec2(layer.grip[0] * size.x, layer.grip[1] * size.y);
    let (sn, cs) = layer.rotation.to_radians().sin_cos();
    let rot = |v: Vec2| vec2(v.x * cs - v.y * sn, v.x * sn + v.y * cs);
    let to3 = |v: Vec2| anchor + right * (v.x / ppu) + down * (v.y / ppu);
    let corners = [vec2(0.0, 0.0), vec2(size.x, 0.0), vec2(size.x, size.y), vec2(0.0, size.y)].map(|c| to3(rot(c - g)));
    let (iw, ih) = (s.image.width as f32, s.image.height as f32);
    let (u0, v0, u1, v1) = (src.x / iw, src.y / ih, (src.x + src.w) / iw, (src.y + src.h) / ih);
    let (top, bottom) = if layer.flip { (v1, v0) } else { (v0, v1) };
    let uvs = [vec2(u0, top), vec2(u1, top), vec2(u1, bottom), vec2(u0, bottom)];
    let vertices = (0..4).map(|k| Vertex { position: corners[k], uv: uvs[k], color: [255; 4], normal: vec4(0.0, 1.0, 0.0, 0.0) }).collect();
    // The farthest corner from the hold point stands in for the tip.
    let far = [vec2(0.0, 0.0), vec2(size.x, 0.0), vec2(size.x, size.y), vec2(0.0, size.y)].into_iter().max_by(|a, b| (*a - g).length_squared().total_cmp(&(*b - g).length_squared())).unwrap_or(g);
    Some((Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: Some(s.texture.clone()) }, to3(rot(far - g))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::ClipMap;

    fn loaded() -> Loaded {
        let glb = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/model_two_bones.glb")).unwrap();
        let def = ModelDef { clips: vec![ClipMap { anim: "walk".into(), clip: "bend".into(), fps: 10.0 }], ..ModelDef::new("m", "models/m.glb") };
        load_one(&def, &Database::default(), &|_| Ok(glb.clone())).unwrap()
    }

    #[test]
    fn poses_step_through_their_clip() {
        let l = loaded();
        // 2 units tall, drawn 40 world px = 80 image px: 40 px per unit; the frame has room.
        assert_eq!(l.ppu, 40.0);
        assert!(l.frame.0 >= 88 && l.frame.1 >= 104 && l.frame.0.is_multiple_of(2));
        let look = Appearance { model: "m".into(), ..Default::default() };
        let gear = Gear::default();
        let req = |anim, time, once| PoseReq { look: &look, gear: &gear, anim, time, once, facing: Facing::Down, bust: false };
        // 1 s at 10 fps: 10 frames; looping time, one-shot fraction; unknown animations play
        // idle, else the first clip.
        assert_eq!(clip_frame(&l, &req("walk", 0.35, false)), (Some("bend".into()), 3, 0.3));
        assert_eq!(clip_frame(&l, &req("walk", 1.25, false)).1, 2);
        assert_eq!(clip_frame(&l, &req("walk", 0.999, true)).1, 9);
        assert_eq!(clip_frame(&l, &req("sit", 0.0, false)).0.as_deref(), Some("bend"));
        // No hands in this model: nothing to hold things with.
        assert!(l.hands.iter().all(Option::is_none));
    }

    #[test]
    fn gear_takes_the_tool_in_use_instead_of_the_weapon() {
        use crate::inventory::ItemStack;
        let eq = crate::state::Equipment { weapon: Some(ItemStack::new("sword", 1)), offhand: Some(ItemStack::new("shield", 1)), ..Default::default() };
        let g = Gear::of(&eq, "", Some("rod"));
        assert_eq!((g.weapon.as_deref(), g.offhand.as_deref()), (Some("rod"), Some("shield")));
        assert_eq!(Gear::of(&eq, "crate", None).carrying, "crate");
        assert_eq!(g.equipment().item(crate::defs::EquipSlot::Weapon), Some("rod"));
    }
}
