//! Content definitions edited in the editor and stored as RON in `defs/`.

use serde::{Deserialize, Serialize};

use crate::entity::EntityKind;
use crate::events::{Condition, Trigger};
use crate::inventory::ItemStack;

/// A tileset image cut into 32 px squares ([`crate::project::TILE_SIZE`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TilesetDef {
    pub id: String,
    /// Image path relative to the project root.
    pub image: String,
    /// Indices of tiles that block movement.
    #[serde(default)]
    pub solid: Vec<u32>,
    /// Ladder tiles: the player climbs them (only up and down, slower, seen from behind);
    /// characters keep off them. Not solid by themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ladder: Vec<u32>,
    /// Animated tiles: wherever `tile` is placed, it cycles through `frames`.
    #[serde(default)]
    pub animations: Vec<TileAnim>,
    /// Tiles that give light wherever they are placed (wall torches, lamps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<TileLight>,
    /// Auto-tiling terrains (grass / dirt, grass / water…): the editor's Terrain tool picks
    /// their edge tiles. Editor-only; games just show the painted tiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terrains: Vec<crate::autotile::Terrain>,
}

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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileLight {
    pub tile: u32,
    pub light: LightSpec,
}

/// The serde defaults (what an entry with only `id` and `image` loads as). The editor's "+ New"
/// value is [`TilesetDef::fresh`].
impl Default for TilesetDef {
    fn default() -> Self {
        TilesetDef { id: String::new(), image: String::new(), solid: vec![], ladder: vec![], animations: vec![], lights: vec![], terrains: vec![] }
    }
}

impl TilesetDef {
    pub fn new(id: &str, image: &str) -> Self {
        TilesetDef { id: id.into(), image: image.into(), ..Default::default() }
    }

    /// A new tileset as the editor creates it.
    pub fn fresh(id: &str) -> Self {
        Self::new(id, "tilesets/new.png")
    }

    pub fn light_of(&self, tile: u32) -> Option<&LightSpec> {
        self.lights.iter().find(|l| l.tile == tile).map(|l| &l.light)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileAnim {
    pub tile: u32,
    pub frames: Vec<u32>,
    pub fps: f32,
}

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

impl TileAnim {
    /// Tile index shown at time `t` (seconds).
    pub fn frame_at(&self, t: f64) -> u32 {
        if self.frames.is_empty() || self.fps <= 0.0 {
            return self.tile;
        }
        let i = (t * self.fps as f64).floor() as usize % self.frames.len();
        self.frames[i]
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
}

fn full_volume() -> f32 {
    1.0
}

/// A sprite sheet: an image cut into equally sized frames.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetDef {
    pub id: String,
    pub image: String,
    pub frame_w: u32,
    pub frame_h: u32,
    /// What the sheet is for (offered by the matching pickers: bodies, hair styles, ...).
    #[serde(default, skip_serializing_if = "SheetKind::is_sprite")]
    pub kind: SheetKind,
    /// Bodies: a woman's body, which wears the female cut of outfits (`Armor::overlay_f`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub female: bool,
    /// Bodies: players may choose it in the character creator (people yes, a slime no).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub playable: bool,
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
        SheetDef { id: String::new(), image: String::new(), frame_w: crate::project::TILE_SIZE, frame_h: crate::project::TILE_SIZE, kind: SheetKind::Sprite, female: false, playable: false, tags: Vec::new() }
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
    /// "hair_short" -> "Short", "body_goblin_female" -> "Goblin female": the id without the kind's
    /// prefix, for pickers.
    pub fn label(&self) -> String {
        let prefix = match self.kind {
            SheetKind::Body => "body_",
            SheetKind::Hair => "hair_",
            SheetKind::Beard => "beard_",
            SheetKind::Outfit => "outfit_",
            SheetKind::Armor => "ov_",
            SheetKind::Held => "held_",
            SheetKind::Sprite => "",
        };
        let s = self.id.strip_prefix(prefix).unwrap_or(&self.id).replace('_', " ");
        let mut c = s.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    }
}

/// Role of a sprite sheet. Layered character models combine sheets in the character layout
/// (4 walk frames per row, rows = facings): a body, hair / beard, an outfit, armor and held items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetKind {
    /// Anything else: props, items, effects.
    #[default]
    Sprite,
    /// A character's body (people: in underwear, skin pixels in the magenta reference ramp;
    /// monsters: the whole creature). Any frame size; 4 walk frames per row, rows = facings.
    Body,
    /// A hair style; hair pixels use the cyan reference ramp.
    Hair,
    Beard,
    /// Clothing drawn over a body (items in the Clothing slot).
    Outfit,
    /// Armor drawn over clothing (head / body armor items).
    Armor,
    /// An item held in a hand (weapons, shields, lights).
    Held,
}

impl SheetKind {
    pub const ALL: [SheetKind; 7] = [SheetKind::Sprite, SheetKind::Body, SheetKind::Hair, SheetKind::Beard, SheetKind::Outfit, SheetKind::Armor, SheetKind::Held];
    pub fn label(self) -> &'static str {
        match self {
            SheetKind::Sprite => "Sprite",
            SheetKind::Body => "Character body",
            SheetKind::Hair => "Hair style",
            SheetKind::Beard => "Beard",
            SheetKind::Outfit => "Outfit (clothing)",
            SheetKind::Armor => "Armor overlay",
            SheetKind::Held => "Held item",
        }
    }
    fn is_sprite(&self) -> bool {
        *self == SheetKind::Sprite
    }
}

/// Look of a layered character model: a body sheet with skin and hair colors (sheets of kind
/// Body / Hair / Beard; "" = none). What it wears comes from its equipment.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub body: String,
    pub skin: [u8; 3],
    pub hair: String,
    pub hair_color: [u8; 3],
    pub beard: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { body: String::new(), skin: [232, 184, 138], hair: String::new(), hair_color: [92, 60, 34], beard: String::new() }
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
    pub damage: i32,
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
    /// A status effect for whoever the weapon hits (melee or projectile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<crate::effects::EffectSpec>,
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
    /// A melee weapon with default reach / arc / knockback.
    pub fn melee(damage: i32, cooldown: f32) -> Self {
        Weapon { damage, cooldown, ranged: None, reach: default_reach(), arc: default_arc(), knockback: default_knockback(), two_handed: false, effect: None }
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
    /// An outfit: changes how a layered character looks (armor is worn over it).
    Clothing,
    /// A tool used for an activity (a fishing rod, later pickaxes, hoes, …): held in the
    /// right hand while it is being used, the weapon otherwise.
    Tool,
}

impl EquipSlot {
    /// Slot by name as scripts write it: "clothing", "head", "body", "weapon", "offhand".
    pub fn parse(s: &str) -> Option<EquipSlot> {
        Some(match s.to_ascii_lowercase().as_str() {
            "clothing" | "clothes" | "outfit" => EquipSlot::Clothing,
            "head" | "helmet" => EquipSlot::Head,
            "body" | "armor" => EquipSlot::Body,
            "weapon" | "hand" | "right" => EquipSlot::Weapon,
            "offhand" | "left" | "left hand" | "shield" => EquipSlot::Offhand,
            "tool" => EquipSlot::Tool,
            _ => return None,
        })
    }
    pub const ALL: [EquipSlot; 6] = [EquipSlot::Clothing, EquipSlot::Head, EquipSlot::Body, EquipSlot::Weapon, EquipSlot::Offhand, EquipSlot::Tool];
    pub fn label(self) -> &'static str {
        match self {
            EquipSlot::Clothing => "Clothing",
            EquipSlot::Head => "Head",
            EquipSlot::Body => "Body",
            EquipSlot::Weapon => "Weapon",
            EquipSlot::Offhand => "Left hand",
            EquipSlot::Tool => "Tool",
        }
    }
}

/// Clothing, helmets, body armor and left-hand items (shields, lanterns, torches).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Armor {
    /// `Clothing`, `Head`, `Body` or `Offhand`.
    pub slot: EquipSlot,
    /// Subtracted from incoming damage (at least 1 damage always gets through).
    #[serde(default)]
    pub defense: i32,
    /// Movement speed change, e.g. -0.1 = 10% slower.
    #[serde(default)]
    pub speed: f32,
    /// Sprite sheet drawn over the character (character layout): the outfit of clothing, the
    /// armor of head / body pieces; "" = none.
    #[serde(default)]
    pub overlay: String,
    /// The female cut: drawn instead of `overlay` on a female body sheet ("" = `overlay`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub overlay_f: String,
}

impl Armor {
    /// The overlay sheet worn on a female (`female`) or other body ("" = none).
    pub fn overlay_for(&self, female: bool) -> &str {
        if female && !self.overlay_f.is_empty() {
            &self.overlay_f
        } else {
            &self.overlay
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ranged {
    /// Projectile sprite, drawn pointing right and rotated to the flight direction.
    pub projectile: SpriteRef,
    /// Pixels per second.
    #[serde(default = "default_projectile_speed")]
    pub speed: f32,
    /// Maximum distance in tiles.
    #[serde(default = "default_range")]
    pub range: f32,
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
    /// Using the item teaches this spell (scrolls, spell books); "" = none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub teaches: String,
    /// Sheet (character layout) drawn in the character's hand while the item is in the Weapon
    /// or left-hand slot, or a tool while it is used; "" = not visible.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub held: String,
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
            teaches: String::new(),
            held: String::new(),
            tool: String::new(),
            use_on: UseTarget::default(),
            crop: None,
            food: false,
        }
    }
}

impl ItemDef {
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

/// A lock that needs a key item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// A lockpicking skill of this level opens it without the key (0 = can't be picked).
    #[serde(default)]
    pub pick_difficulty: u32,
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
            pick_difficulty: 0,
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
    },
}

impl Behavior {
    /// Chasing with only a sight range (no leash, never flees, no kept distance).
    pub fn chase(sight: f32) -> Self {
        Behavior::Chase { sight, leash: 0.0, flee_below: 0.0, keep_distance: 0.0 }
    }
}

/// A character: a layered model (`appearance` + `equipment`) with behavior and combat values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterDef {
    pub id: String,
    pub name: String,
    /// Legacy: the pre-drawn sheet of old projects; becomes the body on load
    /// (`Database::upgrade_characters`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sheet: String,
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
    pub touch_effect: Option<crate::effects::EffectSpec>,
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
}

/// The serde defaults (what an entry with only `id` and `name` loads as). The editor's "+ New"
/// value is [`CharacterDef::fresh`]; the character builder fills in a body and clothes.
impl Default for CharacterDef {
    fn default() -> Self {
        CharacterDef {
            id: String::new(),
            name: String::new(),
            sheet: String::new(),
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
    pub tilesets: Vec<TilesetDef>,
    pub sheets: Vec<SheetDef>,
    pub items: Vec<ItemDef>,
    pub props: Vec<PropDef>,
    pub characters: Vec<CharacterDef>,
    pub sounds: Vec<SoundDef>,
    pub quests: Vec<crate::quest::QuestDef>,
    /// Multi-tile assets for the editor (houses, walls, rooms…); the game doesn't use them.
    pub structures: Vec<StructureDef>,
    /// Skills and spells (`ProjectConfig::skills` switches the systems on).
    pub skills: Vec<crate::skills::SkillDef>,
    pub spells: Vec<crate::skills::SpellDef>,
    /// Crafting recipes.
    pub recipes: Vec<crate::crafting::RecipeDef>,
    /// Achievements (the game's; unlocks are the player's, see `achievements::Progress`).
    pub achievements: Vec<crate::achievements::AchievementDef>,
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
    /// Tilesets the tiles come from; a tile's slot indexes into this list.
    #[serde(default)]
    pub tilesets: Vec<String>,
    /// Tiles per map layer (0 = ground, 1 = detail, 2 = overhead, …), row by row (`w × h`),
    /// encoded like map tiles: 0 = keep the map's tile, `StructureDef::CLEAR` = empty it.
    #[serde(default)]
    pub layers: Vec<Vec<u32>>,
    /// Collision override per tile (`CollisionOverride` as a byte; 0 = keep the map's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collision: Vec<u8>,
    /// Entities placed with it (doors, chests, signs…); positions are relative to the
    /// structure's top-left corner (uids are assigned when placing).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<crate::entity::EntityInstance>,
}

/// Id → position caches of the lists the game looks up per entity per frame (items, props,
/// characters, sheets, tilesets). A hit is verified (`list[i].id == id`) and a miss or a stale
/// entry falls back to the linear search, so a list edited in place (the editor's forms, the
/// agent server) only makes lookups slower until [`Database::reindex`] runs, never wrong.
#[derive(Clone, Default)]
pub struct DefIndex {
    tilesets: std::collections::HashMap<String, usize>,
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
        write!(f, "DefIndex({} ids)", self.tilesets.len() + self.sheets.len() + self.items.len() + self.props.len() + self.characters.len())
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
        self.index = DefIndex { tilesets: build(&self.tilesets), sheets: build(&self.sheets), items: build(&self.items), props: build(&self.props), characters: build(&self.characters) };
    }

    fn lookup<'a, T: Def>(list: &'a [T], index: &std::collections::HashMap<String, usize>, id: &str) -> Option<&'a T> {
        if let Some(d) = index.get(id).and_then(|&i| list.get(i)) {
            if d.id() == id {
                return Some(d);
            }
        }
        list.iter().find(|d| d.id() == id)
    }

    pub fn tileset(&self, id: &str) -> Option<&TilesetDef> {
        Self::lookup(&self.tilesets, &self.index.tilesets, id)
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
    pub fn skill(&self, id: &str) -> Option<&crate::skills::SkillDef> {
        self.skills.iter().find(|d| d.id == id)
    }
    pub fn spell(&self, id: &str) -> Option<&crate::skills::SpellDef> {
        self.spells.iter().find(|d| d.id == id)
    }
    pub fn achievement(&self, id: &str) -> Option<&crate::achievements::AchievementDef> {
        self.achievements.iter().find(|d| d.id == id)
    }
    pub fn recipe(&self, id: &str) -> Option<&crate::crafting::RecipeDef> {
        self.recipes.iter().find(|d| d.id == id)
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

    /// Old projects: a character's pre-drawn `sheet` becomes the body of its model (the sheet
    /// has no reference-ramp pixels, so nothing is recolored).
    pub fn upgrade_characters(&mut self) {
        for d in &mut self.characters {
            if d.appearance.body.is_empty() && !d.sheet.is_empty() {
                d.appearance.body = std::mem::take(&mut d.sheet);
                if let Some(s) = self.sheets.iter_mut().find(|s| s.id == d.appearance.body) {
                    s.kind = SheetKind::Body;
                }
            } else {
                d.sheet.clear();
            }
        }
    }

    /// Is `sheet` a female body?
    pub fn female_body(&self, sheet: &str) -> bool {
        self.sheet(sheet).is_some_and(|s| s.female)
    }

    /// Bodies players may choose for a character of sex `female` (in sheet order).
    pub fn playable_bodies(&self, female: bool) -> Vec<&SheetDef> {
        self.sheets.iter().filter(|s| s.kind == SheetKind::Body && s.playable && s.female == female).collect()
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

impl_def!(TilesetDef, crate::refactor::RefKind::Tileset, tilesets);
impl_def!(SheetDef, crate::refactor::RefKind::Sheet, sheets);
impl_def!(ItemDef, crate::refactor::RefKind::Item, items, refs = crate::refactor::refs_of_item);
impl_def!(PropDef, crate::refactor::RefKind::Prop, props, refs = crate::refactor::refs_of_prop);
impl_def!(CharacterDef, crate::refactor::RefKind::Character, characters, refs = crate::refactor::refs_of_character);
impl_def!(SoundDef, crate::refactor::RefKind::Sound, sounds);
impl_def!(crate::quest::QuestDef, crate::refactor::RefKind::Quest, quests);
impl_def!(StructureDef, crate::refactor::RefKind::Structure, structures, refs = crate::refactor::refs_of_structure);
impl_def!(crate::skills::SkillDef, crate::refactor::RefKind::Skill, skills, refs = crate::refactor::refs_of_skill);
impl_def!(crate::skills::SpellDef, crate::refactor::RefKind::Spell, spells, refs = crate::refactor::refs_of_spell);
impl_def!(crate::crafting::RecipeDef, crate::refactor::RefKind::Recipe, recipes, refs = crate::refactor::refs_of_recipe);
impl_def!(crate::achievements::AchievementDef, crate::refactor::RefKind::Achievement, achievements);

/// Runs `$body` with `$T` bound to the definition type of `$kind` (a
/// [`RefKind`](crate::refactor::RefKind)); `$else` for `Map`, which is no definition.
///
/// ```ignore
/// let n = with_def_kind!(kind, |T| T::list(&db).len(), else => 0);
/// ```
#[macro_export]
macro_rules! with_def_kind {
    ($kind:expr, |$T:ident| $body:expr, else => $else:expr) => {
        match $kind {
            $crate::refactor::RefKind::Tileset => {
                type $T = $crate::defs::TilesetDef;
                $body
            }
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
            $crate::refactor::RefKind::Skill => {
                type $T = $crate::skills::SkillDef;
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
            $crate::refactor::RefKind::Map => $else,
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

    /// `Default` is what a file entry with only the required fields loads as (so tests and the
    /// editor can write `..Default::default()`), and `fresh` is the editor's "+ New" value.
    #[test]
    fn defaults_match_the_file_format() {
        let t: TilesetDef = ron::from_str(r#"(id: "t", image: "tilesets/t.png")"#).unwrap();
        assert_eq!(t, TilesetDef::new("t", "tilesets/t.png"));
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
        assert_eq!((TilesetDef::fresh("x").image.as_str(), SheetDef::fresh("x").image.as_str()), ("tilesets/new.png", "sprites/new.png"));
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
        item.armor = Some(Armor { slot: EquipSlot::Head, defense: 1, speed: 0.0, overlay: String::new(), overlay_f: String::new() });
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
    fn tile_animation_frames() {
        let a = TileAnim { tile: 3, frames: vec![3, 40, 41], fps: 2.0 };
        assert_eq!(a.frame_at(0.0), 3);
        assert_eq!(a.frame_at(0.6), 40);
        assert_eq!(a.frame_at(1.2), 41);
        assert_eq!(a.frame_at(1.5), 3);
        let still = TileAnim { tile: 3, frames: vec![], fps: 2.0 };
        assert_eq!(still.frame_at(9.0), 3);
    }
}

#[cfg(test)]
mod furniture_tests {
    use super::*;

    #[test]
    fn ladder_tiles_in_ron() {
        // Tilesets from before ladders load without them; empty lists aren't written.
        let t: TilesetDef = ron::from_str(r#"(id: "t", image: "tilesets/t.png", solid: [1])"#).unwrap();
        assert!(t.ladder.is_empty());
        assert!(!ron::to_string(&t).unwrap().contains("ladder"));
        let t = TilesetDef { ladder: vec![4, 9], ..t };
        let back: TilesetDef = ron::from_str(&ron::to_string(&t).unwrap()).unwrap();
        assert_eq!(back.ladder, vec![4, 9]);
    }

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
