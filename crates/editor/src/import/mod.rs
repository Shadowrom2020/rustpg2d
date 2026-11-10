//! File › Import…: bring existing assets in. Tiled maps (`.tmx` / `.tmj`) become maps with
//! their tilesets, LPC character sheets become character-layout sheets (bodies, hair,
//! clothing…), Aseprite exports become sprite sheets with named animations. Imports never
//! overwrite: files and ids get a free name. The result (what was imported, what was left out)
//! is shown in a window.

pub mod aseprite;
pub mod lpc;
pub mod tiled;

use std::path::Path;

use anyhow::{bail, Context, Result};
use egui_macroquad::egui;
use rustpg2d_engine::defs::{SheetDef, SheetKind};
use rustpg2d_engine::imageio::Rgba;

use crate::app::Editor;
use crate::ui::{self, ph};

/// The project's (fixed) tile size.
pub const TS: u32 = rustpg2d_engine::project::TILE_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportKind {
    Tiled,
    Lpc,
    Aseprite,
}

impl ImportKind {
    pub const ALL: [ImportKind; 3] = [ImportKind::Tiled, ImportKind::Lpc, ImportKind::Aseprite];
    pub fn label(self) -> &'static str {
        match self {
            ImportKind::Tiled => "Tiled map",
            ImportKind::Lpc => "LPC character sheet",
            ImportKind::Aseprite => "Aseprite sprite sheet",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
            ImportKind::Tiled => "A .tmx or .tmj map (orthogonal, not infinite). Its tilesets are added (scaled to 32 px tiles), tile layers become layers (names with \"over\" / \"above\" draw above characters), a layer named \"collision\" marks solid tiles, tiles with a \"solid\" property block, rectangles of object layers become trigger zones and points named spawn:NAME spawn points. The map is saved and opened.",
            ImportKind::Lpc => "A Universal LPC Spritesheet (64×64 frames, 13 wide, 21+ rows) or a 4-row LPC walk cycle. The walk animation becomes a character sheet (rows down, left, right, up; 4 frames); the other animations are left out. Colors stay as they are (no skin / hair recoloring).",
            ImportKind::Aseprite => "The JSON exported with Aseprite's File › Export Sprite Sheet (array or hash), next to its PNG. Frames are packed into a grid; tags become named animations offered by the Effect / Fly actions and effect(…, #{ tag: \"name\" }).",
        }
    }
    fn filter(self) -> (&'static str, &'static [&'static str]) {
        match self {
            ImportKind::Tiled => ("Tiled map", &["tmx", "tmj", "json"]),
            ImportKind::Lpc => ("PNG image", &["png"]),
            ImportKind::Aseprite => ("Aseprite JSON", &["json"]),
        }
    }
}

/// File dialog filter of an import.
pub fn file_filter(kind: ImportKind) -> (&'static str, &'static [&'static str]) {
    kind.filter()
}

/// The File › Import… window: what to import and, for sheets, the id and kind.
pub struct ImportDialog {
    pub kind: ImportKind,
    /// Map name / sheet id ("" = from the file name).
    pub id: String,
    pub sheet_kind: SheetKind,
    pub female: bool,
    /// The new sheet's scale (`SheetDef::scale`: 2 = high-resolution art).
    pub scale: u32,
}

impl Default for ImportDialog {
    fn default() -> Self {
        ImportDialog { kind: ImportKind::Tiled, id: String::new(), sheet_kind: SheetKind::Body, female: false, scale: 1 }
    }
}

/// An id from any text: lower case letters, digits and underscores.
pub fn sanitize(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_string();
    if out.is_empty() {
        "imported".into()
    } else {
        out
    }
}

pub use rustpg2d_engine::refactor::unique_id;

/// Writes `bytes` to `<root>/<subdir>/<stem>.<ext>` under a file name not used yet; returns
/// the project-relative path.
pub fn write_new(root: &Path, subdir: &str, stem: &str, ext: &str, bytes: &[u8]) -> Result<String> {
    let dir = root.join(subdir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let name = unique_id(stem, |c| dir.join(format!("{c}.{ext}")).exists());
    std::fs::write(dir.join(format!("{name}.{ext}")), bytes).with_context(|| format!("writing {subdir}/{name}.{ext}"))?;
    Ok(format!("{subdir}/{name}.{ext}"))
}

fn stem(path: &Path) -> String {
    sanitize(&path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
}

impl Editor {
    /// Imports `path`; `id` = map name / sheet id ("" = from the file name). Returns the report.
    /// `scale`: the new sheet's `SheetDef::scale` (LPC, Aseprite).
    pub fn run_import(&mut self, kind: ImportKind, path: &Path, id: &str, sheet_kind: SheetKind, female: bool, scale: u32) -> Result<String> {
        let scale = scale.clamp(1, 4);
        if !self.project_open {
            bail!("open a project first");
        }
        let root = self.project.root.clone();
        let base = if id.trim().is_empty() { stem(path) } else { sanitize(id) };
        let msg = match kind {
            ImportKind::Tiled => {
                let parsed = tiled::parse_file(path)?;
                let maps = self.maps.clone();
                let name = unique_id(&base, |c| maps.iter().any(|m| m == c) || root.join("maps").join(format!("{c}.map")).exists());
                let (map, report) = tiled::convert(&parsed, &name, &root, &mut self.project.db)?;
                self.project.save_map(&map).context("saving the map")?;
                let msg = report.message(&map);
                self.after_import();
                if !self.maps.contains(&name) {
                    self.maps.push(name.clone());
                    self.maps.sort();
                }
                self.open_map(&name);
                self.mode = crate::app::Mode::Map;
                msg
            }
            ImportKind::Lpc => {
                let img = Rgba::decode(&std::fs::read(path).with_context(|| format!("reading {}", path.display()))?)?;
                let (sheet, note) = lpc::convert(&img)?;
                let id = unique_id(&base, |c| self.project.db.sheet(c).is_some());
                let rel = write_new(&root, "sprites", &id, "png", &sheet.encode()?)?;
                self.project.db.sheets.push(SheetDef { kind: sheet_kind, female: female && sheet_kind == SheetKind::Body, scale, ..SheetDef::new(&id, &rel, lpc::FRAME, lpc::FRAME) });
                self.after_import();
                let mut msg = format!(
                    "Imported LPC sheet '{id}' as {} ({rel}: the walk cycle, 4 frames per direction, 64×64). Colors stay as drawn (no skin / hair recoloring).",
                    sheet_kind.label()
                );
                if sheet_kind == SheetKind::Body {
                    msg += " Use it in Database › Characters › Look & equipment (Body); LPC hair and clothing of the same size fit on it.";
                }
                if !note.is_empty() {
                    msg += &format!(" Note: {note}.");
                }
                msg
            }
            ImportKind::Aseprite => {
                let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
                let parsed = aseprite::parse(&text)?;
                let dir = path.parent().unwrap_or(Path::new("."));
                let image = if parsed.image.is_empty() { path.with_extension("png") } else { dir.join(&parsed.image) };
                let img = Rgba::decode(&std::fs::read(&image).with_context(|| format!("reading the sheet image {}", image.display()))?)?;
                let (sheet, tags, notes) = aseprite::build(&parsed, &img);
                let id = unique_id(&base, |c| self.project.db.sheet(c).is_some());
                let rel = write_new(&root, "sprites", &id, "png", &sheet.encode()?)?;
                let names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();
                self.project.db.sheets.push(SheetDef { tags, scale, ..SheetDef::new(&id, &rel, parsed.frame_w, parsed.frame_h) });
                self.after_import();
                let mut msg = format!(
                    "Imported sprite sheet '{id}' ({rel}: {} frames of {}×{}; animations: {}).",
                    parsed.frames.len(),
                    parsed.frame_w,
                    parsed.frame_h,
                    if names.is_empty() { "none".into() } else { names.join(", ") }
                );
                if !notes.is_empty() {
                    msg += &format!(" Note: {}.", notes.join("; "));
                }
                msg
            }
        };
        Ok(msg)
    }

    /// New definitions are saved right away (an imported map refers to them) and loaded.
    fn after_import(&mut self) {
        self.save_defs();
        self.reload_assets();
    }

    /// Runs an import and shows its result.
    pub fn import_and_report(&mut self, kind: ImportKind, path: &Path) {
        let d = self.import.take().unwrap_or_default();
        let r = self.run_import(kind, path, &d.id, d.sheet_kind, d.female, d.scale);
        let text = match &r {
            Ok(m) => m.clone(),
            Err(e) => format!("Import failed: {e:#}"),
        };
        self.import_report = Some((r.is_ok(), text));
        self.show_result(r);
    }

    /// File › Import… window and the result window afterwards.
    pub fn import_window(&mut self, ctx: &egui::Context) {
        let mut choose = None;
        let mut close = false;
        if let Some(d) = &mut self.import {
            egui::Window::new(ui::t(ph::DOWNLOAD_SIMPLE, "Import")).collapsible(false).resizable(false).default_width(460.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for k in ImportKind::ALL {
                        ui.selectable_value(&mut d.kind, k, k.label());
                    }
                });
                ui.add(egui::Label::new(egui::RichText::new(d.kind.help()).weak()).wrap());
                ui.separator();
                egui::Grid::new("import_opts").num_columns(2).show(ui, |ui| {
                    ui.label(if d.kind == ImportKind::Tiled { "Map name" } else { "Sheet id" });
                    ui.add(egui::TextEdit::singleline(&mut d.id).hint_text("from the file name").desired_width(200.0));
                    ui.end_row();
                    if d.kind == ImportKind::Lpc {
                        ui.label("Kind");
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("import_kind").selected_text(d.sheet_kind.label()).show_ui(ui, |ui| {
                                for k in SheetKind::ALL.into_iter().filter(|k| *k != SheetKind::Sprite) {
                                    ui.selectable_value(&mut d.sheet_kind, k, k.label());
                                }
                            });
                            if d.sheet_kind == SheetKind::Body {
                                ui.checkbox(&mut d.female, "female body");
                            }
                        });
                        ui.end_row();
                    }
                    if d.kind != ImportKind::Tiled {
                        ui.label("Scale");
                        ui.horizontal(|ui| {
                            for k in 1..=4u32 {
                                ui.radio_value(&mut d.scale, k, format!("{k}\u{00D7}"));
                            }
                        })
                        .response
                        .on_hover_text("2\u{00D7} for high-resolution art: the frames are drawn at half their pixel size (64\u{00D7}96 fills a 32\u{00D7}48 place)");
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(ui::t(ph::FOLDER_OPEN, "Choose file…")).clicked() {
                        choose = Some(d.kind);
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        if close {
            self.import = None;
        }
        if let Some(k) = choose {
            self.open_file_dialog(crate::files::Purpose::Import(k));
        }
        let mut ok = false;
        if let Some((good, text)) = &self.import_report {
            egui::Window::new(if *good { "Import done" } else { "Import failed" }).collapsible(false).resizable(false).default_width(480.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                let col = if *good { ui.visuals().text_color() } else { egui::Color32::from_rgb(255, 140, 120) };
                ui.add(egui::Label::new(egui::RichText::new(text).color(col)).wrap());
                ok = ui.button("OK").clicked();
            });
        }
        if ok {
            self.import_report = None;
        }
    }
}
