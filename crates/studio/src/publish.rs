//! The Publishing page: the game exported to play without the editor. What it is called and
//! where it goes, for which platforms and as which packages; under that the pre-flight (what
//! stands in the way, what is worth knowing, what will be written) and the *Export* button,
//! which is there once nothing stands in the way.

use eframe::egui;
use editor_core::export::{ExportOpts, Preflight};
use editor_core::package;
use editor_core::Workspace;
use rustpg2d_engine::release::Platform;

#[derive(Default)]
pub struct PublishPage {
    /// The project the page was filled in for (another project: filled in again).
    project: std::path::PathBuf,
    filled: bool,
    pub name: String,
    pub out: String,
    pub opts: ExportOpts,
    /// The app id was typed by hand (else it follows the game's name).
    app_id_edited: bool,
    /// What the last export said.
    pub result: Option<Result<String, String>>,
    flatpak_ok: bool,
    /// The pre-flight as it was looked at last, and what it was looked at for.
    preflight: Preflight,
    key: String,
}

impl PublishPage {
    fn fill(&mut self, ws: &Workspace) {
        // (Once for a project: a name cleared to type another stays cleared.)
        if self.filled && self.project == ws.project.root {
            return;
        }
        self.filled = true;
        self.project = ws.project.root.clone();
        self.name = ws.project.config.name.clone();
        self.out = ws.default_export_dir().display().to_string();
        self.opts = ws.default_export_opts();
        self.app_id_edited = false;
        self.result = None;
        self.flatpak_ok = package::flatpak_available();
        self.key.clear();
    }

    /// The pre-flight for what is ticked now (looked at again when something changed).
    pub fn preflight(&mut self, ws: &Workspace) -> &Preflight {
        self.fill(ws);
        let key = format!("{}|{}|{:?}|{}|{}|{}", self.name, self.out, self.opts, ws.timeline.status(), ws.is_dirty(), ws.problems.status());
        if key != self.key {
            self.key = key;
            self.preflight = ws.export_preflight(&self.name, &self.out, &self.opts);
        }
        &self.preflight
    }

    /// Exports with what is set (the script's `publish go` and the button).
    pub fn export(&mut self, ws: &mut Workspace) {
        self.fill(ws);
        let r = ws.export_game(&self.name, &self.out, &self.opts);
        ws.set_status(match &r {
            Ok(m) => m.lines().next().unwrap_or_default().to_string(),
            Err(e) => format!("Export failed: {e}"),
        });
        self.result = Some(r);
        self.key.clear();
    }

    /// The script's `publish WHAT VALUE`: `name`, `out`, or a switch `linux|windows|macos|web|zip|pack|appimage|flatpak on|off`.
    pub fn set(&mut self, ws: &Workspace, what: &str, value: &str) -> Result<(), String> {
        self.fill(ws);
        let on = value == "on";
        match what {
            "name" => self.name = value.to_string(),
            "out" => self.out = value.to_string(),
            "linux" => self.opts.linux = on,
            "windows" => self.opts.windows = on,
            "macos" => self.opts.macos = on,
            "web" => self.opts.web = on,
            "zip" => self.opts.zip = on,
            "pack" => self.opts.pack = on,
            "appimage" => self.opts.appimage = on,
            "flatpak" => self.opts.flatpak = on,
            other => return Err(format!("publish {other}: name, out, linux, windows, macos, web, zip, pack, appimage, flatpak")),
        }
        Ok(())
    }

    /// `stops=N notes=N writes=N result=…` for the script's `status`.
    pub fn status(&self) -> String {
        format!("stops={} notes={} writes={} result={}", self.preflight.stops.len(), self.preflight.notes.len(), self.preflight.writes.len(), match &self.result {
            None => "-",
            Some(Ok(_)) => "ok",
            Some(Err(_)) => "failed",
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ws: &mut Workspace) {
        self.fill(ws);
        ui.label(egui::RichText::new("Publishing").heading());
        ui.label("A copy of the game that runs without the editor: the game's program and the project's data (maps, definitions, scripts, and the pictures, sounds and models in use; never save games).");
        ui.weak("rustpg2d-LICENSE.txt (the MIT license of the R(ust)PG2D engine in the game's program) is in every export and package. Your game's own license is yours to choose.");
        ui.add_space(6.0);
        let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui, egui::Id)| {
            ui.horizontal(|ui| {
                let id = ui.allocate_ui_with_layout(egui::vec2(150.0, ui.spacing().interact_size.y), egui::Layout::left_to_right(egui::Align::Center), |ui| ui.label(label).id).inner;
                add(ui, id);
            });
        };
        row(ui, "Game name", &mut |ui, by| {
            ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(300.0)).labelled_by(by);
        });
        row(ui, "Goes into the folder", &mut |ui, by| {
            ui.add(egui::TextEdit::singleline(&mut self.out).desired_width(520.0)).labelled_by(by);
            let start = Some(std::path::PathBuf::from(self.out.trim())).filter(|p| !p.as_os_str().is_empty());
            if let Some(picked) = crate::pick::browse(ui, crate::pick::Key::PublishOut, "Browse…", "The folder the game is exported into", || crate::pick::Want::Folder, start) {
                self.out = picked.display().to_string();
            }
        });
        row(ui, "For", &mut |ui, _| {
            ui.checkbox(&mut self.opts.linux, "Linux").on_hover_text("A folder with the game for Linux (x86_64): players run the program in it");
            ui.checkbox(&mut self.opts.windows, "Windows").on_hover_text("A folder with NAME.exe for Windows 10 / 11 (64-bit): players double-click it. Not signed, so Windows asks once (More info, Run anyway)");
            ui.checkbox(&mut self.opts.macos, "macOS").on_hover_text("An application NAME.app for macOS 11 or newer (Apple Silicon and Intel): players drag it to Applications and open it. Not signed by Apple, so macOS refuses the first start until the player allows it (System Settings, Privacy & Security, Open Anyway)");
            ui.checkbox(&mut self.opts.web, "Web (browser, itch.io)").on_hover_text("The game compiled to WebAssembly with an index.html: it runs in current browsers from any web server, or uploaded to itch.io as an HTML game (the zip). Saves stay in the player's browser");
        });
        row(ui, "", &mut |ui, _| {
            ui.checkbox(&mut self.opts.zip, "Also a .zip of each, to share");
            ui.checkbox(&mut self.opts.pack, "The game's files in one file (data.pack)").on_hover_text("The maps, definitions, scripts, pictures, sounds and models in one data.pack beside the program instead of a data folder with every file: a tidy download, nothing to delete or change by accident. (It keeps casual eyes out; it is no protection.)");
        });
        let linux_here = self.opts.linux && Platform::host() == Platform::Linux;
        if !linux_here {
            self.opts.appimage = false;
            self.opts.flatpak = false;
        }
        if linux_here {
            let flatpak_ok = self.flatpak_ok;
            row(ui, "Linux packages", &mut |ui, _| {
                ui.checkbox(&mut self.opts.appimage, "AppImage").on_hover_text("One file that runs on most Linux distributions: players download it, make it executable and start it. The AppImage runtime is downloaded once (from github.com/AppImage) and kept");
                ui.add_enabled(flatpak_ok, egui::Checkbox::new(&mut self.opts.flatpak, "Flatpak bundle")).on_hover_text("A sandboxed .flatpak file: players install it with `flatpak install --user GAME.flatpak` and start it from the app menu").on_disabled_hover_text("Flatpak bundles need the flatpak command (install Flatpak with your package manager)");
            });
        }
        if self.opts.flatpak {
            row(ui, "Flatpak app id", &mut |ui, by| {
                if ui.add(egui::TextEdit::singleline(&mut self.opts.app_id).desired_width(260.0)).labelled_by(by).changed() {
                    self.app_id_edited = true;
                }
                egui::ComboBox::from_id_salt("flatpak runtime").selected_text(format!("runtime {}", self.opts.runtime)).show_ui(ui, |ui| {
                    for r in package::FLATPAK_RUNTIMES {
                        ui.selectable_value(&mut self.opts.runtime, r.to_string(), format!("freedesktop {r}"));
                    }
                });
            });
        }
        if !self.app_id_edited {
            self.opts.app_id = package::default_app_id(&rustpg2d_engine::release::slug(&self.name));
        }
        if self.opts.appimage || self.opts.flatpak || self.opts.macos {
            row(ui, "Icon", &mut |ui, by| {
                ui.add(egui::TextEdit::singleline(&mut self.opts.icon).hint_text("a PNG file; none: the hero's portrait or 3D model").desired_width(420.0)).labelled_by(by);
            });
        }

        // Whose work is in the game: the packs its assets came from, with their licences.
        let used = ws.packs_used();
        if !used.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.label(egui::RichText::new("Packs in this game").strong());
            ui.weak("What came from packs of the asset library, and what their makers ask for. The credit lines belong on the game's Credits page.");
            for (_, pack, n) in &used {
                ui.horizontal_wrapped(|ui| {
                    ui.label(pack.title.as_str());
                    ui.weak(format!("{}{} · {n} asset{}", if pack.author.is_empty() { String::new() } else { format!("by {} · ", pack.author) }, if pack.licence.is_empty() { "no licence named".to_string() } else { pack.licence.clone() }, if *n == 1 { "" } else { "s" }));
                });
                ui.weak(format!("    Credit line: {}", pack.line()));
            }
            let missing = ws.credits_missing();
            ui.horizontal(|ui| {
                if ui.add_enabled(!missing.is_empty(), egui::Button::new(match missing.len() {
                    0 => "The credits hold them all".to_string(),
                    1 => "Add its credit line to the credits".to_string(),
                    n => format!("Add {n} credit lines to the credits"),
                })).on_hover_text("Puts the lines at the end of the game's credits (Title & menus): players read them on the Credits page").clicked() {
                    let n = ws.add_pack_credits();
                    ws.set_status(format!("Added {n} credit line{} to the game's credits", if n == 1 { "" } else { "s" }));
                }
            });
        }

        // The pre-flight.
        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new("Before it goes out").strong());
        let pre = self.preflight(ws).clone();
        for s in &pre.stops {
            ui.colored_label(ui.visuals().error_fg_color, s.as_str());
        }
        for n in &pre.notes {
            ui.colored_label(ui.visuals().warn_fg_color, n.as_str());
        }
        if pre.stops.is_empty() && pre.notes.is_empty() {
            ui.weak("Nothing stands in the way.");
        }
        if !pre.writes.is_empty() {
            ui.add_space(4.0);
            ui.weak("What is written:");
            for w in &pre.writes {
                ui.weak(w.as_str());
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let r = ui.add_enabled(pre.stops.is_empty(), egui::Button::new(egui::RichText::new("Export").strong())).on_hover_text("Saves everything, then writes the game (the window waits while it is written)").on_disabled_hover_text("Something stands in the way: see above");
            if r.clicked() {
                self.export(ws);
            }
            if ui.button("Check the project").on_hover_text("Looks for what players would run into: missing things, dead ends, scripts that don't compile").clicked() {
                let _ = ws.run_line("check.problems");
            }
        });
        match &self.result {
            Some(Ok(m)) => {
                ui.add_space(6.0);
                for line in m.lines() {
                    ui.colored_label(egui::Color32::from_rgb(140, 220, 140), line);
                }
            }
            Some(Err(e)) => {
                ui.add_space(6.0);
                ui.colored_label(ui.visuals().error_fg_color, e.as_str());
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    struct State {
        ws: Workspace,
        page: PublishPage,
    }

    #[test]
    fn the_page_says_what_is_written_and_what_stands_in_the_way() {
        // (The example itself: nothing is written while a runner is missing, and a test's own
        // program has none beside it.)
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_project");
        let ws = Workspace::new(dir, editor_core::Gpu::No).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 900.0)).build_ui_state(|ui, s: &mut State| s.page.show(ui, &mut s.ws), State { ws, page: PublishPage::default() });
        h.run();
        // Filled in from the project: its name, a folder beside it, this computer's platform.
        assert_eq!(h.state().page.name, h.state().ws.project.config.name);
        assert!(h.state().page.out.ends_with("releases"));
        assert!(h.query_all_by_label("Before it goes out").next().is_some());
        let writes = |h: &Harness<'static, State>| h.state().page.preflight.writes.len();
        assert_eq!(writes(&h), 1);
        // Another platform ticked: one more thing written, and its runner asked for.
        h.get_by_label("Web (browser, itch.io)").click();
        h.run();
        assert!(h.state().page.opts.web);
        assert_eq!(writes(&h), 2);
        assert!(h.state().page.preflight.writes[1].starts_with("Web: "));
        // The app id follows the game's name until it is typed by hand.
        h.state_mut().page.name = "Knight Tales".into();
        h.run();
        assert_eq!(h.state().page.opts.app_id, "org.rustpg2d.knight_tales");
        // Without a name nothing goes out.
        h.state_mut().page.name.clear();
        h.run();
        assert!(h.state().page.preflight.stops.iter().any(|s| s.contains("needs a name")));
        assert!(h.query_all_by_label_contains("The game needs a name").next().is_some());
    }
    #[test]
    fn the_packs_in_the_game_are_listed_and_their_credit_lines_added() {
        // A new project in which the hero came from a pack (as adding him from the library notes it).
        let dir = std::env::temp_dir().join(format!("rustpg2d-publish-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        editor_core::files::create_project(&dir).unwrap();
        let mut ws = Workspace::new(dir.clone(), editor_core::Gpu::No).unwrap();
        ws.make_def(rustpg2d_engine::refactor::RefKind::Character, "Hero", None).unwrap();
        ws.meta.assets.entry("character:hero".into()).or_default().origin = Some(editor_core::meta::Origin { pack: "anna.heroes".into(), version: "1.2".into() });
        ws.meta.packs.insert("anna.heroes".into(), editor_core::meta::PackCredit { title: "Heroes".into(), author: "Anna".into(), licence: "CC BY 4.0".into(), attribution: "Hero figures by Anna (CC BY 4.0)".into() });
        ws.history_frame(None);
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 1000.0)).build_ui_state(
            |ui, s: &mut State| {
                s.ws.history_frame(None);
                s.page.show(ui, &mut s.ws)
            },
            State { ws, page: PublishPage::default() },
        );
        h.run();
        // The pack is listed with what its maker asks for, and the pre-flight says the credits lack it.
        h.get_by_label("Packs in this game");
        h.get_by_label("Heroes");
        assert!(h.query_all_by_label_contains("by Anna · CC BY 4.0 · 1 asset").next().is_some());
        assert!(h.query_all_by_label_contains("Credit line: Hero figures by Anna (CC BY 4.0)").next().is_some());
        assert!(h.state().page.preflight.notes.iter().any(|n| n.contains("credits don't hold: Hero figures by Anna")), "{:?}", h.state().page.preflight.notes);
        // One click puts the line into the game's credits, as a step; nothing is left to add.
        h.get_by_label("Add its credit line to the credits").click();
        h.run();
        h.run();
        let s = h.state();
        assert_eq!(s.ws.project.config.credits, "Hero figures by Anna (CC BY 4.0)");
        assert!(s.ws.timeline.done.last().is_some_and(|c| c.label.contains("credit lines")));
        assert!(!s.page.preflight.notes.iter().any(|n| n.contains("credits don't hold")));
        h.get_by_label("The credits hold them all");
        let _ = std::fs::remove_dir_all(dir);
    }

}
