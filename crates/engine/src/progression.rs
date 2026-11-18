//! Experience and levels (optional per project, `ProjectConfig::progression`): defeated
//! characters and completed quests give XP, levels follow the SRD 5.1's experience table, and
//! every level raises the hero's hit points (a hit die: its average or a roll, plus CON), the
//! mana (with magic) and the proficiency bonus. The hero's XP and level are in the save game
//! (`WorldState::xp`, `WorldState::level`); the runtime part is `runtime/progression.rs`.

use serde::{Deserialize, Serialize};

/// How the hero's hit points grow per level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitPoints {
    /// The hit die's average (6 for a d10), plus CON.
    #[default]
    Average,
    /// A roll of the hit die (the seeded dice), plus CON.
    Rolled,
}

/// Project settings for experience and levels (`project.ron`; the XP curve of 0.27 and older,
/// `base_xp`, `growth` and `hp_per_level`, is ignored: the SRD table replaced it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progression {
    /// Off: XP isn't counted, nothing is shown, characters and quests give none.
    pub enabled: bool,
    /// The highest level (at most 20; XP keeps counting there, but no level is gained).
    pub max_level: u32,
    /// How hit points grow per level.
    pub hit_points: HitPoints,
    /// Max mana gained per level (with magic).
    pub mana_per_level: u32,
}

impl Default for Progression {
    fn default() -> Self {
        Progression { enabled: false, max_level: 20, hit_points: HitPoints::Average, mana_per_level: 2 }
    }
}

/// The SRD's experience table: the XP a character needs for levels 1 to 20.
pub const XP_TABLE: [u64; 20] = [0, 300, 900, 2_700, 6_500, 14_000, 23_000, 34_000, 48_000, 64_000, 85_000, 100_000, 120_000, 140_000, 165_000, 195_000, 225_000, 265_000, 305_000, 355_000];

/// The hit die every hero has until classes arrive (a fighter's d10).
pub const HIT_DIE: i32 = 10;

impl Progression {
    fn top(&self) -> u32 {
        self.max_level.clamp(1, 20)
    }

    /// XP needed to go from `level` to `level + 1`.
    pub fn xp_to_next(&self, level: u32) -> u64 {
        let l = level.clamp(1, 19) as usize;
        XP_TABLE[l] - XP_TABLE[l - 1]
    }

    /// Total XP needed to reach `level` (level 1 = 0).
    pub fn xp_for_level(&self, level: u32) -> u64 {
        XP_TABLE[level.clamp(1, 20) as usize - 1]
    }

    /// The level a total of `xp` reaches (1 ..= max_level).
    pub fn level_for(&self, xp: u64) -> u32 {
        (1..=self.top()).rev().find(|l| xp >= self.xp_for_level(*l)).unwrap_or(1)
    }

    /// Progress inside the current level: (XP gained since reaching it, XP the next level
    /// needs). At the highest level both are the last step's XP (a full bar).
    pub fn progress(&self, level: u32, xp: u64) -> (u64, u64) {
        let level = level.max(1);
        if level >= self.top() {
            let n = self.xp_to_next(level.saturating_sub(1).max(1));
            return (n, n);
        }
        let need = self.xp_to_next(level);
        (xp.saturating_sub(self.xp_for_level(level)).min(need), need)
    }
}

/// The hero's max HP at `level` (SRD): the hit die's maximum plus the CON modifier at level 1,
/// then per level the HP that level gave (`rolls[k]` for level k + 2; missing = the die's
/// average) plus CON, at least 1 per level; plus `per_level` for every level (a hill dwarf's
/// toughness).
pub fn max_hp(level: u32, con_mod: i32, rolls: &[i32], per_level: i32) -> i32 {
    max_hp_die(HIT_DIE, level, con_mod, rolls, per_level)
}

/// [`max_hp`] with a class's hit die (`die` sides).
pub fn max_hp_die(die: i32, level: u32, con_mod: i32, rolls: &[i32], per_level: i32) -> i32 {
    let level = level.max(1);
    let avg = die / 2 + 1;
    let mut hp = (die + con_mod).max(1) + per_level;
    for k in 0..(level - 1) as usize {
        hp += (rolls.get(k).copied().unwrap_or(avg) + con_mod).max(1) + per_level;
    }
    hp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_srd_table() {
        let p = Progression { enabled: true, ..Default::default() };
        assert_eq!(p.xp_to_next(1), 300);
        assert_eq!(p.xp_to_next(2), 600);
        assert_eq!(p.xp_for_level(1), 0);
        assert_eq!(p.xp_for_level(5), 6_500);
        assert_eq!(p.xp_for_level(20), 355_000);
        for (k, xp) in XP_TABLE.iter().enumerate() {
            assert_eq!(p.level_for(*xp), k as u32 + 1, "{xp} XP");
            if *xp > 0 {
                assert_eq!(p.level_for(xp - 1), k as u32, "{} XP", xp - 1);
            }
        }
        assert_eq!(p.level_for(u64::MAX), 20);
        assert_eq!(p.progress(2, 400), (100, 600));
        let top = p.progress(20, u64::MAX);
        assert_eq!(top.0, top.1);
        let capped = Progression { max_level: 5, ..p.clone() };
        assert_eq!(capped.level_for(1_000_000), 5);
        let zero = Progression { max_level: 0, ..p };
        assert_eq!(zero.level_for(1000), 1);
    }

    #[test]
    fn hit_points_by_level() {
        // Level 1: 10 + CON; each level the average 6 + CON, or the roll.
        assert_eq!(max_hp(1, 2, &[], 0), 12);
        assert_eq!(max_hp(5, 2, &[], 0), 12 + 4 * 8);
        assert_eq!(max_hp(3, 1, &[2, 10], 0), 11 + 3 + 11);
        assert_eq!(max_hp(3, -3, &[1, 1], 0), 7 + 1 + 1, "at least 1 per level");
        // A hill dwarf: +1 per level.
        assert_eq!(max_hp(5, 1, &[], 1), 11 + 4 * 7 + 5);
    }

    #[test]
    fn hit_points_follow_the_class_hit_die() {
        // A wizard (d6): 6 + CON, then 4 + CON; a barbarian (d12): 12 + CON, then 7 + CON.
        assert_eq!(max_hp_die(6, 3, 1, &[], 0), 7 + 5 + 5);
        assert_eq!(max_hp_die(12, 2, 2, &[], 0), 14 + 9);
        // Rolls count from level 2; draconic resilience adds 1 per level.
        assert_eq!(max_hp_die(8, 3, 0, &[8, 1], 1), 9 + 9 + 2);
        assert_eq!(max_hp_die(10, 4, 1, &[], 0), max_hp(4, 1, &[], 0), "no class: the d10");
    }

    #[test]
    fn old_projects_have_it_off_and_their_curve_is_ignored() {
        let p: Progression = ron::from_str("()").unwrap();
        assert_eq!(p, Progression::default());
        assert!(!p.enabled);
        let old: Progression = ron::from_str("(enabled: true, base_xp: 60, growth: 1.4, max_level: 20, hp_per_level: 2, mana_per_level: 2)").unwrap();
        assert_eq!((old.enabled, old.hit_points, old.mana_per_level), (true, HitPoints::Average, 2));
    }
}
