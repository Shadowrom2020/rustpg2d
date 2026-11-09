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
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::bail;
use anyhow::{Context, Result};

use crate::project::Project;

/// Folder next to the executable that holds the project in a bundle.
pub const DATA_DIR: &str = "data";

/// R(ust)PG2D's license (the repository's LICENSE file), shipped with every exported game for the
/// engine inside it.
pub const ENGINE_LICENSE: &str = include_str!("../../../LICENSE");
/// Its file name in exports and game packages.
pub const ENGINE_LICENSE_FILE: &str = "rustpg2d-LICENSE.txt";

/// Start of a web game's `data.pack`.
const WEB_PACK_MAGIC: &[u8; 8] = b"R2DWEB1\0";

/// A web game's `data.pack`: every file of the game in one download (relative paths and
/// contents, postcard after a magic header). The files are mostly compressed already (PNG, Ogg,
/// maps); web servers compress the rest on the way.
pub fn web_pack(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut out = WEB_PACK_MAGIC.to_vec();
    out.extend(postcard::to_stdvec(files).context("packing the game files")?);
    Ok(out)
}

/// The files of a `data.pack` (see [`web_pack`]).
pub fn read_web_pack(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let body = bytes.strip_prefix(WEB_PACK_MAGIC.as_slice()).context("not an R(ust)PG2D data.pack (or from another version)")?;
    postcard::from_bytes(body).context("reading data.pack")
}

/// The platform an exported game runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// x86_64 Linux (the `rustpg2d-game` executable).
    Linux,
    /// x86_64 Windows (`rustpg2d-game.exe`).
    Windows,
    /// macOS 11 or newer, Apple Silicon and Intel (`rustpg2d-game-macos`, a universal binary):
    /// the game is an application bundle `<Name>.app`.
    MacOS,
}

impl Platform {
    pub const ALL: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::MacOS];

    /// The platform this program runs on (Linux for anything that isn't Windows or macOS).
    pub fn host() -> Platform {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Linux => "Linux",
            Platform::Windows => "Windows",
            Platform::MacOS => "macOS",
        }
    }

    /// File name of R(ust)PG2D's game runner for this platform next to the editor: the host's own
    /// is `rustpg2d-game` (`.exe` on Windows; the editor's play-tests run it), the others are
    /// shipped as `rustpg2d-game-linux`, `rustpg2d-game.exe`, `rustpg2d-game-macos` (universal).
    pub fn game_runner(self) -> &'static str {
        match (self, self == Platform::host()) {
            (Platform::Windows, _) => "rustpg2d-game.exe",
            (_, true) => "rustpg2d-game",
            (Platform::Linux, false) => "rustpg2d-game-linux",
            (Platform::MacOS, false) => "rustpg2d-game-macos",
        }
    }

    /// The bundle folder of a game (`<slug>`, `<slug>-windows`, `<slug>-macos`).
    pub fn folder(self, slug: &str) -> String {
        match self {
            Platform::Linux => slug.to_string(),
            Platform::Windows => format!("{slug}-windows"),
            Platform::MacOS => format!("{slug}-macos"),
        }
    }

    /// The game executable in the bundle folder (`/`-separated).
    pub fn exe(self, name: &str, slug: &str) -> String {
        match self {
            Platform::Linux => slug.to_string(),
            Platform::Windows => format!("{slug}.exe"),
            Platform::MacOS => format!("{}/Contents/MacOS/{slug}", app_bundle(name, slug)),
        }
    }

    /// The game data in the bundle folder (`/`-separated); `bundled_root` finds it from the executable.
    pub fn data(self, name: &str, slug: &str) -> String {
        match self {
            Platform::Linux | Platform::Windows => DATA_DIR.to_string(),
            Platform::MacOS => format!("{}/Contents/Resources/{DATA_DIR}", app_bundle(name, slug)),
        }
    }

    /// The zip of a bundle.
    pub fn archive(self, slug: &str) -> String {
        format!("{slug}-{}.zip", self.label().to_lowercase())
    }

    /// Where the exported game keeps its save games, as shown to players (README, export dialog).
    pub fn saves_hint(self, slug: &str) -> String {
        match self {
            Platform::Linux => format!("~/.local/share/{slug}/saves"),
            Platform::Windows => format!("%APPDATA%\\{slug}\\saves"),
            Platform::MacOS => format!("~/Library/Application Support/{slug}/saves"),
        }
    }

    /// The README of a bundle.
    #[cfg(not(target_arch = "wasm32"))]
    fn readme(self, name: &str, slug: &str) -> String {
        let saves = self.saves_hint(slug);
        let how = match self {
            Platform::Linux => format!("Run ./{slug} to play (Linux, x86_64).\nThe game data is in data/; save games are kept in {saves}."),
            Platform::Windows => format!(
                "Double-click {slug}.exe to play (Windows 10 or newer, 64-bit).\nThe game data is in the data folder; save games are kept in {saves}.\nWindows may warn that the publisher is unknown (the game isn't signed): choose \"More info\", then \"Run anyway\"."
            ),
            Platform::MacOS => {
                let app = app_bundle(name, slug);
                format!(
                    "Drag {app} into your Applications folder and double-click it to play (macOS 11 or newer, Apple Silicon and Intel).\nSave games are kept in {saves}.\n\nThe game isn't signed by Apple, so the first start is refused (\"Apple could not verify ...\"). Open System Settings > Privacy & Security, scroll down and choose \"Open Anyway\" next to {app}, then confirm with \"Open Anyway\". (On macOS 14 and older: right-click the app, choose Open, then Open.) Alternatively, in Terminal: xattr -dr com.apple.quarantine \"/Applications/{app}\""
                )
            }
        };
        let line_end = if self == Platform::Windows { "\r\n" } else { "\n" };
        format!("{name}\n\n{how}\n\nMade with R(ust)PG2D. The game executable is R(ust)PG2D's game runner, licensed under the MIT License: see {ENGINE_LICENSE_FILE}.\n").replace('\n', line_end)
    }
}

/// The macOS application bundle of a game: its name (without characters Finder or the zip can't
/// take) + `.app`; the slug if nothing is left.
pub fn app_bundle(name: &str, slug: &str) -> String {
    let clean: String = name.trim().chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '"' | '*' | '?' | '<' | '>' | '|') && !c.is_control()).collect();
    let clean = clean.trim().trim_start_matches('.').trim();
    format!("{}.app", if clean.is_empty() { slug } else { clean })
}

/// The macOS bundle identifier of a game (`org.rustpg2d.<slug>`).
#[cfg(not(target_arch = "wasm32"))]
fn bundle_id(slug: &str) -> String {
    let id: String = slug.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    format!("org.rustpg2d.{id}")
}

/// `Contents/Info.plist` of a game's macOS bundle.
#[cfg(not(target_arch = "wasm32"))]
fn info_plist(name: &str, slug: &str, icon: bool) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let id = bundle_id(slug);
    let icon = if icon { format!("    <key>CFBundleIconFile</key>\n    <string>{slug}.icns</string>\n") } else { String::new() };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundleExecutable</key>
    <string>{slug}</string>
{icon}    <key>CFBundleIdentifier</key>
    <string>{id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.role-playing-games</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#,
        name = esc(name)
    )
}

/// An `.icns` icon from a PNG of any size: the picture centred on a square, scaled (nearest
/// neighbour, pixel art) to 128, 256 and 512 px PNG entries, which macOS 10.7+ reads.
pub fn icns(png: &[u8]) -> Result<Vec<u8>> {
    let img = crate::imageio::Rgba::decode(png).context("the icon isn't a PNG image")?;
    let side = img.w.max(img.h).max(1);
    let mut square = crate::imageio::Rgba::new(side, side);
    let (ox, oy) = (((side - img.w) / 2) as i32, ((side - img.h) / 2) as i32);
    for y in 0..img.h as i32 {
        for x in 0..img.w as i32 {
            square.set(x + ox, y + oy, img.get(x, y));
        }
    }
    let mut body = Vec::new();
    for (kind, size) in [(b"ic07", 128u32), (b"ic08", 256), (b"ic09", 512)] {
        let data = square.resized(size, size).encode()?;
        body.extend_from_slice(kind);
        body.extend_from_slice(&(data.len() as u32 + 8).to_be_bytes());
        body.extend_from_slice(&data);
    }
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&(body.len() as u32 + 8).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
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

/// The project bundled with the running executable, if any: `data/` next to it, or in a macOS
/// application bundle `Contents/Resources/data` (the executable is in `Contents/MacOS`).
pub fn bundled_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join(DATA_DIR), dir.join("..").join("Resources").join(DATA_DIR)].into_iter().find(|d| d.join("project.ron").exists())
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
    // The translations the game offers.
    for code in &cfg.languages {
        let rel = format!("lang/{code}.ron");
        if root.join(&rel).exists() {
            files.insert(rel);
        } else {
            c.warnings.push(format!("The project offers the language '{code}' but has no {rel}; only R(ust)PG2D's own texts will be translated."));
        }
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
/// (error).
#[cfg(not(target_arch = "wasm32"))]
pub fn export(project: &Project, name: &str, platform: Platform, game_binary: &Path, out_dir: &Path, zip: bool, icon: Option<&[u8]>) -> Result<Exported> {
    let (files, check) = check(project);
    if !check.errors.is_empty() {
        bail!("{}", check.errors.join("\n"));
    }
    if !game_binary.is_file() {
        bail!("game executable not found: {}", game_binary.display());
    }
    let slug = slug(name);
    let head = std::fs::File::open(game_binary).and_then(|f| {
        let mut b = Vec::new();
        std::io::Read::read_to_end(&mut std::io::Read::take(f, 4), &mut b).map(|_| b)
    })?;
    match platform {
        Platform::Windows if !head.starts_with(b"MZ") => bail!("{} is not a Windows program", game_binary.display()),
        // Mach-O 64-bit (arm64 or x86_64) or a universal binary.
        Platform::MacOS if !matches!(head.as_slice(), [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe]) => bail!("{} is not a macOS program", game_binary.display()),
        _ => {}
    }
    let folder = out_dir.join(platform.folder(&slug));
    let project_root = project.root.canonicalize().unwrap_or_else(|_| project.root.clone());
    let folder_abs = std::fs::create_dir_all(out_dir).ok().and_then(|_| out_dir.canonicalize().ok()).map(|d| d.join(platform.folder(&slug)));
    if folder_abs.as_ref().is_some_and(|f| project_root.starts_with(f) || f.starts_with(&project_root)) {
        bail!("choose an output folder outside the project");
    }
    if folder.exists() {
        // Only an earlier export is replaced (any app name: the game may have been renamed).
        let earlier = folder.join(DATA_DIR).join("project.ron").exists()
            || std::fs::read_dir(&folder).into_iter().flatten().flatten().any(|e| e.path().extension().is_some_and(|x| x == "app") && e.path().join("Contents/Resources").join(DATA_DIR).join("project.ron").exists());
        if !earlier {
            bail!("{} already exists and is not an earlier export; choose another folder or name", folder.display());
        }
        std::fs::remove_dir_all(&folder).with_context(|| format!("removing the earlier export {}", folder.display()))?;
    }
    let data = folder.join(platform.data(name, &slug));
    let mut bytes = 0u64;
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
    let exe_rel = platform.exe(name, &slug);
    let exe = folder.join(&exe_rel);
    std::fs::create_dir_all(exe.parent().unwrap())?;
    std::fs::copy(game_binary, &exe).context("copying the game executable")?;
    // From the file: `std::fs::copy` returns 0 under Wine.
    bytes += std::fs::metadata(&exe).map_or(0, |m| m.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))?;
    }
    let mut extra = 3;
    if platform == Platform::MacOS {
        let contents = folder.join(app_bundle(name, &slug)).join("Contents");
        let icns = icon.map(icns).transpose()?;
        if let Some(i) = &icns {
            std::fs::write(contents.join("Resources").join(format!("{slug}.icns")), i)?;
            extra += 1;
        }
        std::fs::write(contents.join("Info.plist"), info_plist(name.trim(), &slug, icns.is_some()))?;
        std::fs::write(contents.join("PkgInfo"), "APPL????")?;
        // Seal the bundle (ad-hoc): the linker's signature of the bare runner doesn't cover it.
        crate::macsign::sign_app(&folder.join(app_bundle(name, &slug)), &bundle_id(&slug), &slug).context("signing the macOS app")?;
        extra += 3;
    }
    std::fs::write(folder.join("README.txt"), platform.readme(name.trim(), &slug))?;
    std::fs::write(folder.join(ENGINE_LICENSE_FILE), ENGINE_LICENSE)?;
    let archive = if zip { Some(write_zip(&folder, &slug, &exe_rel, &out_dir.join(platform.archive(&slug)))?) } else { None };
    Ok(Exported { folder, archive, files: files.len() + extra, bytes, warnings: check.warnings })
}

/// The web game's page, loader and storage plugin (`crates/engine/web/`).
pub const WEB_INDEX: &str = include_str!("../web/index.html");
pub const WEB_LOADER: &str = include_str!("../web/mq_js_bundle.js");
pub const WEB_PLUGIN: &str = include_str!("../web/rustpg2d.js");
/// The game compiled to WebAssembly, as the page loads it.
pub const WEB_WASM: &str = "rustpg2d-game.wasm";

/// Writes the web version of `project` named `name` into `out_dir/<slug>-web/`: `index.html`,
/// macroquad's loader, R(ust)PG2D's plugin, `game_wasm` (R(ust)PG2D's game runner compiled for
/// `wasm32-unknown-unknown`) and `data.pack` with the game's files; with `zip`, also
/// `out_dir/<slug>-web.zip` with those files at its root (as itch.io wants HTML games). An
/// earlier web export in the same place is replaced; any other folder of that name is left alone.
#[cfg(not(target_arch = "wasm32"))]
pub fn export_web(project: &Project, name: &str, game_wasm: &Path, out_dir: &Path, zip: bool) -> Result<Exported> {
    let (files, check) = check(project);
    if !check.errors.is_empty() {
        bail!("{}", check.errors.join("\n"));
    }
    let wasm = std::fs::read(game_wasm).with_context(|| format!("the web game runner {}", game_wasm.display()))?;
    if !wasm.starts_with(b"\0asm") {
        bail!("{} is not a WebAssembly file", game_wasm.display());
    }
    let slug = slug(name);
    let folder = out_dir.join(format!("{slug}-web"));
    let project_root = project.root.canonicalize().unwrap_or_else(|_| project.root.clone());
    let folder_abs = std::fs::create_dir_all(out_dir).ok().and_then(|_| out_dir.canonicalize().ok()).map(|d| d.join(format!("{slug}-web")));
    if folder_abs.as_ref().is_some_and(|f| project_root.starts_with(f) || f.starts_with(&project_root)) {
        bail!("choose an output folder outside the project");
    }
    if folder.exists() {
        if !folder.join("data.pack").exists() {
            bail!("{} already exists and is not an earlier web export; choose another folder or name", folder.display());
        }
        std::fs::remove_dir_all(&folder).with_context(|| format!("removing the earlier export {}", folder.display()))?;
    }
    std::fs::create_dir_all(&folder)?;
    let mut packed = Vec::new();
    for rel in &files {
        if rel == "project.ron" {
            continue;
        }
        packed.push((rel.clone(), std::fs::read(project.root.join(rel)).with_context(|| format!("reading {rel}"))?));
    }
    // The bundled project carries the chosen name (title, storage of settings).
    let mut cfg = project.config.clone();
    cfg.name = name.trim().to_string();
    packed.push(("project.ron".into(), ron::ser::to_string_pretty(&cfg, ron::ser::PrettyConfig::default().struct_names(false))?.into_bytes()));
    let title = name.trim().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let readme = format!(
        "{name}\n\nThe web version: open index.html through a web server (browsers don't run WebAssembly from file:// pages), e.g. `python3 -m http.server` in this folder and http://localhost:8000, or upload {slug}-web.zip to itch.io as an HTML game.\nSave games and settings stay in the player's browser (local storage of the page's site).\n\nMade with R(ust)PG2D. The game ({WEB_WASM}) is R(ust)PG2D's game runner, licensed under the MIT License: see {ENGINE_LICENSE_FILE}; mq_js_bundle.js is macroquad's loader (MIT OR Apache-2.0).\n",
        name = name.trim()
    );
    let out: Vec<(&str, Vec<u8>)> = vec![
        ("index.html", WEB_INDEX.replace("{title}", &title).into_bytes()),
        ("mq_js_bundle.js", WEB_LOADER.as_bytes().to_vec()),
        ("rustpg2d.js", WEB_PLUGIN.as_bytes().to_vec()),
        (WEB_WASM, wasm),
        ("data.pack", web_pack(&packed)?),
        ("README.txt", readme.into_bytes()),
        (ENGINE_LICENSE_FILE, ENGINE_LICENSE.as_bytes().to_vec()),
    ];
    let mut bytes = 0u64;
    for (n, b) in &out {
        std::fs::write(folder.join(n), b).with_context(|| format!("writing {n}"))?;
        bytes += b.len() as u64;
    }
    let archive = if zip {
        let path = out_dir.join(format!("{slug}-web.zip"));
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?);
        for (n, b) in &out {
            zw.start_file(*n, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o644).last_modified_time(zip_time()))?;
            zw.write_all(b)?;
        }
        zw.finish()?;
        Some(path)
    } else {
        None
    };
    Ok(Exported { folder, archive, files: out.len(), bytes, warnings: check.warnings })
}

/// Zips the bundle folder as `<slug>/…` (keeping the executable's permissions).
/// Now (UTC) as a zip timestamp; without one every entry would be dated 1980.
#[cfg(not(target_arch = "wasm32"))]
pub fn zip_time() -> zip::DateTime {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    zip::DateTime::from_date_and_time(year as u16, month as u8, day as u8, (rem / 3600) as u8, (rem % 3600 / 60) as u8, (rem % 60) as u8).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
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
            // Unix attributes whatever the host: Linux players need the executable bit.
            .system(zip::System::Unix)
            .unix_permissions(if exe { 0o755 } else { 0o644 })
            .last_modified_time(zip_time())
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
    fn runner_names_depend_on_the_host() {
        // The host's runner is the play-test runner `rustpg2d-game`; the others carry their platform.
        let host = Platform::host();
        for p in Platform::ALL {
            let name = p.game_runner();
            match p {
                Platform::Windows => assert_eq!(name, "rustpg2d-game.exe"),
                _ if p == host => assert_eq!(name, "rustpg2d-game"),
                Platform::Linux => assert_eq!(name, "rustpg2d-game-linux"),
                Platform::MacOS => assert_eq!(name, "rustpg2d-game-macos"),
            }
        }
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
        let db = Database { sheets: vec![SheetDef { id: "a".into(), image: "sprites/a.png".into(), frame_w: 32, frame_h: 32, ..Default::default() }], ..Default::default() };
        let mut p = Project { root: root.clone(), config: ProjectConfig::new("Test"), db };
        p.save_config().unwrap();
        p.save_defs().unwrap();
        let (_, c) = check(&p);
        assert!(!c.errors.is_empty(), "no maps yet");
        let mut map = Map::new("start", 8, 8);
        map.zones.push(Zone { uid: 1, name: "start".into(), x: 1, y: 1, w: 1, h: 1, kind: ZoneKind::Spawn, triggers: vec![], loot: None });
        p.save_map(&map).unwrap();
        p.config.player = "nobody".into();
        // A translation travels with the game.
        p.config.languages = vec!["de".into(), "fr".into()];
        std::fs::create_dir_all(root.join("lang")).unwrap();
        std::fs::write(root.join("lang/de.ron"), "(name: \"Deutsch\")").unwrap();
        let bin = tmp.join("rustpg2d-game");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let out = tmp.join("out");
        let r = export(&p, "Test Quest", Platform::Linux, &bin, &out, true, None).unwrap();
        let f = out.join("test-quest");
        assert!(f.join("test-quest").is_file());
        assert!(f.join("data/project.ron").is_file());
        assert!(f.join("data/maps/start.map").is_file());
        assert!(f.join("data/sprites/a.png").is_file());
        assert!(f.join("data/lang/de.ron").is_file(), "translations are exported");
        assert!(r.warnings.iter().any(|w| w.contains("'fr'")), "a language without its file is reported");
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
        // The web version: one pack with the game's files, the page and the loader.
        let wasm = tmp.join("rustpg2d-game.wasm");
        std::fs::write(&wasm, b"\0asm\x01\0\0\0").unwrap();
        assert!(export_web(&p, "Test Quest", &bin, &out, false).is_err(), "not WebAssembly");
        let r = export_web(&p, "Test Quest", &wasm, &out, true).unwrap();
        let w = out.join("test-quest-web");
        assert_eq!(r.folder, w);
        let html = std::fs::read_to_string(w.join("index.html")).unwrap();
        assert!(html.contains("<title>Test Quest</title>") && html.contains("load(\"rustpg2d-game.wasm\")"));
        assert!(html.contains("oncontextmenu=\"return false\"") && html.contains("\"contextmenu\""), "right-click is game input, not the browser's menu");
        assert!(w.join("mq_js_bundle.js").is_file() && w.join("rustpg2d.js").is_file() && w.join(WEB_WASM).is_file());
        let packed = read_web_pack(&std::fs::read(w.join("data.pack")).unwrap()).unwrap();
        let names: Vec<&str> = packed.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"maps/start.map") && names.contains(&"sprites/a.png") && names.contains(&"project.ron") && names.contains(&"lang/de.ron"));
        assert!(!names.iter().any(|n| n.starts_with("saves/") || n.contains("unused")), "{names:?}");
        let cfg: ProjectConfig = ron::from_str(std::str::from_utf8(&packed.iter().find(|(n, _)| n == "project.ron").unwrap().1).unwrap()).unwrap();
        assert_eq!(cfg.name, "Test Quest");
        let mut za = zip::ZipArchive::new(std::fs::File::open(out.join("test-quest-web.zip")).unwrap()).unwrap();
        assert!(za.by_name("index.html").is_ok(), "itch.io wants index.html at the zip's root");
        assert!(read_web_pack(b"junk").is_err());
        // Never into the project itself.
        assert!(export(&p, "Test Quest", Platform::Linux, &bin, &root, false, None).is_err());
        // Windows: needs a Windows program; its own folder, exe, zip and README next to Linux's.
        assert!(export(&p, "Test Quest", Platform::Windows, &bin, &out, false, None).is_err(), "not an exe");
        let win = tmp.join("rustpg2d-game.exe");
        std::fs::write(&win, b"MZ\x90\0fake").unwrap();
        let r = export(&p, "Test Quest", Platform::Windows, &win, &out, true, None).unwrap();
        let w = out.join("test-quest-windows");
        assert_eq!(r.folder, w);
        assert!(w.join("test-quest.exe").is_file() && w.join("data/maps/start.map").is_file());
        let readme = std::fs::read_to_string(w.join("README.txt")).unwrap();
        assert!(readme.contains("test-quest.exe") && readme.contains("%APPDATA%\\test-quest\\saves") && readme.contains("\r\n"), "{readme}");
        assert!(f.join("test-quest").is_file(), "the Linux export stays");
        let mut za = zip::ZipArchive::new(std::fs::File::open(out.join("test-quest-windows.zip")).unwrap()).unwrap();
        assert!(za.by_name("test-quest/test-quest.exe").is_ok() && za.by_name("test-quest/data/project.ron").is_ok());
        // macOS: an application bundle named after the game, data in Contents/Resources, an icon.
        assert!(export(&p, "Test Quest", Platform::MacOS, &win, &out, false, None).is_err(), "not a Mach-O program");
        let mac = tmp.join("rustpg2d-game-macos");
        std::fs::write(&mac, crate::macsign::tests::tiny_macho(true)).unwrap();
        let icon = crate::imageio::Rgba::new(20, 32).encode().unwrap();
        let r = export(&p, "Test: Quest", Platform::MacOS, &mac, &out, true, Some(&icon)).unwrap();
        let m = out.join("test-quest-macos");
        assert_eq!(r.folder, m);
        let app = m.join("Test Quest.app/Contents");
        assert!(app.join("MacOS/test-quest").is_file() && app.join("Resources/data/maps/start.map").is_file());
        assert_eq!(Project::load(&app.join("Resources/data")).unwrap().config.name, "Test: Quest");
        let plist = std::fs::read_to_string(app.join("Info.plist")).unwrap();
        assert!(plist.contains("<string>test-quest</string>") && plist.contains("<string>Test: Quest</string>") && plist.contains("test-quest.icns"), "{plist}");
        let icns = std::fs::read(app.join("Resources/test-quest.icns")).unwrap();
        assert!(icns.starts_with(b"icns") && u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize == icns.len() && icns[8..12] == *b"ic07");
        assert!(std::fs::read_to_string(m.join("README.txt")).unwrap().contains("~/Library/Application Support/test-quest/saves"));
        let res = std::fs::read_to_string(app.join("_CodeSignature/CodeResources")).unwrap();
        assert!(res.contains("<key>Resources/data/maps/start.map</key>") && res.contains("<key>Resources/test-quest.icns</key>"), "the bundle is sealed");
        let mut za = zip::ZipArchive::new(std::fs::File::open(out.join("test-quest-macos.zip")).unwrap()).unwrap();
        let exe = za.by_name("test-quest/Test Quest.app/Contents/MacOS/test-quest").unwrap();
        assert_eq!(exe.unix_mode().map(|m| m & 0o777), Some(0o755));
        assert!(exe.last_modified().is_some_and(|t| t.year() >= 2025), "dated, not 1980");
        drop(exe);
        // Renamed game: the earlier bundle (other app name) is still recognised and replaced.
        export(&p, "Test Quest!", Platform::MacOS, &mac, &out, false, None).unwrap();
        assert!(!m.join("Test Quest.app").exists() && m.join("Test Quest!.app/Contents/MacOS/test-quest").is_file());
        assert!(!m.join("Test Quest!.app/Contents/Resources/test-quest.icns").exists() && !std::fs::read_to_string(m.join("Test Quest!.app/Contents/Info.plist")).unwrap().contains("icns"));
        assert_eq!(app_bundle("../a/b", "x"), "ab.app");
        assert_eq!(app_bundle(" /: ", "my-game"), "my-game.app");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
