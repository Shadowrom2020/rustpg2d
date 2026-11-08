//! `cargo xtask-dist`: the R(ust)PG2D toolkit as AppImage and Flatpak for Linux and a zip for
//! Windows (feature `dist`).
//!
//! Options: `--out DIR` (default `dist/`), `--bin-dir DIR` (the Linux release binaries, default
//! `target/release`), `--windows-dir DIR` (the Windows release binaries `rustpg2d-editor.exe` and
//! `rustpg2d-game.exe`, cross-compiled with MinGW-w64; default `target/x86_64-pc-windows-gnu/
//! release`), `--require-windows` (fail without them: CI). The Windows game runner also ships in
//! the Linux packages and the Linux one in the Windows zip, so either editor exports games for
//! both.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use rustpg2d_packaging::{AppSpec, Entry, Source};

use crate::{read, root, workspace_version};

/// The oldest glibc the packaged binaries may need (Debian 12, Ubuntu 22.04, RHEL 9).
const MAX_GLIBC: (u32, u32) = (2, 35);
const APP_ID: &str = "io.github.Shadowrom2020.rustpg2d";

pub fn dist(args: &[String]) -> Result<()> {
    let root = root();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let out = opt("--out").unwrap_or_else(|| root.join("dist"));
    let bins = opt("--bin-dir").unwrap_or_else(|| root.join("target/release"));
    let version = workspace_version(&read(&root.join("Cargo.toml"))?)?;
    let (editor, game) = (bins.join("rustpg2d-editor"), bins.join("rustpg2d-game"));
    let game_linux = game.clone();
    for b in [&editor, &game] {
        let bytes = std::fs::read(b).with_context(|| format!("{} not found: run `cargo build --release` first", b.display()))?;
        match rustpg2d_packaging::glibc_needed(&bytes) {
            Some(g) if g > MAX_GLIBC => bail!("{} needs glibc {}.{} (at most {}.{} allowed): build on an older system", b.display(), g.0, g.1, MAX_GLIBC.0, MAX_GLIBC.1),
            Some(g) => println!("{}: needs glibc {}.{}", b.display(), g.0, g.1),
            None => println!("{}: no glibc version information", b.display()),
        }
    }
    let win_dir = opt("--windows-dir").unwrap_or_else(|| root.join("target/x86_64-pc-windows-gnu/release"));
    let (win_editor, win_game) = (win_dir.join("rustpg2d-editor.exe"), win_dir.join("rustpg2d-game.exe"));
    let windows = win_editor.is_file() && win_game.is_file();
    if windows {
        for w in [&win_editor, &win_game] {
            if !std::fs::read(w)?.starts_with(b"MZ") {
                bail!("{} is not a Windows program", w.display());
            }
        }
        println!("{}: the Windows build", win_dir.display());
    } else if args.iter().any(|a| a == "--require-windows") {
        bail!("the Windows build is missing in {}: cargo build --release --target x86_64-pc-windows-gnu -p rustpg2d-editor -p rustpg2d-game", win_dir.display());
    } else {
        println!("no Windows build in {}: no Windows zip, and the packaged editor won't export Windows games", win_dir.display());
    }
    let windows_game = windows.then(|| win_game.clone());
    let example = root.join("example_project");
    let project = rustpg2d_engine::project::Project::load(&example).context("loading example_project")?;
    let icon = rustpg2d_packaging::hero_icon(&project).unwrap_or_else(rustpg2d_packaging::emblem).encode()?;
    let mut spec = AppSpec {
        command: "rustpg2d-editor".into(),
        name: "R(ust)PG2D".into(),
        comment: "Editor and engine for 2D top-down role-playing games".into(),
        categories: "Development;Game;".into(),
        app_id: APP_ID.into(),
        runtime: rustpg2d_packaging::FLATPAK_RUNTIMES[0].into(),
        // Window, sound, devices (GPU, gamepads for play-tests); projects anywhere in home; network for the AppImage runtime of
        // game exports; the host's flatpak for Flatpak game exports.
        finish_args: ["--share=ipc", "--socket=x11", "--socket=pulseaudio", "--device=all", "--share=network", "--filesystem=home", "--talk-name=org.freedesktop.Flatpak"]
            .map(String::from)
            .to_vec(),
        files: vec![
            Entry { dest: "lib/rustpg2d/rustpg2d-editor".into(), src: Source::Path(editor), exec: true },
            Entry { dest: "lib/rustpg2d/rustpg2d-game".into(), src: Source::Path(game), exec: true },
        ],
        bin: vec![("rustpg2d-editor".into(), "lib/rustpg2d/rustpg2d-editor".into()), ("rustpg2d-game".into(), "lib/rustpg2d/rustpg2d-game".into())],
        icon,
        license: "MIT".into(),
    };
    if let Some(w) = windows_game {
        // Next to the editor, where File > Export game looks for it.
        spec.files.push(Entry { dest: "lib/rustpg2d/rustpg2d-game.exe".into(), src: Source::Path(w), exec: false });
    }
    let license = root.join("LICENSE");
    if !license.is_file() {
        bail!("LICENSE is missing: the packages must carry R(ust)PG2D's license");
    }
    spec.files.push(Entry { dest: format!("share/licenses/{APP_ID}/LICENSE"), src: Source::Path(license), exec: false });
    // The example project without the developer's save games.
    let before = spec.files.len();
    spec.add_dir(&example, "share/rustpg2d/example_project", &|_| false)?;
    let mut i = before;
    while i < spec.files.len() {
        if spec.files[i].dest.starts_with("share/rustpg2d/example_project/saves/") {
            spec.files.remove(i);
        } else {
            i += 1;
        }
    }
    std::fs::create_dir_all(&out)?;
    let appimage = out.join(format!("rustpg2d-{version}-x86_64.AppImage"));
    let flatpak = out.join(format!("rustpg2d-{version}.flatpak"));
    rustpg2d_packaging::appimage(&spec, &appimage)?;
    println!("{}", appimage.display());
    rustpg2d_packaging::flatpak_bundle(&spec, &flatpak)?;
    println!("{}", flatpak.display());
    let mut made = vec![appimage, flatpak];
    if windows {
        let zip = out.join(format!("rustpg2d-{version}-windows-x86_64.zip"));
        windows_zip(&zip, &version, &win_editor, &win_game, &game_linux, &example)?;
        println!("{}", zip.display());
        made.push(zip);
    }
    let mut sums = String::new();
    for p in &made {
        sums += &format!("{}  {}\n", rustpg2d_packaging::sha256_hex(&std::fs::read(p)?), p.file_name().unwrap().to_string_lossy());
    }
    std::fs::write(out.join("SHA256SUMS"), sums)?;
    Ok(())
}


/// The R(ust)PG2D download for Windows: `rustpg2d-<version>/` with both executables, the Linux
/// game runner (Linux exports), the example project (without save games), license and README.
fn windows_zip(path: &std::path::Path, version: &str, editor: &std::path::Path, game: &std::path::Path, linux_game: &std::path::Path, example: &std::path::Path) -> Result<()> {
    use std::io::Write;
    let top = format!("rustpg2d-{version}");
    let mut zw = zip::ZipWriter::new(std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).system(zip::System::Unix).large_file(true);
    let mut add = |name: &str, bytes: &[u8], mode: u32| -> Result<()> {
        zw.start_file(format!("{top}/{name}"), opts.unix_permissions(mode))?;
        zw.write_all(bytes)?;
        Ok(())
    };
    add("rustpg2d-editor.exe", &std::fs::read(editor)?, 0o755)?;
    add("rustpg2d-game.exe", &std::fs::read(game)?, 0o755)?;
    add("rustpg2d-game", &std::fs::read(linux_game)?, 0o755)?;
    add("LICENSE.txt", read(&root().join("LICENSE"))?.replace('\n', "\r\n").as_bytes(), 0o644)?;
    add("README.txt", WINDOWS_README.replace("{version}", version).replace('\n', "\r\n").as_bytes(), 0o644)?;
    let mut stack = vec![example.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p != example.join("saves") {
                    stack.push(p);
                }
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    for f in files {
        let rel = f.strip_prefix(example)?.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        add(&format!("example_project/{rel}"), &std::fs::read(&f)?, 0o644)?;
    }
    zw.finish()?;
    Ok(())
}

const WINDOWS_README: &str = "R(ust)PG2D {version} for Windows (10 / 11, 64-bit)

Start rustpg2d-editor.exe: the welcome screen opens the example project or one of your own.
F5 in the editor play-tests the open map with rustpg2d-game.exe.

R(ust)PG2D isn't code-signed: the first start shows \"Windows protected your PC\". Choose
\"More info\", then \"Run anyway\".

Your files:
- the example project you open is copied to %USERPROFILE%\\rustpg2d\\example_project
- the asset library: %LOCALAPPDATA%\\rustpg2d\\library
- settings and the recent projects: %APPDATA%\\rustpg2d

File > Export game makes games for Windows (rustpg2d-game.exe) and Linux (rustpg2d-game, the
Linux game runner in this folder). AppImage and Flatpak packages are made by R(ust)PG2D on Linux.

Documentation and source: https://github.com/Shadowrom2020/rustpg2d
R(ust)PG2D is MIT licensed: see LICENSE.txt.
";
