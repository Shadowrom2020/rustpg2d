//! `cargo xtask-dist`: the R(ust)PG2D toolkit as AppImage and Flatpak (feature `dist`).
//!
//! Options: `--out DIR` (default `dist/`), `--bin-dir DIR` (release binaries, default
//! `target/release`), `--web-wasm FILE` (the game for browsers, shipped next to the editor so it
//! exports web games; default `target/wasm32-unknown-unknown/release/rustpg2d-game.wasm` if it
//! exists), `--require-web` (fail without it: CI).

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
    for b in [&editor, &game] {
        let bytes = std::fs::read(b).with_context(|| format!("{} not found: run `cargo build --release` first", b.display()))?;
        match rustpg2d_packaging::glibc_needed(&bytes) {
            Some(g) if g > MAX_GLIBC => bail!("{} needs glibc {}.{} (at most {}.{} allowed): build on an older system", b.display(), g.0, g.1, MAX_GLIBC.0, MAX_GLIBC.1),
            Some(g) => println!("{}: needs glibc {}.{}", b.display(), g.0, g.1),
            None => println!("{}: no glibc version information", b.display()),
        }
    }
    let wasm = opt("--web-wasm").or_else(|| Some(root.join("target/wasm32-unknown-unknown/release/rustpg2d-game.wasm")).filter(|p| p.is_file()));
    match &wasm {
        Some(w) => {
            if !std::fs::read(w).with_context(|| format!("{} not found", w.display()))?.starts_with(b"\0asm") {
                bail!("{} is not a WebAssembly file", w.display());
            }
            println!("{}: the web game", w.display());
        }
        None if args.iter().any(|a| a == "--require-web") => bail!("the web game is missing: cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game"),
        None => println!("no web game (target/wasm32-unknown-unknown/release/rustpg2d-game.wasm): the packaged editor won't export web games"),
    }
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
    if let Some(w) = wasm {
        // Next to the editor, where File > Export game looks for it.
        spec.files.push(Entry { dest: "lib/rustpg2d/rustpg2d-game.wasm".into(), src: Source::Path(w), exec: false });
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
    let mut sums = String::new();
    for p in [&appimage, &flatpak] {
        sums += &format!("{}  {}\n", rustpg2d_packaging::sha256_hex(&std::fs::read(p)?), p.file_name().unwrap().to_string_lossy());
    }
    std::fs::write(out.join("SHA256SUMS"), sums)?;
    Ok(())
}

