//! File → Export game: writes a standalone bundle (game executable + project data), and on
//! request a zip, an AppImage and a Flatpak bundle of it (package.rs).

use std::path::PathBuf;

use egui_macroquad::egui::{self, Color32};
use rustpg2d_engine::release;

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

impl Default for ExportOpts {
    fn default() -> Self {
        ExportOpts { zip: false, appimage: false, flatpak: false, app_id: String::new(), runtime: package::FLATPAK_RUNTIMES[0].into(), icon: String::new() }
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

/// The rustpg2d-game executable to ship: a release build if there is one, else the one next to
/// the editor. Returns (path, is a debug build).
pub fn game_binary() -> Option<(PathBuf, bool)> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let next_to = dir.join("rustpg2d-game");
    // Inside a cargo target folder, prefer target/release over target/debug.
    if dir.file_name().is_some_and(|n| n == "debug") {
        let rel = dir.parent()?.join("release/rustpg2d-game");
        if rel.is_file() {
            return Some((rel, false));
        }
        return next_to.is_file().then_some((next_to, true));
    }
    next_to.is_file().then_some((next_to, false))
}

impl Editor {
    pub fn open_export(&mut self) {
        let out = self.project.root.canonicalize().ok().and_then(|r| r.parent().map(|p| p.join("releases"))).unwrap_or_else(|| PathBuf::from("releases"));
        let name = self.project.config.name.clone();
        let opts = ExportOpts { zip: true, app_id: package::default_app_id(&release::slug(&name)), ..Default::default() };
        self.export = Some(ExportDialog { name, out: out.display().to_string(), opts, result: None, flatpak_ok: package::flatpak_available(), app_id_edited: false, glibc: game_binary().and_then(|(p, _)| glibc_note(&p)) });
    }

    /// Saves everything, then exports (the folder, then the packages asked for). Returns a
    /// summary.
    pub fn run_export(&mut self, name: &str, out: &str, opts: &ExportOpts) -> Result<String, String> {
        self.save_all();
        if self.doc.as_ref().is_some_and(|d| d.is_dirty()) || self.defs_dirty {
            return Err("Unsaved changes could not be saved; nothing was exported.".into());
        }
        let Some((bin, debug)) = game_binary() else {
            return Err("rustpg2d-game was not found next to the editor. Build it with `cargo build --release -p rustpg2d-game`.".into());
        };
        let out_dir = std::path::Path::new(out);
        let r = release::export(&self.project, name, &bin, out_dir, opts.zip).map_err(|e| format!("{e:#}"))?;
        let mut msg = format!("Exported to {} ({} files, {:.1} MB)", r.folder.display(), r.files, r.bytes as f64 / 1_048_576.0);
        if let Some(a) = &r.archive {
            msg += &format!("\nArchive: {}", a.display());
        }
        if opts.appimage || opts.flatpak {
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
        for w in r.warnings {
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
                ui.label("");
                ui.checkbox(&mut d.opts.zip, "Also create a .zip to share");
                ui.end_row();
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
            let out = d.out.trim_end_matches('/');
            ui.weak(format!("Result: {out}/{slug}/{slug} (run it to play), saves go to ~/.local/share/{slug}/saves"));
            if d.opts.appimage {
                ui.weak(format!("AppImage: {out}/{slug}-x86_64.AppImage"));
            }
            if d.opts.flatpak {
                ui.weak(format!("Flatpak: {out}/{slug}.flatpak (saves in ~/.var/app/{}/data/{slug}/saves)", d.opts.app_id.trim()));
            }
            if let Some((note, wide)) = &d.glibc {
                if *wide {
                    ui.weak(note);
                } else {
                    ui.colored_label(Color32::YELLOW, note);
                }
            }
            match game_binary() {
                Some((p, false)) => ui.weak(format!("Game executable: {}", p.display())),
                Some((p, true)) => ui.colored_label(Color32::YELLOW, format!("Only a debug build of the game was found ({}). It works but runs slower; `cargo build --release -p rustpg2d-game` gives a fast one.", p.display())),
                None => ui.colored_label(Color32::LIGHT_RED, "rustpg2d-game not found next to the editor; build it with `cargo build --release -p rustpg2d-game`."),
            };
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
            let ok = check.errors.is_empty() && !d.name.trim().is_empty() && !d.out.trim().is_empty() && game_binary().is_some() && app_id_ok;
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
