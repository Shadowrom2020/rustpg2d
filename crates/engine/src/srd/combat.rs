//! The SRD's combat rules as pure functions: attack rolls against armor class with advantage and
//! disadvantage, critical hits, cover, damage dice, death saving throws and movement costs. The
//! turns themselves (who acts, the grid, the UI) are `runtime/turns/`.

use super::dice::{Mode, D20};

/// Feet per tile: a tile is a 5 ft square.
pub const FEET_PER_TILE: u32 = 5;

/// Cover between an attacker and its target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cover {
    #[default]
    None,
    /// +2 AC and DEX saves.
    Half,
    /// +5 AC and DEX saves.
    ThreeQuarters,
    /// Can't be targeted.
    Total,
}

impl Cover {
    pub fn ac_bonus(self) -> i32 {
        match self {
            Cover::None | Cover::Total => 0,
            Cover::Half => 2,
            Cover::ThreeQuarters => 5,
        }
    }

    /// The cover of a line blocked in `blocked` of `lines` lines (attacker's corner to the
    /// target's corners): none, half (some), three-quarters (most), total (all).
    pub fn of_lines(blocked: usize, lines: usize) -> Cover {
        match (blocked, lines) {
            (0, _) | (_, 0) => Cover::None,
            (b, l) if b >= l => Cover::Total,
            (b, l) if b * 4 >= l * 3 => Cover::ThreeQuarters,
            _ => Cover::Half,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Cover::None => "none",
            Cover::Half => "half",
            Cover::ThreeQuarters => "three-quarters",
            Cover::Total => "total",
        }
    }
}

/// What speaks for advantage and for disadvantage on an attack roll; both cancel out (the SRD).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Odds {
    pub advantage: bool,
    pub disadvantage: bool,
}

impl Odds {
    pub fn mode(self) -> Mode {
        match (self.advantage, self.disadvantage) {
            (true, false) => Mode::Advantage,
            (false, true) => Mode::Disadvantage,
            _ => Mode::Normal,
        }
    }
}

/// Everything about an attack that decides advantage and disadvantage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackSituation {
    pub ranged: bool,
    /// The target is within 5 ft of the attacker.
    pub adjacent: bool,
    /// A hostile creature that can act is within 5 ft of a ranged attacker.
    pub enemy_adjacent: bool,
    /// Beyond the normal range (up to the long range).
    pub long_range: bool,
    /// The attacker's conditions give disadvantage on its attacks (poisoned, prone, restrained,
    /// blinded, frightened, exhaustion 3+).
    pub attacker_disadvantage: bool,
    /// The attacker can't be seen (invisible, hidden).
    pub attacker_unseen: bool,
    pub target_prone: bool,
    /// Attacks against the target have advantage (restrained, paralyzed, stunned, unconscious,
    /// blinded).
    pub target_helpless: bool,
    /// The target can't be seen (invisible).
    pub target_unseen: bool,
    /// The target took the Dodge action (and can see the attacker).
    pub target_dodging: bool,
    /// An ally helped against this target.
    pub helped: bool,
}

/// The SRD's advantage and disadvantage on an attack roll.
pub fn odds(s: &AttackSituation) -> Odds {
    let mut o = Odds::default();
    if s.attacker_unseen || s.target_helpless || s.helped {
        o.advantage = true;
    }
    if s.attacker_disadvantage || s.target_unseen || s.target_dodging {
        o.disadvantage = true;
    }
    if s.target_prone {
        if s.adjacent && !s.ranged {
            o.advantage = true;
        } else {
            o.disadvantage = true;
        }
    }
    if s.ranged && (s.enemy_adjacent || s.long_range) {
        o.disadvantage = true;
    }
    o
}

/// An attack roll, resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackRoll {
    /// The d20 kept.
    pub d20: u32,
    /// Both d20s with advantage / disadvantage ("14/6"), else one.
    pub other: Option<u32>,
    pub bonus: i32,
    pub total: i32,
    pub ac: i32,
    pub hit: bool,
    pub crit: bool,
    pub mode: Mode,
}

/// Rolls an attack: d20 (+ the mode) + `bonus` against `ac`; a natural 20 hits and is critical,
/// a natural 1 misses; `crit_on`: the lowest natural roll that is critical (20; 19 for a keen
/// weapon), and `auto_crit`: every hit is critical (a helpless target within 5 ft).
pub fn attack(bonus: i32, ac: i32, mode: Mode, crit_on: u32, auto_crit: bool) -> AttackRoll {
    let d = D20::roll(mode);
    resolve(d.kept, d.other, bonus, ac, mode, crit_on, auto_crit)
}

/// [`attack`] with the dice given (tests).
pub fn resolve(d20: u32, other: Option<u32>, bonus: i32, ac: i32, mode: Mode, crit_on: u32, auto_crit: bool) -> AttackRoll {
    let total = d20 as i32 + bonus;
    let hit = d20 != 1 && (d20 == 20 || total >= ac);
    let crit = hit && (d20 >= crit_on.clamp(2, 20) || auto_crit);
    AttackRoll { d20, other, bonus, total, ac, hit, crit, mode }
}

impl AttackRoll {
    /// The chance (0-1) that an attack with `bonus` against `ac` hits, with `mode` (the UI's
    /// "65%").
    pub fn chance(bonus: i32, ac: i32, mode: Mode) -> f32 {
        let one = (1..=20).filter(|d| *d != 1 && (*d == 20 || *d + bonus >= ac)).count() as f32 / 20.0;
        match mode {
            Mode::Normal => one,
            Mode::Advantage => 1.0 - (1.0 - one) * (1.0 - one),
            Mode::Disadvantage => one * one,
        }
    }

    /// "d20 (14/6) +5 = 19 vs AC 15: hit".
    pub fn text(&self) -> String {
        let dice = match self.other {
            Some(o) => format!("{}/{}", self.d20, o),
            None => self.d20.to_string(),
        };
        format!("d20 ({dice}) {:+} = {} vs AC {}", self.bonus, self.total, self.ac)
    }
}

/// A damage expression with its dice doubled for a critical hit ("1d8+3" -> "2d8+3").
pub fn crit_dice(expr: &str) -> String {
    let mut out = String::new();
    let mut num = String::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            num.push(c);
        } else if c == 'd' || c == 'D' {
            let n: u32 = if num.is_empty() { 1 } else { num.parse().unwrap_or(1) };
            out.push_str(&(n * 2).to_string());
            out.push('d');
            num.clear();
        } else {
            out.push_str(&num);
            num.clear();
            out.push(c);
        }
        i += 1;
    }
    out.push_str(&num);
    out
}

/// A death saving throw's state (the hero and party members at 0 HP).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeathSaves {
    pub successes: u8,
    pub failures: u8,
    pub stable: bool,
}

/// What a death saving throw (or a hit while down) led to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathOutcome {
    Going,
    Stable,
    /// A natural 20: back on 1 HP.
    Up,
    Dead,
}

impl DeathSaves {
    /// A death saving throw with the d20 `roll`: 10+ a success, a natural 20 = 1 HP, a natural 1
    /// two failures; three successes = stable, three failures = dead.
    pub fn roll(&mut self, roll: u32) -> DeathOutcome {
        match roll {
            20 => {
                *self = DeathSaves::default();
                return DeathOutcome::Up;
            }
            1 => self.failures += 2,
            r if r >= 10 => self.successes += 1,
            _ => self.failures += 1,
        }
        self.outcome()
    }

    /// Damage while at 0 HP: a failure (a critical hit, two).
    pub fn hit(&mut self, crit: bool) -> DeathOutcome {
        self.stable = false;
        self.failures += if crit { 2 } else { 1 };
        self.outcome()
    }

    fn outcome(&mut self) -> DeathOutcome {
        if self.failures >= 3 {
            DeathOutcome::Dead
        } else if self.successes >= 3 {
            self.stable = true;
            DeathOutcome::Stable
        } else {
            DeathOutcome::Going
        }
    }
}

/// The feet a step costs: 5, double on difficult terrain (diagonals cost the same, the SRD's
/// simple rule).
/// Instant death from massive damage: damage that drops a creature to 0 HP and still has at
/// least its hit point maximum left (`hp_before` 0 = already down: the damage alone).
pub fn massive_damage(hp_before: i32, damage: i32, max_hp: i32) -> bool {
    max_hp > 0 && damage - hp_before.max(0) >= max_hp
}

/// Whether a creature of size `by` can grapple or shove one of size `target`: at most one size
/// larger.
pub fn can_grapple(by: super::Size, target: super::Size) -> bool {
    target as u8 <= by as u8 + 1
}

/// Whether dragging a grappled creature of size `held` halves the speed of one of size `by`:
/// unless it is two or more sizes smaller.
pub fn dragging_halves(by: super::Size, held: super::Size) -> bool {
    held as u8 + 2 > by as u8
}

/// The ammunition found again after a fight: half of what was shot, rounded down (the SRD's
/// *Ammunition* property).
pub fn recovered_ammo(shot: u32) -> u32 {
    shot / 2
}

pub fn step_cost(difficult: bool) -> u32 {
    if difficult {
        2 * FEET_PER_TILE
    } else {
        FEET_PER_TILE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapples_by_size() {
        use crate::srd::Size::*;
        assert!(can_grapple(Medium, Large) && !can_grapple(Medium, Huge) && can_grapple(Small, Medium) && !can_grapple(Small, Large));
        assert!(dragging_halves(Medium, Medium) && dragging_halves(Medium, Small) && !dragging_halves(Medium, Tiny));
    }

    #[test]
    fn half_the_ammunition_comes_back() {
        assert_eq!((recovered_ammo(0), recovered_ammo(1), recovered_ammo(5), recovered_ammo(20)), (0, 0, 2, 10));
    }

    #[test]
    fn massive_damage_kills_outright() {
        // A 12 HP hero at 6 HP: 18 damage leaves 12 past 0 = its maximum.
        assert!(massive_damage(6, 18, 12));
        assert!(!massive_damage(6, 17, 12));
        // Already down: the damage alone.
        assert!(massive_damage(0, 12, 12));
        assert!(!massive_damage(0, 11, 12));
        assert!(!massive_damage(5, 100, 0), "no maximum, no rule");
    }

    #[test]
    fn advantage_and_disadvantage_come_from_the_situation() {
        let base = AttackSituation::default();
        assert_eq!(odds(&base).mode(), Mode::Normal);
        assert_eq!(odds(&AttackSituation { target_prone: true, adjacent: true, ..base }).mode(), Mode::Advantage);
        assert_eq!(odds(&AttackSituation { target_prone: true, ranged: true, ..base }).mode(), Mode::Disadvantage);
        assert_eq!(odds(&AttackSituation { ranged: true, enemy_adjacent: true, ..base }).mode(), Mode::Disadvantage);
        assert_eq!(odds(&AttackSituation { ranged: true, long_range: true, ..base }).mode(), Mode::Disadvantage);
        assert_eq!(odds(&AttackSituation { target_helpless: true, attacker_disadvantage: true, ..base }).mode(), Mode::Normal, "both cancel out");
        assert_eq!(odds(&AttackSituation { attacker_unseen: true, ..base }).mode(), Mode::Advantage);
        assert_eq!(odds(&AttackSituation { target_dodging: true, ..base }).mode(), Mode::Disadvantage);
        assert_eq!(odds(&AttackSituation { helped: true, ..base }).mode(), Mode::Advantage);
    }

    #[test]
    fn attacks_hit_miss_and_crit() {
        let r = resolve(12, None, 5, 17, Mode::Normal, 20, false);
        assert!(r.hit && !r.crit && r.total == 17, "meets the AC");
        assert!(!resolve(11, None, 5, 17, Mode::Normal, 20, false).hit);
        let nat20 = resolve(20, None, -5, 30, Mode::Normal, 20, false);
        assert!(nat20.hit && nat20.crit);
        assert!(!resolve(1, None, 30, 10, Mode::Normal, 20, false).hit, "a natural 1 misses");
        assert!(resolve(19, None, 0, 10, Mode::Normal, 19, false).crit, "keen: 19-20");
        assert!(resolve(5, None, 10, 10, Mode::Normal, 20, true).crit, "helpless target within 5 ft");
        assert!(!resolve(5, None, 0, 10, Mode::Normal, 20, true).crit, "a miss is no crit");
        assert_eq!(resolve(14, Some(6), 3, 12, Mode::Advantage, 20, false).text(), "d20 (14/6) +3 = 17 vs AC 12");
        assert!((AttackRoll::chance(5, 15, Mode::Normal) - 0.55).abs() < 1e-6);
        assert!((AttackRoll::chance(5, 15, Mode::Advantage) - (1.0 - 0.45 * 0.45)).abs() < 1e-6);
        assert!((AttackRoll::chance(0, 40, Mode::Normal) - 0.05).abs() < 1e-6, "a 20 always hits");
        assert!((AttackRoll::chance(40, 5, Mode::Normal) - 0.95).abs() < 1e-6, "a 1 always misses");
    }

    #[test]
    fn critical_hits_double_the_dice() {
        assert_eq!(crit_dice("1d8+3"), "2d8+3");
        assert_eq!(crit_dice("2d6"), "4d6");
        assert_eq!(crit_dice("d4+STR"), "2d4+STR");
        assert_eq!(crit_dice("1d6+1d4"), "2d6+2d4");
        assert_eq!(crit_dice("5"), "5");
    }

    #[test]
    fn cover_from_blocked_lines() {
        assert_eq!(Cover::of_lines(0, 4), Cover::None);
        assert_eq!(Cover::of_lines(1, 4), Cover::Half);
        assert_eq!(Cover::of_lines(3, 4), Cover::ThreeQuarters);
        assert_eq!(Cover::of_lines(4, 4), Cover::Total);
        assert_eq!((Cover::Half.ac_bonus(), Cover::ThreeQuarters.ac_bonus()), (2, 5));
    }

    #[test]
    fn death_saves() {
        let mut d = DeathSaves::default();
        assert_eq!(d.roll(12), DeathOutcome::Going);
        assert_eq!(d.roll(3), DeathOutcome::Going);
        assert_eq!(d.roll(15), DeathOutcome::Going);
        assert_eq!(d.roll(10), DeathOutcome::Stable);
        assert!(d.stable);
        let mut d = DeathSaves::default();
        assert_eq!(d.roll(1), DeathOutcome::Going, "a natural 1 is two failures");
        assert_eq!(d.failures, 2);
        assert_eq!(d.hit(false), DeathOutcome::Dead);
        let mut d = DeathSaves { failures: 2, ..Default::default() };
        assert_eq!(d.roll(20), DeathOutcome::Up);
        assert_eq!(d, DeathSaves::default());
        let mut d = DeathSaves::default();
        assert_eq!(d.hit(true), DeathOutcome::Going);
        assert_eq!(d.failures, 2, "a critical hit while down: two failures");
        assert_eq!((step_cost(false), step_cost(true)), (5, 10));
    }
}
