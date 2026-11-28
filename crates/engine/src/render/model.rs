//! Drawing 3D character models as 2D images. A pose (model, colours, gear, animation frame,
//! facing) is rendered once off screen: the skinned model softly lit (a warm sun from the
//! north-west over a cool sky, like the pre-rendered props), the held items and a carried prop as pictures at its hands, from an orthographic
//! camera looking down 30° (portraits 20°), at the figures' pixel density (2 image pixels per world pixel); then a
//! dark outline is added while it is copied into an atlas page. The world draws that image like a
//! sprite, so depth sorting, lights, tints and the UI work as for any picture. Pages fill lazily;
//! when all are full, the page drawn from longest ago is cleared and reused.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use gfx::material::{BlendFactor, BlendState, BlendValue, Equation};
use gfx::prelude::*;

use crate::defs::{Appearance, ClipMap, Database, HoldStyle, ModelDef, DEFAULT_HIT};
use crate::entity::Facing;
use crate::hands::{Hand, InHand};
use crate::model::{hold_rotation, Model};

/// Image pixels per world pixel of rendered characters (as the 2x figures).
pub const PX: f32 = 2.0;
/// Camera elevation (degrees): the isometric view's 30° from above (a 2:1 ground, `render::iso`).
pub const ELEVATION: f32 = 30.0;
/// Portraits (busts) look at the face from a little above.
const BUST_ELEVATION: f32 = 20.0;
/// Figures turn this much against the world, so the world's axes run along the isometric
/// view's diamonds (south = down-left on the screen).
pub const ISO_TURN: f32 = -std::f32::consts::FRAC_PI_4;
/// Feet this many image pixels above a frame's bottom (room for the shadow).
const FEET_MARGIN: f32 = 4.0;
const PAGE: u32 = 1024;
/// Megabytes of one atlas page (RGBA).
const PAGE_MB: u32 = PAGE * PAGE * 4 / (1024 * 1024);
/// The pose atlas's size unless the project sets one (`ProjectConfig::model_atlas_mb`).
pub const DEFAULT_ATLAS_MB: u32 = 32;
/// The sizes a project may choose (MB).
pub const ATLAS_MB: std::ops::RangeInclusive<u32> = 8..=128;

/// Pages of an atlas of `mb` megabytes (clamped to [`ATLAS_MB`]).
pub fn atlas_pages(mb: u32) -> usize {
    (mb.clamp(*ATLAS_MB.start(), *ATLAS_MB.end()) / PAGE_MB) as usize
}
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
    /// A backpack (`EquipSlot::Back`): its model on the upper back.
    pub back: Option<String>,
    /// In combat (`runtime/battle.rs`): the weapon's stance and a one-handed weapon in the hand;
    /// out of it the plain idle and walk, and a one-handed weapon hangs at the hip.
    pub ready: bool,
    /// Worn items whose [`crate::defs::ItemDef::wear`] may show on the figure: clothing, body
    /// armour, cloak, headgear, in that order (a later one takes a slot from an earlier one).
    pub worn: Vec<String>,
}

impl Gear {
    /// From equipment, a carried prop and the tool in use (it replaces the weapon).
    pub fn of(eq: &crate::state::Equipment, carrying: &str, tool: Option<&str>) -> Gear {
        use crate::defs::EquipSlot;
        Gear {
            weapon: tool.or(eq.item(EquipSlot::Weapon)).map(str::to_string),
            offhand: eq.item(EquipSlot::Offhand).map(str::to_string),
            carrying: carrying.to_string(),
            back: eq.item(EquipSlot::Back).map(str::to_string),
            ready: false,
            worn: [EquipSlot::Clothing, EquipSlot::Body, EquipSlot::Feet, EquipSlot::Hands, EquipSlot::Cloak, EquipSlot::Head].into_iter().filter_map(|s| eq.item(s)).map(str::to_string).collect(),
        }
    }

    pub fn equipment(&self) -> crate::state::Equipment {
        use crate::inventory::ItemStack;
        let one = |i: &Option<String>| i.as_ref().map(|i| ItemStack::new(i, 1));
        crate::state::Equipment { weapon: one(&self.weapon), offhand: one(&self.offhand), back: one(&self.back), ..Default::default() }
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
    /// Turned a further 45° from `facing` (-1, 0, 1; positive turns as from down towards right):
    /// characters walking diagonally ([`diagonal_turn`]).
    pub turn: i8,
    /// A portrait: the head and shoulders close up ([`BUST`] pixels square, the rest pose).
    pub bust: bool,
}

/// The extra turn of a character that faces `facing` and moves along `heading` (screen
/// pixels): -1 or 1 when the heading is one of the four diagonals next to the facing, else 0.
pub fn diagonal_turn(facing: Facing, heading: Vec2) -> i8 {
    if heading.length_squared() < 1e-6 {
        return 0;
    }
    // Eighths of a turn, as `facing_yaw` counts them: 0 down, 2 right, 4 up, -2 left.
    let k = (heading.x.atan2(heading.y) / std::f32::consts::FRAC_PI_4).round() as i32;
    let f = (facing_yaw(facing) / std::f32::consts::FRAC_PI_4).round() as i32;
    let d = (k - f).rem_euclid(8);
    match d {
        1 => 1,
        7 => -1,
        _ => 0,
    }
}

/// The compass direction a pose shows (for `status`): `s`, `se`, `e`, `ne`, `n`, `nw`, `w`, `sw`.
pub fn compass(facing: Facing, turn: i8) -> &'static str {
    let f = (facing_yaw(facing) / std::f32::consts::FRAC_PI_4).round() as i32;
    ["s", "se", "e", "ne", "n", "nw", "w", "sw"][(f + turn as i32).rem_euclid(8) as usize]
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
        // (Without a model's hit point; `Models::clip_req` places them by it.)
        Anim::Windup(p) => ("attack", DEFAULT_HIT * p as f32 / 1000.0, true),
        Anim::Swing(p) => ("attack", DEFAULT_HIT + (1.0 - DEFAULT_HIT) * p as f32 / 1000.0, true),
        Anim::Strike(p) => ("attack", strike_time(DEFAULT_HIT, p), true),
        Anim::Block(p) => ("block", DEFAULT_HIT * p as f32 / 1000.0, true),
    }
}

/// A grip axis that doesn't run back into the forearm (`along` = wrist from the elbow): a
/// clip's fist turned over in a thrust would otherwise point a spear at the wearer's elbow.
fn out_of_fist(axis: Vec3, along: Vec3) -> Vec3 {
    if axis.dot(along) < -0.5 {
        -axis
    } else {
        axis
    }
}

/// The player's strike (`k` per mille of it) in a clip whose blow lands at `hit`: the damage is
/// dealt at once, so the wind-up (from 60 % of it) is hurried through the first quarter and the
/// blow lands there; the rest of the clip follows.
fn strike_time(hit: f32, k: u16) -> f32 {
    const LAND: f32 = 0.25;
    let p = k.min(1000) as f32 / 1000.0;
    if p < LAND {
        hit * (0.6 + 0.4 * p / LAND)
    } else {
        hit + (1.0 - hit) * (p - LAND) / (1.0 - LAND)
    }
}

/// The weapon that hangs at the hip instead of being in the hand: out of combat (`!gear.ready`),
/// not attacking or blocking (`anim`: the clip's animation name), a one-handed melee weapon with
/// a 3D model that isn't long (spears stay in the hands: [`crate::defs::Weapon::long`]).
pub fn sheathed<'a>(db: &Database, gear: &'a Gear, anim: &str) -> Option<&'a str> {
    if gear.ready || anim.starts_with("attack") || anim.starts_with("block") {
        return None;
    }
    let id = gear.weapon.as_deref()?;
    let d = db.item(id)?;
    let w = d.weapon.as_ref()?;
    (w.ranged.is_none() && !w.two_handed && !w.long() && d.model.is_some()).then_some(id)
}

/// The animation name an item names for a base animation (`attack`, `idle`, `walk`, `block`).
fn item_anim(db: &Database, item: Option<&String>, base: &str) -> Option<String> {
    let d = db.item(item?)?;
    let a = crate::defs::ItemAnims::of(d);
    let name = match base {
        "attack" => a.attack,
        "idle" => a.idle,
        "walk" => a.walk,
        "block" => a.block,
        _ => String::new(),
    };
    (!name.is_empty()).then_some(name)
}

/// Where a rendered pose is: its atlas page and rectangle (image pixels), the feet point in it,
/// the tips of held items (fishing lines start there).
#[derive(Clone, Debug)]
pub struct Cell {
    page: usize,
    rect: Rect,
    /// The standard frame inside the cell (cell pixels): the cell is bigger where held items
    /// or limbs stick out of it. Boxes in the UI show this part at their size.
    base: Rect,
    pub feet: Vec2,
    /// The head bone (the base of the head) in the cell, for portraits.
    pub head: Option<Vec2>,
    pub tips: Vec<(String, Vec2)>,
    /// The part of the cell the figure covers with what it wears and holds (cell pixels): a
    /// person stands in the middle of a roomy frame, a cow fills hers. Thumbnails show this part.
    pub drawn: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PoseKey {
    model: String,
    colors: u64,
    gear: u64,
    /// The pieces shown ([`parts_of`]).
    parts: u64,
    anim: String,
    frame: u32,
    facing: u8,
    turn: i8,
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
    /// Where a backpack sits ([`BackBone`]); None without a spine.
    back: Option<BackBone>,
    /// Where a sheathed weapon hangs ([`HipBone`]); None without hips.
    hip: Option<HipBone>,
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
    /// A piece's joints as nodes of the bodies it was worn on (by body id; [`Model::bind_to`]).
    binds: RefCell<HashMap<String, std::rc::Rc<Vec<Option<usize>>>>>,
    /// Longer from nose to tail than it is wide ([`long_body`]): a still picture shows its side.
    long: bool,
    /// A still model standing on the ground: where it stands (model units, x east / z south
    /// from its origin, at rest), for its contact shadow; None for figures and things in the air.
    ground: Option<Rect>,
}

/// The pieces the figure of `look` shows on body model `body`, as model ids in slot order: the
/// body's own ([`ModelDef::parts`]), replaced per slot by the look's choices
/// ([`Appearance::parts`]) and then by what it wears ([`Gear::worn`] items with a
/// [`crate::defs::ItemDef::wear`] style); pieces hidden by another ([`crate::defs::ModelPart::hides`]) are left out.
/// A piece is named by its model id or by its style (the body's own piece of that style).
pub fn parts_of(db: &Database, body: &str, look: &Appearance, gear: &Gear) -> Vec<String> {
    let Some(def) = db.model(body) else { return Vec::new() };
    if def.parts.is_empty() && look.parts.is_empty() && gear.worn.is_empty() {
        return Vec::new();
    }
    let fits = |m: &ModelDef| m.part.as_ref().is_some_and(|p| p.fits.is_empty() || p.fits.iter().any(|f| f == body));
    let styled = |style: &str| db.models.iter().find(|m| m.part.as_ref().is_some_and(|p| !p.style.is_empty() && p.style == style) && fits(m));
    let piece = |name: &str| -> Option<&ModelDef> {
        match db.model(name).filter(|m| m.part.is_some()) {
            Some(m) if fits(m) => Some(m),
            // Another body's piece: this body's piece of the same style.
            Some(m) => m.part.as_ref().filter(|p| !p.style.is_empty()).and_then(|p| styled(&p.style)),
            None => styled(name),
        }
    };
    let mut slots: std::collections::BTreeMap<&str, &ModelDef> = std::collections::BTreeMap::new();
    for name in &def.parts {
        if let Some(m) = piece(name) {
            slots.insert(m.part.as_ref().map_or("", |p| p.slot.as_str()), m);
        }
    }
    for (slot, name) in &look.parts {
        if name.is_empty() || name == crate::defs::NO_PART {
            slots.remove(slot.as_str());
        } else if let Some(m) = piece(name) {
            slots.insert(m.part.as_ref().map_or("", |p| p.slot.as_str()), m);
        }
    }
    for item in &gear.worn {
        if let Some(m) = db.item(item).filter(|i| !i.wear.is_empty()).and_then(|i| piece(&i.wear)) {
            slots.insert(m.part.as_ref().map_or("", |p| p.slot.as_str()), m);
        }
    }
    let hidden: Vec<&str> = slots.values().flat_map(|m| m.part.iter().flat_map(|p| p.hides.iter().map(String::as_str))).collect();
    slots.iter().filter(|(slot, _)| !hidden.contains(slot)).map(|(_, m)| m.id.clone()).collect()
}

/// The upper back of a model: the spine bone nearest the shoulders and, in its own space, the
/// point on the back's skin behind it, up along the spine and out of the back.
#[derive(Clone, Copy, Debug)]
struct BackBone {
    node: usize,
    at: Vec3,
    up: Vec3,
    out: Vec3,
}

/// [`BackBone`] of a model at rest (it faces +Z, so its back is the skin furthest toward -Z
/// around the bone's height and the body's middle).
fn back_bone(model: &Model, rest: &[Mat4]) -> Option<BackBone> {
    let node = ["spine02", "spine2", "upperchest", "chest", "spine01", "spine1", "spine"].iter().find_map(|n| model.node(n))?;
    let m = rest[node];
    let at = m.w_axis.truncate();
    let (lo, hi) = model.bounds();
    let h = (hi.y - lo.y).max(1e-3);
    let mut skin = Vec::new();
    model.skin(rest, &mut skin);
    let near = skin.iter().map(|p| p.0).filter(|p| (p.y - at.y).abs() < h * 0.08 && (p.x - at.x).abs() < h * 0.08);
    let z = near.map(|p| p.z).fold(f32::MAX, f32::min);
    let z = if z == f32::MAX { at.z - h * 0.08 } else { z };
    let inv = m.inverse();
    Some(BackBone { node, at: inv.transform_point3(vec3(at.x, at.y, z)), up: inv.transform_vector3(Vec3::Y).normalize_or_zero(), out: inv.transform_vector3(-Vec3::Z).normalize_or_zero() })
}

/// The left hip of a model: its hip bone and, in its own space, the point at the side of the
/// body (the belt) where a sheathed weapon's handle sits, down, forward and outward.
#[derive(Clone, Copy, Debug)]
struct HipBone {
    node: usize,
    at: Vec3,
    down: Vec3,
    fwd: Vec3,
    out: Vec3,
}

/// [`HipBone`] of a model at rest (facing +Z: its left is +X): the outside of the left thigh
/// around the hip bone's height (not the widest skin there: the hands hang beside the hips).
fn hip_bone(model: &Model, rest: &[Mat4]) -> Option<HipBone> {
    let node = ["hips", "pelvis", "hip"].iter().find_map(|n| model.node(n))?;
    let m = rest[node];
    let at = m.w_axis.truncate();
    let (lo, hi) = model.bounds();
    let h = (hi.y - lo.y).max(1e-3);
    // The left thigh's top (+X), else a guess beside the hip bone.
    let thigh = ["leftupleg", "leftthigh", "thigh_l", "upperleg_l", "leftleg"]
        .iter()
        .find_map(|n| model.node(n))
        .map(|n| rest[n].w_axis.truncate())
        .filter(|t| t.x > at.x)
        .map_or(at.x + h * 0.05, |t| t.x);
    let mut skin = Vec::new();
    model.skin(rest, &mut skin);
    let side = skin
        .iter()
        .map(|p| p.0)
        .filter(|p| (p.y - at.y).abs() < h * 0.05 && (p.z - at.z).abs() < h * 0.08 && (p.x - thigh).abs() < h * 0.07)
        .map(|p| p.x)
        .fold(f32::MIN, f32::max);
    let side = if side == f32::MIN { thigh + h * 0.04 } else { side };
    // On the belt at the side, a little in front and above the hip bone.
    let point = vec3(side, at.y + h * 0.03, at.z + h * 0.03);
    let inv = m.inverse();
    let dir = |v: Vec3| inv.transform_vector3(v).normalize_or_zero();
    Some(HipBone { node, at: inv.transform_point3(point), down: dir(-Vec3::Y), fwd: dir(Vec3::Z), out: dir(Vec3::X) })
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
    /// The frame a pose on it was last drawn (or added).
    stamp: u64,
    /// A texture of one big pose alone (a building), its own size: not shelf packed, not counted
    /// in the atlas's pages.
    solo: bool,
}

/// Poses wider or taller than this many pixels (buildings, big props) get a texture of their own
/// instead of a shelf on a page, at most [`SOLO_MAX`] of them (the oldest is reused).
const SOLO_FROM: u32 = PAGE / 2;
const SOLO_MAX: usize = 48;

#[derive(Default)]
struct Cache {
    pages: Vec<Page>,
    current: usize,
    poses: HashMap<PoseKey, Cell>,
    scratch: HashMap<(u32, u32), RenderTarget>,
    /// Targets the ground shadows of still models are drawn into (by size).
    shadow_scratch: HashMap<(u32, u32), RenderTarget>,
    material: Option<Material>,
    depth_material: Option<Material>,
    shadow_material: Option<Material>,
    frame: u64,
    renders: u32,
    renders_total: u64,
    /// Pages the atlas may have (0: the default size).
    max_pages: usize,
    /// Full pages cleared for new poses.
    evictions: u64,
}

impl Cache {
    /// A pose is drawn this frame: its page is in use.
    fn touch(&mut self, cell: &Cell) {
        if let Some(p) = self.pages.get_mut(cell.page) {
            p.stamp = self.frame;
        }
    }
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
    /// The spread of its points (for its automatic front).
    cov: Mat3,
}

/// The project's models and their rendered poses.
#[derive(Default)]
pub struct Models {
    loaded: HashMap<String, Loaded>,
    /// Held items' models by file.
    items: HashMap<String, ItemMesh>,
    cache: RefCell<Cache>,
    pub errors: Vec<String>,
    /// Models that loaded but won't draw whole (more vertices than one draw takes).
    pub warnings: Vec<String>,
}

/// The vertex half of the model shaders (WGSL): the normal travels to the fragment shader.
const VERTEX: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) n: vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = u.Projection * u.Model * vec4<f32>(v.position, 1.0);
    out.uv = v.texcoord;
    out.color = v.color0;
    out.n = v.normal.xyz;
    return out;
}
"#;

const FRAGMENT: &str = r#"
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(Texture, TextureSampler, in.uv) * in.color;
    if (c.a < 0.5) {
        discard;
    }
    let d = dot(normalize(in.n), u.LightDir);
    // Soft light like the pre-rendered props': a warm sun from the north-west over a cool sky.
    let sun = smoothstep(-0.3, 0.85, d);
    let light = mix(vec3<f32>(0.58, 0.62, 0.72), vec3<f32>(1.06, 1.02, 0.93), sun);
    return vec4<f32>(c.rgb * light, 1.0);
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
                    if let Some(w) = oversize(&def.id, &l.model) {
                        eprintln!("[models] {w}");
                        m.warnings.push(w);
                    }
                    m.loaded.insert(def.id.clone(), l);
                }
                Err(e) => {
                    eprintln!("[models] {}: {e:#}", def.id);
                    m.errors.push(format!("model {}: {e:#}", def.id));
                }
            }
        }
        m.add_buildings(db, &crate::building::picture_reader(db, read));
        for it in &db.items {
            let Some(im) = it.model.as_ref().filter(|im| !m.items.contains_key(&im.file)) else { continue };
            match read(&im.file).map_err(anyhow::Error::from).and_then(|b| Model::load(&b)) {
                Ok(model) => {
                    let mut points = Vec::new();
                    model.skin(&model.pose(None, 0.0), &mut points);
                    let (lo, hi) = points.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), (p, _)| (lo.min(*p), hi.max(*p)));
                    let len = (hi - lo).max_element().max(1e-4);
                    let uvs = model.vertices.iter().map(|v| v.uv).collect();
                    let cov = crate::model::spread(&points.iter().map(|p| p.0).collect::<Vec<_>>()).1;
                    m.items.insert(im.file.clone(), ItemMesh { points, uvs, indices: model.indices.clone(), image: model.texture.clone(), texture: RefCell::new(None), len, cov });
                }
                Err(e) => {
                    eprintln!("[models] item {}: {e:#}", it.id);
                    m.errors.push(format!("item model {} ({}): {e:#}", it.id, im.file));
                }
            }
        }
        m
    }

    /// Builds the models of the building props of `db` ([`crate::building`]), their pictures read
    /// through `read`.
    fn add_buildings(&mut self, db: &Database, picture: &dyn Fn(&crate::defs::SpriteRef) -> Option<crate::building::Picture>) {
        let others = neighbours(db, None);
        for p in db.props.iter().filter(|p| p.model.is_empty()) {
            let Some(shape) = &p.building else { continue };
            let built = crate::building::build_joined(shape, p.hitbox, picture, &others);
            let id = crate::building::model_id(&p.id);
            let def = building_def(&id, &p.name, built.height);
            if let Some(w) = oversize(&p.id, &built.model) {
                eprintln!("[models] {w}");
                self.warnings.push(w);
            }
            self.loaded.insert(id, loaded(&def, db, built.model, None));
        }
    }

    /// Builds the building props' models again (one changed in the editor), their pictures
    /// through `picture`: the old ones and their rendered poses go.
    pub fn rebuild_buildings(&mut self, db: &Database, picture: &dyn Fn(&crate::defs::SpriteRef) -> Option<crate::building::Picture>) {
        self.loaded.retain(|id, _| crate::building::prop_of(id).is_none());
        self.cache.borrow_mut().poses.retain(|k, _| crate::building::prop_of(&k.model).is_none());
        self.add_buildings(db, picture);
    }

    /// Builds building prop `p`'s model again (its shape is being edited: `db` may still hold the
    /// old one), and those of the buildings joined to it, their pictures through `picture`; a
    /// prop that is no building loses it.
    pub fn rebuild_building(&mut self, p: &crate::defs::PropDef, db: &Database, picture: &dyn Fn(&crate::defs::SpriteRef) -> Option<crate::building::Picture>) {
        let others = neighbours(db, Some(p));
        let partners: Vec<&crate::defs::PropDef> = db.props.iter().filter(|o| o.id != p.id && o.building.as_ref().is_some_and(|b| b.joins.iter().any(|j| j.prop == p.id))).collect();
        for q in std::iter::once(p).chain(partners) {
            let id = crate::building::model_id(&q.id);
            self.loaded.remove(&id);
            self.cache.borrow_mut().poses.retain(|k, _| k.model != id);
            if let (Some(shape), true) = (&q.building, q.model.is_empty()) {
                let built = crate::building::build_joined(shape, q.hitbox, picture, &others);
                let def = building_def(&id, &q.name, built.height);
                self.loaded.insert(id, loaded(&def, &Database::default(), built.model, None));
            }
        }
    }

    /// Sets the pose atlas's size in MB (`ProjectConfig::model_atlas_mb`); when it has more pages
    /// already, every pose renders again.
    pub fn set_atlas_mb(&self, mb: u32) {
        let mut c = self.cache.borrow_mut();
        c.max_pages = atlas_pages(mb);
        if c.pages.iter().filter(|p| !p.solo).count() > c.max_pages {
            // (Poses point at pages by index, big ones' own textures among them: start afresh.)
            c.pages.clear();
            c.poses.clear();
            c.current = 0;
        }
    }

    /// Forgets every rendered pose (an item model's marks changed).
    pub fn clear_poses(&self) {
        self.cache.borrow_mut().poses.clear();
    }

    /// An inventory icon of an item's model (`size` pixels square): the model alone, its handle
    /// pointing up-right like the drawn icons, cel shaded and outlined. Needs a window (GPU).
    pub fn render_item_icon(&self, spec: &crate::defs::ItemModel, size: u32) -> Option<Image> {
        let mesh = self.items.get(&spec.file)?;
        // Weapons and tools point up-right like the drawn icons, their face to the viewer;
        // lanterns hang, shields show their face.
        let diag = vec3(1.0, 1.0, 0.0).normalize();
        let (to_axis, to_front) = match spec.hold {
            HoldStyle::Grip | HoldStyle::Upright => (diag, vec3(-1.0, 1.0, 0.0).normalize()),
            HoldStyle::Hang => (-Vec3::Y, Vec3::Z),
            HoldStyle::Strap | HoldStyle::Back => (Vec3::Y, Vec3::Z),
        };
        let rot = hold_rotation(Vec3::from(spec.axis), item_front(spec, mesh), to_axis, to_front);
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
        draw_mesh(&Mesh { vertices, indices: mesh.indices.clone(), texture: Some(texture) });
        gl_use_default_material();
        let mut cam2 = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size as f32, size as f32));
        cam2.zoom.y = -cam2.zoom.y;
        cam2.render_target = Some(out.clone());
        set_camera(&cam2);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        let dark = Color::new(0.15, 0.10, 0.09, 0.5);
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

    /// What a look is drawn as: its model, else the mannequin.
    pub fn look_of(&self, a: &Appearance) -> Appearance {
        let model = if self.has(&a.model) { a.model.clone() } else { MANNEQUIN.to_string() };
        Appearance { model, ..a.clone() }
    }

    /// The facing a still picture of a look shows best (thumbnails, cards): the side of a model
    /// that is long from nose to tail (a wolf, a horse, a shark), else the front.
    pub fn portrait_facing(&self, a: &Appearance) -> Facing {
        if self.loaded.get(&a.model).is_some_and(|l| l.long) {
            Facing::Left
        } else {
            Facing::Down
        }
    }

    /// A model's frame size in world pixels (as a figure's frame).
    /// Where still model `id` stands on the ground, turned to `facing`: world pixels from its
    /// origin (x east, y south); None for figures and models in the air (a torch on a wall).
    pub fn ground_rect(&self, id: &str, facing: Facing) -> Option<Rect> {
        let l = self.loaded.get(id)?;
        let g = l.ground?;
        let k = l.def.height / l.height.max(1e-3);
        let yaw = facing_yaw(facing);
        let (s, c) = yaw.sin_cos();
        let corners = [vec2(g.x, g.y), vec2(g.x + g.w, g.y), vec2(g.x, g.y + g.h), vec2(g.x + g.w, g.y + g.h)].map(|p| vec2(p.x * c + p.y * s, -p.x * s + p.y * c) * k);
        let (lo, hi) = corners.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        Some(Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y))
    }

    pub fn frame_size(&self, id: &str) -> Option<(f32, f32)> {
        self.loaded.get(id).map(|l| (l.frame.0 as f32 / PX, l.frame.1 as f32 / PX))
    }

    /// Cache numbers for `status`: poses, pages (of the most allowed), renders this frame,
    /// renders in all, pages cleared for new poses.
    pub fn status(&self) -> String {
        let c = self.cache.borrow();
        format!("{}/{}/{}p/{}r/{}/{}e", c.poses.len(), c.pages.len(), page_cap(&c), c.renders, c.renders_total, c.evictions)
    }

    /// Call once per frame (resets the per-frame render budget).
    pub fn begin_frame(&self) {
        let mut c = self.cache.borrow_mut();
        c.frame += 1;
        c.renders = 0;
    }

    /// The rendered pose (rendering it now when needed and the frame's budget allows).
    pub fn pose(&self, db: &Database, req: &PoseReq) -> Option<Cell> {
        self.pose_with(db, req, true)
    }

    /// The head and shoulders of a look as an image ([`BUST`] pixels square, rendered now
    /// whatever the frame's budget; read back from the GPU, so call it once, e.g. when exporting).
    pub fn bust_image(&self, db: &Database, look: &Appearance) -> Option<Image> {
        let look = self.look_of(look);
        let req = PoseReq { look: &look, gear: &Gear::default(), anim: "idle", time: 0.0, once: true, facing: Facing::Down, turn: 0, bust: true };
        let cell = self.pose_with(db, &req, false)?;
        let c = self.cache.borrow();
        let page = c.pages.get(cell.page)?;
        let all = page.rt.texture.get_texture_data();
        let r = cell.rect;
        let mut out = Image::gen_image_color(r.w as u16, r.h as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        // (Atlas pages are drawn with a plain 2D camera: rows come back top first.)
        for y in 0..r.h as u32 {
            for x in 0..r.w as u32 {
                out.set_pixel(x, y, all.get_pixel(r.x as u32 + x, r.y as u32 + y));
            }
        }
        Some(out)
    }

    /// A contact sheet of `look` as the game draws it: a row per animation, `moments` moments of
    /// it per facing, on `ground` (image pixels, [`PX`] per world pixel; cells of `cell` pixels
    /// with the feet at the same place in each: the middle, three quarters down). Rendered now, whatever the frame's budget.
    #[allow(clippy::too_many_arguments)]
    pub fn sheet(&self, db: &Database, look: &Appearance, gear: &Gear, anims: &[String], facings: &[Facing], moments: usize, ground: Color) -> Option<(Image, (u32, u32))> {
        let look = self.look_of(look);
        let (fw, fh) = self.loaded.get(&look.model)?.frame;
        // Room for swings and for what the figure holds.
        // (Room below the feet too: a building's front corner reaches past them.)
        let (cw, ch) = (fw * 2, fh * 2);
        let (cols, rows) = ((facings.len() * moments).max(1) as u32, anims.len().max(1) as u32);
        if cols * cw > 8192 || rows * ch > 8192 {
            return None;
        }
        let mut out = Image::gen_image_color((cols * cw) as u16, (rows * ch) as u16, ground);
        for (r, anim) in anims.iter().enumerate() {
            for (fi, facing) in facings.iter().enumerate() {
                for m in 0..moments {
                    let req = PoseReq { look: &look, gear, anim, time: (m as f32 + 0.5) / moments as f32, once: true, facing: *facing, turn: 0, bust: false };
                    let Some(cell) = self.pose_with(db, &req, false) else { continue };
                    // (Read at once: a later pose may reuse the page.)
                    let c = self.cache.borrow();
                    let Some(page) = c.pages.get(cell.page) else { continue };
                    let all = page.rt.texture.get_texture_data();
                    let feet = ivec2(((fi * moments + m) as u32 * cw + cw / 2) as i32, (r as u32 * ch + ch * 3 / 4) as i32);
                    let top_left = feet - ivec2(cell.feet.x as i32, cell.feet.y as i32);
                    for y in 0..cell.rect.h as i32 {
                        for x in 0..cell.rect.w as i32 {
                            let (ox, oy) = (top_left.x + x, top_left.y + y);
                            let px = all.get_pixel(cell.rect.x as u32 + x as u32, cell.rect.y as u32 + y as u32);
                            if px.a > 0.5 && ox >= 0 && oy >= 0 && (ox as u32) < cols * cw && (oy as u32) < rows * ch {
                                out.set_pixel(ox as u32, oy as u32, px);
                            }
                        }
                    }
                }
            }
        }
        Some((out, (cw, ch)))
    }

    /// What to know before using model `id`: its size against what one draw takes, its colours and
    /// colour features, its bones and animations, and for a piece the bones the figures it fits
    /// lack. Lines starting with `!` are problems. None = the model isn't loaded.
    pub fn check(&self, db: &Database, id: &str) -> Option<Vec<String>> {
        let l = self.loaded.get(id)?;
        let m = &l.model;
        let mut out = Vec::new();
        let (v, t) = (m.vertices.len(), m.indices.len() / 3);
        out.push(format!("{}{v} vertices, {t} triangles (one draw takes {DRAW_VERTICES} vertices and {} triangles{})", if oversize(id, m).is_some() { "! " } else { "" }, DRAW_INDICES / 3, if oversize(id, m).is_some() { ": parts of it will be missing" } else { "" }));
        let colors: std::collections::HashSet<[u8; 3]> = l.texture.as_ref().map_or_else(Default::default, |t| t.bytes.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).map(|p| [p[0], p[1], p[2]]).collect());
        out.push(match colors.len() {
            0 => "! no texture and no coloured materials".to_string(),
            n if n > 64 => format!("! {n} colours in its texture: colour features match exact colours, reduce it to a small palette (32 or fewer)"),
            n => format!("{n} colours"),
        });
        for f in &l.def.features {
            for s in f.shades.iter().filter(|s| !colors.contains(*s)) {
                out.push(format!("! colour feature {}: its shade #{:02x}{:02x}{:02x} is no colour of the model", f.name, s[0], s[1], s[2]));
            }
        }
        // The size of one pixel of the drawn figure in the model's units, and the model's finest
        // detail against it (the shortest edge of a triangle).
        let figure = match &l.def.part {
            Some(p) => p.fits.iter().find_map(|b| self.loaded.get(b)),
            None => Some(l),
        };
        let edge = m.indices.as_chunks::<3>().0.iter().flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]).map(|(a, b)| (m.vertices[a as usize].pos - m.vertices[b as usize].pos).length()).filter(|d| *d > 1e-6).fold(f32::MAX, f32::min);
        if let Some(f) = figure.filter(|_| edge < f32::MAX) {
            let px = edge * f.ppu;
            out.push(format!("its finest detail is {px:.2} pixels of the drawn figure{}", if px < 0.75 { " (finer than a pixel: it won't show)" } else { "" }));
        }
        match &l.def.part {
            Some(p) => {
                out.push(format!("a piece: slot \"{}\"{}, {}", p.slot, if p.style.is_empty() { String::new() } else { format!(", style \"{}\"", p.style) }, if p.fits.is_empty() { "fits every figure".to_string() } else { format!("fits {}", p.fits.join(", ")) }));
                if p.slot.is_empty() {
                    out.push("! it has no slot: it never shows".to_string());
                }
                for b in &p.fits {
                    match self.loaded.get(b) {
                        None => out.push(format!("! it fits {b}, which isn't a loaded model")),
                        Some(body) => {
                            let bind = m.bind_to(&body.model);
                            let used: std::collections::HashSet<usize> = m.vertices.iter().flat_map(|v| (0..4).filter(|k| v.weights[*k] > 0.0).map(|k| v.joints[k] as usize)).collect();
                            let missing: Vec<&str> = bind.iter().enumerate().filter(|(k, b)| b.is_none() && used.contains(k)).filter_map(|(k, _)| m.joints.get(k).and_then(|j| m.nodes.get(*j)).map(|n| n.name.as_str())).collect();
                            if !missing.is_empty() {
                                out.push(format!("! on {b} the bones {} are missing: what hangs on them stays in place", missing.join(", ")));
                            }
                        }
                    }
                }
            }
            None => {
                out.push(format!("a figure {} px tall ({:.3} of its own units), {} bones, hands: {}", l.def.height, l.height, m.joints.len(), match (l.hands[0].is_some(), l.hands[1].is_some()) {
                    (true, true) => "both",
                    (true, false) => "right only",
                    (false, true) => "left only",
                    _ => "none (it can't hold items)",
                }));
                // (Work animations, sitting and weapon stances are extras; these four every figure plays.)
                for anim in ["walk", "idle", "attack", "hurt"] {
                    match l.def.clip_of(anim) {
                        None => out.push(format!("! no clip for the animation {anim}")),
                        Some(c) if m.clip(&c.clip).is_none() => out.push(format!("! the animation {anim} plays the clip {}, which its files don't have", c.clip)),
                        _ => {}
                    }
                }
                for p in &l.def.parts {
                    match db.model(p).and_then(|d| d.part.as_ref()) {
                        None => out.push(format!("! it wears {p}, which is no piece")),
                        Some(part) if !(part.fits.is_empty() || part.fits.iter().any(|f| f == id)) => out.push(format!("! it wears {p}, which doesn't list it under the figures it fits")),
                        _ => {}
                    }
                }
            }
        }
        Some(out)
    }

    /// [`Self::pose`], within the frame's render budget or (`budget` false) rendering now.
    fn pose_with(&self, db: &Database, req: &PoseReq, budget: bool) -> Option<Cell> {
        let l = self.loaded.get(&req.look.model)?;
        // Busts use the rest pose: the head straight, facing the front.
        let (clip_name, frame, time) = if req.bust { (None, 0, 0.0) } else { clip_frame(l, req) };
        let anim = if req.bust { "@bust" } else { req.anim };
        let part_ids = parts_of(db, &req.look.model, req.look, req.gear);
        let parts: Vec<&Loaded> = part_ids.iter().filter_map(|id| self.loaded.get(id)).collect();
        let key = PoseKey { model: req.look.model.clone(), colors: hash(&req.look.colors), gear: hash(req.gear), parts: hash(&part_ids), anim: anim.to_string(), frame, facing: req.facing as u8, turn: if req.bust { 0 } else { req.turn } };
        let mut c = self.cache.borrow_mut();
        if let Some(cell) = c.poses.get(&key).cloned() {
            c.touch(&cell);
            return Some(cell);
        }
        if budget && c.renders >= RENDERS_PER_FRAME {
            // Over budget: any frame of the same model, colours and facing, to draw meanwhile.
            let other = c.poses.iter().find(|(k, _)| k.model == key.model && k.colors == key.colors && k.facing == key.facing).map(|(_, v)| v.clone());
            if let Some(cell) = &other {
                c.touch(cell);
            }
            return other;
        }
        c.renders += 1;
        c.renders_total += 1;
        let cell = render(&mut c, l, &parts, &self.items, db, req, clip_name.as_deref(), time);
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

    /// [`Models::draw`], mirrored left to right when `flip` (decorations placed mirrored).
    pub fn draw_flipped(&self, cell: &Cell, feet: Vec2, size: f32, tint: Color, flip: bool) {
        if !flip {
            return self.draw(cell, feet, size, tint);
        }
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        let dest = vec2(cell.rect.w, cell.rect.h) / PX * size;
        let top_left = vec2(feet.x - (cell.rect.w - cell.feet.x) / PX * size, feet.y - cell.feet.y / PX * size);
        draw_texture_ex(&page.rt.texture, top_left.x, top_left.y, tint, DrawTextureParams { source: Some(cell.rect), dest_size: Some(dest), flip_x: true, ..Default::default() });
    }

    /// Draws a rendered pose into `dest` (screen pixels; UI portraits, windows).
    pub fn draw_in(&self, cell: &Cell, dest: Rect, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        let (kx, ky) = (dest.w / cell.base.w, dest.h / cell.base.h);
        let at = vec2(dest.x - cell.base.x * kx, dest.y - cell.base.y * ky);
        draw_texture_ex(&page.rt.texture, at.x, at.y, tint, DrawTextureParams { source: Some(cell.rect), dest_size: Some(vec2(cell.rect.w * kx, cell.rect.h * ky)), ..Default::default() });
    }

    /// Draws part `part` (fractions of the frame: x, y, w, h) of a rendered pose into `dest`.
    pub fn draw_part(&self, cell: &Cell, part: Rect, dest: Rect, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        let r = Rect::new(cell.rect.x + cell.base.x, cell.rect.y + cell.base.y, cell.base.w, cell.base.h);
        let src = Rect::new(r.x + part.x * r.w, r.y + part.y * r.h, part.w * r.w, part.h * r.h);
        draw_texture_ex(&page.rt.texture, dest.x, dest.y, tint, DrawTextureParams { source: Some(src), dest_size: Some(dest.size()), ..Default::default() });
    }

    /// Draws a rendered pose turned by `angle` (radians) about `center` (world pixels), as big
    /// as it is drawn standing (a character lying in a bed).
    pub fn draw_rotated(&self, cell: &Cell, center: Vec2, angle: f32, size: f32, tint: Color) {
        let c = self.cache.borrow();
        let Some(page) = c.pages.get(cell.page) else { return };
        // The standard frame's middle at `center`, turned about it.
        let dest = vec2(cell.rect.w, cell.rect.h) / PX * size;
        let mid = vec2(cell.base.x + cell.base.w / 2.0, cell.base.y + cell.base.h / 2.0) / PX * size;
        let params = DrawTextureParams { source: Some(cell.rect), dest_size: Some(dest), rotation: angle, pivot: Some(center), ..Default::default() };
        draw_texture_ex(&page.rt.texture, center.x - mid.x, center.y - mid.y, tint, params);
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
        if l.def.file != def.file || l.def.clip_files != def.clip_files || l.def.shared_clips != def.shared_clips {
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
            l.long = long_body(&l.model);
        }
        l.height = standing(def, &l.model);
        (l.frame, l.ppu, l.target_y) = layout(def, l.height);
        // A piece shows on other models' poses, and a body's own pieces are part of its poses.
        let pieces = def.part.is_some() || l.def.part != def.part || l.def.parts != def.parts;
        l.def = effective(def, l.fitted.as_deref());
        l.textures.borrow_mut().clear();
        self.cache.borrow_mut().poses.retain(|k, _| !pieces && k.model != def.id);
        true
    }

    /// The atlas texture of a rendered pose, the pose's place in it (texture coordinates 0..1)
    /// and the standard frame's place in the pose (fractions of it): the editor shows it in egui
    /// without copying.
    pub fn cell_texture(&self, cell: &Cell) -> Option<(Texture2D, Rect, Rect)> {
        let c = self.cache.borrow();
        let page = c.pages.get(cell.page)?;
        let r = cell.rect;
        let (pw, ph) = (page.rt.texture.width(), page.rt.texture.height());
        let b = cell.base;
        Some((page.rt.texture.clone(), Rect::new(r.x / pw, r.y / ph, r.w / pw, r.h / ph), Rect::new(b.x / r.w, b.y / r.h, b.w / r.w, b.h / r.h)))
    }

    /// Whether model `id` has a clip for animation `anim`.
    /// How long one loop of a model's animation lasts as it plays (seconds; its clip at the
    /// mapped fps). None without the model or a clip for it.
    pub fn cycle_seconds(&self, id: &str, anim: &str) -> Option<f32> {
        let l = self.loaded.get(id)?;
        let map = l.def.clip_of(anim)?;
        let clip = l.model.clip(&map.clip)?;
        let fps = map.fps.max(1.0);
        Some(((clip.duration * fps).round()).max(1.0) / fps)
    }

    /// The clip mapping of model `id`'s animation `anim`, if the model has that clip.
    pub fn clip_map(&self, id: &str, anim: &str) -> Option<&ClipMap> {
        self.loaded.get(id).and_then(|l| l.def.clip_of(anim).filter(|c| l.model.clip(&c.clip).is_some()))
    }

    /// The animation a character with `gear` plays for `base` ("attack", "idle", "walk", "block"):
    /// the weapon's (or bare hands' `attack_unarmed`), for idle and block the left-hand item's
    /// next, as far as model `id` has the clip; else the plain one (`block` without a clip:
    /// `idle`).
    pub fn anim_for(&self, db: &Database, id: &str, gear: &Gear, base: &str) -> String {
        // Out of combat: standing and walking relaxed, whatever is in hand.
        if !gear.ready && matches!(base, "idle" | "walk") {
            return base.into();
        }
        let has = |n: &str| self.clip_map(id, n).is_some();
        let mut wanted: Vec<String> = Vec::new();
        match base {
            "attack" => {
                wanted.extend(item_anim(db, gear.weapon.as_ref(), base));
                if gear.weapon.is_none() {
                    wanted.push("attack_unarmed".into());
                }
            }
            "idle" | "block" => {
                // A shield blocks before the weapon parries; standing, the weapon shows first.
                let (first, second) = if base == "block" { (&gear.offhand, &gear.weapon) } else { (&gear.weapon, &gear.offhand) };
                wanted.extend(item_anim(db, first.as_ref(), base));
                wanted.extend(item_anim(db, second.as_ref(), base));
            }
            "walk" => wanted.extend(item_anim(db, gear.weapon.as_ref(), base)),
            _ => {}
        }
        wanted.into_iter().find(|n| has(n)).unwrap_or_else(|| if base == "block" { "idle".into() } else { base.into() })
    }

    /// What model `look.model` plays for `anim` holding `gear`: (animation name, time, once). Like
    /// [`anim_req`], with the animation chosen by the gear ([`Models::anim_for`]) and attacks and
    /// blocks placed by the clip's hit point.
    pub fn clip_req(&self, db: &Database, look: &Appearance, gear: &Gear, anim: super::character::Anim, now: f32) -> (String, f32, bool) {
        use super::character::Anim;
        let (base, time, once) = anim_req(anim, now);
        let id = look.model.as_str();
        let name = match base {
            "attack" | "idle" | "walk" | "block" => self.anim_for(db, id, gear, base),
            other => return (other.to_string(), time, once),
        };
        let hit = self.clip_map(id, &name).map_or(DEFAULT_HIT, |c| c.hit_point());
        let p = |k: u16| k.min(1000) as f32 / 1000.0;
        let time = match anim {
            Anim::Windup(k) | Anim::Block(k) => hit * p(k),
            Anim::Swing(k) => hit + (1.0 - hit) * p(k),
            Anim::Strike(k) => strike_time(hit, k),
            // Blocking without a block clip: standing (idle loops on the clock).
            _ if base == "block" => now,
            _ => time,
        };
        let once = once && !(base == "block" && name == "idle");
        (name, time, once)
    }

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

/// What is wrong with a model too large for one draw (None = it fits): the rest of its
/// triangles would silently be left out.
pub fn oversize(id: &str, model: &Model) -> Option<String> {
    (model.vertices.len() > DRAW_VERTICES || model.indices.len() > DRAW_INDICES).then(|| {
        format!("model {id}: {} vertices and {} triangles, more than the {DRAW_VERTICES} vertices / {} triangles drawn in one piece: parts of it will be missing", model.vertices.len(), model.indices.len() / 3, DRAW_INDICES / 3)
    })
}

fn load_one(def: &ModelDef, db: &Database, read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>) -> anyhow::Result<Loaded> {
    use anyhow::Context;
    let load = |def: &ModelDef| -> anyhow::Result<Model> {
        let bytes = read(&def.file).with_context(|| format!("reading {}", def.file))?;
        let mut model = if def.height >= crate::model::BIG_MODEL { Model::load_with(&bytes, crate::model::BIG_TEXTURE_MAX, crate::model::BIG_TEXTURE_COLORS)? } else { Model::load(&bytes)? };
        for f in &def.clip_files {
            model.add_clips(&read(f).with_context(|| format!("reading {f}"))?)?;
        }
        for f in &def.shared_clips {
            model.add_clips_retargeted(&read(f).with_context(|| format!("reading {f}"))?)?;
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
    Ok(loaded(def, db, model, fitted))
}

/// A loaded model of `def` from its mesh (`fitted`: the rig template it was fitted to).
fn loaded(def: &ModelDef, _db: &Database, model: Model, fitted: Option<Box<Fitted>>) -> Loaded {
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
        // The local axis that points most to the front (+Z) at rest: the thumb side. Not one
        // along the forearm (the fingers' way): a rest pose with the arms reaching forward
        // would hold things along the fingers.
        let wrist = rest[node].w_axis.truncate();
        let fore = parent.map_or(Vec3::ZERO, |p| (wrist - rest[p].w_axis.truncate()).normalize_or_zero());
        let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let across = axes.into_iter().filter(|a| (rot * *a).normalize_or_zero().dot(fore).abs() < 0.7);
        let thumb = across.max_by(|a, b| (rot * *a).normalize_or_zero().z.total_cmp(&(rot * *b).normalize_or_zero().z))?;
        Some(HandBone { node, parent, thumb })
    };
    let hands = [hand("right"), hand("left")];
    let back = back_bone(&model, &rest);
    let hip = hip_bone(&model, &rest);
    let height = standing(def, &model);
    let (frame, ppu, target_y) = layout(def, height);
    let long = long_body(&model);
    let texture = model.texture.clone();
    let ground = (model.clips.is_empty() && model.joints.len() <= 1)
        .then(|| model.bounds())
        .filter(|(lo, _)| lo.y < height * 0.1)
        .map(|(lo, hi)| Rect::new(lo.x, lo.z, hi.x - lo.x, hi.z - lo.z));
    Loaded { def: effective(def, fitted.as_deref()), model, texture, align, hands, back, hip, frame, height, ppu, target_y, textures: RefCell::new(HashMap::new()), fitted, binds: RefCell::new(HashMap::new()), long, ground }
}

/// The standing figure's height in model units: [`ModelDef::stand`] when the file is only a
/// piece of it, else the mesh's own.
fn standing(def: &ModelDef, model: &Model) -> f32 {
    if def.stand > 0.0 {
        return def.stand;
    }
    let (lo, hi) = model.bounds();
    (hi.y - lo.y).max(1e-3)
}

/// Whether a model at rest is much longer from front to back than it is wide: an animal on four
/// legs, a fish. (People, with their arms, are wider than deep; a spider is as wide as long.)
fn long_body(model: &Model) -> bool {
    let (lo, hi) = model.bounds();
    let size = hi - lo;
    size.z > size.x * 1.5 && size.z > size.y * 0.6
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

/// The model definition of a building's model `id`: drawn `height` world pixels tall from the
/// ground (a roof alone floats: its own mesh is shorter than that).
fn building_def(id: &str, name: &str, height: f32) -> ModelDef {
    ModelDef { name: name.to_string(), height, stand: height / crate::project::TILE_SIZE as f32, ..ModelDef::new(id, "") }
}

/// Building props' shapes and footprints by id, `changed` instead of its copy in `db`.
fn neighbours<'a>(db: &'a Database, changed: Option<&'a crate::defs::PropDef>) -> impl Fn(&str) -> Option<(crate::defs::BuildingShape, [f32; 2])> + 'a {
    move |id: &str| {
        let p = changed.filter(|p| p.id == id).or_else(|| db.prop(id))?;
        Some((p.building.clone()?, p.hitbox))
    }
}

/// Draws into the depth buffer only: a still model's hidden geometry ([`Model::occluder`]).
fn depth_material(c: &mut Cache) -> Material {
    if c.depth_material.is_none() {
        let pipeline = PipelineParams { depth_write: true, depth_test: Comparison::LessOrEqual, color_write: (false, false, false, false), ..Default::default() };
        let m = load_material(ShaderSource::Wgsl { source: &format!("{VERTEX}{FRAGMENT_DEPTH}") }, MaterialParams { pipeline_params: pipeline, uniforms: vec![], textures: vec![] }).expect("depth shader");
        c.depth_material = Some(m);
    }
    c.depth_material.clone().unwrap()
}

/// How far hidden geometry is pushed away from the camera (model units: a tile).
const OCCLUDER_BIAS: f32 = 0.06;

/// Draws a still model's hidden geometry into the depth buffer (nothing when it has none).
fn draw_occluder(c: &mut Cache, model: &Model, turn: Mat4) {
    if model.occluder.is_empty() {
        return;
    }
    gl_use_material(&depth_material(c));
    // A little farther from the camera than it is: where two joined buildings meet, both draw a
    // hair past the line they meet on, so no crack shows between their pictures.
    let e = ELEVATION.to_radians();
    let away = -vec3(0.0, e.sin(), e.cos()) * OCCLUDER_BIAS;
    let v: Vec<Vertex> = model.occluder.iter().map(|p| Vertex { position: turn.transform_point3(*p) + away, uv: Vec2::ZERO, color: [255; 4], normal: Vec4::ZERO }).collect();
    for chunk in v.chunks(30_000) {
        draw_mesh(&Mesh { vertices: chunk.to_vec(), indices: (0..chunk.len() as u16).collect(), texture: None });
    }
    gl_use_default_material();
}

/// The ground shadow where hidden geometry doesn't cover it: its colour, depth tested.
fn shadow_material(c: &mut Cache) -> Material {
    if c.shadow_material.is_none() {
        let pipeline = PipelineParams { depth_write: false, depth_test: Comparison::LessOrEqual, ..Default::default() };
        let m = load_material(ShaderSource::Wgsl { source: &format!("{VERTEX}{FRAGMENT_SHADOW}") }, MaterialParams { pipeline_params: pipeline, uniforms: vec![], textures: vec![] }).expect("shadow shader");
        c.shadow_material = Some(m);
    }
    c.shadow_material.clone().unwrap()
}

const FRAGMENT_SHADOW: &str = r#"
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const FRAGMENT_DEPTH: &str = r#"
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}
"#;

fn material(c: &mut Cache) -> Material {
    if c.material.is_none() {
        let pipeline = PipelineParams {
            depth_write: true,
            depth_test: Comparison::LessOrEqual,
            color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
            ..Default::default()
        };
        let m = load_material(
            ShaderSource::Wgsl { source: &format!("{VERTEX}{FRAGMENT}") },
            MaterialParams { pipeline_params: pipeline, uniforms: vec![UniformDesc::new("LightDir", UniformType::Float3)], textures: vec![] },
        )
        .expect("model shader");
        c.material = Some(m);
    }
    c.material.clone().unwrap()
}

/// The pages an atlas may have.
fn page_cap(c: &Cache) -> usize {
    if c.max_pages == 0 {
        atlas_pages(DEFAULT_ATLAS_MB)
    } else {
        c.max_pages
    }
}

/// The page to clear when all are full: the one drawn from longest ago (the first of equals).
fn pick_page(stamps: &[u64]) -> usize {
    stamps.iter().enumerate().min_by_key(|(_, s)| **s).map_or(0, |(i, _)| i)
}

/// A page cell of `w x h` pixels (a new page, or the page drawn from longest ago cleared when
/// all are full).
fn alloc(c: &mut Cache, w: u32, h: u32) -> (usize, Rect) {
    if w > SOLO_FROM || h > SOLO_FROM {
        return alloc_solo(c, w, h);
    }
    loop {
        // (Never a big pose's own texture: those aren't shelves.)
        if let Some(p) = c.pages.get_mut(c.current).filter(|p| !p.solo) {
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
        if c.pages.iter().filter(|p| !p.solo).count() < page_cap(c) {
            let rt = render_target_ex(PAGE, PAGE, RenderTargetParams { sample_count: 1, depth: false });
            rt.texture.set_filter(FilterMode::Nearest);
            push_camera_state();
            set_camera(&Camera2D { render_target: Some(rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, PAGE as f32, PAGE as f32)) });
            clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
            pop_camera_state();
            c.pages.push(Page { rt, x: 0, y: 0, row: 0, stamp: c.frame, solo: false });
            c.current = c.pages.len() - 1;
        } else {
            // Reuse the page drawn from longest ago: its poses render again when needed.
            let shared: Vec<usize> = (0..c.pages.len()).filter(|&i| !c.pages[i].solo).collect();
            let oldest = shared[pick_page(&shared.iter().map(|&i| c.pages[i].stamp).collect::<Vec<_>>())];
            c.evictions += 1;
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

/// A texture of its own for a big pose of `w x h` pixels ([`SOLO_FROM`]): a new one, or the one
/// drawn from longest ago when there are [`SOLO_MAX`].
fn alloc_solo(c: &mut Cache, w: u32, h: u32) -> (usize, Rect) {
    let rt = render_target_ex(w, h, RenderTargetParams { sample_count: 1, depth: false });
    rt.texture.set_filter(FilterMode::Nearest);
    push_camera_state();
    set_camera(&Camera2D { render_target: Some(rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, w as f32, h as f32)) });
    clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
    pop_camera_state();
    let solos: Vec<usize> = (0..c.pages.len()).filter(|&i| c.pages[i].solo).collect();
    let page = Page { rt, x: w, y: 0, row: h, stamp: c.frame, solo: true };
    let at = if solos.len() < SOLO_MAX {
        c.pages.push(page);
        c.pages.len() - 1
    } else {
        let oldest = solos[pick_page(&solos.iter().map(|&i| c.pages[i].stamp).collect::<Vec<_>>())];
        c.evictions += 1;
        c.poses.retain(|_, cell| cell.page != oldest);
        c.pages[oldest] = page;
        oldest
    };
    (at, Rect::new(0.0, 0.0, w as f32, h as f32))
}

/// The colour features of `def` that `look` recolours, with their colours: the look's own
/// colour, else the feature's default.
/// A piece worn on `body` takes the body's default for a feature of the same name before its
/// own (an orc's green skin on the hands of its clothes).
fn feature_colors<'a>(def: &'a ModelDef, look: &Appearance, body: Option<&ModelDef>) -> Vec<(&'a [[u8; 3]], [u8; 3])> {
    let of_body = |name: &str| body.and_then(|b| b.features.iter().find(|f| f.name == name)).and_then(|f| f.default);
    def.features.iter().filter_map(|f| look.colors.get(&f.name).copied().or_else(|| of_body(&f.name)).or(f.default).map(|c| (f.shades.as_slice(), c))).collect()
}

/// The texture of `l` in the colours of `look` (its colour features recoloured).
/// `body`: the model a piece is worn on.
fn texture_for(l: &Loaded, look: &Appearance, body: Option<&ModelDef>) -> Option<Texture2D> {
    let key = hash(&(&look.colors, body.map(|b| &b.id)));
    if let Some(t) = l.textures.borrow().get(&key) {
        return Some(t.clone());
    }
    let mut img = l.texture.clone()?;
    let features = feature_colors(&l.def, look, body);
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
fn render(c: &mut Cache, l: &Loaded, parts: &[&Loaded], items: &HashMap<String, ItemMesh>, db: &Database, req: &PoseReq, clip: Option<&str>, time: f32) -> Cell {
    let (w, h) = if req.bust { (BUST, BUST) } else { l.frame };
    let clip_ref = clip.and_then(|n| l.model.clip(n));
    let world = l.model.pose(clip_ref, time);
    let mut skinned = Vec::new();
    l.model.skin(&world, &mut skinned);
    let extra = if req.bust { 0.0 } else { req.turn as f32 * std::f32::consts::FRAC_PI_4 + ISO_TURN };
    let yaw = facing_yaw(req.facing) + extra + clip.and_then(|n| l.align.get(n)).copied().unwrap_or(0.0);
    let turn = Mat4::from_rotation_y(yaw);
    let vertices: Vec<Vertex> = skinned
        .iter()
        .zip(&l.model.vertices)
        .map(|((p, n), v)| {
            let n = turn.transform_vector3(*n);
            Vertex { position: turn.transform_point3(*p), uv: v.uv, color: [255; 4], normal: vec4(n.x, n.y, n.z, 0.0) }
        })
        .collect();
    // A still model (a building, a cart: no clips, no bones) standing on the ground casts a shadow
    // on it, down and to the right on the screen like the pre-rendered props': its points slid
    // along the sun. One hanging in the air (a torch on a wall) casts none.
    let grounded = || vertices.iter().map(|v| v.position.y).fold(f32::MAX, f32::min) < l.height * 0.1;
    let shadow: Vec<Vertex> = if !req.bust && l.model.clips.is_empty() && l.model.joints.len() <= 1 && grounded() {
        vertices.iter().map(|v| Vertex { position: vec3(v.position.x + v.position.y * SHADOW_RUN.x, 0.0, v.position.z + v.position.y * SHADOW_RUN.y), uv: v.uv, color: [0, 0, 0, 255], normal: v.normal }).collect()
    } else {
        Vec::new()
    };
    // The pieces it wears, in the body's pose.
    let pieces: Vec<Mesh> = parts
        .iter()
        .map(|p| {
            let bind = p.binds.borrow_mut().entry(l.def.id.clone()).or_insert_with(|| std::rc::Rc::new(p.model.bind_to(&l.model))).clone();
            p.model.skin_on(&world, &bind, &mut skinned);
            let vertices = skinned
                .iter()
                .zip(&p.model.vertices)
                .map(|((q, n), v)| {
                    let n = turn.transform_vector3(*n);
                    Vertex { position: turn.transform_point3(*q), uv: v.uv, color: [255; 4], normal: vec4(n.x, n.y, n.z, 0.0) }
                })
                .collect();
            Mesh { vertices, indices: p.model.indices.clone(), texture: texture_for(p, req.look, Some(&l.def)) }
        })
        .collect();
    let e = if req.bust { BUST_ELEVATION } else { ELEVATION }.to_radians();
    // A bust looks at the head (its bone is at the head's base); a pose at the whole figure.
    let head_bone = l.model.node("head").map(|n| turn.transform_point3(world[n].w_axis.truncate()));
    let target = match (req.bust, head_bone) {
        (true, Some(hb)) => hb + vec3(0.0, l.height * 0.02, 0.0),
        (true, None) => vec3(0.0, l.height * 0.85, 0.0),
        _ => vec3(0.0, l.target_y, 0.0),
    };
    let back = vec3(0.0, e.sin(), e.cos());
    let screen_right = Vec3::X;
    let screen_down = -vec3(0.0, e.cos(), -e.sin());
    // The camera for a frame of `fw` x `fh` pixels whose middle is `shift` pixels off the
    // standard frame's: model point -> frame pixel (y down).
    let camera = |fw: u32, fh: u32, shift: Vec2| {
        let target = target + screen_right * (shift.x / l.ppu) + screen_down * (shift.y / l.ppu);
        let span = if req.bust { l.height * 0.42 } else { fh as f32 / l.ppu };
        Camera3D {
            position: target + back * 50.0,
            target,
            up: Vec3::Y,
            fovy: span,
            aspect: Some(fw as f32 / fh as f32),
            projection: Projection::Orthographics,
            render_target: None,
            viewport: None,
            z_near: 0.1,
            z_far: 100.0,
        }
    };
    let projector = |cam: &Camera3D, fw: u32, fh: u32| {
        let view = cam.matrix();
        move |p: Vec3| {
            let q = view.project_point3(p);
            vec2((q.x + 1.0) / 2.0 * fw as f32, (1.0 - q.y) / 2.0 * fh as f32)
        }
    };
    let to_px0 = projector(&camera(w, h, Vec2::ZERO), w, h);
    // The hands: grip point (a little past the wrist), grip axis and the forearm's direction
    // (where the knuckles point), in model space after the turn.
    let hand = |hb: &Option<HandBone>| -> Option<(Vec3, Vec3, Vec3)> {
        let hb = (*hb)?;
        let m = world[hb.node];
        let wrist = m.w_axis.truncate();
        let elbow = hb.parent.map_or(wrist - Vec3::Y, |p| world[p].w_axis.truncate());
        let along = (wrist - elbow).normalize_or_zero();
        let grip = wrist + along * (0.07 * l.height / 1.7);
        let axis = Mat3::from_mat4(m).mul_vec3(hb.thumb).normalize_or_zero();
        Some((turn.transform_point3(grip), turn.transform_vector3(axis), turn.transform_vector3(along)))
    };
    let hands3 = [hand(&l.hands[0]), hand(&l.hands[1])];
    let eq = req.gear.equipment();
    // (A carried prop has no item model: it is held, not shown.)
    let layers: Vec<InHand> = if req.bust { Vec::new() } else { crate::hands::in_hands(db, &eq, &req.gear.carrying, None).into_iter().filter(|h| !h.carried).collect() };
    // Held items, placed (not drawn yet): their models shaded on the hand; with their far tips.
    let mut held: Vec<(Mesh, String, Vec3)> = Vec::new();
    // Out of combat a one-handed weapon hangs at the left hip, not in the hand.
    let hip = l.hip.zip(sheathed(db, req.gear, req.anim)).and_then(|(hb, id)| db.item(id)?.model.as_ref().and_then(|m| items.get(&m.file).map(|mesh| (hb, id, m, mesh))));
    for layer in &layers {
        if hip.is_some_and(|h| h.1 == layer.id) {
            continue;
        }
        let anchor = match layer.hand {
            Hand::Right => hands3[0].map(|h| h.0),
            Hand::Left => hands3[1].map(|h| h.0),
            // Between the fists when both are on it; hanging apart at the sides: the right one.
            Hand::Both => match (hands3[0], hands3[1]) {
                (Some(a), Some(b)) if (a.0 - b.0).length() < l.height * 0.3 => Some((a.0 + b.0) / 2.0),
                (a, b) => a.or(b).map(|h| h.0),
            },
        };
        let Some(anchor) = anchor else { continue };
        // The item's own 3D model: its mesh on the hand, the handle along the grip axis (an item
        // without a model isn't shown).
        let im = db.item(&layer.id).and_then(|d| d.model.as_ref()).and_then(|m| items.get(&m.file).map(|mesh| (m, mesh)));
        if let Some((spec, mesh)) = im {
            let one = match layer.hand {
                Hand::Right => hands3[0],
                Hand::Left => hands3[1],
                Hand::Both => hands3[0].or(hands3[1]),
            };
            let Some((_, thumb, along)) = one else { continue };
            let forward = turn.transform_vector3(Vec3::Z);
            let (to_axis, to_front) = match spec.hold {
                HoldStyle::Grip => match (layer.hand, hands3[0], hands3[1]) {
                    // Two-handed, both fists on it: along the line through them, the right
                    // hand's way (fists far apart, hanging at the sides: in the right hand).
                    (Hand::Both, Some(r), Some(lh)) if (l.height * 0.08..l.height * 0.3).contains(&(r.0 - lh.0).length()) => {
                        let d = (r.0 - lh.0).normalize_or_zero();
                        (out_of_fist(if d.dot(r.1) >= 0.0 { d } else { -d }, r.2), r.2)
                    }
                    _ => (out_of_fist(thumb, along), along),
                },
                // Up, leaning a little with the fist.
                HoldStyle::Upright => ((Vec3::Y * 2.0 + thumb).normalize_or(Vec3::Y), forward),
                HoldStyle::Hang => (-Vec3::Y, forward),
                // A backpack in the hand: carried by its top, upright.
                HoldStyle::Back => (Vec3::Y, forward),
                HoldStyle::Strap => {
                    // Turned to the front and out to the side of its hand.
                    let out = vec3(anchor.x, 0.0, anchor.z).normalize_or_zero();
                    (Vec3::Y, (forward + out * 0.7).normalize_or(forward))
                }
            };
            let (m, far) = placed_item(spec, mesh, anchor, to_axis, to_front, spec.size * PX / l.ppu);
            held.push((m, layer.id.clone(), far));
        }
    }
    if let Some((hb, id, spec, mesh)) = hip {
        // The handle at the belt, the blade down and a little back, its edge forward.
        let m = world[hb.node];
        let anchor = turn.transform_point3(m.transform_point3(hb.at));
        let v = |d: Vec3| turn.transform_vector3(m.transform_vector3(d)).normalize_or_zero();
        let (down, fwd, out) = (v(hb.down), v(hb.fwd), v(hb.out));
        // (Along the thigh: no outward tilt, the leg is narrower below.)
        let axis = (down - fwd * 0.45 - out * 0.05).normalize_or(down);
        let (mesh, far) = placed_item(spec, mesh, anchor, axis, fwd, spec.size * PX / l.ppu);
        held.push((mesh, id.to_string(), far));
    }
    // A backpack on the upper back: against the back's skin, its top along the spine, its front
    // facing away from the body.
    let worn = req.gear.back.as_ref().filter(|_| !req.bust).and_then(|id| db.item(id)).and_then(|d| d.model.as_ref()).and_then(|m| items.get(&m.file).map(|mesh| (m, mesh)));
    if let (Some((spec, mesh)), Some(bb)) = (worn, l.back) {
        let m = world[bb.node];
        let anchor = turn.transform_point3(m.transform_point3(bb.at));
        let up = turn.transform_vector3(m.transform_vector3(bb.up)).normalize_or(Vec3::Y);
        let out = turn.transform_vector3(m.transform_vector3(bb.out)).normalize_or(-Vec3::Z);
        let (mesh, far) = placed_item(spec, mesh, anchor, up, out, spec.size * PX / l.ppu);
        held.push((mesh, req.gear.back.clone().unwrap_or_default(), far));
    }
    // The frame: the standard one, grown by whatever sticks out of it (a spear, a swung arm),
    // in steps of 8 pixels and at most one and a half figures each way.
    // What the figure itself covers (without its shadow), in the standard frame's pixels.
    let (mut flo, mut fhi) = (vec2(f32::MAX, f32::MAX), vec2(f32::MIN, f32::MIN));
    for p in vertices.iter().map(|v| v.position).chain(pieces.iter().flat_map(|m| m.vertices.iter().map(|v| v.position))).chain(held.iter().flat_map(|(m, ..)| m.vertices.iter().map(|v| v.position))) {
        let s = to_px0(p);
        (flo, fhi) = (flo.min(s), fhi.max(s));
    }
    let (left, right, top, bottom) = if req.bust {
        (0, 0, 0, 0)
    } else {
        let (mut lo, mut hi) = (vec2(0.0, 0.0), vec2(w as f32, h as f32));
        let points = vertices.iter().chain(&shadow).map(|v| v.position).chain(pieces.iter().flat_map(|m| m.vertices.iter().map(|v| v.position))).chain(held.iter().flat_map(|(m, ..)| m.vertices.iter().map(|v| v.position)));
        for p in points {
            let s = to_px0(p);
            (lo, hi) = (lo.min(s), hi.max(s));
        }
        // (Wide objects, a cart or a market stall, reach further than a figure's arms.)
        let (lo3, hi3) = l.model.bounds();
        let across = (hi3.x - lo3.x).max(hi3.z - lo3.z) * l.ppu;
        let cap = (l.def.height.max(4.0) * PX * 1.5).max(across * 1.2);
        let grow = |d: f32| if d <= 0.0 { 0 } else { (((d + 2.0).min(cap) / 8.0).ceil() * 8.0) as u32 };
        (grow(-lo.x), grow(hi.x - w as f32), grow(-lo.y), grow(hi.y - h as f32))
    };
    let (w0, h0) = (w, h);
    let (w, h) = (w0 + left + right, h0 + top + bottom);
    let shift = vec2((right as f32 - left as f32) / 2.0, (bottom as f32 - top as f32) / 2.0);
    let scratch = c
        .scratch
        .entry((w, h))
        .or_insert_with(|| {
            let rt = render_target_ex(w, h, RenderTargetParams { sample_count: 1, depth: true });
            rt.texture.set_filter(FilterMode::Nearest);
            rt
        })
        .clone();
    let cam = Camera3D { render_target: Some(scratch.clone()), ..camera(w, h, shift) };
    let to_px = projector(&cam, w, h);
    let mat = material(c);
    push_camera_state();
    set_camera(&cam);
    clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
    draw_occluder(c, &l.model, turn);
    gl_use_material(&mat);
    let light = vec3(-0.45, 0.75, 0.55).normalize();
    mat.set_uniform("LightDir", light);
    draw_mesh(&Mesh { vertices, indices: l.model.indices.clone(), texture: texture_for(l, req.look, None) });
    for m in &pieces {
        draw_mesh(m);
    }
    let mut tips = Vec::new();
    for (mesh, id, tip) in &held {
        draw_mesh(mesh);
        tips.push((id.clone(), to_px(*tip)));
    }
    gl_use_default_material();
    // The shadow, flat black in a target of its own (overlapping triangles stay one shade); a
    // joined building's neighbour hides what falls under it.
    let shadow_rt = (!shadow.is_empty()).then(|| {
        let rt = c
            .shadow_scratch
            .entry((w, h))
            .or_insert_with(|| {
                let rt = render_target_ex(w, h, RenderTargetParams { sample_count: 1, depth: true });
                rt.texture.set_filter(FilterMode::Nearest);
                rt
            })
            .clone();
        set_camera(&Camera3D { render_target: Some(rt.clone()), ..camera(w, h, shift) });
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        let cast = l.model.indices[..l.model.indices.len() - l.model.shadowless.min(l.model.indices.len())].to_vec();
        if l.model.occluder.is_empty() {
            draw_mesh(&Mesh { vertices: shadow, indices: cast, texture: None });
        } else {
            draw_occluder(c, &l.model, turn);
            gl_use_material(&shadow_material(c));
            draw_mesh(&Mesh { vertices: shadow, indices: cast, texture: None });
            gl_use_default_material();
        }
        rt
    });
    // Into the atlas, with a soft dark outline: the picture four times shifted by a pixel in a
    // dark, half-clear tint, then itself (it keeps figures apart from the painted ground).
    let (page, rect) = alloc(c, w + 2, h + 2);
    set_camera(&page_camera(&c.pages[page].rt));
    if let Some(rt) = &shadow_rt {
        draw_texture_ex(&rt.texture, rect.x + 1.0, rect.y + 1.0, Color::new(0.0, 0.0, 0.0, SHADOW_ALPHA), DrawTextureParams { dest_size: Some(vec2(w as f32, h as f32)), flip_y: true, ..Default::default() });
    }
    // (A tint multiplies: this darkens every colour to the outline's dark brown or below.)
    let dark = Color::new(0.15, 0.10, 0.09, 1.0);
    let (x, y) = (rect.x + 1.0, rect.y + 1.0);
    // (Not for a joined building: its outline would draw a seam across the walls it shares.)
    let outline: &[(f32, f32)] = if l.model.occluder.is_empty() { &[(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] } else { &[] };
    for (dx, dy) in outline {
        draw_texture_ex(&scratch.texture, x + dx, y + dy, dark, DrawTextureParams { dest_size: Some(vec2(w as f32, h as f32)), flip_y: true, ..Default::default() });
    }
    draw_texture_ex(&scratch.texture, x, y, WHITE, DrawTextureParams { dest_size: Some(vec2(w as f32, h as f32)), flip_y: true, ..Default::default() });
    pop_camera_state();
    let feet = to_px(Vec3::ZERO) + vec2(1.0, 1.0);
    let head = l.model.node("head").map(|n| to_px(turn.transform_point3(world[n].w_axis.truncate())) + vec2(1.0, 1.0));
    let base = Rect::new(left as f32, top as f32, (w0 + 2) as f32, (h0 + 2) as f32);
    // (With the outline's pixel around it, inside the cell.)
    let whole = vec2((w + 2) as f32, (h + 2) as f32);
    let (dlo, dhi) = ((flo + vec2(left as f32, top as f32)).floor().max(Vec2::ZERO), (fhi + vec2(left as f32 + 2.0, top as f32 + 2.0)).ceil().min(whole));
    let drawn = if dhi.x > dlo.x && dhi.y > dlo.y { Rect::new(dlo.x, dlo.y, dhi.x - dlo.x, dhi.y - dlo.y) } else { Rect::new(0.0, 0.0, whole.x, whole.y) };
    Cell { page, rect, base, feet, head, tips: tips.into_iter().map(|(id, p)| (id, p + vec2(1.0, 1.0))).collect(), drawn }
}

/// How far a still model's shadow reaches per unit of height (across the screen, toward the
/// viewer): the sun stands high in the north-west.
const SHADOW_RUN: Vec2 = Vec2::new(0.55, 0.35);
/// The same shadow for an upright picture, per pixel of its height on the screen: a point that
/// high (`h / cos 30°` above the ground) lands `SHADOW_RUN * that` along the ground, seen 30°
/// from above (toward the viewer: down by its sine).
pub const SHADOW_SHEAR: Vec2 = Vec2::new(0.55 / 0.866_025_4, 0.35 * 0.5 / 0.866_025_4);
/// How dark the ground shadow of still models is.
const SHADOW_ALPHA: f32 = 0.72;

/// A camera drawing into an atlas page in its texture's own pixels (y down as sampled).
fn page_camera(rt: &RenderTarget) -> Camera2D {
    let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, rt.texture.width(), rt.texture.height()));
    cam.zoom.y = -cam.zoom.y;
    cam.render_target = Some(rt.clone());
    cam
}

/// An item's model as a mesh in the pose: its hold point at `anchor`, its handle along `to_axis`
/// and its front toward `to_front`, `length` model units long; with its far end.
fn placed_item(spec: &crate::defs::ItemModel, mesh: &ItemMesh, anchor: Vec3, to_axis: Vec3, to_front: Vec3, length: f32) -> (Mesh, Vec3) {
    let k = length / mesh.len;
    let rot = hold_rotation(Vec3::from(spec.axis), item_front(spec, mesh), to_axis, to_front);
    let grip = Vec3::from(spec.grip);
    let place = |p: Vec3| anchor + rot * ((p - grip) * k);
    let vertices: Vec<Vertex> = mesh
        .points
        .iter()
        .zip(&mesh.uvs)
        .map(|((p, n), uv)| {
            let n = rot * *n;
            Vertex { position: place(*p), uv: *uv, color: [255; 4], normal: vec4(n.x, n.y, n.z, 0.0) }
        })
        .collect();
    let texture = mesh
        .texture
        .borrow_mut()
        .get_or_insert_with(|| {
            let t = mesh.image.as_ref().map_or_else(|| Texture2D::from_rgba8(1, 1, &[200, 200, 200, 255]), Texture2D::from_image);
            t.set_filter(FilterMode::Nearest);
            t
        })
        .clone();
    let far = mesh.points.iter().map(|p| p.0).max_by(|a, b| (*a - grip).length_squared().total_cmp(&(*b - grip).length_squared())).unwrap_or(grip);
    (Mesh { vertices, indices: mesh.indices.clone(), texture: Some(texture) }, place(far))
}

/// An item model's front: its own, else the wider side across its handle.
fn item_front(spec: &crate::defs::ItemModel, mesh: &ItemMesh) -> Vec3 {
    if spec.front != [0.0; 3] {
        Vec3::from(spec.front)
    } else {
        Vec3::from(crate::model::auto_front(spec.axis, mesh.cov))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::ClipMap;

    fn loaded() -> Loaded {
        let glb = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/model_two_bones.glb")).unwrap();
        let def = ModelDef { clips: vec![ClipMap { anim: "walk".into(), clip: "bend".into(), fps: 10.0, hit: None }], ..ModelDef::new("m", "models/m.glb") };
        load_one(&def, &Database::default(), &|_| Ok(glb.clone())).unwrap()
    }

    #[test]
    fn a_long_body_is_pictured_from_the_side() {
        // The fixture is a thin upright bar: pictured from the front, like a person.
        let mut l = loaded();
        assert!(!l.long);
        // Laid along z it is a body long from nose to tail.
        for v in l.model.vertices.iter_mut() {
            v.pos = vec3(v.pos.x, v.pos.z, v.pos.y);
        }
        assert!(long_body(&l.model));
    }

    #[test]
    fn a_looks_size_is_a_percentage_and_optional_in_files() {
        let old: Appearance = ron::from_str(r#"(model: "hero", body: "")"#).unwrap();
        assert_eq!((old.size, old.scale()), (0, 1.0));
        assert!(!ron::to_string(&old).unwrap().contains("size"));
        let big = Appearance { size: 115, ..old.clone() };
        assert!((big.scale() - 1.15).abs() < 1e-6);
        assert_eq!(ron::from_str::<Appearance>(&ron::to_string(&big).unwrap()).unwrap().size, 115);
        // Out of range sizes are kept sane.
        assert_eq!(Appearance { size: 5, ..old }.scale(), 0.25);
    }

    #[test]
    fn a_feature_without_a_chosen_colour_takes_its_default() {
        use crate::defs::ColorFeature;
        let skin = ColorFeature { name: "skin".into(), shades: vec![[195, 152, 90]], colors: vec![], default: Some([194, 152, 120]) };
        let hair = ColorFeature { name: "hair".into(), shades: vec![[249, 187, 47]], colors: vec![], default: None };
        let def = ModelDef { features: vec![skin, hair], ..ModelDef::new("m", "models/m.glb") };
        // No colours chosen: the skin takes its default, the hair stays as drawn.
        let none = Appearance::default();
        assert_eq!(feature_colors(&def, &none, None), vec![(&[[195u8, 152, 90]][..], [194, 152, 120])]);
        // A chosen colour wins over the default.
        let mut look = Appearance::default();
        look.colors.insert("skin".into(), [90, 58, 36]);
        look.colors.insert("hair".into(), [30, 22, 18]);
        assert_eq!(feature_colors(&def, &look, None), vec![(&[[195u8, 152, 90]][..], [90, 58, 36]), (&[[249u8, 187, 47]][..], [30, 22, 18])]);
        // Files without the field load (None), and None isn't written.
        let f: ColorFeature = ron::from_str(r#"(name: "skin", shades: [(1, 2, 3)])"#).unwrap();
        assert_eq!(f.default, None);
        assert!(!ron::to_string(&f).unwrap().contains("default"));
    }

    fn piece(id: &str, slot: &str, style: &str, fits: &[&str], hides: &[&str]) -> ModelDef {
        let part = crate::defs::ModelPart { slot: slot.into(), style: style.into(), fits: fits.iter().map(|s| s.to_string()).collect(), hides: hides.iter().map(|s| s.to_string()).collect(), offer: false };
        ModelDef { part: Some(part), ..ModelDef::new(id, &format!("models/{id}.glb")) }
    }

    #[test]
    fn a_figure_wears_its_bodys_pieces_then_its_choices_then_its_equipment() {
        let mut db = Database::default();
        db.models.push(ModelDef { parts: vec!["clothes_m".into(), "hair_short".into()], ..ModelDef::new("hero", "models/hero.glb") });
        db.models.push(ModelDef { parts: vec!["clothes_d".into()], ..ModelDef::new("dwarf", "models/dwarf.glb") });
        db.models.push(piece("clothes_m", "outfit", "clothes", &["hero"], &[]));
        db.models.push(piece("clothes_d", "outfit", "clothes", &["dwarf"], &[]));
        db.models.push(piece("plate_m", "outfit", "plate_armor", &["hero"], &[]));
        db.models.push(piece("plate_d", "outfit", "plate_armor", &["dwarf"], &[]));
        db.models.push(piece("hair_short", "hair", "hair_short", &["hero", "dwarf"], &[]));
        db.models.push(piece("hair_long", "hair", "hair_long", &["hero", "dwarf"], &[]));
        db.models.push(piece("helmet", "headwear", "helmet", &["hero", "dwarf"], &["hair"]));
        db.items.push(crate::defs::ItemDef { wear: "plate_armor".into(), ..crate::defs::ItemDef::fresh("plate") });
        db.items.push(crate::defs::ItemDef { wear: "helmet".into(), ..crate::defs::ItemDef::fresh("helm") });
        db.reindex();
        let (plain, none) = (Appearance { model: "hero".into(), ..Default::default() }, Gear::default());
        assert_eq!(parts_of(&db, "hero", &plain, &none), ["hair_short", "clothes_m"]);
        // A look's choice by style or by id replaces the body's piece; "-" takes it off.
        let mut look = plain.clone();
        look.parts.insert("hair".into(), "hair_long".into());
        assert_eq!(parts_of(&db, "hero", &look, &none), ["hair_long", "clothes_m"]);
        look.parts.insert("hair".into(), crate::defs::NO_PART.into());
        assert_eq!(parts_of(&db, "hero", &look, &none), ["clothes_m"]);
        // Worn armour shows as the body's own piece of the item's style; a helmet hides the hair.
        let armoured = Gear { worn: vec!["plate".into()], ..Default::default() };
        assert_eq!(parts_of(&db, "hero", &plain, &armoured), ["hair_short", "plate_m"]);
        assert_eq!(parts_of(&db, "dwarf", &Appearance { model: "dwarf".into(), ..Default::default() }, &armoured), ["plate_d"]);
        let helmed = Gear { worn: vec!["plate".into(), "helm".into()], ..Default::default() };
        assert_eq!(parts_of(&db, "hero", &plain, &helmed), ["helmet", "plate_m"]);
        // Another body's piece named by id: this body's piece of that style.
        let mut borrowed = Appearance { model: "dwarf".into(), ..Default::default() };
        borrowed.parts.insert("outfit".into(), "plate_m".into());
        assert_eq!(parts_of(&db, "dwarf", &borrowed, &none), ["plate_d"]);
        // A model without pieces and nothing worn: nothing to look up.
        assert!(parts_of(&db, "helmet", &plain, &none).is_empty());
    }

    #[test]
    fn a_piece_takes_the_bodys_default_colour_before_its_own() {
        let skin = |default| crate::defs::ColorFeature { name: "skin".into(), shades: vec![[224, 172, 132]], colors: vec![], default };
        let orc = ModelDef { features: vec![skin(Some([125, 154, 98]))], ..ModelDef::new("orc", "models/orc.glb") };
        let clothes = ModelDef { features: vec![skin(None)], ..ModelDef::new("clothes", "models/clothes.glb") };
        let mut look = Appearance::default();
        assert!(feature_colors(&clothes, &look, None).is_empty());
        assert_eq!(feature_colors(&clothes, &look, Some(&orc)), vec![(&[[224u8, 172, 132]][..], [125, 154, 98])]);
        look.colors.insert("skin".into(), [90, 58, 36]);
        assert_eq!(feature_colors(&clothes, &look, Some(&orc)), vec![(&[[224u8, 172, 132]][..], [90, 58, 36])]);
    }

    #[test]
    fn diagonal_walkers_turn_by_an_eighth() {
        // Facing right (the side wins at 45°) or down, walking down-right: halfway between.
        assert_eq!(diagonal_turn(Facing::Right, vec2(1.0, 1.0)), -1);
        assert_eq!(diagonal_turn(Facing::Down, vec2(1.0, 1.0)), 1);
        assert_eq!(compass(Facing::Right, -1), "se");
        assert_eq!(compass(Facing::Down, 1), "se");
        // Up-left, from both sides; straight lines and standing still don't turn.
        assert_eq!(compass(Facing::Up, diagonal_turn(Facing::Up, vec2(-1.0, -1.0))), "nw");
        assert_eq!(compass(Facing::Left, diagonal_turn(Facing::Left, vec2(-1.0, -1.0))), "nw");
        assert_eq!(diagonal_turn(Facing::Left, vec2(-1.0, 0.0)), 0);
        assert_eq!(diagonal_turn(Facing::Down, Vec2::ZERO), 0);
        // Walking backwards (facing up, moving down-left) keeps the facing.
        assert_eq!(diagonal_turn(Facing::Up, vec2(-1.0, 1.0)), 0);
        // Nearly straight: the nearest eighth is the facing itself.
        assert_eq!(diagonal_turn(Facing::Right, vec2(1.0, 0.2)), 0);
    }

    #[test]
    fn the_atlas_clears_the_page_drawn_from_longest_ago() {
        // Page 0 was filled first but its poses are drawn every frame: page 1 goes.
        assert_eq!(pick_page(&[90, 12, 40]), 1);
        assert_eq!(pick_page(&[7, 7, 9]), 0);
        assert_eq!(pick_page(&[]), 0);
        // Sizes: 4 MB pages, clamped to 8..=128 MB.
        assert_eq!(atlas_pages(DEFAULT_ATLAS_MB), 8);
        assert_eq!(atlas_pages(1), 2);
        assert_eq!(atlas_pages(10_000), 32);
        assert_eq!(page_cap(&Cache::default()), 8);
    }

    #[test]
    fn poses_step_through_their_clip() {
        let l = loaded();
        // 2 units tall, drawn 40 world px = 80 image px: 40 px per unit; the frame has room.
        assert_eq!(l.ppu, 40.0);
        assert!(l.frame.0 >= 88 && l.frame.1 >= 104 && l.frame.0.is_multiple_of(2));
        let look = Appearance { model: "m".into(), ..Default::default() };
        let gear = Gear::default();
        let req = |anim, time, once| PoseReq { look: &look, gear: &gear, anim, time, once, facing: Facing::Down, turn: 0, bust: false };
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

    #[test]
    fn a_short_weapon_hangs_at_the_hip_a_long_one_stays_in_the_hands() {
        use crate::defs::{ItemDef, ItemModel, Weapon};
        let mut db = Database::default();
        let with = |id: &str, w: Weapon| ItemDef { weapon: Some(w), model: Some(ItemModel::new("models/x.glb")), ..ItemDef::fresh(id) };
        db.items.push(with("sword", Weapon::melee(2, 0.4)));
        // The SRD's spear: versatile, not two-handed, but long.
        db.items.push(with("spear", Weapon { reach: 1.9, ..Weapon::melee(2, 0.5) }));
        let gear = |id: &str| Gear { weapon: Some(id.into()), ..Default::default() };
        assert_eq!(sheathed(&db, &gear("sword"), "idle"), Some("sword"));
        assert_eq!(sheathed(&db, &gear("sword"), "attack_1h"), None, "attacking: in the hand");
        assert_eq!(sheathed(&db, &gear("spear"), "idle"), None);
    }
}
