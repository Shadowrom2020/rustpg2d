//! Standalone release bundles of a project.
//!
//! Layout of a bundle folder (Linux; a Windows bundle is `<slug>-windows/` with `<slug>.exe`):
//! ```text
//! <slug>/
//!   <slug>        the game executable (a copy of rustpg2d-game)
//!   data/         project.ron, defs/, maps/, scripts/ and the images / sounds in use
//!   README.txt
//!   rustpg2d-LICENSE.txt   the engine's license (MIT): the game executable is R(ust)PG2D's game
//!                        runner; the game itself is licensed as its creator decides
//! ```
//! Started without arguments, the game binary looks for `data/` next to itself (see
//! [`bundled_root`]) and keeps save games in the user's data folder (see [`user_saves_dir`]).

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::project::Project;

/// Folder next to the executable that holds the project in a bundle.
pub const DATA_DIR: &str = "data";

/// R(ust)PG2D's license (the repository's LICENSE file), shipped with every exported game for the
/// engine inside it.
pub const ENGINE_LICENSE: &str = include_str!("../../../LICENSE");
/// Its file name in exports and game packages.
pub const ENGINE_LICENSE_FILE: &str = "rustpg2d-LICENSE.txt";

/// The platform an exported game runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// x86_64 Linux (the `rustpg2d-game` executable).
    Linux,
    /// x86_64 Windows (`rustpg2d-game.exe`).
    Windows,
}

impl Platform {
    pub const ALL: [Platform; 2] = [Platform::Linux, Platform::Windows];

    /// The platform this program runs on (Linux for anything that isn't Windows).
    pub fn host() -> Platform {
        if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Linux => "Linux",
            Platform::Windows => "Windows",
        }
    }

    /// File name of R(ust)PG2D's game runner for this platform.
    pub fn game_runner(self) -> &'static str {
        match self {
            Platform::Linux => "rustpg2d-game",
            Platform::Windows => "rustpg2d-game.exe",
        }
    }

    /// The bundle folder of a game (`<slug>`, `<slug>-windows`).
    pub fn folder(self, slug: &str) -> String {
        match self {
            Platform::Linux => slug.to_string(),
            Platform::Windows => format!("{slug}-windows"),
        }
    }

    /// The game executable's file name in the bundle.
    pub fn exe(self, slug: &str) -> String {
        match self {
            Platform::Linux => slug.to_string(),
            Platform::Windows => format!("{slug}.exe"),
        }
    }

    /// The zip of a bundle.
    pub fn archive(self, slug: &str) -> String {
        format!("{slug}-{}.zip", self.label().to_lowercase())
    }

    /// The README of a bundle.
    fn readme(self, name: &str, slug: &str) -> String {
        let how = match self {
            Platform::Linux => format!("Run ./{slug} to play (Linux, x86_64).\nThe game data is in data/; save games are kept in {}.", crate::paths::saves_hint(slug, false)),
            Platform::Windows => format!(
                "Double-click {slug}.exe to play (Windows 10 or newer, 64-bit).\nThe game data is in the data folder; save games are kept in {}.\nWindows may warn that the publisher is unknown (the game isn't signed): choose \"More info\", then \"Run anyway\".",
                crate::paths::saves_hint(slug, true)
            ),
        };
        let line_end = if self == Platform::Windows { "\r\n" } else { "\n" };
        format!("{name}\n\n{how}\n\nMade with R(ust)PG2D. The game executable is R(ust)PG2D's game runner, licensed under the MIT License: see {ENGINE_LICENSE_FILE}.\n").replace('\n', line_end)
    }
}

/// A file-name friendly version of a game name ("My Game!" → "my-game").
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "game".into() } else { s }
}

/// The project bundled next to the running executable, if any.
pub fn bundled_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let data = exe.parent()?.join(DATA_DIR);
    data.join("project.ron").exists().then_some(data)
}

/// Where a released game keeps its save games: `$XDG_DATA_HOME/<slug>/saves` (default
/// `~/.local/share/<slug>/saves`), on Windows `%APPDATA%\<slug>\saves`.
pub fn user_saves_dir(game_name: &str) -> Option<PathBuf> {
    Some(crate::paths::saves_base()?.join(slug(game_name)).join("saves"))
}

/// Problems found before exporting.
#[derive(Debug, Default)]
pub struct Check {
    /// Prevent the export.
    pub errors: Vec<String>,
    /// Worth knowing, but the game runs.
    pub warnings: Vec<String>,
}

/// Files of the project the game needs (relative paths), plus problems found on the way.
pub fn check(project: &Project) -> (Vec<String>, Check) {
    let mut c = Check::default();
    let root = &project.root;
    let cfg = &project.config;
    let maps = project.list_maps();
    if maps.is_empty() {
        c.errors.push("The project has no maps.".into());
    } else if !maps.contains(&cfg.start_map) {
        c.errors.push(format!("The start map '{}' does not exist (Database → Project).", cfg.start_map));
    } else if let Ok(m) = project.load_map(&cfg.start_map) {
        if !m.zones.iter().any(|z| z.name == cfg.start_spawn) {
            c.warnings.push(format!("The start map has no spawn point '{}'; the player starts in the map's center.", cfg.start_spawn));
        }
    }
    if cfg.tile_size != crate::project::TILE_SIZE {
        c.errors.push(format!("The project uses {}px tiles; open it in the editor to convert it to 32px.", cfg.tile_size));
    }
    if cfg.player.is_empty() || project.db.character(&cfg.player).is_none() {
        c.warnings.push("No player character is set (Database → Project); a placeholder will be used.".into());
    }
    let mut files: BTreeSet<String> = BTreeSet::new();
    files.insert("project.ron".into());
    for d in ["tilesets", "sheets", "items", "props", "characters", "sounds", "quests", "skills", "spells", "recipes", "achievements"] {
        let rel = format!("defs/{d}.ron");
        if root.join(&rel).exists() {
            files.insert(rel);
        }
    }
    for m in &maps {
        files.insert(format!("maps/{m}.map"));
    }
    if let Ok(rd) = std::fs::read_dir(project.scripts_dir()) {
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "rhai") {
                files.insert(format!("scripts/{}", e.file_name().to_string_lossy()));
            }
        }
    }
    let db = &project.db;
    let assets = db.tilesets.iter().map(|d| (&d.image, "tileset", &d.id)).chain(db.sheets.iter().map(|d| (&d.image, "sprite sheet", &d.id))).chain(db.sounds.iter().map(|d| (&d.file, "sound", &d.id)));
    for (rel, kind, id) in assets {
        if rel.is_empty() || !root.join(rel).exists() {
            c.warnings.push(format!("The {kind} '{id}' has no file ({rel}); it will be missing in the game."));
        } else if rel.starts_with('/') || rel.split('/').any(|p| p == "..") {
            c.errors.push(format!("The {kind} '{id}' uses a file outside the project ({rel}); import it into the project first."));
        } else {
            files.insert(rel.clone());
        }
    }
    (files.into_iter().collect(), c)
}

/// What an export produced.
#[derive(Debug)]
pub struct Exported {
    pub folder: PathBuf,
    pub archive: Option<PathBuf>,
    pub files: usize,
    pub bytes: u64,
    pub warnings: Vec<String>,
}

/// Writes a bundle of `project` named `name` for `platform` into `out_dir/<slug>/` (Windows:
/// `<slug>-windows/`), using `game_binary` (R(ust)PG2D's game runner for that platform) as the
/// executable; with `zip`, also `out_dir/<slug>-linux.zip` / `<slug>-windows.zip`. An earlier
/// export in the same place is replaced; any other existing folder of that name is left alone
/// (error). `mesa` (Windows): a folder with Mesa's software OpenGL copied into the bundle as
/// `mesa/`, so the game also starts on PCs without an OpenGL 3 driver (`gl_fallback`).
pub fn export(project: &Project, name: &str, platform: Platform, game_binary: &Path, out_dir: &Path, zip: bool, mesa: Option<&Path>) -> Result<Exported> {
    let (files, check) = check(project);
    if !check.errors.is_empty() {
        bail!("{}", check.errors.join("\n"));
    }
    if !game_binary.is_file() {
        bail!("game executable not found: {}", game_binary.display());
    }
    let slug = slug(name);
    if platform == Platform::Windows && std::fs::read(game_binary).map_or(true, |b| !b.starts_with(b"MZ")) {
        bail!("{} is not a Windows program", game_binary.display());
    }
    let folder = out_dir.join(platform.folder(&slug));
    let project_root = project.root.canonicalize().unwrap_or_else(|_| project.root.clone());
    let folder_abs = std::fs::create_dir_all(out_dir).ok().and_then(|_| out_dir.canonicalize().ok()).map(|d| d.join(&slug));
    if folder_abs.as_ref().is_some_and(|f| project_root.starts_with(f) || f.starts_with(&project_root)) {
        bail!("choose an output folder outside the project");
    }
    if folder.exists() {
        if !folder.join(DATA_DIR).join("project.ron").exists() {
            bail!("{} already exists and is not an earlier export; choose another folder or name", folder.display());
        }
        std::fs::remove_dir_all(&folder).with_context(|| format!("removing the earlier export {}", folder.display()))?;
    }
    let data = folder.join(DATA_DIR);
    let mut bytes = 0u64;
    let mut extra = 0usize;
    for rel in &files {
        let (src, dst) = (project.root.join(rel), data.join(rel));
        std::fs::create_dir_all(dst.parent().unwrap())?;
        std::fs::copy(&src, &dst).with_context(|| format!("copying {rel}"))?;
        // (Sizes from the files: `copy`'s return value is 0 under Wine.)
        bytes += std::fs::metadata(&dst).map_or(0, |m| m.len());
    }
    // The bundled project carries the chosen name (window title, save folder).
    let mut cfg = project.config.clone();
    cfg.name = name.trim().to_string();
    crate::project::write_ron(&data.join("project.ron"), &cfg)?;
    let exe = folder.join(platform.exe(&slug));
    std::fs::copy(game_binary, &exe).context("copying the game executable")?;
    bytes += std::fs::metadata(&exe).map_or(0, |m| m.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))?;
    }
    if let (Platform::Windows, Some(dir)) = (platform, mesa) {
        let dst = folder.join(crate::gl_fallback::MESA_DIR);
        std::fs::create_dir_all(&dst)?;
        for e in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?.flatten() {
            if e.path().is_file() {
                std::fs::copy(e.path(), dst.join(e.file_name())).with_context(|| format!("copying {}", e.path().display()))?;
                bytes += std::fs::metadata(dst.join(e.file_name())).map_or(0, |m| m.len());
                extra += 1;
            }
        }
    }
    std::fs::write(folder.join("README.txt"), platform.readme(name.trim(), &slug))?;
    std::fs::write(folder.join(ENGINE_LICENSE_FILE), ENGINE_LICENSE)?;
    let archive = if zip { Some(write_zip(&folder, &slug, &platform.exe(&slug), &out_dir.join(platform.archive(&slug)))?) } else { None };
    Ok(Exported { folder, archive, files: files.len() + 3 + extra, bytes, warnings: check.warnings })
}

/// Zips the bundle folder as `<slug>/…` (keeping the executable's permissions).
fn write_zip(folder: &Path, slug: &str, exe_name: &str, path: &Path) -> Result<PathBuf> {
    let path = path.to_path_buf();
    let file = std::fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    let mut zw = zip::ZipWriter::new(file);
    let mut stack = vec![folder.to_path_buf()];
    let mut entries = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                entries.push(p);
            }
        }
    }
    entries.sort();
    for p in entries {
        let rel = p.strip_prefix(folder)?.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        let exe = rel == exe_name;
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            // Unix attributes also when zipped on Windows: Linux players need the executable bit.
            .system(zip::System::Unix)
            .unix_permissions(if exe { 0o755 } else { 0o644 })
            .large_file(std::fs::metadata(&p)?.len() >= u32::MAX as u64);
        zw.start_file(format!("{slug}/{rel}"), opts)?;
        zw.write_all(&std::fs::read(&p)?)?;
    }
    zw.finish()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{Database, SheetDef};
    use crate::map::{Map, Zone, ZoneKind};
    use crate::project::ProjectConfig;

    #[test]
    fn slugs() {
        assert_eq!(slug("My Game!"), "my-game");
        assert_eq!(slug("  Ünïcode -- Quest 2 "), "n-code-quest-2");
        assert_eq!(slug("!!!"), "game");
    }

    #[test]
    fn export_bundle() {
        let tmp = std::env::temp_dir().join(format!("rustpg2d_release_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let root = tmp.join("proj");
        std::fs::create_dir_all(root.join("sprites")).unwrap();
        std::fs::write(root.join("sprites/a.png"), b"png").unwrap();
        std::fs::create_dir_all(root.join("saves")).unwrap();
        std::fs::write(root.join("saves/quick.ron"), b"()").unwrap();
        std::fs::write(root.join("sprites/unused.png"), b"x").unwrap();
        let db = Database { sheets: vec![SheetDef { id: "a".into(), image: "sprites/a.png".into(), frame_w: 32, frame_h: 32, kind: Default::default(), female: false, playable: false, tags: vec![] }], ..Default::default() };
        let mut p = Project { root: root.clone(), config: ProjectConfig::new("Test"), db };
        p.save_config().unwrap();
        p.save_defs().unwrap();
        let (_, c) = check(&p);
        assert!(!c.errors.is_empty(), "no maps yet");
        let mut map = Map::new("start", 8, 8);
        map.zones.push(Zone { uid: 1, name: "start".into(), x: 1, y: 1, w: 1, h: 1, kind: ZoneKind::Spawn, triggers: vec![], loot: None });
        p.save_map(&map).unwrap();
        p.config.player = "nobody".into();
        let bin = tmp.join("rustpg2d-game");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let out = tmp.join("out");
        let r = export(&p, "Test Quest", Platform::Linux, &bin, &out, true, None).unwrap();
        let f = out.join("test-quest");
        assert!(f.join("test-quest").is_file());
        assert!(f.join("data/project.ron").is_file());
        assert!(f.join("data/maps/start.map").is_file());
        assert!(f.join("data/sprites/a.png").is_file());
        let lic = std::fs::read_to_string(f.join(ENGINE_LICENSE_FILE)).unwrap();
        assert!(lic.starts_with("MIT License") && lic.contains("Jürgen Herrmann"), "the engine's license ships with the game");
        assert!(!f.join("data/sprites/unused.png").exists(), "only used files");
        assert!(!f.join("data/saves").exists(), "never ship saves");
        assert_eq!(Project::load(&f.join("data")).unwrap().config.name, "Test Quest");
        assert!(r.warnings.iter().any(|w| w.contains("player")));
        let zip_path = r.archive.unwrap();
        let mut za = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        assert!(za.by_name("test-quest/rustpg2d-LICENSE.txt").is_ok());
        let exe = za.by_name("test-quest/test-quest").unwrap();
        assert_eq!(exe.unix_mode().map(|m| m & 0o777), Some(0o755));
        drop(exe);
        // Exporting again replaces the earlier export; a foreign folder is refused.
        export(&p, "Test Quest", Platform::Linux, &bin, &out, false, None).unwrap();
        std::fs::create_dir_all(out.join("other")).unwrap();
        assert!(export(&p, "Other", Platform::Linux, &bin, &out, false, None).is_err());
        // Never into the project itself.
        assert!(export(&p, "Test Quest", Platform::Linux, &bin, &root, false, None).is_err());
        // Windows: needs a Windows program; its own folder, exe, zip and README next to Linux's.
        assert!(export(&p, "Test Quest", Platform::Windows, &bin, &out, false, None).is_err(), "not an exe");
        let win = tmp.join("rustpg2d-game.exe");
        std::fs::write(&win, b"MZ\x90\0fake").unwrap();
        let mesa = tmp.join("mesa");
        std::fs::create_dir_all(&mesa).unwrap();
        std::fs::write(mesa.join("opengl32.dll"), b"MZ dll").unwrap();
        let r = export(&p, "Test Quest", Platform::Windows, &win, &out, true, Some(&mesa)).unwrap();
        let w = out.join("test-quest-windows");
        assert_eq!(r.folder, w);
        assert!(w.join("test-quest.exe").is_file() && w.join("data/maps/start.map").is_file());
        assert!(w.join("mesa/opengl32.dll").is_file(), "the software OpenGL fallback");
        let readme = std::fs::read_to_string(w.join("README.txt")).unwrap();
        assert!(readme.contains("test-quest.exe") && readme.contains("%APPDATA%\\test-quest\\saves") && readme.contains("\r\n"), "{readme}");
        assert!(f.join("test-quest").is_file(), "the Linux export stays");
        let mut za = zip::ZipArchive::new(std::fs::File::open(out.join("test-quest-windows.zip")).unwrap()).unwrap();
        assert!(za.by_name("test-quest/test-quest.exe").is_ok() && za.by_name("test-quest/data/project.ron").is_ok());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
