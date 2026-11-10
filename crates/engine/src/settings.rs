//! Player options (graphics, sound, text): chosen in the game's Options menu and kept between
//! sessions per game in `$XDG_CONFIG_HOME/rustpg2d/<game>/settings.ron` (`~/.config/...`;
//! `$RUSTPG2D_SETTINGS_DIR` overrides the folder).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub fullscreen: bool,
    /// Zoom on top of the project's pixel scale (1 = as designed).
    pub zoom: f32,
    pub show_fps: bool,
    /// Volumes 0..1.
    pub master_volume: f32,
    pub music_volume: f32,
    pub effects_volume: f32,
    /// Environmental sounds: rain and other weather.
    pub ambience_volume: f32,
    /// Dialog text speed multiplier (0 = instant).
    pub text_speed: f32,
    /// Screen shake and the red flash when hurt (off for players who find them unpleasant).
    pub screen_effects: bool,
    /// Keys the player changed (Options › Controls): action id -> key names; other actions keep
    /// their default keys.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub keys: std::collections::BTreeMap<String, Vec<String>>,
    /// Gamepad buttons the player changed: action id -> button names (`a`, `lb`, …).
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub pad: std::collections::BTreeMap<String, Vec<String>>,
    /// How gamepad buttons are shown: `auto` (from the pad), `xbox`, `playstation`, `nintendo`.
    #[serde(skip_serializing_if = "is_auto")]
    pub pad_style: String,
    /// The language the player chose (code; None = the system's language if the game has it,
    /// else the game's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { fullscreen: false, zoom: 1.0, show_fps: false, master_volume: 1.0, music_volume: 0.8, effects_volume: 1.0, ambience_volume: 0.7, text_speed: 1.0, screen_effects: true, keys: Default::default(), pad: Default::default(), pad_style: "auto".into(), language: None }
    }
}

fn is_auto(s: &String) -> bool {
    s.is_empty() || s == "auto"
}

/// Zoom levels the Options menu cycles through.
pub const ZOOMS: [f32; 6] = [0.75, 1.0, 1.25, 1.5, 2.0, 2.5];
/// Text speeds: slow, normal, fast, instant.
pub const TEXT_SPEEDS: [(f32, &str); 4] = [(0.5, "Slow"), (1.0, "Normal"), (2.0, "Fast"), (0.0, "Instant")];

impl Settings {
    /// Folder of a game's settings (by the game's name).
    pub fn dir(game_name: &str) -> Option<PathBuf> {
        // The web game: kept in the browser's storage (`sys`).
        if cfg!(target_arch = "wasm32") {
            return Some(PathBuf::from("settings").join(crate::release::slug(game_name)));
        }
        if let Some(d) = std::env::var_os("RUSTPG2D_SETTINGS_DIR") {
            return Some(PathBuf::from(d).join(crate::release::slug(game_name)));
        }
        let base = crate::paths::config_base()?;
        let slug = crate::release::slug(game_name);
        let dir = base.join("rustpg2d").join(&slug);
        crate::legacy::adopt(&dir, &base.join(crate::legacy::OLD_NAME).join(&slug));
        Some(dir)
    }

    /// The saved settings of a game (defaults if there are none or they can't be read).
    pub fn load(game_name: &str) -> Settings {
        let Some(path) = Self::dir(game_name).map(|d| d.join("settings.ron")) else { return Settings::default() };
        match crate::sys::read_to_string(&path) {
            Ok(s) => ron::from_str(&s).unwrap_or_else(|e| {
                eprintln!("[settings] {}: {e}; using defaults", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, game_name: &str) -> anyhow::Result<PathBuf> {
        let dir = Self::dir(game_name).ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let path = dir.join("settings.ron");
        crate::project::write_ron(&path, self)?;
        Ok(path)
    }

    /// Effective volumes of music and effects (master applied).
    pub fn music(&self) -> f32 {
        (self.master_volume * self.music_volume).clamp(0.0, 1.0)
    }
    pub fn effects(&self) -> f32 {
        (self.master_volume * self.effects_volume).clamp(0.0, 1.0)
    }
    pub fn ambience(&self) -> f32 {
        (self.master_volume * self.ambience_volume).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("rustpg2d_settings_{}", std::process::id()));
        std::env::set_var("RUSTPG2D_SETTINGS_DIR", &dir);
        assert_eq!(Settings::load("My Game"), Settings::default());
        let s = Settings { fullscreen: true, zoom: 2.0, music_volume: 0.3, ..Default::default() };
        let path = s.save("My Game").unwrap();
        assert!(path.ends_with("my-game/settings.ron"));
        assert_eq!(Settings::load("My Game"), s);
        // Partial / older files fill in defaults.
        std::fs::write(&path, "(zoom: 1.5)").unwrap();
        assert_eq!(Settings::load("My Game"), Settings { zoom: 1.5, ..Default::default() });
        assert!((s.music() - 0.3).abs() < 1e-6);
        // Files from before the ambience volume get its default.
        std::fs::write(&path, "(effects_volume: 0.5)").unwrap();
        let old = Settings::load("My Game");
        assert_eq!((old.effects_volume, old.ambience_volume), (0.5, 0.7));
        assert!((Settings { master_volume: 0.5, ..old }.ambience() - 0.35).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(dir);
    }
}
