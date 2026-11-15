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

/// The user's pictures folder: Linux's `XDG_PICTURES_DIR` (`user-dirs.dirs`, translated names such
/// as `~/Bilder` included), else `~/Pictures`; Windows `%USERPROFILE%\Pictures`; macOS
/// `~/Pictures`. None when there is no home.
pub fn pictures() -> Option<PathBuf> {
    let home = home()?;
    if cfg!(all(unix, not(target_os = "macos"))) {
        if let Some(dir) = var("XDG_PICTURES_DIR") {
            return Some(dir);
        }
        // (Inside a Flatpak the config folder is the app's own: the home's file is read too.)
        let read = |dir: Option<PathBuf>| dir.and_then(|c| std::fs::read_to_string(c.join("user-dirs.dirs")).ok());
        let dirs = read(config_base()).or_else(|| read(Some(home.join(".config")))).unwrap_or_default();
        if let Some(dir) = user_dir(&dirs, "XDG_PICTURES_DIR", &home) {
            return Some(dir);
        }
    }
    Some(home.join("Pictures"))
}

/// A folder of `user-dirs.dirs` (`XDG_PICTURES_DIR="$HOME/Bilder"`), `$HOME` expanded.
fn user_dir(file: &str, key: &str, home: &std::path::Path) -> Option<PathBuf> {
    let line = file.lines().map(str::trim).find(|l| l.starts_with(key) && l[key.len()..].trim_start().starts_with('='))?;
    let value = line.split_once('=')?.1.trim().trim_matches('"');
    let path = match value.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(value),
    };
    // (A folder set to the home itself means "none".)
    (path != home && path.is_absolute()).then_some(path)
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
