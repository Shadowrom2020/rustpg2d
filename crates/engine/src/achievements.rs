//! Achievements: definitions (`defs/achievements.ron`) with conditions the game checks while
//! playing, and the player's unlocked ones. Unlocks belong to the player, not to a save game:
//! they are kept in `achievements.ron` next to the game's `settings.ron`
//! (`Settings::dir`; `RUSTPG2D_SETTINGS_DIR` moves both).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::defs::SpriteRef;
use crate::events::Condition;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AchievementDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: SpriteRef,
    /// Shown as "???" until unlocked (secrets).
    #[serde(default)]
    pub hidden: bool,
    /// Unlocked as soon as all of them hold (checked twice a second while playing). Empty =
    /// only the *Unlock achievement* action / `unlock_achievement(id)` unlocks it.
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

impl AchievementDef {
    pub fn new(id: &str, name: &str, description: &str, icon: SpriteRef, conditions: Vec<Condition>) -> Self {
        AchievementDef { id: id.into(), name: name.into(), description: description.into(), icon, hidden: false, conditions }
    }
}

/// The player's unlocked achievements: id → when (seconds since the epoch).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    pub unlocked: BTreeMap<String, u64>,
}

impl Progress {
    fn path(game_name: &str) -> Option<PathBuf> {
        crate::settings::Settings::dir(game_name).map(|d| d.join("achievements.ron"))
    }

    /// The player's achievements of a game (none if there is no file or it can't be read).
    pub fn load(game_name: &str) -> Progress {
        Self::path(game_name).map_or_else(Progress::default, |p| Self::load_from(&p))
    }

    fn load_from(path: &std::path::Path) -> Progress {
        match crate::sys::read_to_string(path) {
            Ok(s) => ron::from_str(&s).unwrap_or_else(|e| {
                eprintln!("[achievements] {}: {e}; starting empty", path.display());
                Progress::default()
            }),
            Err(_) => Progress::default(),
        }
    }

    pub fn save(&self, game_name: &str) -> anyhow::Result<PathBuf> {
        let path = Self::path(game_name).ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        crate::project::write_ron(&path, self)?;
        Ok(path)
    }

    /// Forgets every unlock (the menu's *Reset*): removes the file.
    pub fn reset(&mut self, game_name: &str) {
        self.unlocked.clear();
        if let Some(p) = Self::path(game_name) {
            let _ = crate::sys::remove_file(&p);
        }
    }

    pub fn has(&self, id: &str) -> bool {
        self.unlocked.contains_key(id)
    }

    /// Marks `id` unlocked at `when`; false if it already was.
    pub fn unlock(&mut self, id: &str, when: u64) -> bool {
        if self.has(id) {
            return false;
        }
        self.unlocked.insert(id.to_string(), when);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_file_round_trip() {
        // (Not through RUSTPG2D_SETTINGS_DIR: the settings test sets it, tests run in parallel.)
        let path = std::env::temp_dir().join(format!("rustpg2d_ach_{}/achievements.ron", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert_eq!(Progress::load_from(&path), Progress::default());
        let mut p = Progress::default();
        assert!(p.unlock("first_blood", 100));
        assert!(!p.unlock("first_blood", 200), "only once");
        crate::project::write_ron(&path, &p).unwrap();
        let back = Progress::load_from(&path);
        assert_eq!(back.unlocked.get("first_blood"), Some(&100));
        let d: AchievementDef = ron::from_str("(id: \"a\", name: \"A\")").unwrap();
        assert!(!d.hidden && d.conditions.is_empty());
    }
}
