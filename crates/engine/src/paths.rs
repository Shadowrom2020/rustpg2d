//! Per-user folders on each platform (no crate: a few environment variables).
//!
//! | | Linux (XDG) | Windows | macOS |
//! |---|---|---|---|
//! | [`config_base`] | `$XDG_CONFIG_HOME`, else `~/.config` | `%APPDATA%` | `~/Library/Application Support` |
//! | [`data_base`] | `$XDG_DATA_HOME`, else `~/.local/share` | `%LOCALAPPDATA%` | `~/Library/Application Support` |
//! | [`saves_base`] | like `data_base` | `%APPDATA%` (save games roam with the profile) | `~/Library/Application Support` |
//! | [`cache_base`] | `$XDG_CACHE_HOME`, else `~/.cache` | `%LOCALAPPDATA%` | `~/Library/Caches` |
//! | [`home`] | `$HOME` | `%USERPROFILE%` | `$HOME` |
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
    if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Application Support"))
    } else if cfg!(windows) {
        var("APPDATA").or_else(|| home().map(|h| h.join("AppData").join("Roaming")))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| home().map(|h| h.join(".config")))
    }
}

/// Larger per-user data (the asset library).
pub fn data_base() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        config_base()
    } else if cfg!(windows) {
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
    if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library").join("Caches"))
    } else if cfg!(windows) {
        data_base()
    } else {
        var("XDG_CACHE_HOME").or_else(|| home().map(|h| h.join(".cache")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_follow_the_platform_conventions() {
        // (Only reads the environment; the variables are this process's own.)
        if cfg!(windows) || cfg!(target_os = "macos") {
            assert!(config_base().is_some() && data_base().is_some());
        } else if std::env::var_os("XDG_CONFIG_HOME").is_none() {
            if let Some(h) = home() {
                assert_eq!(config_base(), Some(h.join(".config")));
            }
        }
    }
}
