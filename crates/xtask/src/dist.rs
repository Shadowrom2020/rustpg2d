//! `cargo xtask-dist`: the R(ust)PG2D toolkit for every desktop (feature `dist`):
//! - Linux: `rustpg2d-X.Y.Z-x86_64.AppImage` and `rustpg2d-X.Y.Z.flatpak`;
//! - Windows: `rustpg2d-X.Y.Z-windows-x86_64.zip` (needs the Windows build);
//! - macOS: `macos/R(ust)PG2D.app` (needs the macOS build), unsigned and not zipped: CI's macOS job
//!   signs it with `codesign` and zips it as `rustpg2d-X.Y.Z-macos.zip` (`macos/macos-app.tar`
//!   carries it there with its file modes);
//! - `SHA256SUMS` of what it wrote.
//!
//! Every editor gets the game runners of the other platforms and the web game next to it, so it
//! exports games for Linux, Windows, macOS and browsers.
//!
//! Options: `--out DIR` (default `dist/`), `--bin-dir DIR` (the Linux release binaries, default
//! `target/release`), `--web-wasm FILE` (the game for browsers, default
//! `target/wasm32-unknown-unknown/release/rustpg2d-game.wasm`), `--windows-dir DIR`
//! (`rustpg2d-editor.exe` and `rustpg2d-game.exe`, cross-compiled with MinGW-w64; default
//! `target/x86_64-pc-windows-gnu/release`), `--macos-dir DIR` (universal `rustpg2d-editor` and
//! `rustpg2d-game`, built on a Mac and joined with `lipo`; default `target/macos`).
//! `--require-web`, `--require-windows`, `--require-macos`: fail without them (CI).

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rustpg2d_packaging::{AppSpec, Entry, Source};

use crate::{read, root, workspace_version};

/// The oldest glibc the packaged binaries may need (Debian 12, Ubuntu 22.04, RHEL 9).
const MAX_GLIBC: (u32, u32) = (2, 35);
const APP_ID: &str = "io.github.Shadowrom2020.rustpg2d";
/// The macOS application's folder name.
pub const MAC_APP: &str = "R(ust)PG2D.app";

fn is_mach_o_universal(b: &[u8]) -> bool {
    b.starts_with(&[0xca, 0xfe, 0xba, 0xbe])
}

pub fn dist(args: &[String]) -> Result<()> {
    let root = root();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let flag = |name: &str| args.iter().any(|a| a == name);
    let out = opt("--out").unwrap_or_else(|| root.join("dist"));
    let bins = opt("--bin-dir").unwrap_or_else(|| root.join("target/release"));
    let version = workspace_version(&read(&root.join("Cargo.toml"))?)?;
    let (editor, game) = (bins.join("rustpg2d-editor"), bins.join("rustpg2d-game"));
    for b in [&editor, &game] {
        let bytes = std::fs::read(b).with_context(|| format!("{} not found: run `cargo build --release` first", b.display()))?;
        match rustpg2d_packaging::glibc_needed(&bytes) {
            Some(g) if g > MAX_GLIBC => bail!("{} needs glibc {}.{} (at most {}.{} allowed): build on an older system", b.display(), g.0, g.1, MAX_GLIBC.0, MAX_GLIBC.1),
            Some(g) => println!("{}: needs glibc {}.{}", b.display(), g.0, g.1),
            None => println!("{}: no glibc version information", b.display()),
        }
    }
    // The web game.
    let wasm = opt("--web-wasm").unwrap_or_else(|| root.join("target/wasm32-unknown-unknown/release/rustpg2d-game.wasm"));
    let wasm = if wasm.is_file() {
        if !std::fs::read(&wasm)?.starts_with(b"\0asm") {
            bail!("{} is not a WebAssembly file", wasm.display());
        }
        println!("{}: the web game", wasm.display());
        Some(wasm)
    } else if flag("--require-web") {
        bail!("the web game is missing: cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game");
    } else {
        println!("no web game ({}): the packaged editors won't export web games", wasm.display());
        None
    };
    // Windows: editor and game.
    let win_dir = opt("--windows-dir").unwrap_or_else(|| root.join("target/x86_64-pc-windows-gnu/release"));
    let (win_editor, win_game) = (win_dir.join("rustpg2d-editor.exe"), win_dir.join("rustpg2d-game.exe"));
    let windows = if win_editor.is_file() && win_game.is_file() {
        for w in [&win_editor, &win_game] {
            if !std::fs::read(w)?.starts_with(b"MZ") {
                bail!("{} is not a Windows program", w.display());
            }
        }
        println!("{}: the Windows build", win_dir.display());
        true
    } else if flag("--require-windows") {
        bail!("the Windows build is missing in {}: cargo build --release --target x86_64-pc-windows-gnu -p rustpg2d-editor -p rustpg2d-game", win_dir.display());
    } else {
        println!("no Windows build in {}: no Windows zip, and the other editors won't export Windows games", win_dir.display());
        false
    };
    // macOS: universal editor and game.
    let mac_dir = opt("--macos-dir").unwrap_or_else(|| root.join("target/macos"));
    let (mac_editor, mac_game) = (mac_dir.join("rustpg2d-editor"), mac_dir.join("rustpg2d-game"));
    let macos = if mac_editor.is_file() && mac_game.is_file() {
        for m in [&mac_editor, &mac_game] {
            if !is_mach_o_universal(&std::fs::read(m)?) {
                bail!("{} is not a universal macOS program (lipo -create the aarch64 and x86_64 builds)", m.display());
            }
        }
        println!("{}: the macOS build", mac_dir.display());
        true
    } else if flag("--require-macos") {
        bail!("the macOS build is missing in {}: universal rustpg2d-editor and rustpg2d-game, built on a Mac (README: Building on macOS)", mac_dir.display());
    } else {
        println!("no macOS build in {}: no macOS app, and the other editors won't export macOS games", mac_dir.display());
        false
    };
    let example = root.join("example_project");
    let project = rustpg2d_engine::project::Project::load(&example).context("loading example_project")?;
    let icon = rustpg2d_packaging::hero_icon(&project).unwrap_or_else(rustpg2d_packaging::emblem).encode()?;
    let license = root.join("LICENSE");
    if !license.is_file() {
        bail!("LICENSE is missing: the packages must carry R(ust)PG2D's license");
    }

    // Linux: AppImage and Flatpak; the foreign runners next to the editor.
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
            Entry { dest: "lib/rustpg2d/rustpg2d-game".into(), src: Source::Path(game.clone()), exec: true },
        ],
        bin: vec![("rustpg2d-editor".into(), "lib/rustpg2d/rustpg2d-editor".into()), ("rustpg2d-game".into(), "lib/rustpg2d/rustpg2d-game".into())],
        icon: icon.clone(),
        license: "MIT".into(),
    };
    // Next to the editor, where File > Export game looks for them.
    if let Some(w) = &wasm {
        spec.files.push(Entry { dest: "lib/rustpg2d/rustpg2d-game.wasm".into(), src: Source::Path(w.clone()), exec: false });
    }
    if windows {
        spec.files.push(Entry { dest: "lib/rustpg2d/rustpg2d-game.exe".into(), src: Source::Path(win_game.clone()), exec: false });
    }
    if macos {
        spec.files.push(Entry { dest: "lib/rustpg2d/rustpg2d-game-macos".into(), src: Source::Path(mac_game.clone()), exec: false });
    }
    spec.files.push(Entry { dest: format!("share/licenses/{APP_ID}/LICENSE"), src: Source::Path(license.clone()), exec: false });
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
    let examples = example_files(&example)?;

    // Windows: one zip.
    if windows {
        let zip = out.join(format!("rustpg2d-{version}-windows-x86_64.zip"));
        let mut files: Vec<(String, Vec<u8>, u32)> = vec![
            ("rustpg2d-editor.exe".into(), std::fs::read(&win_editor)?, 0o755),
            ("rustpg2d-game.exe".into(), std::fs::read(&win_game)?, 0o755),
            ("rustpg2d-game-linux".into(), std::fs::read(&game)?, 0o755),
            ("LICENSE.txt".into(), read(&license)?.replace('\n', "\r\n").into_bytes(), 0o644),
            ("README.txt".into(), WINDOWS_README.replace("{version}", &version).replace('\n', "\r\n").into_bytes(), 0o644),
        ];
        if macos {
            files.push(("rustpg2d-game-macos".into(), std::fs::read(&mac_game)?, 0o644));
        }
        if let Some(w) = &wasm {
            files.push(("rustpg2d-game.wasm".into(), std::fs::read(w)?, 0o644));
        }
        for (rel, path) in &examples {
            files.push((format!("example_project/{rel}"), std::fs::read(path)?, 0o644));
        }
        write_zip(&zip, &format!("rustpg2d-{version}"), &files)?;
        println!("{}", zip.display());
        made.push(zip);
    }

    // macOS: the application bundle, signed and zipped on a Mac.
    if macos {
        let mac_out = out.join("macos");
        let app = mac_out.join(MAC_APP);
        if app.exists() {
            std::fs::remove_dir_all(&app)?;
        }
        let contents = app.join("Contents");
        let res = contents.join("Resources");
        let put = |dest: &Path, bytes: &[u8], exec: bool| -> Result<()> {
            std::fs::create_dir_all(dest.parent().unwrap())?;
            std::fs::write(dest, bytes)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dest, std::fs::Permissions::from_mode(if exec { 0o755 } else { 0o644 }))?;
            }
            #[cfg(not(unix))]
            let _ = exec;
            Ok(())
        };
        // Code in MacOS (signed as part of the app), everything else in Resources.
        put(&contents.join("MacOS/rustpg2d-editor"), &std::fs::read(&mac_editor)?, true)?;
        put(&contents.join("MacOS/rustpg2d-game"), &std::fs::read(&mac_game)?, true)?;
        put(&contents.join("Info.plist"), mac_info_plist(&version).as_bytes(), false)?;
        put(&contents.join("PkgInfo"), b"APPL????", false)?;
        put(&res.join("rustpg2d.icns"), &rustpg2d_engine::release::icns(&icon)?, false)?;
        put(&res.join("rustpg2d-game-linux"), &std::fs::read(&game)?, false)?;
        if windows {
            put(&res.join("rustpg2d-game.exe"), &std::fs::read(&win_game)?, false)?;
        }
        if let Some(w) = &wasm {
            put(&res.join("rustpg2d-game.wasm"), &std::fs::read(w)?, false)?;
        }
        put(&res.join("LICENSE"), &std::fs::read(&license)?, false)?;
        for (rel, path) in &examples {
            put(&res.join("example_project").join(rel), &std::fs::read(path)?, false)?;
        }
        // A tar keeps the file modes through CI's artifacts (which drop them).
        let tar = mac_out.join("macos-app.tar");
        let status = std::process::Command::new("tar").arg("-cf").arg(&tar).arg("-C").arg(&mac_out).arg(MAC_APP).status().context("running tar")?;
        if !status.success() {
            bail!("tar failed for {}", app.display());
        }
        println!("{} (+ .tar; sign and zip it on a Mac)", app.display());
    }

    let mut sums = String::new();
    for p in &made {
        sums += &format!("{}  {}\n", rustpg2d_packaging::sha256_hex(&std::fs::read(p)?), p.file_name().unwrap().to_string_lossy());
    }
    std::fs::write(out.join("SHA256SUMS"), sums)?;
    Ok(())
}

/// The example project's files (relative `/` path, file) without the developer's save games.
fn example_files(example: &Path) -> Result<Vec<(String, PathBuf)>> {
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
                let rel = p.strip_prefix(example)?.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                files.push((rel, p));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// A zip with everything under `top/`, Unix modes kept (the Linux runner stays executable).
fn write_zip(path: &Path, top: &str, files: &[(String, Vec<u8>, u32)]) -> Result<()> {
    let mut zw = zip::ZipWriter::new(std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).system(zip::System::Unix).large_file(true).last_modified_time(rustpg2d_engine::release::zip_time());
    for (name, bytes, mode) in files {
        zw.start_file(format!("{top}/{name}"), opts.unix_permissions(*mode))?;
        zw.write_all(bytes)?;
    }
    zw.finish()?;
    Ok(())
}

/// `Contents/Info.plist` of the macOS application.
fn mac_info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>R(ust)PG2D</string>
    <key>CFBundleExecutable</key>
    <string>rustpg2d-editor</string>
    <key>CFBundleIconFile</key>
    <string>rustpg2d.icns</string>
    <key>CFBundleIdentifier</key>
    <string>{APP_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>R(ust)PG2D</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#
    )
}

const WINDOWS_README: &str = "R(ust)PG2D {version} for Windows (10 / 11, 64-bit)

Start rustpg2d-editor.exe: the welcome screen opens the example project or one of your own.
F5 in the editor play-tests the open map with rustpg2d-game.exe. Both need a graphics driver
with OpenGL 3 (in a virtual machine: turn on 3D acceleration).

R(ust)PG2D isn't code-signed: the first start shows \"Windows protected your PC\". Choose
\"More info\", then \"Run anyway\".

Your files:
- the example project you open is copied to %USERPROFILE%\\rustpg2d\\example_project
- the asset library: %LOCALAPPDATA%\\rustpg2d\\library
- settings and the recent projects: %APPDATA%\\rustpg2d

File > Export game makes games for Windows (rustpg2d-game.exe), Linux (rustpg2d-game-linux),
macOS (rustpg2d-game-macos) and browsers (rustpg2d-game.wasm), all in this folder. AppImage and
Flatpak packages are made by R(ust)PG2D on Linux.

Documentation and source: https://github.com/Shadowrom2020/rustpg2d
R(ust)PG2D is MIT licensed: see LICENSE.txt.
";
