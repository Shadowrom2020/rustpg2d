//! Content definitions edited in the editor and stored as RON in `defs/`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::entity::EntityKind;
use crate::events::{Condition, Trigger};
use crate::inventory::ItemStack;

/// A light source: a soft circle of light around a tile, prop, character or carried item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightSpec {
    /// Radius in tiles.
    #[serde(default = "default_light_radius")]
    pub radius: f32,
    #[serde(default = "default_light_color")]
    pub color: [u8; 3],
    /// 0..1: how strongly it lights its circle.
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    /// Flickers like a flame (torches, campfires).
    #[serde(default)]
    pub flicker: bool,
}

fn default_light_radius() -> f32 {
    4.0
}
fn default_light_color() -> [u8; 3] {
    [255, 214, 160]
}
fn default_intensity() -> f32 {
    1.0
}

impl Default for LightSpec {
    fn default() -> Self {
        LightSpec { radius: default_light_radius(), color: default_light_color(), intensity: default_intensity(), flicker: false }
    }
}

impl LightSpec {
    pub fn new(radius: f32, color: [u8; 3], flicker: bool) -> Self {
        LightSpec { radius, color, intensity: 1.0, flicker }
    }
    /// A flickering warm torch light.
    pub fn torch(radius: f32) -> Self {
        LightSpec::new(radius, [255, 190, 120], true)
    }
}

/// A tile of a tileset that gives light.
impl ItemDef {
    /// The equipment slot this item goes into, if it can be equipped.
    pub fn equip_slot(&self) -> Option<EquipSlot> {
        if !self.tool.is_empty() {
            Some(EquipSlot::Tool)
        } else if self.weapon.is_some() {
            Some(EquipSlot::Weapon)
        } else {
            self.armor.as_ref().map(|a| a.slot)
        }
    }
}

/// A sound effect or music track (WAV or OGG).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundDef {
    pub id: String,
    /// Path relative to the project root.
    pub file: String,
    #[serde(default = "full_volume")]
    pub volume: f32,
    /// What it sounds like, for players who can't hear it ("Door creaks", "Dog barks"): shown
    /// when it plays with *Captions* on (Options › Accessibility); for music the track's name.
    /// Translated like dialogs. Empty = no caption (footsteps, UI blips).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caption: String,
}

fn full_volume() -> f32 {
    1.0
}

/// A sprite sheet: an image cut into equally sized frames.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetDef {
    pub id: String,
    pub image: String,
    /// Frame size in image pixels.
    pub frame_w: u32,
    pub frame_h: u32,
    /// Image pixels per world pixel (1 = as before; 2 = high resolution: 64 x 96 frames fill a
    /// 32 x 48 place). Positions, hitboxes and tiles stay in world pixels.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: u32,
    /// What the sheet is for (emotion icons over heads, or anything else).
    #[serde(default, skip_serializing_if = "SheetKind::is_sprite")]
    pub kind: SheetKind,
    /// Named frame ranges (animations), e.g. from an Aseprite export: offered by the *Effect* /
    /// *Fly* actions and the `tag` option of `effect()` / `fly()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<SheetTag>,
}

/// A named animation of a sprite sheet: frames `first..=last` at `fps`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetTag {
    pub name: String,
    pub first: u32,
    pub last: u32,
    pub fps: f32,
}

/// The serde defaults for the optional fields; `frame_w` / `frame_h` (required in files) are one
/// tile. The editor's "+ New" value is [`SheetDef::fresh`].
impl Default for SheetDef {
    fn default() -> Self {
        SheetDef { id: String::new(), image: String::new(), frame_w: crate::project::TILE_SIZE, frame_h: crate::project::TILE_SIZE, scale: 1, kind: SheetKind::Sprite, tags: Vec::new() }
    }
}

impl SheetDef {
    pub fn new(id: &str, image: &str, frame_w: u32, frame_h: u32) -> Self {
        SheetDef { id: id.into(), image: image.into(), frame_w, frame_h, ..Default::default() }
    }

    /// A new sheet as the editor creates it (one-tile frames).
    pub fn fresh(id: &str) -> Self {
        Self::new(id, "sprites/new.png", crate::project::TILE_SIZE, crate::project::TILE_SIZE)
    }
    /// "goblin_scout" -> "Goblin scout": the id as a label, for pickers.
    pub fn label(&self) -> String {
        let s = self.id.replace('_', " ");
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    }
}

/// A part of a model that can take another colour: the art's own `shades` of it (exact colours,
/// darkest to lightest or in any order) are replaced by the chosen colour, each as much darker or
/// lighter as it was than the feature's middle shade.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorFeature {
    /// "skin", "hair", "clothing"...: characters and the creator name the feature.
    pub name: String,
    pub shades: Vec<[u8; 3]>,
    /// The colours offered (the character creator's swatches; empty = the editor offers any).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<[u8; 3]>,
    /// The colour the feature takes when a look chose none (the creator's *As drawn*, characters
    /// without colours); None = the art's own shades. For art whose own colours don't suit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<[u8; 3]>,
}

impl ColorFeature {
    /// "hair_colour" -> "Hair colour": the creator's row label (translatable content).
    pub fn label(&self) -> String {
        let s = self.name.replace('_', " ");
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    }
}

/// Role of a sprite sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetKind {
    /// Anything else: props, items, effects. (Sheets of the characters of earlier versions,
    /// layered parts and the 0.17 figures, load as plain sprites: characters are 3D models.)
    #[default]
    #[serde(alias = "Hair", alias = "Beard", alias = "Outfit", alias = "Armor", alias = "Held", alias = "Body")]
    Sprite,
    /// Emotion icons shown over characters' heads: every tag (named frame range) is an emotion
    /// (`emote(target, "joy")`, the *Show emotion* action).
    Emotions,
}

impl SheetKind {
    pub const ALL: [SheetKind; 2] = [SheetKind::Sprite, SheetKind::Emotions];
    pub fn label(self) -> &'static str {
        match self {
            SheetKind::Sprite => "Sprite",
            SheetKind::Emotions => "Emotions (icons over heads)",
        }
    }
    fn is_sprite(&self) -> bool {
        *self == SheetKind::Sprite
    }
}

/// Look of a character: its 3D model (`ModelDef`; "" = its figure `body`, a sheet of kind Body)
/// and the colours chosen for the colour features (feature name -> colour; a feature not listed
/// keeps the art's colours).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    pub body: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, [u8; 3]>,
    /// The character's size in percent of its model (0 = 100), times its race's size: the drawn
    /// figure, what it holds and its hitbox. Set on the character definition (*Size*); placed
    /// characters and the player (the player character's definition) take it from there.
    #[serde(skip_serializing_if = "is_zero_u16")]
    pub size: u16,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

impl Appearance {
    /// The size as a factor (1 = as the model is).
    pub fn scale(&self) -> f32 {
        if self.size == 0 { 1.0 } else { (self.size as f32 / 100.0).clamp(0.25, 4.0) }
    }

    /// Whether it shows anything: a model or a figure.
    pub fn has_look(&self) -> bool {
        !self.model.is_empty() || !self.body.is_empty()
    }
}

/// A held item's 3D model: a `.glb` under `models/`, where the hand holds it, which way its
/// handle runs and which side is its front (the model's own units and axes), how it is held and
/// how long it is in the world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemModel {
    pub file: String,
    /// The hold point.
    #[serde(default)]
    pub grip: [f32; 3],
    /// The direction from the hold point toward the business end (the blade, the head; a
    /// lantern's body, a shield's top): it lies on the hand's grip axis (see [`HoldStyle`]).
    #[serde(default = "item_axis")]
    pub axis: [f32; 3],
    /// Length of the whole model in world pixels (a sword about 22, a spear 50).
    #[serde(default = "item_size")]
    pub size: f32,
    /// Its front, across the handle: the edge of a blade, the head of an axe or pickaxe, a bow's
    /// back, a shield's face. It points where the hand's knuckles do (a shield: away from the
    /// body). Zero: the wider side across the handle ([`crate::model::auto_front`]).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub front: [f32; 3],
    /// How it is held.
    #[serde(default, skip_serializing_if = "HoldStyle::is_grip")]
    pub hold: HoldStyle,
}

/// How a held item model sits in the hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoldStyle {
    /// In the fist, its handle along the grip axis and its front where the knuckles point
    /// (swords, axes, tools, bows; two-handed: along the line through both fists).
    #[default]
    Grip,
    /// Held up at the hand, its handle pointing up whatever the arm does (torches, staffs).
    Upright,
    /// Hanging from the hand, its handle above it (lanterns, buckets).
    Hang,
    /// On the forearm, its face turned away from the body and its top up (shields).
    Strap,
    /// On the back, against the upper back, its top up and its front facing away from the body
    /// (backpacks): not in a hand.
    Back,
}

impl HoldStyle {
    pub const ALL: [HoldStyle; 5] = [HoldStyle::Grip, HoldStyle::Upright, HoldStyle::Hang, HoldStyle::Strap, HoldStyle::Back];
    fn is_grip(&self) -> bool {
        *self == HoldStyle::Grip
    }
    pub fn label(self) -> &'static str {
        match self {
            HoldStyle::Grip => "Grip",
            HoldStyle::Upright => "Upright",
            HoldStyle::Hang => "Hang",
            HoldStyle::Strap => "Strap",
            HoldStyle::Back => "On the back",
        }
    }
}

fn is_zero3(v: &[f32; 3]) -> bool {
    *v == [0.0; 3]
}

fn item_axis() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}
fn item_size() -> f32 {
    22.0
}

impl ItemModel {
    pub fn new(file: &str) -> ItemModel {
        ItemModel { file: file.into(), grip: [0.0; 3], axis: item_axis(), size: item_size(), front: [0.0; 3], hold: HoldStyle::Grip }
    }
}

/// A 3D character model (`defs/models.ron`): a rigged `.glb` under `models/`, more files with clips
/// of the same rig, the clip each animation plays, its standing height and its colour features
/// (shades of its texture).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelDef {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The model file (`models/hero.glb`).
    pub file: String,
    /// More `.glb` files whose clips the model plays (Meshy writes animations as separate files).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clip_files: Vec<String>,
    /// Clip files rigged on another model (same bone names): they are fitted to this model's
    /// bones when it loads (`Model::add_clips_retargeted`), so one file serves many models.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_clips: Vec<String>,
    /// Standing height in world pixels (a person: 40, two tiles is 64).
    #[serde(default = "model_height")]
    pub height: f32,
    /// Animation name ("walk", "idle", "attack", "hurt", "sit", "fish"...) -> clip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<ClipMap>,
    /// Colour features of its texture (as a figure sheet's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<ColorFeature>,
    /// Offered by the character creator; to female players.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub playable: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub female: bool,
    /// An unrigged model (an animal, a monster) animated by a rig template: another model's
    /// skeleton fitted to it, and that model's clips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rig: Option<RigFit>,
}

/// A rig template fitted to an unrigged model ([`crate::model::Model::fit_rig`]): the template's
/// skeleton is stretched over the model's bounds, then single joints are moved.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RigFit {
    /// The rigged model (an id) whose skeleton and clips it takes.
    pub template: String,
    /// Turns the model about the up axis first (degrees), so it faces the front (+Z, toward the
    /// viewer) like the template.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub turn: f32,
    /// Joints moved after the fit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joints: Vec<JointNudge>,
}

/// A joint of a fitted rig template moved by hand.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JointNudge {
    /// The joint's name in the template.
    pub joint: String,
    /// How far, in parts of the model's height (x right, y up, z front).
    pub offset: [f32; 3],
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn model_height() -> f32 {
    40.0
}

/// The clip an animation of a model plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipMap {
    /// "walk", "idle", "attack", "hurt" or an extra ("sit", "sleep", "fish", "hammer", "farm"…).
    pub anim: String,
    /// The clip's name in the model's files.
    pub clip: String,
    /// Frames per second it is drawn with (stepped, like pixel animation).
    #[serde(default = "clip_fps")]
    pub fps: f32,
    /// Attacks: where in the clip (0..1) the blow lands, a shot is loosed or a block is up; None =
    /// [`DEFAULT_HIT`]. The attack plays up to it during the wind-up and on from it in the swing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit: Option<f32>,
}

/// Where a clip's blow lands when its [`ClipMap::hit`] isn't set (fraction of the clip).
pub const DEFAULT_HIT: f32 = 0.42;

impl ClipMap {
    /// [`ClipMap::hit`], else [`DEFAULT_HIT`], kept inside the clip.
    pub fn hit_point(&self) -> f32 {
        self.hit.unwrap_or(DEFAULT_HIT).clamp(0.05, 0.95)
    }
}

fn clip_fps() -> f32 {
    12.0
}

impl ModelDef {
    pub fn new(id: &str, file: &str) -> ModelDef {
        ModelDef { id: id.into(), name: String::new(), file: file.into(), clip_files: Vec::new(), shared_clips: Vec::new(), height: model_height(), clips: Vec::new(), features: Vec::new(), playable: false, female: false, rig: None }
    }

    /// Every clip file it reads: its own and the shared ones.
    pub fn clip_paths(&self) -> impl Iterator<Item = &String> {
        self.clip_files.iter().chain(self.shared_clips.iter())
    }

    /// The clip of animation `anim`.
    pub fn clip_of(&self, anim: &str) -> Option<&ClipMap> {
        self.clips.iter().find(|c| c.anim == anim)
    }

    /// The template's nudges as (joint, offset).
    pub fn nudges(&self) -> Vec<(String, [f32; 3])> {
        self.rig.as_ref().map_or_else(Vec::new, |r| r.joints.iter().map(|j| (j.joint.clone(), j.offset)).collect())
    }

    pub fn label(&self) -> &str {
        if self.name.is_empty() { &self.id } else { &self.name }
    }

    pub fn feature(&self, name: &str) -> Option<&ColorFeature> {
        self.features.iter().find(|f| f.name == name)
    }
}

/// A sprite: a block of `w × h` frames (cells) of a sheet, starting at `frame`
/// (its top-left cell). Multi-cell sprites make props bigger than one frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpriteRef {
    pub sheet: String,
    pub frame: u32,
    /// Width in cells (0 or 1 = single cell).
    #[serde(default = "one_cell", skip_serializing_if = "is_one")]
    pub w: u32,
    /// Height in cells.
    #[serde(default = "one_cell", skip_serializing_if = "is_one")]
    pub h: u32,
}

impl SpriteRef {
    /// No sprite chosen.
    pub fn is_empty(&self) -> bool {
        self.sheet.is_empty()
    }
}

fn one_cell() -> u32 {
    1
}
fn is_one(v: &u32) -> bool {
    *v <= 1
}

impl SpriteRef {
    pub fn new(sheet: &str, frame: u32) -> Self {
        SpriteRef { sheet: sheet.into(), frame, w: 1, h: 1 }
    }
    /// A `w × h` cell block.
    pub fn block(sheet: &str, frame: u32, w: u32, h: u32) -> Self {
        SpriteRef { sheet: sheet.into(), frame, w, h }
    }
    /// Size in cells, at least 1×1.
    pub fn cells(&self) -> (u32, u32) {
        (self.w.max(1), self.h.max(1))
    }
}

/// Makes an item (or a character) a weapon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weapon {
    /// Real-time damage of 0.28 and older; turn-based combat rolls `dice` (empty = dice from this:
    /// 1 = 1d4, 2 = 1d6, 3 = 1d8, 4 = 1d10, 5+ = 1d12).
    pub damage: i32,
    /// The SRD damage dice of a hit ("1d8"; the wielder's ability modifier is added).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dice: String,
    /// Seconds between attacks.
    #[serde(default = "default_cooldown")]
    pub cooldown: f32,
    /// Shoots projectiles instead of swinging.
    #[serde(default)]
    pub ranged: Option<Ranged>,
    /// Melee: how far the swing reaches, in tiles.
    #[serde(default = "default_reach")]
    pub reach: f32,
    /// Melee: half-angle of the swing in degrees (90 = everything in front).
    #[serde(default = "default_arc")]
    pub arc: f32,
    /// Knockback multiplier for hit targets.
    #[serde(default = "default_knockback")]
    pub knockback: f32,
    /// Needs both hands: nothing can be held in the left hand at the same time.
    #[serde(default)]
    pub two_handed: bool,
    /// A condition (and / or ongoing damage) for whoever the weapon hits (melee or projectile);
    /// until 0.27 a status effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<crate::srd::ConditionSpec>,
    /// Chance (0..1) that a hit of the player's is critical: double damage.
    #[serde(default = "default_crit")]
    pub crit: f32,
    /// What its hits do: slashing, piercing, fire… (`None` = untyped: no resistance applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<crate::srd::DamageType>,
}

fn default_crit() -> f32 {
    0.05
}
fn default_reach() -> f32 {
    1.0
}
fn default_arc() -> f32 {
    60.0
}
fn default_knockback() -> f32 {
    1.0
}

impl Weapon {
    /// The damage dice of a hit: `dice`, else from the old damage number.
    pub fn damage_dice(&self) -> String {
        if !self.dice.trim().is_empty() {
            return self.dice.trim().to_string();
        }
        match self.damage {
            i32::MIN..=1 => "1d4",
            2 => "1d6",
            3 => "1d8",
            4 => "1d10",
            _ => "1d12",
        }
        .into()
    }

    /// A melee weapon with default reach / arc / knockback.
    pub fn melee(damage: i32, cooldown: f32) -> Self {
        Weapon { damage, cooldown, ranged: None, reach: default_reach(), arc: default_arc(), knockback: default_knockback(), two_handed: false, effect: None, crit: default_crit(), damage_type: None, dice: String::new() }
    }
    pub fn ranged(damage: i32, cooldown: f32, ranged: Ranged) -> Self {
        Weapon { ranged: Some(ranged), ..Weapon::melee(damage, cooldown) }
    }
}

/// Equipment slots of the character sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Head,
    Body,
    Weapon,
    /// Left hand: a shield or a light (not while holding a two-handed weapon).
    Offhand,
    /// Clothing (its defense and speed; the figure's art shows what it wears).
    Clothing,
    /// A tool used for an activity (a fishing rod, later pickaxes, hoes, …): held in the
    /// right hand while it is being used, the weapon otherwise.
    Tool,
    /// On the back: a backpack (its `ItemDef::bag_slots` enlarge the player's inventory).
    Back,
}

impl EquipSlot {
    /// Slot by name as scripts write it: "clothing", "head", "body", "weapon", "offhand", "tool",
    /// "back".
    pub fn parse(s: &str) -> Option<EquipSlot> {
        Some(match s.to_ascii_lowercase().as_str() {
            "clothing" | "clothes" | "outfit" => EquipSlot::Clothing,
            "head" | "helmet" => EquipSlot::Head,
            "body" | "armor" => EquipSlot::Body,
            "weapon" | "hand" | "right" => EquipSlot::Weapon,
            "offhand" | "left" | "left hand" | "shield" => EquipSlot::Offhand,
            "tool" => EquipSlot::Tool,
            "back" | "backpack" | "bag" => EquipSlot::Back,
            _ => return None,
        })
    }
    pub const ALL: [EquipSlot; 7] = [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Weapon, EquipSlot::Offhand, EquipSlot::Tool, EquipSlot::Back];
    pub fn label(self) -> &'static str {
        match self {
            EquipSlot::Clothing => "Clothing",
            EquipSlot::Head => "Head",
            EquipSlot::Body => "Body",
            EquipSlot::Weapon => "Weapon",
            EquipSlot::Offhand => "Left hand",
            EquipSlot::Tool => "Tool",
            EquipSlot::Back => "Backpack",
        }
    }
}

/// Clothing, helmets, body armor and left-hand items (shields, lanterns, torches).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Armor {
    /// `Clothing`, `Head`, `Body`, `Offhand` or `Back` (backpacks).
    pub slot: EquipSlot,
    /// Subtracted from incoming damage (at least 1 damage always gets through).
    #[serde(default)]
    pub defense: i32,
    /// Movement speed change, e.g. -0.1 = 10% slower.
    #[serde(default)]
    pub speed: f32,
    /// Damage types its wearer resists (half damage): `fire`, `poison`…
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resistances: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ranged {
    /// Projectile sprite, drawn pointing right and rotated to the flight direction.
    pub projectile: SpriteRef,
    /// Pixels per second.
    #[serde(default = "default_projectile_speed")]
    pub speed: f32,
    /// Maximum distance in tiles (turn-based combat: the normal range; farther than it, up to
    /// `long_range`, attacks have disadvantage).
    #[serde(default = "default_range")]
    pub range: f32,
    /// The long range in tiles (0 = twice `range`).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub long_range: f32,
    /// Item consumed per shot ("" = unlimited).
    #[serde(default)]
    pub ammo: String,
}

fn default_cooldown() -> f32 {
    0.4
}
fn default_projectile_speed() -> f32 {
    440.0
}
fn default_range() -> f32 {
    8.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub icon: SpriteRef,
    /// Removed from the inventory when used (only if an `on_use` trigger ran).
    #[serde(default)]
    pub consumable: bool,
    /// Triggers (event `UseItem`) run when the item is used without a target.
    #[serde(default)]
    pub on_use: Vec<Trigger>,
    /// Using a weapon item from the inventory equips it.
    #[serde(default)]
    pub weapon: Option<Weapon>,
    /// Helmet / body armor (equipped like weapons).
    #[serde(default)]
    pub armor: Option<Armor>,
    /// How many fit in one inventory slot.
    #[serde(default = "default_max_stack")]
    pub max_stack: u32,
    /// Base trade value (merchants without their own price use it; 0 = worthless).
    #[serde(default)]
    pub value: u32,
    /// Gives light while equipped (e.g. a lantern in the left hand) or lying on the ground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightSpec>,
    /// How it looks in a character's hand (weapons, left-hand items, tools); None = its icon,
    /// held at its lower-left end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<crate::hands::HeldSprite>,
    /// Its 3D model, held by 3D model characters (else they hold its picture).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ItemModel>,
    /// Using the item teaches this spell (scrolls, spell books); "" = none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub teaches: String,
    /// What kind of tool it is ("fishing rod", "pickaxe", …; "" = no tool): tools go into the
    /// Tool slot and are what activities ask for (a fishing spot needs a fishing rod).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool: String,
    /// Whom a healing item (its *on use* heals) is used on: the player, a companion, or a choice
    /// between them while companions are around.
    #[serde(default, skip_serializing_if = "UseTarget::is_player")]
    pub use_on: UseTarget,
    /// A seed: what grows from it on a farm plot (`PropDef::plot`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    /// Food: its *on use* healing gets the player's *Food* skill bonus.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub food: bool,
    /// A backpack (armor in the `Back` slot): this many more inventory slots while the player
    /// wears it.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub bag_slots: u32,
    /// The animations a character plays with it in hand (weapons, shields): names that models map
    /// to clips. Empty ones are estimated from the weapon ([`ItemAnims::estimate`]).
    #[serde(default, skip_serializing_if = "ItemAnims::is_empty")]
    pub anims: ItemAnims,
    /// Furniture: using the item in a home of the player's (`WorldState::homes`) places this prop
    /// on the free tile in front; picking the prop up gives the item back. "" = not furniture.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub furniture: String,
}

/// Animation names of an item (`ItemDef::anims`); "" = by its kind ([`ItemAnims::estimate`]),
/// and a model without the named clip plays its plain one (`attack`, `idle`, `walk`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemAnims {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub attack: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub idle: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub walk: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub block: String,
}

/// The standard animation names the editor offers (models map them to clips): attacks, idles,
/// blocks.
pub const STANDARD_ANIMS: [&str; 14] = [
    "attack_1h",
    "attack_2h",
    "attack_thrust",
    "attack_bow",
    "attack_crossbow",
    "attack_cast",
    "attack_unarmed",
    "idle_1h",
    "idle_2h",
    "idle_bow",
    "idle_spear",
    "block_shield",
    "block_weapon",
    "block_2h",
];

impl ItemAnims {
    pub fn is_empty(&self) -> bool {
        self.attack.is_empty() && self.idle.is_empty() && self.walk.is_empty() && self.block.is_empty()
    }

    /// The animations an item has by its kind: a ranged weapon shoots (`attack_crossbow` when its
    /// id or name says crossbow, else `attack_bow`, idling `idle_bow`), a two-handed one with a
    /// long reach thrusts (`attack_thrust`, `idle_spear`), another two-handed one swings overhead
    /// (`attack_2h`, `idle_2h`, parrying `block_2h`), other weapons slash (`attack_1h`, `idle_1h`,
    /// `block_weapon`); a shield (armor with defense in the left hand) blocks (`block_shield`).
    pub fn estimate(item: &ItemDef) -> ItemAnims {
        let named = |w: &str| item.id.to_lowercase().contains(w) || item.name.to_lowercase().contains(w);
        let (attack, idle, block) = match &item.weapon {
            Some(w) if w.ranged.is_some() => (if named("crossbow") { "attack_crossbow" } else { "attack_bow" }, "idle_bow", ""),
            Some(w) if w.two_handed && w.reach > 1.4 => ("attack_thrust", "idle_spear", "block_2h"),
            Some(w) if w.two_handed => ("attack_2h", "idle_2h", "block_2h"),
            Some(_) => ("attack_1h", "idle_1h", "block_weapon"),
            // A shield: armor with defense worn in the left hand (lanterns and torches have none).
            None if item.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Offhand && a.defense > 0) => ("", "", "block_shield"),
            None => ("", "", ""),
        };
        ItemAnims { attack: attack.into(), idle: idle.into(), walk: String::new(), block: block.into() }
    }

    /// The item's own names, the estimated ones filling the empty ones.
    pub fn of(item: &ItemDef) -> ItemAnims {
        let e = Self::estimate(item);
        let a = &item.anims;
        let pick = |own: &String, est: String| if own.is_empty() { est } else { own.clone() };
        ItemAnims { attack: pick(&a.attack, e.attack), idle: pick(&a.idle, e.idle), walk: pick(&a.walk, e.walk), block: pick(&a.block, e.block) }
    }
}

/// The serde defaults (what an entry with only `id`, `name` and `icon` loads as). The editor's
/// "+ New" value is [`ItemDef::fresh`].
impl Default for ItemDef {
    fn default() -> Self {
        ItemDef {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            icon: SpriteRef::default(),
            consumable: false,
            on_use: vec![],
            weapon: None,
            armor: None,
            max_stack: default_max_stack(),
            value: 0,
            light: None,
            held: None,
            model: None,
            teaches: String::new(),
            tool: String::new(),
            use_on: UseTarget::default(),
            crop: None,
            food: false,
            bag_slots: 0,
            anims: ItemAnims::default(),
            furniture: String::new(),
        }
    }
}

impl ItemDef {
    /// How its 3D model is held, how long it is (world pixels) and, when its shape can't tell,
    /// how far along its length it is held, by its kind, for a model of this size (`ext`: its
    /// length, width and thickness, [`crate::model::extents`]): shields on the forearm, lights upright (long ones, torches) or hanging
    /// (lanterns), bows upright at their middle, weapons and tools in the fist; two-handed
    /// weapons about a character's height (40), bows 30, one-handed weapons and tools 24.
    pub fn hold_by_kind(&self, ext: [f32; 3]) -> (HoldStyle, f32, Option<f32>) {
        let mut e = ext;
        e.sort_by(|a, b| b.total_cmp(a));
        let long = e[0] > e[1].max(1e-6) * 2.2;
        let offhand = self.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Offhand);
        if self.armor.as_ref().is_some_and(|a| a.slot == EquipSlot::Back) {
            return (HoldStyle::Back, 16.0, None);
        }
        match &self.weapon {
            _ if offhand && self.light.is_some() => if long { (HoldStyle::Upright, 16.0, None) } else { (HoldStyle::Hang, 12.0, None) },
            _ if offhand => (HoldStyle::Strap, 18.0, None),
            Some(w) if w.ranged.is_some() && e[0] > e[1].max(1e-6) * 1.6 => (HoldStyle::Upright, 30.0, Some(0.5)),
            Some(w) if w.ranged.is_some() => (HoldStyle::Grip, 24.0, None),
            Some(w) if w.two_handed => (HoldStyle::Grip, 44.0, None),
            _ => (HoldStyle::Grip, 24.0, None),
        }
    }

    /// A new item as the editor creates it.
    pub fn fresh(id: &str) -> Self {
        ItemDef { id: id.into(), name: "New item".into(), ..Default::default() }
    }
}

/// What grows from a seed (`ItemDef::crop`): its stages one after another, the last one is the
/// ripe crop, harvested with E for `harvest` (the plot is then empty again).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    #[serde(default)]
    pub stages: Vec<CropStage>,
    #[serde(default)]
    pub harvest: Vec<LootEntry>,
}

/// One look of a growing crop and how many in-game hours it lasts (the last, ripe stage's
/// hours don't count).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CropStage {
    pub sprite: SpriteRef,
    #[serde(default)]
    pub hours: f32,
}

impl Crop {
    /// In-game hours from planting to ripe.
    pub fn grow_hours(&self) -> f32 {
        let n = self.stages.len().saturating_sub(1);
        self.stages[..n].iter().map(|s| s.hours.max(0.0)).sum()
    }

    /// The stage shown `hours` after planting (the last one = ripe); None without stages.
    pub fn stage_at(&self, hours: f64) -> Option<usize> {
        if self.stages.is_empty() {
            return None;
        }
        let mut t = 0.0f64;
        for (k, s) in self.stages.iter().enumerate().take(self.stages.len() - 1) {
            t += s.hours.max(0.0) as f64;
            if hours < t {
                return Some(k);
            }
        }
        Some(self.stages.len() - 1)
    }

    pub fn is_ripe(&self, hours: f64) -> bool {
        self.stage_at(hours) == Some(self.stages.len().saturating_sub(1)) && !self.stages.is_empty()
    }
}

/// Whom a healing item is used on (`ItemDef::use_on`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseTarget {
    /// Always the player (as before).
    #[default]
    Player,
    /// Only companions (a salve for the dog).
    Ally,
    /// The player or a companion: with companions on the map the player chooses.
    Choose,
}

impl UseTarget {
    pub const ALL: [UseTarget; 3] = [UseTarget::Player, UseTarget::Ally, UseTarget::Choose];

    fn is_player(&self) -> bool {
        *self == UseTarget::Player
    }

    pub fn label(self) -> &'static str {
        match self {
            UseTarget::Player => "Player",
            UseTarget::Ally => "Companions",
            UseTarget::Choose => "Player or companion",
        }
    }
}

fn default_max_stack() -> u32 {
    crate::inventory::DEFAULT_STACK
}

/// What happens to a prop when its lock is opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OnUnlock {
    /// Stays as it is (containers then simply open).
    Nothing,
    /// The prop disappears (gates, rubble, ...).
    Disappear,
    /// Legacy: switches to a frame offset and stops blocking movement. Converted to
    /// [`OnUnlock::ToState`] when definitions and maps are loaded (if a matching state exists).
    Open { frame: u32 },
    /// Switches the prop to one of its states (sprite + whether it blocks movement).
    ToState { state: String },
}

/// A lock that needs a key item, or can be picked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "LockSpecRepr")]
pub struct LockSpec {
    /// Item id of the key.
    pub key: String,
    /// The key is used up when unlocking.
    #[serde(default = "yes")]
    pub consume_key: bool,
    #[serde(default = "on_unlock_default")]
    pub on_unlock: OnUnlock,
    /// Shown without the key ("" = "It's locked. You need <key name>.").
    #[serde(default)]
    pub locked_text: String,
    /// Shown when unlocking ("" = "Unlocked with <key name>.").
    #[serde(default)]
    pub unlock_text: String,
    /// Once open, the player can lock it again if they have the key (without a key item:
    /// close it again). Not possible for props that disappear when unlocked.
    #[serde(default)]
    pub relock: bool,
    /// Shown when locking it again ("" = "Locked with <key name>." / "Closed.").
    #[serde(default)]
    pub lock_text: String,
    /// Picking it without the key: a DEX (Thieves' Tools) check against this DC (0 = it can't
    /// be picked).
    #[serde(default)]
    pub pick_dc: u32,
}

/// A lock as files have it: `pick_dc` since 0.28; until then `pick_difficulty`, a lockpicking
/// skill level, converted with [`crate::srd::old_level_dc`].
#[derive(Deserialize)]
struct LockSpecRepr {
    key: String,
    #[serde(default = "yes")]
    consume_key: bool,
    #[serde(default = "on_unlock_default")]
    on_unlock: OnUnlock,
    #[serde(default)]
    locked_text: String,
    #[serde(default)]
    unlock_text: String,
    #[serde(default)]
    relock: bool,
    #[serde(default)]
    lock_text: String,
    #[serde(default)]
    pick_dc: u32,
    #[serde(default)]
    pick_difficulty: u32,
}

impl From<LockSpecRepr> for LockSpec {
    fn from(r: LockSpecRepr) -> Self {
        let pick_dc = if r.pick_dc > 0 { r.pick_dc } else { crate::srd::old_level_dc(r.pick_difficulty) };
        LockSpec { key: r.key, consume_key: r.consume_key, on_unlock: r.on_unlock, locked_text: r.locked_text, unlock_text: r.unlock_text, relock: r.relock, lock_text: r.lock_text, pick_dc }
    }
}

fn yes() -> bool {
    true
}
impl LockSpec {
    /// Whether an opened lock can be locked (closed) again.
    pub fn can_relock(&self) -> bool {
        self.relock && self.on_unlock != OnUnlock::Disappear
    }
}

fn on_unlock_default() -> OnUnlock {
    OnUnlock::Nothing
}

impl Default for LockSpec {
    fn default() -> Self {
        LockSpec {
            key: String::new(),
            consume_key: true,
            on_unlock: OnUnlock::Nothing,
            locked_text: String::new(),
            unlock_text: String::new(),
            pick_dc: 0,
            relock: false,
            lock_text: String::new(),
        }
    }
}

/// A lootable container (chest, barrel, ...): its size, starting contents and lock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// Number of slots.
    pub size: u32,
    /// Starting contents, placed into the slots in order.
    #[serde(default)]
    pub items: Vec<ItemStack>,
    /// All must hold for the container to open.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Message shown when the conditions don't hold.
    #[serde(default)]
    pub locked_text: String,
    /// Legacy frame offset set the first time it is opened (converted to `open_state`).
    #[serde(default)]
    pub open_frame: u32,
    /// State the prop switches to the first time it is opened ("" = no change).
    #[serde(default)]
    pub open_state: String,
}

impl Default for ContainerSpec {
    fn default() -> Self {
        ContainerSpec { size: 8, items: vec![], conditions: vec![], locked_text: String::new(), open_frame: 0, open_state: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropDef {
    pub id: String,
    pub name: String,
    /// Sprite of the default state (named `default_state`).
    pub sprite: SpriteRef,
    /// Whether the default state blocks movement.
    #[serde(default)]
    pub solid: bool,
    /// Name of the default state (the one placed props start in).
    #[serde(default = "default_state_name")]
    pub default_state: String,
    /// Further states (e.g. "open", "destroyed"), each with its own sprite.
    #[serde(default)]
    pub states: Vec<PropState>,
    /// Collision / interaction box (w, h) in pixels, anchored bottom-center.
    #[serde(default = "default_box")]
    pub hitbox: [f32; 2],
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Default container for every placed instance (instances can override it).
    #[serde(default)]
    pub container: Option<ContainerSpec>,
    /// Default lock for every placed instance (instances can override it).
    #[serde(default)]
    pub lock: Option<LockSpec>,
    /// Light of the default state (e.g. a lit lamp).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightSpec>,
    /// Furniture: where and how a character uses it (sits, lies, stands at it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_spot: Option<UseSpot>,
    /// Gathering: hitting it with a tool gives items (ore veins, trees, bushes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gather: Option<Gather>,
    /// A state that changes back after some in-game time (a depleted vein refills).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regrow: Option<Regrow>,
    /// A farm plot: seeds (items with a crop) are planted on it and grow over in-game time.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub plot: bool,
    /// A low prop (a barrel, a chair, a fence): when solid it stops walking but not shots or
    /// sight. Solid props that aren't low stop both (trees, statues, stalls).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub low: bool,
    /// Lies flat on the ground (a rug, a mat, a puddle): drawn under everyone like a *Flat*
    /// decoration instead of sorted by its feet, and placed as furniture by its picture.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flat: bool,
    /// A door: E opens it (its "open" state) and closes it again (its first state); a lock keeps
    /// it shut until its key or a lockpick opens it, and with the key closing it locks it again.
    /// Characters walking their routes open unlocked doors (and locked ones they carry the key
    /// of) and close them behind them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub door: bool,
    /// How characters carry it in both hands (*Carry prop*): its hold point and orientation mark
    /// (None: the middle of its lower half, its horizontal along the hands).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carry: Option<crate::hands::HeldSprite>,
    /// A ladder: the player climbs it (characters keep off), placed as a decoration.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub climb: bool,
    /// Placed on a grid of this many pixels (16 or 32: wall pieces); 0 = anywhere.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub snap: u16,
    /// A wall set: the prop for each neighbour mask (`decor::NORTH | EAST | SOUTH | WEST`, 16
    /// entries; "" = this prop). A line of the set picks its pieces by their neighbours.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<String>,
    /// An animated picture: the default state cycles through these frames of its sheet (a torch,
    /// a flag, a water wheel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim: Option<FrameAnim>,
}

/// Frames of a sprite sheet shown one after the other (`PropDef::anim`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameAnim {
    pub frames: Vec<u32>,
    pub fps: f32,
}

impl FrameAnim {
    /// The frame shown at `time` seconds.
    pub fn frame_at(&self, time: f64) -> Option<u32> {
        let n = self.frames.len();
        (n > 0).then(|| self.frames[((time * self.fps.max(0.01) as f64) as usize) % n])
    }
}

/// The serde defaults (what an entry with only `id`, `name` and `sprite` loads as: not solid,
/// a 24 x 16 px hitbox). The editor's "+ New" value is [`PropDef::fresh`].
impl Default for PropDef {
    fn default() -> Self {
        PropDef {
            id: String::new(),
            name: String::new(),
            sprite: SpriteRef::default(),
            solid: false,
            default_state: default_state_name(),
            states: vec![],
            hitbox: default_box(),
            triggers: vec![],
            container: None,
            lock: None,
            light: None,
            use_spot: None,
            gather: None,
            regrow: None,
            plot: false,
            low: false,
            flat: false,
            door: false,
            carry: None,
            climb: false,
            snap: 0,
            pieces: vec![],
            anim: None,
        }
    }
}

impl PropDef {
    /// A new prop as the editor creates it: solid, a 28 x 16 px hitbox.
    pub fn fresh(id: &str) -> Self {
        PropDef { id: id.into(), name: "New prop".into(), solid: true, hitbox: [28.0, 16.0], ..Default::default() }
    }
}

/// A prop the player gathers from (`PropDef::gather`): `hits` hits with a tool of kind `tool`
/// (equipped or carried; "" = bare hands) roll `loot`, then the prop switches to `then_state`
/// (it can't be gathered in that state).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gather {
    #[serde(default)]
    pub tool: String,
    #[serde(default = "three")]
    pub hits: u32,
    #[serde(default)]
    pub loot: Vec<LootEntry>,
    #[serde(default)]
    pub then_state: String,
}

fn three() -> u32 {
    3
}

/// A prop state that turns into another one after `hours` in-game hours (`PropDef::regrow`),
/// also while the player is on another map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Regrow {
    pub from: String,
    #[serde(default)]
    pub to: String,
    pub hours: f32,
}

/// How a character uses a piece of furniture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pose {
    /// Chairs, benches: a short rest.
    #[default]
    Sit,
    /// Beds: sleeping skips time.
    Lie,
    /// Counters, anvils, desks: only facing it.
    Stand,
}

impl Pose {
    pub const ALL: [Pose; 3] = [Pose::Sit, Pose::Lie, Pose::Stand];
    pub fn label(self) -> &'static str {
        match self {
            Pose::Sit => "Sit",
            Pose::Lie => "Lie (bed)",
            Pose::Stand => "Stand",
        }
    }
}

/// The spot of a piece of furniture: where the user's feet go (relative to the prop's feet,
/// pixels) and how they are posed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UseSpot {
    #[serde(default)]
    pub pose: Pose,
    #[serde(default)]
    pub offset: [f32; 2],
    /// The way the user looks (chairs: any of the four). In a bed: the side the head is on
    /// (Up = at the top, an upright bed; Left / Right = a sideways bed; Down = at the bottom).
    #[serde(default)]
    pub facing: crate::entity::Facing,
    /// Beds: the hour sleeping lasts until.
    #[serde(default = "default_wake")]
    pub sleep_until: f32,
    /// Drawn over the user while it is used (a bed's blanket); empty sheet = nothing.
    #[serde(default)]
    pub cover: SpriteRef,
    /// The furniture can only be used when these hold (a paid inn room)...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<crate::events::Condition>,
    /// ...else this is shown ("" = "You can't use that now.").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub refuse_text: String,
}

fn default_wake() -> f32 {
    7.0
}

impl Default for UseSpot {
    fn default() -> Self {
        UseSpot { pose: Pose::Sit, offset: [0.0, 0.0], facing: Default::default(), sleep_until: default_wake(), cover: SpriteRef::default(), conditions: Vec::new(), refuse_text: String::new() }
    }
}

/// A named look of a prop: its sprite and whether it blocks movement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropState {
    pub name: String,
    pub sprite: SpriteRef,
    #[serde(default)]
    pub solid: bool,
    /// Light in this state (e.g. "lit"; none for "off").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightSpec>,
}

pub fn default_state_name() -> String {
    "default".into()
}

impl PropDef {
    /// Light of state `i` (0 = default state).
    pub fn state_light(&self, i: usize) -> Option<&LightSpec> {
        if i == 0 {
            self.light.as_ref()
        } else {
            self.states.get(i - 1).and_then(|s| s.light.as_ref())
        }
    }

    /// All state names, the default state first.
    pub fn state_names(&self) -> Vec<String> {
        std::iter::once(self.default_state.clone()).chain(self.states.iter().map(|s| s.name.clone())).collect()
    }
    /// Index of a state (0 = default; "" also means the default state).
    pub fn state_index(&self, name: &str) -> Option<usize> {
        if name.is_empty() || name == self.default_state {
            return Some(0);
        }
        self.states.iter().position(|s| s.name == name).map(|i| i + 1)
    }
    pub fn state_name(&self, index: usize) -> &str {
        if index == 0 { &self.default_state } else { self.states.get(index - 1).map_or("", |s| s.name.as_str()) }
    }
    /// Sprite and solidity of state `index` (out of range: the default state).
    /// The picture of state `index` at `time` seconds (the default state's `anim` cycles).
    pub fn sprite_at(&self, index: usize, time: f64) -> std::borrow::Cow<'_, SpriteRef> {
        let s = self.state_look(index).0;
        match self.anim.as_ref().filter(|_| index == 0).and_then(|a| a.frame_at(time)) {
            Some(frame) => std::borrow::Cow::Owned(SpriteRef { frame, ..s.clone() }),
            None => std::borrow::Cow::Borrowed(s),
        }
    }

    pub fn state_look(&self, index: usize) -> (&SpriteRef, bool) {
        match index.checked_sub(1).and_then(|i| self.states.get(i)) {
            Some(s) => (&s.sprite, s.solid),
            None => (&self.sprite, self.solid),
        }
    }
    /// The state whose sprite is the default sprite shifted by `offset` frames (old frame
    /// offsets); created as `name` if there is none.
    fn state_for_offset(&mut self, offset: u32, name: &str, solid: bool) -> String {
        let sprite = SpriteRef { frame: self.sprite.frame + offset, ..self.sprite.clone() };
        if offset == 0 {
            return self.default_state.clone();
        }
        if let Some(s) = self.states.iter().find(|s| s.sprite == sprite) {
            return s.name.clone();
        }
        let mut n = name.to_string();
        let mut k = 2;
        while self.state_index(&n).is_some() {
            n = format!("{name}_{k}");
            k += 1;
        }
        self.states.push(PropState { name: n.clone(), sprite, solid, light: None });
        n
    }
    /// Converts the legacy frame offsets of the lock / container to named states.
    pub fn upgrade_states(&mut self) {
        let lock_offset = match self.lock.as_ref().map(|l| &l.on_unlock) {
            Some(OnUnlock::Open { frame }) => Some(*frame),
            _ => None,
        };
        if let Some(off) = lock_offset {
            if self.states.is_empty() && self.default_state == default_state_name() {
                self.default_state = "closed".into();
            }
            let st = self.state_for_offset(off, "open", false);
            self.lock.as_mut().unwrap().on_unlock = OnUnlock::ToState { state: st };
        }
        let open_frame = self.container.as_ref().map_or(0, |c| c.open_frame);
        if open_frame > 0 && self.container.as_ref().is_some_and(|c| c.open_state.is_empty()) {
            if self.states.is_empty() && self.default_state == default_state_name() {
                self.default_state = "closed".into();
            }
            let solid = self.solid;
            let st = self.state_for_offset(open_frame, "open", solid);
            let c = self.container.as_mut().unwrap();
            c.open_state = st;
            c.open_frame = 0;
        }
    }
    /// The state a legacy frame offset of an instance refers to (if any).
    pub fn state_of_offset(&self, offset: u32) -> Option<String> {
        if offset == 0 {
            return Some(self.default_state.clone());
        }
        let sprite = SpriteRef { frame: self.sprite.frame + offset, ..self.sprite.clone() };
        self.states.iter().find(|s| s.sprite == sprite).map(|s| s.name.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Behavior {
    Static,
    /// Walks randomly within `radius` tiles of its start position.
    Wander { radius: f32 },
    /// Runs at the player when it sees them within `sight` tiles, otherwise wanders near home.
    Chase {
        sight: f32,
        /// Gives up and walks back home when this many tiles from home (0 = never).
        #[serde(default)]
        leash: f32,
        /// Runs away from the player below this fraction of its max HP (0 = never).
        #[serde(default)]
        flee_below: f32,
        /// Shooters back off to stay this many tiles from the player (0 = they just stop
        /// coming closer once the player is well within range).
        #[serde(default)]
        keep_distance: f32,
        /// Fights as a group with others that chase: at most two close in on one target at a
        /// time (the rest circle and wait for a gap), they spread out instead of stacking, and the
        /// badly hurt run to their allies and come back.
        #[serde(default = "yes")]
        group: bool,
        /// On noticing the player, alerts others of the same kind within this many tiles (0 = never).
        #[serde(default = "default_alert")]
        alert: f32,
    },
}

fn default_alert() -> f32 {
    6.0
}

impl Behavior {
    /// Chasing with only a sight range (no leash, never flees, no kept distance; fights as a
    /// group and alerts its kind within 6 tiles).
    pub fn chase(sight: f32) -> Self {
        Behavior::Chase { sight, leash: 0.0, flee_below: 0.0, keep_distance: 0.0, group: true, alert: default_alert() }
    }
}

/// A character: a figure (`appearance`), what it carries (`equipment`, counted in play; held items
/// show in figures with hand marks, `hands.rs`),
/// behavior and combat values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterDef {
    pub id: String,
    pub name: String,
    /// Its race (`races::RaceDef`; "" = none): for conditions, scripts and factions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub race: String,
    /// Its faction (`ProjectConfig::factions`; "" = its race's).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub faction: String,
    /// Movement speed in pixels per second.
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_box")]
    pub hitbox: [f32; 2],
    #[serde(default = "default_behavior")]
    pub behavior: Behavior,
    /// Hit points; 0 = can't be damaged. For the player character this is the max HP.
    #[serde(default)]
    pub max_hp: i32,
    /// Contact damage (hostile characters) or attack damage (player character).
    #[serde(default)]
    pub damage: i32,
    /// Hurts the player on contact.
    #[serde(default)]
    pub hostile: bool,
    /// Hostile characters with a ranged weapon shoot at the player.
    #[serde(default)]
    pub weapon: Option<Weapon>,
    /// Subtracted from incoming damage (player: base defense before armor).
    #[serde(default)]
    pub defense: i32,
    /// Experience the player gains for defeating the character (with experience and levels on).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub xp: u32,
    /// A status effect its touch gives the player (hostile characters, with contact damage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub touch_effect: Option<crate::srd::ConditionSpec>,
    /// Seconds a hostile character winds up before its weapon swing or lunge (0 = the standard
    /// 0.4 s swing / 0.3 s lunge): the time the player has to step out of the way.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub windup: f32,
    /// What the bestiary says about it (shown once the player has defeated one).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Makes the character a merchant (placed instances can override it).
    #[serde(default)]
    pub shop: Option<ShopSpec>,
    /// What the character can drop when defeated (placed instances and spawners can
    /// override it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loot: Vec<LootEntry>,
    /// A light the character carries (placed instances can override it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightSpec>,
    /// The model: body, skin, hair (dressed by `equipment`).
    #[serde(default)]
    pub appearance: Appearance,
    /// What the character wears and holds (players: what a new game starts with).
    #[serde(default, skip_serializing_if = "crate::state::Equipment::is_empty")]
    pub equipment: crate::state::Equipment,
    /// What the character carries (dropped when it is defeated; players: start inventory).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inventory: Vec<crate::inventory::ItemStack>,
    /// As a companion it can't be given gear (a wolf, a slime); it still carries items.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gear_locked: bool,
    /// A drawn portrait for dialogs and the HUD (empty = the head and shoulders of its model).
    #[serde(default, skip_serializing_if = "SpriteRef::is_empty")]
    pub portrait: SpriteRef,
    /// Portraits for moods ("worried", "angry", ...), shown while a dialog sets that mood.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub moods: std::collections::BTreeMap<String, SpriteRef>,
    /// As a companion: its mana for spells it learns (0 = it can't learn any).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub max_mana: u32,
    /// Mana it regains per second.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub mana_regen: f32,
    /// Its SRD stat block (abilities, armor class, hit points, attacks, challenge; `None` = none):
    /// fights use it; scripts read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<crate::srd::StatBlock>,
    /// A party member's class (with its subclass and choices): as a companion it levels with the
    /// hero and fights with its class features (`None` = none, its stat block alone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<crate::classes::HeroClass>,
    /// The level it joins at (with a class; 0 = 1).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub level: u32,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

/// The serde defaults (what an entry with only `id` and `name` loads as). The editor's "+ New"
/// value is [`CharacterDef::fresh`]; the character builder fills in a body and clothes.
impl Default for CharacterDef {
    fn default() -> Self {
        CharacterDef {
            id: String::new(),
            name: String::new(),
            race: String::new(),
            faction: String::new(),
            speed: default_speed(),
            hitbox: default_box(),
            behavior: default_behavior(),
            max_hp: 0,
            damage: 0,
            hostile: false,
            weapon: None,
            defense: 0,
            xp: 0,
            touch_effect: None,
            windup: 0.0,
            description: String::new(),
            triggers: vec![],
            shop: None,
            loot: vec![],
            light: None,
            appearance: Appearance::default(),
            equipment: crate::state::Equipment::default(),
            inventory: vec![],
            gear_locked: false,
            portrait: SpriteRef::default(),
            moods: Default::default(),
            max_mana: 0,
            mana_regen: 0.0,
            stats: None,
            class: None,
            level: 0,
        }
    }
}

impl CharacterDef {
    /// A new character as the editor creates it (before the builder picks its look).
    pub fn fresh(id: &str) -> Self {
        CharacterDef { id: id.into(), name: "New character".into(), ..Default::default() }
    }
}

/// One line of a loot table: `item` drops with `chance` percent, `min..=max` of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LootEntry {
    pub item: String,
    /// Drop chance in percent (0-100).
    pub chance: f32,
    #[serde(default = "one")]
    pub min: u32,
    #[serde(default = "one")]
    pub max: u32,
}

impl LootEntry {
    pub fn new(item: &str, chance: f32, min: u32, max: u32) -> Self {
        LootEntry { item: item.to_string(), chance, min, max }
    }
}

/// Rolls a loot table: every entry independently drops with its chance. `rand01()` returns
/// numbers in 0..1, `range(min, max)` an integer in min..=max.
pub fn roll_loot(table: &[LootEntry], mut rand01: impl FnMut() -> f32, mut range: impl FnMut(u32, u32) -> u32) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for e in table.iter().filter(|e| !e.item.is_empty()) {
        if rand01() * 100.0 < e.chance {
            let (lo, hi) = (e.min.min(e.max), e.min.max(e.max));
            let n = range(lo, hi);
            if n == 0 {
                continue;
            }
            match out.iter_mut().find(|(i, _)| *i == e.item) {
                Some(x) => x.1 += n,
                None => out.push((e.item.clone(), n)),
            }
        }
    }
    out
}

/// One line of a merchant's stock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShopEntry {
    pub item: String,
    /// How many the merchant has (ignored when `unlimited`).
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default)]
    pub unlimited: bool,
    /// Price in currency; 0 = the item's value.
    #[serde(default)]
    pub price: u32,
}

impl ShopEntry {
    pub fn new(item: &str, count: u32, price: u32) -> Self {
        ShopEntry { item: item.to_string(), count, unlimited: false, price }
    }
    /// Whether the merchant still has one to sell.
    pub fn available(&self) -> bool {
        self.unlimited || self.count > 0
    }
}

fn one() -> u32 {
    1
}

/// A merchant: what they sell, for what, and what they pay for the player's items.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShopSpec {
    /// Trade window title ("" = the character's name).
    #[serde(default)]
    pub title: String,
    /// Item used as money.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Stock (and prices) when the game starts.
    #[serde(default)]
    pub items: Vec<ShopEntry>,
    /// Percent of the price paid when the player sells an item; 0 = doesn't buy.
    #[serde(default = "default_sell_rate")]
    pub sell_rate: u32,
    /// Talking to the character opens the shop (after its Interact triggers). Otherwise
    /// only the "Open shop" action / open_shop() does.
    #[serde(default = "yes")]
    pub open_on_talk: bool,
}

fn default_currency() -> String {
    "coin".into()
}
fn default_sell_rate() -> u32 {
    50
}

impl Default for ShopSpec {
    fn default() -> Self {
        ShopSpec { title: String::new(), currency: default_currency(), items: Vec::new(), sell_rate: default_sell_rate(), open_on_talk: true }
    }
}

impl ShopSpec {
    /// What the merchant asks for one `item` (stock entries are looked up in `stock`).
    pub fn buy_price(stock: &[ShopEntry], item: &str, db: &Database) -> u32 {
        match stock.iter().find(|e| e.item == item) {
            Some(e) if e.price > 0 => e.price,
            _ => db.item(item).map_or(0, |d| d.value),
        }
    }
    /// What the merchant pays for one `item` (None = won't buy it).
    pub fn sell_price(&self, stock: &[ShopEntry], item: &str, db: &Database) -> Option<u32> {
        if self.sell_rate == 0 || item == self.currency {
            return None;
        }
        let base = Self::buy_price(stock, item, db);
        (base > 0).then(|| (base * self.sell_rate / 100).max(1))
    }
}

fn default_box() -> [f32; 2] {
    [24.0, 16.0]
}
fn default_speed() -> f32 {
    120.0
}
fn default_behavior() -> Behavior {
    Behavior::Static
}

/// All definitions of a project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Database {
    pub sheets: Vec<SheetDef>,
    pub items: Vec<ItemDef>,
    pub props: Vec<PropDef>,
    pub characters: Vec<CharacterDef>,
    pub sounds: Vec<SoundDef>,
    pub quests: Vec<crate::quest::QuestDef>,
    /// Multi-tile assets for the editor (houses, walls, rooms…); the game doesn't use them.
    pub structures: Vec<StructureDef>,
    /// Spells (`ProjectConfig::skills.magic` switches magic on).
    pub spells: Vec<crate::skills::SpellDef>,
    /// Crafting recipes.
    pub recipes: Vec<crate::crafting::RecipeDef>,
    /// Achievements (the game's; unlocks are the player's, see `achievements::Progress`).
    pub achievements: Vec<crate::achievements::AchievementDef>,
    /// Races (humans, goblins, elves…; the player picks one in the creator).
    pub races: Vec<crate::races::RaceDef>,
    /// 3D character models.
    pub models: Vec<ModelDef>,
    /// Ground materials (the painted ground of maps).
    pub materials: Vec<crate::ground::MaterialDef>,
    /// Scatter sets (the editor's scatter brush: forests, flowers).
    pub scatter: Vec<crate::decor::ScatterDef>,
    /// Fonts (`ProjectConfig::fonts` picks which the game uses).
    pub fonts: Vec<crate::fonts::FontDef>,
    /// UI skins (`ProjectConfig::skin` picks the one the game uses).
    pub skins: Vec<crate::skin::SkinDef>,
    /// Common events: named action lists (*Call common event*, parallel / autorun ones).
    pub common_events: Vec<crate::common_events::CommonEventDef>,
    /// Backgrounds the character creator offers (the SRD's Acolyte, a project's own).
    pub backgrounds: Vec<crate::backgrounds::BackgroundDef>,
    /// Classes (the SRD's twelve, a project's own): the hero's and party members'.
    pub classes: Vec<crate::classes::ClassDef>,
    /// Id → position caches of the hot lists ([`Database::reindex`]); never wrong, see [`DefIndex`].
    #[doc(hidden)]
    pub index: DefIndex,
}

/// A multi-tile asset ("structure"): tiles on several map layers, collision overrides and
/// entities that the editor places on a map in one click (a house, a wall segment, a room…).
/// Placing it writes ordinary tiles and entities, so the game never needs it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StructureDef {
    pub id: String,
    pub name: String,
    /// Group in the editor's palette ("Buildings", "Walls", "Rooms", "Nature", …).
    #[serde(default)]
    pub category: String,
    /// Size in tiles.
    pub w: u32,
    pub h: u32,
    /// Tile structures (R(ust)PG2D 0.20 and older; converted when the project is opened): the
    /// tilesets their tiles come from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tilesets: Vec<String>,
    /// Tile structures: tiles per old map layer (0 = ground, 1 = detail, 2 = overhead), row by
    /// row (`w × h`): 0 = keep the map's tile, `StructureDef::CLEAR` = empty it, else a tile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<Vec<u32>>,
    /// Collision override per tile (`CollisionOverride` as a byte; 0 = keep the map's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collision: Vec<u8>,
    /// Entities placed with it (doors, chests, signs…); positions are relative to the
    /// structure's top-left corner (uids are assigned when placing).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<crate::entity::EntityInstance>,
    /// Material ids of the painted ground; a `ground` cell holds its slot here plus one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<String>,
    /// Ground cells (`ground::CELLS` per tile each way, row by row): 0 = keep the map's,
    /// `StructureDef::CLEAR_GROUND` = no ground, else a `materials` slot plus one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ground: Vec<u8>,
    /// Prop ids of the decorations; `Decor::def` indexes into this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decor_defs: Vec<String>,
    /// Decorations, feet positions relative to the structure's top-left corner (pixels).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decor: Vec<crate::decor::Decor>,
    /// Per tile: 1 = placing removes the map's decorations standing there (row by row).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear: Vec<u8>,
}

/// Id → position caches of the lists the game looks up per entity per frame (items, props,
/// characters, sheets). A hit is verified (`list[i].id == id`) and a miss or a stale
/// entry falls back to the linear search, so a list edited in place (the editor's forms, the
/// agent server) only makes lookups slower until [`Database::reindex`] runs, never wrong.
#[derive(Clone, Default)]
pub struct DefIndex {
    sheets: std::collections::HashMap<String, usize>,
    items: std::collections::HashMap<String, usize>,
    props: std::collections::HashMap<String, usize>,
    characters: std::collections::HashMap<String, usize>,
}

/// A cache is not content: two databases with the same definitions are equal.
impl PartialEq for DefIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for DefIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DefIndex({} ids)", self.sheets.len() + self.items.len() + self.props.len() + self.characters.len())
    }
}

impl Database {
    /// Rebuilds the id caches after the lists changed (loading, translating, the editor's forms,
    /// the agent server). Lookups stay correct without it; this keeps them fast.
    pub fn reindex(&mut self) {
        fn build<T: Def>(list: &[T]) -> std::collections::HashMap<String, usize> {
            let mut m = std::collections::HashMap::with_capacity(list.len());
            for (i, d) in list.iter().enumerate() {
                m.entry(d.id().to_string()).or_insert(i); // the first of a duplicated id, like `find`
            }
            m
        }
        self.index = DefIndex { sheets: build(&self.sheets), items: build(&self.items), props: build(&self.props), characters: build(&self.characters) };
    }

    fn lookup<'a, T: Def>(list: &'a [T], index: &std::collections::HashMap<String, usize>, id: &str) -> Option<&'a T> {
        if let Some(d) = index.get(id).and_then(|&i| list.get(i)) {
            if d.id() == id {
                return Some(d);
            }
        }
        list.iter().find(|d| d.id() == id)
    }

    pub fn sheet(&self, id: &str) -> Option<&SheetDef> {
        Self::lookup(&self.sheets, &self.index.sheets, id)
    }
    /// Item id -> tool kind of every tool item (`ItemDef::tool`).
    pub fn tool_kinds(&self) -> crate::crafting::ToolKinds {
        self.items.iter().filter(|d| !d.tool.is_empty()).map(|d| (d.id.clone(), d.tool.clone())).collect()
    }

    pub fn item(&self, id: &str) -> Option<&ItemDef> {
        Self::lookup(&self.items, &self.index.items, id)
    }
    pub fn prop(&self, id: &str) -> Option<&PropDef> {
        Self::lookup(&self.props, &self.index.props, id)
    }
    pub fn character(&self, id: &str) -> Option<&CharacterDef> {
        Self::lookup(&self.characters, &self.index.characters, id)
    }
    pub fn quest(&self, id: &str) -> Option<&crate::quest::QuestDef> {
        self.quests.iter().find(|d| d.id == id)
    }
    pub fn sound(&self, id: &str) -> Option<&SoundDef> {
        self.sounds.iter().find(|d| d.id == id)
    }
    pub fn structure(&self, id: &str) -> Option<&StructureDef> {
        self.structures.iter().find(|d| d.id == id)
    }
    pub fn spell(&self, id: &str) -> Option<&crate::skills::SpellDef> {
        self.spells.iter().find(|d| d.id == id)
    }
    pub fn scatter_set(&self, id: &str) -> Option<&crate::decor::ScatterDef> {
        self.scatter.iter().find(|d| d.id == id)
    }
    pub fn font(&self, id: &str) -> Option<&crate::fonts::FontDef> {
        self.fonts.iter().find(|d| d.id == id)
    }
    pub fn skin(&self, id: &str) -> Option<&crate::skin::SkinDef> {
        self.skins.iter().find(|d| d.id == id)
    }
    pub fn common_event(&self, id: &str) -> Option<&crate::common_events::CommonEventDef> {
        self.common_events.iter().find(|d| d.id == id)
    }

    pub fn material(&self, id: &str) -> Option<&crate::ground::MaterialDef> {
        self.materials.iter().find(|d| d.id == id)
    }

    pub fn achievement(&self, id: &str) -> Option<&crate::achievements::AchievementDef> {
        self.achievements.iter().find(|d| d.id == id)
    }
    pub fn recipe(&self, id: &str) -> Option<&crate::crafting::RecipeDef> {
        self.recipes.iter().find(|d| d.id == id)
    }

    pub fn race(&self, id: &str) -> Option<&crate::races::RaceDef> {
        self.races.iter().find(|d| d.id == id)
    }

    /// A race with its parent race folded in (a subrace's whole SRD data, [`RaceDef::merged`]).
    pub fn race_full(&self, id: &str) -> Option<crate::races::RaceDef> {
        let r = self.race(id)?;
        Some(r.merged(self.race(&r.parent)))
    }

    pub fn background(&self, id: &str) -> Option<&crate::backgrounds::BackgroundDef> {
        self.backgrounds.iter().find(|d| d.id == id)
    }

    pub fn class(&self, id: &str) -> Option<&crate::classes::ClassDef> {
        self.classes.iter().find(|d| d.id == id)
    }

    pub fn model(&self, id: &str) -> Option<&ModelDef> {
        self.models.iter().find(|d| d.id == id)
    }

    /// A character definition's faction: its own, else its race's ("" = none).
    pub fn faction_of(&self, character: &str) -> String {
        let Some(c) = self.character(character) else { return String::new() };
        if !c.faction.is_empty() {
            return c.faction.clone();
        }
        self.race(&c.race).map(|r| r.faction.clone()).unwrap_or_default()
    }

    /// The race whose bodies include `body` (the first one).
    pub fn race_of_body(&self, body: &str) -> Option<&crate::races::RaceDef> {
        (!body.is_empty()).then(|| self.races.iter().find(|r| r.bodies_male.iter().chain(&r.bodies_female).any(|b| b == body))).flatten()
    }

    /// Size of characters with this body (its race's; 1 without one).
    pub fn body_size(&self, body: &str) -> f32 {
        self.race_of_body(body).map_or(1.0, |r| r.size.clamp(0.25, 4.0))
    }
    /// Recipes of a crafting station, in definition order.
    pub fn recipes_at<'a>(&'a self, station: &'a str) -> impl Iterator<Item = &'a crate::crafting::RecipeDef> + 'a {
        self.recipes.iter().filter(move |r| r.station == station)
    }

    /// Ids of all definitions of a kind (for editor dropdowns).
    pub fn ids(&self, kind: EntityKind) -> Vec<String> {
        match kind {
            EntityKind::Prop => self.props.iter().map(|d| d.id.clone()).collect(),
            EntityKind::Character => self.characters.iter().map(|d| d.id.clone()).collect(),
            EntityKind::Item => self.items.iter().map(|d| d.id.clone()).collect(),
        }
    }

    /// Triggers defined on the definition behind an entity.
    /// Max stack sizes of all items (installed into inventories).
    pub fn stack_limits(&self) -> std::sync::Arc<std::collections::HashMap<String, u32>> {
        std::sync::Arc::new(self.items.iter().map(|d| (d.id.clone(), d.max_stack.max(1))).collect())
    }

    /// The container of a placed entity: its own override, else the prop definition's.
    pub fn container_of<'a>(&'a self, e: &'a crate::entity::EntityInstance) -> Option<&'a ContainerSpec> {
        if e.kind != EntityKind::Prop {
            return None;
        }
        e.container.as_ref().or_else(|| self.prop(&e.def).and_then(|d| d.container.as_ref()))
    }

    /// The merchant spec of a placed character (instance override or its definition's).
    pub fn shop_of<'a>(&'a self, e: &'a crate::entity::EntityInstance) -> Option<&'a ShopSpec> {
        if e.kind != EntityKind::Character {
            return None;
        }
        e.shop.as_ref().or_else(|| self.character(&e.def).and_then(|d| d.shop.as_ref()))
    }

    /// Converts legacy frame offsets in all prop definitions to named states.
    pub fn upgrade_prop_states(&mut self) {
        for d in &mut self.props {
            d.upgrade_states();
        }
    }

    /// Converts a placed prop's legacy frame offsets (start frame, lock, container) to the
    /// definition's named states, where one matches.
    pub fn upgrade_instance(&self, e: &mut crate::entity::EntityInstance) {
        let Some(d) = (e.kind == EntityKind::Prop).then(|| self.prop(&e.def)).flatten() else { return };
        if e.frame > 0 && e.state.is_empty() {
            if let Some(st) = d.state_of_offset(e.frame) {
                e.state = st;
                e.frame = 0;
            }
        }
        if let Some(l) = &mut e.lock {
            if let OnUnlock::Open { frame } = l.on_unlock {
                if let Some(st) = d.state_of_offset(frame) {
                    l.on_unlock = OnUnlock::ToState { state: st };
                }
            }
        }
        if let Some(c) = &mut e.container {
            if c.open_frame > 0 && c.open_state.is_empty() {
                if let Some(st) = d.state_of_offset(c.open_frame) {
                    c.open_state = st;
                    c.open_frame = 0;
                }
            }
        }
    }

    /// The lock of a placed entity: its own override, else the prop definition's.
    pub fn lock_of<'a>(&'a self, e: &'a crate::entity::EntityInstance) -> Option<&'a LockSpec> {
        if e.kind != EntityKind::Prop {
            return None;
        }
        e.lock.as_ref().or_else(|| self.prop(&e.def).and_then(|d| d.lock.as_ref()))
    }

    pub fn def_triggers(&self, kind: EntityKind, id: &str) -> &[Trigger] {
        match kind {
            EntityKind::Prop => self.prop(id).map(|d| &d.triggers[..]),
            EntityKind::Character => self.character(id).map(|d| &d.triggers[..]),
            EntityKind::Item => None,
        }
        .unwrap_or(&[])
    }
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// What every definition list shares: an id and its list in the [`Database`]. [`with_def_kind!`]
/// picks the type of a [`RefKind`](crate::refactor::RefKind) at run time, so one generic function
/// serves all twelve lists (the library, the agent server, renames).
pub trait Def: Clone + PartialEq + std::fmt::Debug + Serialize + serde::de::DeserializeOwned + 'static {
    const KIND: crate::refactor::RefKind;
    fn id(&self) -> &str;
    fn id_mut(&mut self) -> &mut String;
    fn list(db: &Database) -> &Vec<Self>;
    fn list_mut(db: &mut Database) -> &mut Vec<Self>;
    /// Other definitions this one refers to (library dependencies); none by default.
    fn refs(&self) -> Vec<(crate::refactor::RefKind, String)> {
        Vec::new()
    }
}

macro_rules! impl_def {
    ($t:ty, $kind:expr, $field:ident $(, refs = $refs:expr)?) => {
        impl Def for $t {
            const KIND: crate::refactor::RefKind = $kind;
            fn id(&self) -> &str {
                &self.id
            }
            fn id_mut(&mut self) -> &mut String {
                &mut self.id
            }
            fn list(db: &Database) -> &Vec<Self> {
                &db.$field
            }
            fn list_mut(db: &mut Database) -> &mut Vec<Self> {
                &mut db.$field
            }
            $(fn refs(&self) -> Vec<(crate::refactor::RefKind, String)> {
                $refs(self)
            })?
        }
    };
}

impl_def!(SheetDef, crate::refactor::RefKind::Sheet, sheets);
impl_def!(ItemDef, crate::refactor::RefKind::Item, items, refs = crate::refactor::refs_of_item);
impl_def!(PropDef, crate::refactor::RefKind::Prop, props, refs = crate::refactor::refs_of_prop);
impl_def!(CharacterDef, crate::refactor::RefKind::Character, characters, refs = crate::refactor::refs_of_character);
impl_def!(SoundDef, crate::refactor::RefKind::Sound, sounds);
impl_def!(crate::quest::QuestDef, crate::refactor::RefKind::Quest, quests, refs = crate::refactor::refs_of_quest);
impl_def!(StructureDef, crate::refactor::RefKind::Structure, structures, refs = crate::refactor::refs_of_structure);
impl_def!(crate::skills::SpellDef, crate::refactor::RefKind::Spell, spells, refs = crate::refactor::refs_of_spell);
impl_def!(crate::crafting::RecipeDef, crate::refactor::RefKind::Recipe, recipes, refs = crate::refactor::refs_of_recipe);
impl_def!(crate::achievements::AchievementDef, crate::refactor::RefKind::Achievement, achievements);
impl_def!(crate::races::RaceDef, crate::refactor::RefKind::Race, races, refs = crate::refactor::refs_of_race);
impl_def!(ModelDef, crate::refactor::RefKind::Model, models, refs = crate::refactor::refs_of_model);
impl_def!(crate::ground::MaterialDef, crate::refactor::RefKind::Material, materials, refs = crate::refactor::refs_of_material);
impl_def!(crate::decor::ScatterDef, crate::refactor::RefKind::Scatter, scatter, refs = crate::refactor::refs_of_scatter);
impl_def!(crate::fonts::FontDef, crate::refactor::RefKind::Font, fonts);
impl_def!(crate::skin::SkinDef, crate::refactor::RefKind::Skin, skins, refs = crate::refactor::refs_of_skin);
impl_def!(crate::common_events::CommonEventDef, crate::refactor::RefKind::CommonEvent, common_events, refs = crate::refactor::refs_of_common_event);
impl_def!(crate::backgrounds::BackgroundDef, crate::refactor::RefKind::Background, backgrounds, refs = crate::refactor::refs_of_background);
impl_def!(crate::classes::ClassDef, crate::refactor::RefKind::Class, classes, refs = crate::refactor::refs_of_class);

/// Runs `$body` with `$T` bound to the definition type of `$kind` (a
/// [`RefKind`](crate::refactor::RefKind)); `$else` for `Map` and `Skill` (a proficiency id), which
/// are no definitions.
///
/// ```ignore
/// let n = with_def_kind!(kind, |T| T::list(&db).len(), else => 0);
/// ```
#[macro_export]
macro_rules! with_def_kind {
    ($kind:expr, |$T:ident| $body:expr, else => $else:expr) => {
        match $kind {
            $crate::refactor::RefKind::Sheet => {
                type $T = $crate::defs::SheetDef;
                $body
            }
            $crate::refactor::RefKind::Item => {
                type $T = $crate::defs::ItemDef;
                $body
            }
            $crate::refactor::RefKind::Prop => {
                type $T = $crate::defs::PropDef;
                $body
            }
            $crate::refactor::RefKind::Character => {
                type $T = $crate::defs::CharacterDef;
                $body
            }
            $crate::refactor::RefKind::Sound => {
                type $T = $crate::defs::SoundDef;
                $body
            }
            $crate::refactor::RefKind::Quest => {
                type $T = $crate::quest::QuestDef;
                $body
            }
            $crate::refactor::RefKind::Structure => {
                type $T = $crate::defs::StructureDef;
                $body
            }
            $crate::refactor::RefKind::Spell => {
                type $T = $crate::skills::SpellDef;
                $body
            }
            $crate::refactor::RefKind::Recipe => {
                type $T = $crate::crafting::RecipeDef;
                $body
            }
            $crate::refactor::RefKind::Achievement => {
                type $T = $crate::achievements::AchievementDef;
                $body
            }
            $crate::refactor::RefKind::Race => {
                type $T = $crate::races::RaceDef;
                $body
            }
            $crate::refactor::RefKind::Model => {
                type $T = $crate::defs::ModelDef;
                $body
            }
            $crate::refactor::RefKind::Material => {
                type $T = $crate::ground::MaterialDef;
                $body
            }
            $crate::refactor::RefKind::Scatter => {
                type $T = $crate::decor::ScatterDef;
                $body
            }
            $crate::refactor::RefKind::Font => {
                type $T = $crate::fonts::FontDef;
                $body
            }
            $crate::refactor::RefKind::Skin => {
                type $T = $crate::skin::SkinDef;
                $body
            }
            $crate::refactor::RefKind::CommonEvent => {
                type $T = $crate::common_events::CommonEventDef;
                $body
            }
            $crate::refactor::RefKind::Background => {
                type $T = $crate::backgrounds::BackgroundDef;
                $body
            }
            $crate::refactor::RefKind::Class => {
                type $T = $crate::classes::ClassDef;
                $body
            }
            $crate::refactor::RefKind::Skill | $crate::refactor::RefKind::Map => $else,
        }
    };
}

/// The lists by kind: generic over a [`Def`] type, or by a [`RefKind`](crate::refactor::RefKind)
/// known at run time.
impl Database {
    pub fn get<T: Def>(&self, id: &str) -> Option<&T> {
        T::list(self).iter().find(|d| d.id() == id)
    }

    pub fn get_mut<T: Def>(&mut self, id: &str) -> Option<&mut T> {
        T::list_mut(self).iter_mut().find(|d| d.id() == id)
    }

    pub fn position<T: Def>(&self, id: &str) -> Option<usize> {
        T::list(self).iter().position(|d| d.id() == id)
    }

    /// Puts `def` in: over the one with its id, or at the end. True when it replaced one.
    pub fn upsert<T: Def>(&mut self, def: T) -> bool {
        let list = T::list_mut(self);
        let replaced = match list.iter().position(|d| d.id() == def.id()) {
            Some(i) => {
                list[i] = def;
                true
            }
            None => {
                list.push(def);
                false
            }
        };
        self.reindex();
        replaced
    }

    /// Removes the definition `id`; true when there was one.
    pub fn remove<T: Def>(&mut self, id: &str) -> bool {
        let list = T::list_mut(self);
        let n = list.len();
        list.retain(|d| d.id() != id);
        let removed = list.len() != n;
        self.reindex();
        removed
    }

    pub fn has(&self, kind: crate::refactor::RefKind, id: &str) -> bool {
        crate::with_def_kind!(kind, |T| self.get::<T>(id).is_some(), else => false)
    }

    pub fn ids_of(&self, kind: crate::refactor::RefKind) -> Vec<String> {
        crate::with_def_kind!(kind, |T| T::list(self).iter().map(|d| d.id().to_string()).collect(), else => Vec::new())
    }

    pub fn index_of(&self, kind: crate::refactor::RefKind, id: &str) -> Option<usize> {
        crate::with_def_kind!(kind, |T| self.position::<T>(id), else => None)
    }

    /// Renames the definition `old` (only it: [`crate::refactor::rename_in_db`] rewrites the
    /// references). True when there was one.
    pub fn rename(&mut self, kind: crate::refactor::RefKind, old: &str, new: &str) -> bool {
        let renamed = crate::with_def_kind!(
            kind,
            |T| match self.get_mut::<T>(old) {
                Some(d) => {
                    *d.id_mut() = new.to_string();
                    true
                }
                None => false,
            },
            else => false
        );
        self.reindex();
        renamed
    }

    /// Do `self` and `other` define `id` the same way (both missing counts as the same)?
    pub fn same(&self, other: &Database, kind: crate::refactor::RefKind, id: &str) -> bool {
        crate::with_def_kind!(kind, |T| self.get::<T>(id) == other.get::<T>(id), else => true)
    }

    /// Copies `from`'s definition of `id` in (over the one with the same id, or at the end).
    /// True when `from` has it.
    pub fn copy_from(&mut self, from: &Database, kind: crate::refactor::RefKind, id: &str) -> bool {
        crate::with_def_kind!(
            kind,
            |T| match from.get::<T>(id) {
                Some(d) => {
                    self.upsert(d.clone());
                    true
                }
                None => false,
            },
            else => false
        )
    }

    /// The definitions `id` refers to directly (none for kinds without references, or an unknown id).
    pub fn refs_of(&self, kind: crate::refactor::RefKind, id: &str) -> Vec<(crate::refactor::RefKind, String)> {
        crate::with_def_kind!(kind, |T| self.get::<T>(id).map(|d| d.refs()).unwrap_or_default(), else => Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_name_their_animations_or_go_by_their_kind() {
        let item = |w: &str| ron::from_str::<ItemDef>(&format!("(id: \"x\", name: \"X\", icon: (sheet: \"s\", frame: 0), {w})")).unwrap();
        let weapon = |extra: &str| item(&format!("weapon: Some((damage: 2, cooldown: 0.5, arc: 60.0, knockback: 1.0{extra}))"));
        // Older files have no `anims`: estimated by the kind of weapon.
        let sword = weapon(", reach: 1.0");
        assert!(sword.anims.is_empty());
        assert_eq!(ItemAnims::of(&sword), ItemAnims { attack: "attack_1h".into(), idle: "idle_1h".into(), walk: String::new(), block: "block_weapon".into() });
        let spear = weapon(", reach: 1.9, two_handed: true");
        assert_eq!((ItemAnims::of(&spear).attack.as_str(), ItemAnims::of(&spear).idle.as_str()), ("attack_thrust", "idle_spear"));
        assert_eq!(ItemAnims::of(&weapon(", reach: 1.0, two_handed: true")).attack, "attack_2h");
        let bow = weapon(", reach: 1.0, ranged: Some((projectile: (sheet: \"s\", frame: 1), speed: 400.0, range: 8.0, ammo: \"arrows\"))");
        assert_eq!((ItemAnims::of(&bow).attack.as_str(), ItemAnims::of(&bow).idle.as_str()), ("attack_bow", "idle_bow"));
        let crossbow = ItemDef { name: "Heavy Crossbow".into(), ..bow.clone() };
        assert_eq!(ItemAnims::of(&crossbow).attack, "attack_crossbow");
        // A shield blocks; a lantern in the left hand (no defense) doesn't.
        assert_eq!(ItemAnims::of(&item("armor: Some((slot: Offhand, defense: 1))")).block, "block_shield");
        assert!(ItemAnims::of(&item("armor: Some((slot: Offhand))")).is_empty());
        // Own names win, the empty ones are estimated; only the given ones are written.
        let own = ItemDef { anims: ItemAnims { attack: "attack_2h".into(), ..Default::default() }, ..sword };
        assert_eq!((ItemAnims::of(&own).attack.as_str(), ItemAnims::of(&own).idle.as_str()), ("attack_2h", "idle_1h"));
        let text = ron::to_string(&own).unwrap();
        assert!(text.contains("anims:(attack:\"attack_2h\")"), "{text}");
        assert_eq!(ron::from_str::<ItemDef>(&text).unwrap().anims, own.anims);
        assert!(!ron::to_string(&bow).unwrap().contains("anims"));
    }

    #[test]
    fn clip_hit_points_are_optional() {
        let old: ClipMap = ron::from_str(r#"(anim: "attack", clip: "Slash", fps: 12.0)"#).unwrap();
        assert_eq!((old.hit, old.hit_point()), (None, DEFAULT_HIT));
        assert!(!ron::to_string(&old).unwrap().contains("hit"));
        let late = ClipMap { hit: Some(0.84), ..old.clone() };
        assert_eq!(ron::from_str::<ClipMap>(&ron::to_string(&late).unwrap()).unwrap().hit, Some(0.84));
        // Kept inside the clip.
        assert_eq!(ClipMap { hit: Some(1.5), ..old }.hit_point(), 0.95);
        // Shared clip files load from older files as none, and count as the model's files.
        let m: ModelDef = ron::from_str(r#"(id: "m", file: "models/m.glb", clip_files: ["models/a.glb"])"#).unwrap();
        assert!(m.shared_clips.is_empty());
        let m = ModelDef { shared_clips: vec!["models/b.glb".into()], ..m };
        assert_eq!(m.clip_paths().collect::<Vec<_>>(), vec!["models/a.glb", "models/b.glb"]);
    }

    #[test]
    fn old_chase_behavior_loads() {
        let b: Behavior = ron::from_str("Chase(sight: 4.0)").unwrap();
        assert_eq!(b, Behavior::chase(4.0));
    }

    #[test]
    fn loot_rolls() {
        let table = vec![
            LootEntry::new("coin", 100.0, 1, 3),
            LootEntry::new("gem", 0.0, 1, 1),
            LootEntry::new("coin", 50.0, 2, 2),
            LootEntry::new("", 100.0, 1, 1),
        ];
        // rand01 = 0.4: 100% and 50% entries drop, 0% never; amounts come from `range`.
        let drops = roll_loot(&table, || 0.4, |lo, hi| hi.max(lo));
        assert_eq!(drops, vec![("coin".to_string(), 5)]);
        // rand01 = 0.6: only the 100% entry.
        assert_eq!(roll_loot(&table, || 0.6, |lo, _| lo), vec![("coin".to_string(), 1)]);
        // Old data without min / max: one item.
        let e: LootEntry = ron::from_str("(item: \"apple\", chance: 25.0)").unwrap();
        assert_eq!((e.min, e.max), (1, 1));
    }

    /// The id caches speed lookups up and can never return the wrong definition: lists edited
    /// in place without `reindex` still resolve correctly (through the linear fallback).
    #[test]
    fn index_is_never_wrong() {
        let mut db = Database::default();
        for id in ["a", "b", "c"] {
            db.items.push(ItemDef::fresh(id));
        }
        db.reindex();
        assert_eq!(db.item("b").map(|d| d.id.as_str()), Some("b"));
        // Edited in place: swapped, renamed, one removed, one added.
        db.items.swap(0, 2);
        db.items[1].id = "bee".into();
        db.items.remove(0);
        db.items.push(ItemDef::fresh("d"));
        assert_eq!(db.items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["bee", "a", "d"]);
        assert!(db.item("c").is_none() && db.item("b").is_none());
        assert_eq!(db.item("bee").map(|d| d.id.as_str()), Some("bee"));
        assert_eq!(db.item("d").map(|d| d.id.as_str()), Some("d"));
        db.reindex();
        assert_eq!(db.item("a").map(|d| d.id.as_str()), Some("a"));
        // Equality ignores the cache.
        let mut other = db.clone();
        other.index = DefIndex::default();
        assert_eq!(db, other);
    }

    /// One generic path to every definition list, by type or by kind.
    #[test]
    fn database_by_kind() {
        use crate::refactor::RefKind;
        let mut db = Database::default();
        db.items.push(ItemDef::fresh("sword"));
        db.props.push(PropDef::fresh("chest"));
        assert!(db.has(RefKind::Item, "sword") && !db.has(RefKind::Item, "chest") && !db.has(RefKind::Map, "village"));
        assert_eq!(db.ids_of(RefKind::Prop), ["chest"]);
        assert_eq!(db.index_of(RefKind::Item, "sword"), Some(0));
        assert_eq!(db.get::<ItemDef>("sword").map(|d| d.name.as_str()), Some("New item"));
        assert!(db.rename(RefKind::Item, "sword", "blade") && !db.rename(RefKind::Item, "sword", "x"));
        assert_eq!(db.ids_of(RefKind::Item), ["blade"]);
        let mut other = Database::default();
        assert!(!other.same(&db, RefKind::Item, "blade") && other.same(&db, RefKind::Item, "nothing"));
        assert!(other.copy_from(&db, RefKind::Prop, "chest") && !other.copy_from(&db, RefKind::Prop, "nothing"));
        assert!(other.same(&db, RefKind::Prop, "chest"));
        assert!(other.upsert(PropDef { name: "Box".into(), ..PropDef::fresh("chest") }));
        assert_eq!(other.props.len(), 1);
        assert!(other.remove::<PropDef>("chest") && other.props.is_empty());
        assert_eq!(<ItemDef as Def>::KIND, RefKind::Item);
        assert!(RefKind::DEFS.iter().all(|k| db.ids_of(*k).len() <= 1));
    }

    /// Sheets of older files are 1x; 1x isn't written out, 2x round-trips.
    #[test]
    fn sheet_scale() {
        let s: SheetDef = ron::from_str(r#"(id: "s", image: "sprites/s.png", frame_w: 32, frame_h: 48)"#).unwrap();
        assert_eq!(s.scale, 1);
        assert!(!ron::to_string(&s).unwrap().contains("scale"));
        let hd = SheetDef { scale: 2, ..SheetDef::new("hd", "sprites/hd.png", 64, 96) };
        let back: SheetDef = ron::from_str(&ron::to_string(&hd).unwrap()).unwrap();
        assert_eq!(back, hd);
    }

    /// Weapons of older files hit critically 5 % of the time.
    #[test]
    fn weapon_crit_default() {
        let w: Weapon = ron::from_str("(damage: 2)").unwrap();
        assert_eq!(w.crit, 0.05);
        assert_eq!(w, Weapon::melee(2, 0.4));
    }

    /// Chasers of older files fight as a group and alert their kind within 6 tiles, like
    /// `Behavior::chase`; both can be switched off.
    #[test]
    fn chase_group_defaults() {
        let b: Behavior = ron::from_str("Chase(sight: 4.0)").unwrap();
        assert_eq!(b, Behavior::chase(4.0));
        let Behavior::Chase { group, alert, .. } = b else { panic!() };
        assert!(group && alert == 6.0);
        let b: Behavior = ron::from_str("Chase(sight: 4.0, group: false, alert: 0.0)").unwrap();
        assert!(matches!(b, Behavior::Chase { group: false, alert: 0.0, .. }));
    }

    /// `Default` is what a file entry with only the required fields loads as (so tests and the
    /// editor can write `..Default::default()`), and `fresh` is the editor's "+ New" value.
    #[test]
    fn defaults_match_the_file_format() {
        let s: SheetDef = ron::from_str(r#"(id: "s", image: "sprites/s.png", frame_w: 32, frame_h: 32)"#).unwrap();
        assert_eq!(s, SheetDef::new("s", "sprites/s.png", 32, 32));
        assert_eq!(s, SheetDef { id: "s".into(), image: "sprites/s.png".into(), ..Default::default() });
        let i: ItemDef = ron::from_str(r#"(id: "i", name: "I", icon: (sheet: "s", frame: 1))"#).unwrap();
        assert_eq!(i, ItemDef { id: "i".into(), name: "I".into(), icon: SpriteRef::new("s", 1), ..Default::default() });
        assert_eq!(i.max_stack, crate::inventory::DEFAULT_STACK);
        let p: PropDef = ron::from_str(r#"(id: "p", name: "P", sprite: (sheet: "s", frame: 2))"#).unwrap();
        assert_eq!(p, PropDef { id: "p".into(), name: "P".into(), sprite: SpriteRef::new("s", 2), ..Default::default() });
        assert!(!p.solid && p.hitbox == [24.0, 16.0] && p.default_state == "default");
        let c: CharacterDef = ron::from_str(r#"(id: "c", name: "C")"#).unwrap();
        assert_eq!(c, CharacterDef { id: "c".into(), name: "C".into(), ..Default::default() });
        assert!(c.speed == 120.0 && c.behavior == Behavior::Static && c.hitbox == [24.0, 16.0]);
        // The editor's new definitions.
        let p = PropDef::fresh("x");
        assert!(p.solid && p.hitbox == [28.0, 16.0] && p.name == "New prop" && p.id == "x");
        assert_eq!((ItemDef::fresh("x").name.as_str(), CharacterDef::fresh("x").name.as_str()), ("New item", "New character"));
        assert_eq!(SheetDef::fresh("x").image.as_str(), "sprites/new.png");
    }

    #[test]
    fn shop_prices() {
        let mut db = Database::default();
        for (id, value) in [("apple", 3), ("rock", 0), ("coin", 1)] {
            db.items.push(ItemDef { id: id.into(), name: id.into(), value, ..Default::default() });
        }
        let spec = ShopSpec { items: vec![ShopEntry::new("apple", 2, 10)], ..Default::default() };
        // Own price wins over the item's value; unlisted items use their value.
        assert_eq!(ShopSpec::buy_price(&spec.items, "apple", &db), 10);
        assert_eq!(ShopSpec::buy_price(&[], "apple", &db), 3);
        // Sells at 50 %, at least 1; worthless items and the currency can't be sold.
        assert_eq!(spec.sell_price(&spec.items, "apple", &db), Some(5));
        assert_eq!(spec.sell_price(&[], "apple", &db), Some(1));
        assert_eq!(spec.sell_price(&[], "rock", &db), None);
        assert_eq!(spec.sell_price(&[], "coin", &db), None);
        let closed = ShopSpec { sell_rate: 0, ..spec.clone() };
        assert_eq!(closed.sell_price(&spec.items, "apple", &db), None);
        // Older files without the new fields still load.
        let old: ShopSpec = ron::from_str("(items: [(item: \"apple\")])").unwrap();
        assert_eq!((old.currency.as_str(), old.sell_rate, old.open_on_talk, old.items[0].count), ("coin", 50, true, 1));
    }

    #[test]
    fn multi_cell_sprites() {
        // Older definitions have no size: single cell.
        let old: SpriteRef = ron::from_str(r#"(sheet: "props", frame: 3)"#).unwrap();
        assert_eq!(old.cells(), (1, 1));
        let big = SpriteRef::block("props", 24, 3, 4);
        let s = ron::to_string(&big).unwrap();
        assert_eq!(ron::from_str::<SpriteRef>(&s).unwrap(), big);
        // Single-cell sprites don't write the size fields.
        assert!(!ron::to_string(&SpriteRef::new("items", 1)).unwrap().contains("w:"));
        assert_eq!(SpriteRef::default().cells(), (1, 1));
    }

    #[test]
    fn weapons_and_armor() {
        // Weapons saved before melee fields existed get sensible defaults.
        let w: Weapon = ron::from_str("(damage: 2, cooldown: 0.5)").unwrap();
        assert_eq!((w.reach, w.arc, w.knockback), (1.0, 60.0, 1.0));
        let mut item: ItemDef = ron::from_str(r#"(id: "cap", name: "Cap", icon: (sheet: "items", frame: 0))"#).unwrap();
        assert_eq!(item.equip_slot(), None);
        assert_eq!(item.max_stack, crate::inventory::DEFAULT_STACK);
        item.armor = Some(Armor { slot: EquipSlot::Head, defense: 1, speed: 0.0, resistances: Vec::new() });
        assert_eq!(item.equip_slot(), Some(EquipSlot::Head));
        item.weapon = Some(Weapon::melee(1, 0.3));
        assert_eq!(item.equip_slot(), Some(EquipSlot::Weapon));
    }

    #[test]
    fn relockable_locks() {
        // Locks saved before relocking existed can't be relocked.
        let old: LockSpec = ron::from_str(r#"(key: "key", on_unlock: Open(frame: 1))"#).unwrap();
        assert!(old.consume_key && !old.relock && !old.can_relock());
        let gate = LockSpec { relock: true, ..old.clone() };
        assert!(gate.can_relock());
        // Props that disappear when unlocked can't come back.
        assert!(!LockSpec { on_unlock: OnUnlock::Disappear, ..gate }.can_relock());
    }

    #[test]
    fn prop_states_and_legacy_offsets() {
        // A gate saved with the old frame-offset lock and a chest with an open frame.
        let mut gate: PropDef = ron::from_str(
            r#"(id: "gate", name: "Gate", sprite: (sheet: "props", frame: 3), solid: true,
                lock: Some((key: "key", on_unlock: Open(frame: 12))))"#,
        )
        .unwrap();
        assert_eq!(gate.state_names(), vec!["default"]);
        gate.upgrade_states();
        assert_eq!(gate.state_names(), vec!["closed", "open"]);
        assert_eq!(gate.lock.as_ref().unwrap().on_unlock, OnUnlock::ToState { state: "open".into() });
        let (sprite, solid) = gate.state_look(1);
        assert_eq!((sprite.frame, solid), (15, false));
        assert!(gate.state_look(0).1);
        assert_eq!(gate.state_index(""), Some(0));
        assert_eq!(gate.state_of_offset(12).as_deref(), Some("open"));
        assert_eq!(gate.state_of_offset(5), None);
        // Upgrading twice changes nothing.
        let once = gate.clone();
        gate.upgrade_states();
        assert_eq!(gate, once);

        let mut chest: PropDef = ron::from_str(
            r#"(id: "chest", name: "Chest", sprite: (sheet: "props", frame: 0), solid: true,
                container: Some((size: 8, open_frame: 1)))"#,
        )
        .unwrap();
        chest.upgrade_states();
        let c = chest.container.as_ref().unwrap();
        assert_eq!((c.open_state.as_str(), c.open_frame), ("open", 0));
        assert!(chest.state_look(1).1, "an open chest still blocks movement");

        // Placed instances with old offsets follow the definition's states.
        let db = Database { props: vec![gate], ..Default::default() };
        let mut e = crate::entity::EntityInstance::new(1, EntityKind::Prop, "gate", 0.0, 0.0);
        e.frame = 12;
        e.lock = Some(LockSpec { key: "k".into(), on_unlock: OnUnlock::Open { frame: 12 }, ..Default::default() });
        db.upgrade_instance(&mut e);
        assert_eq!((e.state.as_str(), e.frame), ("open", 0));
        assert_eq!(e.lock.unwrap().on_unlock, OnUnlock::ToState { state: "open".into() });
    }

    #[test]
    fn animated_props_cycle_their_frames() {
        let a = FrameAnim { frames: vec![3, 40, 41], fps: 2.0 };
        assert_eq!((a.frame_at(0.0), a.frame_at(0.6), a.frame_at(1.2), a.frame_at(1.5)), (Some(3), Some(40), Some(41), Some(3)));
        assert_eq!(FrameAnim { frames: vec![], fps: 2.0 }.frame_at(9.0), None);
        let mut p = PropDef { id: "torch".into(), sprite: SpriteRef::new("s", 3), anim: Some(a), ..Default::default() };
        p.states.push(PropState { name: "out".into(), sprite: SpriteRef::new("s", 9), solid: false, light: None });
        assert_eq!(p.sprite_at(0, 0.6).frame, 40);
        assert_eq!(p.sprite_at(1, 0.6).frame, 9, "other states don't animate");
        // Props without one don't write it.
        assert!(!ron::to_string(&PropDef::default()).unwrap().contains("anim"));
    }
}

#[cfg(test)]
mod furniture_tests {
    use super::*;

    #[test]
    fn use_spots_in_ron() {
        // Props from before furniture have none; a minimal spot gets the defaults.
        let p: PropDef = ron::from_str("(id: \"bed\", name: \"Bed\", sprite: (sheet: \"props\", frame: 0))").unwrap();
        assert!(p.use_spot.is_none());
        let s: UseSpot = ron::from_str("(pose: Lie)").unwrap();
        assert_eq!((s.pose, s.sleep_until, s.cover.sheet.as_str()), (Pose::Lie, 7.0, ""));
        assert!(!ron::to_string(&p).unwrap().contains("use_spot"));
    }
}

#[cfg(test)]
mod use_on_tests {
    use super::*;

    #[test]
    fn items_default_to_the_player() {
        let old: ItemDef = ron::from_str(r#"(id: "potion", name: "Red Potion", icon: (sheet: "items", frame: 1))"#).unwrap();
        assert_eq!(old.use_on, UseTarget::Player);
        assert!(!ron::to_string(&old).unwrap().contains("use_on"), "the default isn't written");
        let d = ItemDef { use_on: UseTarget::Choose, ..old };
        let back: ItemDef = ron::from_str(&ron::to_string(&d).unwrap()).unwrap();
        assert_eq!(back.use_on, UseTarget::Choose);
    }
}

#[cfg(test)]
mod gather_tests {
    use super::*;

    #[test]
    fn gathering_props_in_ron() {
        let old: PropDef = ron::from_str(r#"(id: "rock", name: "Rock", sprite: (sheet: "props", frame: 0))"#).unwrap();
        assert!(old.gather.is_none() && old.regrow.is_none());
        let p: PropDef = ron::from_str(r#"(id: "vein", name: "Vein", sprite: (sheet: "props", frame: 0), gather: Some((tool: "pickaxe", loot: [(item: "ore", chance: 100.0)], then_state: "mined out")), regrow: Some((from: "mined out", hours: 72.0)))"#).unwrap();
        let g = p.gather.unwrap();
        assert_eq!((g.hits, g.tool.as_str(), g.then_state.as_str()), (3, "pickaxe", "mined out"));
        assert_eq!(p.regrow.unwrap().to, "", "\"\" = back to the default state");
    }
}

#[cfg(test)]
mod crop_tests {
    use super::*;

    #[test]
    fn crop_stages_over_time() {
        let st = |h: f32| CropStage { sprite: SpriteRef::new("props", 0), hours: h };
        let c = Crop { stages: vec![st(8.0), st(16.0), st(99.0)], harvest: vec![] };
        assert_eq!(c.grow_hours(), 24.0, "the ripe stage's hours don't count");
        assert_eq!((c.stage_at(0.0), c.stage_at(7.9), c.stage_at(8.0), c.stage_at(23.9)), (Some(0), Some(0), Some(1), Some(1)));
        assert!(!c.is_ripe(23.9) && c.is_ripe(24.0) && c.is_ripe(500.0));
        assert_eq!(Crop::default().stage_at(5.0), None);
        // Old items and props have no crop / plot; a saved plot keeps its planting.
        let it: ItemDef = ron::from_str(r#"(id: "seed", name: "Seed", icon: (sheet: "items", frame: 0))"#).unwrap();
        assert!(it.crop.is_none());
        let p: PropDef = ron::from_str(r#"(id: "soil", name: "Soil", sprite: (sheet: "props", frame: 0))"#).unwrap();
        assert!(!p.plot);
        let st: crate::state::EntityState = ron::from_str(r#"(active: true, frame: 0, x: 1.0, y: 2.0, facing: Down, planted: Some((seed: "wheat_seeds", at: 30.5)))"#).unwrap();
        assert_eq!(st.planted.unwrap().bonus, 0.0);
    }
}
