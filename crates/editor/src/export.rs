//! File → Export game: writes a standalone bundle (game executable + project data) for Linux
//! and / or Windows, and on request a zip of each, an AppImage and a Flatpak bundle (package.rs,
//! Linux only).

use std::path::PathBuf;

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::release::{self, Platform};

use crate::app::Editor;
use crate::files::Purpose;
use crate::package;

pub struct ExportDialog {
    pub name: String,
    pub out: String,
    pub opts: ExportOpts,
    pub result: Option<Result<String, String>>,
    /// The `flatpak` command is installed (checked when the dialog opens).
    flatpak_ok: bool,
    /// The app id was edited by hand (else it follows the game name).
    app_id_edited: bool,
    /// Which glibc the game executable needs (read when the dialog opens).
    glibc: Option<(String, bool)>,
}

/// What an export produces besides the game folder.
#[derive(Clone, Debug)]
pub struct ExportOpts {
    /// Bundles for these platforms.
    pub linux: bool,
    pub windows: bool,
    pub zip: bool,
    pub appimage: bool,
    pub flatpak: bool,
    /// Flatpak application id.
    pub app_id: String,
    /// Flatpak runtime branch (`package::FLATPAK_RUNTIMES`).
    pub runtime: String,
    /// Icon PNG of the packages ("" = the player character).
    pub icon: String,
}

impl ExportOpts {
    /// The platforms ticked.
    pub fn platforms(&self) -> Vec<Platform> {
        Platform::ALL
            .into_iter()
            .filter(|p| match p {
                Platform::Linux => self.linux,
                Platform::Windows => self.windows,
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
            zip: false,
            appimage: false,
            flatpak: false,
            app_id: String::new(),
            runtime: package::FLATPAK_RUNTIMES[0].into(),
            icon: String::new(),
        }
    }
}

/// The oldest glibc the game binary runs on, as a note for the dialog (None = unknown).
fn glibc_note(bin: &std::path::Path) -> Option<(String, bool)> {
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
    }
}

/// R(ust)PG2D's game runner for `platform` to ship (`rustpg2d-game`, `rustpg2d-game.exe`): the
/// one next to the editor (packaged R(ust)PG2D ships both); inside a cargo target folder a
/// release build if there is one (also a cross build in `target/<triple>/release`), else the
/// debug build. Returns (path, is a debug build).
pub fn game_binary(platform: Platform) -> Option<(PathBuf, bool)> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = platform.game_runner();
    let next_to = dir.join(name);
    let in_target = dir.file_name().is_some_and(|n| n == "debug" || n == "release");
    if !in_target {
        return next_to.is_file().then_some((next_to, false));
    }
    let target = dir.parent()?;
    for rel in [target.join("release").join(name), target.join(triple(platform)).join("release").join(name)] {
        if rel.is_file() {
            return Some((rel, false));
        }
    }
    let debug = dir.file_name().is_some_and(|n| n == "debug");
    next_to.is_file().then_some((next_to, debug))
}

/// How to get a missing game runner (the dialog and the error).
fn missing_runner(platform: Platform) -> String {
    match platform {
        Platform::Windows if Platform::host() != Platform::Windows => {
            "rustpg2d-game.exe (the Windows game runner) was not found next to the editor. Packaged R(ust)PG2D includes it; from source: `cargo build --release --target x86_64-pc-windows-gnu -p rustpg2d-game` (needs the MinGW-w64 cross compiler).".into()
        }
        Platform::Linux if Platform::host() != Platform::Linux => "rustpg2d-game (the Linux game runner) was not found next to the editor; the R(ust)PG2D download for Windows includes it.".into(),
        p => format!("{} was not found next to the editor. Build it with `cargo build --release -p rustpg2d-game`.", p.game_runner()),
    }
}

impl Editor {
    pub fn open_export(&mut self) {
        let out = self.project.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.join("releases"))).unwrap_or_else(|| PathBuf::from("releases"));
        let name = self.project.config.name.clone();
        let opts = ExportOpts { zip: true, app_id: package::default_app_id(&release::slug(&name)), ..Default::default() };
        self.export = Some(ExportDialog {
            name,
            out: out.display().to_string(),
            opts,
            result: None,
            flatpak_ok: package::flatpak_available(),
            app_id_edited: false,
            glibc: game_binary(Platform::Linux).and_then(|(p, _)| glibc_note(&p)),
        });
    }

    /// Saves everything, then exports (the folder, then the packages asked for). Returns a
    /// summary.
    pub fn run_export(&mut self, name: &str, out: &str, opts: &ExportOpts) -> Result<String, String> {
        self.save_all();
        if self.doc.as_ref().is_some_and(|d| d.is_dirty()) || self.defs_dirty {
            return Err("Unsaved changes could not be saved; nothing was exported.".into());
        }
        let platforms = opts.platforms();
        if platforms.is_empty() {
            return Err("Choose at least one platform (Linux, Windows).".into());
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
        for (p, (bin, dbg)) in platforms.iter().zip(&bins) {
            let r = release::export(&self.project, name, *p, bin, out_dir, opts.zip).map_err(|e| format!("{}: {e:#}", p.label()))?;
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
        let mut msg = lines.join("\n");
        if let (Some(folder), true) = (&linux_folder, opts.appimage || opts.flatpak) {
            let r = release::Exported { folder: folder.clone(), archive: None, files: 0, bytes: 0, warnings: vec![] };
            let slug = release::slug(name);
            let size = |p: &std::path::Path| std::fs::metadata(p).map_or(0.0, |m| m.len() as f64 / 1_048_576.0);
            match package::icon(&self.project, &opts.icon) {
                Err(e) => msg += &format!("\nNo packages: {e:#}"),
                Ok(icon) => {
                    if opts.appimage {
                        match package::appimage(&r.folder, &slug, name.trim(), &icon, out_dir) {
                            Ok(p) => msg += &format!("\nAppImage: {} ({:.1} MB)", p.display(), size(&p)),
                            Err(e) => msg += &format!("\nAppImage failed: {e:#}"),
                        }
                    }
                    if opts.flatpak {
                        match package::flatpak(&r.folder, &slug, name.trim(), opts.app_id.trim(), &opts.runtime, &icon, out_dir) {
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

    pub fn export_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.export.take() else { return };
        let mut open = true;
        let mut go = false;
        let (_, check) = release::check(&self.project);
        egui::Window::new("Export game").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label("Creates a standalone copy of the game that runs without the editor: the game executable plus the project's data (maps, definitions, scripts and the images and sounds in use; never save games).");
            ui.weak("rustpg2d-LICENSE.txt (the MIT license of the R(ust)PG2D engine in the game executable) is included in every export and package. Your game's own license is up to you.");
            egui::Grid::new("export").num_columns(2).show(ui, |ui| {
                ui.label("Game name");
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(260.0));
                ui.end_row();
                ui.label("Output folder");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut d.out).desired_width(260.0));
                    if ui.button("Browse…").clicked() {
                        self.browse_request = Some(Purpose::ExportGameFolder);
                    }
                });
                ui.end_row();
                ui.label("Platforms");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut d.opts.linux, "Linux").on_hover_text("A folder with the game for Linux (x86_64): players run the executable in it");
                    ui.checkbox(&mut d.opts.windows, "Windows").on_hover_text("A folder with NAME.exe for Windows 10 / 11 (64-bit): players double-click it. Not signed, so Windows asks once (More info, Run anyway)");
                });
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut d.opts.zip, "Also create a .zip to share (one per platform)");
                ui.end_row();
                if !d.opts.linux || Platform::host() != Platform::Linux {
                    d.opts.appimage = false;
                    d.opts.flatpak = false;
                }
            });
            egui::Grid::new("export_packages").num_columns(2).show(ui, |ui| {
                if d.opts.linux && Platform::host() == Platform::Linux {
                    ui.label("Linux packages");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut d.opts.appimage, "AppImage").on_hover_text(
                            "One executable file that runs on most Linux distributions: players download it, make it executable and start it. The AppImage runtime is downloaded once (from github.com/AppImage) and cached.",
                        );
                        ui.add_enabled_ui(d.flatpak_ok, |ui| {
                            ui.checkbox(&mut d.opts.flatpak, "Flatpak bundle").on_hover_text(
                                "A sandboxed .flatpak file: players install it with `flatpak install --user GAME.flatpak` (Flatpak downloads the freedesktop runtime from Flathub) and start it from the app menu.",
                            );
                        });
                        if !d.flatpak_ok {
                            ui.weak("Flatpak bundles need the flatpak command (install Flatpak with your package manager).");
                        }
                    });
                    ui.end_row();
                }
                if d.opts.flatpak {
                    ui.label("Flatpak app id");
                    ui.horizontal(|ui| {
                        if ui.add(egui::TextEdit::singleline(&mut d.opts.app_id).desired_width(200.0)).changed() {
                            d.app_id_edited = true;
                        }
                        egui::ComboBox::from_id_salt("flatpak_runtime").selected_text(format!("runtime {}", d.opts.runtime)).show_ui(ui, |ui| {
                            for r in package::FLATPAK_RUNTIMES {
                                ui.selectable_value(&mut d.opts.runtime, r.to_string(), format!("freedesktop {r}"));
                            }
                        });
                    });
                    ui.end_row();
                }
                if d.opts.appimage || d.opts.flatpak {
                    ui.label("Icon");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.opts.icon).hint_text("the player character").desired_width(200.0));
                        if ui.button("Browse…").clicked() {
                            self.browse_request = Some(Purpose::ExportIcon);
                        }
                    });
                    ui.end_row();
                }
            });
            let slug = release::slug(&d.name);
            if !d.app_id_edited {
                d.opts.app_id = package::default_app_id(&slug);
            }
            if d.opts.flatpak {
                if let Err(e) = package::check_app_id(d.opts.app_id.trim()) {
                    ui.colored_label(Color32::LIGHT_RED, format!("App id {e}"));
                }
            }
            let out = d.out.trim_end_matches(['/', '\\']);
            let sep = std::path::MAIN_SEPARATOR;
            if d.opts.linux {
                ui.weak(format!("Linux: {out}{sep}{slug}{sep}{slug} (run it to play), saves go to {}", rustpg2d_engine::paths::saves_hint(&slug, false)));
            }
            if d.opts.windows {
                ui.weak(format!("Windows: {out}{sep}{}{sep}{slug}.exe, saves go to {}", Platform::Windows.folder(&slug), rustpg2d_engine::paths::saves_hint(&slug, true)));
            }
            if d.opts.appimage {
                ui.weak(format!("AppImage: {out}/{slug}-x86_64.AppImage"));
            }
            if d.opts.flatpak {
                ui.weak(format!("Flatpak: {out}/{slug}.flatpak (saves in ~/.var/app/{}/data/{slug}/saves)", d.opts.app_id.trim()));
            }
            if let (Some((note, wide)), true) = (&d.glibc, d.opts.linux) {
                if *wide {
                    ui.weak(note);
                } else {
                    ui.colored_label(Color32::YELLOW, note);
                }
            }
            for p in d.opts.platforms() {
                match game_binary(p) {
                    Some((b, false)) => ui.weak(format!("{} game runner: {}", p.label(), b.display())),
                    Some((b, true)) => ui.colored_label(Color32::YELLOW, format!("Only a debug build of the game was found ({}). It works but runs slower; `cargo build --release -p rustpg2d-game` gives a fast one.", b.display())),
                    None => ui.colored_label(Color32::LIGHT_RED, missing_runner(p)),
                };
            }
            for e in &check.errors {
                ui.colored_label(Color32::LIGHT_RED, format!("\u{E4F6} {e}"));
            }
            for w in &check.warnings {
                ui.colored_label(Color32::YELLOW, format!("\u{E4E0} {w}"));
            }
            if let Some(r) = &d.result {
                match r {
                    Ok(m) => ui.colored_label(Color32::LIGHT_GREEN, m),
                    Err(e) => ui.colored_label(Color32::LIGHT_RED, e),
                };
            }
            let app_id_ok = !d.opts.flatpak || package::check_app_id(d.opts.app_id.trim()).is_ok();
            let ok = check.errors.is_empty() && !d.name.trim().is_empty() && !d.out.trim().is_empty() && !d.opts.platforms().is_empty() && d.opts.platforms().into_iter().all(|p| game_binary(p).is_some()) && app_id_ok;
            if ui.add_enabled(ok, egui::Button::new("\u{E390} Export")).clicked() {
                go = true;
            }
        });
        if go {
            let r = self.run_export(&d.name, &d.out, &d.opts);
            self.set_status(match &r {
                Ok(m) => m.lines().next().unwrap_or_default().to_string(),
                Err(e) => format!("Export failed: {e}"),
            });
            d.result = Some(r);
        }
        if open {
            self.export = Some(d);
        }
    }
}
