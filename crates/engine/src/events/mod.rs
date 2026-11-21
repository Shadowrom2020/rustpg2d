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
    /// A turn-based fight started / ended (map triggers, and each fighting character's own).
    CombatStart,
    CombatEnd,
    /// The hero searched (the action bar's Search): zones they stand in, things within 2 tiles.
    Search,
    /// A trap sprang on the hero / was disarmed (its zone's or prop's triggers).
    TrapSprung,
    TrapDisarmed,
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
    /// A fight started on the map (map triggers; a character's own when it fights in it).
    CombatStart,
    /// The fight is over (no enemy fights on; map triggers and the fighters' own).
    CombatEnd,
    /// The hero searched here (the action bar's Search): a zone they stand in, a prop or a
    /// character within 2 tiles. *Search found* compares the roll.
    Search,
    /// The zone's or prop's trap sprang on the hero.
    TrapSprung,
    /// The zone's or prop's trap was disarmed.
    TrapDisarmed,
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
            (O::Map, T::MapEnter | T::Sleep | T::Hour { .. } | T::CombatStart | T::CombatEnd) => true,
            (O::Zone, T::Enter | T::Leave | T::Interact | T::Search | T::TrapSprung | T::TrapDisarmed) => true,
            (O::Character, T::Interact | T::UseItem { .. } | T::Defeat | T::HpBelow { .. } | T::CombatStart | T::CombatEnd | T::Search) => true,
            (O::Prop, T::Interact | T::UseItem { .. } | T::Unlock | T::Lock | T::Sleep | T::Search | T::Defeat | T::TrapSprung | T::TrapDisarmed) => true,
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
            | (TriggerOn::Sleep, GameEvent::Sleep)
            | (TriggerOn::CombatStart, GameEvent::CombatStart)
            | (TriggerOn::CombatEnd, GameEvent::CombatEnd)
            | (TriggerOn::Search, GameEvent::Search)
            | (TriggerOn::TrapSprung, GameEvent::TrapSprung)
            | (TriggerOn::TrapDisarmed, GameEvent::TrapDisarmed) => true,
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
            TriggerOn::CombatStart => "Fight starts",
            TriggerOn::CombatEnd => "Fight ends",
            TriggerOn::Search => "Searched",
            TriggerOn::TrapSprung => "Trap sprung",
            TriggerOn::TrapDisarmed => "Trap disarmed",
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
            TriggerOn::CombatStart,
            TriggerOn::CombatEnd,
            TriggerOn::Search,
            TriggerOn::TrapSprung,
            TriggerOn::TrapDisarmed,
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
    /// Whether the hero is proficient in a skill, tool or saving throw (`stealth`, `thieves-tools`,
    /// `save:dex`): 1 when proficient, 0 when not, compared with `level` ("≥ 1" = proficient).
    /// Until 0.27 a skill tree's level.
    Skill { skill: String, op: CmpOp, level: i64 },
    /// The player has learned the spell.
    KnowsSpell { spell: String },
    /// The player's level (experience and levels) compared with a value.
    Level { op: CmpOp, value: i64 },
    /// The hero has a condition (`Any` = any condition; until 0.27 a status effect).
    HasEffect { kind: crate::srd::Condition },
    /// How many characters of a definition the player has defeated, compared with a number.
    Defeated { character: String, op: CmpOp, count: i64 },
    /// How a character (or "player") rests on furniture: "sit", "lie", "stand"; "" = on its feet.
    Pose { target: TargetName, pose: String },
    /// The player's race (`races::RaceDef`; "" = none chosen).
    PlayerRace { race: String },
    /// The player's standing with a faction (-100..100) compared with a value.
    FactionStanding { faction: String, op: CmpOp, value: i64 },
    /// A timer's seconds left compared with a value (a timer that isn't running has 0: "> 0" =
    /// running).
    Timer { name: String, op: CmpOp, seconds: i64 },
    /// A text variable equals a text (`ignore_case`: "Rosehill" = "rosehill"; leading and
    /// trailing spaces never count; an empty `value` = the variable is empty).
    TextVar { name: String, value: String, #[serde(default)] ignore_case: bool },
    /// The last ability check or saving throw (*Ability check*, `check()`, a lock, crafting)
    /// succeeded, by at least `by` (0 = just succeeded).
    CheckPassed {
        #[serde(default)]
        by: i64,
    },
    /// The hero's class (`classes::ClassDef`) or subclass, at a level compared with `level`
    /// (`class` "" = any class; no class = false).
    Class {
        class: String,
        #[serde(default = "cmp_ge")]
        op: CmpOp,
        #[serde(default = "one_i64")]
        level: i64,
    },
    /// The hero's money (every coin carried) in copper pieces, compared with `cp`.
    Money {
        #[serde(default = "cmp_ge")]
        op: CmpOp,
        cp: i64,
    },
    /// The hero is attuned to the magic item.
    Attuned { item: String },
    /// A character (by name or definition id) was knocked out in a fight instead of killed.
    KnockedOut { name: String },
    /// The hero's passive check of `what` (10 + the bonus, ±5 for advantage / disadvantage: dim
    /// light for Perception) is at least `dc`: noticed without rolling.
    Passive { what: String, dc: i32 },
    /// The hero's last search (the action bar's Search: Perception) reached `dc`.
    SearchRoll { dc: i32 },
    /// The hero has this disease, caught and not cured ("" = any disease).
    HasDisease { disease: String },
}

fn cmp_ge() -> CmpOp {
    CmpOp::Ge
}

fn one_i64() -> i64 {
    1
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
            Cond::Skill { .. } => "Proficient in",
            Cond::KnowsSpell { .. } => "Knows spell",
            Cond::Level { .. } => "Player level",
            Cond::HasEffect { .. } => "Player has condition",
            Cond::Defeated { .. } => "Defeated count",
            Cond::Pose { .. } => "Pose (sitting, asleep)",
            Cond::PlayerRace { .. } => "Player race",
            Cond::FactionStanding { .. } => "Faction standing",
            Cond::Timer { .. } => "Timer",
            Cond::TextVar { .. } => "Text variable",
            Cond::CheckPassed { .. } => "Check passed",
            Cond::Class { .. } => "Has class",
            Cond::Money { .. } => "Has money",
            Cond::Attuned { .. } => "Is attuned",
            Cond::KnockedOut { .. } => "Was knocked out",
            Cond::Passive { .. } => "Passive check",
            Cond::SearchRoll { .. } => "Search found",
            Cond::HasDisease { .. } => "Has disease",
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
            Cond::HasEffect { kind: crate::srd::Condition::Poisoned },
            Cond::Defeated { character: String::new(), op: CmpOp::Ge, count: 10 },
            Cond::Pose { target: "self".into(), pose: "lie".into() },
            Cond::PlayerRace { race: String::new() },
            Cond::FactionStanding { faction: String::new(), op: CmpOp::Le, value: -20 },
            Cond::Timer { name: "timer".into(), op: CmpOp::Gt, seconds: 0 },
            Cond::TextVar { name: String::new(), value: String::new(), ignore_case: true },
            Cond::CheckPassed { by: 0 },
            Cond::Class { class: String::new(), op: CmpOp::Ge, level: 1 },
            Cond::Money { op: CmpOp::Ge, cp: 100 },
            Cond::Attuned { item: String::new() },
            Cond::KnockedOut { name: String::new() },
            Cond::Passive { what: "perception".into(), dc: 13 },
            Cond::SearchRoll { dc: 15 },
            Cond::HasDisease { disease: String::new() },
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
            Cond::Skill { skill, op, level } => op.apply(world.is_proficient(skill) as i64, *level),
            Cond::KnowsSpell { spell } => world.spells.contains(spell) || world.prepared.iter().any(|s| s == spell),
            Cond::Level { op, value } => op.apply(world.player_level() as i64, *value),
            Cond::HasEffect { kind } => crate::srd::conditions::has(&world.effects, *kind),
            Cond::Defeated { character, op, count } => op.apply(world.defeated(character) as i64, *count),
            // Needs the running game (see `conditions_met`).
            Cond::Pose { .. } => false,
            Cond::PlayerRace { race } => world.player_race == *race || (!race.is_empty() && world.player_race_parent == *race),
            // The saved standing; the running game adds the starting one (see `conditions_met`).
            Cond::FactionStanding { faction, op, value } => op.apply(world.standing.get(faction).copied().unwrap_or(0) as i64, *value),
            Cond::Timer { name, op, seconds } => op.apply(world.timers.get(name).map_or(0, |t| t.left.ceil() as i64), *seconds),
            Cond::TextVar { name, value, ignore_case } => {
                let (a, b) = (world.text(name).trim(), value.trim());
                if *ignore_case {
                    a.to_lowercase() == b.to_lowercase()
                } else {
                    a == b
                }
            }
            Cond::CheckPassed { by } => world.last_check.as_ref().is_some_and(|r| r.success == Some(true) && r.margin() >= *by),
            Cond::Class { class, op, level } => {
                world.class.as_ref().is_some_and(|c| class.is_empty() || c.class == *class || (!c.subclass.is_empty() && c.subclass == *class)) && op.apply(world.player_level() as i64, *level)
            }
            Cond::Money { op, cp } => op.apply(world.money_cp() as i64, *cp),
            Cond::Attuned { item } => world.attuned.iter().any(|a| a == item),
            Cond::KnockedOut { name } => world.was_knocked_out(name),
            Cond::Passive { what, dc } => world.passive(what).is_some_and(|v| v >= *dc as i64),
            Cond::SearchRoll { dc } => world.last_search.is_some_and(|v| v >= *dc as i64),
            Cond::HasDisease { disease } => world.diseases.iter().any(|d| disease.is_empty() || d.id == *disease),
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
    PlaySound {
        sound: String,
        /// Times its own volume (1 = as set in the Sounds form).
        #[serde(default = "one_f32")]
        volume: f32,
        /// 1 = as it is, 2 = an octave higher (and faster), 0.5 = an octave lower.
        #[serde(default = "one_f32")]
        pitch: f32,
        /// -1 = left, 0 = both sides, 1 = right.
        #[serde(default)]
        pan: f32,
    },
    /// Switch the looping music (empty = stop); `fade` > 0 crossfades over that many seconds.
    PlayMusic {
        sound: String,
        #[serde(default)]
        fade: f32,
    },
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
    /// Until 0.27: skill points. Does nothing since the SRD rules (no template).
    GiveSkillPoints { amount: i32 },
    /// Makes the hero proficient in a skill, tool or saving throw (`level` > 0) or takes the
    /// proficiency away (0). Until 0.27 a skill tree's level.
    SetSkill { skill: String, level: u32 },
    /// Teaches a spell.
    LearnSpell { spell: String },
    /// Mana (negative = drain).
    AddMana { amount: i32 },
    /// Damages every character within `radius` tiles of the player (spell effects).
    DamageAround {
        damage: i32,
        radius: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
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
    DamageArea {
        x: f32,
        y: f32,
        radius: f32,
        damage: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
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
    /// Hides or shows the game's HUD (status frame, clock, action bar, buttons), e.g. for a
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
    /// *Apply condition*: a condition (and / or ongoing damage) on the player or a character,
    /// unless its saving throw succeeds (until 0.27 a status effect).
    ApplyEffect { target: TargetName, effect: crate::srd::ConditionSpec },
    /// *Remove condition* (`None` or `Any` = all of them, ongoing damage too) from the player or a
    /// character.
    CureEffect { target: TargetName, kind: Option<crate::srd::Condition> },
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
    /// A custom lock puzzle: a DEX (Thieves' Tools) check against the DC `difficulty`; 1 (picked)
    /// or 0 goes into variable `var`. (Until 0.27 the lockpicking game with a skill level as
    /// difficulty; the editor converts old levels to DCs.)
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
    /// Changes the player's standing with a faction (`factions.rs`; -100..100, at -20 or below
    /// its characters attack the player).
    ChangeStanding { faction: String, amount: i32 },
    /// A character (or "player") carries a prop in both hands ("" = puts it down); while carrying
    /// it holds nothing else and the player can't attack.
    Carry { target: TargetName, prop: String },
    /// A short line in a speech bubble over a character's head (`player` too) that doesn't stop
    /// play: one of `lines` at random, not the one it said last. `seconds` 0 = by the length.
    Bark { target: TargetName, lines: Vec<String>, #[serde(default)] seconds: f32 },
    /// A fighting character (*Chase* behaviour) runs to where another character (or "player")
    /// is, following it for a while, and fights whoever it sees on the way: a chief calling
    /// his men.
    RushTo { target: TargetName, to: TargetName },
    /// Shows a picture on a layer (1 to 100, drawn in number order above the map and below the
    /// HUD; a picture already there is replaced): a frame of a sprite sheet, placed `x`, `y`
    /// screen pixels from its `anchor` (like HUD elements), or with `on_map` its middle on map
    /// tile `x`, `y`. `opacity` 0 to 1, `rotation` in degrees, `tint` multiplies its colours.
    /// Saved with the game.
    ShowPicture {
        layer: u32,
        sheet: String,
        #[serde(default)]
        frame: u32,
        #[serde(default)]
        anchor: crate::script_ui::Anchor,
        #[serde(default)]
        on_map: bool,
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default = "one_f32")]
        scale: f32,
        #[serde(default = "one_f32")]
        opacity: f32,
        #[serde(default)]
        rotation: f32,
        #[serde(default = "white")]
        tint: [u8; 3],
        #[serde(default)]
        blend: crate::pictures::Blend,
    },
    /// Moves a picture to new values (each one left out keeps its value) over `seconds` with
    /// `easing`; `wait`: the event goes on when it is there.
    MovePicture {
        layer: u32,
        #[serde(default)]
        to: crate::pictures::PictureTargets,
        #[serde(default = "one_f32")]
        seconds: f32,
        #[serde(default)]
        easing: crate::pictures::Easing,
        #[serde(default)]
        wait: bool,
    },
    /// Takes the picture of a layer away (0 = every picture).
    ErasePicture { layer: u32 },
    /// Colours the whole screen (over the lit world and the pictures, under the HUD) with
    /// `color` (its alpha = how strongly; 0 = no tint) over `seconds`; stays until changed and is
    /// saved. `wait`: the event goes on when it is there.
    TintScreen {
        color: [u8; 4],
        #[serde(default = "one_f32")]
        seconds: f32,
        #[serde(default)]
        wait: bool,
    },
    /// A flash of `color` (alpha = its strength) fading out over `seconds` (lightning, a spell).
    /// Left out when the player turned off screen effects.
    FlashScreen {
        color: [u8; 4],
        #[serde(default = "half_second")]
        seconds: f32,
    },
    /// Covers the screen with `color` over `seconds` (dialogs and the HUD stay over it); it lasts
    /// across map changes until *Fade in*. `wait`: the event goes on when it is covered.
    FadeOut {
        #[serde(default = "one_f32")]
        seconds: f32,
        #[serde(default)]
        color: [u8; 3],
        #[serde(default = "yes")]
        wait: bool,
    },
    /// Uncovers the screen of *Fade out* over `seconds`.
    FadeIn {
        #[serde(default = "one_f32")]
        seconds: f32,
        #[serde(default = "yes")]
        wait: bool,
    },
    /// Starts (or restarts) a named timer of `seconds`; `shown`: at the top of the screen. When it
    /// runs out it runs `function` and / or fires the custom event `event`. Timers pause while a
    /// menu or a dialog is open, and are saved.
    StartTimer {
        name: String,
        seconds: f32,
        #[serde(default)]
        shown: bool,
        #[serde(default)]
        function: String,
        #[serde(default)]
        event: String,
    },
    /// Stops a timer (nothing runs).
    StopTimer { name: String },
    /// Asks the player for a whole number between `min` and `max` (a window with `text` over
    /// it; `digits` > 0 limits how many) and puts it into variable `var`; the event waits.
    InputNumber {
        text: String,
        var: String,
        #[serde(default)]
        digits: u32,
        #[serde(default)]
        min: i64,
        #[serde(default = "nine_nine_nine")]
        max: i64,
    },
    /// Asks the player for a text of at most `max_len` characters (prefilled with the text
    /// variable's value) and puts it into text variable `var`; the event waits.
    InputText {
        text: String,
        var: String,
        #[serde(default = "sixteen")]
        max_len: u32,
    },
    /// A character (`target`; "player" too) walks a list of steps (`routes::RouteStep`).
    /// `repeat`: from the start again when done (the event never waits for it); `skip_blocked`:
    /// a step whose tile is taken is left out (else it waits until it is free); `wait`: the
    /// event goes on when the route is done.
    MoveRoute {
        target: TargetName,
        steps: Vec<crate::routes::RouteStep>,
        #[serde(default)]
        repeat: bool,
        #[serde(default = "yes")]
        skip_blocked: bool,
        #[serde(default)]
        wait: bool,
    },
    /// Runs a common event's actions here, as if they stood in this list.
    CallCommon { id: String },
    /// Pans (and zooms) the camera to a tile, a character it then follows, or back to the
    /// player, over `seconds`; it returns to the player when the event or cut-scene is over.
    Camera {
        #[serde(default)]
        aim: crate::camera::CameraAim,
        #[serde(default = "one_f32")]
        seconds: f32,
        #[serde(default)]
        easing: crate::pictures::Easing,
        /// 1 = the usual size, 2 = twice as close.
        #[serde(default = "one_f32")]
        zoom: f32,
        #[serde(default)]
        wait: bool,
    },
    /// Text rolling up the screen over a background colour (credits, a story intro); the event
    /// waits until it has gone or the player skips it. Lines starting with `# ` are headings.
    ScrollText {
        text: String,
        /// Screen pixels per second.
        #[serde(default = "image_speed")]
        speed: f32,
        #[serde(default = "opaque_black")]
        background: [u8; 4],
    },
    /// A music effect (a fanfare): played once while the music ducks, then the music comes back.
    PlayMusicEffect {
        sound: String,
        /// The event waits until it is over.
        #[serde(default)]
        wait: bool,
    },
    /// Changes which faction a character on this map (or "player") belongs to; "" = back to its
    /// own (the placed character's, its definition's, its race's). Saved; who is hostile to whom
    /// is worked out again at once.
    SetFaction { target: TargetName, faction: String },
    /// Rolls an ability check (`what`: a skill like `persuasion`, an ability like `dex`, or
    /// ability:tool like `dex:thieves-tools`) or with `save` a saving throw (`what` = the ability)
    /// against `dc`, writes it into the roll log, remembers it (*Check passed*) and runs `then` on
    /// a success, `otherwise` on a failure.
    Check {
        what: String,
        #[serde(default)]
        save: bool,
        dc: i32,
        #[serde(default)]
        mode: crate::srd::Mode,
        #[serde(default)]
        then: Vec<Action>,
        #[serde(default)]
        otherwise: Vec<Action>,
    },
    /// Rolls dice (`2d6+STR`, `1d20`, `8d6`) into variable `var`, and into the roll log.
    Roll { dice: String, var: String },
    /// A short rest: the rest window, where the hero spends hit dice; an hour passes.
    ShortRest,
    /// A long rest of `hours` (8): full HP, half the hit dice back, one level of exhaustion less,
    /// timed conditions over.
    LongRest {
        #[serde(default = "eight")]
        hours: f32,
    },
    /// Starts a turn-based fight with the hostile characters near the player (nothing without
    /// any, or during one).
    StartCombat,
    /// Gives the hero (`target` "player") or a party member a class: its saving throws and
    /// proficiencies; the level's choices open the level-up window. `level` 0 keeps the level
    /// (a party member's is set when above 0).
    SetClass {
        target: TargetName,
        class: String,
        #[serde(default)]
        level: u32,
    },
    /// The hero casts a spell outside a fight on `target` ("player" or a character on this map):
    /// with a slot unless `free` (a scroll, a story's casting: the effect without the cost).
    CastSpell {
        spell: String,
        #[serde(default = "player_target")]
        target: TargetName,
        #[serde(default)]
        free: bool,
    },
    /// Gives the hero money (`cp` copper pieces) in the fewest gold, silver and copper pieces.
    GiveMoney { cp: u32 },
    /// Takes money (`cp` copper pieces) from the hero's coins, the change given back; nothing
    /// when they have less (check *Has money* first).
    TakeMoney { cp: u32 },
    /// The hero falls `feet` (a trapdoor, a collapsing floor): 1d6 bludgeoning per 10 ft (at most
    /// 20d6), prone for a moment; they stay where they are.
    Fall { feet: u32 },
    /// The hero eats `food` and drinks `water` days' worth (a ration = 1 day of food, a waterskin =
    /// half a day of water): what *Food and water* counts (Database › Project › Rules).
    Nourish { food: f32, water: f32 },
    /// A group check: the hero and every companion on the map roll `what` against `dc`; when at
    /// least half succeed, `then` runs, else `otherwise` (*Check passed* reads the outcome).
    GroupCheck {
        what: String,
        dc: i32,
        #[serde(default)]
        then: Vec<Action>,
        #[serde(default)]
        otherwise: Vec<Action>,
    },
    /// A contest: the hero's `what` against the character `target`'s `against` (empty = the
    /// same); the higher total wins (a tie isn't a win): `then`, else `otherwise`.
    Contest {
        what: String,
        target: TargetName,
        #[serde(default)]
        against: String,
        #[serde(default)]
        then: Vec<Action>,
        #[serde(default)]
        otherwise: Vec<Action>,
    },
    /// Damages a prop with object statistics (`target`: its name, "self"): `amount` of
    /// `damage_type` (None = untyped), past its threshold; at 0 hit points it breaks.
    DamageProp {
        target: TargetName,
        amount: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<crate::srd::DamageType>,
    },
    /// Disarms the trap of the zone or prop named `target` on this map (a lever, a hidden
    /// switch): it no longer springs.
    DisarmTrap { target: TargetName },
    /// The SRD's poisons: gives poison `item` (an item with a poison) to the hero or a character: its
    /// Constitution save, damage and conditions (a drugged ale, a gas trap).
    Poison { target: TargetName, item: String },
    /// The SRD's diseases: the hero is exposed to `disease` (`defs/diseases.ron`): its saving throw
    /// (`save`; else it is caught), then it waits for its onset.
    ExposeDisease { disease: String, #[serde(default = "yes")] save: bool },
    /// Ends the hero's disease `disease` ("" = every disease): its exhaustion stays, its other
    /// symptoms end.
    CureDisease { disease: String },
}

fn player_target() -> TargetName {
    "player".into()
}

fn eight() -> f32 {
    8.0
}

fn opaque_black() -> [u8; 4] {
    [0, 0, 0, 255]
}

fn nine_nine_nine() -> i64 {
    999
}

fn sixteen() -> u32 {
    16
}

fn half_second() -> f32 {
    0.5
}

fn white() -> [u8; 3] {
    [255, 255, 255]
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
        if let Action::If { then, otherwise, .. } | Action::Check { then, otherwise, .. } | Action::GroupCheck { then, otherwise, .. } | Action::Contest { then, otherwise, .. } = self {
            for a in then.iter().chain(otherwise) {
                a.walk(f);
            }
        }
    }

    /// [`Action::walk`] with mutable access.
    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Action)) {
        f(self);
        if let Action::If { then, otherwise, .. } | Action::Check { then, otherwise, .. } | Action::GroupCheck { then, otherwise, .. } | Action::Contest { then, otherwise, .. } = self {
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
            Action::GiveSkillPoints { .. } => "Give skill points (unused)",
            Action::SetSkill { .. } => "Set proficiency",
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
            Action::ApplyEffect { .. } => "Apply condition",
            Action::CureEffect { .. } => "Remove condition",
            Action::Shake { .. } => "Shake the screen",
            Action::SetWeather { .. } => "Set weather",
            Action::Schedule { .. } => "Schedule",
            Action::Companion { .. } => "Companion",
            Action::OpenCrafting { .. } => "Open crafting",
            Action::UnlockAchievement { .. } => "Unlock achievement",
            Action::Lockpick { .. } => "Pick a lock (DEX check)",
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
            Action::ChangeStanding { .. } => "Change standing",
            Action::Carry { .. } => "Carry prop",
            Action::Bark { .. } => "Bark (speech bubble)",
            Action::RushTo { .. } => "Rush to (fighters)",
            Action::ShowPicture { .. } => "Show picture",
            Action::MovePicture { .. } => "Move picture",
            Action::ErasePicture { .. } => "Erase picture",
            Action::TintScreen { .. } => "Tint screen",
            Action::FlashScreen { .. } => "Flash screen",
            Action::FadeOut { .. } => "Fade out",
            Action::FadeIn { .. } => "Fade in",
            Action::StartTimer { .. } => "Start timer",
            Action::StopTimer { .. } => "Stop timer",
            Action::InputNumber { .. } => "Input number",
            Action::InputText { .. } => "Input text",
            Action::MoveRoute { .. } => "Move route",
            Action::CallCommon { .. } => "Call common event",
            Action::Camera { .. } => "Camera",
            Action::ScrollText { .. } => "Scroll text",
            Action::PlayMusicEffect { .. } => "Play music effect",
            Action::SetFaction { .. } => "Set faction",
            Action::Check { save: false, .. } => "Ability check",
            Action::Check { save: true, .. } => "Saving throw",
            Action::Roll { .. } => "Roll dice",
            Action::ShortRest => "Short rest",
            Action::LongRest { .. } => "Long rest",
            Action::StartCombat => "Start fight",
            Action::SetClass { .. } => "Set class",
            Action::CastSpell { .. } => "Cast spell",
            Action::GiveMoney { .. } => "Give money",
            Action::TakeMoney { .. } => "Take money",
            Action::Fall { .. } => "Fall",
            Action::Nourish { .. } => "Eat / drink",
            Action::GroupCheck { .. } => "Group check",
            Action::Contest { .. } => "Contest",
            Action::DamageProp { .. } => "Damage prop",
            Action::DisarmTrap { .. } => "Disarm trap",
            Action::Poison { .. } => "Expose to poison",
            Action::ExposeDisease { .. } => "Expose to disease",
            Action::CureDisease { .. } => "Cure disease",
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
            Action::PlaySound { sound: s(), volume: 1.0, pitch: 1.0, pan: 0.0 },
            Action::PlayMusic { sound: s(), fade: 0.0 },
            Action::Heal { amount: 1 },
            Action::SetLocked { target: "self".into(), locked: false },
            Action::SetState { target: "self".into(), state: String::new() },
            Action::OpenShop { target: "self".into() },
            Action::Quest { quest: s(), op: crate::quest::QuestOp::Start, stage: 0 },
            Action::Patrol { target: "self".into(), active: true, points: Vec::new() },
            Action::SetTime { hour: 8.0 },
            Action::SetSkill { skill: String::new(), level: 1 },
            Action::LearnSpell { spell: String::new() },
            Action::AddMana { amount: 10 },
            Action::DamageAround { damage: 2, radius: 2.0, damage_type: None },
            Action::PlayerLight { radius: 5.0, seconds: 30.0, color: spell_light_color(), flicker: false },
            Action::Aim { function: s() },
            Action::Fly { sprite: FxSprite::default(), x: 0.0, y: 0.0, speed: 8.0, function: s() },
            Action::Effect { sprite: FxSprite::default(), x: 0.0, y: 0.0 },
            Action::DamageArea { x: 0.0, y: 0.0, radius: 2.0, damage: 2, damage_type: None },
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
            Action::ApplyEffect { target: "player".into(), effect: crate::srd::ConditionSpec::template(crate::srd::Condition::Poisoned) },
            Action::CureEffect { target: "player".into(), kind: Some(crate::srd::Condition::Poisoned) },
            Action::Shake { seconds: 0.5, strength: 1.5 },
            Action::SetWeather { map: s(), weather: Some(crate::map::Weather::new(crate::map::WeatherKind::Rain, 0.6)) },
            Action::Schedule { target: "self".into(), active: false },
            Action::Companion { target: "self".into(), join: true },
            Action::OpenCrafting { station: s(), title: s() },
            Action::UnlockAchievement { id: s() },
            Action::Lockpick { difficulty: 12, var: "picked".into() },
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
            Action::ChangeStanding { faction: String::new(), amount: 10 },
            Action::Carry { target: "player".into(), prop: String::new() },
            Action::Bark { target: "self".into(), lines: vec!["Fine weather today!".into()], seconds: 0.0 },
            Action::RushTo { target: s(), to: "self".into() },
            Action::ShowPicture {
                layer: 1,
                sheet: s(),
                frame: 0,
                anchor: crate::script_ui::Anchor::Center,
                on_map: false,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                opacity: 1.0,
                rotation: 0.0,
                tint: white(),
                blend: crate::pictures::Blend::Normal,
            },
            Action::MovePicture { layer: 1, to: crate::pictures::PictureTargets { opacity: Some(0.0), ..Default::default() }, seconds: 1.0, easing: crate::pictures::Easing::InOut, wait: false },
            Action::ErasePicture { layer: 1 },
            Action::TintScreen { color: [255, 140, 60, 70], seconds: 2.0, wait: false },
            Action::FlashScreen { color: [255, 255, 255, 200], seconds: 0.5 },
            Action::FadeOut { seconds: 1.0, color: [0, 0, 0], wait: true },
            Action::FadeIn { seconds: 1.0, wait: true },
            Action::StartTimer { name: "timer".into(), seconds: 60.0, shown: true, function: s(), event: s() },
            Action::StopTimer { name: "timer".into() },
            Action::InputNumber { text: "How many?".into(), var: "answer".into(), digits: 0, min: 0, max: 999 },
            Action::InputText { text: "What is your name?".into(), var: "name".into(), max_len: 16 },
            Action::MoveRoute { target: "self".into(), steps: vec![crate::routes::RouteStep::Move { dir: crate::entity::Facing::Down, tiles: 1 }], repeat: false, skip_blocked: true, wait: true },
            Action::CallCommon { id: s() },
            Action::Camera { aim: crate::camera::CameraAim::Tile { x: 0, y: 0 }, seconds: 1.0, easing: crate::pictures::Easing::InOut, zoom: 1.0, wait: true },
            Action::ScrollText { text: "# The End\n\nThank you for playing!".into(), speed: 40.0, background: [0, 0, 0, 255] },
            Action::PlayMusicEffect { sound: s(), wait: false },
            Action::SetFaction { target: "self".into(), faction: String::new() },
            Action::Check { what: "persuasion".into(), save: false, dc: 12, mode: crate::srd::Mode::Normal, then: Vec::new(), otherwise: Vec::new() },
            Action::Check { what: "dex".into(), save: true, dc: 12, mode: crate::srd::Mode::Normal, then: Vec::new(), otherwise: Vec::new() },
            Action::Roll { dice: "1d20".into(), var: "roll".into() },
            Action::ShortRest,
            Action::LongRest { hours: 8.0 },
            Action::StartCombat,
            Action::SetClass { target: "player".into(), class: "fighter".into(), level: 0 },
            Action::CastSpell { spell: String::new(), target: "player".into(), free: true },
            Action::GiveMoney { cp: 100 },
            Action::TakeMoney { cp: 100 },
            Action::Fall { feet: 20 },
            Action::Nourish { food: 1.0, water: 0.0 },
            Action::GroupCheck { what: "stealth".into(), dc: 12, then: Vec::new(), otherwise: Vec::new() },
            Action::Contest { what: "athletics".into(), target: String::new(), against: String::new(), then: Vec::new(), otherwise: Vec::new() },
            Action::DamageProp { target: "self".into(), amount: 5, damage_type: Some(crate::srd::DamageType::Bludgeoning) },
            Action::DisarmTrap { target: String::new() },
            Action::Poison { target: "player".into(), item: "poison-basic-vial".into() },
            Action::ExposeDisease { disease: String::new(), save: true },
            Action::CureDisease { disease: String::new() },
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
    fn text_variables_show_and_compare() {
        let mut w = WorldState::default();
        w.set_var("house_name", 3);
        w.texts.insert("house_name".into(), "Rosehill".into());
        assert_eq!(w.expand_text("Welcome home to {house_name}!"), "Welcome home to Rosehill!", "a text variable wins");
        let c = |value: &str, ignore_case: bool| Condition::new(Cond::TextVar { name: "house_name".into(), value: value.into(), ignore_case }).eval(&w, &mut |_| false);
        assert!(c("Rosehill", false) && c(" rosehill ", true) && !c("rosehill", false) && !c("", false));
        assert!(Condition::new(Cond::TextVar { name: "unset".into(), value: String::new(), ignore_case: false }).eval(&w, &mut |_| false));
    }

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
        // (A prop is defeated when it breaks: object statistics.)
        assert!(TriggerOwner::Character.allows(&TriggerOn::Defeat) && TriggerOwner::Prop.allows(&TriggerOn::Defeat) && !TriggerOwner::Pickup.allows(&TriggerOn::Defeat));
        assert!(!TriggerOwner::ItemUse.allows(&TriggerOn::Custom { name: "x".into() }));
        assert_eq!(TriggerOwner::Map.templates().iter().map(|t| t.label()).collect::<Vec<_>>(), vec!["Map enter", "Custom event", "Woke up", "Time reached", "Fight starts", "Fight ends"]);
        assert!(TriggerOwner::Character.allows(&TriggerOn::CombatStart) && !TriggerOwner::Prop.allows(&TriggerOn::CombatEnd));
        // Searched: zones, props and characters; Search found compares the last search.
        assert!([TriggerOwner::Zone, TriggerOwner::Prop, TriggerOwner::Character].iter().all(|o| o.allows(&TriggerOn::Search)) && !TriggerOwner::Map.allows(&TriggerOn::Search));
        assert!(TriggerOn::Search.matches(&GameEvent::Search) && !TriggerOn::Enter.matches(&GameEvent::Search));
        let mut w = WorldState::default();
        let found = |w: &WorldState| Condition::new(Cond::SearchRoll { dc: 15 }).eval(w, &mut |_| false);
        assert!(!found(&w), "no search yet");
        w.last_search = Some(15);
        assert!(found(&w));
        w.last_search = Some(14);
        assert!(!found(&w));
        w.training.sight = crate::srd::check::Sight::Dark;
        let r = w.roll_check("perception").unwrap();
        assert_eq!((r.total, r.success), (0, None), "searching in the dark finds nothing");
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
        let poisoned = [Condition::new(Cond::HasEffect { kind: crate::srd::Condition::Poisoned })];
        let any = [Condition::new(Cond::HasEffect { kind: crate::srd::Condition::Any })];
        assert!(!met(&poisoned, &w, &mut no_script) && !met(&any, &w, &mut no_script));
        crate::srd::conditions::apply(&mut w.effects, &crate::srd::ConditionSpec::new(crate::srd::Condition::Poisoned, 3.0));
        assert!(met(&poisoned, &w, &mut no_script) && met(&any, &w, &mut no_script));
        // A condition of 0.27 (a status effect's name) reads as the nearest condition.
        let old: Cond = ron::from_str("HasEffect(kind: Poison)").unwrap();
        assert_eq!(old, Cond::HasEffect { kind: crate::srd::Condition::Poisoned });
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
