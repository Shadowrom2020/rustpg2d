//! Experience and levels (optional per project, `ProjectConfig::progression`): defeated
//! characters and completed quests give XP, and every level raises the player's max HP and mana
//! (and the SRD proficiency bonus). The player's XP and level are in the save game (`WorldState::xp`,
//! `WorldState::level`); the runtime part is `runtime/progression.rs`.

use serde::{Deserialize, Serialize};

/// Project settings for experience and levels (`project.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progression {
    /// Off: XP isn't counted, nothing is shown, characters and quests give none.
    pub enabled: bool,
    /// XP needed to go from level 1 to level 2.
    pub base_xp: u32,
    /// Each further level needs this many times the XP of the one before.
    pub growth: f32,
    /// The highest level (XP keeps counting there, but no level is gained).
    pub max_level: u32,
    /// Max HP gained per level.
    pub hp_per_level: i32,
    /// Max mana gained per level (with magic).
    pub mana_per_level: u32,
}

impl Default for Progression {
    fn default() -> Self {
        Progression { enabled: false, base_xp: 100, growth: 1.5, max_level: 20, hp_per_level: 2, mana_per_level: 2 }
    }
}

impl Progression {
    /// XP needed to go from `level` to `level + 1`: `base_xp · growth^(level-1)`, at least 1.
    pub fn xp_to_next(&self, level: u32) -> u64 {
        let n = self.base_xp as f64 * (self.growth.max(1.0) as f64).powi(level.max(1) as i32 - 1);
        (n.round() as u64).max(1)
    }

    /// Total XP needed to reach `level` (level 1 = 0).
    pub fn xp_for_level(&self, level: u32) -> u64 {
        (1..level.max(1)).map(|l| self.xp_to_next(l)).fold(0u64, |a, b| a.saturating_add(b))
    }

    /// The level a total of `xp` reaches (1 ..= max_level).
    pub fn level_for(&self, xp: u64) -> u32 {
        let mut level = 1;
        let mut need = 0u64;
        while level < self.max_level.max(1) {
            need = need.saturating_add(self.xp_to_next(level));
            if xp < need {
                break;
            }
            level += 1;
        }
        level
    }

    /// Progress inside the current level: (XP gained since reaching it, XP the next level
    /// needs). At the highest level both are the last step's XP (a full bar).
    pub fn progress(&self, level: u32, xp: u64) -> (u64, u64) {
        let level = level.max(1);
        if level >= self.max_level.max(1) {
            let n = self.xp_to_next(level.saturating_sub(1).max(1));
            return (n, n);
        }
        let need = self.xp_to_next(level);
        (xp.saturating_sub(self.xp_for_level(level)).min(need), need)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        let p = Progression::default();
        assert_eq!(p.xp_to_next(1), 100);
        assert_eq!(p.xp_to_next(2), 150);
        assert_eq!(p.xp_to_next(3), 225);
        assert_eq!(p.xp_for_level(1), 0);
        assert_eq!(p.xp_for_level(3), 250);
        assert_eq!(p.level_for(0), 1);
        assert_eq!(p.level_for(99), 1);
        assert_eq!(p.level_for(100), 2);
        assert_eq!(p.level_for(249), 2);
        assert_eq!(p.level_for(250), 3);
        assert_eq!(p.level_for(u64::MAX), 20);
        assert_eq!(p.progress(2, 130), (30, 150));
        let top = p.progress(20, u64::MAX);
        assert_eq!(top.0, top.1);
        // Degenerate settings stay sane.
        let flat = Progression { base_xp: 0, growth: 0.0, max_level: 0, ..p };
        assert_eq!(flat.xp_to_next(5), 1);
        assert_eq!(flat.level_for(1000), 1);
    }

    #[test]
    fn old_projects_have_it_off() {
        let p: Progression = ron::from_str("()").unwrap();
        assert_eq!(p, Progression::default());
        assert!(!p.enabled);
    }
}
