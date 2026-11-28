//! Exporting the game (the Publishing page): a standalone copy for Linux, Windows and / or
//! macOS (an application bundle), on request a zip of each, an AppImage and a Flatpak bundle
//! (`package.rs`, on Linux), and the web version. Before that the pre-flight: what stands in
//! the way of an export, and what a creator should know of before giving the game away.

use std::path::PathBuf;

use rustpg2d_engine::release::{self, Platform};

use crate::package;
use crate::workspace::Workspace;

/// What an export produces besides the game folder.
#[derive(Clone, Debug)]
pub struct ExportOpts {
    /// Bundles for these platforms.
    pub linux: bool,
    pub windows: bool,
    pub macos: bool,
    /// The web version (WebAssembly + `data.pack` + `index.html`).
    pub web: bool,
    pub zip: bool,
    pub appimage: bool,
    pub flatpak: bool,
    /// Flatpak application id.
    pub app_id: String,
    /// Flatpak runtime branch (`package::FLATPAK_RUNTIMES`).
    pub runtime: String,
    /// Icon PNG of the packages and the macOS app ("" = the player character).
    pub icon: String,
    /// The game's files in one `data.pack` (else a `data` folder of loose files).
    pub pack: bool,
}

impl ExportOpts {
    /// The platforms ticked.
    pub fn platforms(&self) -> Vec<Platform> {
        Platform::ALL
            .into_iter()
            .filter(|p| match p {
                Platform::Linux => self.linux,
                Platform::Windows => self.windows,
                Platform::MacOS => self.macos,
            })
            .collect()
    }
}

impl Default for ExportOpts {
    fn default() -> Self {
        let host = Platform::host();
        ExportOpts {
            linux: host == Platform::Linux,
            windows: host == Platform::Windows,
            macos: host == Platform::MacOS,
            web: false,
            zip: false,
            appimage: false,
            flatpak: false,
            app_id: String::new(),
            runtime: package::FLATPAK_RUNTIMES[0].into(),
            icon: String::new(),
            pack: true,
        }
    }
}

/// The oldest glibc the game binary runs on, as a note for the dialog (None = unknown).
pub fn glibc_note(bin: &std::path::Path) -> Option<(String, bool)> {
    let (maj, min) = package::glibc_needed(&std::fs::read(bin).ok()?)?;
    // 2.34: Debian 12, Ubuntu 22.04, Fedora 35, RHEL 9 and newer.
    let wide = (maj, min) <= (2, 34);
    let text = if wide {
        format!("Runs on Linux with glibc {maj}.{min} or newer (Debian 12, Ubuntu 22.04, Fedora 35, RHEL 9 and later), and in Flatpak.")
    } else {
        format!("This game executable needs glibc {maj}.{min}: older distributions (and Flatpak runtimes with an older glibc) can't start it. Rebuild rustpg2d-game from this R(ust)PG2D version.")
    };
    Some((text, wide))
}

/// Cargo's target triple of a platform's game runner (cross builds: `target/<triple>/release`).
fn triple(platform: Platform) -> &'static str {
    match platform {
        Platform::Linux => "x86_64-unknown-linux-gnu",
        Platform::Windows => "x86_64-pc-windows-gnu",
        // lipo of the aarch64 and x86_64 builds (made on a Mac; CI's macOS job).
        Platform::MacOS => "universal-apple-darwin",
    }
}

/// The folders a packaged R(ust)PG2D keeps game runners in: next to the editor, and in a macOS
/// app also `Contents/Resources` (only Mach-O code may sit next to the editor in `Contents/MacOS`).
fn runner_dirs(dir: &std::path::Path) -> [PathBuf; 2] {
    [dir.to_path_buf(), dir.join("..").join("Resources")]
}

/// R(ust)PG2D's game runner for `platform` to ship (`Platform::game_runner`): the one a packaged
/// R(ust)PG2D ships next to the editor (all of them); inside a cargo target folder a release
/// build if there is one (also a cross build in `target/<triple>/release`), else the debug
/// build. Returns (path, is a debug build).
pub fn game_binary(platform: Platform) -> Option<(PathBuf, bool)> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = platform.game_runner();
    let in_target = dir.file_name().is_some_and(|n| n == "debug" || n == "release");
    if !in_target {
        return runner_dirs(dir).into_iter().map(|d| d.join(name)).find(|p| p.is_file()).map(|p| (p, false));
    }
    let target = dir.parent()?;
    // A cross build keeps cargo's name (`rustpg2d-game`, `.exe`).
    let cargo_name = if platform == Platform::Windows { "rustpg2d-game.exe" } else { "rustpg2d-game" };
    for rel in [target.join("release").join(name), target.join(triple(platform)).join("release").join(cargo_name)] {
        if rel.is_file() {
            return Some((rel, false));
        }
    }
    let debug = dir.file_name().is_some_and(|n| n == "debug");
    let next_to = dir.join(name);
    next_to.is_file().then_some((next_to, debug))
}

/// The game runner compiled for the web (`rustpg2d-game.wasm`): next to the editor (packaged
/// R(ust)PG2D ships it), else in the cargo target folder
/// (`cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game`).
pub fn web_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    if let Some(p) = runner_dirs(dir).into_iter().map(|d| d.join(release::WEB_WASM)).find(|p| p.is_file()) {
        return Some(p);
    }
    let in_target = dir.file_name().is_some_and(|n| n == "debug" || n == "release");
    let built = dir.parent()?.join("wasm32-unknown-unknown/release").join(release::WEB_WASM);
    (in_target && built.is_file()).then_some(built)
}

pub const NO_WEB_RUNNER: &str = "rustpg2d-game.wasm (the web game runner) was not found next to the editor. Packaged R(ust)PG2D includes it; from source: `rustup target add wasm32-unknown-unknown`, then `cargo build --release --target wasm32-unknown-unknown -p rustpg2d-game`.";

/// How to get a missing game runner (the dialog and the error).
pub fn missing_runner(platform: Platform) -> String {
    match platform {
        Platform::Windows if Platform::host() != Platform::Windows => {
            "rustpg2d-game.exe (the Windows game runner) was not found next to the editor. Packaged R(ust)PG2D includes it; from source: `cargo build --release --target x86_64-pc-windows-gnu -p rustpg2d-game` (needs the MinGW-w64 cross compiler).".into()
        }
        Platform::MacOS if Platform::host() != Platform::MacOS => {
            "rustpg2d-game-macos (the macOS game runner, a universal binary) was not found next to the editor. Packaged R(ust)PG2D includes it; from source it has to be built on a Mac (see README, Exporting for macOS) and copied to target/release/rustpg2d-game-macos.".into()
        }
        Platform::Linux if Platform::host() != Platform::Linux => {
            "rustpg2d-game-linux (the Linux game runner) was not found next to the editor. The R(ust)PG2D downloads for Windows and macOS include it; from source: build rustpg2d-game on Linux and copy it to target/release/rustpg2d-game-linux.".into()
        }
        p => format!("{} was not found next to the editor. Build it with `cargo build --release -p rustpg2d-game`.", p.game_runner()),
    }
}


/// The engine build a game runner was made from (`release::runner_build`), read once per file
/// as it is now (a runner is big; the pre-flight asks often).
fn runner_tag(path: &std::path::Path) -> Option<String> {
    type Seen = std::collections::HashMap<PathBuf, (Option<std::time::SystemTime>, u64, Option<String>)>;
    static SEEN: std::sync::Mutex<Option<Seen>> = std::sync::Mutex::new(None);
    let meta = std::fs::metadata(path).ok()?;
    let now = (meta.modified().ok(), meta.len());
    let mut seen = SEEN.lock().ok()?;
    let seen = seen.get_or_insert_with(Default::default);
    if let Some((time, len, tag)) = seen.get(path) {
        if (*time, *len) == now {
            return tag.clone();
        }
    }
    let tag = std::fs::read(path).ok().and_then(|bytes| release::runner_build(&bytes).map(str::to_string));
    seen.insert(path.to_path_buf(), (now.0, now.1, tag.clone()));
    tag
}

/// Whether the game runner `exe` was built from this editor's engine (it prints its build when
/// asked with `--engine-build`): a runner of another engine may not read the maps and definitions
/// this editor writes. The Play tab's *Own window* refuses such a runner; the agents' `play_test`
/// runs it with this sentence as a warning.
pub fn runner_check(exe: &std::path::Path) -> Result<(), String> {
    runner_check_within(exe, std::time::Duration::from_secs(5))
}

fn runner_check_within(exe: &std::path::Path, wait: std::time::Duration) -> Result<(), String> {
    use std::io::Read;
    if !exe.is_file() {
        return Err(format!("The game runner {} was not found: build it (cargo build -p rustpg2d-game).", exe.display()));
    }
    // (Its name, not its whole path: the sentence goes into the editor's message line.)
    let name = exe.file_name().map_or_else(|| exe.display().to_string(), |n| n.to_string_lossy().into_owned());
    let differs = || format!("The game runner {name} was built from another engine version than this editor and may not read this project: rebuild it (cargo build -p rustpg2d-game).");
    let mut child = std::process::Command::new(exe)
        .arg("--engine-build")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("The game runner {} could not be started: {e}", exe.display()))?;
    // (A runner too old to know `--engine-build` would open its window and stay: it is stopped.)
    let until = std::time::Instant::now() + wait;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(differs());
            }
        }
    }
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if out.trim() == rustpg2d_engine::ENGINE_BUILD {
        Ok(())
    } else {
        Err(differs())
    }
}

/// What the Publishing page says before an export.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preflight {
    /// What stands in the way: nothing is exported while one of these is there.
    pub stops: Vec<String>,
    /// Worth knowing; the export goes through.
    pub notes: Vec<String>,
    /// What will be written where, for what was ticked.
    pub writes: Vec<String>,
}

impl Workspace {
    /// The head and shoulders of the project's player character, rendered for the game's icon
    /// (`None` without a player character, a model to draw, or a graphics adapter).
    pub fn model_icon(&self) -> Option<rustpg2d_engine::imageio::Rgba> {
        if self.gpu == crate::Gpu::No {
            return None;
        }
        let def = self.project.db.character(&self.project.config.player)?;
        let img = self.assets.models.bust_image(&self.project.db, &def.appearance)?;
        let rgba = rustpg2d_engine::imageio::Rgba { w: img.width as u32, h: img.height as u32, px: img.bytes };
        rustpg2d_engine::icon::fit_icon(&rgba)
    }

    /// Where exports go unless the creator says otherwise: `releases` beside the project file
    /// (or folder), never in its working copy.
    pub fn default_export_dir(&self) -> PathBuf {
        self.project_parent().map(|p| p.join("releases")).unwrap_or_else(|| PathBuf::from("releases"))
    }

    /// The export's options as a creator starts with them: this computer's platform, a zip, the
    /// app id made from the game's name.
    pub fn default_export_opts(&self) -> ExportOpts {
        ExportOpts { zip: true, app_id: package::default_app_id(&release::slug(&self.project.config.name)), ..Default::default() }
    }

    /// What stands in the way of exporting the game as `name` into `out` with `opts`, what is
    /// worth knowing, and what would be written.
    pub fn export_preflight(&self, name: &str, out: &str, opts: &ExportOpts) -> Preflight {
        let mut p = Preflight::default();
        let (_, check) = release::check(&self.project);
        p.stops.extend(check.errors);
        p.notes.extend(check.warnings);
        if name.trim().is_empty() {
            p.stops.push("The game needs a name.".into());
        }
        if out.trim().is_empty() {
            p.stops.push("Say where the export goes (a folder).".into());
        }
        let platforms = opts.platforms();
        if platforms.is_empty() && !opts.web {
            p.stops.push("Choose at least one platform (Linux, Windows, macOS, Web).".into());
        }
        let slug = release::slug(name);
        let out = out.trim().trim_end_matches(['/', '\\']);
        let sep = std::path::MAIN_SEPARATOR;
        for platform in &platforms {
            // A runner made from another engine than this editor couldn't read what it writes.
            if let Some((b, _)) = game_binary(*platform) {
                let tag = runner_tag(&b);
                if tag.as_deref() != Some(rustpg2d_engine::ENGINE_BUILD) {
                    p.stops.push(format!(
                        "{} was built from {} than this editor, so the exported game might not read this project. Rebuild it from the editor's sources (`cargo build --release -p rustpg2d-game`), or use the runner that came with this R(ust)PG2D.",
                        b.display(),
                        if tag.is_some() { "another engine" } else { "an older engine" }
                    ));
                }
            }
            match game_binary(*platform) {
                Some((_, false)) => {}
                Some((b, true)) => p.notes.push(format!("Only a debug build of the game was found ({}). It works but runs slower; `cargo build --release -p rustpg2d-game` gives a fast one.", b.display())),
                None => p.stops.push(missing_runner(*platform)),
            }
            p.writes.push(match platform {
                Platform::Linux => format!("Linux: {out}{sep}{slug}{sep}{slug} (run it to play); saves go to {}", Platform::Linux.saves_hint(&slug)),
                Platform::Windows => format!("Windows: {out}{sep}{}{sep}{slug}.exe; saves go to {}", Platform::Windows.folder(&slug), Platform::Windows.saves_hint(&slug)),
                Platform::MacOS => format!("macOS: {out}{sep}{}{sep}{}; saves go to {}", Platform::MacOS.folder(&slug), release::app_bundle(name, &slug), Platform::MacOS.saves_hint(&slug)),
            });
        }
        // Packs whose makers ask to be named, and aren't in the game's credits.
        let unnamed = self.credits_missing();
        if !unnamed.is_empty() {
            p.notes.push(format!("The game's credits don't hold: {}. The packs this game uses ask to be named (Packs in this game, above).", unnamed.join("; ")));
        }
        if opts.web {
            if web_binary().is_none() {
                p.stops.push(NO_WEB_RUNNER.into());
            }
            p.writes.push(format!("Web: {out}{sep}{slug}-web{sep}index.html (serve the folder, or upload {slug}-web.zip to itch.io); saves stay in the player's browser"));
        }
        if opts.appimage {
            p.writes.push(format!("AppImage: {out}{sep}{slug}-x86_64.AppImage"));
        }
        if opts.flatpak {
            if let Err(e) = package::check_app_id(opts.app_id.trim()) {
                p.stops.push(format!("The Flatpak app id {e}"));
            }
            if !package::flatpak_available() {
                p.stops.push("Flatpak bundles need the flatpak command (install Flatpak with your package manager).".into());
            }
            p.writes.push(format!("Flatpak: {out}{sep}{slug}.flatpak (saves in ~/.var/app/{}/data/{slug}/saves)", opts.app_id.trim()));
        }
        if opts.linux {
            if let Some((note, wide)) = game_binary(Platform::Linux).and_then(|(b, _)| glibc_note(&b)) {
                if !wide {
                    p.notes.push(note);
                }
            }
        }
        // What the project check found, when it was run.
        let (errors, warnings) = self.problems.counts();
        if errors > 0 {
            p.notes.push(format!("The project check lists {errors} error{} ({warnings} warning{}): see Problems.", if errors == 1 { "" } else { "s" }, if warnings == 1 { "" } else { "s" }));
        } else if self.problems.status() == "-" {
            p.notes.push("The project wasn't checked yet: Check the project (Problems) looks for what players would run into.".into());
        }
        p
    }

    /// Saves everything, then exports the game as `name` into the folder `out` (the folders,
    /// then the packages asked for). Returns what was written, a line each.
    pub fn export_game(&mut self, name: &str, out: &str, opts: &ExportOpts) -> Result<String, String> {
        self.save_all();
        if self.is_dirty() {
            return Err("Unsaved changes could not be saved; nothing was exported.".into());
        }
        let platforms = opts.platforms();
        if platforms.is_empty() && !opts.web {
            return Err("Choose at least one platform (Linux, Windows, macOS, Web).".into());
        }
        let mut bins = Vec::new();
        for p in &platforms {
            bins.push(game_binary(*p).ok_or_else(|| missing_runner(*p))?);
        }
        let out_dir = std::path::Path::new(out);
        let mut lines = Vec::new();
        let mut linux_folder = None;
        let mut warnings = Vec::new();
        let mut debug = false;
        // The game's icon: the packages, the macOS app and (as icon.png in the data) the game's
        // window show it.
        let icon = package::icon(&self.project, &opts.icon, &|| self.model_icon());
        if let Err(e) = &icon {
            lines.push(format!("Exported without an icon: {e:#}"));
        }
        for (p, (bin, dbg)) in platforms.iter().zip(&bins) {
            let r = release::export(&self.project, name, *p, bin, out_dir, opts.zip, icon.as_deref().ok(), opts.pack).map_err(|e| format!("{}: {e:#}", p.label()))?;
            lines.push(format!("{} export: {} ({} files, {:.1} MB)", p.label(), r.folder.display(), r.files, r.bytes as f64 / 1_048_576.0));
            if let Some(a) = &r.archive {
                lines.push(format!("Archive: {}", a.display()));
            }
            if *p == Platform::Linux {
                linux_folder = Some(r.folder.clone());
            }
            debug |= dbg;
            warnings = r.warnings;
        }
        if opts.web {
            let wasm = web_binary().ok_or(NO_WEB_RUNNER)?;
            let r = release::export_web(&self.project, name, &wasm, out_dir, opts.zip).map_err(|e| format!("Web: {e:#}"))?;
            lines.push(format!("Web export: {} ({} files, {:.1} MB)", r.folder.display(), r.files, r.bytes as f64 / 1_048_576.0));
            if let Some(a) = &r.archive {
                lines.push(format!("Archive (itch.io): {}", a.display()));
            }
            warnings = r.warnings;
        }
        let mut msg = lines.join("\n");
        if let (Some(folder), true) = (&linux_folder, opts.appimage || opts.flatpak) {
            let r = release::Exported { folder: folder.clone(), archive: None, files: 0, bytes: 0, warnings: vec![] };
            let slug = release::slug(name);
            let size = |p: &std::path::Path| std::fs::metadata(p).map_or(0.0, |m| m.len() as f64 / 1_048_576.0);
            match &icon {
                Err(e) => msg += &format!("\nNo packages: {e:#}"),
                Ok(icon) => {
                    if opts.appimage {
                        match package::appimage(&r.folder, &slug, name.trim(), icon, out_dir) {
                            Ok(p) => msg += &format!("\nAppImage: {} ({:.1} MB)", p.display(), size(&p)),
                            Err(e) => msg += &format!("\nAppImage failed: {e:#}"),
                        }
                    }
                    if opts.flatpak {
                        match package::flatpak(&r.folder, &slug, name.trim(), opts.app_id.trim(), &opts.runtime, icon, out_dir) {
                            Ok(p) => msg += &format!("\nFlatpak: {} ({:.1} MB; install with: flatpak install --user {})", p.display(), size(&p), p.display()),
                            Err(e) => msg += &format!("\nFlatpak failed: {e:#}"),
                        }
                    }
                }
            }
        }
        if debug {
            msg += "\nNote: this used a debug build of the game. For full speed run `cargo build --release -p rustpg2d-game` and export again.";
        }
        for w in warnings {
            msg += &format!("\nWarning: {w}");
        }
        Ok(msg)
    }

}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A stand-in for `rustpg2d-game`: a shell script that answers `--engine-build` with `body`.
    fn fake_runner(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// The check, tried again while another test thread's fork still holds the new script open
    /// (`Text file busy`).
    fn check(exe: &std::path::Path, wait: Duration) -> Result<(), String> {
        for _ in 0..20 {
            match runner_check_within(exe, wait) {
                Err(e) if e.contains("could not be started") => std::thread::sleep(Duration::from_millis(50)),
                r => return r,
            }
        }
        runner_check_within(exe, wait)
    }

    #[test]
    fn a_runner_of_another_engine_is_refused() {
        let dir = std::env::temp_dir().join(format!("rustpg2d-runner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let same = fake_runner(&dir, "same", &format!("echo {}", rustpg2d_engine::ENGINE_BUILD));
        assert_eq!(check(&same, Duration::from_secs(5)), Ok(()));
        let other = fake_runner(&dir, "other", "echo 0123456789abcdef");
        let e = check(&other, Duration::from_secs(5)).unwrap_err();
        assert!(e.contains("another engine version") && e.contains("cargo build -p rustpg2d-game"), "{e}");
        // (Too old to know the word: it doesn't answer, and is stopped.)
        let old = fake_runner(&dir, "old", "exec sleep 30");
        assert!(check(&old, Duration::from_millis(300)).unwrap_err().contains("another engine version"));
        assert!(check(&dir.join("missing"), Duration::from_secs(5)).unwrap_err().contains("was not found"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
