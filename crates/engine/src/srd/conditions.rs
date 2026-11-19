//! The SRD's conditions (blinded, poisoned, prone… and exhaustion with its six levels) on the
//! hero and on characters, with their rules, and ongoing damage or healing per round (6 seconds).
//! They replaced the status effects of 0.27 (poison per second, haste, strength…): an old effect
//! reads as its nearest condition (`ConditionSpec`'s and `ActiveCondition`'s reprs). Weapons and
//! touches apply them, so do the *Apply condition* / *Remove condition* actions and scripts. The
//! hero's are in the save game (`WorldState::effects`); the runtime part is `runtime/effects.rs`.

use serde::{Deserialize, Serialize};

use super::damage::DamageType;

/// Seconds of a round: ongoing damage and healing come once per round.
pub const ROUND: f32 = 6.0;

/// The SRD's conditions. `Any` stands for "every condition" in *Has condition* / *Remove
/// condition*, and for an effect of 0.27 that has no condition (haste, strength…).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Condition {
    Blinded,
    Charmed,
    Deafened,
    Frightened,
    Grappled,
    Incapacitated,
    Invisible,
    Paralyzed,
    Petrified,
    #[serde(alias = "Poison")]
    Poisoned,
    Prone,
    #[serde(alias = "Slow")]
    Restrained,
    #[serde(alias = "Stun")]
    Stunned,
    Unconscious,
    /// Levels 1 to 6 ([`ActiveCondition::level`]).
    Exhaustion,
    #[serde(other)]
    Any,
}

impl Condition {
    /// The conditions (without `Any`).
    pub const ALL: [Condition; 15] = [
        Condition::Blinded,
        Condition::Charmed,
        Condition::Deafened,
        Condition::Frightened,
        Condition::Grappled,
        Condition::Incapacitated,
        Condition::Invisible,
        Condition::Paralyzed,
        Condition::Petrified,
        Condition::Poisoned,
        Condition::Prone,
        Condition::Restrained,
        Condition::Stunned,
        Condition::Unconscious,
        Condition::Exhaustion,
    ];

    /// The SRD's index id (`poisoned`), as scripts and status lines name it.
    pub fn id(self) -> &'static str {
        match self {
            Condition::Blinded => "blinded",
            Condition::Charmed => "charmed",
            Condition::Deafened => "deafened",
            Condition::Frightened => "frightened",
            Condition::Grappled => "grappled",
            Condition::Incapacitated => "incapacitated",
            Condition::Invisible => "invisible",
            Condition::Paralyzed => "paralyzed",
            Condition::Petrified => "petrified",
            Condition::Poisoned => "poisoned",
            Condition::Prone => "prone",
            Condition::Restrained => "restrained",
            Condition::Stunned => "stunned",
            Condition::Unconscious => "unconscious",
            Condition::Exhaustion => "exhaustion",
            Condition::Any => "any",
        }
    }

    /// "Poisoned".
    pub fn name(self) -> &'static str {
        match self {
            Condition::Blinded => "Blinded",
            Condition::Charmed => "Charmed",
            Condition::Deafened => "Deafened",
            Condition::Frightened => "Frightened",
            Condition::Grappled => "Grappled",
            Condition::Incapacitated => "Incapacitated",
            Condition::Invisible => "Invisible",
            Condition::Paralyzed => "Paralyzed",
            Condition::Petrified => "Petrified",
            Condition::Poisoned => "Poisoned",
            Condition::Prone => "Prone",
            Condition::Restrained => "Restrained",
            Condition::Stunned => "Stunned",
            Condition::Unconscious => "Unconscious",
            Condition::Exhaustion => "Exhaustion",
            Condition::Any => "Any",
        }
    }

    /// A condition by id or name; 0.27's effect names read as their nearest condition (`poison`,
    /// `stun`, `slow`), the others as `Any` (`None` = nothing known by that name).
    pub fn parse(s: &str) -> Option<Condition> {
        let s = s.trim().to_ascii_lowercase();
        if let Some(c) = Condition::ALL.into_iter().find(|c| c.id() == s) {
            return Some(c);
        }
        match s.as_str() {
            "poison" => Some(Condition::Poisoned),
            "stun" => Some(Condition::Stunned),
            "slow" => Some(Condition::Restrained),
            "any" | "all" | "" | "burn" | "regen" | "haste" | "strength" | "weakness" | "shield" => Some(Condition::Any),
            _ => None,
        }
    }

    /// The badge over heads and on the HUD (ASCII: the game font has nothing else).
    pub fn badge(self) -> &'static str {
        match self {
            Condition::Blinded => "BLD",
            Condition::Charmed => "CHM",
            Condition::Deafened => "DEF",
            Condition::Frightened => "FRT",
            Condition::Grappled => "GRP",
            Condition::Incapacitated => "INC",
            Condition::Invisible => "INV",
            Condition::Paralyzed => "PAR",
            Condition::Petrified => "PET",
            Condition::Poisoned => "PSN",
            Condition::Prone => "PRN",
            Condition::Restrained => "RST",
            Condition::Stunned => "STN",
            Condition::Unconscious => "UNC",
            Condition::Exhaustion => "EXH",
            Condition::Any => "",
        }
    }

    /// The badge colour.
    pub fn color(self) -> [u8; 3] {
        match self {
            Condition::Poisoned => [90, 190, 60],
            Condition::Stunned | Condition::Paralyzed | Condition::Incapacitated => [150, 150, 160],
            Condition::Petrified => [170, 160, 140],
            Condition::Prone | Condition::Grappled | Condition::Restrained => [90, 130, 200],
            Condition::Frightened => [200, 120, 220],
            Condition::Charmed => [230, 110, 170],
            Condition::Invisible => [180, 220, 240],
            Condition::Blinded | Condition::Deafened => [120, 110, 130],
            Condition::Unconscious => [110, 110, 150],
            Condition::Exhaustion => [210, 150, 70],
            Condition::Any => [200, 200, 200],
        }
    }

    /// What the SRD says it does (the editor's hover text, the manual).
    pub fn rules_text(self) -> &'static str {
        match self {
            Condition::Blinded => "Can't see: fails checks that need sight; attacks against it have advantage, its own disadvantage.",
            Condition::Charmed => "Can't attack the charmer; the charmer has advantage on social checks against it.",
            Condition::Deafened => "Can't hear: fails checks that need hearing.",
            Condition::Frightened => "Disadvantage on ability checks and attacks while the source is in sight; won't move closer to it.",
            Condition::Grappled => "Speed 0.",
            Condition::Incapacitated => "Can't take actions: no attacks, no using things.",
            Condition::Invisible => "Can't be seen: attacks against it have disadvantage, its own advantage.",
            Condition::Paralyzed => "Incapacitated, can't move; fails STR and DEX saves; hits against it are critical.",
            Condition::Petrified => "Turned to stone: incapacitated, can't move, fails STR and DEX saves, resistant to all damage.",
            Condition::Poisoned => "Disadvantage on attack rolls and ability checks.",
            Condition::Prone => "Lying down: crawls at half speed; its attacks have disadvantage.",
            Condition::Restrained => "Speed 0; disadvantage on attacks and DEX saves; attacks against it have advantage.",
            Condition::Stunned => "Incapacitated, can't move; fails STR and DEX saves.",
            Condition::Unconscious => "Incapacitated, can't move, lies prone; fails STR and DEX saves; hits against it are critical.",
            Condition::Exhaustion => "1: disadvantage on checks; 2: half speed; 3: disadvantage on attacks and saves; 4: max HP halved; 5: speed 0; 6: death.",
            Condition::Any => "",
        }
    }
}

/// A saving throw that keeps a condition off: the ability (`con`) against a DC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveDc {
    pub ability: String,
    pub dc: i32,
}

/// Damage (a type) or healing (no type) every round while it lasts: dice like `1d4` or `2`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ongoing {
    pub dice: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<DamageType>,
}

impl Ongoing {
    /// `1d4fire`, `2heal` (status lines).
    pub fn text(&self) -> String {
        format!("{}{}", self.dice.replace(' ', ""), self.damage_type.map_or("heal", |t| t.id()))
    }
}

/// What a weapon, a touch, an action or a script applies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SpecRepr")]
pub struct ConditionSpec {
    /// `None` = only the ongoing damage or healing.
    pub condition: Option<Condition>,
    /// How long it lasts (0 = until it is removed or a rest ends it).
    pub seconds: f32,
    /// Exhaustion: the levels it adds (0 = 1).
    #[serde(skip_serializing_if = "is_zero")]
    pub level: u8,
    /// The saving throw that keeps it off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<SaveDc>,
    /// Damage or healing per round while it lasts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ongoing: Option<Ongoing>,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl ConditionSpec {
    pub fn new(condition: Condition, seconds: f32) -> ConditionSpec {
        ConditionSpec { condition: Some(condition), seconds, level: 0, save: None, ongoing: None }
    }

    /// A new one for the editor.
    pub fn template(condition: Condition) -> ConditionSpec {
        ConditionSpec::new(condition, if condition == Condition::Exhaustion { 0.0 } else { 6.0 })
    }

    /// Nothing happens when it is applied (an old haste or strength effect).
    pub fn is_nothing(&self) -> bool {
        self.condition.is_none_or(|c| c == Condition::Any) && self.ongoing.is_none()
    }

    /// "Poisoned 12 s (CON DC 10) + 1 poison per round" (editor lists, hints).
    pub fn summary(&self) -> String {
        let mut s = match self.condition {
            Some(Condition::Exhaustion) => format!("Exhaustion +{}", self.level.max(1)),
            Some(c) if c != Condition::Any => c.name().to_string(),
            _ => String::new(),
        };
        if let Some(o) = &self.ongoing {
            let what = o.damage_type.map_or("healing".to_string(), |t| t.id().to_string());
            s = if s.is_empty() { format!("{} {what} per round", o.dice) } else { format!("{s} + {} {what} per round", o.dice) };
        }
        if s.is_empty() {
            return "nothing".into();
        }
        if self.seconds > 0.0 {
            s = format!("{s}, {} s", self.seconds);
        }
        if let Some(sv) = &self.save {
            s = format!("{s} ({} DC {})", sv.ability.to_ascii_uppercase(), sv.dc);
        }
        s
    }
}

/// 0.27's effect kinds, for reading old data (`None` = new data).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
enum OldKind {
    #[default]
    None,
    Poison,
    Burn,
    Regen,
    Slow,
    Haste,
    Stun,
    Strength,
    Weakness,
    Shield,
}

/// An old effect (`kind`, `strength` per second / percent / points) as a condition: poison =
/// poisoned + poison damage per round, burning = fire per round, regeneration = healing per
/// round, stun = stunned, slow = restrained; haste, strength, weakness and shield = nothing.
fn from_old(kind: OldKind, strength: f32, seconds: f32) -> ConditionSpec {
    let per_round = || ((strength * ROUND).round() as i64).max(1).to_string();
    let ongoing = |t: Option<DamageType>| Some(Ongoing { dice: per_round(), damage_type: t });
    let (condition, ongoing) = match kind {
        OldKind::Poison => (Some(Condition::Poisoned), ongoing(Some(DamageType::Poison))),
        OldKind::Burn => (None, ongoing(Some(DamageType::Fire))),
        OldKind::Regen => (None, ongoing(None)),
        OldKind::Stun => (Some(Condition::Stunned), None),
        OldKind::Slow => (Some(Condition::Restrained), None),
        OldKind::Haste | OldKind::Strength | OldKind::Weakness | OldKind::Shield | OldKind::None => (None, None),
    };
    ConditionSpec { condition, seconds: seconds.max(0.0), level: 0, save: None, ongoing }
}

/// What a script's `apply_effect(target, name, strength, seconds)` (deprecated since 0.28) gives:
/// an effect name of 0.27 as its condition, or a condition's id (`None` = unknown name).
pub fn from_effect(name: &str, strength: f32, seconds: f32) -> Option<ConditionSpec> {
    let old = match name.trim().to_ascii_lowercase().as_str() {
        "poison" => OldKind::Poison,
        "burn" => OldKind::Burn,
        "regen" => OldKind::Regen,
        "slow" => OldKind::Slow,
        "haste" => OldKind::Haste,
        "stun" => OldKind::Stun,
        "strength" => OldKind::Strength,
        "weakness" => OldKind::Weakness,
        "shield" => OldKind::Shield,
        other => return Condition::ALL.into_iter().find(|c| c.id() == other).map(|c| ConditionSpec::new(c, seconds)),
    };
    Some(from_old(old, strength, seconds))
}

#[derive(Deserialize)]
#[serde(default)]
struct SpecRepr {
    condition: Option<Condition>,
    seconds: f32,
    level: u8,
    save: Option<SaveDc>,
    ongoing: Option<Ongoing>,
    /// 0.27 and older.
    kind: OldKind,
    strength: f32,
}

impl Default for SpecRepr {
    fn default() -> Self {
        SpecRepr { condition: None, seconds: 0.0, level: 0, save: None, ongoing: None, kind: OldKind::None, strength: 0.0 }
    }
}

impl From<SpecRepr> for ConditionSpec {
    fn from(r: SpecRepr) -> Self {
        match r.kind {
            OldKind::None => ConditionSpec { condition: r.condition, seconds: r.seconds.max(0.0), level: r.level, save: r.save, ongoing: r.ongoing },
            k => from_old(k, r.strength, r.seconds),
        }
    }
}

/// A condition that is running.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "ActiveRepr")]
pub struct ActiveCondition {
    pub condition: Option<Condition>,
    /// Exhaustion's level (1-6).
    #[serde(skip_serializing_if = "is_zero")]
    pub level: u8,
    /// Seconds left (`None` = until removed).
    pub remaining: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ongoing: Option<Ongoing>,
    /// Seconds to the next round's ongoing damage or healing.
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub next_round: f32,
    /// What gave it: a spell's id (concentration ending removes it); "" = anything else.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// A spell's boon or bane instead of a condition (Bless, Shield of Faith, Mage Armor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buff: Option<Buff>,
}

/// A spell's boon or bane while it lasts (an [`ActiveCondition`] without a condition).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Buff {
    /// A die added to attack rolls (Bless 1d4), or taken off them (`minus`: Bane).
    AttackDie { dice: String, #[serde(default)] minus: bool },
    /// A die added to saving throws, or taken off them.
    SaveDie { dice: String, #[serde(default)] minus: bool },
    /// Armor class + `bonus` (Shield of Faith 2, Shield 5).
    Ac { bonus: i32 },
    /// Armor class at least `base` + DEX without body armor (Mage Armor 13).
    AcBase { base: i32 },
    /// Speed + `feet` (Longstrider 10; negative slows).
    Speed { feet: i32 },
    /// Haste: +2 AC, speed doubled, advantage on DEX saves.
    Haste,
    /// Resistance to damage types (`fire`; "all" = every type).
    Resistance { types: Vec<String> },
}

/// The buffs running in `list`.
pub fn buffs(list: &[ActiveCondition]) -> impl Iterator<Item = &Buff> {
    list.iter().filter_map(|a| a.buff.as_ref())
}

/// Starts a buff for `seconds` (0 = until removed) from `source`; the same source's buff of the
/// same kind is renewed.
pub fn apply_buff(list: &mut Vec<ActiveCondition>, buff: Buff, seconds: f32, source: &str) {
    list.retain(|a| !(a.source == source && a.buff.as_ref().is_some_and(|b| std::mem::discriminant(b) == std::mem::discriminant(&buff))));
    list.push(ActiveCondition { condition: None, level: 0, remaining: (seconds > 0.0).then_some(seconds), ongoing: None, next_round: 0.0, source: source.to_string(), buff: Some(buff) });
}

fn is_zero_f32(n: &f32) -> bool {
    *n == 0.0
}

#[derive(Deserialize)]
#[serde(default)]
struct ActiveRepr {
    condition: Option<Condition>,
    level: u8,
    /// `Some(2.0)` / `None`, or a plain `2.0` (0.27's effects).
    #[serde(deserialize_with = "seconds_or_none")]
    remaining: Option<f32>,
    ongoing: Option<Ongoing>,
    next_round: f32,
    source: String,
    buff: Option<Buff>,
    kind: OldKind,
    strength: f32,
}

impl Default for ActiveRepr {
    fn default() -> Self {
        ActiveRepr { condition: None, level: 0, remaining: None, ongoing: None, next_round: 0.0, source: String::new(), buff: None, kind: OldKind::None, strength: 0.0 }
    }
}

/// Reads `Some(2.0)`, `None` or a plain number.
fn seconds_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Option<f32>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("seconds, Some(seconds) or None")
        }
        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }
        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v as f32))
        }
        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D2: serde::Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            f32::deserialize(d).map(Some)
        }
    }
    d.deserialize_any(V)
}

impl From<ActiveRepr> for ActiveCondition {
    fn from(r: ActiveRepr) -> Self {
        match r.kind {
            OldKind::None => ActiveCondition { condition: r.condition, level: r.level, remaining: r.remaining, ongoing: r.ongoing, next_round: r.next_round, source: r.source, buff: r.buff },
            k => {
                // (An old effect's `remaining` was a plain number.)
                let s = from_old(k, r.strength, r.remaining.unwrap_or(0.0));
                ActiveCondition { condition: s.condition, level: 0, remaining: Some(s.seconds), ongoing: s.ongoing, next_round: ROUND, source: String::new(), buff: None }
            }
        }
    }
}

impl ActiveCondition {
    /// `poisoned:11.2`, `exhaustion2`, `ongoing:1d4fire:5.0` (status lines).
    pub fn status(&self) -> String {
        if self.buff.is_some() {
            let name = format!("buff:{}", self.source.split('@').next().unwrap_or(""));
            return match self.remaining {
                Some(r) => format!("{name}:{r:.1}"),
                None => name,
            };
        }
        let name = match self.condition {
            Some(Condition::Exhaustion) => format!("exhaustion{}", self.level),
            Some(c) if c != Condition::Any => c.id().to_string(),
            _ => "ongoing".to_string(),
        };
        let name = match (&self.ongoing, self.condition.is_some_and(|c| c != Condition::Any)) {
            (Some(o), true) => format!("{name}+{}", o.text()),
            (Some(o), false) => format!("{name}:{}", o.text()),
            _ => name,
        };
        match self.remaining {
            Some(r) => format!("{name}:{r:.1}"),
            None => name,
        }
    }

    /// The badge: `PSN 5`, `EXH2`, `FIR 3`, `REG 3`.
    pub fn badge(&self) -> String {
        let label = match (self.condition, &self.ongoing) {
            _ if self.buff.is_some() => self.source.split('@').next().unwrap_or("").chars().filter(|c| c.is_ascii_alphabetic()).take(3).collect::<String>().to_ascii_uppercase(),
            (Some(Condition::Exhaustion), _) => format!("EXH{}", self.level),
            (Some(c), _) if c != Condition::Any => c.badge().to_string(),
            (_, Some(o)) => o.damage_type.map_or("REG".to_string(), |t| t.id()[..3].to_ascii_uppercase()),
            _ => "?".into(),
        };
        match self.remaining {
            Some(r) => format!("{label} {}", r.ceil() as i32),
            None => label,
        }
    }

    /// The badge colour.
    pub fn color(&self) -> [u8; 3] {
        if self.buff.is_some() {
            return [120, 170, 240];
        }
        match (self.condition, &self.ongoing) {
            (Some(c), _) if c != Condition::Any => c.color(),
            (_, Some(o)) if o.damage_type.is_none() => [230, 80, 120],
            (_, Some(o)) if o.damage_type == Some(DamageType::Fire) => [240, 120, 30],
            _ => [200, 160, 90],
        }
    }
}

/// Starts what `spec` gives (its saving throw already failed or there is none). A condition that
/// is running lasts the longer of the two; exhaustion adds levels (up to 6); ongoing damage of
/// the same dice and type restarts.
pub fn apply(list: &mut Vec<ActiveCondition>, spec: &ConditionSpec) {
    apply_from(list, spec, "");
}

/// [`apply`] with what gave it (a spell's id: [`remove_source`] ends it).
pub fn apply_from(list: &mut Vec<ActiveCondition>, spec: &ConditionSpec, source: &str) {
    if spec.is_nothing() {
        return;
    }
    let remaining = (spec.seconds > 0.0).then_some(spec.seconds);
    let condition = spec.condition.filter(|c| *c != Condition::Any);
    let same = |a: &ActiveCondition| match condition {
        Some(c) => a.condition == Some(c),
        None => a.condition.is_none() && a.ongoing == spec.ongoing,
    };
    match list.iter_mut().find(|a| same(a)) {
        Some(a) => {
            a.remaining = match (a.remaining, remaining) {
                (None, _) | (_, None) => None,
                (Some(x), Some(y)) => Some(x.max(y)),
            };
            if condition == Some(Condition::Exhaustion) {
                a.level = (a.level + spec.level.max(1)).min(6);
            }
            if spec.ongoing.is_some() {
                a.ongoing = spec.ongoing.clone();
            }
            if !source.is_empty() {
                a.source = source.to_string();
            }
        }
        None => list.push(ActiveCondition {
            condition,
            level: if condition == Some(Condition::Exhaustion) { spec.level.clamp(1, 6) } else { 0 },
            remaining,
            ongoing: spec.ongoing.clone(),
            next_round: 0.0,
            source: source.to_string(),
            buff: None,
        }),
    }
}

/// Ends the conditions `source` gave (a spell whose concentration ended). Returns how many.
pub fn remove_source(list: &mut Vec<ActiveCondition>, source: &str) -> usize {
    let before = list.len();
    list.retain(|a| source.is_empty() || a.source != source);
    before - list.len()
}

/// Ends a condition (`Any` = everything, ongoing damage too). Returns how many ended.
pub fn remove(list: &mut Vec<ActiveCondition>, condition: Condition) -> usize {
    let before = list.len();
    list.retain(|a| condition != Condition::Any && a.condition != Some(condition));
    before - list.len()
}

/// Lowers exhaustion by `levels` (it ends at 0).
pub fn lower_exhaustion(list: &mut Vec<ActiveCondition>, levels: u8) {
    for a in list.iter_mut().filter(|a| a.condition == Some(Condition::Exhaustion)) {
        a.level = a.level.saturating_sub(levels);
    }
    list.retain(|a| a.condition != Some(Condition::Exhaustion) || a.level > 0);
}

/// Sets exhaustion to `level` (0 = none).
pub fn set_exhaustion(list: &mut Vec<ActiveCondition>, level: u8) {
    remove(list, Condition::Exhaustion);
    if level > 0 {
        apply(list, &ConditionSpec { level: level.min(6), ..ConditionSpec::new(Condition::Exhaustion, 0.0) });
    }
}

/// Exhaustion's level (0 = none).
pub fn exhaustion(list: &[ActiveCondition]) -> u8 {
    list.iter().filter(|a| a.condition == Some(Condition::Exhaustion)).map(|a| a.level).max().unwrap_or(0)
}

pub fn has(list: &[ActiveCondition], c: Condition) -> bool {
    if c == Condition::Any {
        return list.iter().any(|a| a.condition.is_some_and(|x| x != Condition::Any));
    }
    list.iter().any(|a| a.condition == Some(c))
}

/// One round's ongoing damage or healing: the amount rolled and its type (`None` = healing).
pub type Tick = (i32, Option<DamageType>);

/// Advances the conditions by `dt` seconds, drops those that ran out and returns the ongoing
/// damage and healing that came due (once when it starts, then every round while it lasts).
pub fn tick(list: &mut Vec<ActiveCondition>, dt: f32) -> Vec<Tick> {
    // (A little slack: a 12 second condition gives two rounds, not a third at its very end.)
    const EPS: f32 = 1e-3;
    let mut out = Vec::new();
    for a in list.iter_mut() {
        if let Some(r) = &mut a.remaining {
            *r -= dt;
        }
        if a.remaining.is_some_and(|r| r <= EPS) {
            continue;
        }
        if let Some(o) = &a.ongoing {
            a.next_round -= dt;
            if a.next_round <= EPS {
                a.next_round += ROUND;
                let amount = super::Expr::parse(&o.dice).map(|e| e.roll(&super::dice::NoMods).total).unwrap_or(0).max(0) as i32;
                out.push((amount, o.damage_type));
            }
        }
    }
    // (An old save's haste or strength is nothing now.)
    list.retain(|a| a.remaining.is_none_or(|r| r > EPS) && (a.condition.is_some_and(|c| c != Condition::Any) || a.ongoing.is_some() || a.buff.is_some()));
    out
}

/// Ends the conditions that have a duration (a long rest: hours have passed).
pub fn end_timed(list: &mut Vec<ActiveCondition>) {
    list.retain(|a| a.remaining.is_none());
}

/// Whether whoever has `list` can act (attack, interact, use things).
pub fn can_act(list: &[ActiveCondition]) -> bool {
    list.is_empty() || rules(list).can_act
}

/// What a creature's conditions do, all together.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    /// Disadvantage on ability checks (poisoned, frightened, exhaustion 1+).
    pub check_disadvantage: bool,
    /// Disadvantage on every saving throw (exhaustion 3+).
    pub save_disadvantage: bool,
    /// Disadvantage on DEX saving throws (restrained).
    pub dex_save_disadvantage: bool,
    /// STR and DEX saving throws fail (paralyzed, petrified, stunned, unconscious).
    pub str_dex_saves_fail: bool,
    /// Checks that need sight fail (blinded).
    pub sight_checks_fail: bool,
    /// Checks that need hearing fail (deafened).
    pub hearing_checks_fail: bool,
    /// Disadvantage on attack rolls (for 0.29's attacks).
    pub attack_disadvantage: bool,
    /// Movement: 1 = as usual, 0.5 = half, 0 = none.
    pub speed: f32,
    /// Can attack, interact, use items and cast (not incapacitated).
    pub can_act: bool,
    /// Lies on the ground (prone, unconscious).
    pub lying: bool,
    pub invisible: bool,
    pub blinded: bool,
    pub charmed: bool,
    pub frightened: bool,
    /// Hits against it are critical (paralyzed, unconscious).
    pub hits_crit: bool,
    /// Resistant to all damage (petrified).
    pub resist_all: bool,
    /// Max HP halved (exhaustion 4+).
    pub max_hp_half: bool,
    /// Exhaustion 6.
    pub dead: bool,
}

/// What `list` does to whoever has it.
pub fn rules(list: &[ActiveCondition]) -> Rules {
    use Condition::*;
    let on = |c: Condition| has(list, c);
    let exh = exhaustion(list);
    let incapacitated = on(Incapacitated) || on(Paralyzed) || on(Petrified) || on(Stunned) || on(Unconscious);
    let still = on(Grappled) || on(Restrained) || on(Paralyzed) || on(Petrified) || on(Stunned) || on(Unconscious) || exh >= 5;
    let half = on(Prone) || exh >= 2;
    Rules {
        check_disadvantage: on(Poisoned) || on(Frightened) || exh >= 1,
        save_disadvantage: exh >= 3,
        dex_save_disadvantage: on(Restrained),
        str_dex_saves_fail: on(Paralyzed) || on(Petrified) || on(Stunned) || on(Unconscious),
        sight_checks_fail: on(Blinded),
        hearing_checks_fail: on(Deafened),
        attack_disadvantage: on(Poisoned) || on(Frightened) || on(Prone) || on(Restrained) || on(Blinded) || exh >= 3,
        speed: if still {
            0.0
        } else if half {
            0.5
        } else {
            1.0
        },
        can_act: !incapacitated && exh < 6,
        lying: on(Prone) || on(Unconscious),
        invisible: on(Invisible),
        blinded: on(Blinded),
        charmed: on(Charmed),
        frightened: on(Frightened),
        hits_crit: on(Paralyzed) || on(Unconscious),
        resist_all: on(Petrified),
        max_hp_half: exh >= 4,
        dead: exh >= 6,
    }
}

/// Status lines: every condition's [`ActiveCondition::status`].
pub fn describe(list: &[ActiveCondition]) -> Vec<String> {
    list.iter().map(ActiveCondition::status).collect()
}

/// Whether a check of `what` (a skill id or `ability:tool`) needs sight / hearing (blinded,
/// deafened characters fail it).
pub fn needs_sight(what: &str) -> bool {
    matches!(what, "perception" | "investigation")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(c: Condition) -> Vec<ActiveCondition> {
        let mut l = Vec::new();
        apply(&mut l, &ConditionSpec::new(c, 10.0));
        l
    }

    #[test]
    fn every_condition_has_its_rules() {
        let none = rules(&[]);
        assert_eq!((none.speed, none.can_act, none.check_disadvantage, none.lying), (1.0, true, false, false));
        use Condition::*;
        for c in Condition::ALL {
            let r = rules(&with(c));
            let expect_speed = match c {
                Grappled | Restrained | Paralyzed | Petrified | Stunned | Unconscious => 0.0,
                Prone => 0.5,
                _ => 1.0,
            };
            assert_eq!(r.speed, expect_speed, "{c:?} speed");
            assert_eq!(r.can_act, !matches!(c, Incapacitated | Paralyzed | Petrified | Stunned | Unconscious), "{c:?} acts");
            assert_eq!(r.check_disadvantage, matches!(c, Poisoned | Frightened | Exhaustion), "{c:?} checks");
            assert_eq!(r.str_dex_saves_fail, matches!(c, Paralyzed | Petrified | Stunned | Unconscious), "{c:?} saves");
            assert_eq!(r.lying, matches!(c, Prone | Unconscious), "{c:?} lying");
            assert_eq!(r.hits_crit, matches!(c, Paralyzed | Unconscious), "{c:?} crits");
            assert_eq!(r.resist_all, c == Petrified);
            assert_eq!(r.sight_checks_fail, c == Blinded);
            assert_eq!(r.hearing_checks_fail, c == Deafened);
            assert_eq!(r.dex_save_disadvantage, c == Restrained);
            assert_eq!((r.invisible, r.blinded, r.charmed, r.frightened), (c == Invisible, c == Blinded, c == Charmed, c == Frightened));
            assert!(!c.badge().is_empty() && c.badge().is_ascii());
            assert_eq!(Condition::parse(c.id()), Some(c));
        }
    }

    #[test]
    fn exhaustion_has_six_levels() {
        let mut l = Vec::new();
        let level = |l: &Vec<ActiveCondition>| (exhaustion(l), rules(l));
        apply(&mut l, &ConditionSpec::new(Condition::Exhaustion, 0.0));
        let (n, r) = level(&l);
        assert_eq!((n, r.check_disadvantage, r.speed), (1, true, 1.0));
        apply(&mut l, &ConditionSpec::new(Condition::Exhaustion, 0.0));
        assert_eq!((exhaustion(&l), rules(&l).speed), (2, 0.5));
        apply(&mut l, &ConditionSpec { level: 2, ..ConditionSpec::new(Condition::Exhaustion, 0.0) });
        let (n, r) = level(&l);
        assert_eq!((n, r.save_disadvantage, r.max_hp_half, r.speed), (4, true, true, 0.5));
        apply(&mut l, &ConditionSpec::new(Condition::Exhaustion, 0.0));
        assert_eq!(rules(&l).speed, 0.0);
        apply(&mut l, &ConditionSpec { level: 3, ..ConditionSpec::new(Condition::Exhaustion, 0.0) });
        assert_eq!(exhaustion(&l), 6);
        assert!(rules(&l).dead);
        lower_exhaustion(&mut l, 1);
        assert_eq!(exhaustion(&l), 5);
        tick(&mut l, 1000.0);
        assert_eq!(exhaustion(&l), 5, "until a rest");
        set_exhaustion(&mut l, 0);
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn ongoing_damage_comes_every_round() {
        let mut l = Vec::new();
        let spec = ConditionSpec { ongoing: Some(Ongoing { dice: "2".into(), damage_type: Some(DamageType::Poison) }), ..ConditionSpec::new(Condition::Poisoned, 12.0) };
        apply(&mut l, &spec);
        let mut hits = Vec::new();
        for _ in 0..130 {
            hits.extend(tick(&mut l, 0.1));
        }
        // At once, after 6 s; then it is over (12 s).
        assert_eq!(hits, vec![(2, Some(DamageType::Poison)), (2, Some(DamageType::Poison))]);
        assert!(l.is_empty());
        apply(&mut l, &ConditionSpec { condition: None, seconds: 0.0, level: 0, save: None, ongoing: Some(Ongoing { dice: "1".into(), damage_type: None }) });
        assert_eq!(tick(&mut l, 0.1), vec![(1, None)], "healing");
        assert_eq!(remove(&mut l, Condition::Any), 1);
    }

    #[test]
    fn applying_again_lasts_longer() {
        let mut l = with(Condition::Prone);
        tick(&mut l, 8.0);
        apply(&mut l, &ConditionSpec::new(Condition::Prone, 1.0));
        assert_eq!((l.len(), l[0].remaining), (1, Some(2.0)));
        apply(&mut l, &ConditionSpec::new(Condition::Prone, 0.0));
        assert_eq!(l[0].remaining, None, "until removed");
        assert!(has(&l, Condition::Any));
        assert_eq!(remove(&mut l, Condition::Poisoned), 0);
        assert_eq!(remove(&mut l, Condition::Prone), 1);
        assert!(!has(&l, Condition::Any));
    }

    #[test]
    fn effects_of_0_27_read_as_conditions() {
        let s: ConditionSpec = ron::from_str("(kind: Poison, strength: 0.5, seconds: 6.0)").unwrap();
        assert_eq!(s.condition, Some(Condition::Poisoned));
        assert_eq!(s.ongoing, Some(Ongoing { dice: "3".into(), damage_type: Some(DamageType::Poison) }));
        let s: ConditionSpec = ron::from_str("(kind: Regen, strength: 1.0, seconds: 8.0)").unwrap();
        assert_eq!((s.condition, s.ongoing.as_ref().map(|o| o.dice.as_str()), s.ongoing.as_ref().and_then(|o| o.damage_type)), (None, Some("6"), None));
        let s: ConditionSpec = ron::from_str("(kind: Slow, strength: 40.0, seconds: 3.0)").unwrap();
        assert_eq!((s.condition, s.seconds), (Some(Condition::Restrained), 3.0));
        let s: ConditionSpec = ron::from_str("(kind: Haste, strength: 30.0, seconds: 30.0)").unwrap();
        assert!(s.is_nothing());
        assert_eq!(s.summary(), "nothing");
        let a: ActiveCondition = ron::from_str("(kind: Burn, strength: 1.0, remaining: 2.0)").unwrap();
        assert_eq!((a.condition, a.remaining, a.ongoing.and_then(|o| o.damage_type)), (None, Some(2.0), Some(DamageType::Fire)));
        // Names in conditions and cures: old ones read as their condition, the rest as Any.
        let c: Vec<Condition> = ron::from_str("[Poison, Stun, Burn, Haste, Prone]").unwrap();
        assert_eq!(c, vec![Condition::Poisoned, Condition::Stunned, Condition::Any, Condition::Any, Condition::Prone]);
        // The new format round-trips.
        let s = ConditionSpec { level: 2, save: Some(SaveDc { ability: "con".into(), dc: 10 }), ongoing: Some(Ongoing { dice: "1d4".into(), damage_type: Some(DamageType::Acid) }), ..ConditionSpec::new(Condition::Exhaustion, 0.0) };
        let back: ConditionSpec = ron::from_str(&ron::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert_eq!(s.summary(), "Exhaustion +2 + 1d4 acid per round (CON DC 10)");
        let a = ActiveCondition { condition: Some(Condition::Poisoned), level: 0, remaining: Some(3.0), ongoing: None, next_round: 0.0, source: String::new(), buff: None };
        let back: ActiveCondition = ron::from_str(&ron::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a);
    }
}
