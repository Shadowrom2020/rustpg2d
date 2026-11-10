//! Event-driven interactions.
//!
//! Gameplay emits [`GameEvent`]s. Entities, zones, maps and items own [`Trigger`]s
//! that react to matching events: if all [`Condition`]s hold, the trigger's
//! [`Action`]s are queued on the [`runner::ActionRunner`], which executes them in
//! order (some, like dialogs, block the sequence until finished).

pub mod runner;

use serde::{Deserialize, Serialize};

use crate::entity::Facing;
use crate::state::WorldState;

/// Something that happened in the game.
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    /// Player pressed the interact key facing an entity or standing in a zone.
    Interact,
    /// Player entered / left a zone.
    Enter,
    Leave,
    /// Player used an item (on the entity in front of them, or on nothing).
    UseItem { item: String },
    /// The map finished loading and the player was placed.
    MapEnter,
    /// User-defined event raised by an `Emit` action or `emit()` in scripts.
    Custom { name: String },
    /// An entity's hit points dropped to zero.
    Defeat,
    /// A prop's lock was opened.
    Unlock,
    /// An opened prop was locked again.
    Lock,
    /// A character's HP dropped from `from` to `to` (fractions of its max HP, still alive).
    HpBelow { from: f32, to: f32 },
    /// The player woke up after sleeping (in a bed, or the Sleep action).
    Sleep,
    /// The clock moved forward from `from` to `to` (hours of the day; `to` < `from` when it
    /// passed midnight).
    Hour { from: f32, to: f32 },
}

/// Which events a trigger reacts to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TriggerOn {
    Interact,
    Enter,
    Leave,
    /// `item` empty = any item.
    UseItem { item: String },
    MapEnter,
    Custom { name: String },
    /// The owning character was defeated in combat.
    Defeat,
    /// The owning prop's lock was opened with its key (or by a Set locked action).
    Unlock,
    /// The owning prop was locked again (by the player with its key, or by a Set locked action).
    Lock,
    /// The owning character's HP fell below `percent` of its max HP (and it is still alive).
    HpBelow { percent: u32 },
    /// The player woke up after sleeping (map triggers; a bed's own triggers when slept in).
    Sleep,
    /// The clock reached `hour` (0-24, e.g. 12.0 = noon, 5.5 = half past five) while the player
    /// is on the map: also when time jumps forward past it (sleeping, Set time). Map triggers.
    Hour { hour: f32 },
}

/// Where a trigger lives. The game only sends some events to each kind of owner (see
/// `runtime::Game::collect`), so only those make sense in its trigger list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerOwner {
    /// Triggers of a map (not of an object on it).
    Map,
    Zone,
    /// A placed character or a character definition.
    Character,
    /// A placed prop or a prop definition.
    Prop,
    /// An item pickup placed on a map.
    Pickup,
    /// An item definition's "on use" triggers.
    ItemUse,
}

impl TriggerOwner {
    /// Whether events of this kind ever reach a trigger of this owner.
    pub fn allows(self, on: &TriggerOn) -> bool {
        use TriggerOn as T;
        use TriggerOwner as O;
        match (self, on) {
            // emit() / "Emit event" reach the map, its zones and its entities.
            (O::Map | O::Zone | O::Character | O::Prop | O::Pickup, T::Custom { .. }) => true,
            (O::Map, T::MapEnter | T::Sleep | T::Hour { .. }) => true,
            (O::Zone, T::Enter | T::Leave | T::Interact) => true,
            (O::Character, T::Interact | T::UseItem { .. } | T::Defeat | T::HpBelow { .. }) => true,
            (O::Prop, T::Interact | T::UseItem { .. } | T::Unlock | T::Lock | T::Sleep) => true,
            (O::Pickup, T::Interact | T::UseItem { .. }) => true,
            (O::ItemUse, T::UseItem { .. }) => true,
            _ => false,
        }
    }

    /// The events offered for this owner (first = default for new triggers).
    pub fn templates(self) -> Vec<TriggerOn> {
        let mut v: Vec<TriggerOn> = TriggerOn::templates().into_iter().filter(|t| self.allows(t)).collect();
        // The most likely event first.
        let first = match self {
            TriggerOwner::Map => TriggerOn::MapEnter,
            TriggerOwner::Zone => TriggerOn::Enter,
            TriggerOwner::ItemUse => TriggerOn::UseItem { item: String::new() },
            _ => TriggerOn::Interact,
        };
        if let Some(k) = v.iter().position(|t| *t == first) {
            let t = v.remove(k);
            v.insert(0, t);
        }
        v
    }

    /// What to use instead of an event that never reaches this owner.
    pub fn replacement(self, on: &TriggerOn) -> TriggerOn {
        match (self, on) {
            (TriggerOwner::Map, _) => TriggerOn::MapEnter,
            (TriggerOwner::Zone, TriggerOn::MapEnter) => TriggerOn::Enter,
            _ => self.templates().remove(0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TriggerOwner::Map => "map triggers",
            TriggerOwner::Zone => "zone triggers",
            TriggerOwner::Character => "character triggers",
            TriggerOwner::Prop => "prop triggers",
            TriggerOwner::Pickup => "item pickup triggers",
            TriggerOwner::ItemUse => "item use triggers",
        }
    }
}

impl TriggerOn {
    pub fn matches(&self, ev: &GameEvent) -> bool {
        match (self, ev) {
            (TriggerOn::Interact, GameEvent::Interact)
            | (TriggerOn::Enter, GameEvent::Enter)
            | (TriggerOn::Leave, GameEvent::Leave)
            | (TriggerOn::MapEnter, GameEvent::MapEnter)
            | (TriggerOn::Defeat, GameEvent::Defeat)
            | (TriggerOn::Unlock, GameEvent::Unlock)
            | (TriggerOn::Lock, GameEvent::Lock)
            | (TriggerOn::Sleep, GameEvent::Sleep) => true,
            (TriggerOn::UseItem { item: want }, GameEvent::UseItem { item }) => {
                want.is_empty() || want == item
            }
            (TriggerOn::Custom { name: want }, GameEvent::Custom { name }) => want == name,
            // Fires when the HP crosses the threshold on its way down.
            (TriggerOn::HpBelow { percent }, GameEvent::HpBelow { from, to }) => {
                let p = *percent as f32 / 100.0;
                *from >= p && *to < p
            }
            // The hour lies in the stretch the clock just moved through (`from` excluded).
            (TriggerOn::Hour { hour }, GameEvent::Hour { from, to }) => {
                let h = hour.rem_euclid(24.0);
                if from <= to {
                    *from < h && h <= *to
                } else {
                    h > *from || h <= *to
                }
            }
            _ => false,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TriggerOn::Interact => "Interact",
            TriggerOn::Enter => "Enter",
            TriggerOn::Leave => "Leave",
            TriggerOn::UseItem { .. } => "Use item",
            TriggerOn::MapEnter => "Map enter",
            TriggerOn::Custom { .. } => "Custom event",
            TriggerOn::Defeat => "Defeated",
            TriggerOn::Unlock => "Unlocked",
            TriggerOn::Lock => "Locked again",
            TriggerOn::HpBelow { .. } => "HP below %",
            TriggerOn::Sleep => "Woke up",
            TriggerOn::Hour { .. } => "Time reached",
        }
    }

    pub fn templates() -> Vec<TriggerOn> {
        vec![
            TriggerOn::Interact,
            TriggerOn::Enter,
            TriggerOn::Leave,
            TriggerOn::UseItem { item: String::new() },
            TriggerOn::MapEnter,
            TriggerOn::Custom { name: String::new() },
            TriggerOn::Defeat,
            TriggerOn::Unlock,
            TriggerOn::Lock,
            TriggerOn::HpBelow { percent: 50 },
            TriggerOn::Sleep,
            TriggerOn::Hour { hour: 12.0 },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    pub on: TriggerOn,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub actions: Vec<Action>,
    /// Fire at most once per save game.
    #[serde(default)]
    pub once: bool,
}

impl Trigger {
    pub fn new(on: TriggerOn) -> Self {
        Trigger { on, conditions: Vec::new(), actions: Vec::new(), once: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub const ALL: [CmpOp; 6] = [CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];
    pub fn apply(self, a: i64, b: i64) -> bool {
        match self {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Cond {
    Flag { name: String },
    Var { name: String, op: CmpOp, value: i64 },
    HasItem { item: String, count: u32 },
    /// Calls a Rhai function returning `bool`.
    Script { function: String },
    PlayerHp { op: CmpOp, value: i64 },
    /// A prop is in the given state.
    PropState { target: TargetName, state: String },
    /// A quest's status / stage (`stage` is used by the stage checks).
    Quest { quest: String, check: crate::quest::QuestCheck, stage: u32 },
    /// The player character is male / female.
    PlayerSex { sex: crate::state::Sex },
    /// The time of day is between `from` and `to` hours (wraps around midnight, e.g. 20 to 6).
    TimeBetween { from: f32, to: f32 },
    /// A skill's level compared with a value.
    Skill { skill: String, op: CmpOp, level: i64 },
    /// The player has learned the spell.
    KnowsSpell { spell: String },
    /// The player's level (experience and levels) compared with a value.
    Level { op: CmpOp, value: i64 },
    /// The player has a status effect.
    HasEffect { kind: crate::effects::EffectKind },
    /// How many characters of a definition the player has defeated, compared with a number.
    Defeated { character: String, op: CmpOp, count: i64 },
    /// How a character (or "player") rests on furniture: "sit", "lie", "stand"; "" = on its feet.
    Pose { target: TargetName, pose: String },
}

/// Whether `hour` lies in `from..to` (hours 0..24; `from > to` wraps around midnight).
pub fn hour_between(hour: f32, from: f32, to: f32) -> bool {
    let h = hour.rem_euclid(24.0);
    if from <= to {
        h >= from && h < to
    } else {
        h >= from || h < to
    }
}

impl Cond {
    pub fn label(&self) -> &'static str {
        match self {
            Cond::Flag { .. } => "Flag set",
            Cond::Var { .. } => "Variable",
            Cond::HasItem { .. } => "Has item",
            Cond::Script { .. } => "Script",
            Cond::PlayerHp { .. } => "Player HP",
            Cond::PropState { .. } => "Prop state",
            Cond::Quest { .. } => "Quest",
            Cond::PlayerSex { .. } => "Player sex",
            Cond::TimeBetween { .. } => "Time of day",
            Cond::Skill { .. } => "Skill level",
            Cond::KnowsSpell { .. } => "Knows spell",
            Cond::Level { .. } => "Player level",
            Cond::HasEffect { .. } => "Player has effect",
            Cond::Defeated { .. } => "Defeated count",
            Cond::Pose { .. } => "Pose (sitting, asleep)",
        }
    }
    pub fn templates() -> Vec<Cond> {
        vec![
            Cond::Flag { name: String::new() },
            Cond::Var { name: String::new(), op: CmpOp::Eq, value: 0 },
            Cond::HasItem { item: String::new(), count: 1 },
            Cond::Script { function: String::new() },
            Cond::PlayerHp { op: CmpOp::Lt, value: 3 },
            Cond::PropState { target: "self".into(), state: String::new() },
            Cond::Quest { quest: String::new(), check: crate::quest::QuestCheck::Active, stage: 0 },
            Cond::PlayerSex { sex: crate::state::Sex::Female },
            Cond::TimeBetween { from: 20.0, to: 6.0 },
            Cond::Skill { skill: String::new(), op: CmpOp::Ge, level: 1 },
            Cond::KnowsSpell { spell: String::new() },
            Cond::Level { op: CmpOp::Ge, value: 2 },
            Cond::HasEffect { kind: crate::effects::EffectKind::Poison },
            Cond::Defeated { character: String::new(), op: CmpOp::Ge, count: 10 },
            Cond::Pose { target: "self".into(), pose: "lie".into() },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub cond: Cond,
    #[serde(default)]
    pub negate: bool,
}

impl Condition {
    pub fn new(cond: Cond) -> Self {
        Condition { cond, negate: false }
    }

    pub fn eval(&self, world: &WorldState, script: &mut dyn FnMut(&str) -> bool) -> bool {
        let v = match &self.cond {
            Cond::Flag { name } => world.flag(name),
            Cond::Var { name, op, value } => op.apply(world.var(name), *value),
            Cond::HasItem { item, count } => world.item_count(item) >= *count,
            Cond::Script { function } => script(function),
            Cond::PlayerHp { op, value } => op.apply(world.player_hp.unwrap_or(0) as i64, *value),
            // Needs the running game (see `conditions_met`).
            Cond::PropState { .. } => false,
            Cond::Quest { quest, check, stage } => world.check_quest(quest, *check, *stage),
            Cond::PlayerSex { sex } => world.player_sex == *sex,
            Cond::TimeBetween { from, to } => hour_between(world.hour, *from, *to),
            Cond::Skill { skill, op, level } => op.apply(world.skill_level(skill) as i64, *level),
            Cond::KnowsSpell { spell } => world.spells.contains(spell),
            Cond::Level { op, value } => op.apply(world.player_level() as i64, *value),
            Cond::HasEffect { kind } => world.effects.iter().any(|e| e.kind == *kind),
            Cond::Defeated { character, op, count } => op.apply(world.defeated(character) as i64, *count),
            // Needs the running game (see `conditions_met`).
            Cond::Pose { .. } => false,
        };
        v != self.negate
    }
}

pub fn all_met(
    conds: &[Condition],
    world: &WorldState,
    script: &mut dyn FnMut(&str) -> bool,
) -> bool {
    conds.iter().all(|c| c.eval(world, script))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarOp {
    Set,
    #[default]
    Add,
}

/// Entity addressed by an action: `"self"` (trigger owner), `"player"`, or an entity name.
pub type TargetName = String;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Show a text box and wait for the player to confirm.
    Dialog { speaker: String, text: String },
    /// Ask a question; the chosen option index (0-based) is stored in variable `var`.
    Choice { speaker: String, text: String, options: Vec<String>, var: String },
    GiveItem { item: String, count: u32 },
    TakeItem { item: String, count: u32 },
    SetFlag { name: String, value: bool },
    SetVar { name: String, op: VarOp, value: i64 },
    /// Move the player to a spawn point (on another map or the current one).
    Teleport { map: String, spawn: String },
    /// Show/hide an entity (hidden entities don't collide or interact). Persisted.
    SetActive { target: TargetName, active: bool },
    /// Change a prop's frame offset (e.g. open a chest). Persisted.
    SetFrame { target: TargetName, frame: u32 },
    /// Walk an entity by a tile offset; blocks until it arrives.
    Move { target: TargetName, dx: i32, dy: i32 },
    Face { target: TargetName, facing: Facing },
    Wait { seconds: f32 },
    /// Raise a custom event; matching triggers run before the rest of this sequence.
    Emit { name: String },
    /// Call a Rhai function from the project's scripts.
    RunScript { function: String },
    If { conditions: Vec<Condition>, then: Vec<Action>, otherwise: Vec<Action> },
    PlaySound { sound: String },
    /// Switch the looping music (empty = stop).
    PlayMusic { sound: String },
    /// Restore (or with a negative amount, remove) player hit points.
    Heal { amount: i32 },
    /// Locks or unlocks a prop without a key (unlocking applies its "on unlock" effect).
    SetLocked { target: TargetName, locked: bool },
    /// Switches a prop to one of its states (sprite and whether it blocks movement).
    SetState { target: TargetName, state: String },
    /// Opens the trade window of a merchant character.
    OpenShop { target: TargetName },
    /// Starts / advances / completes / fails a quest (`stage` is used by `SetStage`).
    Quest { quest: String, op: crate::quest::QuestOp, stage: u32 },
    /// Starts (the character's own route, or `points` in tile coordinates) or stops a
    /// character's patrol.
    Patrol {
        target: TargetName,
        active: bool,
        #[serde(default)]
        points: Vec<(i32, i32)>,
    },
    /// Sets the time of day (hours, 0..24).
    SetTime { hour: f32 },
    /// Skill points the player can spend (negative takes them away).
    GiveSkillPoints { amount: i32 },
    /// Sets a skill to a level directly (no points spent).
    SetSkill { skill: String, level: u32 },
    /// Teaches a spell.
    LearnSpell { spell: String },
    /// Mana (negative = drain).
    AddMana { amount: i32 },
    /// Damages every character within `radius` tiles of the player (spell effects).
    DamageAround { damage: i32, radius: f32 },
    /// A light around the player for a while (radius in tiles).
    PlayerLight {
        radius: f32,
        seconds: f32,
        #[serde(default = "spell_light_color")]
        color: [u8; 3],
        #[serde(default)]
        flicker: bool,
    },
    /// Starts magic targeting: the player's next click (an enemy or a spot) calls the Rhai
    /// function `function(x, y)` with the target in tile coordinates (X: straight ahead,
    /// right-click / Esc cancel). Started by a spell's script, the spell's mana and cooldown are
    /// only spent once a target is chosen.
    Aim { function: String },
    /// An animated sprite flies from the player to tile point (`x`, `y`) at `speed` tiles per
    /// second, then calls `function(x, y)` (if set).
    Fly { sprite: FxSprite, x: f32, y: f32, speed: f32, function: String },
    /// Plays an animated sprite once, centered on tile point (`x`, `y`) (explosions, sparkles).
    Effect { sprite: FxSprite, x: f32, y: f32 },
    /// Damages every character within `radius` tiles of tile point (`x`, `y`) (scaled by the
    /// player's spell damage skills).
    DamageArea { x: f32, y: f32, radius: f32, damage: i32 },
    /// A light at tile point (`x`, `y`) for `seconds` (radius in tiles; fades out at the end).
    LightAt { x: f32, y: f32, radius: f32, seconds: f32, color: [u8; 3], flicker: bool },
    /// Heals the player and friendly characters within `radius` tiles of tile point (`x`, `y`).
    HealArea { x: f32, y: f32, radius: f32, amount: i32 },
    /// Puts an item into its slot (clothing, armor, weapon, left hand) on a character; what it
    /// wore there goes to its carried items. The player takes it from the inventory (or gets it).
    Equip { target: TargetName, item: String },
    /// Takes off what a character wears / holds in a slot (into its carried items).
    Unequip { target: TargetName, slot: crate::defs::EquipSlot },
    /// Adds items to what a character carries (the player: the inventory).
    GiveItemTo { target: TargetName, item: String, count: u32 },
    /// Takes items from what a character carries.
    TakeItemFrom { target: TargetName, item: String, count: u32 },
    /// Hides or shows the game's HUD (status frame, clock, quick bar, buttons), e.g. for a
    /// cut-scene; dialogs and windows the player opens still show. The player toggles it with H.
    SetUiVisible { visible: bool },
    /// Plays the Rhai function `function` as a cut-scene: the HUD is hidden, black bars slide
    /// in, the player and the other characters wait while its actions run (walking, dialogs,
    /// waits, ...); afterwards everything is as before.
    Cutscene { function: String },
    /// Ends a cut-scene (queued after its actions by `Cutscene`).
    EndCutscene,
    /// Walks a character (or the player) to the middle of tile (`x`, `y`); blocks until it arrives.
    WalkTo { target: TargetName, x: i32, y: i32 },
    /// Walks a character (or the player) up to another one (`to`), stopping a tile before it;
    /// then both face each other. Blocks until it arrives.
    Approach { target: TargetName, to: TargetName },
    /// Puts a character (or the player) on tile (`x`, `y`) at once (no walking).
    Place { target: TargetName, x: i32, y: i32 },
    /// Darkens the scene to `amount` (0 = normal, 1 = deep night) over `seconds`, e.g. so that
    /// lights show in a cut-scene by day. Not saved (loading resets it); a cut-scene's end fades
    /// back to normal.
    Dim { amount: f32, seconds: f32 },
    /// Switches the enemy spawners of a map off (or on again): `zone` = one spawner zone by
    /// name, "" = all of them; `map` "" = the current map. Their characters on the current map
    /// vanish. Saved.
    SetSpawners { map: String, zone: String, active: bool },
    /// Defeats every hostile character on the current map at once (with their loot, dropped
    /// items and *Defeated* triggers, like a defeat in combat).
    DefeatEnemies,
    /// Saves the game to the autosave slot as soon as the running event (dialogs, cut-scene)
    /// has finished.
    Autosave,
    /// Gives the player experience (negative takes it away; the level never drops).
    GiveXp { amount: i64 },
    /// Starts a status effect on the player or a character.
    ApplyEffect { target: TargetName, effect: crate::effects::EffectSpec },
    /// Ends a status effect (`None` = all of them) on the player or a character.
    CureEffect { target: TargetName, kind: Option<crate::effects::EffectKind> },
    /// Shakes the screen (`strength` 1 = like a hit) - earthquakes, explosions, a boss landing.
    Shake { seconds: f32, strength: f32 },
    /// Weather of a map ("" = the current one); `None` = back to the map's own weather. Kept
    /// in the save game.
    SetWeather { map: String, weather: Option<crate::map::Weather> },
    /// Makes a character follow its daily schedule again (`active`) or stop following it.
    Schedule { target: TargetName, active: bool },
    /// A placed character joins the player as a companion (`join`) or leaves (goes back to
    /// where it is placed).
    Companion { target: TargetName, join: bool },
    /// Opens the crafting window with the recipes of `station` (`title` "" = "Crafting").
    OpenCrafting { station: String, title: String },
    /// Unlocks an achievement (the player's, kept across save games).
    UnlockAchievement { id: String },
    /// The lockpicking game for a custom puzzle: 1 (picked) or 0 goes into variable `var`;
    /// blocks until it is over. `difficulty` like a lock's (the player's skill makes it easier).
    Lockpick { difficulty: u32, var: String },
    /// The player sleeps `hours` (camping, an inn's "Rest"): the screen fades, the clock jumps,
    /// HP and mana are restored; blocks until the player is awake again.
    Sleep { hours: f32 },
    /// Heals a character (negative = hurts it): `"player"`, `"self"` or a character's name. On
    /// an item's *on use* the player's choice of target replaces a plain *Heal player*.
    HealCharacter { target: TargetName, amount: i32 },
    /// Tells a companion (or "all" of them) what to do: "attack", "defend", "passive" (how it
    /// fights), "stay" (wait here) or "follow".
    CompanionOrder { target: TargetName, order: String },
    /// Opens a companion's window (its gear and bag next to the player's inventory) once the
    /// running event is over.
    OpenCompanion { target: TargetName },
    /// Sets the mood a character's portrait shows (one of its definition's mood portraits; ""
    /// = its usual one) until changed or the running event is over. `target`: a character on
    /// the map, or a character definition's id or name.
    Mood { target: TargetName, mood: String },
    /// Shows a picture over the game: a sprite sheet drawn whole, at `scale` (1 = its own
    /// pixels) or scaled down to fit the window, aspect ratio kept. `scroll`: a picture taller
    /// than the window is fitted to its width and pans down and back up at `speed` (screen
    /// pixels per second); the player can scroll it too. `wait`: the event waits until the player
    /// closes it; otherwise it stays under the following dialogs until *Hide image* or the end of
    /// the event.
    ShowImage {
        sheet: String,
        #[serde(default = "yes")]
        wait: bool,
        #[serde(default)]
        scroll: bool,
        #[serde(default = "image_speed")]
        speed: f32,
        #[serde(default = "one_f32")]
        scale: f32,
    },
    /// Takes a picture of *Show image* away.
    HideImage,
    /// Changes a merchant's stock (`target`: a merchant on the map): *Add* `count` of `item` (a
    /// new line at `price`, 0 = the item's value), or *Set* it to `count` (0 removes the line).
    /// What villagers produce reaches their stall this way. Kept in save games.
    ShopStock {
        target: TargetName,
        item: String,
        #[serde(default)]
        op: VarOp,
        count: u32,
        #[serde(default)]
        price: u32,
    },
    /// Teaches a character a spell (`target`: a companion, or any character on the map, which
    /// casts it once it follows the player). Companions cast *Heal* and *Bolt* spells from their
    /// definition's mana.
    TeachSpell { target: TargetName, spell: String },
    /// Shows an emotion over a character's head (`player` too): a tag of an *Emotions* sprite
    /// sheet. `seconds` 0 = 2.5 s, below 0 = until cleared; `wait`: the event goes on when it is over.
    Emote { target: TargetName, emotion: String, #[serde(default)] seconds: f32, #[serde(default)] wait: bool },
    /// Takes a character's emotion away.
    ClearEmote { target: TargetName },
}

fn yes() -> bool {
    true
}

/// Default speed of a scrolling picture (screen pixels per second).
pub fn image_speed() -> f32 {
    40.0
}

fn one_f32() -> f32 {
    1.0
}

/// Default color of spell lights (a cool white-blue).
pub fn spell_light_color() -> [u8; 3] {
    [190, 210, 255]
}

/// An animated sprite for the `Fly` / `Effect` actions: frames `first..=last` of a sprite sheet
/// (both 0 = every frame of the sheet).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxSprite {
    pub sheet: String,
    pub first: u32,
    pub last: u32,
    /// Frames per second.
    pub fps: f32,
    /// Size factor (1 = the sheet's frame size).
    pub scale: f32,
    /// A light around it (radius in tiles, 0 = none); explosions' lights fade out.
    pub light: f32,
    /// The color of that light (default: a warm fire glow).
    pub glow: [u8; 3],
    /// *Effect*: shakes the screen this many seconds when it starts (0 = no shake).
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub shake: f32,
    /// A named animation of the sheet (`SheetDef::tags`): replaces `first` / `last` / `fps`
    /// when the game plays it ("" = use them as they are).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

impl Default for FxSprite {
    fn default() -> Self {
        FxSprite { sheet: String::new(), first: 0, last: 0, fps: 12.0, scale: 1.0, light: 0.0, glow: [255, 160, 70], shake: 0.0, tag: String::new() }
    }
}

impl Action {
    /// Visits this action and, through `If`, every nested action (depth first, this one first),
    /// so nobody has to spell the recursion out.
    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Action)) {
        f(self);
        if let Action::If { then, otherwise, .. } = self {
            for a in then.iter().chain(otherwise) {
                a.walk(f);
            }
        }
    }

    /// [`Action::walk`] with mutable access.
    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Action)) {
        f(self);
        if let Action::If { then, otherwise, .. } = self {
            for a in then.iter_mut().chain(otherwise.iter_mut()) {
                a.walk_mut(f);
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Action::Dialog { .. } => "Dialog",
            Action::Choice { .. } => "Choice",
            Action::GiveItem { .. } => "Give item",
            Action::TakeItem { .. } => "Take item",
            Action::SetFlag { .. } => "Set flag",
            Action::SetVar { .. } => "Set variable",
            Action::Teleport { .. } => "Teleport",
            Action::SetActive { .. } => "Set active",
            Action::SetFrame { .. } => "Set frame",
            Action::Move { .. } => "Move",
            Action::Face { .. } => "Face",
            Action::Wait { .. } => "Wait",
            Action::Emit { .. } => "Emit event",
            Action::RunScript { .. } => "Run script",
            Action::If { .. } => "If",
            Action::PlaySound { .. } => "Play sound",
            Action::PlayMusic { .. } => "Play music",
            Action::Heal { .. } => "Heal player",
            Action::SetLocked { .. } => "Set locked",
            Action::SetState { .. } => "Set prop state",
            Action::OpenShop { .. } => "Open shop",
            Action::Quest { .. } => "Quest",
            Action::Patrol { .. } => "Patrol",
            Action::SetTime { .. } => "Set time of day",
            Action::GiveSkillPoints { .. } => "Give skill points",
            Action::SetSkill { .. } => "Set skill level",
            Action::LearnSpell { .. } => "Learn spell",
            Action::AddMana { .. } => "Add mana",
            Action::DamageAround { .. } => "Damage around player",
            Action::PlayerLight { .. } => "Light around player",
            Action::Aim { .. } => "Aim (then run script)",
            Action::Fly { .. } => "Fly sprite to point",
            Action::Effect { .. } => "Play effect",
            Action::DamageArea { .. } => "Damage area",
            Action::LightAt { .. } => "Light at point",
            Action::HealArea { .. } => "Heal area",
            Action::Equip { .. } => "Equip item",
            Action::Unequip { .. } => "Unequip slot",
            Action::GiveItemTo { .. } => "Give item to character",
            Action::TakeItemFrom { .. } => "Take item from character",
            Action::SetUiVisible { .. } => "Show / hide UI",
            Action::Cutscene { .. } => "Play cutscene",
            Action::EndCutscene => "End cutscene",
            Action::WalkTo { .. } => "Walk to tile",
            Action::Approach { .. } => "Walk up to character",
            Action::Place { .. } => "Place at tile",
            Action::Dim { .. } => "Dim the scene",
            Action::SetSpawners { .. } => "Spawners on / off",
            Action::DefeatEnemies => "Defeat all enemies",
            Action::Autosave => "Autosave",
            Action::GiveXp { .. } => "Give experience",
            Action::ApplyEffect { .. } => "Apply effect",
            Action::CureEffect { .. } => "Cure effect",
            Action::Shake { .. } => "Shake the screen",
            Action::SetWeather { .. } => "Set weather",
            Action::Schedule { .. } => "Schedule",
            Action::Companion { .. } => "Companion",
            Action::OpenCrafting { .. } => "Open crafting",
            Action::UnlockAchievement { .. } => "Unlock achievement",
            Action::Lockpick { .. } => "Lockpicking game",
            Action::Sleep { .. } => "Sleep",
            Action::HealCharacter { .. } => "Heal character",
            Action::CompanionOrder { .. } => "Companion order",
            Action::OpenCompanion { .. } => "Open companion window",
            Action::Mood { .. } => "Set mood",
            Action::ShowImage { .. } => "Show image",
            Action::HideImage => "Hide image",
            Action::ShopStock { .. } => "Shop stock",
            Action::TeachSpell { .. } => "Teach spell",
            Action::Emote { .. } => "Show emotion",
            Action::ClearEmote { .. } => "Clear emotion",
        }
    }

    pub fn templates() -> Vec<Action> {
        let s = String::new;
        vec![
            Action::Dialog { speaker: s(), text: s() },
            Action::Choice { speaker: s(), text: s(), options: vec!["Yes".into(), "No".into()], var: "choice".into() },
            Action::GiveItem { item: s(), count: 1 },
            Action::TakeItem { item: s(), count: 1 },
            Action::SetFlag { name: s(), value: true },
            Action::SetVar { name: s(), op: VarOp::Set, value: 0 },
            Action::Teleport { map: s(), spawn: s() },
            Action::SetActive { target: "self".into(), active: false },
            Action::SetFrame { target: "self".into(), frame: 1 },
            Action::Move { target: "self".into(), dx: 0, dy: 1 },
            Action::Face { target: "self".into(), facing: Facing::Down },
            Action::Wait { seconds: 0.5 },
            Action::Emit { name: s() },
            Action::RunScript { function: s() },
            Action::If { conditions: Vec::new(), then: Vec::new(), otherwise: Vec::new() },
            Action::PlaySound { sound: s() },
            Action::PlayMusic { sound: s() },
            Action::Heal { amount: 1 },
            Action::SetLocked { target: "self".into(), locked: false },
            Action::SetState { target: "self".into(), state: String::new() },
            Action::OpenShop { target: "self".into() },
            Action::Quest { quest: s(), op: crate::quest::QuestOp::Start, stage: 0 },
            Action::Patrol { target: "self".into(), active: true, points: Vec::new() },
            Action::SetTime { hour: 8.0 },
            Action::GiveSkillPoints { amount: 1 },
            Action::SetSkill { skill: String::new(), level: 1 },
            Action::LearnSpell { spell: String::new() },
            Action::AddMana { amount: 10 },
            Action::DamageAround { damage: 2, radius: 2.0 },
            Action::PlayerLight { radius: 5.0, seconds: 30.0, color: spell_light_color(), flicker: false },
            Action::Aim { function: s() },
            Action::Fly { sprite: FxSprite::default(), x: 0.0, y: 0.0, speed: 8.0, function: s() },
            Action::Effect { sprite: FxSprite::default(), x: 0.0, y: 0.0 },
            Action::DamageArea { x: 0.0, y: 0.0, radius: 2.0, damage: 2 },
            Action::LightAt { x: 0.0, y: 0.0, radius: 3.0, seconds: 10.0, color: [255, 190, 120], flicker: true },
            Action::HealArea { x: 0.0, y: 0.0, radius: 2.0, amount: 2 },
            Action::Equip { target: "self".into(), item: s() },
            Action::Unequip { target: "self".into(), slot: crate::defs::EquipSlot::Weapon },
            Action::GiveItemTo { target: "self".into(), item: s(), count: 1 },
            Action::TakeItemFrom { target: "self".into(), item: s(), count: 1 },
            Action::SetUiVisible { visible: false },
            Action::Cutscene { function: s() },
            Action::WalkTo { target: "self".into(), x: 0, y: 0 },
            Action::Approach { target: "self".into(), to: "player".into() },
            Action::Place { target: "player".into(), x: 0, y: 0 },
            Action::Dim { amount: 0.7, seconds: 1.0 },
            Action::SetSpawners { map: s(), zone: s(), active: false },
            Action::DefeatEnemies,
            Action::Autosave,
            Action::GiveXp { amount: 10 },
            Action::ApplyEffect { target: "player".into(), effect: crate::effects::EffectSpec::template(crate::effects::EffectKind::Poison) },
            Action::CureEffect { target: "player".into(), kind: Some(crate::effects::EffectKind::Poison) },
            Action::Shake { seconds: 0.5, strength: 1.5 },
            Action::SetWeather { map: s(), weather: Some(crate::map::Weather::new(crate::map::WeatherKind::Rain, 0.6)) },
            Action::Schedule { target: "self".into(), active: false },
            Action::Companion { target: "self".into(), join: true },
            Action::OpenCrafting { station: s(), title: s() },
            Action::UnlockAchievement { id: s() },
            Action::Lockpick { difficulty: 1, var: "picked".into() },
            Action::Sleep { hours: 8.0 },
            Action::HealCharacter { target: "self".into(), amount: 2 },
            Action::CompanionOrder { target: "self".into(), order: "stay".into() },
            Action::OpenCompanion { target: "self".into() },
            Action::Mood { target: "self".into(), mood: "worried".into() },
            Action::ShowImage { sheet: String::new(), wait: true, scroll: false, speed: image_speed(), scale: 1.0 },
            Action::HideImage,
            Action::ShopStock { target: "self".into(), item: String::new(), op: VarOp::Add, count: 3, price: 0 },
            Action::TeachSpell { target: "self".into(), spell: String::new() },
            Action::Emote { target: "self".into(), emotion: "joy".into(), seconds: 0.0, wait: false },
            Action::ClearEmote { target: "self".into() },
        ]
    }
}

/// Where a list of triggers lives: the walks over a whole project ([`walk_db_triggers`],
/// [`walk_map_triggers`], `Project::for_each_trigger`) hand it along with the triggers.
#[derive(Clone, Copy)]
pub enum TriggerSite<'a> {
    /// An item definition's *on use* triggers.
    Item(&'a crate::defs::ItemDef),
    Prop(&'a crate::defs::PropDef),
    Character(&'a crate::defs::CharacterDef),
    /// A map's own triggers.
    Map(&'a str),
    Zone { map: &'a str, zone: &'a crate::map::Zone },
    /// A placed entity's own triggers (its definition's are separate).
    Entity { map: &'a str, entity: &'a crate::entity::EntityInstance },
}

impl TriggerSite<'_> {
    /// The owner kind that decides which events reach these triggers.
    pub fn owner(&self) -> TriggerOwner {
        match self {
            TriggerSite::Item(_) => TriggerOwner::ItemUse,
            TriggerSite::Prop(_) => TriggerOwner::Prop,
            TriggerSite::Character(_) => TriggerOwner::Character,
            TriggerSite::Map(_) => TriggerOwner::Map,
            TriggerSite::Zone { .. } => TriggerOwner::Zone,
            TriggerSite::Entity { entity, .. } => match entity.kind {
                crate::entity::EntityKind::Character => TriggerOwner::Character,
                crate::entity::EntityKind::Prop => TriggerOwner::Prop,
                crate::entity::EntityKind::Item => TriggerOwner::Pickup,
            },
        }
    }

    /// Where it is, for messages: "item sword", "map village, zone gate".
    pub fn label(&self) -> String {
        match self {
            TriggerSite::Item(d) => format!("item {}", d.id),
            TriggerSite::Prop(d) => format!("prop {}", d.id),
            TriggerSite::Character(d) => format!("character {}", d.id),
            TriggerSite::Map(m) => format!("map {m}"),
            TriggerSite::Zone { map, zone } => format!("map {map}, zone {}", zone.name),
            TriggerSite::Entity { map, entity } => format!("map {map}, {}", entity.label()),
        }
    }
}

/// Every trigger list of the definitions: items' *on use*, props', characters'.
pub fn walk_db_triggers<'a>(db: &'a crate::defs::Database, f: &mut dyn FnMut(TriggerSite<'a>, &'a [Trigger])) {
    for d in &db.items {
        f(TriggerSite::Item(d), &d.on_use);
    }
    for d in &db.props {
        f(TriggerSite::Prop(d), &d.triggers);
    }
    for d in &db.characters {
        f(TriggerSite::Character(d), &d.triggers);
    }
}

/// Every trigger list on a map: the map's own, its zones', its placed entities'.
pub fn walk_map_triggers<'a>(name: &'a str, map: &'a crate::map::Map, f: &mut dyn FnMut(TriggerSite<'a>, &'a [Trigger])) {
    f(TriggerSite::Map(name), &map.triggers);
    for zone in &map.zones {
        f(TriggerSite::Zone { map: name, zone }, &zone.triggers);
    }
    for entity in &map.entities {
        f(TriggerSite::Entity { map: name, entity }, &entity.triggers);
    }
}

/// Triggers in a project whose event never reaches them: "where: On <event>".
pub fn useless_triggers(project: &crate::project::Project) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |owner: TriggerOwner, ts: &[Trigger], place: String| {
        for t in ts.iter().filter(|t| !owner.allows(&t.on)) {
            out.push(format!("{place}: On {}", t.on.label()));
        }
    };
    project.for_each_trigger(&mut |site, ts| check(site.owner(), ts, site.label()));
    out
}

/// Every action of `actions`, nested ones (inside `If`) included.
pub fn walk_actions<'a>(actions: &'a [Action], f: &mut dyn FnMut(&'a Action)) {
    for a in actions {
        a.walk(f);
    }
}

/// [`walk_actions`] with mutable access.
pub fn walk_actions_mut(actions: &mut [Action], f: &mut dyn FnMut(&mut Action)) {
    for a in actions {
        a.walk_mut(f);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn time_of_day_condition_wraps_midnight() {
        use crate::state::WorldState;
        assert!(hour_between(22.0, 20.0, 6.0) && hour_between(3.0, 20.0, 6.0) && !hour_between(12.0, 20.0, 6.0));
        assert!(hour_between(9.0, 8.0, 18.0) && !hour_between(18.0, 8.0, 18.0));
        let mut w = WorldState { hour: 23.5, ..Default::default() };
        let night = Condition::new(Cond::TimeBetween { from: 20.0, to: 6.0 });
        assert!(night.eval(&w, &mut |_| false));
        w.hour = 10.0;
        assert!(!night.eval(&w, &mut |_| false));
    }

    use super::*;

    #[test]
    fn trigger_walk_visits_every_site() {
        use crate::entity::{EntityInstance, EntityKind};
        use crate::map::{Map, Zone, ZoneKind};
        let mut map = Map::new("m", 4, 4);
        map.triggers.push(Trigger::new(TriggerOn::MapEnter));
        map.zones.push(Zone { uid: 1, name: "gate".into(), x: 0, y: 0, w: 1, h: 1, kind: ZoneKind::Trigger, triggers: vec![Trigger::new(TriggerOn::Enter)], loot: None });
        let mut npc = EntityInstance::new(2, EntityKind::Character, "guard", 63.0, 63.0);
        npc.triggers.push(Trigger::new(TriggerOn::Interact));
        map.entities.push(npc);
        let mut coin = EntityInstance::new(3, EntityKind::Item, "coin", 95.0, 95.0);
        coin.triggers.push(Trigger::new(TriggerOn::Interact));
        map.entities.push(coin);
        let mut seen = Vec::new();
        walk_map_triggers("m", &map, &mut |site, ts| seen.push((site.label(), site.owner(), ts.len())));
        assert_eq!(seen.len(), 4, "the map, its zone and both entities");
        assert_eq!(seen[0], ("map m".to_string(), TriggerOwner::Map, 1));
        assert_eq!(seen[1], ("map m, zone gate".to_string(), TriggerOwner::Zone, 1));
        assert_eq!(seen[2].1, TriggerOwner::Character);
        assert_eq!(seen[3].1, TriggerOwner::Pickup);
        assert!(seen[2].0.starts_with("map m, "), "{}", seen[2].0);
        // The definitions: one site per item, prop and character.
        let mut db = crate::defs::Database::default();
        db.items.push(crate::defs::ItemDef::fresh("potion"));
        db.props.push(crate::defs::PropDef::fresh("chest"));
        db.characters.push(crate::defs::CharacterDef::fresh("guard"));
        let mut owners = Vec::new();
        walk_db_triggers(&db, &mut |site, _| owners.push(site.owner()));
        assert_eq!(owners, [TriggerOwner::ItemUse, TriggerOwner::Prop, TriggerOwner::Character]);
    }

    #[test]
    fn trigger_owners() {
        assert!(TriggerOwner::Map.allows(&TriggerOn::MapEnter) && !TriggerOwner::Map.allows(&TriggerOn::Enter));
        assert!(TriggerOwner::Zone.allows(&TriggerOn::Enter) && !TriggerOwner::Zone.allows(&TriggerOn::MapEnter));
        assert!(!TriggerOwner::Zone.allows(&TriggerOn::UseItem { item: String::new() }));
        assert!(TriggerOwner::Prop.allows(&TriggerOn::Unlock) && !TriggerOwner::Character.allows(&TriggerOn::Unlock));
        assert!(TriggerOwner::Character.allows(&TriggerOn::Defeat) && !TriggerOwner::Prop.allows(&TriggerOn::Defeat));
        assert!(!TriggerOwner::ItemUse.allows(&TriggerOn::Custom { name: "x".into() }));
        assert_eq!(TriggerOwner::Map.templates().iter().map(|t| t.label()).collect::<Vec<_>>(), vec!["Map enter", "Custom event", "Woke up", "Time reached"]);
        assert_eq!(TriggerOwner::Map.replacement(&TriggerOn::Enter), TriggerOn::MapEnter);
        assert_eq!(TriggerOwner::Zone.replacement(&TriggerOn::MapEnter), TriggerOn::Enter);
    }

    #[test]
    fn example_project_has_no_useless_triggers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let project = crate::project::Project::load(&root).unwrap();
        assert_eq!(useless_triggers(&project), Vec::<String>::new());
    }

    #[test]
    fn trigger_matching() {
        let any = TriggerOn::UseItem { item: String::new() };
        let key = TriggerOn::UseItem { item: "key".into() };
        let ev = GameEvent::UseItem { item: "key".into() };
        assert!(any.matches(&ev) && key.matches(&ev));
        assert!(!key.matches(&GameEvent::UseItem { item: "potion".into() }));
        assert!(!TriggerOn::Interact.matches(&GameEvent::Enter));
        let c = TriggerOn::Custom { name: "boom".into() };
        assert!(c.matches(&GameEvent::Custom { name: "boom".into() }));
    }

    #[test]
    fn hp_below_fires_on_crossing() {
        let t = TriggerOn::HpBelow { percent: 50 };
        assert!(t.matches(&GameEvent::HpBelow { from: 0.6, to: 0.4 }));
        assert!(t.matches(&GameEvent::HpBelow { from: 0.5, to: 0.3 }));
        assert!(!t.matches(&GameEvent::HpBelow { from: 0.4, to: 0.2 }), "already below");
        assert!(!t.matches(&GameEvent::HpBelow { from: 0.9, to: 0.6 }), "not there yet");
        assert!(TriggerOwner::Character.allows(&t));
        assert!(!TriggerOwner::Prop.allows(&t));
    }

    #[test]
    fn hour_fires_when_the_clock_passes_it() {
        let noon = TriggerOn::Hour { hour: 12.0 };
        assert!(noon.matches(&GameEvent::Hour { from: 11.95, to: 12.01 }));
        assert!(noon.matches(&GameEvent::Hour { from: 11.5, to: 12.0 }), "reaching it exactly");
        assert!(!noon.matches(&GameEvent::Hour { from: 12.0, to: 12.2 }), "already past");
        assert!(!noon.matches(&GameEvent::Hour { from: 9.0, to: 11.9 }));
        // Over midnight (sleeping from 22:00 to 7:00).
        let dawn = TriggerOn::Hour { hour: 5.5 };
        assert!(dawn.matches(&GameEvent::Hour { from: 22.0, to: 7.0 }));
        assert!(!noon.matches(&GameEvent::Hour { from: 22.0, to: 7.0 }));
        assert!(TriggerOn::Hour { hour: 23.5 }.matches(&GameEvent::Hour { from: 22.0, to: 7.0 }));
        assert!(TriggerOwner::Map.allows(&noon));
        assert!(!TriggerOwner::Zone.allows(&noon) && !TriggerOwner::Character.allows(&noon));
    }

    #[test]
    fn conditions() {
        let mut w = WorldState::default();
        w.set_flag("door_open", true);
        w.set_var("gold", 5);
        w.give_item("key", 2);
        let mut no_script = |_: &str| panic!("not called");
        let met = |c: &[Condition], w: &WorldState, s: &mut dyn FnMut(&str) -> bool| all_met(c, w, s);
        assert!(met(&[Condition::new(Cond::Flag { name: "door_open".into() })], &w, &mut no_script));
        assert!(met(
            &[Condition { cond: Cond::Flag { name: "nope".into() }, negate: true }],
            &w,
            &mut no_script
        ));
        assert!(met(
            &[Condition::new(Cond::Var { name: "gold".into(), op: CmpOp::Ge, value: 5 })],
            &w,
            &mut no_script
        ));
        assert!(!met(
            &[Condition::new(Cond::HasItem { item: "key".into(), count: 3 })],
            &w,
            &mut no_script
        ));
        let mut yes = |f: &str| f == "ok";
        assert!(met(&[Condition::new(Cond::Script { function: "ok".into() })], &w, &mut yes));
        // A fresh world (and an old save) is level 1.
        let level = |op, value| Condition::new(Cond::Level { op, value });
        assert!(met(&[level(CmpOp::Eq, 1)], &w, &mut no_script));
        w.level = 3;
        assert!(met(&[level(CmpOp::Ge, 3)], &w, &mut no_script));
        assert!(!met(&[level(CmpOp::Lt, 3)], &w, &mut no_script));
        let poisoned = [Condition::new(Cond::HasEffect { kind: crate::effects::EffectKind::Poison })];
        assert!(!met(&poisoned, &w, &mut no_script));
        crate::effects::apply(&mut w.effects, &crate::effects::EffectSpec::new(crate::effects::EffectKind::Poison, 1.0, 3.0));
        assert!(met(&poisoned, &w, &mut no_script));
        let slimes = |count| [Condition::new(Cond::Defeated { character: "slime".into(), op: CmpOp::Ge, count })];
        assert!(met(&slimes(0), &w, &mut no_script) && !met(&slimes(1), &w, &mut no_script));
        w.bestiary.insert("slime".into(), 3);
        assert!(met(&slimes(3), &w, &mut no_script) && !met(&slimes(4), &w, &mut no_script));
    }

    #[test]
    fn triggers_serialize_to_ron() {
        let t = Trigger {
            on: TriggerOn::Interact,
            conditions: vec![Condition::new(Cond::HasItem { item: "key".into(), count: 1 })],
            actions: vec![Action::Dialog { speaker: "Bob".into(), text: "Hi".into() }],
            once: true,
        };
        let s = ron::to_string(&t).unwrap();
        let back: Trigger = ron::from_str(&s).unwrap();
        assert_eq!(back, t);
    }
}
