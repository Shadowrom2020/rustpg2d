//! Linux packages built from a file tree: **AppImage** (one executable file for most
//! distributions) and **Flatpak bundle** (sandboxed, `flatpak install --user X.flatpak`).
//!
//! Used by the editor's *Export game* (a game: its executable next to `data/`) and by
//! `cargo xtask-dist` (the R(ust)PG2D toolkit: editor, game runner and the example project).
//!
//! Both formats get the same tree below their prefix (`usr/` in the AppImage, `/app` in the
//! Flatpak), commands as symlinks in `bin/`, a desktop entry and an icon:
//! - AppImage: the official AppImage runtime (type 2, static; downloaded once, pinned by SHA-256,
//!   cached in `~/.cache/rustpg2d/`) followed by a squashfs image written with `backhand` - no
//!   AppImage tools needed. `AppRun` links to `usr/bin/<command>`.
//! - Flatpak: a build folder finished with `flatpak build-finish`, exported to a temporary OSTree
//!   repo and bundled with `flatpak build-bundle` (only the `flatpak` command is needed; inside a
//!   Flatpak sandbox it is reached with `flatpak-spawn --host`). The bundle names Flathub as the
//!   runtime source, so installing it fetches the runtime.
//!
//! Executables find their files next to themselves: the kernel resolves the `bin/` links, so
//! `current_exe()` is the real file under `lib/`.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use backhand::compression::Compressor;
use backhand::{FilesystemCompressor, FilesystemWriter, NodeHeader};
use rustpg2d_engine::hash::{hex, sha256};

/// The AppImage runtime (type 2, static: needs no libfuse on the player's machine, only
/// `fusermount`; without FUSE, `--appimage-extract-and-run` works).
const RUNTIME_VERSION: &str = "20251108";
const RUNTIME_URL: &str = "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64";
const RUNTIME_SHA256: &str = "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d";

/// Flatpak runtimes offered (freedesktop SDK branches; glibc 2.42 / 2.44).
pub const FLATPAK_RUNTIMES: [&str; 2] = ["25.08", "26.08"];
const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// Size of generated icons.
pub use rustpg2d_engine::icon::{emblem, hero_icon, ICON};

/// Where a file's bytes come from.
pub enum Source {
    Bytes(Vec<u8>),
    Path(PathBuf),
}

impl Source {
    fn read(&self) -> Result<Vec<u8>> {
        match self {
            Source::Bytes(b) => Ok(b.clone()),
            Source::Path(p) => std::fs::read(p).with_context(|| format!("reading {}", p.display())),
        }
    }
}

/// A file of the package: `dest` relative to the prefix (e.g. `lib/game/game`).
pub struct Entry {
    pub dest: String,
    pub src: Source,
    pub exec: bool,
}

/// What to package.
pub struct AppSpec {
    /// The command started by the desktop entry / AppImage (a name in `bin/`).
    pub command: String,
    /// Display name and one-line description.
    pub name: String,
    pub comment: String,
    /// Desktop entry categories, e.g. "Game;RolePlaying;".
    pub categories: String,
    /// Flatpak application id (also its desktop entry / icon name).
    pub app_id: String,
    /// Freedesktop runtime branch of the Flatpak (`FLATPAK_RUNTIMES`).
    pub runtime: String,
    /// `flatpak build-finish` permissions, e.g. `--socket=x11`.
    pub finish_args: Vec<String>,
    pub files: Vec<Entry>,
    /// Commands: (name in `bin/`, target relative to the prefix).
    pub bin: Vec<(String, String)>,
    /// PNG.
    pub icon: Vec<u8>,
    /// SPDX license of the application for its AppStream metadata ("" = none stated, e.g. a
    /// game whose creator didn't choose one).
    pub license: String,
}

impl AppSpec {
    /// Adds every file below `dir` (sorted) under `dest`; `exec` decides by relative path.
    pub fn add_dir(&mut self, dir: &Path, dest: &str, exec: &dyn Fn(&str) -> bool) -> Result<()> {
        for rel in walk(dir)? {
            self.files.push(Entry { dest: format!("{dest}/{rel}"), src: Source::Path(dir.join(&rel)), exec: exec(&rel) });
        }
        Ok(())
    }
}

// ------------------------------------------------------------------ AppImage

/// Writes the AppImage to `path`.
pub fn appimage(spec: &AppSpec, path: &Path) -> Result<()> {
    let runtime = appimage_runtime()?;
    let squashfs = appimage_squashfs(spec)?;
    std::fs::write(path, [runtime.as_slice(), squashfs.as_slice()].concat()).with_context(|| format!("writing {}", path.display()))?;
    set_executable(path)
}

/// The AppDir as a squashfs image:
/// ```text
/// AppRun -> usr/bin/<command>     <command>.desktop     <command>.png     .DirIcon
/// usr/<files>   usr/bin/<links>   usr/share/applications, usr/share/icons
/// ```
pub fn appimage_squashfs(spec: &AppSpec) -> Result<Vec<u8>> {
    let mtime = now() as u32;
    let dir = NodeHeader::new(0o755, 0, 0, mtime);
    let file = NodeHeader::new(0o644, 0, 0, mtime);
    let exe = NodeHeader::new(0o755, 0, 0, mtime);
    let mut fs = FilesystemWriter::default();
    fs.set_compressor(FilesystemCompressor::new(Compressor::Gzip, None)?);
    fs.set_time(mtime);
    fs.set_root_mode(0o755);
    let cmd = &spec.command;
    let desktop = desktop_entry(spec, cmd, cmd);
    // backhand keeps readers until write(): give it owned bytes.
    let mut files: Vec<(String, Vec<u8>, NodeHeader)> = vec![
        (format!("{cmd}.desktop"), desktop.clone().into_bytes(), file),
        (format!("{cmd}.png"), spec.icon.clone(), file),
        (format!("usr/share/applications/{cmd}.desktop"), desktop.into_bytes(), file),
        (format!("usr/share/icons/hicolor/{ICON}x{ICON}/apps/{cmd}.png"), spec.icon.clone(), file),
    ];
    for e in &spec.files {
        files.push((format!("usr/{}", e.dest), e.src.read()?, if e.exec { exe } else { file }));
    }
    let links: Vec<(String, String)> = spec.bin.iter().map(|(name, target)| (format!("usr/bin/{name}"), format!("../{target}"))).collect();
    let mut dirs: Vec<String> = files.iter().map(|f| &f.0).chain(links.iter().map(|l| &l.0)).flat_map(|p| parents(p)).collect();
    dirs.sort();
    dirs.dedup();
    for d in &dirs {
        fs.push_dir(d, dir)?;
    }
    for (path, bytes, header) in files {
        fs.push_file(Cursor::new(bytes), path, header)?;
    }
    for (path, target) in links {
        fs.push_symlink(target, path, exe)?;
    }
    fs.push_symlink(format!("usr/bin/{cmd}"), "AppRun", exe)?;
    fs.push_symlink(format!("{cmd}.png"), ".DirIcon", file)?;
    let mut out = Cursor::new(Vec::new());
    fs.write(&mut out).context("writing the squashfs image")?;
    Ok(out.into_inner())
}

/// The cached AppImage runtime, downloaded on first use (curl or wget) and checked against
/// its pinned hash.
fn appimage_runtime() -> Result<Vec<u8>> {
    let cache = cache_dir().join(format!("appimage-runtime-{RUNTIME_VERSION}-x86_64"));
    if let Ok(b) = std::fs::read(&cache) {
        if hex(&sha256(&b)) == RUNTIME_SHA256 {
            return Ok(b);
        }
    }
    std::fs::create_dir_all(cache.parent().unwrap())?;
    let part = cache.with_extension("part");
    let ok = |c: &mut Command| c.status().is_ok_and(|s| s.success());
    let fetched = ok(Command::new("curl").args(["-fsSL", "--retry", "2", "-o"]).arg(&part).arg(RUNTIME_URL))
        || ok(Command::new("wget").args(["-q", "-O"]).arg(&part).arg(RUNTIME_URL));
    if !fetched {
        let _ = std::fs::remove_file(&part);
        bail!(
            "could not download the AppImage runtime (needs curl or wget and an internet connection). You can also download {RUNTIME_URL} yourself and save it as {}",
            cache.display()
        );
    }
    let b = std::fs::read(&part)?;
    if hex(&sha256(&b)) != RUNTIME_SHA256 {
        let _ = std::fs::remove_file(&part);
        bail!("the downloaded AppImage runtime has an unexpected checksum; nothing was used");
    }
    std::fs::rename(&part, &cache)?;
    Ok(b)
}

// ------------------------------------------------------------------ Flatpak

/// Inside a Flatpak sandbox the host's `flatpak` is reached through `flatpak-spawn --host`.
fn flatpak_cmd() -> Command {
    if Path::new("/.flatpak-info").exists() {
        let mut c = Command::new("flatpak-spawn");
        c.args(["--host", "flatpak"]);
        c
    } else {
        Command::new("flatpak")
    }
}

/// Whether the `flatpak` command is available (needed for Flatpak bundles).
pub fn flatpak_available() -> bool {
    flatpak_cmd().arg("--version").output().is_ok_and(|o| o.status.success())
}

/// Checks a Flatpak application id: at least three dot-separated parts of letters, digits,
/// `_` and `-`, none starting with a digit.
pub fn check_app_id(id: &str) -> Result<(), String> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() < 3 {
        return Err("needs at least three parts, like org.example.MyGame".into());
    }
    for p in &parts {
        if p.is_empty() || p.starts_with(|c: char| c.is_ascii_digit()) || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("'{p}' isn't allowed: letters, digits, _ and - only, not starting with a digit"));
        }
    }
    if id.len() > 255 {
        return Err("is too long".into());
    }
    Ok(())
}

/// Writes the Flatpak bundle to `path`, working in a temporary folder next to it (the host's
/// flatpak must see it when called from a sandbox).
pub fn flatpak_bundle(spec: &AppSpec, path: &Path) -> Result<()> {
    if !cfg!(unix) {
        bail!("Flatpak bundles can only be made on Linux (they need the flatpak command)");
    }
    check_app_id(&spec.app_id).map_err(|e| anyhow::anyhow!("Flatpak app id: {e}"))?;
    let name = path.file_name().map_or("app".into(), |n| n.to_string_lossy().into_owned());
    let work = path.parent().unwrap_or(Path::new(".")).join(format!(".{name}-build"));
    let _ = std::fs::remove_dir_all(&work);
    let res = flatpak_in(&work, spec, path);
    let _ = std::fs::remove_dir_all(&work);
    res
}

fn flatpak_in(work: &Path, spec: &AppSpec, path: &Path) -> Result<()> {
    let build = work.join("build");
    let files = build.join("files");
    for e in &spec.files {
        let dst = files.join(&e.dest);
        write(&dst, &e.src.read()?)?;
        if e.exec {
            set_executable(&dst)?;
        }
    }
    for (name, target) in &spec.bin {
        let link = files.join("bin").join(name);
        std::fs::create_dir_all(link.parent().unwrap())?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(format!("../{target}"), link)?;
        #[cfg(not(unix))]
        let _ = (target, link);
    }
    let id = &spec.app_id;
    let share = files.join("share");
    write(&share.join(format!("applications/{id}.desktop")), desktop_entry(spec, &spec.command, id).as_bytes())?;
    write(&share.join(format!("icons/hicolor/{ICON}x{ICON}/apps/{id}.png")), &spec.icon)?;
    write(&share.join(format!("metainfo/{id}.metainfo.xml")), metainfo(spec).as_bytes())?;
    std::fs::create_dir_all(build.join("var"))?;
    let rt = &spec.runtime;
    std::fs::write(build.join("metadata"), format!("[Application]\nname={id}\nruntime=org.freedesktop.Platform/x86_64/{rt}\nsdk=org.freedesktop.Sdk/x86_64/{rt}\n"))?;
    run(flatpak_cmd().args(["build-finish", "--command", &spec.command]).args(&spec.finish_args).arg(&build))?;
    let repo = work.join("repo");
    run(flatpak_cmd().args(["build-export", "--no-update-summary"]).arg(&repo).arg(&build).arg("stable"))?;
    run(flatpak_cmd().arg("build-bundle").arg(format!("--runtime-repo={FLATHUB_REPO}")).arg(&repo).arg(path).arg(id).arg("stable"))?;
    Ok(())
}

/// AppStream metadata (shown by software centers when the bundle is opened).
fn metainfo(spec: &AppSpec) -> String {
    let (id, name, summary) = (&spec.app_id, xml_escape(&spec.name), xml_escape(&spec.comment));
    let license = if spec.license.is_empty() { String::new() } else { format!("  <project_license>{}</project_license>\n", xml_escape(&spec.license)) };
    let cats: String = spec.categories.split(';').filter(|c| !c.is_empty()).map(|c| format!("<category>{c}</category>")).collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>{id}</id>
  <name>{name}</name>
  <summary>{summary}</summary>
  <metadata_license>CC0-1.0</metadata_license>
{license}  <description><p>{summary}.</p></description>
  <launchable type="desktop-id">{id}.desktop</launchable>
  <categories>{cats}</categories>
  <content_rating type="oars-1.1"/>
</component>
"#
    )
}

fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd.output().with_context(|| format!("running {:?}", cmd.get_program()))?;
    if !out.status.success() {
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).filter(|a| a.starts_with("build")).collect();
        bail!("flatpak {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

// ------------------------------------------------------------------ shared

/// A desktop entry (`exec` = command, `icon` = icon name).
fn desktop_entry(spec: &AppSpec, exec: &str, icon: &str) -> String {
    let clean = |s: &str| s.chars().filter(|c| !c.is_control()).collect::<String>();
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={exec}\nIcon={icon}\nTerminal=false\nCategories={}\n",
        clean(&spec.name),
        clean(&spec.comment),
        clean(&spec.categories)
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The highest glibc version a Linux executable needs, e.g. (2, 34): the non-weak `GLIBC_2.x`
/// entries of its ELF version-needs table (weak ones, like the optional `pidfd_*` functions of
/// Rust's standard library, don't stop it from starting). For data that isn't an ELF file the
/// `GLIBC_2.x` names are just searched. None if there are none.
pub fn glibc_needed(binary: &[u8]) -> Option<(u32, u32)> {
    let names = elf_version_needs(binary).unwrap_or_else(|| scan_glibc_names(binary));
    names.iter().filter_map(|n| n.strip_prefix("GLIBC_2.")?.split('.').next()?.parse::<u32>().ok()).max().map(|m| (2, m))
}

/// Names of the non-weak version needs of a 64-bit little-endian ELF file.
fn elf_version_needs(b: &[u8]) -> Option<Vec<String>> {
    const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
    const VER_FLG_WEAK: u16 = 0x2;
    if b.get(..6)? != b"\x7fELF\x02\x01" {
        return None;
    }
    let u16_at = |o: usize| b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]));
    let u32_at = |o: usize| b.get(o..o + 4).map(|x| u32::from_le_bytes(x.try_into().unwrap()));
    let u64_at = |o: usize| b.get(o..o + 8).map(|x| u64::from_le_bytes(x.try_into().unwrap()) as usize);
    let (shoff, shentsize, shnum) = (u64_at(0x28)?, u16_at(0x3a)? as usize, u16_at(0x3c)? as usize);
    let section = |k: usize| -> Option<(u32, usize, usize, usize)> {
        let h = shoff + k * shentsize;
        Some((u32_at(h + 4)?, u64_at(h + 0x18)?, u64_at(h + 0x20)?, u32_at(h + 0x28)? as usize))
    };
    let (_, off, _, link) = (0..shnum).filter_map(section).find(|s| s.0 == SHT_GNU_VERNEED)?;
    let (_, str_off, str_size, _) = section(link)?;
    let name_at = |o: usize| -> Option<String> {
        let s = b.get(str_off + o..str_off + str_size)?;
        Some(String::from_utf8_lossy(&s[..s.iter().position(|&c| c == 0)?]).into_owned())
    };
    let mut out = Vec::new();
    let mut vn = off;
    loop {
        let (cnt, aux, next) = (u16_at(vn + 2)?, u32_at(vn + 8)? as usize, u32_at(vn + 12)? as usize);
        let mut a = vn + aux;
        for _ in 0..cnt {
            let (flags, name, anext) = (u16_at(a + 4)?, u32_at(a + 8)? as usize, u32_at(a + 12)? as usize);
            if flags & VER_FLG_WEAK == 0 {
                out.push(name_at(name)?);
            }
            a += anext;
        }
        if next == 0 {
            return Some(out);
        }
        vn += next;
    }
}

/// `GLIBC_2.x` names anywhere in the data.
fn scan_glibc_names(binary: &[u8]) -> Vec<String> {
    const PAT: &[u8] = b"GLIBC_2.";
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = binary[i..].windows(PAT.len()).position(|w| w == PAT) {
        let start = i + k + PAT.len();
        let digits: String = binary[start..].iter().take_while(|b| b.is_ascii_digit()).map(|&b| b as char).collect();
        out.push(format!("GLIBC_2.{digits}"));
        i = start;
    }
    out
}

fn cache_dir() -> PathBuf {
    let base = rustpg2d_engine::paths::cache_base().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("rustpg2d");
    // The downloaded AppImage runtime of a rust2d install (saves a download, works offline).
    rustpg2d_engine::legacy::adopt(&dir, &base.join(rustpg2d_engine::legacy::OLD_NAME));
    dir
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Marks a file executable (Unix; elsewhere files have no such bit: nothing to do).
fn set_executable(p: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).with_context(|| format!("making {} executable", p.display()))?;
    }
    let _ = p;
    Ok(())
}

fn write(p: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, bytes).with_context(|| format!("writing {}", p.display()))
}

/// Files below `dir` as relative paths with `/`, sorted.
fn walk(dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))?.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir)?.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The parent folders of a relative path ("a/b/c" -> "a", "a/b").
fn parents(p: &str) -> Vec<String> {
    let parts: Vec<&str> = p.split('/').collect();
    (1..parts.len()).map(|n| parts[..n].join("/")).collect()
}

// ------------------------------------------------------------------ SHA-256

/// SHA-256 of `bytes` as lowercase hex (SHA256SUMS files).
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sha256(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn app_ids() {
        assert!(check_app_id("org.rustpg2d.knight_tales").is_ok());
        assert!(check_app_id("org.example").is_err());
        assert!(check_app_id("org.example.1game").is_err());
        assert!(check_app_id("org.exa mple.game").is_err());
        assert!(check_app_id("io.github.someone.My-Game").is_ok());
    }

    #[test]
    #[cfg(target_os = "linux")] // reads the test binary itself as an ELF file
    fn glibc_scan() {
        assert_eq!(glibc_needed(b"\0GLIBC_2.2.5\0GLIBC_2.34\0GLIBC_2.17\0"), Some((2, 34)));
        assert_eq!(glibc_needed(b"no versions here"), None);
        // A real ELF file: the test binary itself needs some glibc 2.x.
        let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        assert!(elf_version_needs(&me).is_some_and(|n| n.iter().any(|x| x.starts_with("GLIBC_2."))));
        assert!(glibc_needed(&me).is_some_and(|v| v >= (2, 2)));
    }

    /// A small spec: an executable next to its data, one command.
    pub fn spec(tmp: &Path) -> AppSpec {
        std::fs::create_dir_all(tmp.join("data/maps")).unwrap();
        std::fs::write(tmp.join("data/project.ron"), b"()").unwrap();
        std::fs::write(tmp.join("data/maps/start.map"), b"map").unwrap();
        let mut s = AppSpec {
            command: "knight-tales".into(),
            name: "Knight Tales".into(),
            comment: "A game".into(),
            categories: "Game;RolePlaying;".into(),
            app_id: "org.rustpg2d.knight_tales".into(),
            runtime: FLATPAK_RUNTIMES[0].into(),
            finish_args: vec!["--socket=x11".into()],
            files: vec![Entry { dest: "lib/knight-tales/knight-tales".into(), src: Source::Bytes(b"\x7fELF game".to_vec()), exec: true }],
            bin: vec![("knight-tales".into(), "lib/knight-tales/knight-tales".into())],
            icon: emblem().encode().unwrap(),
            license: String::new(),
        };
        s.add_dir(&tmp.join("data"), "lib/knight-tales/data", &|_| false).unwrap();
        s
    }

    #[test]
    #[cfg(unix)] // AppImages are made on Linux (paths, permissions)
    fn appimage_contents() {
        use backhand::{FilesystemReader, InnerNode};
        let tmp = std::env::temp_dir().join(format!("rustpg2d_appimage_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let img = appimage_squashfs(&spec(&tmp)).unwrap();
        let fs = FilesystemReader::from_reader(Cursor::new(img)).unwrap();
        let mut seen = std::collections::BTreeMap::new();
        for node in fs.files() {
            let path = node.fullpath.to_string_lossy().trim_start_matches('/').to_string();
            let what = match &node.inner {
                InnerNode::File(f) => {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut fs.file(f).reader(), &mut buf).unwrap();
                    format!("file {} {:o}", String::from_utf8_lossy(&buf[..buf.len().min(9)]), node.header.permissions)
                }
                InnerNode::Symlink(s) => format!("-> {}", s.link.display()),
                InnerNode::Dir(_) => "dir".into(),
                _ => "other".into(),
            };
            seen.insert(path, what);
        }
        assert_eq!(seen["AppRun"], "-> usr/bin/knight-tales");
        assert_eq!(seen["usr/bin/knight-tales"], "-> ../lib/knight-tales/knight-tales");
        assert_eq!(seen[".DirIcon"], "-> knight-tales.png");
        assert_eq!(seen["usr/lib/knight-tales/knight-tales"], "file \x7fELF game 755");
        assert_eq!(seen["usr/lib/knight-tales/data/maps/start.map"], "file map 644");
        assert!(seen["knight-tales.desktop"].starts_with("file [Desktop"));
        assert!(seen.contains_key("usr/share/icons/hicolor/256x256/apps/knight-tales.png"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn desktop_and_metainfo() {
        let tmp = std::env::temp_dir().join(format!("rustpg2d_meta_test_{}", std::process::id()));
        let s = spec(&tmp);
        let d = desktop_entry(&s, "knight-tales", "org.rustpg2d.knight_tales");
        assert!(d.contains("Exec=knight-tales\n") && d.contains("Icon=org.rustpg2d.knight_tales\n") && d.contains("Categories=Game;RolePlaying;\n"));
        let m = metainfo(&s);
        assert!(m.contains("<category>Game</category><category>RolePlaying</category>"));
        assert!(!m.contains("project_license"), "no license claimed for a game");
        let m = metainfo(&AppSpec { license: "MIT".into(), ..s });
        assert!(m.contains("<project_license>MIT</project_license>"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
