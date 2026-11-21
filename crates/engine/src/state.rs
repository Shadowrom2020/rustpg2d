//! Persistent game state: flags, variables, inventory and per-map entity changes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::entity::Facing;
use crate::defs::EquipSlot;
use crate::inventory::{Inventory, ItemStack};

/// Items worn / held by a character (the player's are not part of the inventory grid).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Equipment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clothing: Option<ItemStack>,
    pub head: Option<ItemStack>,
    pub body: Option<ItemStack>,
    pub weapon: Option<ItemStack>,
    /// Left hand: shield or light.
    pub offhand: Option<ItemStack>,
    /// A tool (fishing rod, …), held while it is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<ItemStack>,
    /// A backpack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<ItemStack>,
    /// Worn magic items: an amulet, a cloak, a belt, boots, gloves or bracers, two rings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub neck: Option<ItemStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloak: Option<ItemStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belt: Option<ItemStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feet: Option<ItemStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hands: Option<ItemStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ring: Option<ItemStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ring2: Option<ItemStack>,
    /// A stack of ammunition: what ranged weapons shoot first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo: Option<ItemStack>,
}

impl Equipment {
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemStack> {
        match slot {
            EquipSlot::Clothing => self.clothing.as_ref(),
            EquipSlot::Head => self.head.as_ref(),
            EquipSlot::Body => self.body.as_ref(),
            EquipSlot::Weapon => self.weapon.as_ref(),
            EquipSlot::Offhand => self.offhand.as_ref(),
            EquipSlot::Tool => self.tool.as_ref(),
            EquipSlot::Back => self.back.as_ref(),
            EquipSlot::Neck => self.neck.as_ref(),
            EquipSlot::Cloak => self.cloak.as_ref(),
            EquipSlot::Belt => self.belt.as_ref(),
            EquipSlot::Feet => self.feet.as_ref(),
            EquipSlot::Hands => self.hands.as_ref(),
            EquipSlot::Ring => self.ring.as_ref(),
            EquipSlot::Ring2 => self.ring2.as_ref(),
            EquipSlot::Ammo => self.ammo.as_ref(),
        }
    }
    pub fn slot_mut(&mut self, slot: EquipSlot) -> &mut Option<ItemStack> {
        match slot {
            EquipSlot::Clothing => &mut self.clothing,
            EquipSlot::Head => &mut self.head,
            EquipSlot::Body => &mut self.body,
            EquipSlot::Weapon => &mut self.weapon,
            EquipSlot::Offhand => &mut self.offhand,
            EquipSlot::Tool => &mut self.tool,
            EquipSlot::Back => &mut self.back,
            EquipSlot::Neck => &mut self.neck,
            EquipSlot::Cloak => &mut self.cloak,
            EquipSlot::Belt => &mut self.belt,
            EquipSlot::Feet => &mut self.feet,
            EquipSlot::Hands => &mut self.hands,
            EquipSlot::Ring => &mut self.ring,
            EquipSlot::Ring2 => &mut self.ring2,
            EquipSlot::Ammo => &mut self.ammo,
        }
    }
    pub fn item(&self, slot: EquipSlot) -> Option<&str> {
        self.get(slot).map(|s| s.item.as_str())
    }
    pub fn is_empty(&self) -> bool {
        EquipSlot::ALL.iter().all(|s| self.get(*s).is_none())
    }
    /// Every equipped stack.
    pub fn stacks_mut(&mut self) -> impl Iterator<Item = &mut ItemStack> {
        [
            &mut self.clothing,
            &mut self.head,
            &mut self.body,
            &mut self.weapon,
            &mut self.offhand,
            &mut self.tool,
            &mut self.back,
            &mut self.neck,
            &mut self.cloak,
            &mut self.belt,
            &mut self.feet,
            &mut self.hands,
            &mut self.ring,
            &mut self.ring2,
            &mut self.ammo,
        ]
        .into_iter()
        .filter_map(|s| s.as_mut())
    }
    pub fn stacks(&self) -> impl Iterator<Item = &ItemStack> {
        EquipSlot::ALL.into_iter().filter_map(|s| self.get(s))
    }
    pub fn count(&self, item: &str) -> u32 {
        EquipSlot::ALL.iter().filter_map(|s| self.get(*s)).filter(|s| s.item == item).map(|s| s.count).sum()
    }
}

/// Runtime changes to a placed entity, persisted across map changes and saves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityState {
    pub active: bool,
    pub frame: u32,
    pub x: f32,
    pub y: f32,
    pub facing: Facing,
    /// Remaining stack size of a partly picked-up item.
    #[serde(default)]
    pub count: Option<u32>,
    /// Its lock has been opened.
    #[serde(default)]
    pub unlocked: bool,
    /// Current state of a prop (None in saves from before named states: `frame` is used).
    #[serde(default)]
    pub state: Option<String>,
    /// Where a patrolling character is on its route.
    #[serde(default)]
    pub patrol: Option<PatrolState>,
    /// A character with a schedule stopped following it (Schedule action / script).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub schedule_off: bool,
    /// When its prop state last changed (`WorldState::total_hours`; regrowing states).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_at: Option<f64>,
    /// What grows on a farm plot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planted: Option<Planted>,
    /// The faction a *Set faction* action gave the character (None = its own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
    /// A prop with object statistics: its hit points when damaged (None = whole).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<i32>,
}

/// A seed planted on a farm plot (`EntityState::planted`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Planted {
    /// The seed item (its `crop`).
    pub seed: String,
    /// When it was planted (`WorldState::total_hours`).
    pub at: f64,
    /// Extra growing hours from rain.
    #[serde(default)]
    pub bonus: f64,
}

/// Progress of a patrolling character (saved, so a loaded game continues the route).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatrolState {
    /// Waypoint it is heading for.
    pub index: u32,
    /// Walking the route backwards (back-and-forth routes).
    #[serde(default)]
    pub back: bool,
    /// Stopped by an action / script.
    #[serde(default)]
    pub stopped: bool,
    /// A route given by an action / script instead of the placed character's own.
    #[serde(default)]
    pub route: Option<crate::entity::PatrolRoute>,
}

/// The save game layout this version writes (`WorldState::version`). Saves of older versions
/// carry none (0); every field they lack has a default. 2 (0.28): the SRD rules; older saves get
/// the project's starting ability scores and proficiencies when they load.
pub const SAVE_VERSION: u32 = 2;

fn save_version<S: serde::Serializer>(_: &u32, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u32(SAVE_VERSION)
}

/// The hero in a beast's shape (Wild Shape, Polymorph): the beast's stat block (its hit points,
/// AC, speeds, attacks, size, senses; STR, DEX and CON, all six scores with Polymorph) stands in
/// for the hero's until it ends.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeroShape {
    /// The beast's character definition.
    pub form: String,
    /// What gave it: a spell's source (`polymorph@player`), a class action (`wild-shape`).
    pub source: String,
    /// The beast's maximum hit points.
    pub max_hp: i32,
    /// The hero's own hit points and ability scores, back when the shape ends.
    pub hero_hp: i32,
    pub abilities: crate::srd::Abilities,
    /// Seconds left (0 = until it ends otherwise: concentration, 0 hit points).
    pub seconds: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldState {
    /// [`SAVE_VERSION`] of the program that wrote the save (0: before 0.12).
    #[serde(default, serialize_with = "save_version")]
    pub version: u32,
    pub flags: BTreeSet<String>,
    pub vars: BTreeMap<String, i64>,
    pub inventory: Inventory,
    /// map name -> entity uid -> state
    pub entities: BTreeMap<String, BTreeMap<u32, EntityState>>,
    /// Keys of `once` triggers that already fired.
    pub fired: BTreeSet<String>,
    pub current_map: String,
    pub player_pos: (f32, f32),
    pub player_facing: Facing,
    /// `None` until the game starts (then set to the player's max HP).
    #[serde(default)]
    pub player_hp: Option<i32>,
    /// Old saves only: item id of the equipped weapon (migrated into `equipment`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub equipped: String,
    #[serde(default)]
    pub equipment: Equipment,
    /// Seconds since the Unix epoch when this was saved.
    #[serde(default)]
    pub saved_at: u64,
    /// Total play time in seconds.
    #[serde(default)]
    pub play_time: f64,
    /// Time of day in hours (0..24), advanced while playing.
    #[serde(default = "crate::project::default_start_hour")]
    pub hour: f32,
    /// Days passed since the game began (the clock wrapping past midnight, sleeping).
    #[serde(default)]
    pub day: u32,
    /// map -> spawner zone uid -> characters defeated for good (`Respawn::Never`).
    #[serde(default)]
    pub spawner_kills: BTreeMap<String, BTreeMap<u32, u32>>,
    /// map -> spawner zones switched off by name ("" = all of the map's spawners).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spawners_off: BTreeMap<String, std::collections::BTreeSet<String>>,
    /// map -> entity uid -> contents of containers that have been opened.
    #[serde(default)]
    pub containers: BTreeMap<String, BTreeMap<u32, Inventory>>,
    /// map -> entity uid -> current stock of merchants the player traded with.
    #[serde(default)]
    pub shops: BTreeMap<String, BTreeMap<u32, Vec<crate::defs::ShopEntry>>>,
    /// quest id -> progress (started quests only).
    #[serde(default)]
    pub quests: BTreeMap<String, crate::quest::QuestProgress>,
    /// The player character's name and sex, chosen in the character creator.
    #[serde(default)]
    pub player_name: String,
    #[serde(default)]
    pub player_sex: Sex,
    /// The player's race (`races::RaceDef`; "" = none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub player_race: String,
    /// The race of the player's subrace (`dwarf` for a hill dwarf; "" = none): *Player race*
    /// conditions match it too.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub player_race_parent: String,
    /// The hero's background (`backgrounds::BackgroundDef`; "" = none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub player_background: String,
    /// Languages the hero speaks (`common`, `dwarvish`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub languages: BTreeSet<String>,
    /// Damage types the hero resists (`poison`, `fire`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub resistances: BTreeSet<String>,
    /// The creator's racial choices: choice id -> the picked options (`draconic-ancestry` ->
    /// `["draconic-ancestry-red"]`, the high elf's cantrip, the background's languages).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub choices: BTreeMap<String, Vec<String>>,
    /// The prop the player carries in both hands ("" = none; `Action::Carry`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub player_carrying: String,
    /// The player's standing with factions that changed (others start from the attitude table).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub standing: BTreeMap<String, i32>,
    /// The faction the player joined (*Set faction* on "player"); "" = their race's, else the
    /// player character's.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub player_faction: String,
    /// map -> items the player dropped there (they stay until picked up again).
    #[serde(default)]
    pub dropped: BTreeMap<String, Vec<DroppedItem>>,
    /// Counter for the uids of dropped items (see [`DROPPED_UID_BASE`]).
    #[serde(default)]
    pub next_drop: u32,
    /// Maps the player owns (`set_home`): furniture can be placed there.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub homes: BTreeSet<String>,
    /// map -> furniture the player placed there (a prop each, see [`FURNITURE_UID_BASE`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub furniture: BTreeMap<String, Vec<PlacedFurniture>>,
    /// Counter for the uids of placed furniture.
    #[serde(default)]
    pub next_furniture: u32,
    /// The hero's ability scores (SRD 5.1), STR to CHA.
    #[serde(default)]
    pub abilities: crate::srd::Abilities,
    /// What the hero is proficient in: skills (`stealth`), tools (`thieves-tools`), saving throws
    /// (`save:dex`); see [`crate::srd::abilities::proficiency_id`].
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub proficiencies: BTreeSet<String>,
    /// The roll log: the last [`crate::srd::check::LOG_LEN`] rolls, newest last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rolls: Vec<crate::srd::RollRecord>,
    /// The last check or saving throw (*Check passed*); not saved.
    #[serde(skip)]
    pub last_check: Option<crate::srd::RollRecord>,
    /// Rolls made so far in this session (the game shows the new ones); not saved.
    #[serde(skip)]
    pub roll_count: u64,
    /// Learned spells.
    #[serde(default)]
    pub spells: BTreeSet<String>,
    /// Current mana.
    #[serde(default)]
    pub mana: f32,
    /// Fog of war: explored tiles per map (only maps with fog of war).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub explored: BTreeMap<String, crate::fog::Explored>,
    /// The look chosen in the character creator (figure and colours).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_appearance: Option<crate::defs::Appearance>,
    /// NPCs whose equipment or carried items changed (scripts, actions): map -> uid -> items.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_items: BTreeMap<String, BTreeMap<u32, NpcItems>>,
    /// The player definition's clothing was put on (saves from before clothing get it once).
    #[serde(default)]
    pub dressed: bool,
    /// Experience points gained in total (experience and levels).
    #[serde(default)]
    pub xp: u64,
    /// The player's level; 0 in old saves and fresh worlds means 1 (see [`WorldState::player_level`]).
    #[serde(default)]
    pub level: u32,
    /// The hit points each level up gave (the hit die's roll; level 2 first; a missing entry =
    /// the die's average): `progression::max_hp` adds CON to them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hp_rolls: Vec<i32>,
    /// Hit dice left to spend on short rests (None = all of them: the level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_dice: Option<u32>,
    /// The hero's class (its subclass and choices; `None` = none: a d10 hit die, no features).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<crate::classes::HeroClass>,
    /// Level-ups waiting for their choices (the level-up window): "player" or a companion's uid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levelups: Vec<PendingLevel>,
    /// Uses of the hero's class resources spent since their rest (resource id -> uses: rages,
    /// ki, Lay on Hands' hit points; `relentless-rage` counts Relentless Rage's saves).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spent: BTreeMap<String, i32>,
    /// Spells prepared for the day (prepared casters and wizards; `spells` are the known ones,
    /// cantrips and a wizard's spellbook).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prepared: Vec<String>,
    /// Spell slots spent since their rest, by slot level (index 0 = level 1).
    #[serde(default, skip_serializing_if = "slots_unspent")]
    pub slots_spent: [u32; 9],
    /// The spell the hero concentrates on: (its effects' source `spell@player`, seconds left).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concentration: Option<(String, f32)>,
    /// The beast shape the hero is in (Wild Shape, Polymorph), `None` in their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<HeroShape>,
    /// The magic items the hero is attuned to (item ids, at most three).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attuned: Vec<String>,
    /// The characters knocked out in fights instead of killed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knocked_out: Vec<KnockedOut>,
    /// The hero's own ability scores while items or buffs raise `abilities` (a Belt of Giant
    /// Strength); None = `abilities` are their own. Change scores through
    /// [`WorldState::base_abilities_mut`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability_base: Option<crate::srd::Abilities>,
    /// The last dawn the charged items regained charges at (days since the start, from 6:00).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_dawn: Option<i64>,
    /// The total of the hero's last search (the action bar's Search; not saved).
    #[serde(skip)]
    pub last_search: Option<i64>,
    /// When the hero's last long rest ended (game hours since the start): one long rest in 24
    /// hours gives its benefits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_long_rest: Option<f64>,
    /// What the class adds to checks (computed from the class; not saved).
    #[serde(skip)]
    pub training: crate::srd::check::Training,
    /// The traps' states (found, disarmed, sprung) by `map/uid` of their zone or prop.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub traps: BTreeMap<String, crate::srd::traps::TrapState>,
    /// The poison on the hero's weapon or ammunition (one dose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coating: Option<crate::srd::poisons::Coating>,
    /// The diseases the hero caught (incubating or sick).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diseases: Vec<crate::diseases::ActiveDisease>,
    /// Food, water and the march of the day (the survival rules; untouched while they are off).
    #[serde(default, skip_serializing_if = "is_default_survival")]
    pub survival: crate::srd::survival::Survival,
    /// The project's coins: (item id, worth in cp), the most valuable first (set by the game from
    /// the definitions; not saved). Money is what these add up to.
    #[serde(skip)]
    pub coins: Vec<(String, u32)>,
    /// The hero's purse in copper pieces (shown on the character sheet). Coins reaching the bag
    /// are added to it ([`WorldState::bank_coins`]); until 0.32 the coins were bag items.
    #[serde(default, skip_serializing_if = "is_zero_u32_ref")]
    pub money: u32,
    /// The hero's conditions (poisoned, prone, exhaustion…) and ongoing damage or healing; until
    /// 0.27 status effects, which read as their nearest condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<crate::srd::ActiveCondition>,
    /// Bestiary: character definition id -> how many the player has defeated.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bestiary: BTreeMap<String, u32>,
    /// The last dialog lines (oldest first, at most [`DIALOG_LOG_MAX`]) for the journal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dialog_log: Vec<LogLine>,
    /// Weather set by actions / scripts, by map name (overrides the map's own weather).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub weather: BTreeMap<String, crate::map::Weather>,
    /// Characters following the player, in the order they joined.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companions: Vec<Companion>,
    /// Text variables (*Input text*, `set_text`): shown by `{name}` placeholders like number
    /// variables (a text variable wins over a number variable of the same name).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub texts: BTreeMap<String, String>,
    /// Running timers (*Start timer*), by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub timers: BTreeMap<String, crate::timers::Timer>,
    /// *Tint screen* and *Fade out*: colours over the whole screen.
    #[serde(default, skip_serializing_if = "crate::screen_fx::ScreenFx::is_empty")]
    pub screen: crate::screen_fx::ScreenFx,
    /// Pictures of *Show picture* by layer (1 to 100), moves in progress included.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pictures: BTreeMap<u32, crate::pictures::Picture>,
    /// HUD elements scripts set (`hud_text`, `hud_image`, `hud_bar`), by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hud: BTreeMap<String, crate::script_ui::HudElement>,
    /// A window a script asked to open or close (`window_open`); the game takes it after the
    /// call. Not saved.
    #[serde(skip)]
    pub window_request: Option<crate::script_ui::WindowRequest>,
    /// Spells characters learned, by where they are placed ("map/uid"): companions cast them.
    /// Kept when a companion is dismissed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub npc_spells: BTreeMap<String, Vec<String>>,
}

/// A placed character that follows the player from map to map. Its look, items and triggers
/// come from where it is placed (`map`, `uid`); dismissed, it goes back there.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    pub map: String,
    pub uid: u32,
    /// Its name (scripts address it by name on every map).
    #[serde(default)]
    pub name: String,
    /// Its HP (None = full).
    #[serde(default)]
    pub hp: Option<i32>,
    /// Knocked down: where it lies on the current map (world pixels); None = on its feet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down: Option<[f32; 2]>,
    /// How it fights (the player's order).
    #[serde(default, skip_serializing_if = "Order::is_attack")]
    pub order: Order,
    /// Told to stay: where it waits on the current map (world pixels); None = follows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post: Option<[f32; 2]>,
    /// Its mana (None = full).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<f32>,
    /// In fights the AI plays its turns (else the player does).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ai: bool,
    /// A party member with a class: its class (choices), experience, level and HP rolls (as the
    /// hero's; `None` = its definition's class at its level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<crate::classes::HeroClass>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub xp: u64,
    #[serde(default, skip_serializing_if = "is_zero_u32_ref")]
    pub level: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hp_rolls: Vec<i32>,
    /// Uses of its class resources spent since their rest (as the hero's `spent`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spent: BTreeMap<String, i32>,
    /// Its spells as the hero's: known (cantrips, spellbook), prepared, slots spent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spells: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prepared: Vec<String>,
    #[serde(default, skip_serializing_if = "slots_unspent")]
    pub slots_spent: [u32; 9],
    /// The magic items it is attuned to (item ids, at most three).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attuned: Vec<String>,
    /// Dead since this game hour (`DownRule::Dead`; revival spells bring it back).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_at: Option<f64>,
}

fn slots_unspent(s: &[u32; 9]) -> bool {
    s.iter().all(|n| *n == 0)
}

fn is_default_survival(s: &crate::srd::survival::Survival) -> bool {
    *s == crate::srd::survival::Survival::default()
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

fn is_zero_u32_ref(n: &u32) -> bool {
    *n == 0
}

/// A level-up whose choices wait for the level-up window.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PendingLevel {
    /// "player", or a companion's uid (its `Companion::uid`).
    pub who: String,
    pub level: u32,
}

/// How a companion fights (`Companion::order`); staying put is separate (`Companion::post`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// Fights hostile characters near the player.
    #[default]
    Attack,
    /// Only fights what hunts the player or itself.
    Defend,
    /// Never fights; backs away from enemies close to it.
    Passive,
}

impl Order {
    pub const ALL: [Order; 3] = [Order::Attack, Order::Defend, Order::Passive];

    fn is_attack(&self) -> bool {
        *self == Order::Attack
    }

    /// The name scripts use (`order("guard", "defend")`).
    pub fn id(self) -> &'static str {
        match self {
            Order::Attack => "attack",
            Order::Defend => "defend",
            Order::Passive => "passive",
        }
    }

    pub fn from_id(s: &str) -> Option<Order> {
        Order::ALL.into_iter().find(|o| o.id().eq_ignore_ascii_case(s.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Order::Attack => "Attack",
            Order::Defend => "Defend",
            Order::Passive => "Passive",
        }
    }
}

/// How many dialog lines the journal keeps.
pub const DIALOG_LOG_MAX: usize = 1000;

/// One line of the dialog log: who said what, or what the player answered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub speaker: String,
    pub text: String,
    /// The option the player picked in a choice.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub answer: bool,
}

/// Worn and carried items of an NPC (saved once they differ from the map / definition).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NpcItems {
    pub equipment: Equipment,
    pub inventory: Vec<ItemStack>,
}

/// Uids of dropped items start here, far above the uids of map entities and of spawned
/// characters (`0x4000_0000..`).
pub const DROPPED_UID_BASE: u32 = 0x6000_0000;

/// Uids of furniture the player placed start here: above visitors (`0x5800_0000..`), below
/// dropped items.
pub const FURNITURE_UID_BASE: u32 = 0x5C00_0000;
const _: () = assert!(FURNITURE_UID_BASE < DROPPED_UID_BASE);

/// A piece of furniture the player placed in a home: the prop and the item it was made from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedFurniture {
    pub uid: u32,
    pub prop: String,
    /// The item picking it up gives back.
    pub item: String,
    /// Feet position in pixels.
    pub x: f32,
    pub y: f32,
}

/// An item stack the player dropped on a map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DroppedItem {
    pub uid: u32,
    pub item: String,
    pub count: u32,
    /// A magic item's used charges (such a pile is never joined with another one).
    #[serde(default, skip_serializing_if = "is_zero_u32_ref")]
    pub spent: u32,
    /// Feet position in pixels.
    pub x: f32,
    pub y: f32,
}

/// The player character's sex (for dialogs, conditions and scripts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sex {
    #[default]
    Male,
    Female,
}

impl Sex {
    pub const ALL: [Sex; 2] = [Sex::Male, Sex::Female];
    pub fn label(self) -> &'static str {
        match self {
            Sex::Male => "Male",
            Sex::Female => "Female",
        }
    }
    /// "male" / "female" (scripts).
    pub fn key(self) -> &'static str {
        match self {
            Sex::Male => "male",
            Sex::Female => "female",
        }
    }
}

/// A character a fight knocked out instead of killing it (the player's choice).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KnockedOut {
    pub map: String,
    pub uid: u32,
    /// Its name (empty = unnamed) and definition id.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub def: String,
}

impl WorldState {
    /// Whether a fight knocked out a character of this name or definition id.
    pub fn was_knocked_out(&self, name: &str) -> bool {
        let n = name.trim();
        !n.is_empty() && self.knocked_out.iter().any(|k| k.name.eq_ignore_ascii_case(n) || k.def == n)
    }

    /// In-game hours since the game began (days and the hour of the day).
    pub fn total_hours(&self) -> f64 {
        self.day as f64 * 24.0 + self.hour as f64
    }

    /// Moves the clock forward by `hours`, counting the days that pass.
    /// The hero's own ability scores to change (items' raised scores are put back first; the
    /// runtime raises them again).
    pub fn base_abilities_mut(&mut self) -> &mut crate::srd::Abilities {
        if let Some(b) = self.ability_base.take() {
            self.abilities = b;
        }
        &mut self.abilities
    }

    pub fn advance_hours(&mut self, hours: f32) {
        let t = self.total_hours() + hours.max(0.0) as f64;
        self.day = (t / 24.0).floor() as u32;
        self.hour = (t - self.day as f64 * 24.0) as f32;
    }

    /// The placed character `uid` of `map` follows the player.
    pub fn is_companion(&self, map: &str, uid: u32) -> bool {
        self.companions.iter().any(|c| c.uid == uid && c.map == map)
    }

    /// The spells the character placed as `uid` on `map` has learned.
    pub fn npc_spells(&self, map: &str, uid: u32) -> &[String] {
        self.npc_spells.get(&format!("{map}/{uid}")).map_or(&[], Vec::as_slice)
    }

    /// Teaches it `spell`; false when it knew it already.
    pub fn teach_npc(&mut self, map: &str, uid: u32, spell: &str) -> bool {
        let known = self.npc_spells.entry(format!("{map}/{uid}")).or_default();
        if known.iter().any(|s| s == spell) {
            return false;
        }
        known.push(spell.to_string());
        true
    }

    /// Adds a line to the dialog log (dropping the oldest beyond [`DIALOG_LOG_MAX`]).
    pub fn log_dialog(&mut self, speaker: &str, text: &str, answer: bool) {
        if text.trim().is_empty() {
            return;
        }
        self.dialog_log.push(LogLine { speaker: speaker.to_string(), text: text.to_string(), answer });
        if self.dialog_log.len() > DIALOG_LOG_MAX {
            let extra = self.dialog_log.len() - DIALOG_LOG_MAX;
            self.dialog_log.drain(..extra);
        }
    }

    /// How many characters of a definition have been defeated.
    pub fn defeated(&self, character: &str) -> u32 {
        self.bestiary.get(character).copied().unwrap_or(0)
    }

    /// Whether the hero understands a language: their own, or every one under Tongues /
    /// Comprehend Languages.
    pub fn knows_language(&self, id: &str) -> bool {
        self.languages.contains(id.trim()) || crate::srd::conditions::buffs(&self.effects).any(|b| matches!(b, crate::srd::conditions::Buff::Tongues))
    }

    /// Game hours since the start (days and the hour).
    pub fn game_hours(&self) -> f64 {
        self.day as f64 * 24.0 + self.hour as f64
    }

    /// The player's level (at least 1).
    pub fn player_level(&self) -> u32 {
        self.level.max(1)
    }

    /// Whether the hero is proficient in a skill, tool or saving throw (`"stealth"`,
    /// `"Thieves' Tools"`, `"save:dex"`).
    pub fn is_proficient(&self, what: &str) -> bool {
        self.proficiencies.contains(&crate::srd::abilities::proficiency_id(what))
    }

    pub fn set_proficient(&mut self, what: &str, on: bool) {
        let id = crate::srd::abilities::proficiency_id(what);
        if id.is_empty() {
            return;
        }
        if on {
            self.proficiencies.insert(id);
        } else {
            self.proficiencies.remove(&id);
        }
    }

    /// The hero's money in copper pieces: the purse (and coins in the bag not banked yet).
    pub fn money_cp(&self) -> u32 {
        self.money + self.coins.iter().map(|(id, w)| self.inventory.count(id) * w).sum::<u32>()
    }

    /// Adds `cp` to the purse; returns what was lost (nothing: the purse has no limit).
    pub fn give_cp(&mut self, cp: u32) -> u32 {
        self.money = self.money.saturating_add(cp);
        0
    }

    /// Pays `cp` from the purse; false (nothing taken) when it holds less.
    pub fn pay_cp(&mut self, cp: u32) -> bool {
        self.bank_coins();
        if self.money < cp {
            return false;
        }
        self.money -= cp;
        true
    }

    /// What a coin is worth in cp (`None` = not a coin of the project).
    pub fn coin_worth(&self, item: &str) -> Option<u32> {
        self.coins.iter().find(|k| k.0 == item).map(|k| k.1).filter(|w| *w > 0)
    }

    /// Moves every coin in the bag into the purse (coins picked up, given, a chest's dragged
    /// over, an old save's); returns the cp added.
    pub fn bank_coins(&mut self) -> u32 {
        let mut added = 0;
        for slot in self.inventory.slots.iter_mut() {
            let Some(s) = slot.as_ref() else { continue };
            let Some(w) = self.coins.iter().find(|k| k.0 == s.item && k.1 > 0).map(|k| k.1) else { continue };
            added += s.count * w;
            *slot = None;
        }
        self.money = self.money.saturating_add(added);
        added
    }

    /// The hero's scores, proficiencies and level for a roll.
    pub fn roller(&self) -> crate::srd::check::Roller<'_> {
        crate::srd::check::Roller { abilities: &self.abilities, proficiencies: &self.proficiencies, level: self.player_level(), training: Some(&self.training) }
    }

    /// The hero's bonus to a check (`"stealth"`, `"dex"`, `"dex:thieves-tools"`) or, with `save`,
    /// a saving throw; `None` = nothing known by that name.
    pub fn check_bonus(&self, what: &str, save: bool) -> Option<i64> {
        crate::srd::Spec::parse(what, save).map(|s| s.modifier(&self.roller()))
    }

    /// Rolls an ability check (or with `save` a saving throw) against `dc`, writes it into the
    /// roll log and remembers it for *Check passed*. `None` (and an error line) = unknown name.
    pub fn check(&mut self, what: &str, dc: i32, mode: crate::srd::Mode, save: bool) -> Option<crate::srd::RollRecord> {
        let Some(spec) = crate::srd::Spec::parse(what, save) else {
            eprintln!("[rules] '{what}' is no {} (an ability like dex, a skill like stealth, or ability:tool like dex:thieves-tools)", if save { "saving throw" } else { "ability check" });
            return None;
        };
        let (mode, fails, helper) = self.check_terms(&spec, mode);
        let (mode, die) = self.boons(&spec, mode);
        // (Failing sight takes its penalty off checks by sight: rolled against a higher DC, shown
        // as a lower total; a die adds the other way.)
        let pen = if !save && crate::srd::conditions::needs_sight(&spec.proficiency) { self.training.sight_penalty.max(0) } else { 0 };
        let pen = pen - die;
        let mut rec = spec.roll(&self.roller(), dc + pen, mode);
        if pen != 0 {
            (rec.modifier, rec.total, rec.dc) = (rec.modifier - pen as i64, rec.total - pen as i64, dc);
        }
        if let Some(h) = helper {
            rec.label = format!("{} ({h} helps)", rec.label);
        }
        if fails {
            rec.success = Some(false);
            rec.dice = "fails (condition)".into();
            rec.natural = 0;
        }
        self.last_check = Some(rec.clone());
        self.log_roll(rec.clone());
        Some(rec)
    }

    /// What changes the hero's roll of `spec`: the mode with the conditions' disadvantage (poisoned,
    /// frightened, exhaustion…), armor, a heavy load, dim light for sight and a helper's advantage;
    /// whether it fails on its own (blinded or in darkness and looking, stunned and dodging); who
    /// helps.
    fn check_terms(&self, spec: &crate::srd::Spec, mode: crate::srd::Mode) -> (crate::srd::Mode, bool, Option<String>) {
        use crate::srd::check::Sight;
        let save = spec.save;
        let rules = crate::srd::conditions::rules(&self.effects);
        let dex = spec.ability == crate::srd::Ability::Dex;
        let mut disadvantage = if save { rules.save_disadvantage || (dex && rules.dex_save_disadvantage) } else { rules.check_disadvantage };
        // Armor: untrained gives disadvantage on STR and DEX; some armor on Stealth.
        let str_dex = dex || spec.ability == crate::srd::Ability::Str;
        disadvantage |= (self.training.armor_untrained && str_dex) || (!save && self.training.stealth_disadvantage && spec.proficiency == "stealth");
        // A heavy load (the variant encumbrance): disadvantage on STR, DEX and CON.
        disadvantage |= self.training.heavy_load && (str_dex || spec.ability == crate::srd::Ability::Con);
        // Looking in dim light (lightly obscured): disadvantage; in darkness the check fails.
        let sight = !save && crate::srd::conditions::needs_sight(&spec.proficiency);
        disadvantage |= sight && self.training.sight == Sight::Dim;
        let mut mode = if disadvantage { mode.combine(crate::srd::Mode::Disadvantage) } else { mode };
        // A companion who knows the skill helps (working together): advantage.
        let helper = if save || spec.proficiency.is_empty() { None } else { self.training.help.get(&spec.proficiency).cloned() };
        if helper.is_some() {
            mode = mode.combine(crate::srd::Mode::Advantage);
        }
        let fails = if save { rules.str_dex_saves_fail && str_dex } else { sight && (rules.sight_checks_fail || self.training.sight == Sight::Dark) };
        (mode, fails, helper)
    }

    /// A group check: the hero and `others` (companions' names and stat blocks) roll `what`
    /// against `dc` into the roll log (nobody helps: everyone rolls for themselves); true when at
    /// least half succeed, which *Check passed* then reads.
    pub fn group_check(&mut self, what: &str, dc: i32, others: &[(String, crate::srd::StatBlock)]) -> bool {
        let help = std::mem::take(&mut self.training.help);
        let hero = self.check(what, dc, crate::srd::Mode::Normal, false);
        self.training.help = help;
        let Some(hero) = hero else { return false };
        let mut passed = usize::from(hero.success == Some(true));
        let mut members = 1;
        for (name, block) in others {
            let Some(rec) = crate::srd::check::roll_block(block, name, what, dc, crate::srd::Mode::Normal) else { continue };
            members += 1;
            passed += usize::from(rec.success == Some(true));
            self.log_roll(rec);
        }
        let ok = crate::srd::check::group_passes(passed, members);
        if let Some(last) = &mut self.last_check {
            last.success = Some(ok);
        }
        ok
    }

    /// A contest: the hero's `what` against `name`'s (`block`) `against` (empty = the same); the
    /// higher total wins, a tie isn't a win. Both rolls go into the roll log, the hero's as a
    /// check against the other total + 1 (*Check passed*). False when either name is unknown.
    pub fn contest(&mut self, what: &str, name: &str, block: &crate::srd::StatBlock, against: &str) -> bool {
        let theirs = if against.trim().is_empty() { what } else { against };
        let Some(mut them) = crate::srd::check::roll_block(block, name, theirs, 0, crate::srd::Mode::Normal) else {
            eprintln!("[rules] contest: '{theirs}' is no check");
            return false;
        };
        let Some(ours) = self.check(what, them.total as i32 + 1, crate::srd::Mode::Normal, false) else { return false };
        them.dc = 0;
        them.success = None;
        self.log_roll(them);
        // (`success` is false too when the check fails on its own: looking in darkness.)
        ours.success == Some(true)
    }

    /// Rolls the hero's check of `what` without a DC (a search): the conditions, light and help
    /// as in [`WorldState::check`], into the roll log; a check that fails on its own totals 0.
    /// The spells' boons on a roll of `spec`: Enhance Ability's and Beacon of Hope's advantage,
    /// Guidance's die (spent by it). Returns the mode and what the dice add.
    fn boons(&mut self, spec: &crate::srd::Spec, mode: crate::srd::Mode) -> (crate::srd::Mode, i32) {
        let (adv, dice, spent) = crate::srd::conditions::check_boons(&self.effects, spec.ability, &spec.proficiency, spec.save);
        let mode = if adv { mode.combine(crate::srd::Mode::Advantage) } else { mode };
        let die: i32 = dice.iter().map(|d| crate::srd::Expr::parse(d).map(|e| e.roll(&crate::srd::dice::NoMods).total).unwrap_or(0) as i32).sum();
        if spent {
            crate::srd::conditions::spend_check_dice(&mut self.effects);
        }
        (mode, die)
    }

    pub fn roll_check(&mut self, what: &str) -> Option<crate::srd::RollRecord> {
        let spec = crate::srd::Spec::parse(what, false)?;
        let (mode, fails, helper) = self.check_terms(&spec, crate::srd::Mode::Normal);
        let (mode, die) = self.boons(&spec, mode);
        let mut rec = spec.roll(&self.roller(), 0, mode);
        (rec.modifier, rec.total) = (rec.modifier + die as i64, rec.total + die as i64);
        if let Some(h) = helper {
            rec.label = format!("{} ({h} helps)", rec.label);
        }
        if fails {
            rec.total = 0;
            rec.dice = "fails (condition)".into();
        }
        rec.success = None;
        self.log_roll(rec.clone());
        Some(rec)
    }

    /// The hero's passive check of `what` (a skill, an ability, ability:tool): 10 + the bonus, +5
    /// with advantage, −5 with disadvantage (dim light for Perception, poison…); 0 when it would
    /// fail anyway (looking in darkness). `None` = nothing known by that name.
    pub fn passive(&self, what: &str) -> Option<i64> {
        let spec = crate::srd::Spec::parse(what, false)?;
        let (mode, fails, _) = self.check_terms(&spec, crate::srd::Mode::Normal);
        if fails {
            return Some(0);
        }
        Some(crate::srd::check::passive_value(spec.modifier(&self.roller()), mode))
    }

    /// Rolls a dice expression (`2d6+STR`) with the hero's modifiers into the roll log; `None`
    /// (and an error line) when it doesn't parse.
    pub fn roll_dice(&mut self, expr: &str) -> Option<crate::srd::RollRecord> {
        let e = match crate::srd::Expr::parse(expr) {
            Ok(e) => e,
            Err(err) => {
                eprintln!("[rules] {err}");
                return None;
            }
        };
        let rolled = e.roll(&self.roller());
        let rec = crate::srd::RollRecord::plain(expr.trim(), &rolled);
        self.log_roll(rec.clone());
        Some(rec)
    }

    /// Adds a roll to the log (keeping the last [`crate::srd::check::LOG_LEN`]).
    pub fn log_roll(&mut self, rec: crate::srd::RollRecord) {
        self.rolls.push(rec);
        let extra = self.rolls.len().saturating_sub(crate::srd::check::LOG_LEN);
        self.rolls.drain(..extra);
        self.roll_count += 1;
    }

    /// Fills in text placeholders (dialogs, choices, quest texts):
    /// `{player}` = the player's name, `{his|her}` = the first word for a male player, the
    /// second for a female one, `{name}` = the value of variable `name`, `{key:confirm}` = the key
    /// or gamepad button of an action (any action id; `interact` = confirm), `{item:coin}` = how
    /// many of an item the player has. Anything else in
    /// braces is kept as it is.
    /// The text in the current language (`lang::tr`, looked up before the placeholders are
    /// filled in) with `{player}`, `{sir|madam}` and `{var}` filled in.
    pub fn expand_text(&self, text: &str) -> String {
        let translated = crate::lang::tr(text);
        let text = translated.as_str();
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            let Some(j) = after.find('}') else {
                out.push_str(&rest[i..]);
                return out;
            };
            let inner = &after[..j];
            let ident = !inner.is_empty() && inner.chars().all(|c| c.is_alphanumeric() || c == '_');
            if inner == "player" {
                out.push_str(&self.player_name);
            } else if let Some(item) = inner.strip_prefix("item:") {
                // {item:coin}: how many the player has.
                out.push_str(&self.inventory.count(item.trim()).to_string());
            } else if let Some(key) = inner.strip_prefix("key:").and_then(crate::runtime::glyphs::key_text) {
                // {key:confirm}: the key ("E") or, after gamepad use, the button ("A") for it.
                out.push_str(&key);
            } else if inner.contains('|') && !inner.contains('{') {
                let mut parts = inner.splitn(2, '|');
                let (m, f) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                out.push_str(if self.player_sex == Sex::Female { f } else { m });
            } else if let Some(t) = self.texts.get(inner).filter(|_| ident) {
                out.push_str(t);
            } else if ident {
                out.push_str(&self.var(inner).to_string());
            } else {
                out.push('{');
                rest = after;
                continue;
            }
            rest = &after[j + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Puts dropped items on a map: onto a pile of the same item within `merge` pixels, else
    /// as a new pile. Returns the pile's uid and whether it is new.
    pub fn add_dropped(&mut self, map: &str, item: &str, count: u32, x: f32, y: f32, merge: f32) -> (u32, bool) {
        self.add_dropped_spent(map, item, count, 0, x, y, merge)
    }

    /// [`WorldState::add_dropped`] for a stack with `spent` used charges (never joined).
    #[allow(clippy::too_many_arguments)]
    pub fn add_dropped_spent(&mut self, map: &str, item: &str, count: u32, spent: u32, x: f32, y: f32, merge: f32) -> (u32, bool) {
        let piles = self.dropped.entry(map.to_string()).or_default();
        if let Some(p) = piles.iter_mut().find(|p| spent == 0 && p.spent == 0 && p.item == item && (p.x - x).hypot(p.y - y) <= merge) {
            p.count += count;
            return (p.uid, false);
        }
        let uid = DROPPED_UID_BASE + self.next_drop;
        self.next_drop += 1;
        piles.push(DroppedItem { uid, item: item.to_string(), count, spent, x, y });
        (uid, true)
    }

    /// The used charges of dropped pile `uid` on `map` (0 = none, or no such pile).
    pub fn dropped_spent(&self, map: &str, uid: u32) -> u32 {
        self.dropped.get(map).and_then(|v| v.iter().find(|p| p.uid == uid)).map_or(0, |p| p.spent)
    }

    /// Saves from 0.0.5 numbered dropped piles from 0x4000_0000, the range of spawned
    /// characters: give such piles uids in the current range.
    pub fn renumber_old_drops(&mut self) {
        let mut next = self.next_drop;
        for p in self.dropped.values_mut().flatten() {
            if p.uid < DROPPED_UID_BASE {
                p.uid = DROPPED_UID_BASE + next;
                next += 1;
            }
        }
        self.next_drop = next;
    }

    /// Saves a piece of furniture placed on `map`; returns its uid.
    pub fn add_furniture(&mut self, map: &str, prop: &str, item: &str, x: f32, y: f32) -> u32 {
        let uid = FURNITURE_UID_BASE + self.next_furniture;
        self.next_furniture += 1;
        self.furniture.entry(map.to_string()).or_default().push(PlacedFurniture { uid, prop: prop.to_string(), item: item.to_string(), x, y });
        uid
    }

    /// Removes placed furniture `uid` of `map` and what was saved about it (state, contents);
    /// returns it.
    pub fn remove_furniture(&mut self, map: &str, uid: u32) -> Option<PlacedFurniture> {
        let list = self.furniture.get_mut(map)?;
        let k = list.iter().position(|f| f.uid == uid)?;
        let f = list.remove(k);
        if list.is_empty() {
            self.furniture.remove(map);
        }
        if let Some(m) = self.entities.get_mut(map) {
            m.remove(&uid);
        }
        if let Some(m) = self.containers.get_mut(map) {
            m.remove(&uid);
        }
        Some(f)
    }

    /// Updates the size of a dropped pile (0 removes it).
    pub fn set_dropped_count(&mut self, map: &str, uid: u32, count: u32) {
        if let Some(piles) = self.dropped.get_mut(map) {
            if count == 0 {
                piles.retain(|p| p.uid != uid);
            } else if let Some(p) = piles.iter_mut().find(|p| p.uid == uid) {
                p.count = count;
            }
            if piles.is_empty() {
                self.dropped.remove(map);
            }
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
    pub fn set_flag(&mut self, name: &str, value: bool) {
        if value {
            self.flags.insert(name.to_string());
        } else {
            self.flags.remove(name);
        }
    }
    /// A text variable ("" when unset).
    pub fn text(&self, name: &str) -> &str {
        self.texts.get(name).map_or("", String::as_str)
    }

    pub fn var(&self, name: &str) -> i64 {
        self.vars.get(name).copied().unwrap_or(0)
    }
    pub fn set_var(&mut self, name: &str, v: i64) {
        self.vars.insert(name.to_string(), v);
    }
    /// Items in the inventory plus equipped ones; a coin: how many of it the purse could pay.
    pub fn item_count(&self, item: &str) -> u32 {
        if let Some(w) = self.coin_worth(item) {
            return self.money_cp() / w;
        }
        self.inventory.count(item) + self.equipment.count(item)
    }
    /// The hero's first stack of `item`: worn first, then the bag.
    pub fn stack_of(&self, item: &str) -> Option<&ItemStack> {
        self.equipment.stacks().find(|s| s.item == item).or_else(|| self.inventory.slots.iter().flatten().find(|s| s.item == item))
    }

    /// [`WorldState::stack_of`], to change it.
    pub fn stack_of_mut(&mut self, item: &str) -> Option<&mut ItemStack> {
        if self.equipment.stacks().any(|s| s.item == item) {
            return self.equipment.stacks_mut().find(|s| s.item == item);
        }
        self.inventory.slots.iter_mut().flatten().find(|s| s.item == item)
    }

    /// Adds items to the player's inventory (coins: to the purse); returns how many did not fit.
    pub fn give_item(&mut self, item: &str, count: u32) -> u32 {
        if let Some(w) = self.coin_worth(item) {
            self.give_cp(count * w);
            return 0;
        }
        self.inventory.add(item, count)
    }
    /// Removes up to `count` items; returns how many were removed (coins: paid from the purse,
    /// as many as it holds).
    /// Takes from the inventory first, then from equipped items.
    pub fn take_item(&mut self, item: &str, count: u32) -> u32 {
        if let Some(w) = self.coin_worth(item) {
            self.bank_coins();
            let n = count.min(self.money / w);
            self.money -= n * w;
            return n;
        }
        let mut taken = self.inventory.remove(item, count);
        for slot in EquipSlot::ALL {
            if taken == count {
                break;
            }
            let s = self.equipment.slot_mut(slot);
            if let Some(st) = s.as_mut().filter(|st| st.item == item) {
                let n = (count - taken).min(st.count);
                st.count -= n;
                taken += n;
                if st.count == 0 {
                    *s = None;
                }
            }
        }
        taken
    }

    pub fn entity_state(&self, map: &str, uid: u32) -> Option<&EntityState> {
        self.entities.get(map)?.get(&uid)
    }
    pub fn set_entity_state(&mut self, map: &str, uid: u32, st: EntityState) {
        self.entities.entry(map.to_string()).or_default().insert(uid, st);
    }

    pub fn to_ron(&self) -> anyhow::Result<String> {
        crate::project::ron_text(self)
    }
    pub fn from_ron(s: &str) -> anyhow::Result<Self> {
        let mut w: WorldState = ron::from_str(s)?;
        let extra = w.dialog_log.len().saturating_sub(DIALOG_LOG_MAX);
        w.dialog_log.drain(..extra);
        Ok(w)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn conditions_change_checks_and_saves() {
        use crate::srd::{conditions, Condition, ConditionSpec, Mode};
        crate::srd::dice::seed(5);
        let mut w = WorldState::default();
        let plain = w.check("athletics", 10, Mode::Normal, false).unwrap();
        assert!(!plain.dice.contains('/'), "one d20: {}", plain.dice);
        conditions::apply(&mut w.effects, &ConditionSpec::new(Condition::Poisoned, 30.0));
        let poisoned = w.check("athletics", 10, Mode::Normal, false).unwrap();
        assert_eq!(poisoned.mode, Mode::Disadvantage, "two d20s, the lower: {}", poisoned.dice);
        assert_eq!(w.check("athletics", 10, Mode::Advantage, false).unwrap().mode, Mode::Normal, "they cancel out");
        assert_eq!(w.check("con", 10, Mode::Normal, true).unwrap().mode, Mode::Normal, "saves aren't poisoned");
        conditions::apply(&mut w.effects, &ConditionSpec::new(Condition::Stunned, 30.0));
        let dex = w.check("dex", 1, Mode::Normal, true).unwrap();
        assert_eq!(dex.success, Some(false), "stunned: DEX saves fail");
        assert_eq!(w.check("wis", 1, Mode::Normal, true).unwrap().success, Some(true));
        conditions::apply(&mut w.effects, &ConditionSpec::new(Condition::Blinded, 30.0));
        assert_eq!(w.check("perception", 1, Mode::Normal, false).unwrap().success, Some(false), "blinded: sight fails");
        // Saved and loaded again (and 0.27's effects read as conditions).
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.effects, w.effects);
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down, effects: [(kind: Poison, strength: 0.5, remaining: 4.0, accum: 0.2), (kind: Haste, strength: 30.0, remaining: 9.0)])"#).unwrap();
        assert!(conditions::has(&old.effects, Condition::Poisoned));
        assert_eq!(old.effects[0].remaining, Some(4.0));
    }

    /// Checks outside fights: light for sight, a helper, passive values, group checks, contests.
    #[test]
    fn checks_outside_fights() {
        use crate::srd::check::Sight;
        use crate::srd::{Abilities, Mode, StatBlock};
        crate::srd::dice::seed(3);
        let mut w = WorldState::default();
        // WIS 10, no proficiency: passive Perception 10; dim light -5 and disadvantage; darkness fails.
        assert_eq!(w.passive("perception"), Some(10));
        w.training.sight = Sight::Dim;
        assert_eq!(w.passive("perception"), Some(5));
        assert_eq!(w.check("perception", 1, Mode::Normal, false).unwrap().mode, Mode::Disadvantage);
        assert_eq!(w.check("athletics", 1, Mode::Normal, false).unwrap().mode, Mode::Normal, "only sight");
        w.training.sight = Sight::Dark;
        assert_eq!(w.passive("perception"), Some(0));
        assert_eq!(w.check("perception", 1, Mode::Normal, false).unwrap().success, Some(false));
        assert_eq!(w.check("wis", 1, Mode::Normal, true).unwrap().success, Some(true), "saves look at nothing");
        w.training.sight = Sight::Bright;
        assert_eq!(w.passive("nonsense"), None);
        // A companion who knows Athletics helps: advantage, named in the label.
        w.training.help.insert("athletics".into(), "Finn".into());
        let helped = w.check("athletics", 10, Mode::Normal, false).unwrap();
        assert_eq!((helped.mode, helped.label.as_str()), (Mode::Advantage, "Athletics (Finn helps)"));
        assert_eq!(w.passive("athletics"), Some(15), "advantage +5");
        w.training.help.clear();
        // Group checks: the hero (STR 30) and two who never make it (STR 1) at DC 15: 1 of 3 fails,
        // the hero with one of them (1 of 2) succeeds.
        w.abilities = Abilities([30, 10, 10, 10, 10, 10]);
        let weak = StatBlock { abilities: Abilities([1, 10, 10, 10, 10, 10]), ..Default::default() };
        let two = [("A".to_string(), weak.clone()), ("B".to_string(), weak.clone())];
        assert!(!w.group_check("athletics", 15, &two));
        assert_eq!(w.last_check.as_ref().unwrap().success, Some(false));
        assert!(w.group_check("athletics", 15, &two[..1]));
        assert!(w.rolls.iter().any(|r| r.label == "A: Athletics"), "companions' rolls in the log");
        // Contests: STR 30 against STR 1 always wins; the other way round never.
        assert!(w.contest("athletics", "Brom", &weak, ""));
        w.abilities = Abilities([1, 10, 10, 10, 10, 10]);
        let strong = StatBlock { abilities: Abilities([30, 10, 10, 10, 10, 10]), ..Default::default() };
        assert!(!w.contest("athletics", "Brom", &strong, ""));
        assert!(w.rolls.last().is_some_and(|r| r.label == "Brom: Athletics" && r.success.is_none()));
        assert!(!w.contest("athletics", "Brom", &strong, "nonsense"));
    }

    #[test]
    fn dropped_piles() {
        let mut w = WorldState::default();
        let (a, new_a) = w.add_dropped("village", "apple", 2, 100.0, 100.0, 16.0);
        // Same item close by joins the pile; another item or far away makes a new one.
        assert_eq!(w.add_dropped("village", "apple", 3, 105.0, 100.0, 16.0), (a, false));
        let (b, new_b) = w.add_dropped("village", "sword", 1, 100.0, 100.0, 16.0);
        assert!(new_a && new_b && a != b && a >= DROPPED_UID_BASE);
        assert_eq!(w.dropped["village"][0].count, 5);
        w.set_dropped_count("village", a, 1);
        assert_eq!(w.dropped["village"][0].count, 1);
        w.set_dropped_count("village", a, 0);
        w.set_dropped_count("village", b, 0);
        assert!(!w.dropped.contains_key("village"));
        // Piles of old saves move out of the spawned characters' uid range.
        let mut old = WorldState::default();
        old.dropped.insert("m".into(), vec![DroppedItem { uid: 0x4000_0000, item: "coin".into(), count: 1, spent: 0, x: 0.0, y: 0.0 }]);
        old.renumber_old_drops();
        assert_eq!((old.dropped["m"][0].uid, old.next_drop), (DROPPED_UID_BASE, 1));
        // Piles survive a save / load round trip.
        w.add_dropped("forest", "coin", 7, 1.0, 2.0, 16.0);
        let back: WorldState = ron::from_str(&ron::to_string(&w).unwrap()).unwrap();
        assert_eq!(back.dropped["forest"][0].count, 7);
        assert_eq!(back.next_drop, w.next_drop);
    }

    #[test]
    fn placed_furniture() {
        let mut w = WorldState::default();
        let a = w.add_furniture("home", "chest", "chest_item", 48.0, 80.0);
        let b = w.add_furniture("home", "chair", "chair_item", 80.0, 80.0);
        assert!(a >= FURNITURE_UID_BASE && b == a + 1 && b < DROPPED_UID_BASE);
        // Its saved state and contents go with it when it is picked up.
        w.entities.entry("home".into()).or_default().insert(a, EntityState { active: true, frame: 0, x: 48.0, y: 80.0, facing: Facing::Down, count: None, unlocked: false, state: Some("open".into()), patrol: None, schedule_off: false, changed_at: None, planted: None, faction: None, hp: None });
        w.containers.entry("home".into()).or_default().insert(a, Inventory::new(4));
        assert_eq!(w.remove_furniture("home", a).map(|f| f.prop), Some("chest".to_string()));
        assert!(!w.entities["home"].contains_key(&a) && !w.containers["home"].contains_key(&a));
        assert!(w.remove_furniture("home", a).is_none());
        w.remove_furniture("home", b);
        assert!(!w.furniture.contains_key("home"));
        // A new piece never reuses a uid.
        assert_eq!(w.add_furniture("home", "rug", "rug_item", 0.0, 0.0), b + 1);
    }

    use super::*;

    #[test]
    fn equipment_counts_and_takes() {
        let mut w = WorldState::default();
        w.give_item("sword", 1);
        w.equipment.weapon = Some(ItemStack::new("sword", 1));
        assert_eq!(w.item_count("sword"), 2);
        assert_eq!(w.take_item("sword", 2), 2);
        assert!(w.equipment.weapon.is_none() && w.inventory.is_empty());
        // Old saves without `equipment` still load; `equipped` is kept for migration.
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {"bow": 1}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down, equipped: "bow")"#).unwrap();
        assert_eq!((old.equipped.as_str(), old.equipment.weapon.is_none()), ("bow", true));
        // Saves from before experience and levels start at level 1 without XP.
        assert_eq!((old.xp, old.player_level()), (0, 1));
        assert!(old.effects.is_empty() && old.bestiary.is_empty() && old.dialog_log.is_empty());
        let mut w = WorldState { xp: 250, level: 3, ..Default::default() };
        crate::srd::conditions::apply(&mut w.effects, &crate::srd::ConditionSpec::new(crate::srd::Condition::Poisoned, 12.5));
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!((back.xp, back.player_level()), (250, 3));
        assert_eq!(back.effects, w.effects);
        let w = WorldState { bestiary: [("slime".to_string(), 4)].into(), ..Default::default() };
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!((back.defeated("slime"), back.defeated("goblin")), (4, 0));
        // A save of 0.31 and older with its quick bar still loads (the quick bar is gone).
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down, quickbar: ["potion", "", "spell:cure-wounds"])"#).unwrap();
        assert_eq!(old.current_map, "m");
    }

    #[test]
    fn weather_overrides_are_saved() {
        use crate::map::{Weather, WeatherKind};
        let old: WorldState = ron::from_str(r#"(flags: [], vars: {}, inventory: {}, entities: {}, fired: [], current_map: "m", player_pos: (0.0, 0.0), player_facing: Down)"#).unwrap();
        assert!(old.weather.is_empty(), "saves from before weather load");
        let mut w = WorldState::default();
        w.weather.insert("forest".into(), Weather::new(WeatherKind::Rain, 0.8));
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.weather.get("forest"), Some(&Weather::new(WeatherKind::Rain, 0.8)));
    }

    #[test]
    fn dialog_log_keeps_the_newest_lines() {
        let mut w = WorldState::default();
        w.log_dialog("Bob", "  ", false);
        assert!(w.dialog_log.is_empty(), "empty text is not logged");
        for i in 0..DIALOG_LOG_MAX + 5 {
            w.log_dialog("Bob", &format!("line {i}"), false);
        }
        w.log_dialog("Alex", "Yes", true);
        assert_eq!(w.dialog_log.len(), DIALOG_LOG_MAX);
        assert_eq!(w.dialog_log[0].text, "line 6");
        assert_eq!(w.dialog_log.last().unwrap(), &LogLine { speaker: "Alex".into(), text: "Yes".into(), answer: true });
        // Saved and loaded; an over-long log (older versions, edited saves) is trimmed on load.
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.dialog_log, w.dialog_log);
        let mut long = w.clone();
        long.dialog_log.extend(w.dialog_log.iter().take(50).cloned());
        assert_eq!(WorldState::from_ron(&long.to_ron().unwrap()).unwrap().dialog_log.len(), DIALOG_LOG_MAX);
    }

    #[test]
    fn money_is_a_purse_coins_go_into() {
        let mut w = WorldState { coins: vec![("pp".into(), 1000), ("coin".into(), 100), ("ep".into(), 50), ("sp".into(), 10), ("cp".into(), 1)], ..Default::default() };
        // Coins given go into the purse, not the bag.
        w.give_item("coin", 1);
        w.give_item("sp", 12);
        w.give_item("cp", 5);
        assert_eq!((w.money_cp(), w.money, w.inventory.used_slots()), (225, 225, 0));
        assert!(!w.pay_cp(300), "short: nothing taken");
        assert_eq!(w.money_cp(), 225);
        assert!(w.pay_cp(200));
        assert_eq!(w.money_cp(), 25);
        // A coin asked for by its id: what the purse could pay, taken from it.
        w.give_cp(1000);
        assert_eq!((w.item_count("coin"), w.item_count("sp")), (10, 102));
        assert_eq!(w.take_item("coin", 3), 3);
        assert_eq!(w.money_cp(), 725);
        assert_eq!(w.take_item("pp", 1), 0, "the purse can't pay a platinum piece");
        // Coins that reached the bag another way (a pile picked up, an old save) are banked.
        w.inventory.add("sp", 7);
        w.inventory.add("rope", 1);
        assert_eq!(w.bank_coins(), 70);
        assert_eq!((w.money, w.inventory.count("sp"), w.inventory.count("rope")), (795, 0, 1));
    }

    #[test]
    fn inventory_and_save_roundtrip() {
        let mut w = WorldState::default();
        w.give_item("coin", 3);
        assert_eq!(w.take_item("coin", 5), 3);
        assert_eq!(w.item_count("coin"), 0);
        assert!(w.inventory.is_empty());
        w.set_flag("a", true);
        w.set_var("x", -4);
        w.set_entity_state(
            "village",
            7,
            EntityState { active: false, frame: 1, x: 1.0, y: 2.0, facing: Facing::Up, count: None, unlocked: true, state: Some("open".into()), patrol: None, schedule_off: false, changed_at: Some(30.0), planted: None, faction: None, hp: Some(7) },
        );
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert!(back.flag("a"));
        assert_eq!(back.var("x"), -4);
        assert_eq!(back.entity_state("village", 7).unwrap().frame, 1);
    }

    #[test]
    fn entity_states_of_old_saves_follow_their_schedule() {
        let old: EntityState = ron::from_str("(active: true, frame: 0, x: 1.0, y: 2.0, facing: Down)").unwrap();
        assert!(!old.schedule_off);
        let off = EntityState { schedule_off: true, ..old.clone() };
        let text = ron::to_string(&off).unwrap();
        assert_eq!(ron::from_str::<EntityState>(&text).unwrap(), off);
        assert!(!ron::to_string(&old).unwrap().contains("schedule_off"), "only written when stopped");
    }

    #[test]
    fn companions_in_saves() {
        // A save without companions (all older ones) loads with none.
        let old = WorldState::default().to_ron().unwrap();
        assert!(!old.contains("companions"));
        assert!(WorldState::from_ron(&old).unwrap().companions.is_empty());
        let mut w = WorldState::default();
        w.companions.push(Companion { ai: false, map: "village".into(), uid: 22, name: "guard".into(), hp: Some(5), down: None, order: Order::Defend, post: Some([100.0, 200.0]), mana: None, ..Default::default() });
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.companions, w.companions);
        assert!(back.is_companion("village", 22) && !back.is_companion("forest", 22));
        // Companions saved before they could be knocked down are on their feet.
        let c: Companion = ron::from_str(r#"(map: "village", uid: 22, name: "guard", hp: Some(5))"#).unwrap();
        assert_eq!(c.down, None);
        let mut down = c.clone();
        down.down = Some([272.0, 590.0]);
        assert_eq!(ron::from_str::<Companion>(&ron::to_string(&down).unwrap()).unwrap(), down);
        assert!(!ron::to_string(&c).unwrap().contains("down"));
        // ... and follow with the Attack order.
        assert_eq!((c.order, c.post), (Order::Attack, None));
        assert!(!ron::to_string(&c).unwrap().contains("order"));
    }

    #[test]
    fn tool_slot_in_saves() {
        // Saves from before the Tool slot have no "tool" field.
        let old: Equipment = ron::from_str("(head: None, body: None, weapon: Some((item: \"sword\", count: 1)), offhand: None)").unwrap();
        assert_eq!(old.get(EquipSlot::Tool), None);
        let mut eq = old.clone();
        *eq.slot_mut(EquipSlot::Tool) = Some(ItemStack::new("fishing_rod", 1));
        let back: Equipment = ron::from_str(&ron::to_string(&eq).unwrap()).unwrap();
        assert_eq!(back.item(EquipSlot::Tool), Some("fishing_rod"));
        assert_eq!(back.count("fishing_rod"), 1, "tools count as owned");
        assert!(!ron::to_string(&old).unwrap().contains("tool"), "no tool, nothing written");
        assert_eq!(EquipSlot::parse("tool"), Some(EquipSlot::Tool));
    }

    #[test]
    fn back_slot_in_saves() {
        // Saves from before backpacks have no "back" field, and none is written without one.
        let old: Equipment = ron::from_str("(head: None, body: None, weapon: None, offhand: None)").unwrap();
        assert_eq!(old.get(EquipSlot::Back), None);
        assert!(!ron::to_string(&old).unwrap().contains("back"));
        let mut eq = old;
        *eq.slot_mut(EquipSlot::Back) = Some(ItemStack::new("small_backpack", 1));
        let again: Equipment = ron::from_str(&ron::to_string(&eq).unwrap()).unwrap();
        assert_eq!(again.item(EquipSlot::Back), Some("small_backpack"));
        assert_eq!(EquipSlot::parse("backpack"), Some(EquipSlot::Back));
    }
}

#[cfg(test)]
mod day_tests {
    use super::*;

    #[test]
    fn days_pass_with_the_clock() {
        // Saves from before days: day 0, entity states without a change time.
        let mut w = WorldState { hour: 22.5, ..Default::default() };
        w.day = 7;
        let ron = w.to_ron().unwrap().replace("    day: 7,\n", "");
        assert!(!ron.contains("day:"), "{ron}");
        let old = WorldState::from_ron(&ron).unwrap();
        assert_eq!((old.day, old.hour), (0, 22.5));
        let st: EntityState = ron::from_str("(active: true, frame: 0, x: 1.0, y: 2.0, facing: Down)").unwrap();
        assert_eq!(st.changed_at, None);
        let mut w = old;
        w.advance_hours(3.0);
        assert_eq!((w.day, w.hour), (1, 1.5));
        w.advance_hours(72.0);
        assert_eq!((w.day, w.hour), (4, 1.5));
        assert_eq!(w.total_hours(), 97.5);
        let back = WorldState::from_ron(&w.to_ron().unwrap()).unwrap();
        assert_eq!(back.day, 4);
    }
}
