//! *Assets › Starter asset library…*: installs the starter asset library ([`crate::starter`]),
//! downloaded from the release of this editor's version or loaded from a file. Offered once per
//! start while none is installed.

use egui_macroquad::egui::{self, Color32};

use crate::app::Editor;
use crate::files::Purpose;
use crate::starter::{self, Download};
use crate::ui::{self, ph};

/// The window's state.
#[derive(Default)]
pub struct StarterUi {
    pub open: bool,
    /// Offered at start already (or not needed).
    pub asked: bool,
    pub download: Option<Download>,
    /// The last outcome: (success, message).
    pub message: Option<(bool, String)>,
}

impl StarterUi {
    /// `starter=` for `status`: the starter library, then the window's state.
    pub fn status(&self) -> String {
        let state = if self.download.is_some() {
            "downloading".to_string()
        } else {
            match &self.message {
                Some((true, _)) => "installed".into(),
                Some((false, _)) => "failed".into(),
                None if self.open => "open".into(),
                None => "closed".into(),
            }
        };
        format!("{}/{state}", starter::status())
    }
}

impl Editor {
    /// Installs a starter library file the creator picked (or an editor script named).
    pub fn install_starter_file(&mut self, path: &std::path::Path) {
        let r = std::fs::read(path).map_err(anyhow::Error::from).and_then(|b| starter::install(&b));
        self.starter_installed(r);
    }

    /// After an install: the asset library gets the new assets, the library views reload.
    fn starter_installed(&mut self, r: anyhow::Result<String>) {
        let r = r.and_then(|v| starter::apply_to_library().map(|_| v));
        let msg = match &r {
            Ok(v) => format!("Installed the starter asset library {v}: its assets are in your asset library."),
            Err(e) => format!("The starter asset library wasn't installed: {e:#}"),
        };
        println!("[starter] {msg}");
        if r.is_ok() {
            self.lib.lib = None;
            self.lib.error = None;
            self.lib.tex_dirty = true;
        }
        self.set_status(msg.clone());
        self.starter_ui.message = Some((r.is_ok(), msg));
    }

    /// The window, offered at start while no starter library is installed.
    pub fn starter_window(&mut self, ctx: &egui::Context) {
        if !self.starter_ui.asked {
            self.starter_ui.asked = true;
            if !self.library_mode && starter::current().is_none() {
                self.starter_ui.open = true;
            }
        }
        if let Some(r) = self.starter_ui.download.as_ref().and_then(|d| d.poll()) {
            self.starter_ui.download = None;
            self.starter_installed(r);
        }
        if !self.starter_ui.open {
            return;
        }
        let mut open = true;
        let (mut download, mut from_file, mut close) = (false, false, false);
        let current = starter::current();
        egui::Window::new(ui::t(ph::PACKAGE, "Starter asset library")).id(egui::Id::new("starter_library")).open(&mut open).default_width(520.0).collapsible(false).show(ctx, |ui| {
            ui.label("The starter asset library holds everything the example game is made of: ground materials, props, characters and their 3D models, items, sounds and music, structures, fonts (also for Chinese, Japanese and Korean). New projects start with some of them, and the Asset Library offers all of them for your games.");
            ui.add_space(4.0);
            match &current {
                Some(s) => {
                    ui.label(egui::RichText::new(format!("In use: version {}", s.describe())).strong());
                }
                None => {
                    ui.label(egui::RichText::new("Not installed yet. It is a download of its own (about 40 MB), made for each version of R(ust)PG2D.").strong());
                }
            }
            if let Some(d) = &self.starter_ui.download {
                let (got, total) = d.progress();
                let mb = |b: u64| b as f32 / 1_048_576.0;
                let text = if total > 0 { format!("{:.1} of {:.1} MB", mb(got), mb(total)) } else { format!("{:.1} MB", mb(got)) };
                ui.add(egui::ProgressBar::new(if total > 0 { got as f32 / total as f32 } else { 0.0 }).text(text).animate(true));
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            if let Some((ok, msg)) = &self.starter_ui.message {
                ui.colored_label(if *ok { Color32::from_rgb(120, 200, 120) } else { Color32::from_rgb(235, 110, 100) }, msg);
            }
            if starter::is_repo_folder() {
                ui.weak("Repository mode: the starter library is the checkout's folder (crates/editor/assets/starter_library).");
                if ui.button("Close").clicked() {
                    close = true;
                }
                return;
            }
            ui.horizontal(|ui| {
                let version = env!("CARGO_PKG_VERSION");
                if ui.button(ui::t(ph::DOWNLOAD_SIMPLE, &format!("Download for R(ust)PG2D {version}"))).on_hover_text("Downloads it from R(ust)PG2D's releases on GitHub, checks it and installs it").clicked() {
                    download = true;
                }
                if ui.button(ui::t(ph::FOLDER_OPEN, "Load from file…")).on_hover_text(format!("Installs a starter library file you downloaded ({})", rustpg2d_engine::starter_pack::file_name(version))).clicked() {
                    from_file = true;
                }
                if ui.button(if current.is_some() { "Close" } else { "Not now" }).clicked() {
                    close = true;
                }
            });
            ui.weak(format!("It is kept in {}.", starter::install_dir().display()));
        });
        if download {
            self.starter_ui.message = None;
            self.starter_ui.download = Some(Download::start());
        }
        if from_file {
            self.starter_ui.message = None;
            self.open_file_dialog(Purpose::StarterLibrary);
        }
        if !open || close {
            self.starter_ui.open = false;
        }
    }
}
