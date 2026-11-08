//! Per-user folders on each platform (no crate: a few environment variables).
//!
//! | | Linux (XDG) | Windows |
//! |---|---|---|
//! | [`config_base`] | `$XDG_CONFIG_HOME`, else `~/.config` | `%APPDATA%` |
//! | [`data_base`] | `$XDG_DATA_HOME`, else `~/.local/share` | `%LOCALAPPDATA%` |
//! | [`saves_base`] | like `data_base` | `%APPDATA%` (save games roam with the profile) |
//! | [`cache_base`] | `$XDG_CACHE_HOME`, else `~/.cache` | `%LOCALAPPDATA%` |
//! | [`home`] | `$HOME` | `%USERPROFILE%` |
//!
//! Callers append their own folder (`rustpg2d/…`, a game's slug).

use std::path::PathBuf;

fn var(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The user's home folder.
pub fn home() -> Option<PathBuf> {
    if cfg!(windows) {
        var("USERPROFILE").or_else(|| var("HOME"))
    } else {
        var("HOME")
    }
}

/// Settings and small per-user files (the editor's recent list, games' settings).
pub fn config_base() -> Option<PathBuf> {
    if cfg!(windows) {
        var("APPDATA").or_else(|| home().map(|h| h.join("AppData").join("Roaming")))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| home().map(|h| h.join(".config")))
    }
}

/// Larger per-user data (the asset library).
pub fn data_base() -> Option<PathBuf> {
    if cfg!(windows) {
        var("LOCALAPPDATA").or_else(|| home().map(|h| h.join("AppData").join("Local")))
    } else {
        var("XDG_DATA_HOME").or_else(|| home().map(|h| h.join(".local").join("share")))
    }
}

/// Where released games keep their save games.
pub fn saves_base() -> Option<PathBuf> {
    if cfg!(windows) {
        config_base()
    } else {
        data_base()
    }
}

/// Downloads that can be fetched again (the AppImage runtime).
pub fn cache_base() -> Option<PathBuf> {
    if cfg!(windows) {
        data_base()
    } else {
        var("XDG_CACHE_HOME").or_else(|| home().map(|h| h.join(".cache")))
    }
}

/// How a path under one of these folders is shown to players (README, export dialog).
pub fn saves_hint(slug: &str, windows: bool) -> String {
    if windows {
        format!("%APPDATA%\\{slug}\\saves")
    } else {
        format!("~/.local/share/{slug}/saves")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_follow_the_platform_conventions() {
        // (Only reads the environment; the variables are this process's own.)
        if cfg!(windows) {
            assert!(config_base().is_some() && data_base().is_some());
        } else if std::env::var_os("XDG_CONFIG_HOME").is_none() {
            if let Some(h) = home() {
                assert_eq!(config_base(), Some(h.join(".config")));
            }
        }
        assert_eq!(saves_hint("my-game", true), "%APPDATA%\\my-game\\saves");
        assert_eq!(saves_hint("my-game", false), "~/.local/share/my-game/saves");
    }
}
